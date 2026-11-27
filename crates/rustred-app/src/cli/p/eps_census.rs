//! `rustred p-eps-census` (skeleton-mode audit, lane sk-census; diagnostic, changes nothing).
//!
//! ```text
//! rustred p-eps-census --package <dir> --max-total <S> --out-dir <dir> [--threads <n>] [--full-load]
//!                      [--axes [--axes-threads <n>]] [--force]
//! ```
//!
//! Every (two-point graph `G` with `L` loops, eps units `u` on its lines) with `L + sum(u) <= S` and whether
//! the package at `<dir>` has a presentation for it through the public lookup (graph -> `normalise` ->
//! `canonical` -> GraphKey -> the presentation of that DAG node). The universe is generated (graphica's graph
//! generator, normal forms of `normalise`, cross-checked against every sector of the cubic tops), never read
//! from the package: see `application::propagator::eps_census`.
//!
//! Writes into `<dir>` (refused when a file exists, unless `--force`):
//! - `eps-census-S<S>.tsv`: one row per (graph key, assignment): loops, lines, units, the lookup key, the
//!   presentation id or MISSING, whether its line slots carry exactly those units, and for a missing row the
//!   cause and a presentation of which it is a sector;
//! - `eps-census-S<S>-summary.tsv`: by (loops, total units, shifted lines, max units): rows, present, missing;
//! - `eps-census-S<S>-causes.tsv`: the missing rows by cause;
//! - `eps-census-S<S>-closure.tsv`: the nodes of the ForcerTree DAG of the missing rows the package lacks;
//! - `eps-census-universe.tsv`: per loop count, the generated universe against the package's nodes;
//! - `eps-census-S<S>.json`: identity, counts and timings;
//! - with `--axes` (implies `--full-load`), `eps-census-S<S>-axes.tsv`: per presentation of a present eps row, its
//!   census sectors that lack an eps line in their support (pinched), and the refusals of a coefficient-free
//!   trace of keys with integer power 0 and -1 on every non-empty subset of its eps lines through the package
//!   (one trace per presentation, `--axes-threads` at a time, default 4, appended to
//!   `eps-census-S<S>-axes-trace.tsv` as each ends: a rerun skips the presentations already there; delete
//!   that file to trace again); also every Custom owner on an eps presentation checked two-sided exactly on its
//!   forced axes.
//!
//! The index is read from `library.json` (the node keys the loader checks against the DAG); `--full-load`
//! loads the package with `load_p_library` and takes the index from the loaded presentations instead,
//! exactly as vakint's lookup does, and fails if the two indices differ.
//!
//! `rustred p-eps-census --help` prints [`USAGE`], which also lists the exit statuses: 0 every row present (and,
//! with `--axes`, every axis check passed), 1 a row missing or an axis check failed (the census verdict, outputs
//! written), 2 usage, 3 `library.json` unreadable, 4 `library.json` malformed or an output file exists without
//! `--force`, 5 the package does not load or its loaded index differs from `library.json`'s, 7 an output cannot
//! be written.
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use crate::AppError;
use crate::application::propagator::eps_census::{
    LoopUniverse, PackageIndex, axes_report, axes_tsv, causes_tsv, census_rows, closure_tsv,
    generated_top_dags, loop_universe, missing_closure, rows_tsv, summary_tsv, universe_table,
};
use crate::application::propagator::{PLibraryOptions, PLibrarySource, load_p_library};
use crate::cli::args::{ArgError, next_utf8_value, next_value, parse_positive_integer};
use crate::cli::error::CliError;

/// `rustred p-eps-census --help`.
pub(crate) const USAGE: &str = "\
rustred p-eps-census: census of eps-shifted families against a p-library package (diagnostic, changes nothing)

USAGE:
    rustred p-eps-census --package <dir> --max-total <S> --out-dir <dir> [--threads <n>] [--full-load]
                         [--axes [--axes-threads <n>]] [--force]
    rustred p-eps-census --help

Lists every (two-point graph G with L loops, eps units u <= 4 per line) with L + sum(u) <= S, modulo the
automorphisms of G, and whether the package has a presentation for it through the public lookup (normalise ->
canonical -> graph key -> the presentation of that DAG node). The universe is generated, never read from the
package.

