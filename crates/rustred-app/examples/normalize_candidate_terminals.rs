//! Offline public-API adapter for Python-steered artifact examples.
//! No topology selection, IBP implementation, numerical master evaluation, or
//! publication authority lives here. Build once in release mode, then invoke
//! separate processes for generation and normalization-proof replay at loading.
//! This does not add independent IBP-source replay to candidate programs.

use std::collections::BTreeSet;
use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::time::Instant;

use rustred::persistence::{
    BinaryIoLimits, ExactTerminalCatalog, TerminalCatalogCoverage, equivalent_generated_programs,
};
use rustred::reduction::ReductionLimits;
use rustred::reduction::terminal_normalization::TerminalNormalizationPlan;
use rustred_app::{
    CandidateBundleLimits, MAX_CANDIDATE_BUNDLE_BYTES, inspect_generated_candidate_bundle,
    load_generated_candidate_bundle,
};
use serde_json::json;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn limits() -> CandidateBundleLimits {
    CandidateBundleLimits {
        max_bundle_bytes: MAX_CANDIDATE_BUNDLE_BYTES,
        max_collection_entries: 8_000_000,
        max_total_coefficient_bytes: 512 * 1024 * 1024,
        ..Default::default()
    }
}

fn read(path: &str, cap: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(cap as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > cap {
        return Err(format!("input exceeds {cap} bytes: {path}").into());
    }
    Ok(bytes)
}

fn write_new(path: &str, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn normalization<const N: usize>(args: &[String]) -> Result<()> {
    let started = Instant::now();
    let bytes = read(&args[3], MAX_CANDIDATE_BUNDLE_BYTES)?;
    let info = inspect_generated_candidate_bundle(&bytes, limits())?;
    let (family, mut reducer) =
        load_generated_candidate_bundle::<N>(&bytes, limits(), ReductionLimits::default())?;
    let plan = match args[1].as_str() {
        "generate" => TerminalNormalizationPlan::vacuum_quadratic_numerators(
            &family,
            reducer.terminals(),
            reducer.ordering(),
            Default::default(),
        )?,
        "verify" => TerminalNormalizationPlan::decode_generated(
            &read(&args[4], BinaryIoLimits::default().max_program_bytes)?,
            &family,
            reducer.terminals(),
            reducer.ordering(),
            Default::default(),
            Default::default(),
        )?,
        _ => unreachable!(),
    };
    // Catalog values are supplied independently. Exact keys/family binding is
    // necessary but does not prove those values or discover new master values.
    let catalog_checked = args[6] != "-";
    if catalog_checked {
        let catalog = ExactTerminalCatalog::decode_generated(
            &read(&args[6], BinaryIoLimits::default().max_program_bytes)?,
            family.fingerprint(),
            N,
            Default::default(),
        )?;
        if catalog.coverage() != TerminalCatalogCoverage::Complete
            || catalog.terms().keys().cloned().collect::<BTreeSet<_>>()
                != *plan.canonical_terminals()
        {
            return Err("precomputed catalog differs from freshly normalized output keys; do not synthesize or drop values".into());
        }
    }
    let stats = plan.statistics();
    let report = json!({
        "schema": "rustred.example-terminal-normalization.v1", "operation": args[1],
        "family_fingerprint": family.fingerprint(), "arity": N,
        "ordering": reducer.ordering().stable_id().to_string(),
        "candidate_status": info.status, "solved_sectors": info.solved_sectors,
        "generated_rules": info.generated_rules, "finite_residuals": info.finite_residuals,
        "max_numerator_rank": info.max_numerator_rank,
        "numerical_depth": info.numerical_depth, "finite_case_policy": info.finite_case_policy.as_str(),
        "raw_terminals": stats.raw_terminals, "normalized_terminals": stats.canonical_terminals,
        "projected_numerators": stats.projected_numerators,
        "verified_generators": stats.verified_generators,
        "catalog_exact_output_keys": catalog_checked,
        "catalog_values_generated": false, "candidate_closure_certified": false,
        "output_keys": plan.canonical_terminals().iter().map(|key| key.powers()).collect::<Vec<_>>(),
        "elapsed_seconds": started.elapsed().as_secs_f64(),
    });
    let encoded = (args[1] == "generate")
        .then(|| plan.encode_native(Default::default()))
        .transpose()?;
    // Installation independently checks family, ordering, exact raw inventory,
    // coefficient context and an empty cache before a success receipt is saved.
    reducer.install_terminal_normalization(plan)?;
    if let Some(encoded) = encoded {
        write_new(&args[4], &encoded)?;
    }
    write_new(&args[5], &serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string(&report)?);
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("compare") if args.len() == 5 => {
            let io = BinaryIoLimits {
                max_program_bytes: MAX_CANDIDATE_BUNDLE_BYTES,
                max_collection_entries: 8_000_000,
                max_total_atom_bytes: 512 * 1024 * 1024,
                ..Default::default()
            };
            let equal = equivalent_generated_programs(
                &read(&args[2], io.max_program_bytes)?, &read(&args[3], io.max_program_bytes)?, io,
            )?;
            write_new(&args[4], &serde_json::to_vec_pretty(&json!({
                "equivalent_generated_programs": equal,
                "scope": "exact native envelope structures and coefficient meaning; no new closure claim"
            }))?)?;
            if !equal {
                return Err("freshly generated program differs from the supplied reference; keep both and investigate".into());
            }
            Ok(())
        }
        Some("generate" | "verify") if args.len() == 7 => {
            if Path::new(&args[5]).exists()
                || (args[1] == "generate" && Path::new(&args[4]).exists())
            {
                return Err("normalization output/report must be new".into());
            }
            // Same runtime-arity dispatch pattern as candidate_bundle: there is
            // no topology- or loop-specific normalization implementation.
            macro_rules! dispatch { ($($n:literal),+) => { match args[2].parse::<usize>()? {
                $($n => normalization::<$n>(&args),)+
                _ => Err("arity must be 1 through 16".into()),
            } }; }
            dispatch!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)
        }
        _ => Err("usage: normalize_candidate_terminals generate|verify ARITY PROGRAM PLAN NEW_REPORT CATALOG_OR_- | compare GENERATED REFERENCE NEW_REPORT".into()),
    }
}
