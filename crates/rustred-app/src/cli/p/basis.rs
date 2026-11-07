//! `rustred p-basis` (owner: M2-T9 rr-basis).
//!
//! ```text
//! rustred p-basis --max-loops <k> [--out <p-layer.json>] [--force] [--families <json>]
//!                 [--points <n>] [--seed <s>] [--threads <t>]
//! ```
//!
//! Assembles the library program of scope `1..=k` (built-in families unless
//! `--families`; Custom owners generated), derives its basis layer
//! (`rustred::propagator::basis::derive_basis_layer`): every Forcer reference
//! master on a node of the library (`no`, `t105` at 1L-3L) is reduced in that
//! node's presentation onto the terminals, the rows are `T = A^-1 (X - S)`, and
//! they are checked at `n` fresh `d` modulo `2^61 - 1` (default 2) against a
//! `Z_p(d)` re-reduction. Prints the identity, the master sites, the rows and
//! the checks; `--out` writes the layer (`p-layer.json` content, schema
//! `rustred.p-basis-layer.v1`). Any failure (more terminals than masters, a
//! singular system, a failed check, a library that cannot be assembled) is an
//! error of the run, exit status 5 (derivation).
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;

use rustred::propagator::basis::{PBasisOptions, coefficient_text, derive_basis_layer};
use rustred::propagator::error::PError;
use rustred::propagator::families::{P_FAMILIES_V1_JSON, PFamiliesFile};
use rustred::propagator::library::{PBasis, PTerm};

use crate::AppError;
use crate::application::propagator::{
    AssembleOptions, CustomOwnerOptions, CustomPayloads, assemble_library,
};
use crate::cli::args::{ArgError, StreamPath, next_utf8_value, next_value, parse_positive_integer};
use crate::cli::error::CliError;
use crate::cli::io::{preflight_output_destination, read_input, write_output};

#[derive(Debug, PartialEq, Eq)]
struct Arguments {
    families: Option<PathBuf>,
    max_loops: u8,
    out: Option<PathBuf>,
    force: bool,
    points: usize,
    seed: u64,
    threads: usize,
}

fn parse(arguments: Vec<OsString>) -> Result<Arguments, ArgError> {
    let mut arguments = arguments.into_iter();
    let (mut families, mut max_loops, mut out, mut force) = (None, None, None, false);
    let defaults = PBasisOptions::default();
    let (mut points, mut seed, mut threads) = (defaults.check_points, defaults.seed, 1);
    let mut seen = BTreeSet::new();
    while let Some(argument) = arguments.next() {
        let argument = argument.into_string().map_err(ArgError::NonUtf8Option)?;
        let option: &'static str = match argument.as_str() {
            "--families" => "--families",
            "--max-loops" => "--max-loops",
            "--out" => "--out",
            "--force" => "--force",
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
            "--out" => out = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--force" => force = true,
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
        out,
        force,
        points,
        seed,
        threads,
    })
}

fn failed(error: PError) -> CliError {
    CliError::Application(AppError::derivation(format!("p basis: {error}")))
}

/// `+ c * [m,e].. * basis` of one term.
fn term_text(term: &PTerm) -> String {
    let basis = match &term.basis {
        PBasis::Unit => "1".to_owned(),
        PBasis::Master(name) => format!("Master({name})"),
        PBasis::Residual(id) => format!("T{}", id.0),
    };
    format!(
        "({}) Gamma{:?} {basis}",
        coefficient_text(&term.coefficient),
        term.tag.exponents()
    )
}

pub(super) fn run(arguments: Vec<OsString>) -> Result<(), CliError> {
    let arguments = parse(arguments).map_err(CliError::Usage)?;
    let out = arguments.out.clone().map(StreamPath::File);
    if let Some(out) = &out {
        preflight_output_destination(out, arguments.force)?;
    }
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
    let options = PBasisOptions {
        check_points: arguments.points,
        seed: arguments.seed,
        ..PBasisOptions::default()
    };
    let derivation = derive_basis_layer(&assembled.library, &options).map_err(failed)?;
    if let Some(out) = &out {
        write_output(out, derivation.layer.to_json().as_bytes(), arguments.force)?;
    }

    let mut stdout = std::io::stdout().lock();
    let report = |error: std::io::Error| CliError::OutputIo(error.to_string());
    writeln!(stdout, "{}", assembled.library.identity()).map_err(report)?;
    for site in &derivation.sites {
        writeln!(
            stdout,
            "master {} ({}, sign {:+}): node {}, presentation {}, key {:?}, reduces to {}",
            site.master.name,
            site.master.topology,
            site.master.sign,
            site.node.0,
            site.presentation.0,
            site.key,
            site.reduction
                .iter()
                .map(term_text)
                .collect::<Vec<_>>()
                .join(" + ")
        )
        .map_err(report)?;
    }
    if !derivation.absent.is_empty() {
        writeln!(
            stdout,
            "masters without a node in this library: {}",
            derivation.absent.join(", ")
        )
        .map_err(report)?;
    }
    for row in derivation.layer.rows() {
        let terminal = &row.terminal;
        writeln!(
            stdout,
            "T{} {} node {} key {:?} [{}] = {}",
            terminal.id.0,
            terminal.class.as_deref().unwrap_or("Unit"),
            terminal
                .node
                .map_or_else(|| "-".to_owned(), |node| node.0.to_string()),
            terminal.key,
            row.provenance,
            row.terms
                .iter()
                .map(term_text)
                .collect::<Vec<_>>()
                .join(" + ")
        )
        .map_err(report)?;
    }
    for check in &derivation.checks {
        writeln!(
            stdout,
            "fresh check: d = {} modulo {}: {} components equal",
            check.d, check.prime, check.components
        )
        .map_err(report)?;
    }
    writeln!(
        stdout,
        "p basis: {} rows, masters {:?}{}",
        derivation.layer.rows().len(),
        derivation.layer.masters(),
        arguments
            .out
            .as_ref()
            .map_or_else(String::new, |path| format!(
                ", written to {}",
                path.display()
            ))
    )
    .map_err(report)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn p_basis_parses_its_options() {
        let parsed = parse(arguments(&[
            "--max-loops",
            "3",
            "--out",
            "layer.json",
            "--force",
            "--points",
            "4",
            "--seed",
            "7",
            "--threads",
            "2",
        ]))
        .unwrap();
        assert_eq!(
            parsed,
            Arguments {
                families: None,
                max_loops: 3,
                out: Some(PathBuf::from("layer.json")),
                force: true,
                points: 4,
                seed: 7,
                threads: 2,
            }
        );
        assert!(matches!(
            parse(arguments(&[])),
            Err(ArgError::MissingRequiredOption("--max-loops"))
        ));
        assert!(matches!(
            parse(arguments(&["--max-loops", "2", "--max-loops", "3"])),
            Err(ArgError::DuplicateOption("--max-loops"))
        ));
        assert!(matches!(
            parse(arguments(&["--max-loops", "2", "--layer"])),
            Err(ArgError::UnknownOption(_))
        ));
    }
}
