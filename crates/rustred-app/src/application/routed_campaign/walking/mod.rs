//! Shared symbolic successor discovery over one immutable owner snapshot.
//! Stable streamed publication is not a family-closure certificate.
mod checkpoint;
mod delegation;
mod descendant_closure;
mod diagnostics;
mod epoch;
#[cfg(test)]
pub(super) use epoch::force_resolver_break;
#[cfg(test)]
mod epoch_lookup_tests;
mod execution;
mod finite_replay;
mod g2;
mod index_report;
mod initial_orthants;
mod initial_overlap;
mod inspection;
mod parallel;
mod physical_parts;
mod publication;
mod queue;
mod rank_telemetry;
#[cfg(all(test, feature = "cli"))]
mod reinspection;
mod rescue;
mod rescue_plan;
mod reuse;
mod routing;
mod verify_closure;
pub use verify_closure::{
    OwnerDomainWalkInventory, OwnerDomainWalkInventoryOptions, owner_domain_walk_inventory,
};
mod work_policy;
mod worker_budget;

use super::{OwnerDomainMatchRequest, RoutedCampaignRequest, input, matching, prepare};
use crate::AppError;
use matching::input::power_bounds_json;
use queue::{Domain, Phase, Queue};
use rustred::solver::{OwnerAppliedLimits, OwnerAppliedStats};
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

pub use checkpoint::{
    OWNER_DOMAIN_WALK_CHECKPOINT_FORMAT, OWNER_DOMAIN_WALK_CHECKPOINT_MANIFEST_MAX_BYTES,
    OWNER_DOMAIN_WALK_CHECKPOINT_SCHEMA, OwnerDomainWalkCheckpointOptions,
};
pub use delegation::SchedulingPolicy as OwnerDomainWalkSchedulingPolicy;
pub use finite_replay::{
    OWNER_DOMAIN_WALK_FINITE_REPLAY_VERSION, OwnerDomainWalkFiniteReplayLimits,
};
pub use g2::G2ResidualAnchors as OwnerDomainWalkG2ResidualAnchors;

/// Resume binding for saved walk state. A CP5 checkpoint records this value
/// and `--resume` refuses any executable whose value differs; a different
/// executable digest with the same value is accepted (recorded, not refused).
///
/// Bump contract: increment whenever a change could make the same saved state
/// evolve differently or mean something different under the new binary, that
/// is any change to
/// - admission ordering (batch/helper ordering, ready-ticket fairness),
/// - the containment predicate, semantic summaries or the minimum-ID choice
///   among containing candidates,
/// - ledger reservation, transfer or publication rules (fences, credits,
///   alias publication, protected initial prefixes),
/// - replay token hashing (`execution/replay.rs`),
/// - inspection event emission order or the effect of an event,
/// - `Ticket` encoding or physical subdivision semantics,
/// - core matching/routing semantics that decide successors or frontiers,
/// - the sidecar/record schema consumed at finalization.
///
/// Transport changes (file layout, section codecs, digests, compaction,
/// scheduling of saves) do not bump this value.
pub const WALK_SEMANTICS_VERSION: u32 = 1;
/// Walk semantics of publication policy `epoch` (the legacy policies Ordered,
/// Ready and OwnerBatched stay at `WALK_SEMANTICS_VERSION`). Probe:
/// `per_policy {ordered: 1, ready: 1, epoch: 4}`.
pub use epoch::EPOCH_WALK_SEMANTICS_VERSION;
pub(crate) use epoch::{EPOCH_WALK_CHECKPOINT_FORMAT, EPOCH_WALK_CHECKPOINT_SCHEMA};
pub use physical_parts::ApplySubdivision as OwnerDomainWalkApplySubdivision;
pub use publication::{
    OwnerDomainWalkEpochDispatchPolicy, OwnerDomainWalkEpochInspectorLookup,
    OwnerDomainWalkEpochPublicationOrder, OwnerDomainWalkPublicationPolicy,
};
pub use rescue::{
    AMENDMENT_SCHEMA as OWNER_DOMAIN_WALK_AMENDMENT_SCHEMA,
    MAX_AMENDMENT_BYTES as OWNER_DOMAIN_WALK_AMENDMENT_MAX_BYTES, OwnerDomainWalkAmendment,
    SCOPE_EXTENSION_SCHEMA as OWNER_DOMAIN_WALK_SCOPE_EXTENSION_SCHEMA,
};
pub use rescue_plan::{
    OWNER_DOMAIN_WALK_RESCUE_PLAN_SCHEMA, OwnerDomainWalkRescuePlan,
    OwnerDomainWalkRescuePlanOptions, OwnerDomainWalkRescueScope, owner_domain_walk_rescue_plan,
};
pub use verify_closure::{
    OWNER_DOMAIN_WALK_VERIFY_SCHEMA, OwnerDomainWalkVerifyMutation, OwnerDomainWalkVerifyOptions,
    OwnerDomainWalkVerifyReferenceLevers, OwnerDomainWalkVerifyReinspect,
    OwnerDomainWalkVerifyScope, owner_domain_walk_verify_closure,
};
pub use work_policy::FrontierPolicy as OwnerDomainWalkFrontierPolicy;

#[derive(Clone, Debug)]
pub struct OwnerDomainWalkRequest {
    pub checkpoint: Option<OwnerDomainWalkCheckpointOptions>,
    /// Experimental exact discharge of the whole initial ID0 singleton or
    /// an exactly equivalent finite zero-lower A/R/D envelope.
    /// Requires a fresh CP6 walk; successful summaries are cold-replayed.
    /// None preserves the existing symbolic walk and checkpoint binding.
    pub finite_replay: Option<OwnerDomainWalkFiniteReplayLimits>,
    /// Optional supervisor stop-file context for epoch checkpoint diagnostics.
    /// Operational only: excluded from mathematical binding; cancellation is
    /// authoritative even when this file is absent or malformed.
    pub epoch_stop_file: Option<std::path::PathBuf>,
    pub apply_subdivision: Option<OwnerDomainWalkApplySubdivision>,
    /// Load policy and per-domain matcher allowances; max_total_pieces applies
    /// only to local-match reports, not this streaming worklist.
    pub matching: OwnerDomainMatchRequest,
    /// The nested matching member is overridden by matching.match_limits.
    pub applied_limits: OwnerAppliedLimits,
    /// One runs inline. More share immutable programs and bounded event slots.
    /// Caller configures affinity and native inner pools; no environment edits.
    pub workers: usize,
    /// Optional explicit compute partition, not an independent pool size cap.
    /// At W>1 reserves N inspectors, W-1-N admission helpers and one coordinator.
    /// W=1 accepts only N=1 inline. Finite containment caps require N=W-1 at W>1.
    /// None preserves the publication policy's existing automatic split.
    pub inspection_workers: Option<usize>,
    /// Ordered is the stable global stream. OwnerBatched uses independent
    /// phase/owner queues; diagnostic identities and capped prefixes may differ.
    pub publication_policy: OwnerDomainWalkPublicationPolicy,
    /// Experimental lockstep inspector lookup; Snapshot requires explicit CP6.
    /// Bound for same-mode resume; no default change or persisted snapshot view.
    pub epoch_inspector_lookup: OwnerDomainWalkEpochInspectorLookup,
    /// Opt-in bounded rolling CP6 execution. False retains lockstep as a
    /// differential control; the choice is frozen across checkpoint resume.
    pub epoch_rolling: bool,
    /// Optional cost/work-growth-guided dispatch. Requires rolling CP6;
    /// observations and unselected candidates survive checkpoint/resume.
    pub epoch_dispatch: OwnerDomainWalkEpochDispatchPolicy,
    /// Fresh rolling publication policy, frozen across checkpoint resume.
    pub epoch_publication_order: OwnerDomainWalkEpochPublicationOrder,
    /// Explicit rolling cut bound (1..=4096). None retains the fixed default
    /// 16, or the existing diagnostic override. A custom cut must match resume.
    pub epoch_cut_size: Option<usize>,
    /// Explicit fresh rolling unmerged-work bound (1..=4096, at least the cut).
    /// None selects the automatic fresh bound and inherits a saved window on
    /// resume; an explicit resumed bound must equal the persisted window.
    pub epoch_window: Option<usize>,
    /// Extra logical reservations backed by complete-result escrow. Zero
    /// retains the normal window; this does not add compute threads.
    pub epoch_result_escrow_jobs: usize,
    /// Admission threshold for retained complete results, not a hard RSS cap.
    /// Required and positive when escrow jobs are enabled.
    pub epoch_result_escrow_bytes: Option<usize>,
    /// Epoch P2 helpers inside `workers`, not additional threads. Zero selects
    /// serial preparation; None retains the helper complement of an explicit
    /// inspection partition, or zero helpers when both are omitted.
    pub epoch_preparation_workers: Option<usize>,
    /// Maximum retained P2 obligations/retirements per cut. These are logical
    /// scratch counts, not bytes or cumulative-work/rank limits. Exceeding one
    /// stops before publication; nothing is truncated. None uses u32::MAX.
    pub epoch_preparation_max_obligations: Option<usize>,
    pub epoch_preparation_max_retirements: Option<usize>,
    /// Optional responsibility transfer under exact containment. The fixed
    /// logical lookahead is independent of physical worker count.
    pub scheduling_policy: OwnerDomainWalkSchedulingPolicy,
    /// Opt-in exact high-D overlap reuse against pinned initial Apply domains.
    /// Requires TransferUnreserved; native work covers the remaining low band.
    pub reuse_initial_d_bands: bool,
    /// Opt-in G2' residual anchors (dispatch-time D-band residual inspection
    /// against the union of merged anchors; requires TransferUnreserved).
    pub g2_residual_anchors: OwnerDomainWalkG2ResidualAnchors,
    /// Resume only: switch a checkpoint written WITHOUT G2' to `union` (a
    /// recorded, append-only binding amendment; the G2' log is back-filled
    /// from the ledger and the record order). Transport, not bound.
    pub g2_activate_on_resume: bool,
    pub max_domains: usize,
    /// Committed logical callbacks, not speculative native attempts or bytes.
    pub max_events: usize,
    /// Aggregate retained input/Apply/Route obligations, independent of events.
    pub max_frontiers: usize,
    /// Record (default) or save-and-stop at the first frontier (A10).
    pub frontier_policy: OwnerDomainWalkFrontierPolicy,
    /// None leaves aggregate general comparisons unlimited. A positive finite
    /// cap is an opt-in diagnostic budget, not a restriction on actual rank.
    pub max_containment_checks: Option<usize>,
    pub route_domain_overcover: bool,
    /// Opt-in necessary degree bound over the union of source numerator rows.
    pub route_joint_source_support_pruning: bool,
    pub max_route_masks: usize,
    /// Resume-time rescue amendments in chain order (`rescue.rs`); empty for
    /// every unamended walk. Not part of the request binding: each file is
    /// bound by its digest chain from that binding instead.
    pub amendments: Vec<OwnerDomainWalkAmendment>,
}
impl OwnerDomainWalkRequest {
    /// Digest of this exact frozen request for the first append-only amendment
    /// parent. This is an identity helper, not checkpoint validation or proof
    /// of closure. Later amendments chain from the preceding file's digest.
    pub fn checkpoint_binding(&self) -> String {
        match self.publication_policy {
            OwnerDomainWalkPublicationPolicy::Epoch => checkpoint::epoch_request_binding(self),
            _ => checkpoint::request_binding(self),
        }
    }

