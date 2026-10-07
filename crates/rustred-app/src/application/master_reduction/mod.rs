//! Optional finite, resumable master-candidate reduction after a saved walk.
//! CP6 coverage and finite IBP row-span identities remain distinct authorities.
mod assistance;
mod refine;
mod storage;
#[cfg(test)]
mod tests;

use crate::application::atomic_file::write_file_atomically;
use crate::{
    AppError, OwnerDomainWalkInventoryOptions, OwnerDomainWalkRequest, owner_domain_walk_inventory,
};
use rustred::persistence::BinaryIoLimits;
use rustred::reduction::terminal_relations::{TerminalRelationLimits, TerminalRelationSession};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use storage::{io, read_json, write_json};

pub use refine::master_refine_published_artifact;

const SCHEMA: &str = "rustred.master-reduction.v1";

/// Publication packages existing knowledge without searching for new IBPs.
/// Refinement explicitly drains a bounded ordinary-source search.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MasterReductionOperation {
    Publish,
    #[default]
    Refine,
}

#[derive(Clone, Debug)]
pub struct MasterReductionOptions {
    pub checkpoint: PathBuf,
    pub directory: PathBuf,
    pub previous_artifact: Option<PathBuf>,
    pub resume: bool,
    pub threads: usize,
    pub seed_depth: u32,
    /// Maximum number of nonpositive slots promoted to +1 in an additional
    /// finite ordinary-IBP seed set. These seeds are not new terminals.
    pub containing_sector_depth: u32,
    /// Incorporate applicable saved candidate equations at finite source support.
    /// This does not upgrade their original-source provenance authority.
    pub saved_rule_assistance: bool,
    /// Add verified circuit-symmetry equations for raw and normalized terminals.
    /// Does not load or certify saved candidate rules.
    pub circuit_symmetry_assistance: bool,
    pub checkpoint_interval: Duration,
    pub operation: MasterReductionOperation,
}

impl MasterReductionOptions {
    pub fn new(checkpoint: impl Into<PathBuf>, directory: impl Into<PathBuf>) -> Self {
        Self {
            checkpoint: checkpoint.into(),
            directory: directory.into(),
            previous_artifact: None,
            resume: false,
            threads: 1,
            seed_depth: 0,
            containing_sector_depth: 0,
            saved_rule_assistance: false,
            circuit_symmetry_assistance: false,
            checkpoint_interval: Duration::from_secs(3600),
            operation: MasterReductionOperation::Refine,
        }
    }
}

/// Metadata-only summary. Native coefficients are loaded only by the explicit
/// load function, not merely to render a table of counts.
pub fn master_reduction_inspect(path: &Path) -> Result<Value, AppError> {
    let manifest = if path.is_dir() {
        // A later paused attempt must never be hidden by an older publication.
        let latest = path.join("latest.json");
        if latest.exists() {
            latest
        } else {
            path.join("artifact.json")
        }
    } else {
        path.to_path_buf()
    };
    let report = read_json(&manifest)?;
    if report["schema"] != SCHEMA {
        return Err(AppError::input("not a master-reduction package"));
    }
    let parent = manifest.parent().unwrap_or(Path::new("."));
    if let Some(name) = report["native_state"]["file"].as_str() {
        let state = storage::safe_child(parent, name)?;
        if std::fs::metadata(state).map_err(io)?.len()
            != report["native_state"]["bytes"].as_u64().unwrap_or(u64::MAX)
        {
            return Err(AppError::input(
                "native master state size differs from manifest",
            ));
        }
    }
    Ok(report)
}

/// Cold-load the self-contained native finite relations. This validates its
/// digest and Symbolica/native structure; it does not certify saved owner rules.
pub fn load_master_reduction(path: &Path) -> Result<TerminalRelationSession, AppError> {
    let report = master_reduction_inspect(path)?;
    let directory = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(Path::new("."))
    };
    let name = report["native_state"]["file"]
        .as_str()
        .ok_or_else(|| AppError::input("master reduction has not yet prepared native state"))?;
    let state = storage::safe_child(directory, name)?;
    let limits = BinaryIoLimits::default();
    let bytes = storage::read_bounded(&state, limits.max_program_bytes)?;
    if Some(blake3::hash(&bytes).to_hex().as_str()) != report["native_state"]["blake3"].as_str() {
        return Err(AppError::input("native master state digest mismatch"));
    }
    TerminalRelationSession::from_native_bytes(&bytes, TerminalRelationLimits::default(), limits)
        .map_err(io)
}

