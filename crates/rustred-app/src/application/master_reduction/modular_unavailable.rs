//! Exact-only builds retain the existing checkpoint/application contract.
use super::*;

fn unavailable() -> AppError {
    AppError::input("modular-circuits requires a RustRed build with the reconstruction feature")
}

pub(super) fn from_report(report: &Value) -> Result<MasterRefinementStrategy, AppError> {
    match report["refinement_strategy"].as_str().unwrap_or("exact") {
        "exact" => Ok(MasterRefinementStrategy::Exact),
        "modular-circuits" => Ok(MasterRefinementStrategy::ModularCircuits),
        _ => Err(AppError::input("unknown saved master refinement strategy")),
    }
}

pub(super) fn for_phase(
    options: &MasterReductionOptions,
    resumed: Option<&Value>,
) -> Result<MasterRefinementStrategy, AppError> {
    let saved = resumed.map(from_report).transpose()?;
    let strategy = options.refinement_strategy.or(saved).unwrap_or_default();
    if strategy == MasterRefinementStrategy::ModularCircuits || options.modular_policy.is_some() {
        return Err(unavailable());
    }
    if options.operation != MasterReductionOperation::Refine
        && options.refinement_strategy.is_some()
    {
        return Err(AppError::input(
            "refinement strategy options require explicit refinement",
        ));
    }
    Ok(strategy)
}

pub(super) fn hash(strategy: MasterRefinementStrategy, hash: &mut blake3::Hasher) {
    if strategy == MasterRefinementStrategy::ModularCircuits {
        hash.update(b"terminal-modular-circuits-v1");
    }
}

pub(super) fn record(
    report: &mut Value,
    strategy: MasterRefinementStrategy,
    _: &MasterReductionOptions,
) -> Result<(), AppError> {
    if strategy == MasterRefinementStrategy::ModularCircuits {
        return Err(unavailable());
    }
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
    Ok(())
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

pub(super) fn complete(_: &Path, _: &Value) -> Result<bool, AppError> {
    Err(unavailable())
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

pub(super) fn inherit_state(_: &Path, _: &Path, report: &Value) -> Result<(), AppError> {
    if report["modular_state"].is_object() {
        Err(unavailable())
    } else {
        Ok(())
    }
}

pub(super) fn execute(
    _: &MasterReductionOptions,
    _: &mut TerminalRelationSession,
    _: &mut Value,
    _: &AtomicBool,
    _: &impl Fn(Value),
    _: Instant,
) -> Result<(), AppError> {
    Err(unavailable())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_only_build_preserves_default_and_rejects_explicit_modular_execution() {
        let mut options = MasterReductionOptions::new("unused", "unused");
        assert_eq!(
            for_phase(&options, None).unwrap(),
            MasterRefinementStrategy::Exact
        );
        let mut report = json!({});
        record(&mut report, MasterRefinementStrategy::Exact, &options).unwrap();
        assert!(report.get("refinement_strategy").is_none());
        options.refinement_strategy = Some(MasterRefinementStrategy::ModularCircuits);
        assert!(
            for_phase(&options, None)
                .unwrap_err()
                .to_string()
                .contains("reconstruction feature")
        );
    }
}
