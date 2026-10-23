//! Connect finite verified symmetries and optional saved candidate programs.
use super::*;
use crate::application::routed_campaign::{
    PreparedTerminalEquations, RoutedCampaignRequest, TerminalEquationProvider,
    prepare_terminal_equations_with_cache_mode,
};
use rustred::family::IntegralKey;
use rustred::reduction::terminal_normalization::TerminalCircuitEquations;
use rustred::reduction::terminal_relations::{TerminalEquation, TerminalRelationFiniteProvider};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Runtime-only execution metadata. The old callable provider remains the
/// contract for dynamic candidate equations and existing serial tests. The finite
/// witness providers additionally guarantee *every* off-map lookup is empty.
/// This is never inferred from past lookups, ranks, supports or sampled zeros.
pub(super) struct PreparedAssistance {
    pub provider: TerminalEquationProvider,
    pub finite: Option<TerminalRelationFiniteProvider>,
}

impl PreparedAssistance {
    pub fn kind(&self) -> &'static str {
        if self.finite.is_some() {
            "finite-immutable"
        } else {
            "dynamic"
        }
    }
}

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
    if options.sector_symmetry_assistance {
        parts.push(super::sector_symmetry::binding(options, session, report)?);
    } else if options.numerator_symmetry_assistance {
        parts.push(super::numerator_symmetry::binding(
            session.family_owner().fingerprint(),
            &session
                .raw_terminals()
                .union(session.normalization().canonical_terminals())
                .cloned()
                .collect(),
            profile::from_report(report)?,
        )?);
    }
    if options.local_dimensional_recurrence_assistance {
        parts.push(super::local_dimension::binding(session, report)?);
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
    if (options.sector_symmetry_policy.is_some()
        || options.sector_symmetry_saved_projection.is_some())
        && (!options.sector_symmetry_assistance
            || options.operation != MasterReductionOperation::Refine)
    {
        return Err(AppError::input(
            "sector-symmetry policy requires explicit sector-symmetry refinement",
        ));
    }
    if options.operation == MasterReductionOperation::Publish {
        if session.assistance_binding().is_some() {
            inherited::validate(session, report)?;
            report["finite_search_restarted_for_policy"] = json!(false);
            report["saved_rule_assistance"] = json!(false);
            report["circuit_symmetry_assistance"] = json!(false);
            report["numerator_symmetry_assistance"] = json!(false);
            report["sector_symmetry_assistance"] = json!(false);
            report
                .as_object_mut()
                .unwrap()
                .remove("local_dimensional_recurrence_assistance");
            report["relation_authority"] = json!(inherited::authority(report)?);
            fresh::record_authority(report);
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
    if options.sector_symmetry_assistance {
        report["sector_symmetry_policy"] = json!(sector_symmetry::policy_name(
            sector_symmetry::for_phase(options, report)?
        ));
        report["sector_symmetry_saved_projection"] =
            json!(sector_symmetry::projection_for_phase(options, report)?);
    } else {
        report
            .as_object_mut()
            .expect("report object")
            .remove("sector_symmetry_policy");
        report
            .as_object_mut()
            .expect("report object")
            .remove("sector_symmetry_saved_projection");
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
    report
        .as_object_mut()
        .expect("report object")
        .remove("numerator_symmetry_preparation");
    report
        .as_object_mut()
        .expect("report object")
        .remove("sector_symmetry_preparation");
    report
        .as_object_mut()
        .unwrap()
        .remove("local_dimensional_recurrence_preparation");
    if changes_policy {
        report
            .as_object_mut()
            .expect("report object")
            .remove("numerator_symmetry_certificate");
        for field in [
            "sector_symmetry_certificate",
            "sector_symmetry_normalization_certificate",
            "sector_symmetry_task_certificate",
            "sector_symmetry_reduced_certificate",
            "sector_symmetry_projection_checkpoint",
            "sector_symmetry_reduction",
            "local_dimensional_recurrence_certificate",
        ] {
            report.as_object_mut().expect("report object").remove(field);
        }
    }
    if changes_policy
        && (session.statistics().completed_source_rows != 0
            || session.statistics().pending_rebuild_rows != 0
            || session.assistance_binding().is_some())
    {
        if session.retained_relations_binding().is_some() {
            return Err(AppError::input(
                "changing equation providers on retained relations requires a new --fresh-search round",
            ));
        }
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
        if report["refinement_strategy"] != "modular-circuits" {
            report["completion_policy"] = json!(
                "finite recorded ordinary-IBP seeds and optional verified circuit/saved equations; retain unresolved auxiliaries"
            );
        }
    }
    report["saved_rule_assistance"] = json!(options.saved_rule_assistance);
    report["circuit_symmetry_assistance"] = json!(options.circuit_symmetry_assistance);
    report["numerator_symmetry_assistance"] = json!(options.numerator_symmetry_assistance);
    report["sector_symmetry_assistance"] = json!(options.sector_symmetry_assistance);
    if options.local_dimensional_recurrence_assistance {
        report["local_dimensional_recurrence_assistance"] = json!(true);
    } else {
        report
            .as_object_mut()
            .unwrap()
            .remove("local_dimensional_recurrence_assistance");
    }
    report["relation_authority"] = json!(authority(
        options.saved_rule_assistance,
        options.circuit_symmetry_assistance,
        options.numerator_symmetry_assistance,
        options.sector_symmetry_assistance,
        sector_symmetry::projection_from_report(report)?,
        options.local_dimensional_recurrence_assistance,
    ));
    fresh::record_authority(report);
    Ok(())
}

pub(super) fn authority(
    saved: bool,
    circuit: bool,
    numerator: bool,
    sector: bool,
    projection: bool,
    local_dimension: bool,
) -> &'static str {
    if local_dimension {
        return if saved || (sector && projection) {
            "exact ordinary-IBP consequences including independently replayed localized dimensional-pair sources and optional verified symmetries, plus authenticated supplied candidate formulas whose original source provenance is not replay-certified"
        } else {
            "finite ordinary IBPs, independently replayed localized dimensional-pair ordinary-source certificates and optional verified same-d symmetries; no shifted-dimension runtime masters or numerical values"
        };
    }
    if sector {
        if !projection && !saved {
            return "finite ordinary IBPs and independently replayed sector momentum-change and exact normalization identities composed with retained exact maps; no new candidate-program application";
        }
        return "exact consequences of ordinary IBPs, independently replayed sector momentum-change identities and authenticated supplied candidate programs; candidate source provenance is not replay-certified";
    }
    match (saved, circuit, numerator) {
        (true, _, true) => {
            "exact consequences of ordinary IBPs, verified numerator momentum-change identities and supplied candidate rules; candidate source provenance is not replay-certified"
        }
        (false, _, true) => {
            "finite ordinary IBPs and independently verified denominator-preserving numerator momentum-change identities with exact normalization; no candidate-rule authority used"
        }
        (true, true, false) => {
            "exact consequences of ordinary IBPs, verified circuit symmetries/routes and supplied candidate rules; candidate source provenance is not replay-certified"
        }
        (true, false, false) => {
            "exact consequences of ordinary IBPs, verified routes and supplied candidate rules; candidate source provenance is not replay-certified"
        }
        (false, true, false) => {
            "finite ordinary IBPs and independently verified circuit change-of-variable equations with exact normalization; no candidate-rule authority used"
        }
        (false, false, false) => "finite ordinary IBP equations with exact normalization",
    }
}

/// Resume never repairs or resets a mismatched persisted policy implicitly.
pub(super) fn validate(
    options: &MasterReductionOptions,
    session: &TerminalRelationSession,
    report: &Value,
) -> Result<(), AppError> {
    sector_symmetry::for_phase(options, report)?;
    sector_symmetry::projection_for_phase(options, report)?;
    profile::for_resume(options.normalization_profile, report)?;
    if options.operation == MasterReductionOperation::Publish
        && (session.assistance_binding().is_some()
            || report.get("inherited_equation_authority").is_some())
    {
        if options.saved_rule_assistance
            || options.circuit_symmetry_assistance
            || options.numerator_symmetry_assistance
            || options.sector_symmetry_assistance
            || options.local_dimensional_recurrence_assistance
            || report["saved_rule_assistance"].as_bool().unwrap_or(false)
            || report["circuit_symmetry_assistance"]
                .as_bool()
                .unwrap_or(false)
            || report["numerator_symmetry_assistance"]
                .as_bool()
                .unwrap_or(false)
            || report["sector_symmetry_assistance"]
                .as_bool()
                .unwrap_or(false)
            || report["local_dimensional_recurrence_assistance"]
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
        || report["numerator_symmetry_assistance"]
            .as_bool()
            .unwrap_or(false)
            != options.numerator_symmetry_assistance
        || report["sector_symmetry_assistance"]
            .as_bool()
            .unwrap_or(false)
            != options.sector_symmetry_assistance
        || report["local_dimensional_recurrence_assistance"]
            .as_bool()
            .unwrap_or(false)
            != options.local_dimensional_recurrence_assistance
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

#[cfg(test)]
pub(super) fn prepare(
    options: &MasterReductionOptions,
    session: &TerminalRelationSession,
    report: &mut Value,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
    started: Instant,
) -> Result<Option<TerminalEquationProvider>, AppError> {
    Ok(
        prepare_for_execution(options, session, report, cancel, observer, started)?
            .map(|prepared| prepared.provider),
    )
}

pub(super) fn prepare_for_execution(
    options: &MasterReductionOptions,
    session: &TerminalRelationSession,
    report: &mut Value,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
    started: Instant,
) -> Result<Option<PreparedAssistance>, AppError> {
    validate(options, session, report)?;
    if !options.saved_rule_assistance
        && !options.circuit_symmetry_assistance
        && !options.numerator_symmetry_assistance
        && !options.sector_symmetry_assistance
        && !options.local_dimensional_recurrence_assistance
    {
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
    let sector = if options.sector_symmetry_assistance {
        Some(super::sector_symmetry::prepare(
            options, session, report, cancel, observer, started,
        )?)
    } else {
        None
    };
    let numerator = if options.numerator_symmetry_assistance && !options.sector_symmetry_assistance
    {
        Some(super::numerator_symmetry::prepare(
            options, session, report, cancel, observer, started,
        )?)
    } else {
        None
    };
    let local_dimension = if options.local_dimensional_recurrence_assistance {
        Some(super::local_dimension::prepare(
            options, session, report, cancel, observer, started,
        )?)
    } else {
        None
    };
    let mut saved = if options.saved_rule_assistance
        || (options.sector_symmetry_assistance
            && sector_symmetry::projection_for_phase(options, report)?)
    {
        Some(prepare_saved(
            options, session, report, cancel, observer, started,
        )?)
    } else {
        None
    };
    let numerator_rows = if let Some(numerator) = &numerator {
        Some(super::numerator_symmetry::prepare_rows(
            numerator,
            saved.as_mut(),
            report,
            cancel,
            observer,
            started,
        )?)
    } else {
        None
    };
    let sector_rows = if let Some(sector) = &sector {
        Some(super::sector_symmetry::prepare_rows(
            options,
            session,
            sector,
            saved.as_mut(),
            report,
            cancel,
            observer,
            started,
        )?)
    } else {
        None
    };
    if !options.saved_rule_assistance {
        // Both native witness services define a finite immutable map over the
        // exact raw/canonical inventory and return [] at every other key.
        let mut keys: std::collections::BTreeSet<_> = session
            .raw_terminals()
            .union(session.normalization().canonical_terminals())
            .cloned()
            .collect();
        if let Some(sector) = &sector_rows {
            keys.extend(sector.keys().cloned());
        }
        let mut rows_by_key = BTreeMap::<IntegralKey, Vec<TerminalEquation>>::new();
        for key in keys {
            let mut rows = circuit
                .as_ref()
                .map_or_else(Vec::new, |plan| plan.equations(&key).to_vec());
            if let Some(numerator) = &numerator_rows {
                rows.extend(numerator.get(&key).into_iter().flatten().cloned());
            }
            if let Some(sector) = &sector_rows {
                rows.extend(sector.get(&key).into_iter().flatten().cloned());
            }
            if let Some(local) = &local_dimension {
                rows.extend(local.equations_for(&key).iter().cloned());
            }
            if !rows.is_empty() {
                rows_by_key.insert(key, rows);
            }
        }
        let shared = Arc::new(rows_by_key);
        let serial = Arc::clone(&shared);
        return Ok(Some(PreparedAssistance {
            provider: Box::new(move |key| Ok(serial.get(key).cloned().unwrap_or_default())),
            finite: Some(TerminalRelationFiniteProvider::new(shared)),
        }));
    }
    Ok(Some(PreparedAssistance {
        finite: None,
        provider: Box::new(move |key| {
            let mut rows = circuit
                .as_ref()
                .map_or_else(Vec::new, |plan| plan.equations(key).to_vec());
            if let Some(numerator) = &numerator_rows {
                rows.extend(numerator.get(key).into_iter().flatten().cloned());
            }
            if let Some(sector) = &sector_rows {
                rows.extend(sector.get(key).into_iter().flatten().cloned());
            }
            if let Some(local) = &local_dimension {
                rows.extend(local.equations_for(key).iter().cloned());
            }
            if let Some(saved) = &mut saved {
                rows.extend((saved.provider)(key)?);
            }
            Ok(rows)
        }),
    }))
}

fn prepare_saved(
    options: &MasterReductionOptions,
    session: &TerminalRelationSession,
    report: &mut Value,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
    started: Instant,
) -> Result<PreparedTerminalEquations, AppError> {
    let selection_name = report["inputs"]["selection"]
        .as_str()
        .ok_or_else(|| AppError::input("publication has no saved owner selection"))?;
    let selection_path = storage::safe_child(&options.directory, selection_name)?;
    let max_source_bytes =
        options
            .finite_policy
            .as_ref()
            .map_or(Ok(16 * 1024 * 1024), |policy| {
                usize::try_from(policy.control.max_bytes).map_err(|_| {
                    AppError::limit("finite equation metadata allowance exceeds platform size")
                })
            })?;
    let selection_bytes = storage::read_bounded(&selection_path, max_source_bytes)?;
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
    configure_saved_resources(options, &mut request);
    let prepared = prepare_terminal_equations_with_cache_mode(
        &request,
        session.family_owner().fingerprint(),
        &expected_digests,
        cancel,
        &|native| {
            let mut output = event(report, "master_reduction_progress", started);
            output["preparation_event"] = native;
            observer(output);
        },
        options.sector_symmetry_assistance,
    )?;
    Ok(prepared)
}

/// Preserve historical saved-only settings; only the opt-in full symmetry-row
/// application delegates resource ownership to its caller's RAM guard.
pub(super) fn configure_saved_resources(
    options: &MasterReductionOptions,
    request: &mut RoutedCampaignRequest,
) {
    if options.numerator_symmetry_assistance || options.sector_symmetry_assistance {
        super::numerator_symmetry::configure_saved_resources(request);
    }
}