OPTIONS:
    --package <dir>       the package directory (library.json and the files it names)
    --max-total <S>       the bound L + sum(u), 1 to 6
    --out-dir <dir>       where the tables go (created; existing tables are refused unless --force)
    --threads <n>         worker threads [default: 1]
    --full-load           load the package and take the index from the loaded presentations (checked against
                          library.json's); without it the index is read from library.json alone
    --axes                also check the eps axes (implies --full-load): no census sector of an eps presentation
                          pinches an eps line, every Custom eps owner is two-sided exactly on them, and keys with
                          integer power 0 and -1 on the eps lines trace through the package without a refusal
                          (resumable: presentations already in eps-census-S<S>-axes-trace.tsv are skipped)
    --axes-threads <n>    presentations traced at a time [default: 4]
    --force               overwrite existing tables

OUTPUT (in --out-dir): eps-census-S<S>.tsv (one row per graph key and unit assignment), -summary.tsv, -causes.tsv,
-closure.tsv, eps-census-universe.tsv, eps-census-S<S>.json; with --axes -axes.tsv and -axes-trace.tsv.

EXIT STATUS:
    0  every row present (and, with --axes, every axis check passed)
    1  a row is missing or an axis check failed (the census verdict; the tables are written)
    2  usage error
    3  library.json cannot be read
    4  library.json is malformed, or an output table exists and --force was not given
    5  the package does not load, or the loaded index differs from library.json's
    7  an output cannot be written
";

#[derive(Debug, PartialEq, Eq)]
struct Arguments {
    package: PathBuf,
    max_total: u8,
    out_dir: PathBuf,
    threads: usize,
    full_load: bool,
    axes: bool,
    axes_threads: usize,
    force: bool,
}

/// The arguments, `None` for `--help` / `-h` (alone).
fn parse(arguments: Vec<OsString>) -> Result<Option<Arguments>, ArgError> {
    if arguments.len() == 1 && (arguments[0] == "--help" || arguments[0] == "-h") {
        return Ok(None);
    }
    let mut arguments = arguments.into_iter();
    let (mut package, mut max_total, mut out_dir) = (None, None, None);
    let (mut threads, mut full_load, mut axes, mut force) = (1, false, false, false);
    let mut axes_threads = 4;
    let mut seen = BTreeSet::new();
    while let Some(argument) = arguments.next() {
        let argument = argument.into_string().map_err(ArgError::NonUtf8Option)?;
        let option: &'static str = match argument.as_str() {
            "--package" => "--package",
            "--max-total" => "--max-total",
            "--out-dir" => "--out-dir",
            "--threads" => "--threads",
            "--full-load" => "--full-load",
            "--axes" => "--axes",
            "--axes-threads" => "--axes-threads",
            "--force" => "--force",
            _ => return Err(ArgError::UnknownOption(argument)),
        };
        if !seen.insert(option) {
            return Err(ArgError::DuplicateOption(option));
        }
        match option {
            "--package" => package = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--out-dir" => out_dir = Some(PathBuf::from(next_value(&mut arguments, option)?)),
            "--max-total" => {
                let value = next_utf8_value(&mut arguments, option)?;
                let total = parse_positive_integer(option, value.clone())?;
                if total > 6 {
                    return Err(ArgError::InvalidValue {
                        option,
                        value,
                        expected: "a bound L + sum(u) from 1 to 6",
                    });
                }
                max_total = Some(total as u8);
            }
            "--threads" => {
                threads = parse_positive_integer(option, next_utf8_value(&mut arguments, option)?)?;
            }
            "--full-load" => full_load = true,
            "--axes" => axes = true,
            "--axes-threads" => {
                axes_threads =
                    parse_positive_integer(option, next_utf8_value(&mut arguments, option)?)?;
            }
            _ => force = true,
        }
    }
    Ok(Some(Arguments {
        package: package.ok_or(ArgError::MissingRequiredOption("--package"))?,
        max_total: max_total.ok_or(ArgError::MissingRequiredOption("--max-total"))?,
        out_dir: out_dir.ok_or(ArgError::MissingRequiredOption("--out-dir"))?,
        threads,
        full_load: full_load || axes,
        axes,
        axes_threads,
        force,
    }))
}

fn failed(message: impl std::fmt::Display) -> CliError {
    CliError::Application(AppError::derivation(format!("p eps census: {message}")))
}

