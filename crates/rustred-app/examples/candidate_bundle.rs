//! Offline candidate generation and independent cold-load/catalog binding check.
//!
//! These bundles are not certified closing artifacts. Generation is deliberately
//! a separate invocation from loading, so downstream evaluation never solves.
use std::collections::BTreeSet;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::time::Instant;

use rustred::persistence::{BinaryIoLimits, ExactTerminalCatalog, TerminalCatalogCoverage};
use rustred::reduction::ReductionLimits;
use rustred_app::{
    CandidateBundleLimits, FamilyCandidatesRequest, MAX_CANDIDATE_BUNDLE_BYTES, family_candidates,
    inspect_generated_candidate_bundle, load_generated_candidate_bundle,
};

fn limits() -> CandidateBundleLimits {
    CandidateBundleLimits {
        max_bundle_bytes: MAX_CANDIDATE_BUNDLE_BYTES,
        max_collection_entries: 8_000_000,
        max_total_coefficient_bytes: 512 * 1024 * 1024,
        ..CandidateBundleLimits::default()
    }
}

fn write_new(path: &str, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)?
        .write_all(bytes)?;
    Ok(())
}

fn verify<const N: usize>(bytes: &[u8], catalog: &[u8]) -> Result<(), Box<dyn Error>> {
    let started = Instant::now();
    let (family, reducer) =
        load_generated_candidate_bundle::<N>(bytes, limits(), ReductionLimits::default())?;
    let catalog = ExactTerminalCatalog::decode_generated(
        catalog,
        family.fingerprint(),
        N,
        BinaryIoLimits::default(),
    )?;
    if catalog.coverage() != TerminalCatalogCoverage::Complete {
        return Err("candidate verification requires a declaration-complete catalog".into());
    }
    let expected: BTreeSet<Vec<i64>> = catalog
        .terms()
        .keys()
        .map(|key| key.powers().to_vec())
        .collect();
    let actual: BTreeSet<_> = reducer
        .terminals()
        .iter()
        .map(|key| key.powers().to_vec())
        .collect();
    if expected != actual {
        return Err(format!(
            "catalog key mismatch: expected {}, loaded {}; missing {:?}; extra {:?}",
            expected.len(),
            actual.len(),
            expected.difference(&actual).take(3).collect::<Vec<_>>(),
            actual.difference(&expected).take(3).collect::<Vec<_>>()
        )
        .into());
    }
    println!(
        "cold_load_us={} terminals={} fingerprint_matches=true exact_terminal_set_matches=true status=uncertified-candidates",
        started.elapsed().as_micros(),
        actual.len()
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("inspect") if args.len() == 3 => {
            let mut bytes = Vec::new();
            fs::File::open(&args[2])?.take(MAX_CANDIDATE_BUNDLE_BYTES as u64 + 1).read_to_end(&mut bytes)?;
            let info = inspect_generated_candidate_bundle(&bytes, limits())?;
            println!(
                "schema={} status={} arity={} sectors={} rules={} terminals={} unique_coefficients={} state_bytes={} coefficient_table_bytes={} program_bytes={} fingerprint={}",
                info.schema, info.status, info.arity, info.solved_sectors,
                info.generated_rules, info.finite_residuals, info.unique_coefficients,
                info.symbolica_state_bytes, info.coefficient_table_bytes, bytes.len(),
                info.family_fingerprint,
            );
        }
        Some("generate") if matches!(args.len(), 8 | 9) => {
            if Path::new(&args[6]).exists() || Path::new(&args[7]).exists() || args[6] == args[7] {
                return Err("bundle and report paths must be distinct and new".into());
            }
            let mut request = FamilyCandidatesRequest::new(fs::read_to_string(&args[2])?);
            request.nonpositive_indices = args[3].split(',').map(str::parse).collect::<Result<_, _>>()?;
            request.n_cores = args[4].parse()?;
            request.exact_backend = args.get(8).map(String::as_str).unwrap_or("sparse").parse()?;
            request.permutation = if args[5] == "default" { None } else {
                Some(args[5].split(',').map(str::parse).collect::<Result<_, _>>()?)
            };
            request.bundle_limits = limits();
            let result = family_candidates(request)?;
            write_new(&args[6], result.bundle())?;
            write_new(&args[7], result.to_toml().as_bytes())?;
            println!("{}", result.to_toml());
        }
        Some("verify") if args.len() == 5 => {
            let mut bytes = Vec::new();
            fs::File::open(&args[3])?.take(MAX_CANDIDATE_BUNDLE_BYTES as u64 + 1).read_to_end(&mut bytes)?;
            if bytes.len() > MAX_CANDIDATE_BUNDLE_BYTES { return Err("bundle exceeds byte cap".into()); }
            let mut catalog = Vec::new();
            let cap = BinaryIoLimits::default().max_program_bytes;
            fs::File::open(&args[4])?.take(cap as u64 + 1).read_to_end(&mut catalog)?;
            if catalog.len() > cap { return Err("catalog exceeds byte cap".into()); }
            macro_rules! dispatch {
                ($($n:literal),+) => { match args[2].parse::<usize>()? {
                    $($n => verify::<$n>(&bytes, &catalog)?,)+
                    _ => return Err("arity must be 1 through 16".into()),
                } };
            }
            dispatch!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16);
        }
        _ => return Err("usage: candidate_bundle generate INPUT NONPOSITIVE_INDICES WORKERS PERMUTATION_OR_default NEW_BUNDLE NEW_REPORT [sparse|semi-numerical] | inspect BUNDLE | verify ARITY BUNDLE TERMINAL_CATALOG".into()),
    }
    Ok(())
}
