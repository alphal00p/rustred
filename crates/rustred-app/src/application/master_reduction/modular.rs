//! Strategy transport and durable modular-search state. Exact maps remain in
//! the existing native session; heuristic progress has its own native cursor.
use super::*;
use rustred::reduction::terminal_relations::{
    TerminalRelationModularOptions, TerminalRelationModularSession, TerminalRelationModularStats,
};

pub(super) const RECIPE: &str = "terminal-modular-circuits-v1";

pub(super) fn for_phase(
    options: &MasterReductionOptions,
    resumed: Option<&Value>,
) -> Result<MasterRefinementStrategy, AppError> {
    let saved = resumed.map(from_report).transpose()?;
    let strategy = options.refinement_strategy.or(saved).unwrap_or_default();
    if saved.is_some_and(|saved| saved != strategy) {
        return Err(AppError::input(
            "cannot change refinement strategy on resume; start a new phase directory",
        ));
    }
    if options.operation != MasterReductionOperation::Refine {
        if options.refinement_strategy.is_some() || options.modular_policy.is_some() {
            return Err(AppError::input(
                "refinement strategy options require explicit refinement",
            ));
        }
        return Ok(MasterRefinementStrategy::Exact);
    }
    if strategy == MasterRefinementStrategy::Exact && options.modular_policy.is_some() {
        return Err(AppError::input(
            "modular policy requires modular-circuits refinement",
        ));
    }
    if strategy == MasterRefinementStrategy::ModularCircuits
        && (options.saved_rule_assistance
            || options.circuit_symmetry_assistance
            || options.numerator_symmetry_assistance
            || options.sector_symmetry_assistance
            || options.sector_symmetry_policy.is_some()
            || options.sector_symmetry_saved_projection.is_some())
    {
        return Err(AppError::input(
            "modular-circuits currently requires ordinary sources without saved-rule, circuit-symmetry, numerator-symmetry or sector-symmetry assistance",
        ));
    }
    if strategy == MasterRefinementStrategy::ModularCircuits
        && (options.relation_batch_rows.is_some() || options.relation_block_rows.is_some())
    {
        return Err(AppError::input(
            "relation batch/block rows configure exact refinement; for modular-circuits use --modular-policy batch_rows/max_candidates instead",
        ));
    }
    if strategy == MasterRefinementStrategy::ModularCircuits {
        let _ = policy(
            options
                .modular_policy
                .as_ref()
                .or_else(|| resumed.and_then(|r| r.get("modular_policy"))),
        )?;
    }
    Ok(strategy)
}

pub(super) fn from_report(report: &Value) -> Result<MasterRefinementStrategy, AppError> {
    match report["refinement_strategy"].as_str().unwrap_or("exact") {
        "exact" => Ok(MasterRefinementStrategy::Exact),
        "modular-circuits" => Ok(MasterRefinementStrategy::ModularCircuits),
        _ => Err(AppError::input("unknown saved master refinement strategy")),
    }
}

pub(super) fn hash(strategy: MasterRefinementStrategy, hash: &mut blake3::Hasher) {
    // Omitted/exact retains the legacy mathematical checkpoint binding.
    if strategy == MasterRefinementStrategy::ModularCircuits {
        hash.update(RECIPE.as_bytes());
    }
}

pub(super) fn record(
    report: &mut Value,
    strategy: MasterRefinementStrategy,
    options: &MasterReductionOptions,
) -> Result<(), AppError> {
    if strategy == MasterRefinementStrategy::ModularCircuits {
        report["refinement_strategy"] = json!("modular-circuits");
        report["refinement_schema"] = json!("rustred.master-refinement.modular-circuits.v1");
        report["completion_policy"] = json!(
            "finite heuristic modular source scan; accepted full-column source identities exactly verified; no exact row-span exhaustion claim"
        );
        report["modular_policy"] = policy_value(policy(options.modular_policy.as_ref())?);
    } else {
        for name in [
            "refinement_strategy",
            "refinement_schema",
            "modular_refinement",
            "modular_state",
            "modular_policy",
            "modular_search_complete",
        ] {
            report.as_object_mut().unwrap().remove(name);
        }
        report["completion_policy"] =
            json!("finite signed-L1 ordinary-IBP search; retain unresolved auxiliaries");
    }
    Ok(())
}

