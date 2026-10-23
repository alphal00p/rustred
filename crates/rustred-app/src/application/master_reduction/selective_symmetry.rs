//! Selected full-integral symmetry/IBP feedback with durable round boundaries.
//!
//! A selected-worklist fixed point is not a proof of a minimal master basis.
//! New source integrals remain auxiliary; only the original raw roster may be
//! returned by the terminal application service.
use super::*;
use rustred::family::IntegralKey;
use rustred::reduction::terminal_normalization::{
    TerminalNormalizationPlan, TerminalSectorSymmetryControl, TerminalSectorSymmetryEquations,
    TerminalSectorSymmetryLimits,
};
use rustred::reduction::terminal_relations::{TerminalEquation, TerminalRelationRow};
use rustred::sector::Mask;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub(super) const RECIPE: &str = "selective-symmetry-ibp-feedback-v1";

pub(super) fn enabled(report: &Value) -> bool {
    report["selective_symmetry_feedback_enabled"] == true
}

pub(super) fn complete(report: &Value) -> bool {
    !enabled(report)
        || (report["selective_symmetry_feedback"]["selected_queue_exhausted"] == true
            && report["selective_symmetry_feedback"]["source_jobs_complete"] == true
            && report["selective_symmetry_feedback"]["ordinary_work_complete"] == true
            && report["selective_symmetry_feedback"]["rounds"]
                .as_array()
                .is_some_and(|rounds| rounds.iter().all(|round| round["stage"] == "complete")))
}

pub(super) fn validate_options(
    options: &MasterReductionOptions,
    resumed: Option<&Value>,
    strategy: MasterRefinementStrategy,
) -> Result<(), AppError> {
    if options.selective_symmetry_feedback
        && (options.operation != MasterReductionOperation::Refine
            || !options.sector_symmetry_assistance
            || strategy != MasterRefinementStrategy::Exact
            || options.saved_rule_assistance
            || options.circuit_symmetry_assistance)
    {
        return Err(AppError::input(
            "selective symmetry feedback requires explicit exact sector-symmetry refinement without dynamic saved-rule or separate circuit providers",
        ));
    }
    if resumed.is_some_and(|report| enabled(report) != options.selective_symmetry_feedback) {
        return Err(AppError::input(
            "cannot change selective symmetry feedback on resume; begin a fresh refinement phase",
        ));
    }
    Ok(())
}

pub(super) fn record(options: &MasterReductionOptions, report: &mut Value) {
    if options.selective_symmetry_feedback {
        report["selective_symmetry_feedback_enabled"] = json!(true);
        report["selective_symmetry_feedback"] = json!({
            "recipe":RECIPE,"stage":"waiting_for_initial_search",
            "selected_queue_exhausted":false,"source_jobs_complete":false,
            "ordinary_work_complete":false,"completed_rounds":0,
            "selected_targets":0,"frontier_keys":0,"rounds":[],
            "algebraic_saturation_claim":false,"minimality_claim":false,
        });
    } else {
        report
            .as_object_mut()
            .unwrap()
            .remove("selective_symmetry_feedback_enabled");
        report
            .as_object_mut()
            .unwrap()
            .remove("selective_symmetry_feedback");
    }
}

fn write_payload(directory: &Path, stem: &str, bytes: &[u8]) -> Result<Value, AppError> {
    let digest = blake3::hash(bytes).to_hex().to_string();
    let file = format!("selective-symmetry-{stem}-{digest}.rrbin");
    write_file_atomically(&directory.join(&file), bytes, true).map_err(io)?;
    Ok(json!({"file":file,"bytes":bytes.len(),"blake3":digest}))
}

fn read_payload(directory: &Path, descriptor: &Value) -> Result<Vec<u8>, AppError> {
    let file = descriptor["file"]
        .as_str()
        .ok_or_else(|| AppError::input("selective symmetry capsule path missing"))?;
    let bytes = storage::read_bounded(
        &storage::safe_child(directory, file)?,
        profile::native_io_limits(None).max_program_bytes,
    )?;
    if descriptor["bytes"].as_u64() != Some(bytes.len() as u64)
        || descriptor["blake3"].as_str() != Some(blake3::hash(&bytes).to_hex().as_str())
    {
        return Err(AppError::input(
            "selective symmetry capsule digest/size mismatch",
        ));
    }
    Ok(bytes)
}

