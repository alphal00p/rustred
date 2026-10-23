//! One finite fixed-d equation cohort with replayed local dimensional sources.
//!
//! The native source plan owns geometry and ordinary-IBP certificates. This
//! transport layer neither invents shifted-dimension master keys nor repeats
//! local nominations during later selective symmetry feedback rounds.
//!
//! Explicit refinement preparation forwards its cancellation flag through
//! cold source replay, using the existing refinement worker budget. Generic
//! inspection has no such context and still uses serial uncontrolled replay.
//! Even controlled replay cannot preempt an indivisible
//! Symbolica operation; cancellation is checked at the core's safe boundaries.
use super::*;
use rustred::family::IntegralKey;
use rustred::reduction::terminal_relations::{
    LocalDimensionRecurrenceControl, LocalDimensionRecurrenceOptions, LocalDimensionRecurrencePlan,
};
use std::cell::Cell;
use std::collections::BTreeSet;
use std::sync::Arc;

const PREFIX: &str = "local-dimensional-recurrence-assistance-v1:";
pub(super) const RECIPE: &str = "verified-local-dimensional-recurrence-v1";

fn keys(session: &TerminalRelationSession) -> BTreeSet<IntegralKey> {
    session
        .raw_terminals()
        .union(session.normalization().canonical_terminals())
        .cloned()
        .collect()
}

fn bound_inventory(
    session: &TerminalRelationSession,
    report: &Value,
    input: &BTreeSet<IntegralKey>,
) -> Result<String, AppError> {
    let mut hash = blake3::Hasher::new();
    hash.update(RECIPE.as_bytes());
    hash.update(session.family_owner().fingerprint().as_bytes());
    hash.update(&u64::try_from(input.len()).map_err(io)?.to_le_bytes());
    for key in input {
        for power in key.powers() {
            hash.update(&power.to_le_bytes());
        }
    }
    profile::from_report(report)?.hash(&mut hash);
    Ok(format!("{PREFIX}{}", hash.finalize().to_hex()))
}

pub(super) fn binding(
    session: &TerminalRelationSession,
    report: &Value,
) -> Result<String, AppError> {
    bound_inventory(session, report, &keys(session))
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
        || report["local_dimensional_recurrence_preparation"]["complete"] == true
}

pub(super) fn validate_options(
    options: &MasterReductionOptions,
    resumed: Option<&Value>,
    strategy: MasterRefinementStrategy,
) -> Result<(), AppError> {
    if options.local_dimensional_recurrence_assistance
        && (options.operation != MasterReductionOperation::Refine
            || strategy != MasterRefinementStrategy::Exact)
    {
        return Err(AppError::input(
            "local dimensional-recurrence assistance requires explicit exact refinement",
        ));
    }
    if resumed.is_some_and(|report| {
        report["local_dimensional_recurrence_assistance"]
            .as_bool()
            .unwrap_or(false)
            != options.local_dimensional_recurrence_assistance
    }) {
        return Err(AppError::input(
            "cannot change local dimensional-recurrence assistance on resume; begin a fresh refinement phase",
        ));
    }
    Ok(())
}

fn bytes(directory: &Path, descriptor: &Value) -> Result<Vec<u8>, AppError> {
    if descriptor["recipe"] != RECIPE {
        return Err(AppError::input(
            "unknown local dimensional-recurrence certificate recipe",
        ));
    }
    let file = descriptor["file"]
        .as_str()
        .ok_or_else(|| AppError::input("local dimensional-recurrence certificate path missing"))?;
    let bytes = storage::read_bounded(
        &storage::safe_child(directory, file)?,
        profile::native_io_limits(None).max_program_bytes,
    )?;
    if descriptor["bytes"].as_u64() != Some(bytes.len() as u64)
        || descriptor["blake3"].as_str() != Some(blake3::hash(&bytes).to_hex().as_str())
    {
        return Err(AppError::input(
            "local dimensional-recurrence certificate size/digest mismatch",
        ));
    }
    Ok(bytes)
}

fn load(
    directory: &Path,
    session: &TerminalRelationSession,
    report: &Value,
) -> Result<Option<LocalDimensionRecurrencePlan>, AppError> {
    // Generic inspection has no cancellation context. Explicit refinement
    // preparation uses the controlled entry below instead.
    load_with_control(
        directory,
        session,
        report,
        1,
        &LocalDimensionRecurrenceControl::default(),
    )
}

fn load_with_control(
    directory: &Path,
    session: &TerminalRelationSession,
    report: &Value,
    workers: usize,
    control: &LocalDimensionRecurrenceControl<'_>,
) -> Result<Option<LocalDimensionRecurrencePlan>, AppError> {
    let descriptor = &report["local_dimensional_recurrence_certificate"];
    let Some(expected) = component(session) else {
        return if descriptor.is_null() {
            Ok(None)
        } else {
            Err(AppError::input(
                "local dimensional-recurrence certificate has no native authority binding",
            ))
        };
    };
    if descriptor.is_null() {
        return if receipt_required(session, report) {
            Err(AppError::input(
                "accepted local dimensional-recurrence cohort has no exact source certificate",
            ))
        } else {
            Ok(None)
        };
    }
    let plan = LocalDimensionRecurrencePlan::from_native_bytes_with_control(
        &bytes(directory, descriptor)?,
        Arc::clone(session.family_owner()),
        LocalDimensionRecurrenceOptions {
            workers,
            ..Default::default()
        },
        profile::native_io_limits(None),
        control,
    )
    .map_err(io)?;
    let actual = bound_inventory(session, report, plan.input_keys())?;
    if actual != expected
        || descriptor["inventory_binding"] != expected
        || descriptor["complete"] != true
        || descriptor["input_keys"].as_u64() != Some(plan.input_keys().len() as u64)
        || !plan.input_keys().is_subset(&keys(session))
    {
        return Err(AppError::input(
            "local dimensional-recurrence source inventory/binding differs",
        ));
    }
    Ok(Some(plan))
}

