//! Publication policy `epoch`: walk semantics 3 (W2.0 protocol, stage S2).
//!
//! Inspectors run whole native inspections with the unchanged visitor; one
//! coordinator merges finished inspections in bulk and stays the only
//! mutator of the walk state. S2 is the lockstep skeleton: Lockstep depth 1
//! (B = 64 lowest Pending IDs per epoch, all results merged in one cut in
//! ascending parent ID), every successor resolved in the merge through the
//! kernel lane's ID-ordered index (canonical min-ID semantics), serial P1-P3,
//! closure through the legacy Tracker with forced refreshes only, typed
//! records in their JSON view. No checkpoints (S3), no inspector-side
//! lookups (S4). Soundness rests on the invariants S1-S7 of the protocol:
//! every edge target and every transfer holds a `Verified` token (`verify`),
//! ledger6 refuses every transition outside its table, a source seals only
//! after its complete sorted edge run.
#![forbid(unsafe_code)]

mod anchors;
mod dispatch;
mod edges;
mod export;
mod inspector;
mod job;
mod ledger6;
mod merge;
mod records;
mod resolve;
mod state;
mod store;
#[cfg(test)]
mod tests;
mod verify;

use super::super::{RoutedCampaignRequest, input, matching, prepare};
use super::execution::records::{Annotations, RecordSink, Sidecar, Streamed};
use super::initial_overlap::InitialOverlapIndex;
use super::queue::{CompactDomain, Domain, Phase, Query};
use super::worker_budget::WorkerBudget;
use super::{
    OwnerDomainWalkFrontierPolicy, OwnerDomainWalkRecords, OwnerDomainWalkRequest,
    OwnerDomainWalkResult, OwnerDomainWalkSchedulingPolicy, index_report, limits_json, mask,
};
use crate::AppError;
use dispatch::{Dispatch, Refill};
use ledger6::Tag;
use matching::input::power_bounds_json;
use merge::{Fatal, MergeConfig, RecordOut, StopReason};
use serde_json::{Value, json};
use state::EpochState;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use verify::QueryImage;

/// Walk semantics of the epoch policy (the legacy policies stay at 1).
pub const EPOCH_WALK_SEMANTICS_VERSION: u32 = 3;
/// Lockstep epoch size B: a constant, independent of the worker count, so
/// that results are byte-identical across widths (IMP-11). The protocol
/// proposed 64; S2 uses 16 [M, fable51_w2_s2_2026-09-28.md]: the combined
/// four-loop controls `four-all` and `four-all-p5` drain at B = 1, 4, 8, 16
/// and 24 (29.8-29.9 k and 31.5 k natives) but enter the documented rank-13
/// (anchor + 1) flood on owner 0111110010 at B = 32 and 64, the same
/// non-drain signature as legacy Ready at W96. B is performance-only and not
/// bound; a wider lockstep front needs S4/S6 work, not a smaller front.
pub(super) const LOCKSTEP_B: usize = 16;
/// Heartbeat spacing (observer events only; never a resolver input).
const HEARTBEAT_SECONDS: f64 = 5.0;
/// Diagnostic seam, not a campaign knob: overrides the lockstep epoch size
/// B (1..=4096). B is performance-only and not bound (A7); a run is
/// identical across widths for any fixed B. Read once per walk; an invalid
/// value is refused, never ignored.
pub(crate) const LOCKSTEP_B_VARIABLE: &str = "RUSTRED_EPOCH_LOCKSTEP_B";

fn lockstep_b() -> Result<usize, String> {
    match std::env::var(LOCKSTEP_B_VARIABLE) {
        Err(std::env::VarError::NotPresent) => Ok(LOCKSTEP_B),
        Ok(value) if value.is_empty() => Ok(LOCKSTEP_B),
        Ok(value) => value
            .parse::<usize>()
            .ok()
            .filter(|b| (1..=4096).contains(b))
            .ok_or_else(|| {
                format!("{LOCKSTEP_B_VARIABLE}={value:?}: expected an integer in 1..=4096")
            }),
        Err(e) => Err(format!("{LOCKSTEP_B_VARIABLE}: {e}")),
    }
}

