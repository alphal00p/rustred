//! One finite symmetry lane, including legacy numerator maps, with separately
//! authenticated exact witnesses and saved-program projection provenance.
use super::*;
use crate::application::routed_campaign::PreparedTerminalEquations;
use rustred::algebra::ExactAlgebraLimits;
use rustred::family::IntegralKey;
use rustred::reduction::terminal_normalization::{
    TerminalNormalizationPlan, TerminalSectorSymmetryControl, TerminalSectorSymmetryEquations,
    TerminalSectorSymmetryLimits, TerminalSectorSymmetryProgress,
};
use rustred::reduction::terminal_relations::{TerminalEquation, TerminalRelationRow};
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;

const PREFIX: &str = "sector-symmetry-assistance-v1:";
const RECIPE: &str = "verified-sector-symmetry-sources-v3";
const NORMALIZATION: &str = "sector-symmetry-normalization-v1";
const PROJECTION: &str = "sector-symmetry-saved-projection-v1";
const SOURCE_ROWS: &str = "sector-symmetry-source-rows-v1";
const TASKS: &str = "sector-symmetry-composed-source-rows-v1";

fn keys(session: &TerminalRelationSession) -> BTreeSet<IntegralKey> {
    session
        .raw_terminals()
        .union(session.normalization().canonical_terminals())
        .cloned()
        .collect()
}

pub(super) fn binding(
    options: &MasterReductionOptions,
    session: &TerminalRelationSession,
    report: &Value,
) -> Result<String, AppError> {
    bound_inventory(
        session,
        report,
        &keys(session),
        for_phase(options, report)?,
        projection_for_phase(options, report)?,
    )
}

pub(super) fn policy_name(policy: MasterSectorSymmetryPolicy) -> &'static str {
    match policy {
        MasterSectorSymmetryPolicy::GeneratorRelations => "generators",
        MasterSectorSymmetryPolicy::PolynomialOrbits => "polynomial-orbits",
    }
}

fn policy_stage(policy: MasterSectorSymmetryPolicy) -> &'static str {
    match policy {
        MasterSectorSymmetryPolicy::GeneratorRelations => "generator_relations",
        MasterSectorSymmetryPolicy::PolynomialOrbits => "whole_polynomial_orbits",
    }
}

pub(super) fn from_report(report: &Value) -> Result<MasterSectorSymmetryPolicy, AppError> {
    match report["sector_symmetry_policy"].as_str() {
        Some("generators") => Ok(MasterSectorSymmetryPolicy::GeneratorRelations),
        Some("polynomial-orbits") => Ok(MasterSectorSymmetryPolicy::PolynomialOrbits),
        None if report["sector_symmetry_assistance"] != true
            && report["inherited_equation_authority"]["sector_symmetry_assistance"] != true =>
        {
            Ok(MasterSectorSymmetryPolicy::GeneratorRelations)
        }
        _ => Err(AppError::input(
            "missing or unknown sector-symmetry source policy",
        )),
    }
}

pub(super) fn for_phase(
    options: &MasterReductionOptions,
    report: &Value,
) -> Result<MasterSectorSymmetryPolicy, AppError> {
    if options.sector_symmetry_policy.is_some() && !options.sector_symmetry_assistance {
        return Err(AppError::input(
            "sector-symmetry policy requires sector-symmetry assistance",
        ));
    }
    let saved = if options.resume && options.sector_symmetry_assistance {
        Some(from_report(report)?)
    } else {
        None
    };
    let selected = options
        .sector_symmetry_policy
        .or(saved)
        .unwrap_or(MasterSectorSymmetryPolicy::GeneratorRelations);
    if saved.is_some_and(|saved| saved != selected) {
        return Err(AppError::input(
            "cannot change sector-symmetry source policy on resume; start a new phase directory",
        ));
    }
    Ok(selected)
}

pub(super) fn projection_from_report(report: &Value) -> Result<bool, AppError> {
    match report.get("sector_symmetry_saved_projection") {
        Some(Value::Bool(selected)) => Ok(*selected),
        None => Ok(true), // Historical strong v3 always projected saved rules.
        _ => Err(AppError::input(
            "invalid sector-symmetry saved-projection mode",
        )),
    }
}

pub(super) fn projection_for_phase(
    options: &MasterReductionOptions,
    report: &Value,
) -> Result<bool, AppError> {
    if options.sector_symmetry_saved_projection.is_some() && !options.sector_symmetry_assistance {
        return Err(AppError::input(
            "sector-symmetry saved projection requires sector-symmetry assistance",
        ));
    }
    let saved = if options.resume && options.sector_symmetry_assistance {
        Some(projection_from_report(report)?)
    } else {
        None
    };
    let selected = options
        .sector_symmetry_saved_projection
        .or(saved)
        .unwrap_or(true);
    if saved.is_some_and(|saved| saved != selected) {
        return Err(AppError::input(
            "cannot change sector-symmetry saved projection on resume; start a new phase directory",
        ));
    }
    Ok(selected)
}

fn final_recipe(report: &Value) -> Result<&'static str, AppError> {
    Ok(if projection_from_report(report)? {
        PROJECTION
    } else {
        SOURCE_ROWS
    })
}

fn program_binding(report: &Value) -> Result<&str, AppError> {
    report["inputs"]["program_binding"].as_str().ok_or_else(|| {
        AppError::input(
            "sector-symmetry assistance requires an authenticated saved program selection",
        )
    })
}

fn component(session: &TerminalRelationSession) -> Option<&str> {
    session
        .assistance_binding()?
        .split(';')
        .find(|part| part.starts_with(PREFIX))
}

