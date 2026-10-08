//! Explicit refinement from an immutable, portable publication.
use super::*;
use std::collections::BTreeSet;

/// Refine an existing publication into a distinct, self-contained directory.
/// No original CP6 checkpoint, reinspection, or campaign inputs are required.
/// `directory`, `seed_depth`, `resume` and `checkpoint_interval` are used;
/// `checkpoint` is deliberately irrelevant. Requested depth is a lower bound:
/// previously completed deeper work is never truncated.
pub fn master_refine_published_artifact(
    source: &Path,
    options: &MasterReductionOptions,
    cancellation: &AtomicBool,
    observer: impl Fn(Value),
) -> Result<Value, AppError> {
    if options.operation != MasterReductionOperation::Refine
        || options.previous_artifact.is_some()
        || options.checkpoint_interval.is_zero()
        || options.threads == 0
    {
        return Err(AppError::input(
            "artifact refinement requires Refine, positive limits and no previous_artifact",
        ));
    }
    let source_directory = if source.is_dir() {
        source
    } else {
        source.parent().unwrap_or(Path::new("."))
    };
    let source_directory = source_directory.canonicalize().map_err(io)?;
    // Resolve before acquiring a lock: even creating a child or lock in the
    // source would violate its immutability.
    storage::reject_overlapping_directories(&source_directory, &options.directory)?;
    for peer in &options.collection_artifacts {
        if peer.exists() {
            let peer_directory = if peer.is_dir() {
                peer.as_path()
            } else {
                peer.parent().unwrap_or(Path::new("."))
            };
            storage::reject_overlapping_directories(
                &peer_directory.canonicalize().map_err(io)?,
                &options.directory,
            )?;
        }
    }
    let _lock = storage::WriterLock::acquire(&options.directory)?;
    let source_report = master_reduction_inspect(source)?;
    if !matches!(
        source_report["status"].as_str(),
        Some("published_unrefined" | "completed_nonminimal")
    ) {
        return Err(AppError::input(
            "refinement source must be a published artifact",
        ));
    }
    let source_metadata = serde_json::to_vec(&source_report).map_err(io)?;
    let source_profile = profile::from_report(&source_report)?;
    let latest = options.directory.join("latest.json");
    let resumed = if options.resume {
        Some(read_json(&latest)?)
    } else {
        None
    };
    let normalization_profile = if let Some(report) = &resumed {
        profile::for_resume(options.normalization_profile, report)?
    } else {
        options.normalization_profile.unwrap_or(source_profile)
    };
    let finite_feedback = collection::feedback_for_phase(options, resumed.as_ref())?;
    let mut hash = blake3::Hasher::new();
    hash.update(b"rustred-published-refinement-v1");
    hash.update(&source_metadata);
    hash.update(&options.seed_depth.to_le_bytes());
    if options.containing_sector_depth != 0 {
        hash.update(b"containing-sector-seeds-v1");
        hash.update(&options.containing_sector_depth.to_le_bytes());
    }
    if options.saved_rule_assistance {
        hash.update(b"saved-rule-assistance-v1");
    }
    if options.circuit_symmetry_assistance {
        hash.update(b"circuit-symmetry-assistance-v1");
    }
    normalization_profile.hash(&mut hash);
    let collection_identity = collection::input_identity(options, resumed.as_ref())?;
    hash.update(collection::STRATEGY.as_bytes());
    if finite_feedback {
        hash.update(collection::FEEDBACK_RECIPE.as_bytes());
    }
    hash.update(&serde_json::to_vec(&collection_identity).map_err(io)?);
    let binding = hash.finalize().to_hex().to_string();
    let started = Instant::now();
    let (mut report, mut session) = if let Some(report) = resumed {
        if report["schema"] != SCHEMA || report["refinement_binding"] != binding {
            return Err(AppError::input(
                "refinement checkpoint has a different source or requested seed depth",
            ));
        }
        let session = load_master_relation_session(&options.directory)?;
        assistance::validate(options, &session, &report)?;
        (report, session)
    } else {
        if latest.exists() {
            return Err(AppError::input(
                "refinement checkpoint exists; use --resume",
            ));
        }
        let mut session = load_master_relation_session(source)?;
        let changes_profile = normalization_profile != source_profile;
        if changes_profile {
            session = TerminalRelationSession::new(
                session.family_owner().clone(),
                session.raw_terminals().clone(),
                options.seed_depth.max(session.statistics().seed_depth),
                normalization_profile.relation_limits(),
            )
            .map_err(io)?;
        } else if options.seed_depth > session.statistics().seed_depth {
            session
                .extend(&BTreeSet::new(), options.seed_depth)
                .map_err(io)?;
        }
        storage::clone_inputs(&source_directory, &options.directory, &source_report)?;
        let mut report = source_report.clone();
        report["schema"] = json!(SCHEMA);
        report
            .as_object_mut()
            .ok_or_else(|| AppError::input("artifact report is not an object"))?
            .remove("artifact");
        report["checkpoint"] = json!({"generation":0});
        report["native_state"] = Value::Null;
        report["refinement_binding"] = json!(binding);
        report["collection_input_identity"] = collection_identity;
        report["collection_requested_paths"] = json!(options.collection_artifacts);
        report["source_artifact"] = json!({
            "manifest_blake3":blake3::hash(&source_metadata).to_hex().to_string(),
            "native_state_blake3":source_report["native_state"]["blake3"],
            "scope_binding":source_report["scope_binding"]
        });
        report["operation"] = json!("refine");
        collection::record_feedback(&mut report, finite_feedback);
        report["seed_depth"] = json!(session.statistics().seed_depth);
        profile::record(&mut report, normalization_profile);
        report["finite_search_restarted_for_normalization_profile"] = json!(changes_profile);
        collection::inherit(source, &options.directory, &mut report)?;
        collection::add_members(options, &session, &mut report)?;
        assistance::configure(options, &mut session, &mut report)?;
        save(
            options,
            &session,
            &mut report,
            "running",
            started,
            &observer,
        )?;
        (report, session)
    };
    if report["status"] == "completed_nonminimal" && report["collection"]["status"] == "completed" {
        let _ = load_master_reduction(&options.directory)?;
        if !session.is_complete() {
            return Err(AppError::input(
                "completed refinement manifest has unfinished native state",
            ));
        }
        publish_final(options, &mut report)?;
        observer(event(&report, "master_reduction_finished", started));
        return Ok(report);
    }
    report["status"] = json!("running");
    report.as_object_mut().unwrap().remove("last_error");
    write_json(&latest, &report)?;
    observer(event(&report, "master_reduction_progress", started));
    match execute_session(
        options,
        &mut session,
        &mut report,
        cancellation,
        &observer,
        started,
    ) {
        Ok(()) => Ok(report),
        Err(error) => {
            let mut durable = read_json(&latest)?;
            durable["status"] = json!(if cancellation.load(Ordering::Relaxed) {
                "paused"
            } else {
                "failed"
            });
            durable["last_error"] = json!(error.to_string());
            write_json(&latest, &durable)?;
            observer(event(&durable, "master_reduction_checkpoint", started));
            if cancellation.load(Ordering::Relaxed) {
                Ok(durable)
            } else {
                Err(error)
            }
        }
    }
}