/// Epoch-specific admission (refused lanes F20 and S2 limits); runs before
/// any input is parsed.
pub(super) fn admit(request: &OwnerDomainWalkRequest) -> Result<(), AppError> {
    if request.max_containment_checks.is_some() {
        return Err(AppError::input(
            "epoch publication refuses a finite containment check allowance (F20)",
        ));
    }
    if request.apply_subdivision.is_some() {
        return Err(AppError::input(
            "epoch publication refuses physical Apply subdivision (F20)",
        ));
    }
    if !matches!(
        request.scheduling_policy,
        OwnerDomainWalkSchedulingPolicy::TransferUnreserved { .. }
    ) {
        return Err(AppError::input(
            "epoch publication requires TransferUnreserved scheduling (transfers are part of semantics 3; the lookahead is unused)",
        ));
    }
    if request.checkpoint.as_ref().is_some_and(|c| c.resume) {
        return Err(AppError::input(
            "epoch publication has no resumable checkpoint yet (CP6 is stage S3); --resume is refused",
        ));
    }
    Ok(())
}

struct Sink<'a>(&'a mut RecordSink);
impl RecordOut for Sink<'_> {
    fn reserve(&mut self) -> Result<(), String> {
        self.0.reserve_one()
    }
    fn push(&mut self, record: Value) -> Result<(), String> {
        self.0.push(record)
    }
}

enum End {
    Drained,
    Stopped(StopReason),
}

/// Initial admission (S_0): each query in document order resolves exactly as
/// a merge-time miss against the store (exact, orthant, minimum live ID) or
/// becomes a new ID; a new initial ID retires the live IDs it contains from
/// the lookup index (nothing transfers: every initial ID is protected).
fn admit_initial<const N: usize>(
    state: &mut EpochState<N>,
    domain: &Domain<N>,
) -> Result<u32, String> {
    let image = CompactDomain::try_from_domain(domain)?;
    let q = QueryImage::new(image)?;
    let query = Query::new(q.core.clone(), domain.phase);
    let published = state.store.len();
    if let Some((id, _, _)) =
        state
            .store
            .lookup(&q, &query, published, &mut state.lookup, &mut state.verify)?
    {
        return Ok(id);
    }
    if state.store.len() >= state.id_cap() {
        return Err("scheduled domain allowance".into());
    }
    let retire = state.store.contained_live(&q, &query, &mut state.lookup)?;
    state.reserve_ids(1)?;
    state.store.exact.try_reserve(&[q.digest])?;
    let id = state.store.push(image, query.compact, q.digest);
    state.admit_id(id);
    let mut expected = 0;
    for &old in &retire {
        if state.is_live(old) {
            expected += 1;
            state.set_live(old, false);
        }
    }
    let removed = state.store.index_survivor(id, &query, &retire)?;
    if removed != expected {
        return Err("initial admission index retirement mismatch".into());
    }
    Ok(id)
}

