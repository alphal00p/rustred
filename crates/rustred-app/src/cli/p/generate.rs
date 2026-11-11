//! `rustred p-generate` (owner: M2-T7c rr-custom).
//!
//! ```text
//! rustred p-generate --families <json> --max-loops <k> --out <dir>
//!                    [--owner <class label>]... [--threads <n>] [--force]
//!                    [--max-seconds <s>] [--max-rss <bytes>] [--lp-from <dir>]
//!                    [--integral-order <json>] [--discovery <json>]
//! rustred p-generate --families <json> --max-loops <k> --list
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
//!
//! `--max-seconds` and `--max-rss` (`4GiB`, `512MiB`, ...) bound the
//! generation of EACH owner in wall time and process RSS
//! (`GenerationBudget`); an owner over budget fails the command (a `time` or
//! `rss` budget error). `--lp-from <dir>` re-generates from recorded LP
//! counts: each owner's count is the `lp_count` of its record
//! `<dir>/<label>.json` from an earlier run (`CustomOwnerOptions::lp_given`),
//! not counted again; a missing record is an error. The record says where its
//! count came from (`lp_count_source`: `counted` or `given`, with
//! `lp_given_from`) and the budget it ran under.
//!
//! `--integral-order <json>` (a `CandidateIntegralOrder` descriptor) and
//! `--discovery <json>` (a `CandidateDiscoveryStrategy`, with its optional
//! bounded rule portfolio) set the generation recipe of every selected owner
//! (`CustomOwnerOptions::integral_order` and `discovery`, M5-R0 WP3); the
//! record keeps both descriptors verbatim. Without them the default recipe is
//! used, as before.
//!
//! `--list` (p5l-converter) generates nothing: it prints the Custom owner
//! nodes of the DAG, one TSV row each (node, loops, eps lines, lines,
//! presentation kind, graph key in hex, canonical graph `u-v*` with `s,t`),
//! without the class label (no classification). With the 5L tops of
//! `data/p/p-families-5L.v1.json` in the families file and `--max-loops 5`
//! these are the 5L Custom owners; their graph keys identify the census nodes
//! (`T001..T283`) whose tables `p-import-table` converts.
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

use rustred::propagator::class::{ClassLimits, classify};
use rustred::propagator::custom::custom_owner_support;
use rustred::propagator::error::PError;
use rustred::propagator::presentation::PresentationKind;
use rustred::propagator::{P_CUSTOM_OWNER_VERSION, P_OWNER_LIBRARY_SEMANTICS};

use crate::AppError;
use crate::application::memory::parse_memory_bytes;
use crate::application::propagator::generate::{
    CustomOwnerNode, CustomOwnerOptions, GenerationBudget, custom_owner_nodes,
    generate_custom_owner_with_report,
};
use crate::application::{CandidateDiscoveryStrategy, CandidateIntegralOrder};
use crate::cli::args::{ArgError, StreamPath, next_utf8_value, next_value, parse_positive_integer};
use crate::cli::error::CliError;
use crate::cli::io::{preflight_output_destination, read_input, write_output};

/// Schema of the `<label>.json` owner record.
pub(crate) const P_CUSTOM_OWNER_RECORD_SCHEMA: &str = "rustred.p-custom-owner.v1";

#[derive(Debug, PartialEq)]
struct Arguments {
    families: PathBuf,
    max_loops: u8,
    out: PathBuf,
    owners: BTreeSet<String>,
    threads: usize,
    force: bool,
    budget: GenerationBudget,
    lp_from: Option<PathBuf>,
    integral_order: Option<PathBuf>,
    discovery: Option<PathBuf>,
    list: bool,
}