    pub fn new(matching: OwnerDomainMatchRequest) -> Self {
        Self {
            checkpoint: None,
            finite_replay: None,
            epoch_stop_file: None,
            apply_subdivision: None,
            matching,
            applied_limits: Default::default(),
            workers: 1,
            inspection_workers: None,
            publication_policy: OwnerDomainWalkPublicationPolicy::Ordered,
            epoch_inspector_lookup: OwnerDomainWalkEpochInspectorLookup::AllMiss,
            epoch_rolling: false,
            epoch_dispatch: OwnerDomainWalkEpochDispatchPolicy::Fifo,
            epoch_publication_order: OwnerDomainWalkEpochPublicationOrder::OldestPrefix,
            epoch_cut_size: None,
            epoch_window: None,
            epoch_result_escrow_jobs: 0,
            epoch_result_escrow_bytes: None,
            epoch_preparation_workers: None,
            epoch_preparation_max_obligations: None,
            epoch_preparation_max_retirements: None,
            scheduling_policy: OwnerDomainWalkSchedulingPolicy::InspectAll,
            reuse_initial_d_bands: false,
            g2_residual_anchors: OwnerDomainWalkG2ResidualAnchors::Off,
            g2_activate_on_resume: false,
            max_domains: 100_000,
            max_events: 1_000_000,
            max_frontiers: 100_000,
            frontier_policy: OwnerDomainWalkFrontierPolicy::Record,
            max_containment_checks: None,
            route_domain_overcover: false,
            route_joint_source_support_pruning: false,
            max_route_masks: 100_000,
            amendments: Vec::new(),
        }
    }

    /// Pure numeric policy projection, before owner admission or native work.
    /// It is diagnostic only; actual execution/cold replay reports the same
    /// projection from the admitted reducer. None preserves default-off output.
    pub fn finite_replay_budget_summary(&self) -> Option<Value> {
        finite_replay::budget_summary(self)
    }

    /// Runtime admission for the experimental finite replay path. Offline
    /// verification uses the original fresh request and independently checks
    /// every retained recipe; this is not a resume-capability promise.
    pub fn validate_finite_replay(&self) -> Result<(), &'static str> {
        if self.finite_replay.is_none() {
            return Ok(());
        }
        if self.publication_policy != OwnerDomainWalkPublicationPolicy::Epoch
            || !self.checkpoint.as_ref().is_some_and(|cp| !cp.resume)
            || !self.amendments.is_empty()
            || self.g2_activate_on_resume
        {
            return Err(
                "finite replay requires a fresh epoch checkpoint, without resume or amendments",
            );
        }
        Ok(())
    }

    fn validate_epoch_inspector_lookup(&self) -> Result<(), &'static str> {
        self.validate_epoch_result_escrow()?;
        let preparation = self.epoch_preparation_workers.is_some()
            || self.epoch_preparation_max_obligations.is_some()
            || self.epoch_preparation_max_retirements.is_some();
        if preparation && self.publication_policy != OwnerDomainWalkPublicationPolicy::Epoch {
            return Err("Epoch preparation options require epoch publication");
        }
        self.epoch_preparation_limits()?;
        if self.publication_policy == OwnerDomainWalkPublicationPolicy::Epoch {
            Self::validate_epoch_worker_partition(
                self.workers,
                self.inspection_workers,
                self.max_containment_checks,
                self.epoch_preparation_workers,
            )?;
        }
        if (self.epoch_publication_order != OwnerDomainWalkEpochPublicationOrder::OldestPrefix
            || self.epoch_cut_size.is_some()
            || self.epoch_window.is_some())
            && !self.epoch_rolling
        {
            return Err("Epoch cut/window/publication options require rolling CP6");
        }
        if self
            .epoch_cut_size
            .is_some_and(|n| !(1..=4096).contains(&n))
            || self.epoch_window.is_some_and(|n| !(1..=4096).contains(&n))
        {
            return Err("Epoch cut and window must be in 1..=4096");
        }
        if let Some(window) = self.epoch_window {
            let cut = self
                .effective_epoch_cut_size()
                .map_err(|_| "Invalid epoch cut size or diagnostic override")?;
            if window < cut {
                return Err("Epoch window must be at least the cut size");
            }
        }
        if self.epoch_dispatch == OwnerDomainWalkEpochDispatchPolicy::Adaptive
            && !self.epoch_rolling
        {
            return Err("Adaptive dispatch requires rolling checkpoint-enabled epoch publication");
        }
        if self.epoch_rolling
            && (self.publication_policy != OwnerDomainWalkPublicationPolicy::Epoch
                || self.checkpoint.is_none())
        {
            return Err("Rolling execution requires checkpoint-enabled epoch publication");
        }
        if self.epoch_inspector_lookup != OwnerDomainWalkEpochInspectorLookup::AllMiss
            && (self.publication_policy != OwnerDomainWalkPublicationPolicy::Epoch
                || self.checkpoint.is_none())
        {
            return Err("Snapshot inspector lookup requires checkpoint-enabled epoch publication");
        }
        Ok(())
    }

    /// Resolve the public cut size and reject a conflicting diagnostic override.
    pub fn effective_epoch_cut_size(&self) -> Result<usize, String> {
        if let Some(cut) = self.epoch_cut_size {
            if !(1..=4096).contains(&cut) {
                return Err("Epoch cut size must be in 1..=4096".into());
            }
            let diagnostic = epoch::lockstep_b()?;
            if diagnostic != epoch::LOCKSTEP_B && diagnostic != cut {
                return Err("Explicit epoch cut conflicts with diagnostic override".into());
            }
            Ok(cut)
        } else {
            epoch::lockstep_b()
        }
    }

    /// Initial logical in-flight bound, not an extra compute pool. Resolve the
    /// fresh bound only; restore retains its authenticated window across widths.
    pub fn resolved_epoch_window(&self, cut_size: usize) -> Result<usize, &'static str> {
        if !(1..=4096).contains(&cut_size) {
            return Err("Epoch cut size must be in 1..=4096");
        }
        if let Some(window) = self.epoch_window {
            if !(cut_size..=4096).contains(&window) {
                return Err("Epoch window must be between its cut size and 4096");
            }
            return Ok(window);
        }
        Ok(if !self.epoch_rolling {
            cut_size
        } else if self.workers == 1 {
            1
        } else {
            worker_budget::WorkerBudget::for_request(self)
                .inspection
                .saturating_add(cut_size)
                .max(cut_size)
                .min(4096)
        })
    }

    pub fn validate_epoch_result_escrow(&self) -> Result<(), &'static str> {
        if self.epoch_result_escrow_jobs == 0 {
            return if self.epoch_result_escrow_bytes.is_none() {
                Ok(())
            } else {
                Err("Epoch result escrow bytes require positive escrow jobs")
            };
        }
        if self.publication_policy != OwnerDomainWalkPublicationPolicy::Epoch
            || !self.epoch_rolling
            || self.checkpoint.is_none()
            || self.epoch_publication_order != OwnerDomainWalkEpochPublicationOrder::OldestPrefix
        {
            return Err(
                "Epoch result escrow requires checkpoint-enabled rolling oldest-prefix publication",
            );
        }
        if !self.epoch_result_escrow_bytes.is_some_and(|n| n > 0) {
            return Err("Epoch result escrow requires a positive explicit byte admission budget");
        }
        let cut = self
            .effective_epoch_cut_size()
            .map_err(|_| "Invalid epoch cut size or diagnostic override")?;
        self.resolved_epoch_total_window(cut).map(|_| ())
    }

    /// Total bounded logical reservation space, including returned results.
    /// W=1 retains the declaration but never dispatches extra work; callers
    /// must clamp publication cuts to the base window, not this total.
    pub fn resolved_epoch_total_window(&self, cut_size: usize) -> Result<usize, &'static str> {
        let base = self.resolved_epoch_window(cut_size)?;
        base.checked_add(self.epoch_result_escrow_jobs)
            .filter(|&total| total <= 4096)
            .ok_or("Epoch base window plus escrow jobs must be at most 4096")
    }

    pub(crate) fn epoch_window(&self, cut_size: usize) -> usize {
        self.resolved_epoch_window(cut_size)
            .expect("validated epoch window")
    }

    pub(crate) fn validate_inspection_workers(
        workers: usize,
        inspection_workers: Option<usize>,
        max_containment_checks: Option<usize>,
    ) -> Result<(), &'static str> {
        worker_budget::validate(workers, inspection_workers, max_containment_checks)
    }

    pub(crate) fn validate_epoch_worker_partition(
        workers: usize,
        inspection: Option<usize>,
        containment_cap: Option<usize>,
        preparation: Option<usize>,
    ) -> Result<(), &'static str> {
        worker_budget::epoch_inspection(workers, inspection, containment_cap, preparation)
            .map(|_| ())
    }

    pub(crate) fn epoch_preparation_limits(&self) -> Result<(usize, usize), &'static str> {
        let resolve = |value: Option<usize>| {
            let value = value.unwrap_or(u32::MAX as usize);
            if value == 0 || value > u32::MAX as usize {
                Err("Epoch preparation counts must be in 1..=u32::MAX")
            } else {
                Ok(value)
            }
        };
        Ok((
            resolve(self.epoch_preparation_max_obligations)?,
            resolve(self.epoch_preparation_max_retirements)?,
        ))
    }
}

#[derive(Clone, Debug)]
pub struct OwnerDomainWalkResult {
    /// Every admitted obligation discharged by inspection or an explicitly
    /// tracked containing representative, without unresolved work. Does not
    /// certify provenance, global route order compatibility, or family closure.
    pub all_scheduled_domains_resolved: bool,
    /// The report. When `records` is streamed (checkpointed walks) its
    /// `domains` entry is `null` here; `write_json` and `into_document`
    /// supply the records.
    pub document: Value,
    pub records: OwnerDomainWalkRecords,
}