pub(super) fn run<const N: usize>(
    request: &OwnerDomainWalkRequest,
    selection: &input::Selection,
    load_limits: crate::CandidateOwnerLoadLimits,
    queries: &[matching::input::Query],
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<OwnerDomainWalkResult, AppError> {
    let started = Instant::now();
    let lockstep = lockstep_b().map_err(AppError::input)?;
    let export_dir = request.checkpoint.as_ref().map(|c| c.directory.clone());
    if let Some(directory) = &export_dir {
        export::prepare_directory(directory).map_err(AppError::input)?;
    }
    let mut load = RoutedCampaignRequest::new(String::new(), String::new());
    load.owner_base = request.matching.owner_base.clone();
    load.reduction_limits = request.matching.reduction_limits;
    let mut owners = Vec::new();
    let reducer = {
        let mut bind = |digests: Vec<String>| {
            owners = digests;
            Ok::<(), String>(())
        };
        prepare::prepare_with_fingerprints::<N>(
            &load,
            selection,
            load_limits,
            cancellation,
            observer,
            Some(&mut bind),
        )?
    };
    let prepared = started.elapsed().as_secs_f64();
    let budget = WorkerBudget::for_request(request);
    let Some(reducer) = reducer else {
        let mut document = json!({"schema":"rustred.owner-domain-walk.json.v6","status":"stopped",
            "stop_reason":"paused","preparation_interrupted":true,
            "all_scheduled_domains_resolved":false,"recursive_worklist_exhausted":false,
            "family_closure_claim":false,"publication_policy":"epoch_merge_stream",
            "walk_semantics_version":EPOCH_WALK_SEMANTICS_VERSION,"workers":request.workers});
        super::finish_timing(&mut document, started, prepared);
        observer(OwnerDomainWalkResult::completion_progress(&document));
        return Ok(OwnerDomainWalkResult {
            all_scheduled_domains_resolved: false,
            document,
            records: OwnerDomainWalkRecords::default(),
        });
    };
    let mut state = EpochState::<N>::new(
        request.max_domains,
        request.max_events,
        request.max_frontiers,
    );
    // Initial admission (mirrors the legacy input handling).
    let mut inputs = Vec::new();
    let mut input_frontiers = Vec::new();
    let mut error: Option<String> = None;
    for query in queries {
        let domain = Domain {
            phase: Phase::Apply,
            owner: query.owner.as_slice().try_into().expect("validated arity"),
            lower: query.lower.clone(),
            upper: query.upper.clone(),
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
                inputs.push(json!({"id":query.id,"domain":null,"source_validity_unresolved":true}));
                continue;
            }
            Domain {
                phase: Phase::Route,
                ..domain
            }
        } else {
            domain
        };
        match admit_initial(&mut state, &domain) {
            Ok(id) => inputs.push(json!({"id":query.id,"domain":id})),
            Err(e) => {
                error = Some(e);
                break;
            }
        }
    }
    state.p0 = state.watermark();
    state.tracker = super::descendant_closure::Tracker::new(state.p0 as usize);
    if !input_frontiers.is_empty() || error.is_some() {
        state
            .tracker
            .disable("initial input obligations were not completely admitted");
    }
    state.counters.frontiers = input_frontiers.len() as u64;
    observer(
        json!({"event":"epoch_initial_admission","operation":"owner_domain_walk",
        "initial_ids":state.p0,"queries":queries.len(),"input_frontiers":input_frontiers.len(),
        "family_closure_claim":false}),
    );
    let overlap = if request.reuse_initial_d_bands && error.is_none() {
        let prefix: Vec<Arc<Domain<N>>> = state.store.domains[..state.p0 as usize]
            .iter()
            .map(|image| Arc::new(image.expand()))
            .collect();
        InitialOverlapIndex::from_initial(&prefix, cancellation)
    } else {
        InitialOverlapIndex::empty()
    };
    let overlap_report = request
        .reuse_initial_d_bands
        .then(|| overlap.build_report());
    // Records: streamed into the export directory, or kept in RAM.
    let mut sink = match &export_dir {
        Some(directory) => RecordSink::Sidecar(Sidecar::new(directory.clone(), export::GENERATION)),
        None => RecordSink::Memory(Vec::new()),
    };
    let threads = if request.workers == 1 {
        0
    } else {
        budget.inspection
    };
    if threads > 0 && !symbolica::license::LicenseManager::is_licensed() {
        return Err(AppError::input(
            "parallel epoch inspectors require a Symbolica license (use --workers 1 otherwise)",
        ));
    }
    let config = MergeConfig {
        frontier_stop: request.frontier_policy == OwnerDomainWalkFrontierPolicy::Stop,
        lockstep: true,
    };
    let traversal_started = Instant::now();
    let mut dispatch = Dispatch::new();
    let context = inspector::Context {
        reducer: &reducer,
        request,
        overlap: &overlap,
        cancellation,
    };
    let mut heartbeat = Instant::now();
    // Wall seconds per phase (indications only; never an input).
    let mut timing = [0f64; 5];
    let outcome: Result<End, Fatal> = if error.is_some() {
        // A failed initial admission never becomes a walk over its prefix.
        Ok(End::Stopped(StopReason::DomainAllowance))
    } else if config.frontier_stop && !input_frontiers.is_empty() {
        // Input frontiers stop the run before the first dispatch (§9.3).
        Ok(End::Stopped(StopReason::FrontierStop))
    } else {
        inspector::with_pool(threads, &context, |run_batch| -> Result<End, Fatal> {
            loop {
                if cancellation.load(Ordering::Acquire) {
                    return Ok(End::Stopped(StopReason::Paused));
                }
                let jobs = match dispatch.refill(&mut state, lockstep) {
                    Refill::Drained => return Ok(End::Drained),
                    Refill::Stalled => {
                        return Err(Fatal(
                            "dispatch_stall: Pending or Reserved IDs but nothing dispatchable"
                                .into(),
                        ));
                    }
                    Refill::Jobs(jobs) => jobs,
                };
                let batch = jobs.iter().map(job::Job::encode).collect();
                let clock = Instant::now();
                let results = run_batch(batch);
                timing[0] += clock.elapsed().as_secs_f64();
                let clock = Instant::now();
                let checked = merge::p1_check(&mut state, results, config)?;
                timing[1] += clock.elapsed().as_secs_f64();
                if let Some(stop) = checked.stop {
                    merge::discard_cut(&mut state, &checked, &mut |id, a| dispatch.requeue(id, a))?;
                    return Ok(End::Stopped(stop));
                }
                let clock = Instant::now();
                let plan = merge::p2_plan(&mut state, &checked)?;
                timing[2] += clock.elapsed().as_secs_f64();
                let clock = Instant::now();
                if let Err(stop) =
                    merge::p3_preflight(&mut state, &checked, &plan, &mut Sink(&mut sink))
                {
                    merge::discard_cut(&mut state, &checked, &mut |id, a| dispatch.requeue(id, a))?;
                    return Ok(End::Stopped(stop));
                }
                let applied = merge::p3_apply(
                    &mut state,
                    checked,
                    plan,
                    config,
                    &records::Builder,
                    &mut Sink(&mut sink),
                    &mut |id, a| dispatch.requeue(id, a),
                )?;
                timing[3] += clock.elapsed().as_secs_f64();
                if heartbeat.elapsed().as_secs_f64() >= HEARTBEAT_SECONDS {
                    heartbeat = Instant::now();
                    observer(heartbeat_event(&state, &dispatch, traversal_started));
                }
                if let Some(stop) = applied.stop {
                    return Ok(End::Stopped(stop));
                }
            }
        })
        .map_err(|e| Fatal(format!("inspector pool: {e}")))
        .and_then(|inner| inner)
    };
    let end = match outcome {
        Ok(end) => end,
        Err(Fatal(message)) => {
            if let Some(directory) = &export_dir {
                let _ = export::write_poison(directory, state.k, &message);
            }
            observer(
                json!({"event":"epoch_engine_fatal","operation":"owner_domain_walk",
                "k":state.k,"error":message,"engine_certification_void":true,
                "family_closure_claim":false}),
            );
            return Err(AppError::internal_invariant(format!(
                "epoch engine-fatal (C5) at merge {}: {message}",
                state.k
            )));
        }
    };
    // The synchronous forced refresh at drain or final (§10.2, S2).
    state.tracker.refresh(&AtomicBool::new(false), true);
    finish(
        request,
        state,
        end,
        sink,
        export_dir,
        FinishParts {
            owners,
            inputs,
            input_frontiers,
            budget,
            overlap_report,
            started,
            prepared,
            traversal_started,
            queued: dispatch.queued(),
            error,
            timing,
            lockstep,
        },
        observer,
    )
}