fn record_plan(report: &mut Value, plan: &LocalDimensionRecurrencePlan, workers: usize) {
    let stats = plan.statistics();
    report["local_dimensional_recurrence_preparation"] = json!({
        "complete":true,"input_keys":stats.input_keys,"processed_keys":stats.input_keys,
        "nominated_directions":stats.nominated_directions,
        "supported_geometries":stats.supported_geometries,
        "degenerate_grams":stats.degenerate_grams,"skipped_geometries":stats.skipped_geometries,
        "skipped_reasons":stats.skipped_reasons,
        "weighted_source_rows":stats.weighted_source_rows,"equations":stats.equations,
        "output_terms":stats.output_terms,"inventory_keys":stats.inventory_keys,
        "reserved_workers":workers,
        "source_assurance":"each fixed-d row replayed as an exact regenerated weighted ordinary-IBP source sum",
        "cohort_scope":"original raw and normalized source keys only; later selective feedback targets are not automatically nominated",
        "shifted_dimension_runtime_keys":false,"new_master_keys":false,"minimality_claim":false,
    });
}

pub(super) fn prepare(
    options: &MasterReductionOptions,
    session: &TerminalRelationSession,
    report: &mut Value,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
    started: Instant,
) -> Result<LocalDimensionRecurrencePlan, AppError> {
    let cancelled = || cancel.load(Ordering::Relaxed);
    if let Some(plan) = load_with_control(
        &options.directory,
        session,
        report,
        options.threads,
        &LocalDimensionRecurrenceControl {
            cancelled: Some(&cancelled),
            progress: None,
        },
    )? {
        record_plan(report, &plan, options.threads);
        return Ok(plan);
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(AppError::execution(
            "paused before local dimensional-recurrence preparation",
        ));
    }
    let input = keys(session);
    report["stage"] = json!("preparing_local_dimensional_recurrences");
    report["local_dimensional_recurrence_preparation"] = json!({"complete":false,"input_keys":input.len(),"processed_keys":0,"reserved_workers":options.threads,"cohort_scope":"original source keys only"});
    observer(event(report, "master_reduction_progress", started));
    let snapshot = report.clone();
    let last_event = Cell::new(Instant::now());
    let cancelled = || cancel.load(Ordering::Relaxed);
    let progress = |p: rustred::reduction::terminal_relations::LocalDimensionRecurrenceProgress| {
        if last_event.get().elapsed() >= Duration::from_millis(250)
            || p.processed_keys == p.input_keys
        {
            let mut output = event(&snapshot, "master_reduction_progress", started);
            output["local_dimensional_recurrence_preparation"] = json!({
                "complete":false,"input_keys":p.input_keys,"processed_keys":p.processed_keys,
                "nominated_directions":p.nominated_directions,"supported_geometries":p.supported_geometries,
                "regenerated_source_rows":p.regenerated_source_rows,"equations":p.equations,
                "completed_witnesses":p.completed_witnesses,"total_witnesses":p.total_witnesses,
                "reserved_workers":options.threads,
            });
            observer(output);
            last_event.set(Instant::now());
        }
    };
    let plan = LocalDimensionRecurrencePlan::prepare(
        Arc::clone(session.family_owner()),
        &input,
        LocalDimensionRecurrenceOptions {
            workers: options.threads,
            ..Default::default()
        },
        &LocalDimensionRecurrenceControl {
            cancelled: Some(&cancelled),
            progress: Some(&progress),
        },
    )
    .map_err(io)?;
    if cancel.load(Ordering::Relaxed) {
        return Err(AppError::execution(
            "paused after local dimensional-recurrence preparation",
        ));
    }
    let encoded = plan
        .encode_native(profile::native_io_limits(None))
        .map_err(io)?;
    let digest = blake3::hash(&encoded).to_hex().to_string();
    let file = format!("local-dimensional-recurrence-{digest}.rrbin");
    write_file_atomically(&options.directory.join(&file), &encoded, true).map_err(io)?;
    report["local_dimensional_recurrence_certificate"] = json!({
        "recipe":RECIPE,"file":file,"bytes":encoded.len(),"blake3":digest,
        "inventory_binding":binding(session,report)?,"input_keys":input.len(),"complete":true,
    });
    record_plan(report, &plan, options.threads);
    // Preparation precedes the ordinary executor. Commit the complete proof
    // capsule before any of its immutable finite provider rows can be queried.
    save(options, session, report, "running", started, observer)?;
    Ok(plan)
}

pub(super) fn validate(
    directory: &Path,
    session: &TerminalRelationSession,
    report: &Value,
) -> Result<(), AppError> {
    let _ = load(directory, session, report)?;
    Ok(())
}

pub(super) fn inherit(source: &Path, output: &Path, report: &Value) -> Result<(), AppError> {
    let descriptor = &report["local_dimensional_recurrence_certificate"];
    if !descriptor.is_null() {
        let encoded = bytes(source, descriptor)?;
        let file = descriptor["file"].as_str().unwrap();
        write_file_atomically(&storage::safe_child(output, file)?, &encoded, true).map_err(io)?;
    }
    Ok(())
}
