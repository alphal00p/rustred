//! Generate the canonical unit-mass K6 artifact using the generic source port.
//! Usage: spired-generate-k6 <new-output.rr> [workers]
//! Existing paths are never overwritten. The CLI owns fresh-process inspection
//! and reduction; this example adds no artifact reader or runtime applier.

use std::error::Error;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use rustred::algebra::CoefficientContext;
use rustred::family::{AffineDenominator, IntegralFamily};
use rustred::foundry::artifact::SourcePortAudit;
use rustred::sector::{Mask, zero};
use rustred::solver::{SectorConfig, SectorExecutor, SectorSolveOptions, SourceSystem};

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut args = std::env::args_os().skip(1);
    let output = PathBuf::from(
        args.next()
            .ok_or("usage: spired-generate-k6 <new-output.rr> [workers]")?,
    );
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
    if args.next().is_some() || output.try_exists()? {
        return Err(
            "expected a new output path and optional worker count; refusing overwrite".into(),
        );
    }
    let executor = SectorExecutor::new(workers)?;
    let start = Instant::now();

    // Exact public-constructor inputs of the canonical K6 family, not the
    // differently routed supplied vac3 reference fixture. Scalar coordinates
    // are k1²,k1.k2,k1.k3,k2²,k2.k3,k3²; all masses are exactly one.
    let context = CoefficientContext::try_new(["d"])?;
    let rows = [
        [1, 0, 0, 0, 0, 0],
        [0, 0, 0, 1, 0, 0],
        [0, 0, 0, 0, 0, 1],
        [1, 0, -2, 0, 0, 1],
        [1, -2, 0, 1, 0, 0],
        [0, 0, 0, 1, -2, 1],
    ];
    let denominators = rows
        .into_iter()
        .map(|row| {
            AffineDenominator::new(
                context.integer(-1),
                row.into_iter()
                    .map(|value| context.integer(value))
                    .collect(),
            )
        })
        .collect();
    let family = IntegralFamily::new(
        "rustred-three-loop-unit-mass-vacuum-k6-v1",
        vec!["k1".into(), "k2".into(), "k3".into()],
        Vec::new(),
        context.clone(),
        context
            .parameter("d")
            .ok_or("missing dimension parameter")?,
        denominators,
        Vec::new(),
        vec![context.zero(); 6],
    )?;
    let analyzer = zero::Analyzer::try_unrestricted(&family)?;
    let mut zeros = Vec::new();
    let mut sectors = Vec::new();
    for bits in 0usize..64 {
        let sector: [bool; 6] = std::array::from_fn(|axis| bits & (1 << axis) != 0);
        match analyzer.analyze(&Mask::try_new(sector)?)? {
            zero::Decision::ProvedZero(_) => zeros.push(sector),
            zero::Decision::Inconclusive(_) => sectors.push(sector),
            zero::Decision::Excluded(_) => {
                return Err("unrestricted canonical sector was excluded".into());
            }
        }
    }
    drop(analyzer);
    sectors.sort_unstable();
    let zeros: Arc<[[bool; 6]]> = zeros.into();
    let sources = SourceSystem::from_family(&family)?;
    let audit = SourcePortAudit::try_new(&family, zeros.clone())?;
    let prepared = start.elapsed();
    let solved = executor.map(
        &sources,
        &sectors,
        &SectorConfig {
            zero_sectors: zeros.clone(),
            ..Default::default()
        },
        SectorSolveOptions::default(),
        |done| Ok::<_, std::io::Error>((done.sector, None, done.solution)),
    )?;
    let generated_rules: usize = solved
        .iter()
        .map(|(_, _, solution)| solution.rules.len())
        .sum();
    let generated = start.elapsed();
    let artifact = audit.install_complete(family, solved)?;
    let installed = start.elapsed();
    let bytes = artifact.encode_durable()?;
    let encoded = start.elapsed();
    // The no-clobber create is repeated after generation to avoid races.
    // A failed write is an error and never a valid published-artifact claim.
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    println!("schema={}", artifact.schema().stable_id());
    println!("algorithm_id={}", artifact.algorithm_id());
    println!("family_fingerprint={}", artifact.family_fingerprint());
    println!("context_fingerprint={}", artifact.context_fingerprint());
    println!(
        "sectors={} zero_sectors={} generated_rules={} rule_cells={} terminals={} bytes={} workers={workers}",
        sectors.len(),
        zeros.len(),
        generated_rules,
        artifact.rule_cells().len(),
        artifact.masters().len(),
        bytes.len()
    );
    for master in artifact.masters() {
        println!("terminal={:?}", master.powers());
    }
    println!(
        "prepared_us={} generation_us={} installation_us={} encode_us={} total_us={}",
        prepared.as_micros(),
        (generated - prepared).as_micros(),
        (installed - generated).as_micros(),
        (encoded - installed).as_micros(),
        start.elapsed().as_micros()
    );
    Ok(())
}