fn write_rows(
    directory: &Path,
    stem: &str,
    session: &TerminalRelationSession,
    rows: &[TerminalEquation],
    binding: &str,
) -> Result<Value, AppError> {
    write_payload(
        directory,
        stem,
        &terminal_import::rows::encode(session.family_owner(), rows, binding)?,
    )
}

fn read_rows(
    directory: &Path,
    session: &TerminalRelationSession,
    descriptor: &Value,
    binding: &str,
) -> Result<Vec<TerminalEquation>, AppError> {
    terminal_import::rows::decode(
        &read_payload(directory, descriptor)?,
        session.family_owner(),
        binding,
    )
}

fn key_rows(
    session: &TerminalRelationSession,
    keys: &BTreeSet<IntegralKey>,
) -> Vec<TerminalEquation> {
    let one = session.family_owner().coefficient_context().one();
    keys.iter()
        .map(|key| TerminalEquation {
            terms: BTreeMap::from([(key.clone(), one.clone())]),
            nonzero_conditions: Vec::new(),
        })
        .collect()
}

fn row_keys(
    session: &TerminalRelationSession,
    rows: &[TerminalEquation],
) -> Result<BTreeSet<IntegralKey>, AppError> {
    let one = session.family_owner().coefficient_context().one();
    let mut keys = BTreeSet::new();
    for row in rows {
        if !row.nonzero_conditions.is_empty() || row.terms.len() != 1 {
            return Err(AppError::input(
                "selective symmetry key capsule is not a unit key list",
            ));
        }
        let (key, value) = row.terms.first_key_value().unwrap();
        if value != &one || !keys.insert(key.clone()) {
            return Err(AppError::input(
                "selective symmetry key capsule contains duplicate/nonunit key",
            ));
        }
    }
    Ok(keys)
}

fn parent_map_rows(session: &TerminalRelationSession) -> Result<Vec<TerminalEquation>, AppError> {
    let context = session.family_owner().coefficient_context();
    let mut rows = Vec::new();
    for (target, rhs) in session.apply_all_terminals().map_err(io)? {
        let mut terms = BTreeMap::from([(target, context.one())]);
        for (key, value) in rhs {
            let value = context
                .try_neg(
                    &value,
                    rustred::algebra::ExactAlgebraLimits::unbounded_resources(),
                )
                .map_err(io)?;
            let next = if let Some(previous) = terms.remove(&key) {
                context
                    .try_add(
                        &previous,
                        &value,
                        rustred::algebra::ExactAlgebraLimits::unbounded_resources(),
                    )
                    .map_err(io)?
            } else {
                value
            };
            if !next.is_zero() {
                terms.insert(key, next);
            }
        }
        rows.push(TerminalEquation {
            terms,
            nonzero_conditions: Vec::new(),
        });
    }
    // One shared guard owner, never a clone for each original integral.
    rows.push(TerminalEquation {
        terms: TerminalRelationRow::new(),
        nonzero_conditions: session.nonzero_conditions().to_vec(),
    });
    Ok(rows)
}

pub(super) fn inherit(source: &Path, output: &Path, report: &Value) -> Result<(), AppError> {
    let Some(rounds) = report["selective_symmetry_feedback"]["rounds"].as_array() else {
        return Ok(());
    };
    for round in rounds {
        for field in [
            "parent_native_state",
            "parent_maps",
            "selection",
            "sources",
            "normalization",
            "tasks",
        ] {
            if !round[field].is_null() {
                let bytes = read_payload(source, &round[field])?;
                let file = round[field]["file"].as_str().unwrap();
                write_file_atomically(&storage::safe_child(output, file)?, &bytes, true)
                    .map_err(io)?;
            }
        }
    }
    Ok(())
}

fn rounds(report: &Value) -> Result<&Vec<Value>, AppError> {
    report["selective_symmetry_feedback"]["rounds"]
        .as_array()
        .ok_or_else(|| AppError::input("selective symmetry round inventory missing"))
}

