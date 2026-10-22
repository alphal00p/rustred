//! Publish only the exact work already retained by a stopped refinement.
use super::*;

/// Export a paused or failed finite refinement to a distinct portable artifact.
/// The source's durable native cursor is preserved. Only pending row reindexing
/// and rebinding previously proved collection maps may run; this never searches
/// for a new source row, assisted equation, alias, or collection relation.
///
/// `directory`, `resume`, `threads`, and `checkpoint_interval` select the output
/// phase. The operation must be `Publish`; search strategy changes are rejected.
pub fn master_publish_partial_refinement(
    source: &Path,
    options: &MasterReductionOptions,
    cancellation: &AtomicBool,
    observer: impl Fn(Value),
) -> Result<Value, AppError> {
    publish_partial(source, options, cancellation, observer, false)
}

/// Recover a durably checkpointed refinement left with status `running` after
/// abrupt process death, and export its exact knowledge to a distinct artifact.
///
/// The existing source writer lock must be exclusively available. No source
/// file, cursor or status is rewritten. In-flight (not checkpointed) work is
/// absent by definition; no new ordinary row or modular probe is evaluated.
pub fn master_recover_interrupted_refinement(
    source: &Path,
    options: &MasterReductionOptions,
    cancellation: &AtomicBool,
    observer: impl Fn(Value),
) -> Result<Value, AppError> {
    publish_partial(source, options, cancellation, observer, true)
}

enum SourceLock {
    Ordinary { _lock: storage::WriterLock },
    Recovery { _file: std::fs::File },
}