fn heartbeat_event<const N: usize>(
    state: &EpochState<N>,
    dispatch: &Dispatch,
    started: Instant,
) -> Value {
    let counts = state.ledger.counts();
    json!({"event":"epoch_heartbeat","operation":"owner_domain_walk","k":state.k,
        "watermark":state.watermark(),"ledger6":counts.json(),
        "natives":state.counters.natives,"aliases":state.counters.aliases,
        "events":state.counters.events,"frontiers":state.counters.frontiers,
        "requeue":dispatch.queued().0,"deferred":dispatch.queued().1,
        "completed_nodes":state.counters.completed,"scheduled_nodes":state.watermark(),
        "queued_nodes":state.pending_or_reserved(),
        "traversal_seconds":started.elapsed().as_secs_f64(),"family_closure_claim":false})
}

struct FinishParts {
    owners: Vec<String>,
    inputs: Vec<Value>,
    input_frontiers: Vec<Value>,
    budget: WorkerBudget,
    overlap_report: Option<super::initial_overlap::InitialOverlapBuildReport>,
    started: Instant,
    prepared: f64,
    traversal_started: Instant,
    queued: (usize, usize),
    error: Option<String>,
    /// inspect (batch wall), P1, P2, P3 (+ preflight), unused
    timing: [f64; 5],
    lockstep: usize,
}

