//! Optional finite, resumable master-candidate reduction after a saved walk.
//! CP6 coverage and finite IBP row-span identities remain distinct authorities.
mod assistance;
mod budget;
mod census_cache;
mod collection;
mod finite_provenance;
mod fresh;
mod inherited;
#[cfg(feature = "reconstruction")]
mod modular;
#[cfg(not(feature = "reconstruction"))]
#[path = "modular_unavailable.rs"]
mod modular;
mod numerator_symmetry;
mod partial;
mod profile;
mod refine;
mod report_counts;
mod sector_symmetry;
mod source;
mod storage;
mod terminal_import;
#[cfg(test)]
mod tests;
mod verification;

use crate::application::atomic_file::write_file_atomically;
use crate::{
    AppError, OwnerDomainWalkInventoryOptions, OwnerDomainWalkRequest, owner_domain_walk_inventory,
};
use rustred::persistence::BinaryIoLimits;
use rustred::reduction::terminal_relations::{
    TerminalRelationExecutionOptions, TerminalRelationExecutor, TerminalRelationLimits,
    TerminalRelationSession,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
#[cfg(test)]
use storage::read_json;
use storage::{io, write_json};

pub use collection::{
    MasterReductionArtifact, load_master_reduction, load_master_reduction_with_policy,
};
pub use partial::{master_publish_partial_refinement, master_recover_interrupted_refinement};
pub use profile::MasterNormalizationProfile;
pub use refine::master_refine_published_artifact;
pub use rustred::reduction::terminal_normalization::TerminalSectorSymmetryPolicy as MasterSectorSymmetryPolicy;
pub use terminal_import::master_import_proved_refinement;

const SCHEMA: &str = "rustred.master-reduction.v2";
const FINITE_SCHEMA: &str = "rustred.master-reduction.v3";
const LEGACY_SOURCE_SCHEMA: &str = "rustred.master-reduction.v1";

/// Publication packages existing knowledge without searching for new IBPs.
/// Refinement explicitly drains a bounded ordinary-source search.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MasterReductionOperation {
    Publish,
    #[default]
    Refine,
}

/// Discovery policy. Both strategies publish only exact terminal identities.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MasterRefinementStrategy {
    #[default]
    Exact,
    ModularCircuits,
}

#[derive(Clone, Debug)]
pub struct MasterReductionOptions {
    /// Caller-selected resource grants for all finite publication provenance
    /// copies and cold reads, including inherited publications. Never restored
    /// from untrusted artifact metadata and independent of the mathematical scope.
    pub finite_policy: Option<crate::FiniteDemandPolicy>,
    pub checkpoint: PathBuf,
    pub directory: PathBuf,
    pub previous_artifact: Option<PathBuf>,
    /// Additional immutable publications whose terminal inventories are
    /// collected with the primary family during explicit refinement.
    pub collection_artifacts: Vec<PathBuf>,
    pub resume: bool,
    /// Start a new relations-only round from a completed publication. Retained
    /// exact maps survive, but old source/search cursors and auxiliary rows do
    /// not. This is an immutable phase mode; resume does not reset it again.
    pub fresh_search: bool,
    pub threads: usize,
    /// Optional operational work-stealing window/block sizes, mutable on resume.
    pub relation_batch_rows: Option<usize>,
    pub relation_block_rows: Option<usize>,
    /// Omitted new phases use exact refinement; resume inherits its strategy.
    pub refinement_strategy: Option<MasterRefinementStrategy>,
    /// Strict JSON modular discovery/reconstruction grants. Omitted on resume
    /// inherits the checkpoint; coefficients and accepted maps remain native.
    pub modular_policy: Option<Value>,
    /// Independently replay the saved dependency graph while extracting the
    /// publication inventory. New phases default off; resume inherits its mode.
    /// Refining an existing artifact preserves its assurance without replay.
    pub deep_verification: Option<bool>,
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
    /// Add exact finite numerator identities from denominator-preserving
    /// integration-variable automorphisms. Independent of ordinary IBP search.
    pub numerator_symmetry_assistance: bool,
    /// Close verified sector momentum maps and apply existing saved programs
    /// to their complete finite identities. Includes the numerator provider.
    pub sector_symmetry_assistance: bool,
    /// New strong phases default to generator relations. Omission on resume
    /// preserves the saved policy; changing it requires a new phase.
    pub sector_symmetry_policy: Option<MasterSectorSymmetryPolicy>,
    /// Apply authenticated saved programs to symmetry offspring. New strong
    /// phases default to true; omission on resume preserves the saved mode.
    pub sector_symmetry_saved_projection: Option<bool>,
    /// Re-eliminate retained family-qualified finite rows after full-U aliases.
    /// New refinements default to enabled; omitted on resume uses the snapshot.
    /// Publication only transports previously proved maps.
    pub finite_feedback: Option<bool>,
    /// Omitted on a new refinement inherits its source; omitted on resume
    /// inherits the checkpoint. Explicit changes require a new phase.
    pub normalization_profile: Option<MasterNormalizationProfile>,
    /// Optional operational column cap for finite refinement. Omitted on resume
    /// inherits its snapshot; zero explicitly removes the cap. New phases have
    /// no application-level column cap and use the supervisor's RAM guard.
    pub max_relation_columns: Option<usize>,
    pub checkpoint_interval: Duration,
    pub operation: MasterReductionOperation,
}