pub(super) fn receipt_required(session: &TerminalRelationSession, report: &Value) -> bool {
    let stats = session.statistics();
    stats.completed_assistance_rows != 0
        || stats.completed_assistance_keys != 0
        || stats.pending_assistance_rows != 0
        || (report["sector_symmetry_reduction"]["complete"] == true
            && matches!(
                report["status"].as_str(),
                Some("published_unrefined" | "completed_nonminimal")
            ))
}

fn write_payload(
    directory: &Path,
    stem: &str,
    bytes: &[u8],
    recipe: &str,
    authority: &str,
) -> Result<Value, AppError> {
    let digest = blake3::hash(bytes).to_hex().to_string();
    let file = format!("{stem}-{digest}.rrbin");
    write_file_atomically(&directory.join(&file), bytes, true).map_err(io)?;
    Ok(
        json!({"recipe":recipe,"file":file,"bytes":bytes.len(),"blake3":digest,"authority_binding":authority}),
    )
}

fn read_payload(directory: &Path, descriptor: &Value, recipe: &str) -> Result<Vec<u8>, AppError> {
    if descriptor["recipe"] != recipe {
        return Err(AppError::input(
            "unknown sector-symmetry certificate recipe",
        ));
    }
    let file = descriptor["file"]
        .as_str()
        .ok_or_else(|| AppError::input("sector-symmetry certificate path missing"))?;
    let bytes = storage::read_bounded(
        &storage::safe_child(directory, file)?,
        profile::native_io_limits(None).max_program_bytes,
    )?;
    if descriptor["bytes"].as_u64() != Some(bytes.len() as u64)
        || descriptor["blake3"].as_str() != Some(blake3::hash(&bytes).to_hex().as_str())
    {
        return Err(AppError::input(
            "sector-symmetry certificate size/digest mismatch",
        ));
    }
    Ok(bytes)
}

fn load(
    directory: &Path,
    session: &TerminalRelationSession,
    report: &Value,
) -> Result<Option<TerminalSectorSymmetryEquations>, AppError> {
    let Some(expected) = component(session) else {
        return Ok(None);
    };
    let descriptor = &report["sector_symmetry_certificate"];
    if descriptor.is_null() {
        if receipt_required(session, report) {
            return Err(AppError::input(
                "accepted sector-symmetry rows have no exact source certificate",
            ));
        }
        return Ok(None);
    }
    let bytes = read_payload(directory, descriptor, RECIPE)?;
    let plan = TerminalSectorSymmetryEquations::from_native_bytes(
        &bytes,
        Arc::clone(session.family_owner()),
        TerminalSectorSymmetryLimits::default(),
        profile::native_io_limits(None),
    )
    .map_err(io)?;
    if plan.legacy_supplement().is_none() {
        return Err(AppError::input(
            "combined sector-symmetry certificate is missing its numerator-symmetry source cohort",
        ));
    }
    // Frozen inherited inventories may be smaller than the new publication.
    // Rebuild the binding from the certificate's original source keys.
    let policy = from_report(report)?;
    let requested = projection_from_report(report)?;
    let actual = bound_inventory(session, report, plan.input_keys(), policy, requested)?;
    if actual != expected
        || descriptor["inventory_binding"].as_str() != Some(expected)
        || plan.policy() != policy
        || descriptor["policy"] != policy_name(policy)
        || descriptor
            .get("saved_projection_requested")
            .map_or(!requested, |value| value.as_bool() != Some(requested))
    {
        return Err(AppError::input(
            "sector-symmetry certificate/provider binding mismatch",
        ));
    }
    Ok(Some(plan))
}

fn bound_inventory(
    session: &TerminalRelationSession,
    report: &Value,
    input: &BTreeSet<IntegralKey>,
    policy: MasterSectorSymmetryPolicy,
    saved_projection: bool,
) -> Result<String, AppError> {
    let mut hash = blake3::Hasher::new();
    hash.update(RECIPE.as_bytes());
    hash.update(policy_name(policy).as_bytes());
    if !saved_projection {
        hash.update(SOURCE_ROWS.as_bytes());
    }
    hash.update(session.family_owner().fingerprint().as_bytes());
    hash.update(&u64::try_from(input.len()).map_err(io)?.to_le_bytes());
    for key in input {
        for power in key.powers() {
            hash.update(&power.to_le_bytes());
        }
    }
    profile::from_report(report)?.hash(&mut hash);
    hash.update(program_binding(report)?.as_bytes());
    MasterNormalizationProfile::RamControlledV1.hash(&mut hash);
    Ok(format!("{PREFIX}{}", hash.finalize().to_hex()))
}