fn publish_partial(
    source: &Path,
    options: &MasterReductionOptions,
    cancellation: &AtomicBool,
    observer: impl Fn(Value),
    recover_interrupted: bool,
) -> Result<Value, AppError> {
    if options.operation != MasterReductionOperation::Publish
        || options.deep_verification.is_some()
        || options.previous_artifact.is_some()
        || options.fresh_search
        || !options.collection_artifacts.is_empty()
        || options.saved_rule_assistance
        || options.circuit_symmetry_assistance
        || options.numerator_symmetry_assistance
        || options.sector_symmetry_assistance
        || options.sector_symmetry_policy.is_some()
        || options.sector_symmetry_saved_projection.is_some()
        || options.containing_sector_depth != 0
        || options.seed_depth != 0
        || options.finite_feedback == Some(true)
        || options.checkpoint_interval.is_zero()
        || options.threads == 0
        || options.refinement_strategy.is_some()
        || options.modular_policy.is_some()
    {
        return Err(AppError::input(
            "partial refinement publication requires Publish, positive limits and no new search or verification strategy",
        ));
    }
    let source_directory = if source.is_dir() {
        source
    } else {
        source.parent().unwrap_or(Path::new("."))
    }
    .canonicalize()
    .map_err(io)?;
    storage::reject_overlapping_directories(&source_directory, &options.directory)?;
    // Acquire the source lock before capturing its manifest/native state. A
    // paused-looking manifest is not sufficient while another writer owns it.
    let _source_lock = if recover_interrupted {
        // Recovery must not create a new lock namespace or even a missing
        // source marker. The same existing file belongs to normal writers.
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(source_directory.join("writer.lock"))
            .map_err(|error| {
                AppError::input(format!(
                    "interrupted recovery requires the existing refinement writer lock: {error}"
                ))
            })?;
        file.try_lock().map_err(|error| {
            AppError::input(format!(
                "cannot recover a refinement while its writer lock is held: {error}"
            ))
        })?;
        SourceLock::Recovery { _file: file }
    } else {
        SourceLock::Ordinary {
            _lock: storage::WriterLock::acquire(&source_directory)?,
        }
    };
    let source_report = inspect_with_policy(source, options.finite_policy.as_ref())?;
    let valid_status = if recover_interrupted {
        source_report["status"] == "running"
    } else {
        matches!(source_report["status"].as_str(), Some("paused" | "failed"))
    };
    if !valid_status
        || source_report["operation"].as_str().unwrap_or("refine") != "refine"
        || source_report["inventory"]["complete"] != true
    {
        return Err(AppError::input(if recover_interrupted {
            "interrupted recovery requires status running and a complete saved-scope refinement inventory"
        } else {
            "partial publication requires a paused or failed refinement with a complete saved-scope inventory; stop an active refinement first"
        }));
    }
    // Authenticate the durable exact cursor before creating any recovery
    // output. Loading replays existing structural/provenance checks only.
    let recovered_session = if recover_interrupted {
        if !source_report["native_state"].is_object()
            || source_report["native_state"]["bytes"]
                .as_u64()
                .is_none_or(|size| size == 0)
            || source_report["checkpoint"]["generation"]
                .as_u64()
                .is_none_or(|generation| generation == 0)
        {
            return Err(AppError::input(
                "interrupted refinement has no complete durable native checkpoint to recover",
            ));
        }
        Some(load_session_with_policy(
            source,
            options.finite_policy.as_ref(),
        )?)
    } else {
        None
    };
    profile::for_resume(options.normalization_profile, &source_report)?;
    let source_metadata = serde_json::to_vec(&source_report).map_err(io)?;
    let source_digest = blake3::hash(&source_metadata).to_hex().to_string();
    let mut hash = blake3::Hasher::new();
    hash.update(b"rustred-partial-refinement-publication-v1");
    if recover_interrupted {
        hash.update(b"exclusive-interrupted-refinement-recovery-v1");
    }
    hash.update(source_digest.as_bytes());
    let binding = hash.finalize().to_hex().to_string();
    let _output_lock = storage::WriterLock::acquire(&options.directory)?;
    let latest = options.directory.join("latest.json");
    let started = Instant::now();
    let (mut report, mut session) = if options.resume {
        drop(recovered_session);
        let report = storage::read_json_with_policy(&latest, options.finite_policy.as_ref())?;
        if (report["schema"] != SCHEMA && report["schema"] != FINITE_SCHEMA)
            || report["operation"] != "publish"
            || report["partial_refinement_binding"] != binding
        {
            return Err(AppError::input(
                "partial publication checkpoint belongs to a different refinement snapshot",
            ));
        }
        let session = load_session_with_policy(&options.directory, options.finite_policy.as_ref())?;
        assistance::validate(options, &session, &report)?;
        (report, session)
    } else {
        if latest.exists() || options.directory.join("artifact.json").exists() {
            return Err(AppError::input(
                "partial publication output already exists; use --resume or a new directory",
            ));
        }
        let mut session = if let Some(session) = recovered_session {
            session
        } else {
            load_session_with_policy(source, options.finite_policy.as_ref())?
        };
        let statistics = session.statistics();
        if statistics.completed_seeds == statistics.seeds
            && statistics.pending_assistance_keys == 0
            && statistics.pending_assistance_rows == 0
        {
            return Err(AppError::input(if recover_interrupted {
                "interrupted recovery of a completed finite source search with unfinished collection is not supported; use its normal collection-resume path"
            } else {
                "finite source search is already complete; resume refinement to finish its collection"
            }));
        }
        let mut report = source_report.clone();
        modular::inherit_state(&source_directory, &options.directory, &report)?;
        inherited::capture(&session, &source_report, &mut report)?;
        storage::clone_inputs_with_policy(
            &source_directory,
            &options.directory,
            &source_report,
            options.finite_policy.as_ref(),
        )?;
        census_cache::inherit_with_cancel(
            &source_directory,
            &options.directory,
            &mut report,
            cancellation,
            options.finite_policy.as_ref(),
        )?;
        collection::inherit_with_policy(
            source,
            &options.directory,
            &mut report,
            options.finite_policy.as_ref(),
        )?;
        let fields = report.as_object_mut().unwrap();
        for field in ["artifact", "last_error", "refinement_binding"] {
            fields.remove(field);
        }
        report["schema"] = json!(current_schema(&report)?);
        report["operation"] = json!("publish");
        report["checkpoint"] = json!({"generation":0});
        report["native_state"] = Value::Null;
        report["partial_refinement_binding"] = json!(binding);
        report["partial_refinement_source"] = json!({
            "recipe":"durable-partial-refinement-v1",
            "manifest_blake3":source_digest,
            "native_state":source_report["native_state"],
            "status":source_report["status"],
            "checkpoint":source_report["checkpoint"],
            "scope_binding":source_report["scope_binding"],
            "refinement_binding":source_report["refinement_binding"],
            "last_error":source_report["last_error"],
            "completed_source_rows":session.statistics().completed_source_rows,
            "completed_seeds":session.statistics().completed_seeds,
            "seeds":session.statistics().seeds,
            "seed_depth":session.statistics().seed_depth,
            "finite_search_complete":false,
            "refinement_complete":false,
            "meaning":"only durably checkpointed exact rows and previously proved collection maps; unfinished finite search retained for later explicit refinement"
        });
        if recover_interrupted {
            report["interrupted_refinement_recovery"] = json!({
                "recipe": "exclusive-interrupted-refinement-recovery-v1",
                "source_manifest_blake3": source_digest,
                "source_native_state": source_report["native_state"],
                "source_checkpoint": source_report["checkpoint"],
                "source_status": "running",
                "exclusive_existing_writer_lock": true,
                "source_files_unchanged": true,
                "in_flight_work": "not present in the durable snapshot; not recovered",
                "new_ordinary_source_rows": 0,
                "new_modular_discovery_or_reconstruction": false,
            });
        }
        collection::record_feedback(&mut report, false);
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
    if report["status"] == "published_unrefined" && report["collection"]["status"] == "completed" {
        let _ = collection::load_with_policy(&options.directory, options.finite_policy.as_ref())?;
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
            let mut durable =
                storage::read_json_with_policy(&latest, options.finite_policy.as_ref())?;
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
