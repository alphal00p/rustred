//! Connect finite verified symmetries and optional saved candidate programs.
use super::*;
use crate::application::routed_campaign::{
    RoutedCampaignRequest, TerminalEquationProvider, prepare_terminal_equations,
};
use rustred::reduction::terminal_normalization::TerminalCircuitEquations;

pub(super) fn expected_binding(
    options: &MasterReductionOptions,
    session: &TerminalRelationSession,
    report: &Value,
) -> Result<Option<String>, AppError> {
    let mut parts = Vec::new();
    if options.saved_rule_assistance {
        let binding = report["inputs"]["program_binding"]
            .as_str()
            .ok_or_else(|| AppError::input("publication has no saved program binding"))?;
        // Preserve the existing saved-only provider binding on resume.
        parts.push(format!("saved-rule-assistance-v1:{binding}"));
    }
    if options.circuit_symmetry_assistance {
        // The circuit provider is deliberately finite: its identity includes
        // the exact raw inventory, not just the selected preparation strategy.
        // Its normalized representatives are determined by that inventory and
        // the authenticated family, and join the finite preparation below.
        let mut hash = blake3::Hasher::new();
        hash.update(session.family_owner().fingerprint().as_bytes());
        hash.update(
            &u64::try_from(session.raw_terminals().len())
                .map_err(io)?
                .to_le_bytes(),
        );
        for key in session.raw_terminals() {
            for power in key.powers() {
                hash.update(&power.to_le_bytes());
            }
        }
        profile::from_report(report)?.hash(&mut hash);
        parts.push(format!(
            "circuit-symmetry-assistance-v1:{}",
            hash.finalize().to_hex()
        ));
    }
    Ok((!parts.is_empty()).then(|| parts.join(";")))
}

/// Configure the finite-search policy before publishing its first native
/// checkpoint. Provider preparation may be cancelled or fail after this step.
pub(super) fn configure(
    options: &MasterReductionOptions,
    session: &mut TerminalRelationSession,
    report: &mut Value,
) -> Result<(), AppError> {
    if options.operation == MasterReductionOperation::Publish {
        if session.assistance_binding().is_some() {
            inherited::validate(session, report)?;
            report["finite_search_restarted_for_policy"] = json!(false);
            report["saved_rule_assistance"] = json!(false);
            report["circuit_symmetry_assistance"] = json!(false);
            report["relation_authority"] = json!(inherited::authority(report)?);
            return Ok(());
        }
        if report.get("inherited_equation_authority").is_some() {
            return Err(AppError::input(
                "inherited equation authority has no native binding",
            ));
        }
    }
    if options.resume && report["native_state"].is_object() {
        return validate(options, session, report);
    }
    let binding = expected_binding(options, session, report)?;
    report
        .as_object_mut()
        .expect("report object")
        .remove("inherited_equation_authority");
    let changes_policy = session.assistance_binding() != binding.as_deref();
    report["finite_search_restarted_for_policy"] = json!(false);
    report
        .as_object_mut()
        .expect("report object")
        .remove("circuit_equation_preparation");
    if changes_policy
        && (session.statistics().completed_source_rows != 0
            || session.statistics().pending_rebuild_rows != 0
            || session.assistance_binding().is_some())
    {
        *session = TerminalRelationSession::new(
            session.family_owner().clone(),
            session.raw_terminals().clone(),
            session.statistics().seed_depth.max(options.seed_depth),
            profile::from_report(report)?.relation_limits(),
        )
        .map_err(io)?;
        report["finite_search_restarted_for_policy"] = json!(true);
    }
    if let Some(binding) = binding {
        session.enable_assistance(binding).map_err(io)?;
    }
    if options.operation == MasterReductionOperation::Refine {
        let inherited = report["containing_sector_depth"].as_u64().unwrap_or(0);
        let depth = inherited.max(u64::from(options.containing_sector_depth));
        let depth = usize::try_from(depth)
            .map_err(|_| AppError::input("containing-sector depth overflows platform size"))?;
        let added = session.extend_containing_sector_seeds(depth).map_err(io)?;
        report["containing_sector_depth"] = json!(depth);
        report["added_containing_sector_seeds"] = json!(added);
        report["requested_containing_sector_depth"] = json!(options.containing_sector_depth);
        report["source_selection"] = json!({
            "signed_l1_depth": session.statistics().seed_depth,
            "containing_sector_depth": depth,
            "new_seeds_are_terminals": false,
        });
        report["completion_policy"] = json!(
            "finite recorded ordinary-IBP seeds and optional verified circuit/saved equations; retain unresolved auxiliaries"
        );
    }
    report["saved_rule_assistance"] = json!(options.saved_rule_assistance);
    report["circuit_symmetry_assistance"] = json!(options.circuit_symmetry_assistance);
    report["relation_authority"] = json!(authority(
        options.saved_rule_assistance,
        options.circuit_symmetry_assistance
    ));
    Ok(())
}

pub(super) fn authority(saved: bool, circuit: bool) -> &'static str {
    match (saved, circuit) {
        (true, true) => {
            "exact consequences of ordinary IBPs, verified circuit symmetries/routes and supplied candidate rules; candidate source provenance is not replay-certified"
        }
        (true, false) => {
            "exact consequences of ordinary IBPs, verified routes and supplied candidate rules; candidate source provenance is not replay-certified"
        }
        (false, true) => {
            "finite ordinary IBPs and independently verified circuit change-of-variable equations with exact normalization; no candidate-rule authority used"
        }
        (false, false) => "finite ordinary IBP equations with exact normalization",
    }
}