pub fn master_reduce_saved_campaign(
    request: &OwnerDomainWalkRequest,
    options: &MasterReductionOptions,
    cancellation: &AtomicBool,
    observer: impl Fn(Value),
) -> Result<Value, AppError> {
    if options.threads == 0 || options.checkpoint_interval.is_zero() {
        return Err(AppError::input(
            "threads and checkpoint interval must be positive",
        ));
    }
    if (options.saved_rule_assistance
        || options.circuit_symmetry_assistance
        || options.containing_sector_depth != 0)
        && options.operation != MasterReductionOperation::Refine
    {
        return Err(AppError::input(
            "additional terminal-search strategies require explicit refinement",
        ));
    }
    let _lock = storage::WriterLock::acquire(&options.directory)?;
    let started = Instant::now();
    let binding = storage::scope_binding(
        request,
        &options.checkpoint,
        options.seed_depth,
        options.containing_sector_depth,
        options.circuit_symmetry_assistance,
    )?;
    let manifest_path = options.directory.join("latest.json");
    let mut report = if options.resume {
        let report = read_json(&manifest_path)?;
        if report["schema"] != SCHEMA || report["scope_binding"] != binding {
            return Err(AppError::input(
                "master checkpoint belongs to a different scope or seed depth; start a new phase directory",
            ));
        }
        if report["operation"].as_str().unwrap_or("refine") != operation_name(options.operation)
            || report["saved_rule_assistance"].as_bool().unwrap_or(false)
                != options.saved_rule_assistance
            || report["circuit_symmetry_assistance"]
                .as_bool()
                .unwrap_or(false)
                != options.circuit_symmetry_assistance
        {
            return Err(AppError::input(
                "cannot change a saved phase operation in place; refine a published artifact into a new directory",
            ));
        }
        report
    } else {
        if manifest_path.exists() {
            return Err(AppError::input("master checkpoint exists; use --resume"));
        }
        json!({"schema":SCHEMA,"phase":"Master reduction","stage":"inventory","status":"running",
            "scope_binding":binding,"seed_depth":options.seed_depth,"checkpoint":{"generation":0},
            "scope":{"queries":"inputs/queries.json","amendments":"inputs/amendments.json"},
            "raw_terminals":null,"normalized_terminals":null,"remaining_terminals":null,
            "relation_rows":0,"eliminated_terminals":0,"completed_work":0,"total_work":null,
            "minimality_claim":false,"family_closure_claim":false,"global_termination_claim":false,
            "completion_policy":"finite signed-L1 ordinary-IBP search; retain unresolved auxiliaries",
            "application":"exact terminal substitutions; routed saved-owner coefficient back-substitution is a separate interface",
            "numerical_master_values_included":false})
    };
    if report["status"] == "completed_nonminimal"
        || (options.operation == MasterReductionOperation::Publish
            && report["status"] == "published_unrefined")
    {
        let session = load_master_reduction(&options.directory)?;
        assistance::validate(options, &session, &report)?;
        if report["status"] == "completed_nonminimal" && !session.is_complete() {
            return Err(AppError::input(
                "completed manifest has unfinished native session",
            ));
        }
        publish_final(options, &mut report)?;
        observer(event(&report, "master_reduction_finished", started));
        return Ok(report);
    }
    report["operation"] = json!(operation_name(options.operation));
    report["saved_rule_assistance"] = json!(options.saved_rule_assistance);
    report["circuit_symmetry_assistance"] = json!(options.circuit_symmetry_assistance);
    report["status"] = json!("running");
    report["scope"] = storage::scope_summary(request)?;
    write_json(&manifest_path, &report)?;
    observer(event(&report, "master_reduction_progress", started));
    let result = run(
        request,
        options,
        cancellation,
        &observer,
        started,
        &mut report,
    );
    match result {
        Ok(()) => Ok(report),
        Err(error) => {
            // The most recent native checkpoint remains authoritative. Never
            // replace its cursor with partially completed/in-flight work.
            let mut saved = read_json(&manifest_path)?;
            saved["status"] = json!(if cancellation.load(Ordering::Relaxed) {
                "paused"
            } else {
                "failed"
            });
            saved["last_error"] = json!(error.to_string());
            write_json(&manifest_path, &saved)?;
            observer(event(&saved, "master_reduction_checkpoint", started));
            if cancellation.load(Ordering::Relaxed) {
                Ok(saved)
            } else {
                Err(error)
            }
        }
    }
}