fn round_binding(
    parent: &Value,
    maps: &Value,
    selected: &BTreeSet<IntegralKey>,
    index: usize,
) -> Result<String, AppError> {
    let mut hash = blake3::Hasher::new();
    hash.update(RECIPE.as_bytes());
    for descriptor in [parent, maps] {
        hash.update(
            descriptor["blake3"]
                .as_str()
                .ok_or_else(|| AppError::input("selective symmetry parent identity missing"))?
                .as_bytes(),
        );
    }
    hash.update(&u64::try_from(index).map_err(io)?.to_le_bytes());
    for key in selected {
        for power in key.powers() {
            hash.update(&power.to_le_bytes());
        }
    }
    Ok(hash.finalize().to_hex().to_string())
}

fn selected_keys(
    directory: &Path,
    session: &TerminalRelationSession,
    round: &Value,
) -> Result<BTreeSet<IntegralKey>, AppError> {
    let binding = round["binding"]
        .as_str()
        .ok_or_else(|| AppError::input("selective symmetry round binding missing"))?;
    row_keys(
        session,
        &read_rows(directory, session, &round["selection"], binding)?,
    )
}

fn attempted_supports(
    directory: &Path,
    session: &TerminalRelationSession,
    report: &Value,
    rank: usize,
    limit: usize,
) -> Result<BTreeSet<Mask>, AppError> {
    let mut visited = BTreeSet::new();
    for round in rounds(report)?.iter().take(limit) {
        if round["stage"] == "complete"
            && round["terminal_rank_epoch"].as_u64() == Some(rank as u64)
        {
            for key in selected_keys(directory, session, round)? {
                visited.insert(Mask::try_from_indices(key.powers()).map_err(io)?);
            }
        }
    }
    Ok(visited)
}

fn normalization_rows(
    session: &TerminalRelationSession,
    plan: &TerminalNormalizationPlan,
) -> Result<Vec<TerminalEquation>, AppError> {
    let context = session.family_owner().coefficient_context();
    let algebra = rustred::algebra::ExactAlgebraLimits::unbounded_resources();
    let mut rows = Vec::new();
    for (target, rhs) in plan.terms() {
        let mut terms = BTreeMap::from([(target.clone(), context.one())]);
        for (key, value) in rhs {
            let value = context.try_neg(value, algebra).map_err(io)?;
            let next = if let Some(old) = terms.remove(key) {
                context.try_add(&old, &value, algebra).map_err(io)?
            } else {
                value
            };
            if !next.is_zero() {
                terms.insert(key.clone(), next);
            }
        }
        if !terms.is_empty() {
            rows.push(TerminalEquation {
                terms,
                nonzero_conditions: Vec::new(),
            });
        }
    }
    Ok(rows)
}

fn source_tasks(
    session: &TerminalRelationSession,
    plan: &TerminalSectorSymmetryEquations,
    normalization: &TerminalNormalizationPlan,
    cancel: &AtomicBool,
) -> Result<Vec<TerminalEquation>, AppError> {
    let mut tasks = Vec::new();
    for key in plan.input_keys() {
        for row in plan.equations(key) {
            if !tasks.contains(row)
                && (!session
                    .authorized_equation_residual_support(row, cancel)
                    .map_err(io)?
                    .is_empty()
                    || !row.nonzero_conditions.is_empty())
            {
                tasks.push(row.clone());
            }
        }
    }
    for row in normalization_rows(session, normalization)? {
        if !tasks.contains(&row)
            && (!session
                .authorized_equation_residual_support(&row, cancel)
                .map_err(io)?
                .is_empty()
                || !row.nonzero_conditions.is_empty())
        {
            tasks.push(row);
        }
    }
    Ok(tasks)
}

