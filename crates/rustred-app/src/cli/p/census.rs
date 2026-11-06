//! `rustred p-census` (owner: M2-T2 rr-census).
//!
//! ```text
//! rustred p-census --families <json> --max-loops <k> --out <census.json>
//!                  [--tops-only] [--threads <n>] [--force]
//! ```
//!
//! The census of the library program of the tops with at most `k` loops: the tops
//! and one presentation per node of their reduction DAG, owners at min-GraphKey
//! nodes, strong completeness included, as JSON
//! (`rustred::propagator::census::P_CENSUS_SCHEMA`: counts per loop order, class
//! labels, every sector). `--tops-only` leaves out the DAG: the labelled sectors of
//! the tops alone, with the first member of a class as its owner.
//!
//! A node on the sector of a top reads its graph from the embedded families file,
//! so families that differ from it fail closed at such a node.
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use rustred::propagator::census::{PCensus, PCensusCounts, PCensusOptions, p_census};
use rustred::propagator::error::PError;
use rustred::propagator::families::load_p_families;
use rustred::propagator::present::{present_node, top_topology};
use rustred::propagator::presentation::Presentation;
use rustred::propagator::topology::{ChildMode, Dag, reduction_graph};
use rustred::propagator::{PresentationId, families::PFamilyEntry};

use crate::AppError;
use crate::cli::args::{ArgError, StreamPath, next_utf8_value, next_value, parse_positive_integer};
use crate::cli::error::CliError;
use crate::cli::io::{preflight_output_destination, read_input, write_output};

#[derive(Debug, PartialEq, Eq)]
struct Arguments {
    families: PathBuf,
    max_loops: u8,
    out: PathBuf,
    tops_only: bool,
    threads: usize,
    force: bool,
}

fn parse(arguments: Vec<OsString>) -> Result<Arguments, ArgError> {
    let mut arguments = arguments.into_iter();
    let (mut families, mut max_loops, mut out) = (None, None, None);
    let (mut tops_only, mut threads, mut force) = (false, 1, false);
    let mut seen = BTreeSet::new();
    while let Some(argument) = arguments.next() {
        let argument = argument.into_string().map_err(ArgError::NonUtf8Option)?;
        let option: &'static str = match argument.as_str() {
            "--families" => "--families",
            "--max-loops" => "--max-loops",
            "--out" => "--out",
            "--threads" => "--threads",
            "--force" => "--force",
            "--tops-only" => "--tops-only",
            _ => return Err(ArgError::UnknownOption(argument)),
        };
        if !seen.insert(option) {
            return Err(ArgError::DuplicateOption(option));
        }
        match option {
            "--families" => families = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--out" => out = Some(PathBuf::from(next_value(&mut arguments, option)?)),
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
            "--tops-only" => tops_only = true,
            _ => force = true,
        }
    }
    Ok(Arguments {
        families: families.ok_or(ArgError::MissingRequiredOption("--families"))?,
        max_loops: max_loops.ok_or(ArgError::MissingRequiredOption("--max-loops"))?,
        out: out.ok_or(ArgError::MissingRequiredOption("--out"))?,
        tops_only,
        threads,
        force,
    })
}

fn failed(error: PError) -> CliError {
    CliError::Application(AppError::derivation(format!("p census: {error}")))
}

/// The DAG of the tops and the presentations of its nodes after the tops, as the
/// library assembly has them.
fn with_node_presentations(
    entries: &[&PFamilyEntry],
    tops: Vec<Arc<Presentation>>,
) -> Result<(Dag, Vec<Arc<Presentation>>), PError> {
    let roots: Vec<_> = entries.iter().map(|entry| top_topology(entry)).collect();
    let dag = reduction_graph(&roots, ChildMode::ForcerTree);
    let context = tops
        .first()
        .ok_or(PError::ContextMismatch)?
        .family
        .coefficient_context()
        .clone();
    let mut next = tops.iter().map(|p| p.id.0 + 1).max().unwrap_or(0);
    let mut presentations = tops.clone();
    for node in &dag.nodes {
        let made = present_node(node, &tops, PresentationId(next), &context)?;
        if let Some(reason) = made.fallback {
            return Err(PError::Generation(format!(
                "node {} left its lane: {reason}",
                node.index
            )));
        }
        if let Some(presentation) = made.presentation {
            presentations.push(Arc::new(presentation));
            next += 1;
        }
    }
    Ok((dag, presentations))
}

/// The census of `presentations` with its check; nothing is written before both
/// pass.
fn census_of(
    presentations: &[Arc<Presentation>],
    dag: Option<&Dag>,
    threads: usize,
) -> Result<(PCensus, Vec<PCensusCounts>, String), CliError> {
    let options = PCensusOptions {
        threads,
        ..PCensusOptions::default()
    };
    let census = p_census(presentations, dag, &options).map_err(failed)?;
    census
        .check_complete_with(presentations, dag, &options)
        .map_err(failed)?;
    let counts = census.counts(presentations).map_err(failed)?;
    let json = census.to_json(presentations).map_err(failed)?;
    Ok((census, counts, json))
}