impl MasterReductionOptions {
    pub fn new(checkpoint: impl Into<PathBuf>, directory: impl Into<PathBuf>) -> Self {
        Self {
            finite_policy: None,
            checkpoint: checkpoint.into(),
            directory: directory.into(),
            previous_artifact: None,
            collection_artifacts: Vec::new(),
            resume: false,
            fresh_search: false,
            threads: 1,
            relation_batch_rows: None,
            relation_block_rows: None,
            refinement_strategy: None,
            modular_policy: None,
            deep_verification: None,
            seed_depth: 0,
            containing_sector_depth: 0,
            saved_rule_assistance: false,
            circuit_symmetry_assistance: false,
            numerator_symmetry_assistance: false,
            sector_symmetry_assistance: false,
            sector_symmetry_policy: None,
            sector_symmetry_saved_projection: None,
            finite_feedback: None,
            normalization_profile: None,
            max_relation_columns: None,
            checkpoint_interval: Duration::from_secs(3600),
            operation: MasterReductionOperation::Refine,
        }
    }
}

/// Metadata-only summary. Native coefficients are loaded only by the explicit
/// load function, not merely to render a table of counts.
pub fn master_reduction_inspect(path: &Path) -> Result<Value, AppError> {
    inspect_with_policy(path, None)
}

/// Inspect with explicit caller grants, including the finite raw census.
pub fn master_reduction_inspect_with_policy(
    path: &Path,
    policy: &crate::FiniteDemandPolicy,
) -> Result<Value, AppError> {
    inspect_with_policy(path, Some(policy))
}

fn inspect_with_policy(
    path: &Path,
    policy: Option<&crate::FiniteDemandPolicy>,
) -> Result<Value, AppError> {
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
    let mut report = storage::read_json_with_policy(&manifest, policy)?;
    if report["schema"] != SCHEMA
        && report["schema"] != LEGACY_SOURCE_SCHEMA
        && report["schema"] != FINITE_SCHEMA
    {
        return Err(AppError::input("not a master-reduction package"));
    }
    finite_provenance::validate_marker(&report)?;
    // Legacy finite publications remain readable as refinement inputs. New
    // collection packages must not be relabelled as a format whose consumers
    // would silently apply only the pre-collection session.
    if report["schema"] == LEGACY_SOURCE_SCHEMA
        && (!report["collection_state"].is_null() || !report["collection"].is_null())
    {
        return Err(AppError::input(
            "terminal collections require master-reduction schema v2",
        ));
    }
    let parent = manifest.parent().unwrap_or(Path::new("."));
    finite_provenance::validate_required(parent, &report, policy)?;
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
    if report["collection"]["status"] == "completed" && !report["collection_state"].is_object() {
        return Err(AppError::input(
            "completed terminal collection has no native state",
        ));
    }
    if let Some(name) = report["collection_state"]["file"].as_str() {
        let state = storage::safe_child(parent, name)?;
        if std::fs::metadata(state).map_err(io)?.len()
            != report["collection_state"]["bytes"]
                .as_u64()
                .unwrap_or(u64::MAX)
        {
            return Err(AppError::input(
                "native collection state size differs from manifest",
            ));
        }
    }
    report_counts::update(&mut report);
    modular::validate_metadata(parent, &report)?;
    Ok(report)
}

/// Cold-load the self-contained native finite relations. This validates its
/// digest and Symbolica/native structure; it does not certify saved owner rules.
pub fn load_master_relation_session(path: &Path) -> Result<TerminalRelationSession, AppError> {
    load_session_with_policy(path, None)
}

