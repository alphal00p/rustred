//! Finite verified momentum-change rows and their portable exact certificates.
use super::*;
use rustred::family::IntegralKey;
use rustred::reduction::terminal_normalization::{
    TerminalNumeratorSymmetryEquations, TerminalNumeratorSymmetryLimits,
};
use rustred::reduction::terminal_relations::TerminalEquation;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

const PREFIX: &str = "numerator-symmetry-assistance-v1:";
const RECIPE: &str = "verified-denominator-preserving-numerator-maps-v1";

/// This opt-in trusted-local refinement is governed by the caller's process
/// RAM guard, not by the general-purpose application's preset work counters.
/// Exact representation bounds and checked arithmetic remain unchanged.
pub(super) fn configure_saved_resources(
    request: &mut crate::application::routed_campaign::RoutedCampaignRequest,
) {
    use rustred::algebra::{ExactAlgebraLimits, IndexedAlgebraLimits};
    use rustred::reduction::ReductionLimits;
    use rustred::sector::symmetry::integral_transport::ExpansionLimits;
    use rustred::solver::RoutedCandidateLimits;

    request.reduction_limits = ReductionLimits {
        exact_algebra: ExactAlgebraLimits::unbounded_resources(),
        indexed_algebra: IndexedAlgebraLimits::unbounded_resources(),
        max_rule_applications: usize::MAX,
        max_cached_integrals: usize::MAX,
        max_cached_coefficient_terms: usize::MAX,
        max_cached_coefficient_bytes: usize::MAX,
        max_pending_frames: usize::MAX,
        max_coalescing_additions: usize::MAX,
        max_factorization_terms: usize::MAX,
    };
    request.trace_limits = RoutedCandidateLimits {
        max_input_targets: usize::MAX,
        max_unique_nodes: usize::MAX,
        max_transport_calls: usize::MAX,
        max_transport_operations: usize::MAX,
        max_transport_endpoints: usize::MAX,
        expansion: ExpansionLimits::unbounded_resources(),
    };
}

/// Optionally apply already supplied candidate programs to each COMPLETE
/// identity. A missing saved reduction leaves the original exact source row,
/// not a partial RHS or a new terminal. Failures are explicit finite receipts.
pub(super) fn prepare_rows(
    plan: &TerminalNumeratorSymmetryEquations,
    mut saved: Option<&mut crate::application::routed_campaign::PreparedTerminalEquations>,
    report: &mut Value,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
    started: Instant,
) -> Result<BTreeMap<IntegralKey, Vec<TerminalEquation>>, AppError> {
    let mut result = BTreeMap::new();
    let mut processed = 0usize;
    let mut reduced = 0usize;
    let mut failures = Vec::new();
    let mut last_event = Instant::now();
    report["stage"] = json!("reducing_numerator_symmetry_equations");
    for key in plan.input_keys() {
        let mut rows = Vec::new();
        for (ordinal, equation) in plan.equations(key).iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                return Err(AppError::execution(
                    "paused before applying the next exact numerator-symmetry equation",
                ));
            }
            let row = if let Some(saved) = &mut saved {
                match (saved.reduce_equation)(equation) {
                    Ok(row) => {
                        reduced += 1;
                        row
                    }
                    Err(error) => {
                        failures.push(json!({
                            "kind": error.kind(),
                            "source": key.powers(),
                            "equation": ordinal,
                            "diagnostic": error.to_string(),
                            "fallback": "original exact source row retained; offspring remain unresolved auxiliary columns",
                        }));
                        equation.clone()
                    }
                }
            } else {
                equation.clone()
            };
            processed += 1;
            if !row.terms.is_empty() && !rows.contains(&row) {
                rows.push(row);
            }
            if last_event.elapsed() >= Duration::from_millis(250) {
                report["numerator_symmetry_reduction"] = json!({
                    "processed_rows": processed,
                    "source_rows": plan.statistics().equations,
                    "saved_reduced_rows": reduced,
                    "saved_application_failures": failures.len(),
                });
                observer(event(report, "master_reduction_progress", started));
                last_event = Instant::now();
            }
        }
        if !rows.is_empty() {
            result.insert(key.clone(), rows);
        }
    }
    report["numerator_symmetry_reduction"] = json!({
        "processed_rows": processed,
        "source_rows": plan.statistics().equations,
        "saved_reduced_rows": reduced,
        "saved_application_failures": failures.len(),
        "failure_diagnostics": failures,
        "scope": "finite witnessed source identities; no new IBP generation and no new terminal declarations",
        "source_identity_certificate": report["numerator_symmetry_certificate"],
    });
    Ok(result)
}

pub(super) fn binding(
    family: &str,
    keys: &BTreeSet<IntegralKey>,
    normalization: MasterNormalizationProfile,
) -> Result<String, AppError> {
    let mut hash = blake3::Hasher::new();
    hash.update(family.as_bytes());
    hash.update(&u64::try_from(keys.len()).map_err(io)?.to_le_bytes());
    for key in keys {
        for power in key.powers() {
            hash.update(&power.to_le_bytes());
        }
    }
    normalization.hash(&mut hash);
    Ok(format!("{PREFIX}{}", hash.finalize().to_hex()))
}