fn prepare_round(
    options: &MasterReductionOptions,
    session: &TerminalRelationSession,
    report: &mut Value,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
    started: Instant,
) -> Result<bool, AppError> {
    let index = rounds(report)?.len();
    let rank = session.remaining_terminals().len();
    if let Some(previous) = rounds(report)?.last() {
        if previous["terminal_rank_epoch"]
            .as_u64()
            .is_some_and(|before| rank as u64 > before)
        {
            return Err(AppError::input(
                "selective symmetry protected-terminal rank unexpectedly increased",
            ));
        }
    }
    let visited = attempted_supports(&options.directory, session, report, rank, index)?;
    report["stage"] = json!("selecting_symmetry_feedback_sources");
    report["selective_symmetry_feedback"]["stage"] = json!("selecting_sources");
    report["selective_symmetry_feedback"]["round"] = json!(index);
    report["selective_symmetry_feedback"]["terminal_rank_epoch"] = json!(rank);
    report["selective_symmetry_feedback"]["visited_supports"] = json!(visited.len());
    observer(event(report, "master_reduction_progress", started));
    let frontier = session
        .terminal_relevant_auxiliary_keys_with_control(cancel)
        .map_err(io)?;
    let proposals = session
        .selective_auxiliary_source_seeds(&visited, cancel)
        .map_err(io)?;
    let selected: BTreeSet<_> = proposals.into_values().collect();
    report["selective_symmetry_feedback"]["frontier_keys"] = json!(frontier.len());
    report["selective_symmetry_feedback"]["pending_targets"] = json!(selected.len());
    if selected.is_empty() {
        let receipt = &mut report["selective_symmetry_feedback"];
        receipt["stage"] = json!("selected_support_fixed_point");
        receipt["selected_queue_exhausted"] = json!(true);
        receipt["source_jobs_complete"] = json!(true);
        receipt["ordinary_work_complete"] = json!(true);
        receipt["unsearched_auxiliary_keys"] = json!(frontier.len());
        receipt["heuristic_scope"] = json!(
            "one cheapest actual terminal-linked auxiliary integral per positive support per protected-terminal-rank epoch; supports reopen only after a strict terminal-rank decrease; no exhaustive all-power or minimality claim"
        );
        return Ok(false);
    }
    // The caller has just committed this exact native U boundary. These bytes
    // and maps, not merely a terminal count, authorize the round selection.
    let parent = report["native_state"].clone();
    if parent["blake3"].is_null() {
        return Err(AppError::input(
            "selective symmetry has no durable parent native state",
        ));
    }
    let maps_binding = format!(
        "{RECIPE}:parent-maps:{}",
        parent["blake3"].as_str().unwrap()
    );
    let maps = write_rows(
        &options.directory,
        "parent-maps",
        session,
        &parent_map_rows(session)?,
        &maps_binding,
    )?;
    let binding = round_binding(&parent, &maps, &selected, index)?;
    let selection = write_rows(
        &options.directory,
        "selection",
        session,
        &key_rows(session, &selected),
        &binding,
    )?;
    let policy = options.sector_symmetry_policy.unwrap_or_default();
    report["selective_symmetry_feedback"]["stage"] = json!("preparing_selected_sources");
    observer(event(report, "master_reduction_progress", started));
    let cancelled = || cancel.load(Ordering::Relaxed);
    let snapshot = report.clone();
    let progress =
        |p: rustred::reduction::terminal_normalization::TerminalSectorSymmetryProgress| {
            let mut output = event(&snapshot, "master_reduction_progress", started);
            output["selective_symmetry_feedback"]["completed_targets"] = json!(p.processed_keys);
            output["selective_symmetry_feedback"]["selected_targets"] = json!(selected.len());
            observer(output);
        };
    let plan = TerminalSectorSymmetryEquations::prepare_with_policy_and_control(
        Arc::clone(session.family_owner()),
        &selected,
        TerminalSectorSymmetryLimits::default(),
        policy,
        &TerminalSectorSymmetryControl {
            cancelled: Some(&cancelled),
            progress: Some(&progress),
        },
    )
    .map_err(io)?;
    let sources = write_payload(
        &options.directory,
        "sources",
        &plan
            .encode_native(profile::native_io_limits(None))
            .map_err(io)?,
    )?;
    if cancel.load(Ordering::Relaxed) {
        return Err(AppError::execution(
            "paused after selected symmetry source preparation",
        ));
    }
    let normalized = TerminalNormalizationPlan::vacuum_quadratic_numerators(
        session.family_owner(),
        &selected,
        session.normalization().ordering(),
        MasterNormalizationProfile::RamControlledV1.normalization_limits(),
    )
    .map_err(io)?;
    let normalization = write_payload(
        &options.directory,
        "normalization",
        &normalized
            .encode_native(profile::native_io_limits(None))
            .map_err(io)?,
    )?;
    let tasks = source_tasks(session, &plan, &normalized, cancel)?;
    let task_descriptor = write_rows(&options.directory, "tasks", session, &tasks, &binding)?;
    let round = json!({"round":index,"binding":binding,"stage":"prepared",
        "parent_native_state":parent,"parent_maps":maps,"parent_maps_binding":maps_binding,
        "selection":selection,"sources":sources,"normalization":normalization,"tasks":task_descriptor,
        "selected_targets":selected.len(),"source_rows":tasks.len(),"terminal_rank_epoch":rank,
        "policy":sector_symmetry::policy_name(policy),"raw_terminals":session.raw_terminals().len(),
        "parent_completed_assistance_rows":session.statistics().completed_assistance_rows,
        "parent_completed_source_rows":session.statistics().completed_source_rows,
        "saved_projection_requested":false,
        "scope":"original witnessed full-integral symmetry/normalization equations and selected ordinary-IBP sources; unknown offspring stay auxiliary"
    });
    report["selective_symmetry_feedback"]["rounds"]
        .as_array_mut()
        .unwrap()
        .push(round);
    report["selective_symmetry_feedback"]["source_jobs_complete"] = json!(false);
    report["selective_symmetry_feedback"]["ordinary_work_complete"] = json!(false);
    report["selective_symmetry_feedback"]["stage"] = json!("prepared");
    save(options, session, report, "running", started, observer)?;
    Ok(true)
}