/// Cold-load with explicit finite provenance resource grants.
pub fn load_master_relation_session_with_policy(
    path: &Path,
    policy: &crate::FiniteDemandPolicy,
) -> Result<TerminalRelationSession, AppError> {
    load_session_with_policy(path, Some(policy))
}

fn load_session_with_policy(
    path: &Path,
    policy: Option<&crate::FiniteDemandPolicy>,
) -> Result<TerminalRelationSession, AppError> {
    let report = inspect_with_policy(path, policy)?;
    let directory = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(Path::new("."))
    };
    census_cache::validate_required_with_policy(directory, &report, policy)?;
    let name = report["native_state"]["file"]
        .as_str()
        .ok_or_else(|| AppError::input("master reduction has not yet prepared native state"))?;
    let state = storage::safe_child(directory, name)?;
    let limits = profile::native_io_limits(None);
    let bytes = storage::read_bounded(&state, limits.max_program_bytes)?;
    if Some(blake3::hash(&bytes).to_hex().as_str()) != report["native_state"]["blake3"].as_str() {
        return Err(AppError::input("native master state digest mismatch"));
    }
    let session = TerminalRelationSession::from_native_bytes(
        &bytes,
        profile::from_report(&report)?.relation_limits(),
        limits,
    )
    .map_err(io)?;
    finite_provenance::validate_session(directory, &report, &session, policy)?;
    fresh::validate(directory, &report, &session)?;
    numerator_symmetry::validate(directory, &session, &report)?;
    sector_symmetry::validate(directory, &session, &report)?;
    if modular::from_report(&report)? == MasterRefinementStrategy::ModularCircuits
        && (report["modular_state"].is_object() || report["status"] == "completed_nonminimal")
    {
        let complete = modular::complete(directory, &report)?;
        if report["status"] == "completed_nonminimal" && !complete {
            return Err(AppError::input(
                "completed modular artifact has an unfinished native discovery state",
            ));
        }
    }
    Ok(session)
}

pub(crate) fn validate_required_source_with_policy(
    directory: &Path,
    report: &Value,
    policy: Option<&crate::FiniteDemandPolicy>,
) -> Result<(), AppError> {
    finite_provenance::validate_marker(report)?;
    census_cache::validate_required_with_policy(directory, report, policy)
}

fn current_schema(report: &Value) -> Result<&'static str, AppError> {
    finite_provenance::validate_marker(report)?;
    Ok(if report["schema"] == FINITE_SCHEMA {
        FINITE_SCHEMA
    } else {
        SCHEMA
    })
}

pub fn master_reduce_saved_campaign(
    request: &OwnerDomainWalkRequest,
    options: &MasterReductionOptions,
    cancellation: &AtomicBool,
    observer: impl Fn(Value),
) -> Result<Value, AppError> {
    master_reduce_source(
        source::SourceRequest::Walk(request),
        options,
        cancellation,
        observer,
    )
}

/// Private complete-generation adapter; no traversal command or new public CLI.
pub(crate) fn master_reduce_finite_demand(
    source: &crate::application::routed_campaign::finite_demand::backend::publication::CompletedFiniteSource,
    options: &MasterReductionOptions,
    cancellation: &AtomicBool,
    observer: impl Fn(Value),
) -> Result<Value, AppError> {
    master_reduce_source(
        source::SourceRequest::Finite(source),
        options,
        cancellation,
        observer,
    )
}