pub(super) fn run(arguments: Vec<OsString>) -> Result<(), CliError> {
    let arguments = parse(arguments).map_err(CliError::Usage)?;
    let out = StreamPath::File(arguments.out.clone());
    preflight_output_destination(&out, arguments.force)?;
    let text = read_input(&StreamPath::File(arguments.families.clone()))?;
    let (file, presentations) =
        load_p_families(&text).map_err(|error| CliError::Input(error.to_string()))?;
    let (entries, selected): (Vec<_>, Vec<_>) = file
        .families
        .iter()
        .zip(presentations)
        .filter(|(entry, _)| entry.loops <= arguments.max_loops)
        .unzip();
    if selected.is_empty() {
        return Err(CliError::Input(format!(
            "{} has no family with at most {} loops",
            arguments.families.display(),
            arguments.max_loops
        )));
    }
    let (dag, presentations) = if arguments.tops_only {
        (None, selected)
    } else {
        let (dag, presentations) = with_node_presentations(&entries, selected).map_err(failed)?;
        (Some(dag), presentations)
    };
    let (census, counts, json) = census_of(&presentations, dag.as_ref(), arguments.threads)?;
    write_output(&out, json.as_bytes(), arguments.force)?;
    let mut stdout = std::io::stdout().lock();
    let report = |error: std::io::Error| CliError::OutputIo(error.to_string());
    for row in &counts {
        writeln!(
            stdout,
            "L{}: {} presentations, {} sectors, {} zero, {} nonzero, {} classes",
            row.loops,
            row.presentations,
            row.sectors,
            row.zero_sectors,
            row.nonzero_sectors,
            row.classes
        )
        .map_err(report)?;
    }
    writeln!(
        stdout,
        "{} classes written to {}",
        census.classes.len(),
        arguments.out.display()
    )
    .map_err(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    use rustred::propagator::families::P_FAMILIES_V1_JSON;

    fn parsed(arguments: &[&str]) -> Result<Arguments, ArgError> {
        parse(arguments.iter().map(OsString::from).collect())
    }

    #[test]
    fn p_census_parses_its_options() {
        let required = ["--families", "f", "--max-loops", "3", "--out", "o"];
        assert_eq!(
            parsed(&required).unwrap(),
            Arguments {
                families: "f".into(),
                max_loops: 3,
                out: "o".into(),
                tops_only: false,
                threads: 1,
                force: false,
            }
        );
        let all = [&required[..], &["--tops-only", "--threads", "4", "--force"]].concat();
        let all = parsed(&all).unwrap();
        assert_eq!((all.tops_only, all.threads, all.force), (true, 4, true));

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
        for wrong in ["0", "-1", "256", "three"] {
            let arguments = ["--families", "f", "--max-loops", wrong, "--out", "o"];
            assert!(matches!(
                parsed(&arguments),
                Err(ArgError::InvalidValue { .. })
            ));
        }
        assert!(matches!(
            parsed(&["--unknown"]),
            Err(ArgError::UnknownOption(_))
        ));
        assert!(matches!(
            parsed(&["--force", "--force"]),
            Err(ArgError::DuplicateOption("--force"))
        ));
    }

    #[test]
    fn p_census_writes_a_census_that_reads_back() {
        let directory = std::env::temp_dir().join(format!("p-census-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let (families, out) = (directory.join("families.json"), directory.join("out.json"));
        std::fs::write(&families, P_FAMILIES_V1_JSON).unwrap();
        let arguments = |loops: &str, extra: &[&str]| -> Vec<OsString> {
            [
                "--families".as_ref(),
                families.as_os_str(),
                "--max-loops".as_ref(),
                loops.as_ref(),
                "--out".as_ref(),
                out.as_os_str(),
                "--force".as_ref(),
            ]
            .into_iter()
            .map(OsString::from)
            .chain(extra.iter().map(OsString::from))
            .collect()
        };
        let (file, presentations) = load_p_families(P_FAMILIES_V1_JSON).unwrap();
        let entries: Vec<_> = file.families.iter().filter(|e| e.loops <= 2).collect();
        let tops: Vec<_> = presentations.into_iter().filter(|p| p.loops <= 2).collect();

        run(arguments("2", &["--tops-only"])).unwrap();
        let census = PCensus::from_json(&std::fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(census.classes.len(), 1 + 4);
        census.check_complete(&tops, None).unwrap();

        run(arguments("2", &[])).unwrap();
        let census = PCensus::from_json(&std::fs::read_to_string(&out).unwrap()).unwrap();
        let (dag, presentations) = with_node_presentations(&entries, tops).unwrap();
        assert_eq!((dag.nodes.len(), census.classes.len()), (7, 2 + 4));
        census.check_complete(&presentations, Some(&dag)).unwrap();

        assert!(matches!(run(arguments("0", &[])), Err(CliError::Usage(_))));
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