fn parse(arguments: Vec<OsString>) -> Result<Arguments, ArgError> {
    let mut arguments = arguments.into_iter();
    let (mut families, mut max_loops, mut out) = (None, None, None);
    let (mut owners, mut threads, mut force) = (BTreeSet::new(), 1, false);
    let (mut budget, mut lp_from) = (GenerationBudget::default(), None);
    let (mut integral_order, mut discovery, mut list) = (None, None, false);
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
            "--max-seconds" => "--max-seconds",
            "--max-rss" => "--max-rss",
            "--lp-from" => "--lp-from",
            "--integral-order" => "--integral-order",
            "--discovery" => "--discovery",
            "--list" => "--list",
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
            "--max-seconds" => {
                let value = next_utf8_value(&mut arguments, option)?;
                let seconds = value.parse::<f64>().ok();
                budget.max_seconds = Some(
                    seconds
                        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
                        .ok_or(ArgError::InvalidValue {
                            option,
                            value,
                            expected: "a positive number of seconds",
                        })?,
                );
            }
            "--max-rss" => {
                let value = next_utf8_value(&mut arguments, option)?;
                budget.max_rss_bytes = Some(
                    parse_memory_bytes(&value)
                        .filter(|bytes| *bytes > 0)
                        .ok_or(ArgError::InvalidValue {
                            option,
                            value,
                            expected: "a positive integer followed by B, KiB, MiB, GiB, or TiB",
                        })?,
                );
            }
            "--lp-from" => lp_from = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--integral-order" => {
                integral_order = Some(PathBuf::from(next_value(&mut arguments, option)?))
            }
            "--discovery" => discovery = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--list" => list = true,
            _ => force = true,
        }
    }
    Ok(Arguments {
        families: families.ok_or(ArgError::MissingRequiredOption("--families"))?,
        max_loops: max_loops.ok_or(ArgError::MissingRequiredOption("--max-loops"))?,
        out: match out {
            Some(out) => out,
            None if list => PathBuf::new(),
            None => return Err(ArgError::MissingRequiredOption("--out")),
        },
        owners,
        threads,
        force,
        budget,
        lp_from,
        integral_order,
        discovery,
        list,
    })
}

/// `--list`: the Custom owner nodes as TSV, nothing generated.
fn list_nodes(nodes: &[CustomOwnerNode]) -> Result<(), CliError> {
    let mut stdout = std::io::stdout().lock();
    let error = |error: std::io::Error| CliError::OutputIo(error.to_string());
    writeln!(stdout, "node\tloops\teps_lines\tlines\tpresentation\tgraph_key\tgraph\ts,t").map_err(error)?;
    for node in nodes {
        let key = node.presentation.node.as_ref().map_or_else(String::new, |reference| {
            reference.graph_key.iter().map(|b| format!("{b:02x}")).collect()
        });
        let lines = node.presentation.slots.iter().filter(|slot| matches!(slot, rustred::propagator::presentation::Slot::Line(_))).count();
        writeln!(
            stdout,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{},{}",
            node.node.0,
            node.loops,
            node.eps_lines,
            lines,
            kind(node),
            key,
            node.graph.lines.iter().map(|&(u, v, e)| format!("{u}-{v}{}", "*".repeat(usize::from(e)))).collect::<Vec<_>>().join(" "),
            node.graph.s,
            node.graph.t,
        )
        .map_err(error)?;
    }
    Ok(())
}