/// The census for `arguments`; `Ok(missing rows)`.
fn census(arguments: &Arguments) -> Result<usize, CliError> {
    let s = arguments.max_total;
    let names = [
        format!("eps-census-S{s}.tsv"),
        format!("eps-census-S{s}-summary.tsv"),
        format!("eps-census-S{s}-causes.tsv"),
        format!("eps-census-S{s}-closure.tsv"),
        "eps-census-universe.tsv".to_owned(),
        format!("eps-census-S{s}.json"),
        format!("eps-census-S{s}-axes.tsv"),
    ];
    std::fs::create_dir_all(&arguments.out_dir).map_err(|e| CliError::OutputIo(e.to_string()))?;
    if !arguments.force
        && let Some(name) = names.iter().find(|n| arguments.out_dir.join(n).exists())
    {
        return Err(CliError::Input(format!(
            "{} exists (use --force)",
            arguments.out_dir.join(name).display()
        )));
    }
    let started = Instant::now();
    let manifest = arguments.package.join("library.json");
    let bytes = std::fs::read(&manifest)
        .map_err(|e| CliError::InputIo(format!("{}: {e}", manifest.display())))?;
    let mut index = PackageIndex::from_manifest_bytes(&bytes)
        .map_err(|e| CliError::Input(format!("{}: {e}", manifest.display())))?;
    let manifest_seconds = started.elapsed().as_secs_f64();
    let mut load_seconds = None;
    let mut loaded_library = None;
    if arguments.full_load {
        let t = Instant::now();
        let library = load_p_library(
            PLibrarySource::Directory(arguments.package.clone()),
            &PLibraryOptions {
                verify_descent: false,
            },
        )
        .map_err(failed)?;
        let loaded = PackageIndex::from_library(&library);
        load_seconds = Some(t.elapsed().as_secs_f64());
        if loaded.by_key != index.by_key || loaded.tops != index.tops {
            return Err(failed(
                "the index of the loaded library differs from library.json's",
            ));
        }
        index = loaded;
        loaded_library = Some(library);
    }
    let t = Instant::now();
    let universes: BTreeMap<u8, LoopUniverse> = (1..=s).map(|l| (l, loop_universe(l))).collect();
    let universe_seconds = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let dags = generated_top_dags(&universes, &index);
    let dag_seconds = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let rows = census_rows(&universes, &index, s);
    let rows_seconds = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let closure = missing_closure(&rows, &index);
    let closure_seconds = t.elapsed().as_secs_f64();
    let axes = match (&loaded_library, arguments.axes) {
        (Some(library), true) => {
            let stream = arguments
                .out_dir
                .join(format!("eps-census-S{s}-axes-trace.tsv"));
            Some(axes_report(library, &rows, &stream, arguments.axes_threads).map_err(failed)?)
        }
        _ => None,
    };
    let axes_problems = axes.as_ref().map_or(0, |a| {
        a.rows
            .values()
            .map(|r| r.pinched + r.refused.unwrap_or(0))
            .sum::<usize>()
            + a.custom_not_two_sided.len()
    });
    let (mut universe, universe_json) = universe_table(&universes, &index);
    universe.push_str(
        "\nmax_loops\tdag_nodes_of_generated_tops\tnot_package_nodes\tpackage_nodes_not_in_dag\n",
    );
    for (l, nodes, outside, missing) in &dags {
        universe.push_str(&format!("{l}\t{nodes}\t{outside}\t{missing}\n"));
    }
    let summary = summary_tsv(&rows);
    let missing = rows.iter().filter(|r| r.presentation.is_none()).count();
    let slot_mismatch = rows.iter().filter(|r| r.slots_match == Some(false)).count();
    let multiple = rows.iter().filter(|r| r.presentations > 1).count();
    let report = serde_json::json!({
        "schema": "rustred.p-eps-census.v1",
        "package": arguments.package.display().to_string(),
        "identity": index.identity,
        "scope": index.scope,
        "max_total": s,
        "max_units_per_line": crate::application::propagator::eps_census::MAX_UNITS_PER_LINE,
        "index_source": if arguments.full_load { "load_p_library" } else { "library.json" },
        "rows": rows.len(),
        "present": rows.len() - missing,
        "missing": missing,
        "missing_but_sector": rows.iter().filter(|r| r.presentation.is_none() && r.sector_of.is_some()).count(),
        "slot_mismatches": slot_mismatch,
        "rows_with_several_presentations": multiple,
        "new_nodes_for_missing": closure.values().sum::<usize>(),
        "axes": axes.as_ref().map(|a| serde_json::json!({
            "presentations": a.rows.len(),
            "census_sectors": a.rows.values().map(|r| r.sectors).sum::<usize>(),
            "pinched_sectors": a.rows.values().map(|r| r.pinched).sum::<usize>(),
            "trace_powers_per_eps_line": crate::application::propagator::eps_census::AXIS_TRACE_POWERS,
            "trace_inputs": a.trace_inputs, "trace_distinct_keys": a.trace_distinct_keys,
            "trace_failures": a.trace_failures, "trace_samples": a.trace_samples, "trace_seconds": a.trace_seconds,
            "custom_eps_owners": a.custom_eps_owners, "custom_not_two_sided": a.custom_not_two_sided,
            "lane_eps_owners": a.lane_eps_owners, "problems": axes_problems,
        })),
        "universe": universe_json,
        "generated_top_dags": dags.iter().map(|(l, n, o, m)| serde_json::json!({
            "max_loops": l, "nodes": n, "not_package_nodes": o, "package_nodes_not_in_dag": m,
        })).collect::<Vec<_>>(),
        "seconds": {
            "manifest": manifest_seconds, "full_load": load_seconds, "universe": universe_seconds,
            "generated_top_dags": dag_seconds,
            "rows": rows_seconds, "closure": closure_seconds,
        },
    });
    let contents = [
        rows_tsv(&rows),
        summary.clone(),
        causes_tsv(&rows),
        closure_tsv(&closure),
        universe.clone(),
        serde_json::to_string_pretty(&report).map_err(failed)? + "\n",
    ];
    let axes_text = axes.as_ref().map(axes_tsv);
    let contents: Vec<&String> = contents.iter().chain(axes_text.as_ref()).collect();
    for (name, text) in names.iter().zip(contents) {
        std::fs::write(arguments.out_dir.join(name), text)
            .map_err(|e| CliError::OutputIo(e.to_string()))?;
    }
    let mut stdout = std::io::stdout().lock();
    let io = |e: std::io::Error| CliError::OutputIo(e.to_string());
    writeln!(
        stdout,
        "package {} ({})",
        arguments.package.display(),
        index.identity
    )
    .map_err(io)?;
    write!(stdout, "{universe}").map_err(io)?;
    write!(stdout, "{summary}").map_err(io)?;
    writeln!(
        stdout,
        "S = {s}: {} rows, {} present, {missing} missing, {slot_mismatch} slot mismatches; written to {}",
        rows.len(),
        rows.len() - missing,
        arguments.out_dir.display()
    )
    .map_err(io)?;
    if let Some(a) = &axes {
        writeln!(
            stdout,
            "axes: {} eps presentations, {} census sectors, {} pinched; {} Custom eps owners, {} not two-sided; trace {} inputs, {} keys, failures {:?} ({:.1} s)",
            a.rows.len(),
            a.rows.values().map(|r| r.sectors).sum::<usize>(),
            a.rows.values().map(|r| r.pinched).sum::<usize>(),
            a.custom_eps_owners,
            a.custom_not_two_sided.len(),
            a.trace_inputs,
            a.trace_distinct_keys,
            a.trace_failures,
            a.trace_seconds,
        )
        .map_err(io)?;
    }
    Ok(missing + axes_problems)
}

