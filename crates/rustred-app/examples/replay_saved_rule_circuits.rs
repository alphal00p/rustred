//! Input-driven selected circuit probe. Outputs bounded diagnostics only;
//! no coefficient display is parsed, and no rule/artifact is installed.
//! Report-byte limits bound emission; callback allocation is bounded by the
//! retained counts and the caller's external process-RSS guard, not streaming.

use rustred::foundry::artifact::ReplayCircuitLimits;
use rustred_app::{CandidateBundleLimits, with_replayed_candidate_rule_circuits};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{fs::File, io::Read, time::Instant};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    bundle: String,
    sector: Vec<bool>,
    ordinals: Vec<usize>,
    max_bundle_bytes: usize,
    max_total_coefficient_bytes: usize,
    max_collection_entries: usize,
    max_coefficient_bytes: usize,
    max_report_bytes: usize,
    retention: Retention,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Retention {
    max_rules: usize,
    max_source_entries: usize,
    max_rhs_terms: usize,
    max_conditions: usize,
    max_coefficient_terms: usize,
    max_coefficient_clone_owned_bytes: usize,
    max_coordinate_cells: usize,
}

fn read(path: &str, limit: usize) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    if limit == 0 || file.metadata().map_err(|e| e.to_string())?.len() > limit as u64 {
        return Err("input exceeds explicit byte allowance".into());
    }
    let mut bytes = Vec::new();
    file.take((limit as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("input grew beyond allowance".into());
    }
    Ok(bytes)
}

fn run<const N: usize>(bytes: &[u8], request: &Request) -> Result<Value, String> {
    let limits = CandidateBundleLimits {
        max_bundle_bytes: request.max_bundle_bytes,
        max_total_coefficient_bytes: request.max_total_coefficient_bytes,
        max_collection_entries: request.max_collection_entries,
        max_coefficient_bytes: request.max_coefficient_bytes,
        ..Default::default()
    };
    let r = &request.retention;
    let retention = ReplayCircuitLimits {
        max_rules: r.max_rules,
        max_source_entries: r.max_source_entries,
        max_rhs_terms: r.max_rhs_terms,
        max_conditions: r.max_conditions,
        max_coefficient_terms: r.max_coefficient_terms,
        max_coefficient_clone_owned_bytes: r.max_coefficient_clone_owned_bytes,
        max_coordinate_cells: r.max_coordinate_cells,
    };
    let sector = request
        .sector
        .as_slice()
        .try_into()
        .map_err(|_| "sector arity mismatch".to_owned())?;
    with_replayed_candidate_rule_circuits::<N, _>(
        bytes,
        limits,
        sector,
        &request.ordinals,
        Default::default(),
        retention,
        |batch| {
            let rules = batch.circuits().iter().map(|rule| json!({
                "ordinal":rule.ordinal(), "fixed":rule.declared_case().fixed().to_vec(),
                "raw_pivot_values":rule.raw_target().powers().iter().map(|p|p.value()).collect::<Vec<_>>(),
                "raw_pivot_symbolic":rule.raw_target().powers().iter().map(|p|p.is_symbolic()).collect::<Vec<_>>(),
                "raw_recenter":rule.raw_recenter().to_vec(),
                "original_source_entries":rule.contributions().len(),
                "sources":rule.contributions().iter().map(|s|json!({
                    "row":s.source_row.stable_string(),"canonical_offset":s.offset.values(),
                    "weight_numerator_terms":s.weight.raw().numerator.nterms(),
                    "weight_denominator_terms":s.weight.raw().denominator.nterms()
                })).collect::<Vec<_>>(),
                "rhs_terms":rule.rhs().len(), "nonzero_conditions":rule.nonzero_conditions().len(),
                "exception_branches":rule.declared_exceptions().branches.len(),
                "application_boxes":rule.application_boxes().map(|(l,u)|json!({"lower":l,"upper":u})).collect::<Vec<_>>()
            })).collect::<Vec<_>>();
            Ok(json!({"schema":"rustred.selected-rule-circuit-probe.v1",
                "status":"REPLAYED_DECLARED_DOMAIN_MODULO_AUTHENTICATED_ZEROS",
                "family_fingerprint":batch.family_fingerprint(),"context_fingerprint":batch.context().fingerprint(),
                "ordering":batch.ordering().stable_id().to_string(),
                "root_sector":batch.root_sector().active_bits(),"sector":batch.sector().to_vec(),
                "authenticated_zero_sectors":batch.zero_certificates().len(),
                "replay_seconds":batch.elapsed().as_secs_f64(),"rules":rules,
                "literal_full_image_claim":false,"descent_claim":false,"closure_claim":false,
                "artifact_written":false,"production_modified":false}))
        },
    )
    .map_err(|e| e.to_string())
}

fn main_result() -> Result<(), String> {
    let start = Instant::now();
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 2 {
        return Err("usage: replay_saved_rule_circuits REQUEST.json".into());
    }
    let request: Request =
        serde_json::from_slice(&read(&args[1], 1024 * 1024)?).map_err(|e| e.to_string())?;
    if request.ordinals.is_empty() || request.max_report_bytes == 0 {
        return Err("a nonempty selected rule batch and report allowance are required".into());
    }
    let bytes = read(&request.bundle, request.max_bundle_bytes)?;
    macro_rules! dispatch { ($($n:literal),+) => { match request.sector.len() {
        $($n => run::<$n>(&bytes, &request),)+
        _ => Err("native candidate arity must be in 1..=16".to_owned()),
    } }; }
    let mut report = dispatch!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)
        .map_err(|e| e.to_string())?;
    report["total_seconds"] = start.elapsed().as_secs_f64().into();
    let text = serde_json::to_string(&report).map_err(|e| e.to_string())?;
    if text.len() > request.max_report_bytes {
        return Err("diagnostic report exceeds allowance".into());
    }
    println!("{text}");
    Ok(())
}

fn main() {
    if let Err(error) = main_result() {
        eprintln!(
            "{}",
            json!({"status":"REFUSED_OR_INCOMPLETE","error":error,"artifact_written":false})
        );
        std::process::exit(2);
    }
}