/// Resume never repairs or resets a mismatched persisted policy implicitly.
pub(super) fn validate(
    options: &MasterReductionOptions,
    session: &TerminalRelationSession,
    report: &Value,
) -> Result<(), AppError> {
    profile::for_resume(options.normalization_profile, report)?;
    if options.operation == MasterReductionOperation::Publish
        && (session.assistance_binding().is_some()
            || report.get("inherited_equation_authority").is_some())
    {
        if options.saved_rule_assistance
            || options.circuit_symmetry_assistance
            || report["saved_rule_assistance"].as_bool().unwrap_or(false)
            || report["circuit_symmetry_assistance"]
                .as_bool()
                .unwrap_or(false)
        {
            return Err(AppError::input(
                "publication cannot request new equation providers",
            ));
        }
        return inherited::validate(session, report);
    }
    let binding = expected_binding(options, session, report)?;
    if report["saved_rule_assistance"].as_bool().unwrap_or(false) != options.saved_rule_assistance
        || report["circuit_symmetry_assistance"]
            .as_bool()
            .unwrap_or(false)
            != options.circuit_symmetry_assistance
        || session.assistance_binding() != binding.as_deref()
        || (options.operation == MasterReductionOperation::Refine
            && report["requested_containing_sector_depth"]
                .as_u64()
                .unwrap_or(0)
                != u64::from(options.containing_sector_depth))
    {
        return Err(AppError::input(
            "native checkpoint, manifest and requested terminal-search policy differ; start a new phase directory",
        ));
    }
    Ok(())
}

pub(super) fn prepare(
    options: &MasterReductionOptions,
    session: &TerminalRelationSession,
    report: &mut Value,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
    started: Instant,
) -> Result<Option<TerminalEquationProvider>, AppError> {
    validate(options, session, report)?;
    if !options.saved_rule_assistance && !options.circuit_symmetry_assistance {
        return Ok(None);
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(AppError::execution(
            "paused before preparing terminal equations",
        ));
    }
    let circuit = if options.circuit_symmetry_assistance {
        report["stage"] = json!("preparing_circuit_equations");
        observer(event(report, "master_reduction_progress", started));
        let keys = session
            .raw_terminals()
            .union(session.normalization().canonical_terminals())
            .cloned()
            .collect();
        let prepared = TerminalCircuitEquations::prepare(
            session.family_owner().clone(),
            &keys,
            rustred::reduction::terminal_normalization::TerminalCircuitLimits {
                normalization: profile::from_report(report)?.normalization_limits(),
                ..Default::default()
            },
        )
        .map_err(io)?;
        let stats = prepared.statistics();
        report["circuit_equation_preparation"] = json!({
            "input_keys": stats.input_keys,
            "analyzed_supports": stats.analyzed_supports,
            "verified_generators": stats.verified_generators,
            "equation_targets": stats.equation_targets,
            "equations": stats.equations,
            "output_terms": stats.output_terms,
            "trivial_equations": stats.trivial_equations,
            "transport_operations": stats.transport_operations,
            "transport_endpoints": stats.transport_endpoints,
            "skipped": stats.skipped.iter().map(|(reason, count)| json!({
                "reason": format!("{reason:?}"), "count": count,
            })).collect::<Vec<_>>(),
            "scope": "raw and normalized terminal inventory only; all transported offspring remain unresolved columns",
        });
        if cancel.load(Ordering::Relaxed) {
            return Err(AppError::execution(
                "paused after preparing circuit equations",
            ));
        }
        Some(prepared)
    } else {
        None
    };
    let mut saved = if options.saved_rule_assistance {
        Some(prepare_saved(
            options, session, report, cancel, observer, started,
        )?)
    } else {
        None
    };
    Ok(Some(Box::new(move |key| {
        let mut rows = circuit
            .as_ref()
            .map_or_else(Vec::new, |plan| plan.equations(key).to_vec());
        if let Some(saved) = &mut saved {
            rows.extend(saved(key)?);
        }
        Ok(rows)
    })))
}

fn prepare_saved(
    options: &MasterReductionOptions,
    session: &TerminalRelationSession,
    report: &mut Value,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
    started: Instant,
) -> Result<TerminalEquationProvider, AppError> {
    let selection_name = report["inputs"]["selection"]
        .as_str()
        .ok_or_else(|| AppError::input("publication has no saved owner selection"))?;
    let selection_path = storage::safe_child(&options.directory, selection_name)?;
    let selection_bytes = storage::read_bounded(&selection_path, 16 * 1024 * 1024)?;
    let binding = blake3::hash(&selection_bytes).to_hex().to_string();
    if Some(binding.as_str()) != report["inputs"]["program_binding"].as_str() {
        return Err(AppError::input("saved equation selection binding mismatch"));
    }
    let expected_digests = report["inputs"]["payloads"]
        .as_array()
        .ok_or_else(|| AppError::input("publication has no saved payload inventory"))?
        .iter()
        .map(|entry| {
            entry["blake3"]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| AppError::input("saved payload digest missing"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    report["stage"] = json!("preparing_saved_equations");
    observer(event(report, "master_reduction_progress", started));
    let mut request = RoutedCampaignRequest::new(
        String::from_utf8(selection_bytes).map_err(io)?,
        String::new(),
    );
    request.owner_base = selection_path
        .parent()
        .ok_or_else(|| AppError::input("saved selection has no parent"))?
        .to_path_buf();
    request.workers = options.threads;
    let prepared = prepare_terminal_equations(
        &request,
        session.family_owner().fingerprint(),
        &expected_digests,
        cancel,
        &|native| {
            let mut output = event(report, "master_reduction_progress", started);
            output["preparation_event"] = native;
            observer(output);
        },
    )?;
    Ok(prepared)
}
