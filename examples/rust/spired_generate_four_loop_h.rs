//! Generic source-port artifact probe for the external four-loop H example.
//!
//! This executable is deliberately a small study driver: the closure and
//! artifact machinery receive an ordinary `IntegralFamily` and never branch on
//! its name or graph.  A production CLI will replace the example constructor
//! with the parsed `examples/input/four_loop_h.toml` project once generic
//! closure orchestration is exposed at the application boundary.
//!
//! Usage: spired-generate-four-loop-h <new-output.rr> [workers] [permutation]

use std::error::Error;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;
use std::time::Instant;

use rustred::foundry::artifact::SourcePortAudit;
use rustred::sector::{Mask, zero};
use rustred::solver::{SectorConfig, SectorExecutor, SectorSolveOptions, SourceSystem};

#[path = "support/spired_families.rs"]
mod spired_families;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let output = args
        .next()
        .ok_or("usage: spired-generate-four-loop-h <new-output.rr> [workers] [permutation]")?;
    let output = std::path::PathBuf::from(output);
    let workers = args
        .next()
        .map(|value| {
            value
                .into_string()
                .map_err(|_| "workers must be an integer")?
                .parse::<usize>()
                .map_err(|_| "workers must be an integer")
        })
        .transpose()?
        .unwrap_or(1);
    let permutation = args
        .next()
        .map(|value| {
            let text = value
                .into_string()
                .map_err(|_| "permutation must be comma-separated integers".to_owned())?;
            let values = text
                .split(',')
                .map(|item| {
                    item.parse::<usize>()
                        .map_err(|_| "permutation must be comma-separated integers".to_owned())
                })
                .collect::<std::result::Result<Vec<_>, _>>()?;
            values
                .try_into()
                .map_err(|_: Vec<usize>| "permutation must contain exactly ten coordinates".to_owned())
        })
        .transpose()?;
    if args.next().is_some() || output.try_exists()? {
        return Err(
            "expected a new output path and optional worker count; refusing overwrite".into(),
        );
    }
    let family = spired_families::four_loop_h()?;
    let analyzer = zero::Analyzer::try_unrestricted(&family)?;
    let mut zeros = Vec::new();
    let mut sectors = Vec::new();
    for bits in 0usize..(1usize << family.denominator_count()) {
        let sector = std::array::from_fn(|axis| bits & (1 << axis) != 0);
        match analyzer.analyze(&Mask::try_new(sector)?)? {
            zero::Decision::ProvedZero(_) => zeros.push(sector),
            zero::Decision::Inconclusive(_) => sectors.push(sector),
            zero::Decision::Excluded(_) => return Err("unrestricted sector was excluded".into()),
        }
    }
    sectors.sort_unstable();
    let zeros: Arc<[[bool; 10]]> = zeros.into();
    let sources = SourceSystem::<10>::from_family(&family)?;
    let audit = SourcePortAudit::try_new(&family, zeros.clone())?;
    let executor = SectorExecutor::new(workers)?;
    let start = Instant::now();
    let solved = executor.map(
        &sources,
        &sectors,
        &SectorConfig {
            zero_sectors: zeros,
            permutation,
            ..Default::default()
        },
        SectorSolveOptions::default(),
        |done| Ok::<_, std::io::Error>((done.sector, None, done.solution)),
    )?;
    let solved_count = solved.len();
    let artifact = audit.install_complete(family, solved)?;
    let bytes = artifact.encode_durable()?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    println!(
        "schema={} arity={} sectors={} terminals={} rule_cells={} bytes={} workers={} total_us={}",
        artifact.schema().stable_id(),
        artifact.arity(),
        solved_count,
        artifact.masters().len(),
        artifact.rule_cells().len(),
        bytes.len(),
        workers,
        start.elapsed().as_micros()
    );
    Ok(())
}