#[derive(Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    batch_rows: Option<usize>,
    discovery_samples: Option<usize>,
    max_candidates: Option<usize>,
    reconstruction: Option<ReconstructionPolicy>,
}
#[derive(Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ReconstructionPolicy {
    arithmetic: Option<ArithmeticPolicy>,
    max_rows: Option<usize>,
    max_columns: Option<usize>,
    max_input_nonzeros: Option<usize>,
    max_coefficient_terms: Option<usize>,
    max_degree: Option<u16>,
    max_probes: Option<usize>,
    max_attempts: Option<usize>,
    max_primes: Option<usize>,
    max_cached_images: Option<usize>,
    max_cached_values: Option<usize>,
    max_weight_slots: Option<usize>,
}
#[derive(Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ArithmeticPolicy {
    max_exponent: Option<u16>,
    max_polynomial_terms: Option<usize>,
    max_term_operations: Option<usize>,
}

fn policy(value: Option<&Value>) -> Result<TerminalRelationModularOptions, AppError> {
    let p: Policy = value
        .map(|value| serde_json::from_value(value.clone()))
        .transpose()
        .map_err(|error| AppError::input(format!("invalid modular refinement policy: {error}")))?
        .unwrap_or_default();
    let mut result = TerminalRelationModularOptions::default();
    macro_rules! assign { ($target:expr, $policy:expr, $($field:ident),+ $(,)?) => {
        $(if let Some(value) = $policy.$field { $target.$field = value; })+
    }; }
    assign!(result, p, batch_rows, discovery_samples, max_candidates);
    if let Some(p) = p.reconstruction {
        assign!(
            result.reconstruction,
            p,
            max_rows,
            max_columns,
            max_input_nonzeros,
            max_coefficient_terms,
            max_degree,
            max_probes,
            max_attempts,
            max_primes,
            max_cached_images,
            max_cached_values,
            max_weight_slots
        );
        if let Some(p) = p.arithmetic {
            assign!(
                result.reconstruction.arithmetic,
                p,
                max_exponent,
                max_polynomial_terms,
                max_term_operations
            );
        }
    }
    let r = result.reconstruction;
    if [
        result.batch_rows,
        result.discovery_samples,
        result.max_candidates,
        r.max_rows,
        r.max_columns,
        r.max_input_nonzeros,
        r.max_coefficient_terms,
        r.max_probes,
        r.max_attempts,
        r.max_cached_images,
        r.max_cached_values,
        r.max_weight_slots,
        r.arithmetic.max_polynomial_terms,
        r.arithmetic.max_term_operations,
    ]
    .contains(&0)
        || r.max_degree == 0
        || r.max_primes < 2
        || r.arithmetic.max_exponent == 0
    {
        return Err(AppError::input(
            "modular resource allowances must be positive; reconstruction requires at least two primes",
        ));
    }
    Ok(result)
}

fn policy_value(p: TerminalRelationModularOptions) -> Value {
    let r = p.reconstruction;
    json!({"batch_rows":p.batch_rows,"discovery_samples":p.discovery_samples,"max_candidates":p.max_candidates,
        "reconstruction":{"max_rows":r.max_rows,"max_columns":r.max_columns,"max_input_nonzeros":r.max_input_nonzeros,
        "max_coefficient_terms":r.max_coefficient_terms,"max_degree":r.max_degree,"max_probes":r.max_probes,
        "max_attempts":r.max_attempts,"max_primes":r.max_primes,"max_cached_images":r.max_cached_images,
        "max_cached_values":r.max_cached_values,"max_weight_slots":r.max_weight_slots,
        "arithmetic":{"max_exponent":r.arithmetic.max_exponent,"max_polynomial_terms":r.arithmetic.max_polynomial_terms,
        "max_term_operations":r.arithmetic.max_term_operations}}})
}