/// Where a walk report's `domains` live: inline in the document (default),
/// or in the checkpoint's record sidecar, read back one record at a time
/// with the finalization annotations applied when the report is written.
#[derive(Clone, Default)]
pub struct OwnerDomainWalkRecords(Option<RecordSource>);
#[derive(Clone)]
enum RecordSource {
    Legacy(execution::records::Streamed),
    Epoch(epoch::record_store::Streamed),
}
impl RecordSource {
    fn project_coordinates(&mut self, n: usize) {
        match self {
            Self::Legacy(source) => source.project_coordinates(n),
            Self::Epoch(source) => source.project_coordinates(n),
        }
    }
    fn write_json(&self, document: &Value, out: impl std::io::Write) -> Result<(), String> {
        match self {
            Self::Legacy(s) => s.write_json(document, out),
            Self::Epoch(s) => s.write_json(document, out),
        }
    }
    fn collect(&self) -> Result<Vec<Value>, String> {
        match self {
            Self::Legacy(s) => s.collect(),
            Self::Epoch(s) => s.collect(),
        }
    }
    fn total(&self) -> usize {
        match self {
            Self::Legacy(s) => s.total(),
            Self::Epoch(s) => s.total(),
        }
    }
    fn segments(&self) -> usize {
        match self {
            Self::Legacy(s) => s.segments(),
            Self::Epoch(s) => s.segments(),
        }
    }
}
impl OwnerDomainWalkRecords {
    pub fn is_streamed(&self) -> bool {
        self.0.is_some()
    }
    /// Pretty JSON of `document`, byte-identical to
    /// `serde_json::to_writer_pretty` of the materialized report; streamed
    /// records are never all resident.
    pub fn write_json(&self, document: &Value, out: impl std::io::Write) -> Result<(), String> {
        match &self.0 {
            Some(streamed) => streamed.write_json(document, out),
            None => serde_json::to_writer_pretty(out, document).map_err(|e| e.to_string()),
        }
    }
    /// Place every record into `document["domains"]` (small runs and tests).
    pub fn materialize(&self, document: &mut Value) -> Result<(), String> {
        if let Some(streamed) = &self.0 {
            document["domains"] = Value::Array(streamed.collect()?);
        }
        Ok(())
    }
}
impl std::fmt::Debug for OwnerDomainWalkRecords {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            None => f.write_str("OwnerDomainWalkRecords::Inline"),
            Some(streamed) => f
                .debug_struct("OwnerDomainWalkRecords::Streamed")
                .field("records", &streamed.total())
                .field("segments", &streamed.segments())
                .finish(),
        }
    }
}

impl OwnerDomainWalkResult {
    /// See `OwnerDomainWalkRecords::write_json`.
    pub fn write_json(&self, out: impl std::io::Write) -> Result<(), String> {
        self.records.write_json(&self.document, out)
    }
    pub fn into_document(mut self) -> Result<Value, String> {
        self.records.materialize(&mut self.document)?;
        Ok(self.document)
    }
    pub(crate) fn completion_progress(document: &Value) -> Value {
        let mut out = json!({"event":"finished", "operation":"owner_domain_walk",
            "full_result_in_output_document":true, "family_closure_claim":false});
        if document["status"] == "paused" {
            out["full_result_in_output_document"] = json!(false);
            out["full_state_in_checkpoint"] = json!(true);
        }
        // CP6 resource-stop and staged drain summaries are not full record
        // inventories, even though their status is not the CP5 `paused` value.
        for key in ["full_result_in_output_document", "full_state_in_checkpoint"] {
            if document[key].is_boolean() {
                out[key] = document[key].clone();
            }
        }
        for key in [
            "status",
            "workers",
            "parallel",
            "scheduled_nodes",
            "completed_nodes",
            "queued_nodes",
            "failed_nodes",
            "processed_nodes",
            "deduplication_hits",
            "exact_domain_hits",
            "full_orthant_hits",
            "job_local_reuse_hits",
            "pre_admitted_orthant_hits",
            "containment_checks",
            "initial_prepass_containment_checks",
            "containment_maintenance_checks",
            "containment_retired_candidates",
            "containment_summary_builds",
            "containment_semantic_hits",
            "containment_semantic_retirements",
            "containment_candidates",
            "containment_index_policy",
            "max_containment_checks",
            "containment_check_policy",
            "bounded_refinement_axes",
            "max_bounded_refinement_cells",
            "routed_domains",
            "route_masks",
            "route_joint_support_masks_pruned",
            "route_domain_overcover",
            "route_joint_source_support_pruning",
            "max_scheduled_finite_rank",
            "unbounded_rank_domains",
            "encountered_numerator_rank",
            "successors",
            "conditional_successors",
            "optional_coefficient_refusals",
            "optional_original_refusals",
            "optional_coalesced_refusals",
            "frontiers",
            "events",
            "committed_events",
            "committed_domains",
            "contiguous_publication_watermark",
            "prepared_seconds",
            "traversal_seconds",
            "native_driver_seconds",
            "traversal_timing_boundary",
            "elapsed_seconds",
            "all_scheduled_domains_resolved",
            "recursive_worklist_exhausted",
            "resume_supported",
        ] {
            out[key] = document[key].clone();
        }
        for key in [
            "publication_policy",
            "requested_publication_policy",
            "worker_allocation",
            "requested_inspection_workers",
            "owner_batched_traversal_started",
            "owner_bucket_count",
            "nonempty_owner_buckets",
            "scheduling_policy",
            "delegation",
            "native_processed_nodes",
            "reuse_initial_d_bands",
            "partial_initial_inspections",
            "g2_residual_anchors",
            "finite_replay",
            "initial_overlap_index",
            "requested_max_queries",
            "requested_max_query_bytes",
            "checkpoint",
            "initial_entry_domains_total",
            "initial_entry_domains_inspected",
            "initial_entry_domains_published",
            "pending_descendant_domains",
            "descendant_closure",
            "frontier_policy",
            "stop_reason",
            "finalization",
            "query_admission",
            "admission_complete",
            "observer_failed",
        ] {
            if let Some(value) = document.get(key) {
                out[key] = value.clone();
            }
        }
        if document["epoch"]["inspector_lookup_mode"].is_string() {
            // Only the bounded public CP6 report supplies this mode marker.
            out["epoch"] = document["epoch"].clone();
        }
        if let Some(error) = document["error"].as_str() {
            out["error"] = json!(error.chars().take(512).collect::<String>());
        }
        out
    }
}

/// Largest symbolic worker budget one walk accepts; the CLI argument parser
/// and the shard supervisor configuration validate against the same bound.
pub const MAX_WALK_WORKERS: usize = 256;

/// Correctness-gate seam, not a campaign knob: an environment variable keeps
/// the frozen campaign argv (validated by the launcher) untouched. Both
/// campaign supervisors remove it from their native children's environment.
pub(crate) const DIAGNOSTIC_PAUSE_VARIABLE: &str = "RUSTRED_WALK_DIAGNOSTIC_PAUSE";

/// Force-save and cooperatively pause a checkpointed walk the first time its
/// state reaches a diagnostic trigger. Read once per walk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiagnosticPause {
    /// Ready: at least two unfinished accepted prefixes and a finished hole.
    ReadyMultiPrefix,
}
impl DiagnosticPause {
    fn from_environment() -> Result<Option<Self>, String> {
        Self::parse(std::env::var_os(DIAGNOSTIC_PAUSE_VARIABLE).as_deref())
    }
    /// Unset or empty disables; an unknown value is refused, never ignored.
    fn parse(value: Option<&std::ffi::OsStr>) -> Result<Option<Self>, String> {
        match value {
            None => Ok(None),
            Some(value) if value.is_empty() => Ok(None),
            Some(value) if value == Self::ReadyMultiPrefix.name() => {
                Ok(Some(Self::ReadyMultiPrefix))
            }
            Some(value) => Err(format!(
                "unsupported {DIAGNOSTIC_PAUSE_VARIABLE} value {value:?}; expected {}",
                Self::ReadyMultiPrefix.name()
            )),
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::ReadyMultiPrefix => "ready-multi-prefix",
        }
    }
    fn fires<const N: usize>(self, state: &execution::State<N>) -> bool {
        match self {
            Self::ReadyMultiPrefix => state.ready_multi_prefix_hole(),
        }
    }
    /// Only a fresh checkpointed Ready walk may pause. The trigger is a state
    /// predicate that a paused checkpoint still satisfies once restored, so a
    /// `--resume` with the variable set would pause again at its first
    /// checkpoint opportunity instead of running to exhaustion.
    fn admit(pause: Option<Self>, request: &OwnerDomainWalkRequest) -> Result<(), String> {
        if pause.is_none() {
            return Ok(());
        }
        match &request.checkpoint {
            Some(checkpoint)
                if request.publication_policy == OwnerDomainWalkPublicationPolicy::Ready =>
            {
                if checkpoint.resume {
                    Err(format!(
                        "{DIAGNOSTIC_PAUSE_VARIABLE} pauses a fresh checkpointed Ready walk only; unset it to --resume"
                    ))
                } else {
                    Ok(())
                }
            }
            _ => Err(format!(
                "{DIAGNOSTIC_PAUSE_VARIABLE} requires a checkpointed Ready walk"
            )),
        }
    }
}

