//! `rustred p-validate` (owner: M2-T8a rr-assemble).
//!
//! ```text
//! rustred p-validate --max-loops <k> [--points <n>] [--seed <s>] [--families <json>]
//!                    [--threads <t>]
//! ```
//!
//! Assembles the library program of scope `1..=k` (built-in families unless
//! `--families`; Custom owners generated), checks its descent certificate, and
//! traces `n` distinct random keys per loop count (default 10^4; the whole box
//! when it is smaller) through it without coefficients: dots 0-3 in total over
//! the lines, numerator rank 0-8 in total over the other lines and the ISPs,
//! random subsectors. Uncovered keys, missing owners and steps that do not
//! descend are counted (`PUncoveredPolicy::Reject`); any of them fails the run
//! with exit status 1.
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;

use rustred::propagator::descent::TraceFailureKind;
use rustred::propagator::error::PError;
use rustred::propagator::families::{P_FAMILIES_V1_JSON, PFamiliesFile};

use crate::AppError;
use crate::application::propagator::assemble::{
    AssembleOptions, ValidateOptions, unapplied_owners, validate_library,
};
use crate::application::propagator::{CustomOwnerOptions, CustomPayloads, assemble_library};
use crate::cli::args::{ArgError, StreamPath, next_utf8_value, next_value, parse_positive_integer};
use crate::cli::error::CliError;
use crate::cli::io::read_input;

#[derive(Debug, PartialEq, Eq)]
struct Arguments {
    families: Option<PathBuf>,
    max_loops: u8,
    points: usize,
    seed: u64,
    threads: usize,
}

fn parse(arguments: Vec<OsString>) -> Result<Arguments, ArgError> {
    let mut arguments = arguments.into_iter();
    let (mut families, mut max_loops) = (None, None);
    let defaults = ValidateOptions::default();
    let (mut points, mut seed, mut threads) = (defaults.points, defaults.seed, 1);
    let mut seen = BTreeSet::new();
    while let Some(argument) = arguments.next() {
        let argument = argument.into_string().map_err(ArgError::NonUtf8Option)?;
        let option: &'static str = match argument.as_str() {
            "--families" => "--families",
            "--max-loops" => "--max-loops",
            "--points" => "--points",
            "--seed" => "--seed",
            "--threads" => "--threads",
            _ => return Err(ArgError::UnknownOption(argument)),
        };
        if !seen.insert(option) {
            return Err(ArgError::DuplicateOption(option));
        }
        match option {
            "--families" => families = Some(PathBuf::from(next_value(&mut arguments, option)?)),
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
            "--points" => {
                points = parse_positive_integer(option, next_utf8_value(&mut arguments, option)?)?;
            }
            "--seed" => {
                let value = next_utf8_value(&mut arguments, option)?;
                seed = value.parse().map_err(|_| ArgError::InvalidValue {
                    option,
                    value,
                    expected: "an unsigned 64-bit integer",
                })?;
            }
            _ => {
                threads = parse_positive_integer(option, next_utf8_value(&mut arguments, option)?)?;
            }
        }
    }
    Ok(Arguments {
        families,
        max_loops: max_loops.ok_or(ArgError::MissingRequiredOption("--max-loops"))?,
        points,
        seed,
        threads,
    })
}

fn failed(error: PError) -> CliError {
    CliError::Application(AppError::derivation(format!("p validate: {error}")))
}

pub(super) fn run(arguments: Vec<OsString>) -> Result<(), CliError> {
    let arguments = parse(arguments).map_err(CliError::Usage)?;
    let text = match &arguments.families {
        Some(path) => read_input(&StreamPath::File(path.clone()))?,
        None => P_FAMILIES_V1_JSON.to_owned(),
    };
    let families: PFamiliesFile =
        serde_json::from_str(&text).map_err(|error| CliError::Input(error.to_string()))?;
    let assembled = assemble_library(
        &families,
        arguments.max_loops,
        CustomPayloads::Generate(CustomOwnerOptions::default()),
        &AssembleOptions {
            threads: arguments.threads,
            verify_descent: true,
        },
    )
    .map_err(failed)?;
    let options = ValidateOptions {
        points: arguments.points,
        seed: arguments.seed,
        ..ValidateOptions::default()
    };
    let rows = validate_library(&assembled, &options).map_err(failed)?;
    let unapplied = unapplied_owners(&assembled, &rows);

    let mut stdout = std::io::stdout().lock();
    let report = |error: std::io::Error| CliError::OutputIo(error.to_string());
    writeln!(stdout, "{}", assembled.library.identity()).map_err(report)?;
    if let Some(descent) = &assembled.descent {
        writeln!(
            stdout,
            "descent certificate: owners {:?}, {} routes, {} owner sectors, {} zero sectors, {} edge maps, {} guarded",
            descent.owners,
            descent.routes,
            descent.owner_sectors,
            descent.zero_sectors,
            descent.edge_maps,
            descent.guarded_owners
        )
        .map_err(report)?;
    }
    let mut failures = 0;
    for row in &rows {
        let trace = &row.report;
        let count = |kind| trace.failures_of(kind);
        writeln!(
            stdout,
            "L{}: {} distinct keys{} (dots <= {} total, rank <= {} total), {} route + {} apply keys, \
             {} owners applied; uncovered {}, missing owner {}, descent {}, other {}; {:.2} s",
            row.loops,
            row.keys,
            if row.box_exhausted { " (whole box)" } else { "" },
            options.max_dots,
            options.max_rank,
            trace.route_keys,
            trace.apply_keys,
            trace.applied.len(),
            count(TraceFailureKind::Uncovered),
            count(TraceFailureKind::MissingOwner),
            count(TraceFailureKind::Descent),
            count(TraceFailureKind::Other),
            row.seconds
        )
        .map_err(report)?;
        for sample in &trace.samples {
            writeln!(
                stdout,
                "  input {} {:?}: {}",
                sample.input, sample.kind, sample.detail
            )
            .map_err(report)?;
        }
        failures += trace.failures.values().sum::<usize>();
    }
    writeln!(
        stdout,
        "{} of {} owners never applied",
        unapplied.len(),
        assembled.library.owners().len()
    )
    .map_err(report)?;
    if failures > 0 {
        return Err(failed(PError::Generation(format!(
            "{failures} traced keys refused"
        ))));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(arguments: &[&str]) -> Result<Arguments, ArgError> {
        parse(arguments.iter().map(OsString::from).collect())
    }

    #[test]
    fn p_validate_parses_its_options() {
        assert_eq!(
            parsed(&["--max-loops", "2"]).unwrap(),
            Arguments {
                families: None,
                max_loops: 2,
                points: 10_000,
                seed: ValidateOptions::default().seed,
                threads: 1,
            }
        );
        let all = parsed(&[
            "--max-loops",
            "3",
            "--points",
            "50",
            "--seed",
            "7",
            "--threads",
            "4",
            "--families",
            "f",
        ])
        .unwrap();
        assert_eq!(
            (all.max_loops, all.points, all.seed, all.threads),
            (3, 50, 7, 4)
        );
        assert_eq!(all.families, Some(PathBuf::from("f")));
        assert!(matches!(
            parsed(&["--points", "5"]),
            Err(ArgError::MissingRequiredOption("--max-loops"))
        ));
        assert!(matches!(
            parsed(&["--max-loops", "0"]),
            Err(ArgError::InvalidValue { .. })
        ));
        assert!(matches!(
            parsed(&["--max-loops", "2", "--seed", "x"]),
            Err(ArgError::InvalidValue { .. })
        ));
    }
}