fn update(report: &mut Value, stats: TerminalRelationModularStats, complete: bool) {
    report["modular_search_complete"] = json!(complete);
    report["modular_refinement"] = json!({"search_recipe":RECIPE,
        "stage":report["modular_refinement"]["stage"].as_str().unwrap_or("finite-field discovery"),
        "discovery_assurance":"heuristic finite-field search; misses do not prove absence",
        "accepted_relation_assurance":"exact Symbolica W*A identity across every physical column",
        "source_rows_scanned":stats.source_rows_scanned,"total_source_rows":stats.total_source_rows,
        "modular_independent_rows":stats.modular_independent_rows,"candidates_found":stats.candidates_found,
        "pending_candidates":stats.pending_candidates,"accepted_relations":stats.accepted_relations,
        "reconstruction_misses":stats.reconstruction_misses,"search_exhausted":stats.search_exhausted});
}

pub(super) fn validate_file(directory: &Path, report: &Value) -> Result<Option<Vec<u8>>, AppError> {
    let Some(name) = report["modular_state"]["file"].as_str() else {
        return Ok(None);
    };
    if from_report(report)? != MasterRefinementStrategy::ModularCircuits {
        return Err(AppError::input(
            "modular state requires a modular refinement strategy",
        ));
    }
    let bytes = storage::read_bounded(
        &storage::safe_child(directory, name)?,
        profile::native_io_limits(None).max_program_bytes,
    )?;
    if report["modular_state"]["bytes"].as_u64() != Some(bytes.len() as u64)
        || report["modular_state"]["blake3"].as_str()
            != Some(blake3::hash(&bytes).to_hex().as_str())
        || report["modular_state"]["accepted_session_blake3"] != report["native_state"]["blake3"]
    {
        return Err(AppError::input(
            "modular checkpoint digest, size or accepted-session binding mismatch",
        ));
    }
    Ok(Some(bytes))
}

pub(super) fn validate_metadata(directory: &Path, report: &Value) -> Result<(), AppError> {
    if let Some(name) = report["modular_state"]["file"].as_str() {
        if from_report(report)? != MasterRefinementStrategy::ModularCircuits
            || std::fs::metadata(storage::safe_child(directory, name)?)
                .map_err(io)?
                .len()
                != report["modular_state"]["bytes"]
                    .as_u64()
                    .unwrap_or(u64::MAX)
            || report["modular_state"]["accepted_session_blake3"]
                != report["native_state"]["blake3"]
        {
            return Err(AppError::input(
                "modular checkpoint size, strategy or accepted-session binding mismatch",
            ));
        }
    }
    Ok(())
}

fn load(
    directory: &Path,
    report: &Value,
) -> Result<Option<TerminalRelationModularSession>, AppError> {
    validate_file(directory, report)?
        .map(|bytes| {
            let state = TerminalRelationModularSession::from_native_bytes(
                &bytes,
                profile::from_report(report)?.relation_limits(),
                profile::native_io_limits(None),
            )
            .map_err(io)?;
            let accepted = state
                .session()
                .to_native_bytes(profile::native_io_limits(None))
                .map_err(io)?;
            if report["native_state"]["blake3"].as_str()
                != Some(blake3::hash(&accepted).to_hex().as_str())
            {
                return Err(AppError::input(
                    "modular checkpoint contains a different accepted exact session",
                ));
            }
            Ok(state)
        })
        .transpose()
}

pub(super) fn complete(directory: &Path, report: &Value) -> Result<bool, AppError> {
    Ok(load(directory, report)?.is_some_and(|state| state.is_search_complete()))
}

pub(super) fn phase_complete(
    directory: &Path,
    report: &Value,
    session: &TerminalRelationSession,
) -> Result<bool, AppError> {
    if from_report(report)? == MasterRefinementStrategy::ModularCircuits {
        complete(directory, report)
    } else {
        Ok(session.is_complete())
    }
}

