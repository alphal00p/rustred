//! `rustred p-generate` (owner: M2-T7c rr-custom).
//!
//! ```text
//! rustred p-generate --families <json> --max-loops <k> --out <dir>
//!                    [--owner <class label>]... [--threads <n>] [--force]
//! ```
//!
//! Generates the Custom owners of the library program of the tops with at most
//! `k` loops (`rustred::propagator::custom`; at `k = 3` NO and t1star05) with
//! the false-knock-out guard, each in its own presentation as assembly
//! presents it, and writes per owner `<dir>/<label>.rrbin`, the payload (a
//! generated candidate bundle, readable by `rustred candidates inspect`), and
//! `<dir>/<label>.json`, its owner record (`lp_count`, `residuals`, two-sided
//! axes, charts) with what the generation measured. `--owner` restricts the run
//! to the named classes. A refused owner (guard, failed chart) fails the
//! command with its complete message; nothing of it is written.
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;

use rustred::propagator::class::{ClassLimits, classify};
use rustred::propagator::custom::custom_owner_support;
use rustred::propagator::error::PError;
use rustred::propagator::presentation::PresentationKind;
use rustred::propagator::{P_CUSTOM_OWNER_VERSION, P_OWNER_LIBRARY_SEMANTICS};

use crate::AppError;
use crate::application::propagator::generate::{
    CustomOwnerNode, CustomOwnerOptions, custom_owner_nodes, generate_custom_owner_with_report,
};
use crate::cli::args::{ArgError, StreamPath, next_utf8_value, next_value, parse_positive_integer};
use crate::cli::error::CliError;
use crate::cli::io::{preflight_output_destination, read_input, write_output};

/// Schema of the `<label>.json` owner record.
pub(crate) const P_CUSTOM_OWNER_RECORD_SCHEMA: &str = "rustred.p-custom-owner.v1";

#[derive(Debug, PartialEq, Eq)]
struct Arguments {
    families: PathBuf,
    max_loops: u8,
    out: PathBuf,
    owners: BTreeSet<String>,
    threads: usize,
    force: bool,
}

fn parse(arguments: Vec<OsString>) -> Result<Arguments, ArgError> {
    let mut arguments = arguments.into_iter();
    let (mut families, mut max_loops, mut out) = (None, None, None);
    let (mut owners, mut threads, mut force) = (BTreeSet::new(), 1, false);
    let mut seen = BTreeSet::new();
    while let Some(argument) = arguments.next() {
        let argument = argument.into_string().map_err(ArgError::NonUtf8Option)?;
        let option: &'static str = match argument.as_str() {
            "--families" => "--families",
            "--max-loops" => "--max-loops",
            "--out" => "--out",
            "--owner" => "--owner",
            "--threads" => "--threads",
            "--force" => "--force",
            _ => return Err(ArgError::UnknownOption(argument)),
        };
        if option != "--owner" && !seen.insert(option) {
            return Err(ArgError::DuplicateOption(option));
        }
        match option {
            "--families" => families = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--out" => out = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--owner" => {
                let label = next_utf8_value(&mut arguments, option)?;
                if !owners.insert(label) {
                    return Err(ArgError::DuplicateOption(option));
                }
            }
            "--max-loops" => {
                let value = next_utf8_value(&mut arguments, option)?;
                max_loops = Some(parse_positive_integer(option, value.clone()).and_then(
                    |loops| {
                        u8::try_from(loops).map_err(|_| ArgError::InvalidValue {
                            option,
                            value,
                            expected: "a loop count below 256",
                        })
                    },
                )?);
            }
            "--threads" => {
                threads = parse_positive_integer(option, next_utf8_value(&mut arguments, option)?)?;
            }
            _ => force = true,
        }
    }
    Ok(Arguments {
        families: families.ok_or(ArgError::MissingRequiredOption("--families"))?,
        max_loops: max_loops.ok_or(ArgError::MissingRequiredOption("--max-loops"))?,
        out: out.ok_or(ArgError::MissingRequiredOption("--out"))?,
        owners,
        threads,
        force,
    })
}

fn failed(error: PError) -> CliError {
    CliError::Application(AppError::derivation(format!("p generate: {error}")))
}

fn mask_text(mask: u64, arity: usize) -> String {
    (0..arity)
        .map(|slot| if mask >> slot & 1 == 1 { '1' } else { '0' })
        .collect()
}

/// The class label of a Custom node's owner sector.
fn label(node: &CustomOwnerNode) -> Result<String, PError> {
    let presentation = &node.presentation;
    let support = custom_owner_support(presentation);
    let classed = classify(presentation, support, &ClassLimits::default())?.ok_or_else(|| {
        PError::Generation(format!(
            "the Custom node {} has no class geometry",
            node.node.0
        ))
    })?;
    Ok(classed.key.label())
}

fn kind(node: &CustomOwnerNode) -> &'static str {
    match node.presentation.kind {
        PresentationKind::Induced { .. } => "induced",
        PresentationKind::EpsNode => "eps-node",
        _ => "other",
    }
}

