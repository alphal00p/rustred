//! `rustred p-content`: the content digest of a p-library package
//! (`application::propagator::content`), its manifest `p-content.json`, a
//! verification against it, and the content key of every presentation.
//!
//! `rustred p-content --help` prints [`USAGE`] (options, output, exit status).
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::AppError;
use crate::application::propagator::content::{
    P_CONTENT_FILE, PContentCheck, compute_p_content, p_content_differences, read_p_content,
};
use crate::application::propagator::{
    PLibraryOptions, PLibrarySource, format::LIBRARY_FILE, load_p_library_checked,
};
use crate::cli::args::{ArgError, next_utf8_value, next_value, parse_positive_integer};
use crate::cli::error::CliError;

/// `rustred p-content --help`.
pub(crate) const USAGE: &str = "\
rustred p-content: the content digest of a p-library package

USAGE:
    rustred p-content --package <dir> [--threads <n>] [--verify] [--write] [--out <file>] [--keys <tsv>] [--force]
    rustred p-content --help

Computes the digest of every file of the package that determines its presentations, ids, routes, orderings,
owner tables and relations (library.json without its producer record and revision label, the families,
topology, census, layer, ordering, routes, orbits, owner payloads, open-nodes.json, relations/), prints it with
the number of files, their bytes and the time taken. Packages without p-content.json (written before it
existed) are digested the same way.

OPTIONS:
    --package <dir>    the package directory
    --threads <n>      files hashed in parallel [default: 1]
    --verify           compare with the package's p-content.json; every difference is printed
    --write            write <dir>/p-content.json
    --out <file>       write the manifest to <file>
    --keys <tsv>       load the package and write the content key of every presentation (id, kind, loops,
                       graph key, eps units of the lines, content key) to <tsv>
    --force            replace an existing p-content.json, --out or --keys file

EXIT STATUS:
    0  done (with --verify: the files have the content p-content.json records)
    1  --verify: the package has no p-content.json, or its files differ from it
    2  usage error
    3  library.json cannot be read
    4  an output file exists and --force was not given
    5  the package is malformed (a file it names is missing or unreadable) or does not load (--keys)
    7  an output cannot be written
";

#[derive(Debug, PartialEq, Eq)]
struct Arguments {
    package: PathBuf,
    threads: usize,
    verify: bool,
    write: bool,
    out: Option<PathBuf>,
    keys: Option<PathBuf>,
    force: bool,
}

/// The arguments, `None` for `--help` / `-h` (alone).
fn parse(arguments: Vec<OsString>) -> Result<Option<Arguments>, ArgError> {
    if arguments.len() == 1 && (arguments[0] == "--help" || arguments[0] == "-h") {
        return Ok(None);
    }
    let mut arguments = arguments.into_iter();
    let (mut package, mut out, mut keys) = (None, None, None);
    let (mut threads, mut verify, mut write, mut force) = (1, false, false, false);
    let mut seen = BTreeSet::new();
    while let Some(argument) = arguments.next() {
        let argument = argument.into_string().map_err(ArgError::NonUtf8Option)?;
        let option: &'static str = match argument.as_str() {
            "--package" => "--package",
            "--threads" => "--threads",
            "--verify" => "--verify",
            "--write" => "--write",
            "--out" => "--out",
            "--keys" => "--keys",
            "--force" => "--force",
            _ => return Err(ArgError::UnknownOption(argument)),
        };
        if !seen.insert(option) {
            return Err(ArgError::DuplicateOption(option));
        }
        match option {
            "--package" => package = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--out" => out = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--keys" => keys = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--threads" => {
                threads = parse_positive_integer(option, next_utf8_value(&mut arguments, option)?)?;
            }
            "--verify" => verify = true,
            "--write" => write = true,
            _ => force = true,
        }
    }
    Ok(Some(Arguments {
        package: package.ok_or(ArgError::MissingRequiredOption("--package"))?,
        threads,
        verify,
        write,
        out,
        keys,
        force,
    }))
}

fn failed(message: impl std::fmt::Display) -> CliError {
    CliError::Application(AppError::derivation(format!("p content: {message}")))
}

fn io(error: std::io::Error) -> CliError {
    CliError::OutputIo(error.to_string())
}

/// Refuse to replace `path` without `--force`.
fn writable(path: &Path, force: bool) -> Result<(), CliError> {
    if path.exists() && !force {
        return Err(CliError::Input(format!(
            "{} exists (use --force)",
            path.display()
        )));
    }
    Ok(())
}