fn enqueue_round(
    options: &MasterReductionOptions,
    session: &mut TerminalRelationSession,
    report: &mut Value,
    started: Instant,
    observer: &impl Fn(Value),
) -> Result<(), AppError> {
    let index = rounds(report)?.len() - 1;
    let round = rounds(report)?[index].clone();
    let binding = round["binding"].as_str().unwrap();
    let tasks = read_rows(&options.directory, session, &round["tasks"], binding)?;
    let selected = selected_keys(&options.directory, session, &round)?;
    // Both mutations are published together. A crash before save retains the
    // prepared parent snapshot and safely repeats this whole enqueue.
    session.enqueue_authorized_equations(&tasks).map_err(io)?;
    let seeds = session.extend_source_seeds(&selected).map_err(io)?;
    let receipt = &mut report["selective_symmetry_feedback"];
    receipt["rounds"][index]["stage"] = json!("executing");
    receipt["rounds"][index]["added_ordinary_seeds"] = json!(seeds);
    receipt["stage"] = json!("joining_selected_symmetry_and_ibp_rows");
    receipt["source_jobs_complete"] = json!(true);
    receipt["ordinary_work_complete"] = json!(false);
    save(options, session, report, "running", started, observer)
}

pub(super) fn execute(
    options: &MasterReductionOptions,
    session: &mut TerminalRelationSession,
    report: &mut Value,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
    started: Instant,
    executor: &TerminalRelationExecutor,
    mut provider: Option<&mut super::assistance::PreparedAssistance>,
) -> Result<(), AppError> {
    if !options.selective_symmetry_feedback {
        return Ok(());
    }
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        if complete(report) {
            return Ok(());
        }
        let stage = rounds(report)?
            .last()
            .and_then(|round| round["stage"].as_str());
        if !matches!(stage, Some("prepared" | "executing")) {
            save(options, session, report, "running", started, observer)?;
            if !prepare_round(options, session, report, cancel, observer, started)? {
                return Ok(());
            }
        }
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        if rounds(report)?.last().unwrap()["stage"] == "prepared" {
            enqueue_round(options, session, report, started, observer)?;
        }
        let mut last_checkpoint = Instant::now();
        while !session.is_complete() && !cancel.load(Ordering::Relaxed) {
            if let Some(provider) = provider.as_deref_mut() {
                if let Some(finite) = &provider.finite {
                    executor
                        .step_with_finite_provider_observed(session, cancel, finite, |progress| {
                            observed_execution(report, progress, observer, started)
                        })
                        .map_err(io)?;
                } else {
                    executor
                        .step_with_provider_observed(
                            session,
                            cancel,
                            &mut provider.provider,
                            |progress| observed_execution(report, progress, observer, started),
                        )
                        .map_err(io)?;
                }
            } else {
                executor
                    .step_observed(session, cancel, |progress| {
                        observed_execution(report, progress, observer, started)
                    })
                    .map_err(io)?;
            }
            if last_checkpoint.elapsed() >= options.checkpoint_interval {
                save(options, session, report, "running", started, observer)?;
                last_checkpoint = Instant::now();
            }
        }
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        let index = rounds(report)?.len() - 1;
        let selected = rounds(report)?[index]["selected_targets"].as_u64().unwrap();
        let receipt = &mut report["selective_symmetry_feedback"];
        receipt["rounds"][index]["stage"] = json!("complete");
        receipt["rounds"][index]["remaining_terminals"] =
            json!(session.remaining_terminals().len());
        receipt["rounds"][index]["completed_assistance_rows"] =
            json!(session.statistics().completed_assistance_rows);
        receipt["rounds"][index]["completed_source_rows"] =
            json!(session.statistics().completed_source_rows);
        receipt["completed_rounds"] = json!(index + 1);
        receipt["selected_targets"] =
            json!(receipt["selected_targets"].as_u64().unwrap_or(0) + selected);
        receipt["ordinary_work_complete"] = json!(true);
        receipt["stage"] = json!("round_committed");
        save(options, session, report, "running", started, observer)?;
    }
}