pub(super) fn prepare(
    options: &MasterReductionOptions,
    session: &TerminalRelationSession,
    report: &mut Value,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
    started: Instant,
) -> Result<TerminalSectorSymmetryEquations, AppError> {
    let policy = for_phase(options, report)?;
    let requested = projection_for_phase(options, report)?;
    let stage = policy_stage(policy);
    report["sector_symmetry_policy"] = json!(policy_name(policy));
    report["sector_symmetry_saved_projection"] = json!(requested);
    report["stage"] = json!("preparing_sector_symmetry_equations");
    let input_count = keys(session).len();
    report["sector_symmetry_preparation"] = json!({"complete":false,"source_jobs_complete":false,"input_keys":input_count,"processed_keys":0,"policy":policy_name(policy),"stage":stage});
    observer(event(report, "master_reduction_progress", started));
    let plan = if let Some(plan) = load(&options.directory, session, report)? {
        if plan.input_keys() != &keys(session) {
            return Err(AppError::input(
                "sector-symmetry certificate inventory differs from the active finite provider",
            ));
        }
        plan
    } else {
        let latest = Cell::new(TerminalSectorSymmetryProgress::default());
        let last_event = Cell::new(Instant::now());
        let snapshot = report.clone();
        let progress = |progress: TerminalSectorSymmetryProgress| {
            latest.set(progress);
            if last_event.get().elapsed() >= Duration::from_millis(250) {
                let mut output = event(&snapshot, "master_reduction_progress", started);
                output["sector_symmetry_preparation"] = json!({"complete":false,"source_jobs_complete":false,"input_keys":input_count,"policy":policy_name(policy),"stage":stage,"processed_keys":progress.processed_keys,"closure_keys":progress.discovered_keys,"analyzed_supports":progress.analyzed_supports,"verified_generators":progress.verified_generators,"orbit_images":progress.orbit_images,"orbit_terms":progress.orbit_terms});
                observer(output);
                last_event.set(Instant::now());
            }
        };
        let cancelled = || cancel.load(Ordering::Relaxed);
        let prepared = TerminalSectorSymmetryEquations::prepare_with_policy_and_control(
            Arc::clone(session.family_owner()),
            &keys(session),
            TerminalSectorSymmetryLimits::default(),
            policy,
            &TerminalSectorSymmetryControl {
                cancelled: Some(&cancelled),
                progress: Some(&progress),
            },
        );
        let p = latest.get();
        report["sector_symmetry_preparation"] = json!({"complete":false,"source_jobs_complete":false,"input_keys":input_count,"policy":policy_name(policy),"stage":stage,"processed_keys":p.processed_keys,"closure_keys":p.discovered_keys,"analyzed_supports":p.analyzed_supports,"verified_generators":p.verified_generators,"orbit_images":p.orbit_images,"orbit_terms":p.orbit_terms});
        let plan = prepared.map_err(io)?;
        if plan.legacy_supplement().is_none() {
            return Err(AppError::input(
                "combined sector-symmetry preparation omitted its numerator-symmetry source cohort",
            ));
        }
        let bytes = plan
            .encode_native(profile::native_io_limits(None))
            .map_err(io)?;
        let mut descriptor = write_payload(
            &options.directory,
            "sector-symmetry",
            &bytes,
            RECIPE,
            &binding(options, session, report)?,
        )?;
        descriptor["inventory_binding"] = json!(binding(options, session, report)?);
        descriptor["policy"] = json!(policy_name(policy));
        descriptor["saved_projection_requested"] = json!(requested);
        descriptor["authority"] = json!(
            "independently replayed exact full-integral momentum maps; no ordinary IBP generation"
        );
        report["sector_symmetry_certificate"] = descriptor;
        plan
    };
    let s = plan.statistics();
    report["sector_symmetry_preparation"] = json!({"complete":true,"source_jobs_complete":true,"policy":policy_name(policy),"stage":stage,"input_keys":s.input_keys,"processed_keys":s.input_keys,"closure_keys":s.closure_keys,"analyzed_supports":s.analyzed_supports,"verified_generators":s.verified_generators,"equation_targets":s.equation_targets,"equations":s.equations,"output_terms":s.output_terms,"trivial_equations":s.trivial_equations,"skipped":s.skipped.iter().map(|(reason,count)| json!({"reason":format!("{reason:?}"),"count":count})).collect::<Vec<_>>(),"scope":"verified source equations for the original finite keys under the selected source policy; endpoint monomials remain auxiliary columns","polynomial_orbit_closure_claim":policy == MasterSectorSymmetryPolicy::PolynomialOrbits});
    report["sector_symmetry_preparation"]["legacy_source_equations"] = json!(
        plan.legacy_supplement()
            .map_or(0, |legacy| legacy.statistics().equations)
    );
    Ok(plan)
}

fn normalization(
    options: &MasterReductionOptions,
    session: &TerminalRelationSession,
    plan: &TerminalSectorSymmetryEquations,
    report: &mut Value,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
    started: Instant,
) -> Result<TerminalNormalizationPlan, AppError> {
    let limits = MasterNormalizationProfile::RamControlledV1.normalization_limits();
    let descriptor = &report["sector_symmetry_normalization_certificate"];
    let authority = report["sector_symmetry_certificate"]["blake3"]
        .as_str()
        .ok_or_else(|| AppError::input("sector source certificate missing"))?
        .to_owned();
    if !descriptor.is_null() {
        if descriptor["authority_binding"] != authority {
            return Err(AppError::input(
                "sector normalization/source binding mismatch",
            ));
        }
        let normalized = TerminalNormalizationPlan::decode_generated(
            &read_payload(&options.directory, descriptor, NORMALIZATION)?,
            session.family_owner(),
            plan.input_keys(),
            session.normalization().ordering(),
            limits,
            profile::native_io_limits(None),
        )
        .map_err(io)?;
        record_normalization(session, report, &normalized);
        return Ok(normalized);
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(AppError::execution(
            "paused before RAM-controlled sector normalization",
        ));
    }
    report["stage"] = json!("normalizing_sector_symmetry_inventory");
    observer(event(report, "master_reduction_progress", started));
    let normalized = TerminalNormalizationPlan::vacuum_quadratic_numerators(
        session.family_owner(),
        plan.input_keys(),
        session.normalization().ordering(),
        limits,
    )
    .map_err(io)?;
    if normalized
        .terms()
        .values()
        .flat_map(BTreeMap::keys)
        .any(|key| !plan.input_keys().contains(key))
    {
        return Err(AppError::input(
            "sector normalization produced a key outside its original finite source inventory",
        ));
    }
    let bytes = normalized
        .encode_native(profile::native_io_limits(None))
        .map_err(io)?;
    let mut descriptor = write_payload(
        &options.directory,
        "sector-symmetry-normalization",
        &bytes,
        NORMALIZATION,
        &authority,
    )?;
    descriptor["normalization_profile"] = json!(MasterNormalizationProfile::RamControlledV1.name());
    report["sector_symmetry_normalization_certificate"] = descriptor;
    record_normalization(session, report, &normalized);
    Ok(normalized)
}