pub(super) fn run(arguments: Vec<OsString>) -> Result<(), CliError> {
    let arguments = parse(arguments).map_err(CliError::Usage)?;
    let text = read_input(&StreamPath::File(arguments.families.clone()))?;
    let nodes = custom_owner_nodes(&text, arguments.max_loops).map_err(failed)?;
    let mut selected = Vec::new();
    for node in nodes {
        let label = label(&node).map_err(failed)?;
        if arguments.owners.is_empty() || arguments.owners.contains(&label) {
            selected.push((label, node));
        }
    }
    let found: BTreeSet<&String> = selected.iter().map(|(label, _)| label).collect();
    if let Some(missing) = arguments.owners.iter().find(|label| !found.contains(label)) {
        return Err(CliError::Input(format!(
            "{missing} is no Custom owner of the tops with at most {} loops",
            arguments.max_loops
        )));
    }
    std::fs::create_dir_all(&arguments.out).map_err(|error| {
        CliError::OutputIo(format!(
            "cannot create {}: {error}",
            arguments.out.display()
        ))
    })?;
    let destination = |label: &str, extension: &str| {
        StreamPath::File(arguments.out.join(format!("{label}.{extension}")))
    };
    for (label, _) in &selected {
        for extension in ["rrbin", "json"] {
            preflight_output_destination(&destination(label, extension), arguments.force)?;
        }
    }
    let options = CustomOwnerOptions {
        support: None,
        n_cores: arguments.threads,
    };
    let mut stdout = std::io::stdout().lock();
    let report_error = |error: std::io::Error| CliError::OutputIo(error.to_string());
    for (label, node) in &selected {
        let presentation = &node.presentation;
        let arity = presentation.slots.len();
        let (payload, report) =
            generate_custom_owner_with_report(presentation, &options).map_err(failed)?;
        let meta = &payload.meta;
        let record = serde_json::json!({
            "schema": P_CUSTOM_OWNER_RECORD_SCHEMA,
            "label": label,
            "node": node.node.0,
            "loops": node.loops,
            "eps_lines": node.eps_lines,
            "presentation": kind(node),
            "arity": arity,
            "family": presentation.family.name(),
            "support": mask_text(report.support, arity),
            "lp_count": meta.lp_count,
            "residuals": meta.residuals,
            "two_sided_axes": mask_text(meta.two_sided_axes, arity),
            "charts": report.charts.iter().map(|chart| serde_json::json!({
                "chart": mask_text(chart.chart, arity),
                "rules": chart.rules,
                "residuals": chart.residuals,
            })).collect::<Vec<_>>(),
            "rules": report.rules,
            "residual_keys": report.residual_keys,
            "ordering": report.ordering,
            "rule_selection": "request default (FirstValid), all-rank, finite cases searched",
            "payload_bytes": report.payload_bytes,
            "lp_seconds": report.lp_seconds,
            "generation_seconds": report.generation_seconds,
            "total_seconds": report.total_seconds,
            "workers": arguments.threads,
            "versions": {
                "P_CUSTOM_OWNER_VERSION": P_CUSTOM_OWNER_VERSION,
                "P_OWNER_LIBRARY_SEMANTICS": P_OWNER_LIBRARY_SEMANTICS,
            },
        });
        let json = serde_json::to_string_pretty(&record)
            .map_err(|error| CliError::OutputIo(error.to_string()))?;
        write_output(
            &destination(label, "rrbin"),
            &payload.bytes,
            arguments.force,
        )?;
        write_output(
            &destination(label, "json"),
            format!("{json}\n").as_bytes(),
            arguments.force,
        )?;
        writeln!(
            stdout,
            "{label} node {} L{} eps {}: {} rules over {} charts, {} residuals (LP {}), {} bytes, {:.3} s",
            node.node.0,
            node.loops,
            node.eps_lines,
            report.rules,
            report.charts.len(),
            meta.residuals,
            meta.lp_count,
            report.payload_bytes,
            report.total_seconds
        )
        .map_err(report_error)?;
    }
    writeln!(
        stdout,
        "{} custom owners written to {}",
        selected.len(),
        arguments.out.display()
    )
    .map_err(report_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(arguments: &[&str]) -> Result<Arguments, ArgError> {
        parse(arguments.iter().map(OsString::from).collect())
    }

    #[test]
    fn p_generate_parses_its_options() {
        let required = ["--families", "f", "--max-loops", "3", "--out", "o"];
        assert_eq!(
            parsed(&required).unwrap(),
            Arguments {
                families: "f".into(),
                max_loops: 3,
                out: "o".into(),
                owners: BTreeSet::new(),
                threads: 1,
                force: false,
            }
        );
        let all = [
            &required[..],
            &["--owner", "a", "--owner", "b", "--threads", "4", "--force"],
        ]
        .concat();
        let all = parsed(&all).unwrap();
        assert_eq!((all.owners.len(), all.threads, all.force), (2, 4, true));
        for missing in [
            &["--max-loops", "3", "--out", "o"][..],
            &["--families", "f", "--out", "o"],
            &["--families", "f", "--max-loops", "3"],
        ] {
            assert!(matches!(
                parsed(missing),
                Err(ArgError::MissingRequiredOption(_))
            ));
        }
        assert!(matches!(
            parsed(&["--owner", "a", "--owner", "a"]),
            Err(ArgError::DuplicateOption("--owner"))
        ));
        assert!(matches!(
            parsed(&["--threads", "0"]),
            Err(ArgError::InvalidValue { .. })
        ));
        assert!(matches!(
            parsed(&["--unknown"]),
            Err(ArgError::UnknownOption(_))
        ));
    }
}