fn run(
    request: &OwnerDomainWalkRequest,
    options: &MasterReductionOptions,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
    started: Instant,
    report: &mut Value,
) -> Result<(), AppError> {
    let mut session = if report["native_state"].is_object() {
        load_master_reduction(&options.directory)?
    } else {
        report["stage"] = json!("inventory");
        let mut inventory_options = OwnerDomainWalkInventoryOptions::new(&options.checkpoint);
        inventory_options.verification.threads = options.threads;
        let inventory =
            owner_domain_walk_inventory(request, &inventory_options, cancel, |native| {
                let mut output = event(report, "master_reduction_progress", started);
                output["inventory_event"] = native;
                observer(output);
            })?;
        let summary = inventory.summary();
        if summary["complete"] != true {
            return Err(AppError::input(
                "master reduction requires full validated saved-scope inventory",
            ));
        }
        report["inventory"] = summary;
        report["raw_terminals"] = json!(inventory.terminal_keys().len());
        if cancel.load(Ordering::Relaxed) {
            return Err(AppError::execution(
                "paused before native master preparation",
            ));
        }
        report["stage"] = json!("normalize");
        observer(event(report, "master_reduction_progress", started));
        let digests: Vec<String> = serde_json::from_value(
            report["inventory"]["verification"]["checkpoint"]["prepared_owner_payload_blake3"]
                .clone(),
        )
        .map_err(|_| AppError::input("cold verifier did not return prepared owner identities"))?;
        report["inputs"] = storage::package_inputs(request, &options.directory, &digests)?;
        if storage::scope_binding(
            request,
            &options.checkpoint,
            options.seed_depth,
            options.containing_sector_depth,
            options.circuit_symmetry_assistance,
        )? != report["scope_binding"]
        {
            return Err(AppError::input(
                "saved scope changed while preparing master reduction",
            ));
        }
        let mut session = if let Some(previous) = &options.previous_artifact {
            let previous_report = master_reduction_inspect(previous)?;
            if !matches!(
                previous_report["status"].as_str(),
                Some("completed_nonminimal" | "published_unrefined")
            ) || previous_report["inputs"]["program_binding"]
                != report["inputs"]["program_binding"]
            {
                return Err(AppError::input(
                    "previous stage is unpublished or has different saved rules/routes",
                ));
            }
            let mut previous_session = load_master_reduction(previous)?;
            if previous_session.family_owner().fingerprint()
                != inventory.family_owner().fingerprint()
            {
                return Err(AppError::input("previous stage family mismatch"));
            }
            previous_session
                .extend(
                    inventory.terminal_keys(),
                    options
                        .seed_depth
                        .max(previous_session.statistics().seed_depth),
                )
                .map_err(io)?;
            report["reused_previous_stage"] = json!(previous_report["scope_binding"]);
            report["containing_sector_depth"] = json!(
                previous_report["containing_sector_depth"]
                    .as_u64()
                    .unwrap_or(0)
            );
            previous_session
        } else {
            TerminalRelationSession::new(
                inventory.family_owner().clone(),
                inventory.terminal_keys().clone(),
                options.seed_depth,
                TerminalRelationLimits::default(),
            )
            .map_err(io)?
        };
        assistance::configure(options, &mut session, report)?;
        save(options, &session, report, "running", started, observer)?;
        session
    };
    execute_session(options, &mut session, report, cancel, observer, started)
}

fn execute_session(
    options: &MasterReductionOptions,
    session: &mut TerminalRelationSession,
    report: &mut Value,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
    started: Instant,
) -> Result<(), AppError> {
    let mut provider = assistance::prepare(options, session, report, cancel, observer, started)?;
    let mut last_checkpoint = Instant::now();
    let mut last_event = Instant::now();
    while !session.is_complete() && !cancel.load(Ordering::Relaxed) {
        // Scope extension may need to reindex old exact rows. This preserves
        // previous refinements, but publication never processes a new source.
        if options.operation == MasterReductionOperation::Publish
            && session.statistics().pending_rebuild_rows == 0
        {
            break;
        }
        if let Some(provider) = &mut provider {
            session.step_with_provider(cancel, provider).map_err(io)?;
        } else {
            session.step(cancel).map_err(io)?;
        }
        if last_event.elapsed() >= Duration::from_millis(250) {
            update_stats(report, &session);
            observer(event(report, "master_reduction_progress", started));
            last_event = Instant::now();
        }
        if last_checkpoint.elapsed() >= options.checkpoint_interval {
            save(options, &session, report, "running", started, observer)?;
            last_checkpoint = Instant::now();
        }
    }
    if cancel.load(Ordering::Relaxed) {
        save(options, &session, report, "paused", started, observer)?;
        return Ok(());
    }
    save(
        options,
        &session,
        report,
        if session.is_complete() {
            "completed_nonminimal"
        } else {
            "published_unrefined"
        },
        started,
        observer,
    )?;
    publish_final(options, report)?;
    observer(event(report, "master_reduction_finished", started));
    Ok(())
}

fn publish_final(options: &MasterReductionOptions, report: &mut Value) -> Result<(), AppError> {
    report["artifact"] = json!(options.directory);
    write_json(&options.directory.join("artifact.json"), report)?;
    write_json(&options.directory.join("latest.json"), report)
}