fn record_normalization(
    session: &TerminalRelationSession,
    report: &mut Value,
    plan: &TerminalNormalizationPlan,
) {
    let statistics = plan.statistics();
    let one = session.family_owner().coefficient_context().one();
    let source_equations = plan
        .terms()
        .iter()
        .filter(|(key, rhs)| rhs.len() != 1 || rhs.get(*key) != Some(&one))
        .count();
    let receipt = &mut report["sector_symmetry_preparation"];
    receipt["normalization_input_keys"] = json!(statistics.raw_terminals);
    receipt["normalization_canonical_keys"] = json!(statistics.canonical_terminals);
    receipt["normalization_unit_aliases"] = json!(statistics.unit_aliases);
    receipt["normalization_projected_numerators"] = json!(statistics.projected_numerators);
    receipt["normalization_source_equations"] = json!(source_equations);
    receipt["normalization_output_terms"] =
        json!(plan.terms().values().map(BTreeMap::len).sum::<usize>());
    receipt["normalization_safe_zero_certificates"] = json!(plan.zero_certificates().len());
}

fn accumulate(
    session: &TerminalRelationSession,
    row: &mut TerminalRelationRow,
    key: IntegralKey,
    value: rustred::algebra::Coefficient,
) -> Result<(), AppError> {
    if value.is_zero() {
        return Ok(());
    }
    let value = match row.remove(&key) {
        Some(previous) => session
            .family_owner()
            .coefficient_context()
            .try_add(&previous, &value, ExactAlgebraLimits::unbounded_resources())
            .map_err(io)?,
        None => value,
    };
    if !value.is_zero() {
        row.insert(key, value);
    }
    Ok(())
}

/// Compose previously proved maps before asking the saved engine to do work.
/// This only substitutes keys explicitly present in the authenticated map.
fn compose(
    session: &TerminalRelationSession,
    equation: &TerminalEquation,
    maps: &BTreeMap<IntegralKey, TerminalRelationRow>,
) -> Result<TerminalEquation, AppError> {
    let mut result = TerminalEquation {
        terms: BTreeMap::new(),
        nonzero_conditions: equation.nonzero_conditions.clone(),
    };
    let context = session.family_owner().coefficient_context();
    for (key, value) in &equation.terms {
        if let Some(rhs) = maps.get(key) {
            for (output, coefficient) in rhs {
                accumulate(
                    session,
                    &mut result.terms,
                    output.clone(),
                    context
                        .try_mul(
                            value,
                            coefficient,
                            ExactAlgebraLimits::unbounded_resources(),
                        )
                        .map_err(io)?,
                )?;
            }
        } else {
            accumulate(session, &mut result.terms, key.clone(), value.clone())?;
        }
    }
    Ok(result)
}

fn projection_base_binding(
    session: &TerminalRelationSession,
    report: &Value,
) -> Result<String, AppError> {
    let mut hash = blake3::Hasher::new();
    hash.update(final_recipe(report)?.as_bytes());
    for field in [
        "sector_symmetry_certificate",
        "sector_symmetry_normalization_certificate",
    ] {
        hash.update(
            report[field]["blake3"]
                .as_str()
                .ok_or_else(|| AppError::input("sector exact preparation certificate missing"))?
                .as_bytes(),
        );
    }
    hash.update(program_binding(report)?.as_bytes());
    // The retained exact-map authority determines the preliminary composition.
    if let Some(binding) = session.retained_relations_binding() {
        hash.update(binding.as_bytes());
    }
    Ok(hash.finalize().to_hex().to_string())
}

fn projection_binding(
    session: &TerminalRelationSession,
    report: &Value,
) -> Result<String, AppError> {
    let mut hash = blake3::Hasher::new();
    hash.update(projection_base_binding(session, report)?.as_bytes());
    hash.update(
        report["sector_symmetry_task_certificate"]["blake3"]
            .as_str()
            .ok_or_else(|| AppError::input("sector projection has no frozen source task vector"))?
            .as_bytes(),
    );
    Ok(hash.finalize().to_hex().to_string())
}

fn publish_projection(
    options: &MasterReductionOptions,
    session: &TerminalRelationSession,
    report: &mut Value,
    rows: &[TerminalEquation],
    total: usize,
    authority: &str,
    complete: bool,
    started: Instant,
) -> Result<(), AppError> {
    let bytes = terminal_import::rows::encode(session.family_owner(), rows, authority)?;
    let mut descriptor = write_payload(
        &options.directory,
        "sector-symmetry-projection",
        &bytes,
        final_recipe(report)?,
        authority,
    )?;
    descriptor["processed_rows"] = json!(rows.len());
    descriptor["source_rows"] = json!(total);
    descriptor["complete"] = json!(complete);
    descriptor["saved_projection_requested"] = json!(projection_from_report(report)?);
    descriptor["authority"] = json!(if projection_from_report(report)? {
        "trusted exact application of authenticated supplied candidate programs; no regenerated IBP-source proof"
    } else {
        "complete witnessed symmetry and normalization source rows composed with retained exact maps; no new candidate-program application"
    });
    let field = if complete {
        "sector_symmetry_reduced_certificate"
    } else {
        "sector_symmetry_projection_checkpoint"
    };
    report[field] = descriptor;
    // Preparation can spend a long time before the ordinary executor gets its
    // next save opportunity. Keep its completed exact rows recoverable now.
    save(options, session, report, "running", started, &|_| {})?;
    Ok(())
}