/// Independently replay each selected momentum/normalization source and bind
/// the generated task vector to its exact parent U, raw roster and guard map.
/// The imported native U remains trusted exact saved row-space authority.
pub(super) fn validate(
    directory: &Path,
    session: &TerminalRelationSession,
    report: &Value,
) -> Result<(), AppError> {
    if !enabled(report) {
        if !report["selective_symmetry_feedback"].is_null() {
            return Err(AppError::input(
                "selective symmetry receipt has no enabled mode",
            ));
        }
        return Ok(());
    }
    if report["selective_symmetry_feedback"]["recipe"] != RECIPE {
        return Err(AppError::input(
            "unknown selective symmetry feedback recipe",
        ));
    }
    if (report["selective_symmetry_feedback"]["selected_queue_exhausted"] == true
        && !complete(report))
        || ((report["status"] == "completed_nonminimal" || report["refinement_complete"] == true)
            && !complete(report))
    {
        return Err(AppError::input(
            "selective symmetry completion metadata has unfinished selected work",
        ));
    }
    let inventory = rounds(report)?;
    let conditions: std::collections::HashSet<_> = session.nonzero_conditions().iter().collect();
    let mut previous_rank = None;
    for (index, round) in inventory.iter().enumerate() {
        if round["round"].as_u64() != Some(index as u64)
            || !matches!(
                round["stage"].as_str(),
                Some("prepared" | "executing" | "complete")
            )
        {
            return Err(AppError::input(
                "selective symmetry round sequence/stage differs",
            ));
        }
        let parent = TerminalRelationSession::from_native_bytes(
            &read_payload(directory, &round["parent_native_state"])?,
            profile::from_report(report)?.relation_limits(),
            profile::native_io_limits(None),
        )
        .map_err(io)?;
        if !parent.is_complete()
            || parent.family_owner().fingerprint() != session.family_owner().fingerprint()
            || parent.raw_terminals() != session.raw_terminals()
            || round["raw_terminals"].as_u64() != Some(session.raw_terminals().len() as u64)
            || round["terminal_rank_epoch"].as_u64()
                != Some(parent.remaining_terminals().len() as u64)
        {
            return Err(AppError::input(
                "selective symmetry parent scope/rank differs",
            ));
        }
        let rank = parent.remaining_terminals().len();
        if previous_rank.is_some_and(|before| rank > before)
            || (index > 0 && inventory[index - 1]["stage"] != "complete")
            || (index > 0
                && inventory[index - 1]["remaining_terminals"].as_u64() != Some(rank as u64))
        {
            return Err(AppError::input(
                "selective symmetry committed parent/rank sequence differs",
            ));
        }
        previous_rank = Some(rank);
        if parent
            .nonzero_conditions()
            .iter()
            .any(|condition| !conditions.contains(condition))
        {
            return Err(AppError::input(
                "selective symmetry current state dropped a parent guard",
            ));
        }
        let selected = selected_keys(directory, session, round)?;
        let visited = attempted_supports(directory, session, report, rank, index)?;
        let expected: BTreeSet<_> = parent
            .selective_auxiliary_source_seeds(&visited, &AtomicBool::new(false))
            .map_err(io)?
            .into_values()
            .collect();
        if selected != expected || selected.is_empty() {
            return Err(AppError::input(
                "selective symmetry selection differs from its frozen native parent",
            ));
        }
        let binding = round_binding(
            &round["parent_native_state"],
            &round["parent_maps"],
            &selected,
            index,
        )?;
        if round["binding"] != binding
            || round["selected_targets"].as_u64() != Some(selected.len() as u64)
        {
            return Err(AppError::input(
                "selective symmetry selected parent binding differs",
            ));
        }
        let maps_binding = round["parent_maps_binding"]
            .as_str()
            .ok_or_else(|| AppError::input("selective symmetry map binding missing"))?;
        let parent_maps = read_rows(directory, &parent, &round["parent_maps"], maps_binding)?;
        if parent_maps != parent_map_rows(&parent)? {
            return Err(AppError::input(
                "selective symmetry parent maps/guards differ",
            ));
        }
        let plan = TerminalSectorSymmetryEquations::from_native_bytes(
            &read_payload(directory, &round["sources"])?,
            Arc::clone(session.family_owner()),
            TerminalSectorSymmetryLimits::default(),
            profile::native_io_limits(None),
        )
        .map_err(io)?;
        if plan.input_keys() != &selected
            || round["policy"] != sector_symmetry::policy_name(plan.policy())
            || plan.policy() != sector_symmetry::from_report(report)?
        {
            return Err(AppError::input("selective symmetry source cohort differs"));
        }
        let normalization = TerminalNormalizationPlan::decode_generated(
            &read_payload(directory, &round["normalization"])?,
            session.family_owner(),
            &selected,
            session.normalization().ordering(),
            MasterNormalizationProfile::RamControlledV1.normalization_limits(),
            profile::native_io_limits(None),
        )
        .map_err(io)?;
        let tasks = read_rows(directory, session, &round["tasks"], &binding)?;
        if tasks != source_tasks(&parent, &plan, &normalization, &AtomicBool::new(false))?
            || round["source_rows"].as_u64() != Some(tasks.len() as u64)
        {
            return Err(AppError::input(
                "selective symmetry task/source replay differs",
            ));
        }
        let parent_stats = parent.statistics();
        if round["parent_completed_assistance_rows"].as_u64()
            != Some(parent_stats.completed_assistance_rows)
            || round["parent_completed_source_rows"].as_u64()
                != Some(parent_stats.completed_source_rows)
        {
            return Err(AppError::input(
                "selective symmetry parent source cursors differ",
            ));
        }
        let next = if let Some(next) = inventory.get(index + 1) {
            Some(
                TerminalRelationSession::from_native_bytes(
                    &read_payload(directory, &next["parent_native_state"])?,
                    profile::from_report(report)?.relation_limits(),
                    profile::native_io_limits(None),
                )
                .map_err(io)?,
            )
        } else {
            None
        };
        let child = next.as_ref().unwrap_or(session);
        let child_stats = child.statistics();
        if round["stage"] == "prepared" {
            if index + 1 != inventory.len()
                || child
                    .to_native_bytes(profile::native_io_limits(None))
                    .map_err(io)?
                    != read_payload(directory, &round["parent_native_state"])?
            {
                return Err(AppError::input(
                    "selective symmetry prepared round changed its native parent",
                ));
            }
        } else {
            let delta = child_stats
                .completed_assistance_rows
                .checked_sub(parent_stats.completed_assistance_rows)
                .ok_or_else(|| AppError::input("selective symmetry assistance prefix regressed"))?;
            if delta > tasks.len() as u64
                || child_stats.completed_source_rows < parent_stats.completed_source_rows
            {
                return Err(AppError::input(
                    "selective symmetry native task/source prefix differs",
                ));
            }
            if round["stage"] == "complete"
                && (delta != tasks.len() as u64
                    || !child.is_complete()
                    || round["remaining_terminals"].as_u64()
                        != Some(child.remaining_terminals().len() as u64)
                    || round["completed_assistance_rows"].as_u64()
                        != Some(child_stats.completed_assistance_rows)
                    || round["completed_source_rows"].as_u64()
                        != Some(child_stats.completed_source_rows))
            {
                return Err(AppError::input(
                    "selective symmetry completed round has an unfinished/mismatched native prefix",
                ));
            }
            let mut expected_seeds = parent.source_seeds().to_vec();
            expected_seeds.extend(
                selected
                    .iter()
                    .filter(|key| !parent.source_seeds().contains(key))
                    .cloned(),
            );
            let added = expected_seeds.len() - parent.source_seeds().len();
            if child.source_seeds() != expected_seeds
                || round["added_ordinary_seeds"].as_u64() != Some(added as u64)
                || child_stats.total_source_rows
                    != parent_stats
                        .total_source_rows
                        .checked_add(
                            (added as u64)
                                .checked_mul(parent_stats.source_rows_per_seed as u64)
                                .ok_or_else(|| {
                                    AppError::input("selective symmetry source work count overflow")
                                })?,
                        )
                        .ok_or_else(|| {
                            AppError::input("selective symmetry source work count overflow")
                        })?
                || child_stats.completed_source_rows > child_stats.total_source_rows
            {
                return Err(AppError::input(
                    "selective symmetry selected ordinary source schedule differs",
                ));
            }
            if let Some((queued, cursor)) = child.externally_queued_equations() {
                if queued != tasks || cursor as u64 != delta {
                    return Err(AppError::input(
                        "selective symmetry queued task payload/cursor differs",
                    ));
                }
            } else if delta != tasks.len() as u64 {
                return Err(AppError::input(
                    "selective symmetry pending task suffix has no native queue",
                ));
            }
            // A generated native U remains the existing trusted row-space
            // authority, not an independently replayed W*A certificate. Still
            // detect semantic prefix corruption: all inherited terminal maps
            // and every actually committed witnessed task must hold in it.
            let mut accepted = parent_maps;
            accepted.extend_from_slice(&tasks[..delta as usize]);
            let child_conditions: std::collections::HashSet<_> =
                child.nonzero_conditions().iter().collect();
            if accepted
                .iter()
                .flat_map(|row| &row.nonzero_conditions)
                .any(|condition| !condition.is_constant() && !child_conditions.contains(condition))
            {
                return Err(AppError::input(
                    "selective symmetry accepted task/map guard was dropped",
                ));
            }
            if !child
                .authorized_equations_are_satisfied(&accepted, &AtomicBool::new(false))
                .map_err(io)?
            {
                return Err(AppError::input(
                    "selective symmetry accepted task/parent map is absent from native row space",
                ));
            }
        }
    }
    if complete(report) {
        if !session.is_complete() {
            return Err(AppError::input(
                "selective symmetry completion has unfinished native work",
            ));
        }
        let visited = attempted_supports(
            directory,
            session,
            report,
            session.remaining_terminals().len(),
            inventory.len(),
        )?;
        if !session
            .selective_auxiliary_source_seeds(&visited, &AtomicBool::new(false))
            .map_err(io)?
            .is_empty()
        {
            return Err(AppError::input(
                "selective symmetry completion has unprocessed selected supports",
            ));
        }
    }
    Ok(())
}