pub(super) fn inherit_state(
    source: &Path,
    destination: &Path,
    report: &Value,
) -> Result<(), AppError> {
    if let Some(bytes) = validate_file(source, report)? {
        let name = report["modular_state"]["file"].as_str().unwrap();
        write_file_atomically(&storage::safe_child(destination, name)?, &bytes, true)
            .map_err(io)?;
    }
    Ok(())
}

fn checkpoint(
    options: &MasterReductionOptions,
    state: &TerminalRelationModularSession,
    report: &mut Value,
    status: &str,
    started: Instant,
    observer: &impl Fn(Value),
) -> Result<(), AppError> {
    let generation = report["checkpoint"]["generation"]
        .as_u64()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| AppError::limit("checkpoint generation overflow"))?;
    let file = format!("modular-state-{generation:016}.rrbin");
    let bytes = state
        .to_native_bytes(profile::native_io_limits(None))
        .map_err(io)?;
    let accepted = state
        .session()
        .to_native_bytes(profile::native_io_limits(None))
        .map_err(io)?;
    write_file_atomically(&options.directory.join(&file), &bytes, true).map_err(io)?;
    report["modular_state"] = json!({"file":file,"bytes":bytes.len(),"blake3":blake3::hash(&bytes).to_hex().to_string(),
        "accepted_session_blake3":blake3::hash(&accepted).to_hex().to_string()});
    update(report, state.statistics(), state.is_search_complete());
    save(options, state.session(), report, status, started, observer)
}