/// The content key of every presentation of the loaded package, as TSV.
fn keys_tsv(package: &Path) -> Result<(String, f64), CliError> {
    use rustred::propagator::presentation::PresentationKind;
    let started = Instant::now();
    let library = load_p_library_checked(
        PLibrarySource::Directory(package.to_owned()),
        &PLibraryOptions {
            verify_descent: false,
        },
        PContentCheck::Off,
    )
    .map_err(failed)?;
    let seconds = started.elapsed().as_secs_f64();
    let mut out = String::from("id\tkind\tloops\tgraph_key\tline_eps_units\tcontent_key\n");
    for presentation in library.presentations() {
        let kind = match &presentation.kind {
            PresentationKind::Top { .. } => "top",
            PresentationKind::Induced { .. } => "induced",
            PresentationKind::InsertionAdapted => "insertion_adapted",
            PresentationKind::CarpetAdapted => "carpet_adapted",
            PresentationKind::TriangleAdapted => "triangle_adapted",
            PresentationKind::DiamondAdapted => "diamond_adapted",
            PresentationKind::SchemeAdapted => "scheme_adapted",
            PresentationKind::EpsNode => "eps_node",
            PresentationKind::Caller => "caller",
        };
        let key = presentation.content_key();
        let graph_key: String = key.graph_key.iter().map(|b| format!("{b:02x}")).collect();
        let eps: Vec<String> = key.line_eps_units.iter().map(u8::to_string).collect();
        out.push_str(&format!(
            "{}\t{kind}\t{}\t{}\t{}\t{}\n",
            presentation.id.0,
            presentation.loops,
            if graph_key.is_empty() {
                "-"
            } else {
                &graph_key
            },
            eps.join(","),
            key.digest()
        ));
    }
    Ok((out, seconds))
}

/// The command; `Ok(false)` when `--verify` found a difference or no manifest.
fn content(arguments: &Arguments) -> Result<bool, CliError> {
    let source = PLibrarySource::Directory(arguments.package.clone());
    let library = arguments.package.join(LIBRARY_FILE);
    if let Err(error) = std::fs::metadata(&library) {
        return Err(CliError::InputIo(format!("{}: {error}", library.display())));
    }
    let manifest_path = arguments.package.join(P_CONTENT_FILE);
    if arguments.write {
        writable(&manifest_path, arguments.force)?;
    }
    for path in [&arguments.out, &arguments.keys].into_iter().flatten() {
        writable(path, arguments.force)?;
    }
    let started = Instant::now();
    let computed = compute_p_content(&source).map_err(failed)?;
    let seconds = started.elapsed().as_secs_f64();
    let bytes = computed.total_bytes();
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "package {}", arguments.package.display()).map_err(io)?;
    writeln!(
        stdout,
        "content {} ({} files, {bytes} bytes, {seconds:.2} s, {:.0} MB/s, {} threads)",
        computed.digest,
        computed.files.len(),
        bytes as f64 / 1e6 / seconds.max(1e-9),
        arguments.threads
    )
    .map_err(io)?;
    let mut same = true;
    if arguments.verify {
        match read_p_content(&source).map_err(failed)? {
            None => {
                writeln!(stdout, "verify: the package has no {P_CONTENT_FILE}").map_err(io)?;
                same = false;
            }
            Some(recorded) => {
                let differences = p_content_differences(&recorded, &computed);
                if differences.is_empty() {
                    writeln!(
                        stdout,
                        "verify: {P_CONTENT_FILE} agrees ({})",
                        recorded.digest
                    )
                    .map_err(io)?;
                } else {
                    writeln!(
                        stdout,
                        "verify: {} differences from {P_CONTENT_FILE} ({})",
                        differences.len(),
                        recorded.digest
                    )
                    .map_err(io)?;
                    for difference in &differences {
                        writeln!(stdout, "  {difference}").map_err(io)?;
                    }
                    same = false;
                }
            }
        }
    }
    let json = computed.to_json().map_err(failed)?;
    if arguments.write {
        std::fs::write(&manifest_path, &json).map_err(io)?;
        writeln!(stdout, "wrote {}", manifest_path.display()).map_err(io)?;
    }
    if let Some(out) = &arguments.out {
        std::fs::write(out, &json).map_err(io)?;
        writeln!(stdout, "wrote {}", out.display()).map_err(io)?;
    }
    if let Some(path) = &arguments.keys {
        let (tsv, load_seconds) = keys_tsv(&arguments.package)?;
        std::fs::write(path, tsv).map_err(io)?;
        writeln!(
            stdout,
            "wrote the presentation content keys to {} (load {load_seconds:.2} s)",
            path.display()
        )
        .map_err(io)?;
    }
    Ok(same)
}

pub(super) fn run(arguments: Vec<OsString>) -> Result<(), CliError> {
    let Some(arguments) = parse(arguments).map_err(CliError::Usage)? else {
        let mut stdout = std::io::stdout().lock();
        return stdout
            .write_all(USAGE.as_bytes())
            .and_then(|()| stdout.flush())
            .map_err(io);
    };
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(arguments.threads)
        .build()
        .map_err(failed)?;
    if pool.install(|| content(&arguments))? {
        Ok(())
    } else {
        Err(CliError::Verdict {
            incomplete: false,
            message: format!(
                "p content: {} does not have the content {P_CONTENT_FILE} records",
                arguments.package.display()
            ),
        })
    }
}