/// The recorded LP count of the owner `label`: `lp_count` of the owner record
/// `<directory>/<label>.json` of an earlier `p-generate`.
fn recorded_lp_count(directory: &Path, label: &str) -> Result<u32, CliError> {
    let path = directory.join(format!("{label}.json"));
    let text = read_input(&StreamPath::File(path.clone()))?;
    let refused = |detail: &str| {
        CliError::Input(format!(
            "{}: {detail}, not the owner record of {label}",
            path.display()
        ))
    };
    let record: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| refused(&error.to_string()))?;
    if record["schema"] != P_CUSTOM_OWNER_RECORD_SCHEMA || record["label"] != label {
        return Err(refused("another schema or label"));
    }
    (record["lp_count"].as_u64())
        .and_then(|count| u32::try_from(count).ok())
        .ok_or_else(|| refused("no lp_count"))
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
    if arguments.list {
        return list_nodes(&nodes);
    }
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
    // Every recorded count before the first generation.
    let mut given = Vec::with_capacity(selected.len());
    for (label, _) in &selected {
        given.push(match &arguments.lp_from {
            Some(directory) => Some(recorded_lp_count(directory, label)?),
            None => None,
        });
    }
    let recipe = |path: &Option<PathBuf>| -> Result<Option<String>, CliError> {
        path.as_ref()
            .map(|path| read_input(&StreamPath::File(path.clone())))
            .transpose()
    };
    let integral_order_text = recipe(&arguments.integral_order)?;
    let discovery_text = recipe(&arguments.discovery)?;
    let integral_order = (integral_order_text.as_deref())
        .map(CandidateIntegralOrder::from_json)
        .transpose()
        .map_err(CliError::Application)?;
    let discovery = (discovery_text.as_deref())
        .map(CandidateDiscoveryStrategy::from_json)
        .transpose()
        .map_err(CliError::Application)?;
    let json_or_null = |text: &Option<String>| {
        (text.as_deref())
            .map(|text| serde_json::from_str::<serde_json::Value>(text).unwrap_or_default())
            .unwrap_or(serde_json::Value::Null)
    };
    let mut stdout = std::io::stdout().lock();
    let report_error = |error: std::io::Error| CliError::OutputIo(error.to_string());
    for ((label, node), lp_given) in selected.iter().zip(given) {
        let presentation = &node.presentation;
        let arity = presentation.slots.len();
        let options = CustomOwnerOptions {
            support: None,
            n_cores: arguments.threads,
            budget: arguments.budget,
            lp_given,
            fallthrough: None,
            integral_order: integral_order.clone(),
            discovery: discovery.clone(),
        };
        let (payload, report) =
            generate_custom_owner_with_report(presentation, &options).map_err(failed)?;
        let lp_given_from = (arguments.lp_from.as_ref())
            .filter(|_| report.lp_given)
            .map(|directory| {
                directory
                    .join(format!("{label}.json"))
                    .display()
                    .to_string()
            });
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
            "lp_count_source": if report.lp_given { "given" } else { "counted" },
            "lp_given_from": lp_given_from,
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
            "rule_selection": if discovery_text.is_some() || integral_order_text.is_some() {
                "recipe (integral_order_recipe, discovery_recipe), all-rank, finite cases searched"
            } else {
                "request default (FirstValid), all-rank, finite cases searched"
            },
            "integral_order_recipe": json_or_null(&integral_order_text),
            "discovery_recipe": json_or_null(&discovery_text),
            "payload_bytes": report.payload_bytes,
            "lp_seconds": report.lp_seconds,
            "generation_seconds": report.generation_seconds,
            "total_seconds": report.total_seconds,
            "workers": arguments.threads,
            "budget": {
                "max_seconds": arguments.budget.max_seconds,
                "max_rss_bytes": arguments.budget.max_rss_bytes,
            },
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
                budget: GenerationBudget::default(),
                lp_from: None,
                integral_order: None,
                discovery: None,
                list: false,
            }
        );
        let all = [
            &required[..],
            &["--owner", "a", "--owner", "b", "--threads", "4", "--force"],
            &[
                "--max-seconds",
                "0.5",
                "--max-rss",
                "2GiB",
                "--lp-from",
                "r",
            ],
        ]
        .concat();
        let all = parsed(&all).unwrap();
        assert_eq!((all.owners.len(), all.threads, all.force), (2, 4, true));
        assert_eq!(
            (all.budget, all.lp_from),
            (
                GenerationBudget {
                    max_seconds: Some(0.5),
                    max_rss_bytes: Some(2 << 30),
                },
                Some(PathBuf::from("r"))
            )
        );
        for bad in [
            &["--max-seconds", "0"][..],
            &["--max-seconds", "-1"],
            &["--max-seconds", "inf"],
            &["--max-rss", "0B"],
            &["--max-rss", "2GB"],
        ] {
            assert!(
                matches!(parsed(bad), Err(ArgError::InvalidValue { .. })),
                "{bad:?}"
            );
        }
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