pub(super) fn run(arguments: Vec<OsString>) -> Result<(), CliError> {
    let Some(arguments) = parse(arguments).map_err(CliError::Usage)? else {
        let mut stdout = std::io::stdout().lock();
        return stdout
            .write_all(USAGE.as_bytes())
            .and_then(|()| stdout.flush())
            .map_err(|e| CliError::OutputIo(e.to_string()));
    };
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(arguments.threads)
        .build()
        .map_err(failed)?;
    let problems = pool.install(|| census(&arguments))?;
    if problems > 0 {
        // Exit status 1: the census verdict (the tables are written).
        return Err(CliError::Verdict {
            incomplete: false,
            message: format!(
                "p eps census: {problems} missing rows or failed axis checks (tables in {})",
                arguments.out_dir.display()
            ),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(arguments: &[&str]) -> Result<Arguments, ArgError> {
        parse(arguments.iter().map(OsString::from).collect())
            .map(|parsed| parsed.expect("not --help"))
    }

    #[test]
    fn p_eps_census_parses_its_options() {
        let required = ["--package", "p", "--max-total", "4", "--out-dir", "o"];
        assert_eq!(
            parsed(&required).unwrap(),
            Arguments {
                package: "p".into(),
                max_total: 4,
                out_dir: "o".into(),
                threads: 1,
                full_load: false,
                axes: false,
                axes_threads: 4,
                force: false,
            }
        );
        let all = [&required[..], &["--threads", "8", "--full-load", "--force"]].concat();
        let all = parsed(&all).unwrap();
        assert_eq!((all.threads, all.full_load, all.force), (8, true, true));
        let axes = parsed(&[&required[..], &["--axes"]].concat()).unwrap();
        assert_eq!((axes.axes, axes.full_load), (true, true));
        for wrong in ["0", "7", "four"] {
            assert!(parsed(&["--package", "p", "--max-total", wrong, "--out-dir", "o"]).is_err());
        }
        assert!(matches!(
            parsed(&["--package", "p", "--out-dir", "o"]),
            Err(ArgError::MissingRequiredOption("--max-total"))
        ));
        assert!(matches!(parsed(&["--x"]), Err(ArgError::UnknownOption(_))));
        for help in ["--help", "-h"] {
            assert_eq!(parse(vec![OsString::from(help)]).unwrap(), None);
        }
        assert!(parse(vec!["--help".into(), "--force".into()]).is_err());
    }
}
