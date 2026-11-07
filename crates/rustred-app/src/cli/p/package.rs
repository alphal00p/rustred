//! `rustred p-package` (owner: M2-T8b rr-package).
//!
//! ```text
//! rustred p-package --max-loops <k> --out <dir> [--reuse-custom <dir>] [--families <json>]
//!                   [--threads <t>]
//! ```
//!
//! Assembles the library program of scope `1..=k` (built-in families unless
//! `--families`) with its descent certificate and its basis layer onto
//! Forcer's masters (`assemble_p_package`), writes it as the package
//! directory `<dir>` (which must not exist or be empty), loads the written
//! files back with the descent certificate and checks that they give the same
//! program (`equivalent_p_libraries`). Custom owners are generated, or with
//! `--reuse-custom` taken verbatim from the package in that directory, whose
//! versions must be this build's. Prints the identity, every file with its
//! size and the total; any failure is exit status 5 (derivation).
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use rustred::propagator::error::PError;
use rustred::propagator::families::{P_FAMILIES_V1_JSON, PFamiliesFile};

use crate::AppError;
use crate::application::propagator::loader::read_custom_payloads;
use crate::application::propagator::package::{
    assemble_p_package, p_library_differences, p_library_files, write_p_library_files,
};
use crate::application::propagator::{
    AssembleOptions, CustomOwnerOptions, CustomPayloads, PLibraryOptions, PLibrarySource,
    load_p_library,
};
use crate::cli::args::{ArgError, StreamPath, next_utf8_value, next_value, parse_positive_integer};
use crate::cli::error::CliError;
use crate::cli::io::read_input;

#[derive(Debug, PartialEq, Eq)]
struct Arguments {
    families: Option<PathBuf>,
    max_loops: u8,
    out: PathBuf,
    reuse_custom: Option<PathBuf>,
    threads: usize,
}

fn parse(arguments: Vec<OsString>) -> Result<Arguments, ArgError> {
    let mut arguments = arguments.into_iter();
    let (mut families, mut max_loops, mut out, mut reuse_custom) = (None, None, None, None);
    let mut threads = 1;
    let mut seen = BTreeSet::new();
    while let Some(argument) = arguments.next() {
        let argument = argument.into_string().map_err(ArgError::NonUtf8Option)?;
        let option: &'static str = match argument.as_str() {
            "--families" => "--families",
            "--max-loops" => "--max-loops",
            "--out" => "--out",
            "--reuse-custom" => "--reuse-custom",
            "--threads" => "--threads",
            _ => return Err(ArgError::UnknownOption(argument)),
        };
        if !seen.insert(option) {
            return Err(ArgError::DuplicateOption(option));
        }
        match option {
            "--families" => families = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--out" => out = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--reuse-custom" => {
                reuse_custom = Some(PathBuf::from(next_value(&mut arguments, option)?));
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
            _ => {
                threads = parse_positive_integer(option, next_utf8_value(&mut arguments, option)?)?;
            }
        }
    }
    let out = out.ok_or(ArgError::MissingRequiredOption("--out"))?;
    if out.as_os_str().is_empty() {
        return Err(ArgError::EmptyPath);
    }
    Ok(Arguments {
        families,
        max_loops: max_loops.ok_or(ArgError::MissingRequiredOption("--max-loops"))?,
        out,
        reuse_custom,
        threads,
    })
}

fn failed(error: PError) -> CliError {
    CliError::Application(AppError::derivation(format!("p package: {error}")))
}

pub(super) fn run(arguments: Vec<OsString>) -> Result<(), CliError> {
    let arguments = parse(arguments).map_err(CliError::Usage)?;
    let text = match &arguments.families {
        Some(path) => read_input(&StreamPath::File(path.clone()))?,
        None => P_FAMILIES_V1_JSON.to_owned(),
    };
    let families: PFamiliesFile =
        serde_json::from_str(&text).map_err(|error| CliError::Input(error.to_string()))?;
    let custom = match &arguments.reuse_custom {
        Some(directory) => CustomPayloads::Provided(
            read_custom_payloads(&PLibrarySource::Directory(directory.clone())).map_err(failed)?,
        ),
        None => CustomPayloads::Generate(CustomOwnerOptions::default()),
    };
    let start = Instant::now();
    let assembled = assemble_p_package(
        &families,
        arguments.max_loops,
        custom,
        &AssembleOptions {
            threads: arguments.threads,
            verify_descent: true,
        },
    )
    .map_err(failed)?;
    let assembled_seconds = start.elapsed().as_secs_f64();
    let files = p_library_files(&assembled).map_err(failed)?;
    write_p_library_files(&files, &arguments.out).map_err(failed)?;
    let start = Instant::now();
    let loaded = load_p_library(
        PLibrarySource::Directory(arguments.out.clone()),
        &PLibraryOptions {
            verify_descent: true,
        },
    )
    .map_err(failed)?;
    let loaded_seconds = start.elapsed().as_secs_f64();
    let differences = p_library_differences(&assembled.library, &loaded).map_err(failed)?;
    if !differences.is_empty() {
        return Err(failed(PError::MalformedLibrary(format!(
            "the written package loads as another program: {}",
            differences.join("; ")
        ))));
    }

    let mut stdout = std::io::stdout().lock();
    let report = |error: std::io::Error| CliError::OutputIo(error.to_string());
    writeln!(stdout, "{}", assembled.library.identity()).map_err(report)?;
    let mut total = 0;
    for (name, bytes) in &files {
        writeln!(stdout, "{:>10}  {name}", bytes.len()).map_err(report)?;
        total += bytes.len();
    }
    writeln!(
        stdout,
        "{total:>10}  total, {} files in {}",
        files.len(),
        arguments.out.display()
    )
    .map_err(report)?;
    writeln!(
        stdout,
        "assembled in {assembled_seconds:.2} s, loaded back with the descent certificate in {loaded_seconds:.2} s: the same program"
    )
    .map_err(report)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(arguments: &[&str]) -> Result<Arguments, ArgError> {
        parse(arguments.iter().map(OsString::from).collect())
    }

    #[test]
    fn p_package_parses_its_options() {
        assert_eq!(
            parsed(&["--max-loops", "2", "--out", "d"]).unwrap(),
            Arguments {
                families: None,
                max_loops: 2,
                out: PathBuf::from("d"),
                reuse_custom: None,
                threads: 1,
            }
        );
        let all = parsed(&[
            "--out",
            "d",
            "--max-loops",
            "3",
            "--reuse-custom",
            "r",
            "--threads",
            "4",
            "--families",
            "f",
        ])
        .unwrap();
        assert_eq!(
            (all.max_loops, all.threads, all.reuse_custom, all.families),
            (3, 4, Some(PathBuf::from("r")), Some(PathBuf::from("f")))
        );
        assert!(matches!(
            parsed(&["--max-loops", "2"]),
            Err(ArgError::MissingRequiredOption("--out"))
        ));
        assert!(matches!(
            parsed(&["--out", "d"]),
            Err(ArgError::MissingRequiredOption("--max-loops"))
        ));
        assert!(matches!(
            parsed(&["--max-loops", "2", "--out", "d", "--out", "e"]),
            Err(ArgError::DuplicateOption("--out"))
        ));
        assert_eq!(failed(PError::Unsupported("x".to_owned())).exit_code(), 5);
    }
}