/// One checkpoint opportunity of a diagnostic walk: the first time the
/// trigger holds, persist exactly that state under the label (its closure
/// snapshot included, see below), journal the trigger and cancel the way a
/// stop request does; the walk's own final save after cancellation carries
/// the same label. Taking `pause` makes it fire at most once per session.
/// Returns whether it fired (the caller then skips its ordinary interval
/// save).
fn diagnostic_checkpoint<const N: usize>(
    pause: &mut Option<DiagnosticPause>,
    store: &mut checkpoint::Store,
    state: &execution::State<N>,
    inputs: &[Value],
    input_frontiers: &[Value],
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<bool, String> {
    let Some(pause) = pause.take_if(|pause| pause.fires(state)) else {
        return Ok(false);
    };
    store.mark_diagnostic_pause(pause.name());
    // Forced like every non-periodic save. The walk cancels right after it
    // and ends with its own final save, so, like that one, it persists the
    // edge log without folding it (`SaveKind::Final`). The pre-save closure
    // refresh is never cut, even when a stop request or a journal failure
    // has already set the run's cancellation, so the closure snapshot is the
    // triggering state's unless scratch memory is short (then the previous
    // snapshot, stale but valid).
    if let Some(event) = store.save_cancellable(
        state,
        inputs,
        input_frontiers,
        checkpoint::SaveKind::Final,
        cancellation,
        observer,
    )? {
        observer(event);
    }
    let mut event = json!({"event":"diagnostic_pause","operation":"owner_domain_walk",
        "diagnostic_pause":pause.name(),"committed_domains":state.published_count(),
        "contiguous_publication_watermark":state.queue.next,"committed_events":state.events,
        "completed_native_inspections":state.completed,"family_closure_claim":false});
    state.add_ready_progress(&mut event);
    observer(event);
    cancellation.store(true, std::sync::atomic::Ordering::Release);
    Ok(true)
}

/// One checkpoint opportunity of an A10 frontier stop. `trigger` holds the
/// committed frontier count at session start (0 for a fresh walk, so initial
/// input frontiers fire before any inspection; the restored count on
/// resume). The first time the walk's count exceeds it and no live inspection
/// holds accepted but uncommitted frontier details (the frontier-bearing
/// inspection has committed), persist exactly that state (its record included) labelled with the stop reason,
/// journal the stop and cancel the way a stop request does; the walk's own
/// final save after cancellation repeats the label. Taking `trigger` makes it
/// fire at most once per session. Returns whether it fired.
fn frontier_stop_checkpoint<const N: usize>(
    trigger: &mut Option<usize>,
    store: &mut checkpoint::Store,
    state: &execution::State<N>,
    inputs: &[Value],
    input_frontiers: &[Value],
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<bool, String> {
    // Rescue (`rescue.rs`): an abandoned obligation's bookkeeping frontier
    // and any frontier of a quarantined (non-live) inspection never stop the
    // walk (0 in an unamended walk).
    // The stop fires once the new frontier's inspection is committed: no
    // live inspection may hold accepted but uncommitted frontier details (a
    // stop inside a chunked stream would leave a half-published inspection
    // whose resume must replay it).
    let Some(baseline) = trigger.take_if(|baseline| {
        state.frontiers.saturating_sub(state.rescue_quiet_frontiers) > *baseline
            && !state.loud_frontier_prefix()
    }) else {
        return Ok(false);
    };
    store.mark_stop_reason(work_policy::FRONTIER_STOP_REASON);
    // Forced and final, like a diagnostic pause: the walk ends after it.
    if let Some(event) = store.save_cancellable(
        state,
        inputs,
        input_frontiers,
        checkpoint::SaveKind::Final,
        cancellation,
        observer,
    )? {
        observer(event);
    }
    let mut event = json!({"event":"frontier_stop","operation":"owner_domain_walk",
        "stop_reason":work_policy::FRONTIER_STOP_REASON,"frontier_policy":"stop",
        "frontiers":state.frontiers,"session_start_frontiers":baseline,
        "input_frontiers":input_frontiers.len(),"committed_domains":state.published_count(),
        "contiguous_publication_watermark":state.queue.next,"committed_events":state.events,
        "completed_native_inspections":state.completed,"family_closure_claim":false});
    state.add_ready_progress(&mut event);
    observer(event);
    cancellation.store(true, std::sync::atomic::Ordering::Release);
    Ok(true)
}

/// Admission of a walk request before any input is parsed: allowances,
/// policy combinations, the diagnostic pause and the worker partition. The
/// host core-budget preflight follows separately: it depends on this
/// process's affinity and license, not on the request.
fn admit_request(request: &OwnerDomainWalkRequest) -> Result<Option<DiagnosticPause>, AppError> {
    request
        .validate_epoch_inspector_lookup()
        .map_err(AppError::input)?;
    request.matching.preflight_queries()?;
    if request.max_domains == 0
        || request.max_events == 0
        || request.max_frontiers == 0
        || !(1..=MAX_WALK_WORKERS).contains(&request.workers)
        || request.max_containment_checks == Some(0)
        || request.max_route_masks == 0
    {
        return Err(AppError::input("invalid symbolic worklist allowances"));
    }
    if request.route_joint_source_support_pruning && !request.route_domain_overcover {
        return Err(AppError::input(
            "joint source-support pruning requires route domain overcover",
        ));
    }
    if request.publication_policy == OwnerDomainWalkPublicationPolicy::Epoch {
        // Semantics3 uses CP6 when checkpointed; memory-only remains explicit.
        // Frontier stop needs no checkpoint; no diagnostic pause is admitted.
        epoch::admit(request)?;
        if DiagnosticPause::from_environment()
            .map_err(AppError::input)?
            .is_some()
        {
            return Err(AppError::input(format!(
                "{DIAGNOSTIC_PAUSE_VARIABLE} is not supported by epoch publication"
            )));
        }
        OwnerDomainWalkRequest::validate_inspection_workers(
            request.workers,
            request.inspection_workers,
            request.max_containment_checks,
        )
        .map_err(AppError::input)?;
        return Ok(None);
    }
    if let Some(checkpoint) = &request.checkpoint {
        if checkpoint.interval_seconds == 0 {
            return Err(AppError::input("checkpoint interval must be positive"));
        }
        if request.publication_policy == OwnerDomainWalkPublicationPolicy::OwnerBatched {
            return Err(AppError::input(
                "checkpointing requires Ordered or Ready publication",
            ));
        }
    }
    if request.frontier_policy == OwnerDomainWalkFrontierPolicy::Stop
        && request.checkpoint.is_none()
    {
        return Err(AppError::input(
            "frontier stop requires a checkpointed Ordered or Ready walk",
        ));
    }
    if !request.amendments.is_empty()
        && !request
            .checkpoint
            .as_ref()
            .is_some_and(|checkpoint| checkpoint.resume)
    {
        return Err(AppError::input(
            "rescue amendments (--amend-queries) require --resume of a checkpointed walk",
        ));
    }
    request.validate_finite_replay().map_err(AppError::input)?;
    let diagnostic_pause = DiagnosticPause::from_environment().map_err(AppError::input)?;
    DiagnosticPause::admit(diagnostic_pause, request).map_err(AppError::input)?;
    if request.apply_subdivision.is_some()
        && request.publication_policy != OwnerDomainWalkPublicationPolicy::Ordered
    {
        return Err(AppError::input(
            "Apply subdivision requires ordered publication",
        ));
    }
    if request.publication_policy == OwnerDomainWalkPublicationPolicy::Ready
        && request.scheduling_policy == OwnerDomainWalkSchedulingPolicy::InspectAll
    {
        return Err(AppError::input(
            "Ready publication requires TransferUnreserved scheduling",
        ));
    }
    OwnerDomainWalkRequest::validate_inspection_workers(
        request.workers,
        request.inspection_workers,
        request.max_containment_checks,
    )
    .map_err(AppError::input)?;
    request
        .scheduling_policy
        .validate(request.max_containment_checks)
        .map_err(|error| AppError::input(error.to_string()))?;
    if request.reuse_initial_d_bands
        && request.scheduling_policy == OwnerDomainWalkSchedulingPolicy::InspectAll
    {
        return Err(AppError::input(
            "initial D-band reuse requires TransferUnreserved scheduling",
        ));
    }
    if request.g2_residual_anchors != OwnerDomainWalkG2ResidualAnchors::Off {
        if request.scheduling_policy == OwnerDomainWalkSchedulingPolicy::InspectAll {
            return Err(AppError::input(
                "G2' residual anchors require TransferUnreserved scheduling",
            ));
        }
        if request.apply_subdivision.is_some() {
            return Err(AppError::input(
                "G2' residual anchors do not support Apply subdivision",
            ));
        }
        if request.publication_policy == OwnerDomainWalkPublicationPolicy::OwnerBatched {
            return Err(AppError::input(
                "G2' residual anchors require Ordered or Ready publication",
            ));
        }
    }
    if request.g2_activate_on_resume
        && (request.g2_residual_anchors == OwnerDomainWalkG2ResidualAnchors::Off
            || !request.checkpoint.as_ref().is_some_and(|c| c.resume))
    {
        return Err(AppError::input(
            "G2' activation requires --resume and --g2-residual-anchors union",
        ));
    }
    Ok(diagnostic_pause)
}

pub fn owner_domain_walk_with_progress(
    request: OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
    observer: impl Fn(Value),
) -> Result<OwnerDomainWalkResult, AppError> {
    walk_with_progress(&request, cancellation, &observer)
}

// Keep the public callback ergonomic without multiplying both arity-dispatch
// trees by every caller's closure type. Progress is delivered by the calling
// coordinator, so neither the borrowed callback nor its captures need Send,
// Sync or 'static. The public wrapper retains the request's ownership scope.
fn walk_with_progress(
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
    observer: &dyn Fn(Value),
) -> Result<OwnerDomainWalkResult, AppError> {
    let diagnostic_pause = admit_request(request)?;
    rustred::campaign::ParallelExecution::preflight_requested_core_budget(request.workers)
        .map_err(|e| AppError::input(e.to_string()))?;
    let (selection, arity, limits) = input::Selection::parse(&request.matching.selection_json)?;
    let queries = matching::input::parse(
        &request.matching.queries_json,
        arity,
        request.matching.max_queries,
        request.matching.max_query_bytes,
    )?;
    let mut admitted = json!({"event":"admitted", "operation":"owner_domain_walk", "arity":arity,
        "input_domains":queries.len(), "workers":request.workers, "max_domains":request.max_domains,
        "max_events":request.max_events, "max_frontiers":request.max_frontiers,
        "max_containment_checks":request.max_containment_checks,
        "containment_check_policy":"general_comparisons_only; null_is_unlimited; checked_counter",
        "route_domain_overcover":request.route_domain_overcover, "max_route_masks":request.max_route_masks,
        "route_joint_source_support_pruning":request.route_joint_source_support_pruning,
        "applied_limits":limits_json(request), "publication_policy":"stable_domain_id_stream",
        "bounded_refinement_axes":matching::refinement_axes_name(request.matching.match_limits.refinement_axes),
        "max_bounded_refinement_cells":request.matching.match_limits.max_bounded_refinement_cells,
        "family_closure_claim":false, "ibp_generation":false});
    admitted["worker_allocation"] =
        worker_budget::WorkerBudget::for_request(request).json(request.inspection_workers);
    if request.scheduling_policy != OwnerDomainWalkSchedulingPolicy::InspectAll {
        admitted["scheduling_policy"] =
            execution::scheduling_policy_json(request.scheduling_policy);
        admitted["publication_policy"] = json!("stable_domain_responsibility_stream");
    }
    if request.reuse_initial_d_bands {
        admitted["reuse_initial_d_bands"] = json!(true);
        admitted["partial_inspection_policy"] =
            json!("exact_initial_high_D_overlap; pinned_anchor_plus_native_residual");
    }
    if request.g2_residual_anchors != OwnerDomainWalkG2ResidualAnchors::Off {
        admitted["g2_residual_anchors"] = json!(request.g2_residual_anchors.name());
    }
    if let Some(limits) = request.finite_replay {
        admitted["finite_replay"] = json!({
            "version": OWNER_DOMAIN_WALK_FINITE_REPLAY_VERSION,
            "scope": "whole_initial_id0_finite_domain",
            "limits": limits,
            "budget": request.finite_replay_budget_summary(),
            "cold_replay_required": true,
        });
    }
    if request.publication_policy == OwnerDomainWalkPublicationPolicy::OwnerBatched {
        admitted["publication_policy"] = json!("owner_batched");
    }
    if request.publication_policy == OwnerDomainWalkPublicationPolicy::Ready {
        admitted["publication_policy"] = json!("ready_ticket_stream");
    }
    if request.publication_policy == OwnerDomainWalkPublicationPolicy::Epoch {
        admitted["publication_policy"] = json!("epoch_merge_stream");
        admitted["walk_semantics_version"] = json!(EPOCH_WALK_SEMANTICS_VERSION);
        if request.checkpoint.is_some() {
            admitted["epoch_inspector_lookup"] = json!(request.epoch_inspector_lookup.name());
            admitted["epoch_rolling"] = json!(request.epoch_rolling);
            admitted["epoch_dispatch"] = json!(request.epoch_dispatch.name());
        }
    }
    if let Some(pause) = diagnostic_pause {
        admitted["diagnostic_pause"] = json!(pause.name());
    }
    // Record (the default) adds no key: its receipts stay byte-identical.
    if request.frontier_policy != OwnerDomainWalkFrontierPolicy::Record {
        admitted["frontier_policy"] = json!(request.frontier_policy.name());
    }
    let with_allowances = |mut event: Value| {
        event["requested_max_queries"] = json!(request.matching.max_queries);
        event["requested_max_query_bytes"] = json!(request.matching.max_query_bytes);
        super::storage::project(&mut event, arity);
        observer(event);
    };
    with_allowances(admitted);
    if request.publication_policy == OwnerDomainWalkPublicationPolicy::Epoch {
        macro_rules! dispatch_epoch { ($($n:literal),*) => { match rustred::campaign_storage_arity(arity ){
            $($n => epoch::run::<$n>(request, &selection, limits, &queries, cancellation, &with_allowances),)*
            _ => Err(crate::AppError::input("campaign arity is not compiled")),
        }} }
        let mut result: OwnerDomainWalkResult = {
            crate::ensure_runtime_arity(arity)?;
            rustred::with_app_runtime_arities!(dispatch_epoch)
        }?;
        super::storage::project(&mut result.document, arity);
        if let Some(records) = &mut result.records.0 {
            records.project_coordinates(arity);
        }
        result.document["requested_max_queries"] = json!(request.matching.max_queries);
        result.document["requested_max_query_bytes"] = json!(request.matching.max_query_bytes);
        return Ok(result);
    }
    macro_rules! dispatch { ($($n:literal),*) => { match rustred::campaign_storage_arity(arity ){
        $($n => run::<$n>(request, &selection, limits, &queries, cancellation, &with_allowances, diagnostic_pause),)*
        _ => Err(crate::AppError::input("campaign arity is not compiled")),
    }} }
    let mut result: OwnerDomainWalkResult = {
        crate::ensure_runtime_arity(arity)?;
        rustred::with_app_runtime_arities!(dispatch)
    }?;
    super::storage::project(&mut result.document, arity);
    if let Some(records) = &mut result.records.0 {
        records.project_coordinates(arity);
    }
    result.document["requested_max_queries"] = json!(request.matching.max_queries);
    result.document["requested_max_query_bytes"] = json!(request.matching.max_query_bytes);
    Ok(result)
}
fn mask<const N: usize>(owner: &[bool; N]) -> String {
    owner.iter().map(|&b| if b { '1' } else { '0' }).collect()
}

fn stats_json(s: OwnerAppliedStats) -> Value {
    json!({"selected_pieces":s.selected_pieces,"term_visits":s.term_visits,"shift_groups":s.shift_groups,
        "application_refinement_steps":s.application_refinement_steps,
        "application_refinement_cells":s.application_refinement_cells,
        "boundary_cells":s.boundary_cells,"sign_splits":s.sign_splits,"native_operations":s.native_operations,
        "correlation_empty_cells":s.correlation_empty_cells,
        "optional_coefficient_refusals":s.optional_coefficient_refusals,"optional_original_refusals":s.optional_original_refusals,
        "optional_coalesced_refusals":s.optional_coalesced_refusals,"coalescing_additions":s.coalescing_additions,
        "events":s.events,"successors":s.successors,"conditional_successors":s.conditional_successors,
        "same_support_successors":s.same_support_successors,"strict_subsupport_successors":s.strict_subsupport_successors,
        "unsupported_support_successors":s.unsupported_support_successors,
        "conditional_unsupported_support_successors":s.conditional_unsupported_support_successors,
        "problems":s.problems,"zero_terms":s.zero_terms,"cancelled_groups":s.cancelled_groups,"zero_sector_groups":s.zero_sector_groups,
        "matching":{"rules":s.matching.rules,"terminal_checks":s.matching.terminal_checks,"predicates":s.matching.predicates,
            "pieces":s.matching.pieces,"cells":s.matching.cells,"split_operations":s.matching.split_operations,
            "coordinate_cells":s.matching.coordinate_cells,"rank_empty_cells":s.matching.rank_empty_cells,
            "correlation_empty_cells":s.matching.correlation_empty_cells,
            "refinement_cells":s.matching.refinement_cells,"refinement_steps":s.matching.refinement_steps}})
}
fn limits_json(r: &OwnerDomainWalkRequest) -> Value {
    let a = r.applied_limits;
    let m = r.matching.match_limits;
    json!({"max_term_visits":a.max_term_visits,"max_shift_groups":a.max_shift_groups,
        "max_boundary_cells":a.max_boundary_cells,"max_sign_splits":a.max_sign_splits,
        "max_native_operations":a.max_native_operations,"max_events":a.max_events,
        "max_scratch_terms":a.max_scratch_terms,"max_scratch_boxes":a.max_scratch_boxes,
        "max_scratch_coordinate_cells":a.max_scratch_coordinate_cells,
        "cell_refinement":physical_parts::cell_refinement_json(a.cell_refinement),
        "matching":{"max_rules":m.max_rules,"max_terminal_checks":m.max_terminal_checks,
            "max_predicates":m.max_predicates,"max_pieces":m.max_pieces,"max_cells":m.max_cells,
            "max_split_operations":m.max_split_operations,"max_coordinate_cells":m.max_coordinate_cells,
            "max_bounded_refinement_cells":m.max_bounded_refinement_cells,
            "refinement_axes":matching::refinement_axes_name(m.refinement_axes),
            "guard_algebra":inspection::debug(&m.guard_algebra)}})
}

/// Stop policy only (Record receipts stay byte-identical): the policy, and
/// the stop reason when this session's frontier stop fired.
fn add_frontier_policy(document: &mut Value, request: &OwnerDomainWalkRequest, stopped: bool) {
    if request.frontier_policy == OwnerDomainWalkFrontierPolicy::Record {
        return;
    }
    document["frontier_policy"] = json!(request.frontier_policy.name());
    if stopped {
        document["stop_reason"] = json!(work_policy::FRONTIER_STOP_REASON);
    }
}

/// Both publication policies use the same post-load boundary. In particular,
/// owner partitioning and ledger/report finalization are not free setup work.
fn finish_timing(document: &mut Value, started: Instant, prepared: f64) {
    let elapsed = started.elapsed().as_secs_f64();
    document["prepared_seconds"] = json!(prepared);
    document["traversal_seconds"] = json!(elapsed - prepared);
    document["elapsed_seconds"] = json!(elapsed);
    document["traversal_timing_boundary"] = json!(
        "after_owner_preparation_through_initial_admission_walk_report_and_queue_cleanup; excludes_owner_unload_and_output_write"
    );
}

fn run<const N: usize>(
    request: &OwnerDomainWalkRequest,
    selection: &input::Selection,
    load_limits: crate::CandidateOwnerLoadLimits,
    queries: &[matching::input::Query],
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
    diagnostic_pause: Option<DiagnosticPause>,
) -> Result<OwnerDomainWalkResult, AppError> {
    let started = Instant::now();
    let mut checkpoint = checkpoint::Store::open(request).map_err(AppError::input)?;
    // Rescue amendments: parsed and chain-checked against the manifest before
    // any restore or owner import (`rescue.rs`).
    let amendments = request
        .amendments
        .iter()
        .map(|amendment| rescue::parse(amendment, selection.physical_arity()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::input)?;
    let first_new_amendment = match checkpoint.as_ref() {
        Some(store) if !amendments.is_empty() || !store.amendments().is_empty() => {
            if !amendments.is_empty() && !store.resumed_state() {
                return Err(AppError::input(
                    "rescue amendments require a resumed walk state, not a fresh walk or a bootstrap checkpoint",
                ));
            }
            rescue::check_chain(
                store.amendments(),
                &amendments,
                &checkpoint::rescue_chain_base(
                    request,
                    store.request_digest(),
                    store.g2_activation(),
                    store.amendments(),
                )
                .map_err(AppError::input)?,
                queries,
            )
            .map_err(AppError::input)?
        }
        _ => 0,
    };
    let latest_checkpoint =
        std::cell::RefCell::new(checkpoint.as_ref().and_then(|s| s.metadata()).cloned());
    let checkpoint_write = std::cell::RefCell::new(None::<Value>);
    let original_observer = observer;
    let enriched_observer = |mut event: Value| {
        if let Some(writing) = event.get("checkpoint_write") {
            *checkpoint_write.borrow_mut() = Some(writing.clone());
        }
        if event["event"] == "checkpoint_saved" {
            *checkpoint_write.borrow_mut() = None;
        }
        if let Some(writing) = checkpoint_write.borrow().as_ref() {
            event["checkpoint_write"] = writing.clone();
        }
        if let Some(metadata) = event.get("checkpoint") {
            *latest_checkpoint.borrow_mut() = Some(metadata.clone());
        } else if let Some(metadata) = latest_checkpoint.borrow().as_ref() {
            event["checkpoint"] = metadata.clone();
        }
        original_observer(event);
    };
    let observer = &enriched_observer;
    // Authenticate and decode before native owner import. No corrupted or
    // incompatible checkpoint is allowed to begin a new inspection.
    let restored = if let Some(store) = checkpoint.as_mut() {
        for event in store.take_open_events() {
            observer(event);
        }
        store.resume::<N>(observer).map_err(AppError::input)?
    } else {
        None
    };
    if let Some(store) = checkpoint.as_mut() {
        if let Some(event) = store.bootstrap().map_err(AppError::input)? {
            observer(event);
        }
    }
    let mut load = RoutedCampaignRequest::new(String::new(), String::new());
    load.workers = request.workers;
    load.owner_base = request.matching.owner_base.clone();
    load.reduction_limits = request.matching.reduction_limits;
    let reducer = if let Some(store) = checkpoint.as_mut() {
        let mut bind = |owners| store.bind_owners(owners);
        prepare::prepare_with_fingerprints::<N>(
            &load,
            selection,
            load_limits,
            cancellation,
            observer,
            Some(&mut bind),
        )?
    } else {
        prepare::prepare::<N>(&load, selection, load_limits, cancellation, observer)?
    };
    let prepared = started.elapsed().as_secs_f64();
    let mut queue = Queue::with_policy(
        request.max_domains,
        request.max_containment_checks,
        request.scheduling_policy,
    )
    .map_err(AppError::input)?;
    if request.publication_policy == OwnerDomainWalkPublicationPolicy::Ready {
        let OwnerDomainWalkSchedulingPolicy::TransferUnreserved { lookahead } =
            request.scheduling_policy
        else {
            unreachable!("validated Ready scheduling policy")
        };
        queue.delegation = Some(
            delegation::Ledger::new_ready(lookahead, request.max_domains)
                .map_err(|e| AppError::input(e.to_string()))?,
        );
    }
    if request.reuse_initial_d_bands && restored.is_none() {
        queue
            .delegation
            .as_mut()
            .expect("validated transfer policy")
            .begin_initial_admission()
            .map_err(|e| AppError::input(e.to_string()))?;
    }
    let mut inputs = Vec::new();
    let mut input_frontiers = Vec::new();
    let mut error = reducer
        .is_none()
        .then(|| "cancelled during preparation".to_owned());
    if let Some(reducer) = &reducer
        && restored.is_none()
    {
        for query in queries {
            let domain = Domain {
                phase: Phase::Apply,
                owner: rustred::storage_array(&query.owner, false).expect("validated arity"),
                lower: rustred::storage_array::<_, N>(&query.lower, 0)
                    .expect("validated query arity")
                    .to_vec(),
                upper: rustred::storage_array::<_, N>(&query.upper, Some(0))
                    .expect("validated query arity")
                    .to_vec(),
                rank: query.rank,
                powers: query.powers,
            };
            let domain = if request.route_domain_overcover
                && !reducer
                    .programs()
                    .owner_sectors()
                    .any(|o| o == &domain.owner)
            {
                if reducer.domain_routing_requires_source_conditions() {
                    if input_frontiers.len() == request.max_frontiers {
                        error = Some("retained frontier allowance".into());
                        break;
                    }
                    input_frontiers.push(json!({"id":query.id,"kind":"initial_route_source_validity_obligation",
                        "owner":mask(&domain.owner),"lower":domain.lower,"upper":domain.upper,"rank":domain.rank,
                        "power_bounds":power_bounds_json(domain.powers),
                        "reached_missing_rule_claim":false}));
                    inputs.push(
                        json!({"id":query.id,"domain":null,"source_validity_unresolved":true}),
                    );
                    continue;
                }
                Domain {
                    phase: Phase::Route,
                    ..domain
                }
            } else {
                domain
            };
            match queue.admit(domain) {
                Ok((id, _)) => inputs.push(json!({"id":query.id,"domain":id})),
                Err(e) => {
                    error = Some(e.into());
                    break;
                }
            }
        }
    }
    if request.reuse_initial_d_bands && restored.is_none() {
        queue
            .delegation
            .as_mut()
            .expect("validated transfer policy")
            .finish_initial_admission()
            .map_err(|e| AppError::input(e.to_string()))?;
    }
    // A failed initial admission must never become a successful walk over its
    // retained prefix. Preserve the existing incomplete-result path below.
    if request.publication_policy == OwnerDomainWalkPublicationPolicy::OwnerBatched
        && error.is_none()
        && let Some(reducer) = &reducer
    {
        let result = execution::owner_batches::run(
            &queue.expand_prefix(queue.domains.len()),
            input_frontiers.len(),
            queue.containment_checks,
            reducer,
            request,
            cancellation,
            observer,
        )
        .map_err(AppError::input)?;
        for input in &mut inputs {
            if let Some(id) = input["domain"].as_u64() {
                input["domain"] = result.initial_handles[id as usize].clone();
            }
        }
        let mut document = result.document;
        document["worker_allocation"] =
            worker_budget::WorkerBudget::for_request(request).json(request.inspection_workers);
        document["inputs"] = Value::Array(inputs);
        document["input_frontiers"] = Value::Array(input_frontiers);
        document["max_bounded_refinement_cells"] =
            json!(request.matching.match_limits.max_bounded_refinement_cells);
        drop(queue);
        finish_timing(&mut document, started, prepared);
        observer(OwnerDomainWalkResult::completion_progress(&document));
        return Ok(OwnerDomainWalkResult {
            all_scheduled_domains_resolved: document["all_scheduled_domains_resolved"] == true,
            document,
            records: OwnerDomainWalkRecords::default(),
        });
    }
    let mut state = execution::State::new(queue, input_frontiers.len(), error);
    let resumed = restored.is_some();
    if let Some(restored) = restored {
        state = restored.state;
        inputs = restored.inputs;
        input_frontiers = restored.input_frontiers;
    }
    // G2' log: started with the walk (before its first save); a restored
    // ledger carries the log iff its checkpoint was written with G2' on (the
    // request binding makes these agree).
    let g2_on = request.g2_residual_anchors != OwnerDomainWalkG2ResidualAnchors::Off;
    if resumed && g2_on && checkpoint.as_ref().is_some_and(|s| s.g2_activating()) {
        // Activation on a checkpoint written without G2' (recorded amendment).
        let mut event = state.g2_backfill().map_err(AppError::input)?;
        event["g2_activation"] = checkpoint
            .as_ref()
            .and_then(|s| s.g2_activation())
            .cloned()
            .unwrap_or(Value::Null);
        observer(event);
    }
    if let Some(ledger) = state.queue.delegation.as_mut() {
        if g2_on && !resumed {
            ledger.enable_g2(state.initial_domain_count);
        }
        if g2_on != ledger.g2().is_some() {
            return Err(AppError::input(
                "checkpoint G2' residual-anchor log disagrees with the request",
            ));
        }
    }
    // A10: the frontier stop fires on the first frontier committed beyond
    // this session's starting count: 0 for a fresh walk (so its initial
    // input frontiers fire before any inspection), the restored count on
    // resume (see `frontier_stop_checkpoint`).
    let mut frontier_trigger = (request.frontier_policy == OwnerDomainWalkFrontierPolicy::Stop)
        .then_some(if resumed { state.frontiers } else { 0 });
    let mut frontier_stopped = false;
    if let Some(reducer) = &reducer {
        if let Some(store) = checkpoint.as_mut() {
            // Frontier rescue (`rescue.rs`): quarantine the frontier taint and
            // admit the new amendments before the forced save persists them.
            if !amendments.is_empty() && state.error.is_none() {
                rescue::apply(
                    &mut state,
                    &mut inputs,
                    store,
                    reducer,
                    request.route_domain_overcover,
                    &amendments,
                    first_new_amendment,
                    observer,
                )
                .map_err(AppError::input)?;
            }
            // Records are streamed to the sidecar from the first commit on.
            store.attach_records(&state).map_err(AppError::input)?;
            if state.error.is_none() {
                if let Some(event) = store
                    .save_cancellable(
                        &state,
                        &inputs,
                        &input_frontiers,
                        checkpoint::SaveKind::Forced,
                        cancellation,
                        observer,
                    )
                    .map_err(AppError::input)?
                {
                    observer(event);
                }
                // Initial input frontiers of a fresh walk stop it here.
                frontier_stopped = frontier_stop_checkpoint(
                    &mut frontier_trigger,
                    store,
                    &state,
                    &inputs,
                    &input_frontiers,
                    cancellation,
                    observer,
                )
                .map_err(AppError::input)?;
            }
            let mut diagnostic_pause = diagnostic_pause;
            execution::run_checkpointed(
                &mut state,
                reducer,
                request,
                cancellation,
                observer,
                &mut |state| {
                    if diagnostic_checkpoint(
                        &mut diagnostic_pause,
                        store,
                        state,
                        &inputs,
                        &input_frontiers,
                        cancellation,
                        observer,
                    )? {
                        return Ok(());
                    }
                    if frontier_stop_checkpoint(
                        &mut frontier_trigger,
                        store,
                        state,
                        &inputs,
                        &input_frontiers,
                        cancellation,
                        observer,
                    )? {
                        frontier_stopped = true;
                        return Ok(());
                    }
                    if let Some(event) = store.save_cancellable(
                        state,
                        &inputs,
                        &input_frontiers,
                        checkpoint::SaveKind::Periodic,
                        cancellation,
                        observer,
                    )? {
                        observer(event);
                    }
                    Ok(())
                },
            );
            // Every frontier stop reports a paused, resumable session (exit
            // 4), also when the run loop never observed the cancellation it
            // raised: a stop on initial input frontiers with an empty
            // worklist, or on the walk's last commit. The final save below
            // then records `paused` (part of the change stamp).
            if frontier_stopped && state.error.is_none() {
                state.checkpoint_paused = true;
            }
            if state.error.is_none() {
                if let Some(event) = store
                    .save_cancellable(
                        &state,
                        &inputs,
                        &input_frontiers,
                        checkpoint::SaveKind::Final,
                        cancellation,
                        observer,
                    )
                    .map_err(AppError::input)?
                {
                    observer(event);
                }
            }
        } else {
            execution::run(&mut state, reducer, request, cancellation, observer);
        }
    }
    state.refresh_closure(state.report_cancellation(cancellation), true);
    if checkpoint.is_some() && (state.checkpoint_paused || reducer.is_none()) {
        // A checkpoint is the state; this receipt must not duplicate the full
        // retained queue and diagnostics (which can be many gigabytes).
        let mut document = json!({"schema":"rustred.owner-domain-walk.paused.json.v1","status":"paused",
            "resume_supported":true,"checkpoint":latest_checkpoint.borrow().clone(),
            "full_state_in_checkpoint":true,"independent_certification":false,
            "family_closure_claim":false,"all_scheduled_domains_resolved":false,"recursive_worklist_exhausted":false,
            "scheduled_nodes":state.queue.domains.len(),"completed_nodes":state.completed,"processed_nodes":state.published_count(),
            "queued_nodes":state.queue.domains.len()-state.published_count(),"committed_domains":state.published_count(),"committed_events":state.events,
            "contiguous_publication_watermark":state.queue.next,
            "events":state.events,"successors":state.successors,"conditional_successors":state.conditional,"frontiers":state.frontiers,
            "initial_entry_domains_total":state.initial_domain_count,"initial_entry_domains_inspected":state.initial_entry_domains_inspected,
            "initial_entry_domains_published":state.initial_published(),
            "pending_descendant_domains":state.pending_descendants(),
            "workers":request.workers,"preparation_interrupted":reducer.is_none(),
            "timing_scope":"this_process_session; canonical counters span checkpoint resumes"});
        document["parallel"] = std::mem::take(&mut state.parallel);
        document["publication_policy"] = json!(if state.ready() {
            "ready_ticket_stream"
        } else {
            "stable_domain_id_stream"
        });
        add_frontier_policy(&mut document, request, frontier_stopped);
        state.add_delegation_progress(&mut document);
        state.add_ready_progress(&mut document);
        state.add_g2_report(&mut document);
        document["descendant_closure"] = state.closure_json();
        add_rescue_report(
            &mut document,
            &state,
            checkpoint.as_ref(),
            queries,
            &amendments,
            &inputs,
        );
        drop(state);
        finish_timing(&mut document, started, prepared);
        observer(OwnerDomainWalkResult::completion_progress(&document));
        return Ok(OwnerDomainWalkResult {
            all_scheduled_domains_resolved: false,
            document,
            records: OwnerDomainWalkRecords::default(),
        });
    }
    let (delegation, resolutions) = state.finalize_delegation();
    let annotations = execution::records::Annotations::new(resolutions, &state.closure.borrow());
    let exhausted = state.error.is_none() && state.published_count() == state.queue.domains.len();
    let resolved = exhausted
        && state.frontiers == 0
        && delegation
            .as_ref()
            .is_none_or(|value| value["all_ledger_obligations_discharged"] == true);
    let mut document = json!({"schema":"rustred.owner-domain-walk.json.v2",
        "status":if resolved {"locally_resolved"} else {"incomplete"},
        "all_scheduled_domains_resolved":resolved,"recursive_worklist_exhausted":exhausted,
        "family_closure_claim":false,"ibp_generation":false,"routing_expanded":false,
        "route_domain_overcover":request.route_domain_overcover,
        "route_joint_source_support_pruning":request.route_joint_source_support_pruning,
        "routed_domains":state.routed,"route_masks":state.route_masks,
        "route_joint_support_masks_pruned":state.route_joint_support_masks_pruned,
        "max_scheduled_finite_rank":state.queue.max_finite_rank,"unbounded_rank_domains":state.queue.unbounded_rank_domains,
        "independent_certification":false,"resume_supported":false,
        "conditional_successors_use_conservative_domain_overcover":true,
        "scheduled_nodes":state.queue.domains.len(),"completed_nodes":state.completed,
        "queued_nodes":state.queue.domains.len().saturating_sub(state.published_count()),
        "processed_nodes":state.published_count(),"failed_nodes":state.native_records.saturating_sub(state.completed),
        "deduplication_hits":state.queue.deduplicated,"containment_checks":state.queue.containment_checks,
        "exact_domain_hits":state.queue.exact_hits,"full_orthant_hits":state.queue.orthant_hits,
        "successors":state.successors,"conditional_successors":state.conditional,
        "optional_coefficient_refusals":state.optional.total,"optional_original_refusals":state.optional.original,
        "optional_coalesced_refusals":state.optional.coalesced,
        "frontiers":state.frontiers,"events":state.events,
        "error":state.error,"prepared_seconds":prepared,
        "traversal_seconds":started.elapsed().as_secs_f64()-prepared,"elapsed_seconds":started.elapsed().as_secs_f64()});
    document["encountered_numerator_rank"] = state.queue.encountered_rank.json();
    add_rescue_report(
        &mut document,
        &state,
        checkpoint.as_ref(),
        queries,
        &amendments,
        &inputs,
    );
    // These trees can dominate campaign RAM. Move their allocations directly;
    // json!(mem::take(...)) would still serialize and clone every nested Value.
    document["inputs"] = Value::Array(inputs);
    document["input_frontiers"] = Value::Array(input_frontiers);
    // In-memory records are annotated and moved (never cloned) into the
    // report; sidecar records are annotated while the report is written.
    let records = match state
        .records
        .replace(execution::records::RecordSink::Memory(Vec::new()))
    {
        execution::records::RecordSink::Memory(mut rows) => {
            for row in &mut rows {
                annotations.apply(row);
            }
            document["domains"] = take_report_array(&mut rows);
            OwnerDomainWalkRecords::default()
        }
        execution::records::RecordSink::Sidecar(sidecar) => {
            document["domains"] = Value::Null;
            OwnerDomainWalkRecords(Some(RecordSource::Legacy(
                execution::records::Streamed::new(sidecar, annotations),
            )))
        }
    };
    // Keep macro expansion bounded without a crate-wide recursion allowance.
    document["workers"] = json!(request.workers);
    if checkpoint.is_some() {
        document["resume_supported"] = json!(true);
        document["checkpoint"] = latest_checkpoint.borrow().clone().unwrap_or(Value::Null);
        document["timing_scope"] =
            json!("this_process_session; canonical counters span checkpoint resumes");
    }
    document["initial_entry_domains_total"] = json!(state.initial_domain_count);
    document["initial_entry_domains_inspected"] = json!(state.initial_entry_domains_inspected);
    document["initial_entry_domains_published"] = json!(state.initial_published());
    document["pending_descendant_domains"] = json!(state.pending_descendants());
    document["descendant_closure"] = state.closure_json();
    document["worker_allocation"] =
        worker_budget::WorkerBudget::for_request(request).json(request.inspection_workers);
    if request.publication_policy == OwnerDomainWalkPublicationPolicy::OwnerBatched {
        document["requested_publication_policy"] = json!("owner_batched");
        document["owner_batched_traversal_started"] = json!(false);
    }
    document["job_local_reuse_hits"] = json!(state.job_local_reuse_hits);
    document["pre_admitted_orthant_hits"] = json!(state.pre_admitted_orthant_hits);
    document["pre_admitted_orthant_policy"] =
        json!("immutable_initial_admitted_phase_owner_rank; pending_not_completed");
    document["pre_admitted_orthant_limits"] = json!({"max_buckets":initial_orthants::MAX_BUCKETS,
        "max_logical_entry_bytes":initial_orthants::MAX_ENTRY_BYTES,
        "container_overhead_and_rss_excluded":true});
    document["job_local_reuse_policy"] =
        json!("per_inspection_after_ordered_emit; pending_not_completed");
    document["job_local_reuse_limits"] = json!({"max_keys":reuse::MAX_KEYS,
        "max_logical_key_bytes":reuse::MAX_KEY_BYTES,"container_overhead_and_rss_excluded":true});
    document["reuse_counter_scope"] = json!(
        "job_local and pre_admitted hits increment aggregate reuse only; skipped exact/orthant/general lookups are not attributed"
    );
    document["max_events"] = json!(request.max_events);
    document["max_frontiers"] = json!(request.max_frontiers);
    add_frontier_policy(&mut document, request, frontier_stopped);
    document["max_containment_checks"] = json!(request.max_containment_checks);
    document["containment_maintenance_checks"] = json!(state.queue.containment_maintenance_checks);
    document["containment_retired_candidates"] = json!(state.queue.containment_retired_candidates);
    document["containment_summary_builds"] = json!(state.queue.containment_summary_builds);
    document["containment_semantic_hits"] = json!(state.queue.containment_semantic_hits);
    document["containment_semantic_retirements"] =
        json!(state.queue.containment_semantic_retirements);
    document["containment_candidates"] = json!(state.queue.containment_candidate_count());
    document["containment_index_policy"] = json!(state.queue.containment_index_policy());
    document["containment_check_policy"] =
        json!("general_comparisons_only; null_is_unlimited; checked_counter");
    document["applied_limits"] = limits_json(request);
    document["bounded_refinement_axes"] = json!(matching::refinement_axes_name(
        request.matching.match_limits.refinement_axes
    ));
    document["max_bounded_refinement_cells"] =
        json!(request.matching.match_limits.max_bounded_refinement_cells);
    document["publication_policy"] = json!("stable_domain_id_stream");
    document["parallel"] = std::mem::take(&mut state.parallel);
    document["uncommitted_inspections"] = take_report_array(&mut state.uncommitted);
    document["successful_publication_matches_serial"] = json!(true);
    document["failure_or_cancellation_prefix_may_differ"] = json!(true);
    document["committed_domains"] = json!(state.published_count());
    document["contiguous_publication_watermark"] = json!(state.queue.next);
    document["committed_events"] = json!(state.events);
    if let Some(delegation) = delegation {
        document["schema"] = json!("rustred.owner-domain-walk.json.v3");
        document["publication_policy"] = json!("stable_domain_responsibility_stream");
        document["scheduling_policy"] =
            execution::scheduling_policy_json(request.scheduling_policy);
        document["native_processed_nodes"] = json!(state.native_records);
        document["delegation"] = delegation;
    }
    if state.ready() {
        document["schema"] = json!("rustred.owner-domain-walk.json.v5");
        document["publication_policy"] = json!("ready_ticket_stream");
        document["successful_publication_matches_serial"] = json!(false);
        document["queue_ids_and_receipt_order_depend_on_readiness"] = json!(true);
    }
    if request.reuse_initial_d_bands {
        document["reuse_initial_d_bands"] = json!(true);
        document["initial_overlap_index"] = index_report::render(
            state.initial_overlap_report,
            index_report::Scope::GlobalInitial,
        );
        document["partial_initial_inspections"] = json!(
            state
                .queue
                .delegation
                .as_ref()
                .map_or(0, |l| l.partial_initial_inspections())
        );
        document["partial_inspection_policy"] =
            json!("exact_initial_high_D_overlap; pinned_anchor_plus_native_residual");
        document["initial_overlap_limits"] = json!({"max_initial_domains":initial_overlap::MAX_INITIAL_DOMAINS,
            "count_scope":"initial_apply_only",
            "max_logical_entry_bytes":initial_overlap::MAX_ENTRY_BYTES,"container_overhead_and_rss_excluded":true});
    }
    state.add_g2_report(&mut document);
    drop(state);
    finish_timing(&mut document, started, prepared);
    observer(OwnerDomainWalkResult::completion_progress(&document));
    Ok(OwnerDomainWalkResult {
        all_scheduled_domains_resolved: resolved,
        document,
        records,
    })
}

/// Rescue blocks of an amended walk's report (`rescue.rs`): the amendment
/// chain, the quarantine size and the per-query certification through closed
/// containing input roots. Nothing is added to an unamended walk's report.
fn add_rescue_report<const N: usize>(
    document: &mut Value,
    state: &execution::State<N>,
    store: Option<&checkpoint::Store>,
    queries: &[matching::input::Query],
    amendments: &[rescue::Parsed],
    inputs: &[Value],
) {
    let Some(store) = store.filter(|store| !store.amendments().is_empty()) else {
        return;
    };
    document["amendments"] = rescue::chain_json(store.amendments());
    document["rescue_abandoned_domains_this_session"] = json!(state.rescue_abandoned);
    document["rescue_quarantined_domains"] = json!(
        (0..state.queue.domains.len())
            .filter(|&id| state.queue.is_quarantined(id))
            .count()
    );
    let mut ids = Vec::new();
    let mut domains = Vec::new();
    for query in queries
        .iter()
        .chain(amendments.iter().flat_map(|a| a.queries.iter()))
    {
        let Some(owner) = rustred::storage_array(&query.owner, false) else {
            return;
        };
        ids.push((query.id.as_str(), query.auxiliary));
        domains.push(Domain {
            phase: Phase::Apply,
            owner,
            lower: rustred::storage_array::<_, N>(&query.lower, 0)
                .expect("validated query arity")
                .to_vec(),
            upper: rustred::storage_array::<_, N>(&query.upper, Some(0))
                .expect("validated query arity")
                .to_vec(),
            rank: query.rank,
            powers: query.powers,
        });
    }
    let closure = state.closure.borrow();
    let total = state.queue.domains.len();
    document["query_certification"] = rescue::query_certification(
        &ids,
        &domains,
        inputs,
        |id| (id < total).then(|| state.queue.domain(id)),
        |id| closure.closed(id),
    );
}

fn take_report_array(values: &mut Vec<Value>) -> Value {
    Value::Array(std::mem::take(values))
}

#[cfg(test)]
mod policy_tests {
    use super::*;

    #[test]
    fn ready_requires_explicit_transfer_and_rejects_subdivision_before_loading() {
        for subdivision in [false, true] {
            let mut request = OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(
                String::new(),
                String::new(),
            ));
            request.publication_policy = OwnerDomainWalkPublicationPolicy::Ready;
            if subdivision {
                request.scheduling_policy = OwnerDomainWalkSchedulingPolicy::TransferUnreserved {
                    lookahead: std::num::NonZeroUsize::new(8).unwrap(),
                };
                request.apply_subdivision =
                    Some(OwnerDomainWalkApplySubdivision { axis: 0, cut: 1 });
            }
            let error = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {
                panic!("unsupported Ready policy must fail before loading owners")
            })
            .unwrap_err();
            let expected = if subdivision {
                "Apply subdivision requires ordered publication"
            } else {
                "Ready publication requires TransferUnreserved scheduling"
            };
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn diagnostic_pause_parses_one_known_trigger_and_refuses_unknown_values() {
        use std::ffi::OsStr;
        assert_eq!(DiagnosticPause::parse(None), Ok(None));
        assert_eq!(DiagnosticPause::parse(Some(OsStr::new(""))), Ok(None));
        assert_eq!(
            DiagnosticPause::parse(Some(OsStr::new("ready-multi-prefix"))),
            Ok(Some(DiagnosticPause::ReadyMultiPrefix))
        );
        for value in ["ready", "READY-MULTI-PREFIX", "ready-multi-prefix "] {
            let error = DiagnosticPause::parse(Some(OsStr::new(value))).unwrap_err();
            assert!(error.contains(DIAGNOSTIC_PAUSE_VARIABLE), "{error}");
        }
        // Only a Ready walk with a finished hole beyond two accepted prefixes
        // fires; an ordinary Ordered state never does.
        let state = execution::State::new(Queue::<1>::new(4, None), 0, None);
        assert!(!DiagnosticPause::ReadyMultiPrefix.fires(&state));
    }

    /// The process environment is never set here: concurrent in-process
    /// walks of other tests read the same variable.
    #[test]
    fn diagnostic_pause_admits_only_a_fresh_checkpointed_ready_walk() {
        let pause = Some(DiagnosticPause::ReadyMultiPrefix);
        for ready in [false, true] {
            for checkpoint in [None, Some(false), Some(true)] {
                let mut request = OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(
                    String::new(),
                    String::new(),
                ));
                if ready {
                    request.publication_policy = OwnerDomainWalkPublicationPolicy::Ready;
                }
                request.checkpoint = checkpoint.map(|resume| OwnerDomainWalkCheckpointOptions {
                    resume,
                    ..OwnerDomainWalkCheckpointOptions::new("unused")
                });
                assert_eq!(DiagnosticPause::admit(None, &request), Ok(()));
                let admitted = DiagnosticPause::admit(pause, &request);
                match (ready, checkpoint) {
                    (true, Some(false)) => assert_eq!(admitted, Ok(())),
                    (true, Some(true)) => {
                        let error = admitted.unwrap_err();
                        assert!(error.contains("unset it to --resume"), "{error}");
                    }
                    _ => {
                        let error = admitted.unwrap_err();
                        assert!(
                            error.contains("requires a checkpointed Ready walk"),
                            "{error}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn report_array_transfer_preserves_allocations_instead_of_cloning() {
        let mut records = vec![json!({"payload":"x".repeat(65_536)})];
        let vector_pointer = records.as_ptr();
        let string_pointer = records[0]["payload"].as_str().unwrap().as_ptr();
        let report = take_report_array(&mut records);
        assert!(records.is_empty());
        assert_eq!(report.as_array().unwrap().as_ptr(), vector_pointer);
        assert_eq!(
            report[0]["payload"].as_str().unwrap().as_ptr(),
            string_pointer
        );
    }

    #[test]
    fn subdivision_owner_batched_is_rejected_by_public_api_before_loading() {
        let mut request =
            OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(String::new(), String::new()));
        request.apply_subdivision = Some(OwnerDomainWalkApplySubdivision { axis: 0, cut: 1 });
        request.publication_policy = OwnerDomainWalkPublicationPolicy::OwnerBatched;
        let error = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {
            panic!("unsupported policy must fail before owner preparation")
        })
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Apply subdivision requires ordered publication")
        );
    }

    #[test]
    fn bounded_refinement_policy_reports_effective_match_limits_not_applied_defaults() {
        use rustred::solver::OwnerDomainRefinementAxes;
        let mut request =
            OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(String::new(), String::new()));
        assert_eq!(
            limits_json(&request)["matching"]["refinement_axes"],
            "inactive-only"
        );
        request.matching.match_limits.refinement_axes = OwnerDomainRefinementAxes::FiniteAxes;
        request.matching.match_limits.max_bounded_refinement_cells = 23;
        assert_eq!(
            limits_json(&request)["matching"]["refinement_axes"],
            "finite-axes"
        );
        assert_eq!(
            limits_json(&request)["matching"]["max_bounded_refinement_cells"],
            23
        );
        assert!(!request.route_domain_overcover);
        assert!(!request.route_joint_source_support_pruning);
        let document =
            json!({"bounded_refinement_axes":"finite-axes", "max_bounded_refinement_cells":23});
        let progress = OwnerDomainWalkResult::completion_progress(&document);
        assert_eq!(progress["bounded_refinement_axes"], "finite-axes");
        assert_eq!(progress["max_bounded_refinement_cells"], 23);
        assert_eq!(progress["family_closure_claim"], false);
    }

    #[test]
    fn initial_d_band_reuse_is_opt_in_and_requires_responsibility_ledger() {
        let mut request =
            OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(String::new(), String::new()));
        assert!(!request.reuse_initial_d_bands);
        request.reuse_initial_d_bands = true;
        let error = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {
            panic!("must reject before preparation")
        })
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("initial D-band reuse requires TransferUnreserved")
        );
        let progress = OwnerDomainWalkResult::completion_progress(
            &json!({"reuse_initial_d_bands":true,"partial_initial_inspections":7}),
        );
        assert_eq!(progress["reuse_initial_d_bands"], true);
        assert_eq!(progress["partial_initial_inspections"], 7);
    }

    #[test]
    fn explicit_domain_storage_budget_has_no_hidden_million_domain_ceiling() {
        for limit in [1_000_001, 10_000_000, usize::MAX] {
            let mut request = OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(
                r#"{"family_fingerprint":"unused","owners":[],"initial_frontier_routes":[]}"#
                    .into(),
                String::new(),
            ));
            request.max_domains = limit;
            let error = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {
                panic!("empty owner input must fail before loading or allocation")
            })
            .unwrap_err();
            assert!(error.to_string().contains("no owners"), "{error}");
        }
        let mut request =
            OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(String::new(), String::new()));
        request.max_domains = 0;
        let error = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {
            panic!("zero budget must fail before loading")
        })
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("invalid symbolic worklist allowances")
        );
    }

    #[test]
    fn containment_defaults_to_unlimited_and_zero_is_rejected_before_loading() {
        let mut request =
            OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(String::new(), String::new()));
        assert_eq!(request.max_containment_checks, None);
        request.max_containment_checks = Some(0);
        let error = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {
            panic!("invalid policy must be rejected before admission")
        })
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("invalid symbolic worklist allowances")
        );
    }

    #[test]
    fn completion_retains_effective_containment_policy_and_distinct_reuse_counter() {
        for limit in [None, Some(17)] {
            let document = json!({"max_containment_checks":limit,
                "containment_check_policy":"general_comparisons_only; null_is_unlimited; checked_counter",
                "containment_checks":19, "pre_admitted_orthant_hits":3,
                "containment_maintenance_checks":4, "containment_retired_candidates":2,
                "containment_summary_builds":11, "containment_semantic_hits":7,
                "containment_semantic_retirements":1,
                "containment_candidates":5, "containment_index_policy":"test_policy"});
            let completion = OwnerDomainWalkResult::completion_progress(&document);
            assert_eq!(completion["max_containment_checks"], json!(limit));
            assert_eq!(
                completion["containment_check_policy"],
                document["containment_check_policy"]
            );
            assert_eq!(completion["containment_checks"], 19);
            for key in [
                "containment_maintenance_checks",
                "containment_retired_candidates",
                "containment_summary_builds",
                "containment_semantic_hits",
                "containment_semantic_retirements",
                "containment_candidates",
                "containment_index_policy",
            ] {
                assert_eq!(completion[key], document[key]);
            }
            assert_eq!(completion["pre_admitted_orthant_hits"], 3);
        }
    }
}