pub(super) fn execute(
    options: &MasterReductionOptions,
    session: &mut TerminalRelationSession,
    report: &mut Value,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
    started: Instant,
) -> Result<(), AppError> {
    budget::configure(options, session, report)?;
    while session.statistics().pending_rebuild_rows > 0 && !cancel.load(Ordering::Relaxed) {
        session.step_rebuild_only(cancel).map_err(io)?;
    }
    if cancel.load(Ordering::Relaxed) {
        save(options, session, report, "paused", started, observer)?;
        return Ok(());
    }
    let initializing = !report["modular_state"].is_object();
    report["stage"] = json!(if initializing {
        "initializing_modular_image"
    } else {
        "loading_modular_checkpoint"
    });
    report["modular_refinement"]["stage"] = json!(if initializing {
        "initializing modular image"
    } else {
        "loading modular checkpoint"
    });
    report["execution"] = json!({"strategy":RECIPE,"workers":1,"discovery_workers":1,"reserved_workers":options.threads,
        "checkpoint":"separate heuristic source cursor and exact accepted terminal session"});
    observer(event(report, "master_reduction_progress", started));
    let mut state = if let Some(state) = load(&options.directory, report)? {
        state
    } else {
        let bytes = session
            .to_native_bytes(profile::native_io_limits(None))
            .map_err(io)?;
        let native = TerminalRelationSession::from_native_bytes(
            &bytes,
            profile::from_report(report)?.relation_limits(),
            profile::native_io_limits(None),
        )
        .map_err(io)?;
        TerminalRelationModularSession::new_with_io(
            native,
            policy(
                options
                    .modular_policy
                    .as_ref()
                    .or_else(|| report.get("modular_policy")),
            )?,
            profile::native_io_limits(None),
        )
        .map_err(io)?
    };
    if let Some(requested) = &options.modular_policy {
        state.set_options(policy(Some(requested))?).map_err(io)?;
    }
    state.set_workers(options.threads).map_err(io)?;
    if report["collection"]["status"] == "completed" && !state.is_search_complete() {
        // Retrying rejected circuits may add terminal equations. Freeze the
        // current collection proof before rebinding it to the enlarged session.
        collection::inherit_with_policy(
            &options.directory,
            &options.directory,
            report,
            options.finite_policy.as_ref(),
        )?;
    }
    state
        .set_max_columns(
            report["max_relation_columns"]
                .as_u64()
                .map(|value| value as usize),
        )
        .map_err(io)?;
    report["modular_policy"] = policy_value(state.options());
    report["modular_refinement"]["stage"] = json!(if state.is_search_complete() {
        "modular discovery completed"
    } else {
        "finite-field discovery"
    });
    report["execution"] = json!({"strategy":RECIPE,"workers":state.workers(),"discovery_workers":1,"reserved_workers":options.threads,
        "checkpoint":"separate heuristic source cursor and exact accepted terminal session"});
    checkpoint(options, &state, report, "running", started, observer)?;
    let mut last_checkpoint = Instant::now();
    let mut last_event = Instant::now();
    while !state.is_search_complete() && !cancel.load(Ordering::Relaxed) {
        state
            .step_observed(cancel, |progress| {
                report["modular_refinement"]["stage"] = json!(progress.stage);
                if last_event.elapsed() >= Duration::from_millis(250) {
                    update(report, progress.statistics, false);
                    observer(event(report, "master_reduction_progress", started));
                    last_event = Instant::now();
                }
            })
            .map_err(io)?;
        update(report, state.statistics(), state.is_search_complete());
        update_stats(report, state.session());
        if last_checkpoint.elapsed() >= options.checkpoint_interval {
            checkpoint(options, &state, report, "running", started, observer)?;
            last_checkpoint = Instant::now();
        }
    }
    checkpoint(
        options,
        &state,
        report,
        if cancel.load(Ordering::Relaxed) {
            "paused"
        } else {
            "running"
        },
        started,
        observer,
    )?;
    *session = state.into_session();
    if cancel.load(Ordering::Relaxed) {
        return Ok(());
    }
    collection::complete(options, session, report, cancel, observer, started)?;
    if cancel.load(Ordering::Relaxed) {
        save(options, session, report, "paused", started, observer)?;
        return Ok(());
    }
    // The exact cursor remains unfinished even after a modular discovery pass.
    save(
        options,
        session,
        report,
        "completed_nonminimal",
        started,
        observer,
    )?;
    publish_final(options, report)?;
    observer(event(report, "master_reduction_finished", started));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn omitted_modular_grants_have_no_tuned_resource_ceilings() {
        let options = policy(None).unwrap();
        assert_eq!(options.discovery_samples, usize::MAX);
        let r = options.reconstruction;
        assert_eq!(
            r.arithmetic,
            rustred::algebra::ExactAlgebraLimits::unbounded_resources()
        );
        for limit in [
            r.max_rows,
            r.max_columns,
            r.max_input_nonzeros,
            r.max_coefficient_terms,
            r.max_probes,
            r.max_attempts,
            r.max_primes,
            r.max_cached_images,
            r.max_cached_values,
            r.max_weight_slots,
        ] {
            assert_eq!(limit, usize::MAX);
        }
        // Symbolica's public reconstruction representation doubles u16 degrees.
        assert_eq!(r.max_degree, u16::MAX / 2);
    }

    #[test]
    fn modular_strategy_rejects_explicit_exact_executor_knobs() {
        let mut options = MasterReductionOptions::new("unused", "unused");
        options.refinement_strategy = Some(MasterRefinementStrategy::ModularCircuits);
        assert_eq!(
            for_phase(&options, None).unwrap(),
            MasterRefinementStrategy::ModularCircuits
        );
        for block in [false, true] {
            options.relation_batch_rows = (!block).then_some(64);
            options.relation_block_rows = block.then_some(8);
            assert!(
                for_phase(&options, None)
                    .unwrap_err()
                    .to_string()
                    .contains("--modular-policy batch_rows/max_candidates")
            );
        }
    }

    #[test]
    fn strict_modular_policy_exposes_symbolica_allowances() {
        let p = policy(Some(&json!({"batch_rows":3,"max_candidates":2,"reconstruction":{"max_primes":23,"max_degree":128,"arithmetic":{"max_term_operations":17}}}))).unwrap();
        assert_eq!(p.batch_rows, 3);
        assert_eq!(p.reconstruction.max_primes, 23);
        assert_eq!(p.reconstruction.max_degree, 128);
        assert_eq!(p.reconstruction.arithmetic.max_term_operations, 17);
        for value in [
            json!({"unknown":1}),
            json!({"batch_rows":0}),
            json!({"reconstruction":{"max_primes":1}}),
        ] {
            assert!(policy(Some(&value)).is_err());
        }
    }
}