pub(super) fn prepare(
    options: &MasterReductionOptions,
    session: &TerminalRelationSession,
    report: &mut Value,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
    started: Instant,
) -> Result<TerminalNumeratorSymmetryEquations, AppError> {
    report["stage"] = json!("preparing_numerator_symmetry_equations");
    observer(event(report, "master_reduction_progress", started));
    let keys = session
        .raw_terminals()
        .union(session.normalization().canonical_terminals())
        .cloned()
        .collect();
    let prepared = if let Some(prepared) = load(&options.directory, session, report)? {
        if prepared.input_keys() != &keys {
            return Err(AppError::input(
                "numerator-symmetry certificate inventory differs from the active finite provider",
            ));
        }
        prepared
    } else {
        let prepared = TerminalNumeratorSymmetryEquations::prepare(
            Arc::clone(session.family_owner()),
            &keys,
            TerminalNumeratorSymmetryLimits::default(),
        )
        .map_err(io)?;
        let bytes = prepared
            .encode_native(profile::native_io_limits(None))
            .map_err(io)?;
        let digest = blake3::hash(&bytes).to_hex().to_string();
        let file = format!("numerator-symmetry-{digest}.rrbin");
        write_file_atomically(&options.directory.join(&file), &bytes, true).map_err(io)?;
        report["numerator_symmetry_certificate"] = json!({
            "recipe": RECIPE,
            "file": file,
            "bytes": bytes.len(),
            "blake3": digest,
            "inventory_binding": binding(
                prepared.family_fingerprint(),
                prepared.input_keys(),
                profile::from_report(report)?,
            )?,
            "authority": "exact replayed loop-momentum maps and complete numerator transport; not ordinary IBP sources",
        });
        prepared
    };
    let stats = prepared.statistics();
    report["numerator_symmetry_preparation"] = json!({
        "input_keys": stats.input_keys,
        "analyzed_supports": stats.analyzed_supports,
        "analyzed_denominators": stats.analyzed_denominators,
        "proposed_generators": stats.proposed_generators,
        "verified_generators": stats.verified_generators,
        "equation_targets": stats.equation_targets,
        "equations": stats.equations,
        "output_terms": stats.output_terms,
        "trivial_equations": stats.trivial_equations,
        "rejected_power_permutations": stats.rejected_power_permutations,
        "skipped": stats.skipped.iter().map(|(reason,count)| json!({
            "reason":format!("{reason:?}"), "count":count,
        })).collect::<Vec<_>>(),
        "scope": "raw and normalized finite retained inventory only; transported offspring remain unresolved columns",
    });
    if cancel.load(Ordering::Relaxed) {
        return Err(AppError::execution(
            "paused after preparing exact numerator-symmetry equations",
        ));
    }
    Ok(prepared)
}

/// Cold-replay the finite witness set. A provider checkpoint taken before its
/// first preparation has no accepted assisted rows and may regenerate it.
pub(super) fn validate(
    directory: &Path,
    session: &TerminalRelationSession,
    report: &Value,
) -> Result<(), AppError> {
    let _ = load(directory, session, report)?;
    Ok(())
}

fn load(
    directory: &Path,
    session: &TerminalRelationSession,
    report: &Value,
) -> Result<Option<TerminalNumeratorSymmetryEquations>, AppError> {
    let component = session
        .assistance_binding()
        .and_then(|binding| binding.split(';').find(|part| part.starts_with(PREFIX)));
    let Some(component) = component else {
        return Ok(None);
    };
    let descriptor = &report["numerator_symmetry_certificate"];
    if descriptor.is_null() {
        let stats = session.statistics();
        if stats.completed_assistance_rows == 0
            && stats.completed_assistance_keys == 0
            && stats.pending_assistance_rows == 0
            && !matches!(
                report["status"].as_str(),
                Some("published_unrefined" | "completed_nonminimal")
            )
        {
            return Ok(None);
        }
        return Err(AppError::input(
            "accepted numerator-symmetry rows have no portable exact certificate",
        ));
    }
    if descriptor["recipe"] != RECIPE {
        return Err(AppError::input(
            "unknown numerator-symmetry certificate recipe",
        ));
    }
    let file = descriptor["file"]
        .as_str()
        .ok_or_else(|| AppError::input("numerator-symmetry certificate path missing"))?;
    let limits = profile::native_io_limits(None);
    let bytes = storage::read_bounded(
        &storage::safe_child(directory, file)?,
        limits.max_program_bytes,
    )?;
    if descriptor["bytes"].as_u64() != Some(bytes.len() as u64)
        || descriptor["blake3"].as_str() != Some(blake3::hash(&bytes).to_hex().as_str())
    {
        return Err(AppError::input(
            "numerator-symmetry certificate size/digest mismatch",
        ));
    }
    let plan = TerminalNumeratorSymmetryEquations::from_native_bytes(
        &bytes,
        Arc::clone(session.family_owner()),
        TerminalNumeratorSymmetryLimits::default(),
        limits,
    )
    .map_err(io)?;
    let actual = binding(
        plan.family_fingerprint(),
        plan.input_keys(),
        profile::from_report(report)?,
    )?;
    if actual != component || descriptor["inventory_binding"].as_str() != Some(component) {
        return Err(AppError::input(
            "numerator-symmetry certificate/provider binding mismatch",
        ));
    }
    Ok(Some(plan))
}

/// Copy the already verified native sidecar without discovering another map.
pub(super) fn inherit(source: &Path, output: &Path, report: &Value) -> Result<(), AppError> {
    let descriptor = &report["numerator_symmetry_certificate"];
    if descriptor.is_null() {
        return Ok(());
    }
    let file = descriptor["file"]
        .as_str()
        .ok_or_else(|| AppError::input("numerator-symmetry certificate path missing"))?;
    let bytes = storage::read_bounded(
        &storage::safe_child(source, file)?,
        profile::native_io_limits(None).max_program_bytes,
    )?;
    if descriptor["bytes"].as_u64() != Some(bytes.len() as u64)
        || descriptor["blake3"].as_str() != Some(blake3::hash(&bytes).to_hex().as_str())
    {
        return Err(AppError::input(
            "inherited numerator-symmetry certificate size/digest mismatch",
        ));
    }
    write_file_atomically(&storage::safe_child(output, file)?, &bytes, true).map_err(io)
}