fn operation_name(operation: MasterReductionOperation) -> &'static str {
    match operation {
        MasterReductionOperation::Publish => "publish",
        MasterReductionOperation::Refine => "refine",
    }
}

fn update_stats(report: &mut Value, session: &TerminalRelationSession) {
    // The core owns all algebraic counts and work cursors; this is only their
    // presentation/transport boundary, shared by CLI and notebook consumers.
    let stats = session.statistics();
    report["stage"] = json!(if stats.pending_assistance_keys > 0
        || stats.pending_assistance_rows > 0
    {
        "assisted_equations"
    } else if stats.pending_rebuild_rows > 0 {
        "elimination"
    } else {
        "relations"
    });
    let initially_normalized = session.normalization().canonical_terminals().len();
    report["normalized_terminals"] = json!(initially_normalized);
    report["algebraic_basis_terminals"] = json!(stats.normalized_terminals);
    report["relation_rows"] = json!(stats.completed_source_rows);
    let assistance_done = stats.completed_assistance_rows + stats.completed_assistance_keys as u64;
    report["completed_work"] =
        json!(stats.completed_source_rows + stats.completed_rebuild_rows + assistance_done);
    report["total_work"] = json!(
        stats.total_source_rows
            + stats.completed_rebuild_rows
            + stats.pending_rebuild_rows as u64
            + assistance_done
            + stats.pending_assistance_keys as u64
            + stats.pending_assistance_rows as u64
    );
    report["independent_rows"] = json!(stats.independent_rows);
    report["columns"] = json!(stats.columns);
    report["auxiliary_columns"] = json!(stats.auxiliary_columns);
    report["nonzeros"] = json!(stats.nonzeros);
    report["completed_seeds"] = json!(stats.completed_seeds);
    report["seeds"] = json!(stats.seeds);
    report["raw_terminals"] = json!(session.raw_terminals().len());
    report["remaining_terminals"] = json!(stats.remaining_terminals);
    report["eliminated_terminals"] =
        json!(initially_normalized.saturating_sub(stats.remaining_terminals));
    report["terminal_relations"] = json!(stats.terminal_relations);
    report["seed_depth"] = json!(stats.seed_depth);
    report["completed_assistance_keys"] = json!(stats.completed_assistance_keys);
    report["pending_assistance_keys"] = json!(stats.pending_assistance_keys);
    report["completed_assistance_rows"] = json!(stats.completed_assistance_rows);
    report["pending_assistance_rows"] = json!(stats.pending_assistance_rows);
    report["refinement_complete"] = json!(stats.complete);
    report["capabilities"] = json!({
        "saved_scope_coverage_verified": report["inventory"]["complete"] == true,
        "native_terminal_state": true,
        "exact_current_terminal_substitutions": stats.pending_rebuild_rows == 0,
        "routed_coefficient_application": false,
        "numerical_master_values": false,
        "minimality_proved": false
    });
}

fn save(
    options: &MasterReductionOptions,
    session: &TerminalRelationSession,
    report: &mut Value,
    status: &str,
    started: Instant,
    observer: &impl Fn(Value),
) -> Result<(), AppError> {
    let save_start = Instant::now();
    let generation = report["checkpoint"]["generation"]
        .as_u64()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| AppError::limit("checkpoint generation overflow"))?;
    let bytes = session
        .to_native_bytes(BinaryIoLimits::default())
        .map_err(io)?;
    let file = format!("state-{generation:016}.rrbin");
    write_file_atomically(&options.directory.join(&file), &bytes, true).map_err(io)?;
    update_stats(report, session);
    report["native_state"] =
        json!({"file":file,"bytes":bytes.len(),"blake3":blake3::hash(&bytes).to_hex().to_string()});
    report["status"] = json!(status);
    if status == "published_unrefined" {
        report["stage"] = json!("published");
    }
    report["elapsed_seconds"] = json!(started.elapsed().as_secs_f64());
    report["checkpoint"] = json!({"generation":generation,"directory":options.directory,
        "state":"saved","bytes":bytes.len(),"duration_seconds":save_start.elapsed().as_secs_f64(),
        "completed_unix_seconds":SystemTime::now().duration_since(UNIX_EPOCH).map_err(io)?.as_secs()});
    write_json(&options.directory.join("latest.json"), report)?;
    observer(event(report, "master_reduction_checkpoint", started));
    Ok(())
}

fn event(report: &Value, kind: &str, started: Instant) -> Value {
    let mut result = Value::Object(
        report
            .as_object()
            .expect("report object")
            .iter()
            .filter(|(key, _)| !matches!(key.as_str(), "inventory" | "inputs"))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    );
    result["event"] = json!(kind);
    result["elapsed_seconds"] = json!(started.elapsed().as_secs_f64());
    result
}