pub(super) fn prepare_rows(
    options: &MasterReductionOptions,
    session: &TerminalRelationSession,
    plan: &TerminalSectorSymmetryEquations,
    mut saved: Option<&mut PreparedTerminalEquations>,
    report: &mut Value,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
    started: Instant,
) -> Result<BTreeMap<IntegralKey, Vec<TerminalEquation>>, AppError> {
    let normalized = normalization(options, session, plan, report, cancel, observer, started)?;
    let base = projection_base_binding(session, report)?;
    let tasks = if report["sector_symmetry_task_certificate"].is_object() {
        let descriptor = &report["sector_symmetry_task_certificate"];
        if descriptor["authority_binding"] != base {
            return Err(AppError::input("sector source task authority differs"));
        }
        terminal_import::rows::decode(
            &read_payload(&options.directory, descriptor, TASKS)?,
            session.family_owner(),
            &base,
        )?
    } else {
        let maps = session.apply_all_terminals().map_err(io)?;
        let mut tasks: Vec<(IntegralKey, TerminalEquation)> = Vec::new();
        for key in plan.input_keys() {
            for row in plan.equations(key) {
                tasks.push((key.clone(), compose(session, row, &maps)?));
            }
            if let Some(rhs) = normalized.terms().get(key) {
                let context = session.family_owner().coefficient_context();
                let mut row = TerminalEquation {
                    terms: BTreeMap::from([(key.clone(), context.one())]),
                    nonzero_conditions: Vec::new(),
                };
                for (output, value) in rhs {
                    accumulate(
                        session,
                        &mut row.terms,
                        output.clone(),
                        context
                            .try_neg(value, ExactAlgebraLimits::unbounded_resources())
                            .map_err(io)?,
                    )?;
                }
                if !row.terms.is_empty() {
                    tasks.push((key.clone(), compose(session, &row, &maps)?));
                }
            }
        }
        // Deterministic cheap equations first; this is scheduling, never a cutoff.
        tasks.sort_by_key(|(source, row)| {
            let rank = row
                .terms
                .keys()
                .map(|key| {
                    key.powers()
                        .iter()
                        .filter(|power| **power < 0)
                        .map(|power| power.unsigned_abs() as u128)
                        .sum::<u128>()
                })
                .max()
                .unwrap_or(0);
            (rank, row.terms.len(), source.clone())
        });
        let mut source_rows: Vec<_> = tasks.into_iter().map(|(_, row)| row).collect();
        if !session.nonzero_conditions().is_empty() {
            // Retained maps share one generic domain. Never clone its often
            // large guard union into every transformed source equation.
            source_rows.push(TerminalEquation {
                terms: BTreeMap::new(),
                nonzero_conditions: session.nonzero_conditions().to_vec(),
            });
        }
        report["stage"] = json!("eliminating_sector_symmetry_source_rows");
        report["sector_symmetry_reduction"] = json!({"complete":false,"witnessed_source_rows":source_rows.len(),"stage":"exact_symmetry_span"});
        observer(event(report, "master_reduction_progress", started));
        if cancel.load(Ordering::Relaxed) {
            return Err(AppError::execution(
                "paused before exact sector-symmetry span elimination",
            ));
        }
        let snapshot = report.clone();
        let basis_event = Cell::new(Instant::now());
        let basis = TerminalRelationSession::independent_equation_basis_with_control(
            Arc::clone(session.family_owner()),
            &source_rows,
            MasterNormalizationProfile::RamControlledV1.relation_limits(),
            cancel,
            |processed, independent| {
                if basis_event.get().elapsed() >= Duration::from_millis(250)
                    || processed == source_rows.len()
                {
                    let mut output = event(&snapshot, "master_reduction_progress", started);
                    output["sector_symmetry_reduction"]["span_processed_rows"] = json!(processed);
                    output["sector_symmetry_reduction"]["independent_source_rows"] =
                        json!(independent);
                    observer(output);
                    basis_event.set(Instant::now());
                }
            },
        )
        .map_err(io)?;
        let mut tasks: Vec<_> = basis
            .equations
            .into_iter()
            .map(|terms| TerminalEquation {
                terms,
                nonzero_conditions: Vec::new(),
            })
            .collect();
        if !basis.nonzero_conditions.is_empty() {
            // The provider authorizes one conjunction for its entire finite
            // row span. Carry it once, including when its row has zero terms.
            tasks.push(TerminalEquation {
                terms: BTreeMap::new(),
                nonzero_conditions: basis.nonzero_conditions,
            });
        }
        tasks.sort_by_key(|row| {
            let rank = row
                .terms
                .keys()
                .map(|key| {
                    key.powers()
                        .iter()
                        .filter(|power| **power < 0)
                        .map(|power| power.unsigned_abs() as u128)
                        .sum::<u128>()
                })
                .max()
                .unwrap_or(0);
            (rank, row.terms.len())
        });
        let bytes = terminal_import::rows::encode(session.family_owner(), &tasks, &base)?;
        let mut descriptor = write_payload(
            &options.directory,
            "sector-symmetry-tasks",
            &bytes,
            TASKS,
            &base,
        )?;
        descriptor["rows"] = json!(tasks.len());
        report["sector_symmetry_task_certificate"] = descriptor;
        tasks
    };
    let authority = projection_binding(session, report)?;
    let requested = projection_for_phase(options, report)?;
    // Once the immutable provider exists, never compose its source rows with
    // a later (improved) reducer and accidentally change a resumed provider.
    let descriptor = &report["sector_symmetry_reduced_certificate"];
    if !descriptor.is_null() {
        if descriptor["authority_binding"] != authority
            || descriptor["complete"] != true
            || descriptor
                .get("saved_projection_requested")
                .map_or(!requested, |value| value.as_bool() != Some(requested))
        {
            return Err(AppError::input(
                "sector saved projection authority/completion differs",
            ));
        }
        let rows = terminal_import::rows::decode(
            &read_payload(&options.directory, descriptor, final_recipe(report)?)?,
            session.family_owner(),
            &authority,
        )?;
        if descriptor["processed_rows"].as_u64() != Some(rows.len() as u64)
            || descriptor["source_rows"].as_u64() != Some(rows.len() as u64)
            || rows.len() != tasks.len()
            || (!requested && rows != tasks)
        {
            return Err(AppError::input("sector projection row inventory differs"));
        }
        return anchored_rows(plan, rows);
    }
    report["sector_symmetry_reduction"]["exact_source_rows_ready"] = json!(true);
    report["sector_symmetry_reduction"]["saved_projection_requested"] = json!(requested);
    if !requested {
        if cancel.load(Ordering::Relaxed) {
            return Err(AppError::execution(
                "paused before publishing complete witnessed sector source rows",
            ));
        }
        let protected = session.remaining_terminals();
        let unresolved: HashSet<_> = tasks
            .iter()
            .flat_map(|row| row.terms.keys())
            .filter(|key| !protected.contains(*key))
            .collect();
        report["stage"] = json!("sector_symmetry_source_rows_ready");
        report["sector_symmetry_reduction"] = json!({"complete":true,"exact_source_rows_ready":true,"saved_projection_requested":false,"saved_projection_complete":null,"all_offspring_reduced":null,"processed_rows":tasks.len(),"source_rows":tasks.len(),"saved_reduced_rows":0,"reused_rows":0,"saved_application_failures":0,"failure_kinds":{},"failure_receipts":[],"incomplete_boundaries":0,"source_terms":tasks.iter().map(|row| row.terms.len()).sum::<usize>(),"projected_terms":null});
        record_scope(report, plan, &HashSet::new(), unresolved.len(), 0, true);
        publish_projection(
            options,
            session,
            report,
            &tasks,
            tasks.len(),
            &authority,
            true,
            started,
        )?;
        observer(event(report, "master_reduction_progress", started));
        return anchored_rows(plan, tasks);
    }
    let maps = session.apply_all_terminals().map_err(io)?;
    let protected = session.remaining_terminals();
    let mut projected = Vec::new();
    let descriptor = if report["sector_symmetry_reduced_certificate"].is_object() {
        &report["sector_symmetry_reduced_certificate"]
    } else {
        &report["sector_symmetry_projection_checkpoint"]
    };
    if !descriptor.is_null() {
        if descriptor["authority_binding"] != authority
            || descriptor["source_rows"].as_u64() != Some(tasks.len() as u64)
        {
            return Err(AppError::input(
                "sector projection preparation binding differs",
            ));
        }
        projected = terminal_import::rows::decode(
            &read_payload(&options.directory, descriptor, PROJECTION)?,
            session.family_owner(),
            &authority,
        )?;
        if descriptor["processed_rows"].as_u64() != Some(projected.len() as u64)
            || projected.len() > tasks.len()
        {
            return Err(AppError::input(
                "sector projection completed-row cursor differs",
            ));
        }
    }
    let mut reduced = report["sector_symmetry_reduction"]["saved_reduced_rows"]
        .as_u64()
        .unwrap_or(0) as usize;
    let mut failures = report["sector_symmetry_reduction"]["saved_application_failures"]
        .as_u64()
        .unwrap_or(0) as usize;
    let reused = report["sector_symmetry_reduction"]["reused_rows"]
        .as_u64()
        .unwrap_or(0) as usize;
    let mut failure_kinds: BTreeMap<String, usize> =
        serde_json::from_value(report["sector_symmetry_reduction"]["failure_kinds"].clone())
            .unwrap_or_default();
    let mut failure_receipts = report["sector_symmetry_reduction"]["failure_receipts"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut seen_conditions: HashSet<_> = projected
        .iter()
        .flat_map(|row| row.nonzero_conditions.iter().cloned())
        .collect();
    let failed_tasks: HashSet<_> = failure_receipts
        .iter()
        .filter_map(|receipt| receipt["source_task"].as_u64())
        .collect();
    let mut unresolved_outputs: HashSet<_> = projected
        .iter()
        .flat_map(|row| row.terms.keys())
        .filter(|key| !protected.contains(*key))
        .cloned()
        .collect();
    let mut saved_new_leaves: HashSet<_> = projected
        .iter()
        .enumerate()
        .filter(|(index, _)| !failed_tasks.contains(&(*index as u64)))
        .flat_map(|(_, row)| row.terms.keys())
        .filter(|key| !plan.input_keys().contains(*key))
        .cloned()
        .collect();
    let mut projected_terms = projected.iter().map(|row| row.terms.len()).sum::<usize>();
    let source_rows = tasks.len();
    let mut last_checkpoint = Instant::now();
    report["stage"] = json!("reducing_sector_symmetry_equations");
    report["sector_symmetry_reduction"]["source_rows"] = json!(source_rows);
    report["sector_symmetry_reduction"]["processed_rows"] = json!(projected.len());
    report["sector_symmetry_reduction"]["complete"] = json!(false);
    for input in tasks.into_iter().skip(projected.len()) {
        if cancel.load(Ordering::Relaxed) {
            publish_projection(
                options,
                session,
                report,
                &projected,
                source_rows,
                &authority,
                false,
                started,
            )?;
            report["sector_symmetry_reduction"]["complete"] = json!(false);
            return Err(AppError::execution(
                "paused during exact sector-symmetry saved projection; completed rows checkpointed",
            ));
        }
        let (mut output, saved_success) = if input.terms.keys().all(|key| protected.contains(key)) {
            (input, false)
        } else {
            // The shared saved engine retains selective memoization, scalar
            // factorization, tensor rules, descent and family-routing checks.
            // Failure retains the complete witnessed row as auxiliary data.
            let snapshot = report.clone();
            let mut application_progress =
                |progress: &rustred::solver::CandidateRoutedApplicationSnapshot| {
                    let mut output = event(&snapshot, "master_reduction_progress", started);
                    output["sector_symmetry_reduction"]["active_row"] = json!({"task":projected.len(),"phase":format!("{:?}",progress.phase),"elapsed_seconds":progress.elapsed.as_secs_f64(),"unique_nodes":progress.unique_nodes,"active_nodes":progress.active_nodes,"pending_frames":progress.pending_frames,"cached_entries":progress.cached.entries});
                    observer(output);
                };
            let unresolved = TerminalEquation {
                terms: input
                    .terms
                    .iter()
                    .filter(|(key, _)| !protected.contains(*key))
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect(),
                nonzero_conditions: input.nonzero_conditions.clone(),
            };
            let saved = saved.as_mut().ok_or_else(|| {
                AppError::input(
                    "sector-symmetry saved projection has no authenticated saved-program reducer",
                )
            })?;
            match (saved.reduce_equation_controlled)(&unresolved, cancel, &mut application_progress)
            {
                Ok(mut output) => {
                    reduced += 1;
                    // Saved application is linear. Keep every already proved
                    // terminal summand and add the fully reduced offspring;
                    // no partial or unresolved RHS becomes a terminal map.
                    for (key, value) in input
                        .terms
                        .iter()
                        .filter(|(key, _)| protected.contains(*key))
                    {
                        accumulate(session, &mut output.terms, key.clone(), value.clone())?;
                    }
                    (compose(session, &output, &maps)?, true)
                }
                Err(error) if error.kind() == "cancelled" => {
                    report["sector_symmetry_reduction"]["complete"] = json!(false);
                    report["sector_symmetry_reduction"]["saved_projection_complete"] = json!(false);
                    publish_projection(
                        options,
                        session,
                        report,
                        &projected,
                        source_rows,
                        &authority,
                        false,
                        started,
                    )?;
                    return Err(AppError::execution(
                        "paused within exact sector-symmetry saved application; completed source tasks checkpointed",
                    ));
                }
                Err(error) => {
                    failures += 1;
                    *failure_kinds.entry(error.kind().to_owned()).or_default() += 1;
                    failure_receipts.push(json!({"source_task":projected.len(),"task_certificate":report["sector_symmetry_task_certificate"]["blake3"],"kind":error.kind(),"diagnostic":error.to_string(),"fallback":"complete exact source row retained; unresolved offspring remain auxiliary columns"}));
                    (input, false)
                }
            }
        };
        for key in output.terms.keys() {
            if !protected.contains(key) {
                unresolved_outputs.insert(key.clone());
            }
            if saved_success && !plan.input_keys().contains(key) {
                saved_new_leaves.insert(key.clone());
            }
        }
        // All rows are delivered together at one original query anchor. Keep
        // the cohort guard conjunction once rather than once per equation.
        output.nonzero_conditions.retain(|condition| {
            if seen_conditions.contains(condition) {
                false
            } else {
                seen_conditions.insert(condition.clone());
                true
            }
        });
        projected_terms += output.terms.len();
        projected.push(output);
        report["sector_symmetry_reduction"] = json!({"processed_rows":projected.len(),"source_rows":source_rows,"saved_reduced_rows":reduced,"reused_rows":reused,"saved_application_failures":failures,"failure_kinds":failure_kinds,"failure_receipts":failure_receipts,"incomplete_boundaries":failures,"projected_terms":projected_terms,"complete":false,"exact_source_rows_ready":true,"saved_projection_requested":true,"saved_projection_complete":false});
        record_scope(
            report,
            plan,
            &saved_new_leaves,
            unresolved_outputs.len(),
            failures,
            false,
        );
        observer(event(report, "master_reduction_progress", started));
        if last_checkpoint.elapsed() >= options.checkpoint_interval {
            publish_projection(
                options,
                session,
                report,
                &projected,
                source_rows,
                &authority,
                false,
                started,
            )?;
            observer(event(report, "master_reduction_checkpoint", started));
            last_checkpoint = Instant::now();
        }
    }
    report["sector_symmetry_reduction"]["complete"] = json!(true);
    report["sector_symmetry_reduction"]["all_offspring_reduced"] = json!(failures == 0);
    report["sector_symmetry_reduction"]["saved_projection_complete"] = json!(failures == 0);
    record_scope(
        report,
        plan,
        &saved_new_leaves,
        unresolved_outputs.len(),
        failures,
        true,
    );
    publish_projection(
        options,
        session,
        report,
        &projected,
        source_rows,
        &authority,
        true,
        started,
    )?;
    anchored_rows(plan, projected)
}

fn record_scope(
    report: &mut Value,
    plan: &TerminalSectorSymmetryEquations,
    saved_new_leaves: &HashSet<IntegralKey>,
    unresolved: usize,
    failures: usize,
    complete: bool,
) {
    let requested = report["sector_symmetry_saved_projection"] != false;
    let receipt = &mut report["sector_symmetry_reduction"];
    receipt["normalization_outputs_outside_source_inventory"] = json!(0);
    receipt["saved_output_keys_outside_symmetry_inventory"] = json!(
        saved_new_leaves
            .iter()
            .filter(|key| !plan.inventory().contains(*key))
            .count()
    );
    receipt["saved_terminal_keys_outside_source_inventory"] = json!(saved_new_leaves.len());
    receipt["deferred_symmetry_sources"] = json!(saved_new_leaves.len());
    receipt["unresolved_output_keys"] = json!(unresolved);
    receipt["combined_closure_complete"] =
        json!(complete && failures == 0 && unresolved == 0 && saved_new_leaves.is_empty());
    receipt["combined_closure_scope"] = json!(if !requested {
        "witnessed symmetry and normalization source cohort composed with retained exact maps; saved projection not requested, auxiliary columns unresolved; ordinary finite IBP search is a separate phase activity"
    } else {
        "original finite sources, their normalization and saved-program projection only; newly returned program leaves and unresolved tails are deferred boundaries, with no unrestricted family-closure claim"
    });
}

fn anchored_rows(
    plan: &TerminalSectorSymmetryEquations,
    projected: Vec<TerminalEquation>,
) -> Result<BTreeMap<IntegralKey, Vec<TerminalEquation>>, AppError> {
    let mut result: BTreeMap<IntegralKey, Vec<TerminalEquation>> = BTreeMap::new();
    // All closure rows belong to one original query anchor. Saved projection
    // can remove every auxiliary from its parent row; indexing by that absent
    // auxiliary would otherwise make a valid descendant equation unreachable.
    let Some(anchor) = plan.input_keys().first() else {
        return if projected.is_empty() {
            Ok(result)
        } else {
            Err(AppError::input(
                "sector equations have no original query anchor",
            ))
        };
    };
    for row in projected {
        if !row.terms.is_empty() || !row.nonzero_conditions.is_empty() {
            let rows = result.entry(anchor.clone()).or_default();
            if !rows.contains(&row) {
                rows.push(row);
            }
        }
    }
    Ok(result)
}

/// Cold load checks exact map and normalization witnesses independently. Saved
/// projections retain trusted supplied-program authority, exactly like the
/// existing candidate-rule import; no ordinary IBP proof is manufactured.
pub(super) fn validate(
    directory: &Path,
    session: &TerminalRelationSession,
    report: &Value,
) -> Result<(), AppError> {
    let Some(plan) = load(directory, session, report)? else {
        return Ok(());
    };
    let descriptor = &report["sector_symmetry_normalization_certificate"];
    if descriptor.is_null() {
        if receipt_required(session, report) {
            return Err(AppError::input(
                "accepted sector rows have no exact normalization certificate",
            ));
        }
        return Ok(());
    }
    if descriptor["authority_binding"] != report["sector_symmetry_certificate"]["blake3"]
        || descriptor["normalization_profile"] != MasterNormalizationProfile::RamControlledV1.name()
    {
        return Err(AppError::input(
            "sector normalization authority/profile differs",
        ));
    }
    TerminalNormalizationPlan::decode_generated(
        &read_payload(directory, descriptor, NORMALIZATION)?,
        session.family_owner(),
        plan.input_keys(),
        session.normalization().ordering(),
        MasterNormalizationProfile::RamControlledV1.normalization_limits(),
        profile::native_io_limits(None),
    )
    .map_err(io)?;
    let descriptor = &report["sector_symmetry_task_certificate"];
    if descriptor.is_null() {
        if receipt_required(session, report) {
            return Err(AppError::input(
                "accepted sector rows have no frozen source tasks",
            ));
        }
        return Ok(());
    }
    let base = projection_base_binding(session, report)?;
    if descriptor["authority_binding"] != base {
        return Err(AppError::input("sector frozen task authority differs"));
    }
    let tasks = terminal_import::rows::decode(
        &read_payload(directory, descriptor, TASKS)?,
        session.family_owner(),
        &base,
    )?;
    if descriptor["rows"].as_u64() != Some(tasks.len() as u64) {
        return Err(AppError::input("sector frozen task row count differs"));
    }
    let authority = projection_binding(session, report)?;
    let descriptor = &report["sector_symmetry_reduced_certificate"];
    if descriptor.is_null() {
        if receipt_required(session, report) {
            return Err(AppError::input(
                "accepted sector rows have no saved projection certificate",
            ));
        }
    } else {
        let requested = projection_from_report(report)?;
        if descriptor["authority_binding"] != authority
            || descriptor["complete"] != true
            || descriptor
                .get("saved_projection_requested")
                .map_or(!requested, |value| value.as_bool() != Some(requested))
        {
            return Err(AppError::input(
                "sector saved projection authority/completion differs",
            ));
        }
        let rows = terminal_import::rows::decode(
            &read_payload(directory, descriptor, final_recipe(report)?)?,
            session.family_owner(),
            &authority,
        )?;
        if descriptor["processed_rows"].as_u64() != Some(rows.len() as u64)
            || descriptor["source_rows"].as_u64() != Some(rows.len() as u64)
            || rows.len() != tasks.len()
            || (!requested && rows != tasks)
        {
            return Err(AppError::input("sector projection row inventory differs"));
        }
    }
    Ok(())
}

pub(super) fn inherit(source: &Path, output: &Path, report: &Value) -> Result<(), AppError> {
    for (field, recipe) in [
        ("sector_symmetry_certificate", RECIPE),
        ("sector_symmetry_normalization_certificate", NORMALIZATION),
        ("sector_symmetry_task_certificate", TASKS),
        ("sector_symmetry_reduced_certificate", final_recipe(report)?),
        ("sector_symmetry_projection_checkpoint", PROJECTION),
    ] {
        let descriptor = &report[field];
        if !descriptor.is_null() {
            let bytes = read_payload(source, descriptor, recipe)?;
            let file = descriptor["file"]
                .as_str()
                .expect("read_payload checked file");
            write_file_atomically(&storage::safe_child(output, file)?, &bytes, true).map_err(io)?;
        }
    }
    Ok(())
}