fn master_reduce_source(
    source: source::SourceRequest<'_>,
    options: &MasterReductionOptions,
    cancellation: &AtomicBool,
    observer: impl Fn(Value),
) -> Result<Value, AppError> {
    let request = source.request();
    if options.fresh_search {
        return Err(AppError::input(
            "relations-only fresh search requires refinement from a completed --artifact source",
        ));
    }
    if !options.collection_artifacts.is_empty() {
        return Err(AppError::input(
            "additional collection artifacts require refinement from a published artifact",
        ));
    }
    if options.threads == 0 || options.checkpoint_interval.is_zero() {
        return Err(AppError::input(
            "threads and checkpoint interval must be positive",
        ));
    }
    if (options.saved_rule_assistance
        || options.circuit_symmetry_assistance
        || options.numerator_symmetry_assistance
        || options.sector_symmetry_assistance
        || options.finite_feedback.is_some()
        || options.containing_sector_depth != 0
        || !options.collection_artifacts.is_empty())
        && options.operation != MasterReductionOperation::Refine
    {
        return Err(AppError::input(
            "additional terminal-search strategies require explicit refinement",
        ));
    }
    let _lock = storage::WriterLock::acquire(&options.directory)?;
    let started = Instant::now();
    let manifest_path = options.directory.join("latest.json");
    let resumed = if options.resume {
        Some(storage::read_json_with_policy(
            &manifest_path,
            options.finite_policy.as_ref(),
        )?)
    } else {
        None
    };
    let mut effective_options = options.clone();
    if options.sector_symmetry_assistance {
        effective_options.sector_symmetry_policy = Some(sector_symmetry::for_phase(
            options,
            resumed.as_ref().unwrap_or(&Value::Null),
        )?);
        effective_options.sector_symmetry_saved_projection =
            Some(sector_symmetry::projection_for_phase(
                options,
                resumed.as_ref().unwrap_or(&Value::Null),
            )?);
    } else if options.sector_symmetry_policy.is_some()
        || options.sector_symmetry_saved_projection.is_some()
    {
        return Err(AppError::input(
            "sector-symmetry policy requires sector-symmetry assistance",
        ));
    }
    let options = &effective_options;
    let normalization_profile = if let Some(report) = &resumed {
        profile::for_resume(options.normalization_profile, report)?
    } else if let Some(previous) = &options.previous_artifact {
        let inherited = profile::from_report(&inspect_with_policy(
            previous,
            options.finite_policy.as_ref(),
        )?)?;
        profile::for_import(options.normalization_profile, inherited, options.operation)?
    } else {
        options.normalization_profile.unwrap_or_default()
    };
    let finite_feedback = collection::feedback_for_phase(options, resumed.as_ref())?;
    let refinement_strategy = modular::for_phase(options, resumed.as_ref())?;
    let deep_verification = verification::for_phase(options, resumed.as_ref())?;
    let binding = strategy_scope_binding(
        source.binding(
            options,
            finite_feedback,
            deep_verification,
            normalization_profile,
        )?,
        refinement_strategy,
    );
    let mut report = if let Some(report) = resumed {
        if report["schema"] != source.schema() || report["scope_binding"] != binding {
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
            || report["numerator_symmetry_assistance"]
                .as_bool()
                .unwrap_or(false)
                != options.numerator_symmetry_assistance
            || report["sector_symmetry_assistance"]
                .as_bool()
                .unwrap_or(false)
                != options.sector_symmetry_assistance
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
        let mut report = json!({"schema":source.schema(),"phase":"Master reduction","stage":"inventory","status":"running",
            "scope_binding":binding,"seed_depth":options.seed_depth,"checkpoint":{"generation":0},
            "scope":{"queries":"inputs/queries.json","amendments":"inputs/amendments.json"},
            "raw_terminals":null,"normalized_terminals":null,"remaining_terminals":null,
            "relation_rows":0,"eliminated_terminals":0,"completed_work":0,"total_work":null,
            "minimality_claim":false,"family_closure_claim":false,"global_termination_claim":false,
            "completion_policy":"finite signed-L1 ordinary-IBP search; retain unresolved auxiliaries",
            "application":"exact terminal substitutions; routed saved-owner coefficient back-substitution is a separate interface",
            "numerical_master_values_included":false});
        profile::record(&mut report, normalization_profile);
        modular::record(&mut report, refinement_strategy, options)?;
        report
    };
    if (report["status"] == "completed_nonminimal"
        && report["collection"]["status"] == "completed"
        && !(refinement_strategy == MasterRefinementStrategy::ModularCircuits
            && options.modular_policy.is_some()))
        || (options.operation == MasterReductionOperation::Publish
            && report["status"] == "published_unrefined"
            && report["collection"]["status"] == "completed")
    {
        let session = load_session_with_policy(&options.directory, options.finite_policy.as_ref())?;
        let _ = collection::load_with_policy(&options.directory, options.finite_policy.as_ref())?;
        assistance::validate(options, &session, &report)?;
        if report["status"] == "completed_nonminimal"
            && !modular::phase_complete(&options.directory, &report, &session)?
        {
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
    report["numerator_symmetry_assistance"] = json!(options.numerator_symmetry_assistance);
    report["sector_symmetry_assistance"] = json!(options.sector_symmetry_assistance);
    if options.sector_symmetry_assistance {
        report["sector_symmetry_policy"] = json!(sector_symmetry::policy_name(
            options.sector_symmetry_policy.unwrap_or_default()
        ));
        report["sector_symmetry_saved_projection"] =
            json!(options.sector_symmetry_saved_projection.unwrap_or(true));
    }
    collection::record_feedback(&mut report, finite_feedback);
    verification::record(&mut report, deep_verification);
    report["status"] = json!("running");
    report["scope"] = storage::scope_summary(request)?;
    source.prepare_provenance(
        &options.directory,
        &mut report,
        cancellation,
        options.finite_policy.as_ref(),
    )?;
    if matches!(source, source::SourceRequest::Finite(_)) {
        observer(
            json!({"event":"finite_source_staged","generation":report["finite_source"]["generation"],
            "status":"complete_source_not_yet_referenced_by_latest"}),
        );
        if cancellation.load(Ordering::Acquire) {
            return Err(AppError::new(
                crate::AppErrorKind::Cancelled,
                "paused before finite source publication marker",
            ));
        }
    }
    write_json(&manifest_path, &report)?;
    observer(event(&report, "master_reduction_progress", started));
    let result = run(
        source,
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
            let mut saved =
                storage::read_json_with_policy(&manifest_path, options.finite_policy.as_ref())?;
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
    source: source::SourceRequest<'_>,
    options: &MasterReductionOptions,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
    started: Instant,
    report: &mut Value,
) -> Result<(), AppError> {
    let request = source.request();
    let mut session = if report["native_state"].is_object() {
        load_session_with_policy(&options.directory, options.finite_policy.as_ref())?
    } else {
        let deep_verification = verification::for_phase(options, Some(report))?;
        report["stage"] = json!(if deep_verification {
            "independent_graph_replay"
        } else {
            "inventory_census"
        });
        let mut inventory_options = OwnerDomainWalkInventoryOptions::new(&options.checkpoint);
        inventory_options.verification.threads = options.threads;
        inventory_options.deep_verification = deep_verification;
        if !deep_verification {
            inventory_options.previous_census = census_cache::current(&options.directory, report)
                .or_else(|| {
                    options
                        .previous_artifact
                        .as_deref()
                        .and_then(census_cache::previous)
                });
        }
        let inventory = match source {
            source::SourceRequest::Finite(captured) => source::SourceInventory::Finite(captured),
            source::SourceRequest::Walk(_) => source::SourceInventory::Walk(
                owner_domain_walk_inventory(request, &inventory_options, cancel, |native| {
                    let mut output = event(report, "master_reduction_progress", started);
                    if let Some(stage) = native["event"].as_str() {
                        if stage.starts_with("inventory_") || stage.starts_with("verify_") {
                            output["stage"] = json!(stage);
                        }
                    }
                    output["verification_progress"] = native.clone();
                    output["inventory_event"] = native;
                    observer(output);
                })?,
            ),
        };
        let summary = inventory.summary();
        if summary["complete"] != true {
            return Err(AppError::input(
                "master reduction requires a complete exact saved-scope inventory",
            ));
        }
        inventory.save(&options.directory, report, options.finite_policy.as_ref())?;
        report["inventory"] = summary;
        report_counts::update(report);
        report["independently_verified"] = json!(verification::independently_verified(report));
        report["publication_assurance"] = report["inventory"]["authority"].clone();
        report["raw_terminals"] = json!(inventory.terminal_keys().len());
        // Preserve completed census work even when cancellation occurs during
        // the subsequent native master preparation. This metadata is not a
        // published artifact or an independent closure certificate.
        write_json(&options.directory.join("latest.json"), report)?;
        if cancel.load(Ordering::Relaxed) {
            return Err(AppError::execution(
                "paused before native master preparation",
            ));
        }
        report["stage"] = json!("normalize");
        observer(event(report, "master_reduction_progress", started));
        let digests = inventory.payload_digests()?;
        if matches!(source, source::SourceRequest::Walk(_)) {
            report["inputs"] = storage::package_inputs(request, &options.directory, &digests)?;
        }
        let normalization_profile = profile::from_report(report)?;
        if strategy_scope_binding(
            source.binding(
                options,
                collection::feedback_for_phase(options, Some(report))?,
                deep_verification,
                normalization_profile,
            )?,
            modular::from_report(report)?,
        ) != report["scope_binding"]
        {
            return Err(AppError::input(
                "saved scope changed while preparing master reduction",
            ));
        }
        let mut session = if let Some(previous) = &options.previous_artifact {
            let previous_report = inspect_with_policy(previous, options.finite_policy.as_ref())?;
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
            let mut previous_session =
                load_session_with_policy(previous, options.finite_policy.as_ref())?;
            finite_provenance::inherit_previous(
                previous,
                &options.directory,
                &previous_report,
                report,
                cancel,
                options.finite_policy.as_ref(),
            )?;
            if previous_report["finite_search_complete"] == false {
                if let Some(provenance) = previous_report.get("partial_refinement_source") {
                    report["partial_refinement_source"] = provenance.clone();
                }
            }
            collection::inherit_with_policy(
                previous,
                &options.directory,
                report,
                options.finite_policy.as_ref(),
            )?;
            if previous_session.family_owner().fingerprint()
                != inventory.family_owner().fingerprint()
            {
                return Err(AppError::input("previous stage family mismatch"));
            }
            let depth = options
                .seed_depth
                .max(previous_session.statistics().seed_depth);
            let changes_profile = normalization_profile != profile::from_report(&previous_report)?;
            if changes_profile && previous_session.retained_relations_binding().is_some() {
                return Err(AppError::input(
                    "changing normalization on retained relations requires a relations-only fresh refinement first",
                ));
            }
            if !changes_profile && options.operation == MasterReductionOperation::Publish {
                inherited::capture(&previous_session, &previous_report, report)?;
            }
            if changes_profile {
                let raw = previous_session
                    .raw_terminals()
                    .union(inventory.terminal_keys())
                    .cloned()
                    .collect();
                previous_session = TerminalRelationSession::new(
                    previous_session.family_owner().clone(),
                    raw,
                    depth,
                    normalization_profile.relation_limits(),
                )
                .map_err(io)?;
                report["previous_stage_source"] = json!(previous_report["scope_binding"]);
            } else {
                extend_previous_inventory(&mut previous_session, inventory.terminal_keys(), depth)?;
                report["reused_previous_stage"] = json!(previous_report["scope_binding"]);
            }
            report["finite_search_restarted_for_normalization_profile"] = json!(changes_profile);
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
                normalization_profile.relation_limits(),
            )
            .map_err(io)?
        };
        collection::add_members(options, &session, report)?;
        assistance::configure(options, &mut session, report)?;
        save(options, &session, report, "running", started, observer)?;
        session
    };
    execute_session(options, &mut session, report, cancel, observer, started)
}

fn extend_previous_inventory(
    session: &mut TerminalRelationSession,
    inventory: &std::collections::BTreeSet<rustred::family::IntegralKey>,
    seed_depth: u32,
) -> Result<(), AppError> {
    // An assurance-only republication must preserve the finite cursor exactly.
    // Even an empty core extension can schedule a row-space rebuild, so only
    // invoke it when the raw inventory or finite seed depth actually changes.
    if inventory != session.raw_terminals() || seed_depth != session.statistics().seed_depth {
        session.extend(inventory, seed_depth).map_err(io)?;
    }
    Ok(())
}

fn execute_session(
    options: &MasterReductionOptions,
    session: &mut TerminalRelationSession,
    report: &mut Value,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
    started: Instant,
) -> Result<(), AppError> {
    if options.operation == MasterReductionOperation::Refine
        && modular::from_report(report)? == MasterRefinementStrategy::ModularCircuits
    {
        return modular::execute(options, session, report, cancel, observer, started);
    }
    budget::configure(options, session, report)?;
    let mut provider =
        assistance::prepare_for_execution(options, session, report, cancel, observer, started)?;
    // Changing an equation-provider policy can replace the native session.
    budget::configure(options, session, report)?;
    write_json(&options.directory.join("latest.json"), report)?;
    let mut last_checkpoint = Instant::now();
    let mut last_event = Instant::now();
    let executor = if options.operation == MasterReductionOperation::Refine {
        let mut execution = TerminalRelationExecutionOptions::new(options.threads);
        if let Some(rows) = options.relation_batch_rows {
            execution.batch_rows = rows;
        }
        if let Some(rows) = options.relation_block_rows {
            execution.block_rows = rows;
        }
        Some(TerminalRelationExecutor::new(execution).map_err(io)?)
    } else {
        None
    };
    if let Some(executor) = &executor {
        let configuration = executor.options();
        let kind = provider.as_ref().map_or("none", |provider| provider.kind());
        let strategy = match kind {
            "finite-immutable" => "assisted-ordered-work-stealing-blocks-v1",
            "dynamic" => "dynamic-assisted-ordered-work-stealing-barriers-v1",
            _ => "ordered-work-stealing-blocks-v1",
        };
        report["execution"] = json!({"strategy":strategy,"provider_kind":kind,
            "stage":"waiting for native batch","mode":"waiting",
            "workers":configuration.workers,"reserved_workers":configuration.workers,"batch_rows":configuration.batch_rows,
            "block_rows":configuration.block_rows,"checkpoint":"committed native ordinary/provider prefix; speculative blocks discarded"});
        observer(event(report, "master_reduction_progress", started));
    }
    while !session.is_complete() && !cancel.load(Ordering::Relaxed) {
        // Scope extension may need to reindex old exact rows. This preserves
        // previous refinements, but publication never processes a new source.
        if options.operation == MasterReductionOperation::Publish
            && session.statistics().pending_rebuild_rows == 0
        {
            break;
        }
        if options.operation == MasterReductionOperation::Publish {
            session.step_rebuild_only(cancel).map_err(io)?;
        } else if let Some(provider) = &mut provider {
            let executor = executor.as_ref().unwrap();
            if let Some(finite) = &provider.finite {
                executor
                    .step_with_finite_provider_observed(session, cancel, finite, |progress| {
                        observed_execution(report, progress, observer, started);
                    })
                    .map_err(io)?;
            } else {
                executor
                    .step_with_provider_observed(
                        session,
                        cancel,
                        &mut provider.provider,
                        |progress| {
                            observed_execution(report, progress, observer, started);
                        },
                    )
                    .map_err(io)?;
            }
        } else {
            executor
                .as_ref()
                .unwrap()
                .step_observed(session, cancel, |progress| {
                    observed_execution(report, progress, observer, started);
                })
                .map_err(io)?;
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
        reset_execution_phase(report, "paused", "idle");
        save(options, &session, report, "paused", started, observer)?;
        return Ok(());
    }
    reset_execution_phase(report, "terminal collection", "serial");
    collection::complete(options, session, report, cancel, observer, started)?;
    if cancel.load(Ordering::Relaxed) {
        reset_execution_phase(report, "paused", "idle");
        save(options, session, report, "paused", started, observer)?;
        return Ok(());
    }
    reset_execution_phase(report, "completed", "idle");
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

/// Native progress identifies the actual phase. A reserved pool is not a
/// measurement of busy workers, and coordinator/serial-provider barriers must
/// not be presented as parallel elimination merely because workers > 1.
fn observed_execution(
    report: &mut Value,
    progress: rustred::reduction::terminal_relations::TerminalRelationExecutionProgress,
    observer: &impl Fn(Value),
    started: Instant,
) {
    report["execution"]["stage"] = json!(progress.stage);
    report["execution"]["mode"] = json!(match progress.stage {
        "preparing rows" | "parallel elimination" => "parallel",
        "provider request" => "barrier",
        "ordered merge" | "atomic block merge" => "merge",
        _ => "serial",
    });
    report["execution"]["rows_total"] = json!(progress.rows_total);
    report["execution"]["rows_prepared"] = json!(progress.rows_prepared);
    report["execution"]["rows_eliminated"] = json!(progress.rows_eliminated);
    report["execution"]["rows_committed"] = json!(progress.rows_committed);
    report["execution"]["blocks_completed"] = json!(progress.blocks_completed);
    report["execution"]["provider_keys_filtered"] = json!(progress.provider_keys_filtered);
    observer(event(report, "master_reduction_progress", started));
}

fn reset_execution_phase(report: &mut Value, stage: &str, mode: &str) {
    let Some(execution) = report.get_mut("execution").and_then(Value::as_object_mut) else {
        return;
    };
    execution.insert("stage".into(), json!(stage));
    execution.insert("mode".into(), json!(mode));
    for name in [
        "rows_total",
        "rows_prepared",
        "rows_eliminated",
        "rows_committed",
        "blocks_completed",
        "provider_keys_filtered",
    ] {
        execution.remove(name);
    }
}

fn publish_final(options: &MasterReductionOptions, report: &mut Value) -> Result<(), AppError> {
    finite_provenance::validate_required(
        &options.directory,
        report,
        options.finite_policy.as_ref(),
    )?;
    report_counts::update(report);
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

fn strategy_scope_binding(binding: String, strategy: MasterRefinementStrategy) -> String {
    if strategy == MasterRefinementStrategy::Exact {
        return binding;
    }
    let mut hash = blake3::Hasher::new();
    hash.update(binding.as_bytes());
    modular::hash(strategy, &mut hash);
    hash.finalize().to_hex().to_string()
}

fn update_stats(report: &mut Value, session: &TerminalRelationSession) {
    // The core owns all algebraic counts and work cursors; this is only their
    // presentation/transport boundary, shared by CLI and notebook consumers.
    let stats = session.statistics();
    report_counts::update(report);
    report["independently_verified"] = json!(verification::independently_verified(report));
    report["publication_assurance"] = json!(if verification::independently_verified(report) {
        "independently_verified"
    } else if report["schema"] == FINITE_SCHEMA && report["inventory"]["complete"] == true {
        "trusted_generated"
    } else if report["inventory"]["complete"] == true {
        "trusted_saved_scope"
    } else {
        "not_completed"
    });
    report["stage"] = json!(if report["collection"]["status"] == "preparing" {
        "terminal_collection"
    } else if report["operation"] != "publish"
        && (stats.pending_assistance_keys > 0 || stats.pending_assistance_rows > 0)
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
    report["finite_remaining_terminals"] = json!(stats.remaining_terminals);
    report["remaining_terminals"] = report["collection"]["primary_remaining_terminals"]
        .as_u64()
        .map_or_else(|| json!(stats.remaining_terminals), |count| json!(count));
    report["eliminated_terminals"] = json!(
        initially_normalized.saturating_sub(
            report["remaining_terminals"]
                .as_u64()
                .unwrap_or(stats.remaining_terminals as u64) as usize
        )
    );
    report["terminal_relations"] = json!(stats.terminal_relations);
    report["seed_depth"] = json!(stats.seed_depth);
    report["completed_assistance_keys"] = json!(stats.completed_assistance_keys);
    report["pending_assistance_keys"] = json!(stats.pending_assistance_keys);
    report["completed_assistance_rows"] = json!(stats.completed_assistance_rows);
    report["pending_assistance_rows"] = json!(stats.pending_assistance_rows);
    report["finite_search_complete"] = json!(stats.complete);
    let modular = report["refinement_strategy"] == "modular-circuits";
    let partial_export =
        report["operation"] == "publish" && report["partial_refinement_source"].is_object();
    report["refinement_complete"] = json!(
        !partial_export
            && (if modular {
                report["modular_search_complete"] == true
            } else {
                stats.complete
            })
            && report["collection"]["status"] == "completed"
    );
    if modular {
        report["exact_source_search_complete"] = json!(stats.complete);
        if report["collection"]["status"] != "preparing" {
            report["stage"] = report["modular_refinement"]["stage"]
                .as_str()
                .map_or_else(|| json!("modular_discovery"), |stage| json!(stage));
        }
    }
    report["capabilities"] = json!({
        "saved_scope_inventory_complete": report["inventory"]["complete"] == true,
        "saved_scope_coverage_verified": verification::independently_verified(report),
        "native_terminal_state": true,
        "exact_current_terminal_substitutions": stats.pending_rebuild_rows == 0,
        "collected_terminal_application": report["collection"]["status"] == "completed",
        "routed_coefficient_application": false,
        "numerical_master_values": false,
        "minimality_proved": false
    });
    if modular {
        report["capabilities"]["modular_discovery_complete"] =
            report["modular_search_complete"].clone();
        report["capabilities"]["exact_source_span_exhausted"] = json!(stats.complete);
        report["capabilities"]["accepted_modular_relations_exactly_verified"] = json!(true);
    }
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
    fresh::validate(&options.directory, report, session)?;
    report["schema"] = json!(current_schema(report)?);
    let generation = report["checkpoint"]["generation"]
        .as_u64()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| AppError::limit("checkpoint generation overflow"))?;
    let bytes = session
        .to_native_bytes(profile::native_io_limits(None))
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
            .map(|(key, value)| {
                let value = if key == "sector_symmetry_reduction" {
                    value.as_object().map_or_else(
                        || value.clone(),
                        |fields| {
                            Value::Object(
                                fields
                                    .iter()
                                    .filter(|(field, _)| field.as_str() != "failure_receipts")
                                    .map(|(field, value)| (field.clone(), value.clone()))
                                    .collect(),
                            )
                        },
                    )
                } else {
                    value.clone()
                };
                (key.clone(), value)
            })
            .collect(),
    );
    result["event"] = json!(kind);
    report_counts::copy_from_inventory(report, &mut result);
    result["elapsed_seconds"] = json!(started.elapsed().as_secs_f64());
    result
}