#[allow(clippy::too_many_arguments)]
fn finish<const N: usize>(
    request: &OwnerDomainWalkRequest,
    state: EpochState<N>,
    end: End,
    sink: RecordSink,
    export_dir: Option<std::path::PathBuf>,
    parts: FinishParts,
    observer: &impl Fn(Value),
) -> Result<OwnerDomainWalkResult, AppError> {
    let counts = state.ledger.counts();
    let total = state.store.len() as u64;
    let natives =
        counts.get(Tag::Native) + counts.get(Tag::NativeFrontier) + counts.get(Tag::NativeError);
    let published = natives + counts.get(Tag::Alias);
    let closure = state.tracker.json(state.store.len(), state.p0 as usize);
    let monitor_available = closure["available"] == true;
    let drained = matches!(end, End::Drained);
    let certified = drained
        && parts.error.is_none()
        && state.counters.frontiers == 0
        && counts.get(Tag::NativeError) == 0
        && counts.get(Tag::Exhausted) == 0
        && parts.input_frontiers.is_empty()
        && monitor_available;
    let stop_reason = match end {
        End::Drained if certified => None,
        End::Drained => Some(StopReason::DrainedUncertified),
        End::Stopped(reason) => Some(reason),
    };
    let (resolutions, delegation) = records::resolve(&state);
    let all_discharged = delegation["all_ledger_obligations_discharged"] == true;
    let resolved = certified && all_discharged;
    let status = if resolved {
        "locally_resolved"
    } else if drained {
        "incomplete"
    } else {
        "stopped"
    };
    let pending_descendants = (state.p0..state.watermark())
        .filter(|&id| matches!(state.tag(id), Tag::Pending | Tag::Reserved))
        .count();
    let initial_published = (0..state.p0)
        .filter(|&id| {
            matches!(
                state.tag(id),
                Tag::Native | Tag::NativeFrontier | Tag::NativeError | Tag::Alias
            )
        })
        .count();
    let unpublished_min = (0..state.watermark())
        .find(|&id| matches!(state.tag(id), Tag::Pending | Tag::Reserved | Tag::Exhausted))
        .unwrap_or(state.watermark());
    let c = state.counters;
    let lookup = state.lookup;
    let workers_json = parts.budget.json(request.inspection_workers);
    let checkpoint_meta = export_dir.as_ref().map(|directory| {
        json!({"format":export::FORMAT,"generation":export::GENERATION,"state":"saved",
            "paused":!drained,"resumable":false,"directory":directory,
            "pending_domains":state.pending_or_reserved(),"committed_domains":published,
            "completed_native_inspections":natives,"committed_events":c.events,
            "stop_reason":stop_reason.map(StopReason::name)})
    });
    let mut document = json!({"schema":"rustred.owner-domain-walk.json.v6","status":status,
        "all_scheduled_domains_resolved":resolved,"recursive_worklist_exhausted":drained,
        "family_closure_claim":false,"ibp_generation":false,"routing_expanded":false,
        "route_domain_overcover":request.route_domain_overcover,
        "route_joint_source_support_pruning":request.route_joint_source_support_pruning,
        "routed_domains":c.routed,"route_masks":c.route_masks,
        "route_joint_support_masks_pruned":c.route_joint_pruned,
        "max_scheduled_finite_rank":state.store.max_finite_rank,
        "unbounded_rank_domains":state.store.unbounded_rank_domains,
        "independent_certification":false,"resume_supported":false,
        "conditional_successors_use_conservative_domain_overcover":true,
        "scheduled_nodes":total,"completed_nodes":c.completed,
        "queued_nodes":total - published,"processed_nodes":published,
        "failed_nodes":counts.get(Tag::NativeError),
        "deduplication_hits":lookup.exact_hits + lookup.orthant_hits + lookup.contained_hits
            + c.antichain_folded + c.job_duplicates,
        "containment_checks":lookup.forward_candidates + lookup.reverse_candidates,
        "exact_domain_hits":lookup.exact_hits,"full_orthant_hits":lookup.orthant_hits,
        "successors":c.successors,"conditional_successors":c.conditional,
        "optional_coefficient_refusals":c.optional_total,"optional_original_refusals":c.optional_original,
        "optional_coalesced_refusals":c.optional_coalesced,
        "frontiers":c.frontiers,"events":c.events,"committed_events":c.events,
        "error":parts.error,"stop_reason":stop_reason.map(StopReason::name),
        "prepared_seconds":parts.prepared,
        "walk_semantics_version":EPOCH_WALK_SEMANTICS_VERSION});
    document["inputs"] = Value::Array(parts.inputs.clone());
    document["input_frontiers"] = Value::Array(parts.input_frontiers.clone());
    document["workers"] = json!(request.workers);
    document["worker_allocation"] = workers_json;
    document["initial_entry_domains_total"] = json!(state.p0);
    document["initial_entry_domains_inspected"] = json!(c.initial_inspected);
    document["initial_entry_domains_published"] = json!(initial_published);
    document["pending_descendant_domains"] = json!(pending_descendants);
    document["descendant_closure"] = closure.clone();
    document["job_local_reuse_hits"] = json!(c.known_reuse + c.job_duplicates);
    document["pre_admitted_orthant_hits"] = json!(0);
    document["max_events"] = json!(request.max_events);
    document["max_frontiers"] = json!(request.max_frontiers);
    document["frontier_policy"] = json!(request.frontier_policy.name());
    document["max_containment_checks"] = Value::Null;
    document["containment_maintenance_checks"] = json!(lookup.reverse_candidates);
    document["containment_retired_candidates"] = json!(c.transfers + c.retired_lookup_only);
    document["containment_index_policy"] = json!(
        "epoch_s2_canonical_in_merge: exact, dominant orthant, minimum live id of the id-ordered SoA index; verify chokepoint"
    );
    document["containment_check_policy"] = json!(
        "semantics 3: forward and reverse candidates handed to the index predicate in the merge"
    );
    document["applied_limits"] = limits_json(request);
    document["bounded_refinement_axes"] = json!(matching::refinement_axes_name(
        request.matching.match_limits.refinement_axes
    ));
    document["max_bounded_refinement_cells"] =
        json!(request.matching.match_limits.max_bounded_refinement_cells);
    document["publication_policy"] = json!("epoch_merge_stream");
    document["scheduling_policy"] =
        super::execution::scheduling_policy_json(request.scheduling_policy);
    document["native_processed_nodes"] = json!(natives);
    document["delegation"] = delegation;
    document["contiguous_publication_watermark"] = json!(unpublished_min);
    document["committed_domains"] = json!(published);
    document["uncommitted_inspections"] = json!([]);
    document["successful_publication_matches_serial"] = json!(false);
    document["parallel"] = json!({"workers":parts.budget.inspection,
        "inspector_threads":if request.workers == 1 {0} else {parts.budget.inspection},
        "merged_inspections":natives,"discarded_inspections":c.discarded,
        "returned_inspections":natives + c.discarded,"requeued_inspections":c.requeued,
        "active_workers":0,"occupied_native_slots":0,"dispatched_uncommitted_domains":0,
        "finished_uncommitted_domains":0,"worker_buffered_events":0,
        "worker_buffered_logical_bytes":0,"completed_escrow_entries":0,
        "first_failure":Value::Null,"non_cancellation_failure":Value::Null,
        "requeue_waiting":parts.queued.0,"deferred_waiting":parts.queued.1});
    if request.reuse_initial_d_bands {
        document["reuse_initial_d_bands"] = json!(true);
        document["initial_overlap_index"] =
            index_report::render(parts.overlap_report, index_report::Scope::GlobalInitial);
        document["partial_initial_inspections"] = json!(c.partials);
        document["partial_inspection_policy"] =
            json!("exact_initial_high_D_overlap; pinned_anchor_plus_native_residual");
    }
    document["epoch"] = json!({"stage":"S2","schedule":{"kind":"lockstep","depth":1,"b":parts.lockstep,
            "b_default":LOCKSTEP_B,"b_override":parts.lockstep != LOCKSTEP_B},
        "resolution":"canonical_in_merge","k":state.k,"watermark":state.watermark(),"p0":state.p0,
        "ledger6":counts.json(),"records_digest":state.edges.records_digest(),
        "edge_digest":state.edges.edge_digest(),"edge_runs":state.edges.runs(),
        "edges":state.edges.edges(),"self_edges":state.edges.self_edges(),
        "anchors":state.anchors.len(),"merged_view":state.merged_view.len(),
        "counters":{"merges":c.merges,"dispatched":c.dispatched,"survivors":c.survivors,
            "miss_requests":c.miss_requests,"antichain_folded":c.antichain_folded,
            "job_local_duplicates":c.job_duplicates,"known_reuse":c.known_reuse,
            "transfers":c.transfers,"retired_lookup_only":c.retired_lookup_only,
            "discarded":c.discarded,"requeued":c.requeued},
        "lookup":{"exact_hits":lookup.exact_hits,"orthant_hits":lookup.orthant_hits,
            "contained_hits":lookup.contained_hits,"misses":lookup.misses,
            "forward_candidates":lookup.forward_candidates,"forward_tests":lookup.forward_tests,
            "reverse_candidates":lookup.reverse_candidates,"reverse_tests":lookup.reverse_tests},
        "verify":state.verify.json(),"store":state.store.storage_json(),
        "certified":certified,"monitor_available":monitor_available,
        "engine_certification_void":false,
        "traversal_seconds":parts.traversal_started.elapsed().as_secs_f64(),
        "phase_wall":{"inspect_batches_seconds":parts.timing[0],"p1_seconds":parts.timing[1],
            "p2_seconds":parts.timing[2],"p3_seconds":parts.timing[3],
            "scope":"coordinator wall per phase, summed over merges (indication only)"}});
    let records = match sink {
        RecordSink::Memory(mut rows) => {
            let annotations = Annotations::new(Some(resolutions), &state.tracker);
            for row in &mut rows {
                annotations.apply(row);
            }
            document["domains"] = Value::Array(rows);
            OwnerDomainWalkRecords::default()
        }
        RecordSink::Sidecar(mut sidecar) => {
            let directory = export_dir.clone().expect("sidecar has an export directory");
            let segment = sidecar
                .seal(export::GENERATION, export::GENERATION + 1)
                .map_err(AppError::input)?;
            let records_json = json!(
                segment
                    .iter()
                    .map(|s| json!({"file":s.file,"first":s.first,
                "count":s.count,"bytes":s.bytes,"blake3":s.blake3}))
                    .collect::<Vec<_>>()
            );
            let metadata = checkpoint_meta.clone().expect("export metadata");
            export::write(
                &directory,
                &state,
                export::ExportParts {
                    binding: super::checkpoint::epoch_request_binding(request),
                    owners: &parts.owners,
                    metadata,
                    counters: json!({"events":c.events,"successors":c.successors,
                        "conditional":c.conditional,"job_local_reuse_hits":c.known_reuse + c.job_duplicates,
                        "pre_admitted_orthant_hits":0,"frontiers":c.frontiers,
                        "completed":c.completed,"natives":natives,"routed":c.routed,
                        "route_masks":c.route_masks,"initial":state.p0,
                        "initial_inspected":c.initial_inspected}),
                    inputs: &parts.inputs,
                    input_frontiers: &parts.input_frontiers,
                    closure: closure.clone(),
                    records: records_json,
                    extra: document["epoch"].clone(),
                },
            )
            .map_err(AppError::input)?;
            document["domains"] = Value::Null;
            let annotations = Annotations::new(Some(resolutions), &state.tracker);
            OwnerDomainWalkRecords(Some(Streamed::new(sidecar, annotations)))
        }
    };
    if let Some(metadata) = checkpoint_meta {
        document["checkpoint"] = metadata;
    }
    super::finish_timing(&mut document, parts.started, parts.prepared);
    observer(OwnerDomainWalkResult::completion_progress(&document));
    Ok(OwnerDomainWalkResult {
        all_scheduled_domains_resolved: resolved,
        document,
        records,
    })
}
