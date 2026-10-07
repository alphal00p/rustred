//! Publication policy `epoch`: walk semantics4 (W2.0 protocol plus S5).
//!
//! Inspectors run whole native inspections with the unchanged visitor; one
//! coordinator merges finished inspections in bulk and stays the only
//! mutator of the walk state. S2 is the lockstep skeleton: Lockstep depth 1
//! (B = 16 lowest Pending IDs per epoch, a constant independent of the
//! worker count; all results merged in one cut in ascending parent ID;
//! diagnostic seam `RUSTRED_EPOCH_LOCKSTEP_B`), every successor resolved in
//! the merge through the
//! kernel lane's ID-ordered index (canonical min-ID semantics), serial P1/P3,
//! closure through the legacy Tracker with forced refreshes only, typed
//! records in binary with an explicit diagnostic JSON view. P2 prepares
//! immutable buckets on reserved helpers. Checkpointed public runs use durable
//! lifecycle; checkpoint-free Memory runs retain the S2 full-result path.
//! Snapshot lookup is an explicit checkpoint-only comparison mode pending gates.
//! Soundness rests on the invariants S1-S7 of the protocol:
//! every edge target and every transfer holds a `Verified` token (`verify`),
//! ledger6 refuses every transition outside its table, a source seals only
//! after its complete sorted edge run.
#![forbid(unsafe_code)]

mod admission;
mod anchors;
mod checkpoint;
mod dispatch;
mod edges;
mod export;
mod g2;
mod inspector;
mod job;
mod ledger6;
mod merge;
pub(super) mod record_store;
pub(super) mod records;
mod rescue;
mod resolve;
mod snapshot;
mod state;
mod store;
#[cfg(test)]
mod tests;
mod verify;

use super::super::{RoutedCampaignRequest, input, matching, prepare};
use super::execution::records::Annotations;
use super::initial_overlap::InitialOverlapIndex;
use super::queue::{CompactDomain, Domain, Query};
use super::worker_budget::WorkerBudget;
use super::{
    OwnerDomainWalkFrontierPolicy, OwnerDomainWalkRecords, OwnerDomainWalkRequest,
    OwnerDomainWalkResult, OwnerDomainWalkSchedulingPolicy, index_report, limits_json,
};
use crate::AppError;
use dispatch::{Dispatch, Refill};
use ledger6::Tag;
use merge::{Fatal, MergeConfig, RecordOut, StopReason};
use record_store::{RecordSink, Sidecar, Streamed};
use serde_json::{Value, json};
use state::EpochState;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use verify::QueryImage;

pub(crate) use checkpoint::{EPOCH_WALK_CHECKPOINT_FORMAT, EPOCH_WALK_CHECKPOINT_SCHEMA};
/// Walk semantics of the epoch policy (the legacy policies stay at 1).
pub const EPOCH_WALK_SEMANTICS_VERSION: u32 = 4;
/// Lockstep epoch size B: a constant, independent of the worker count, so
/// that results are byte-identical across widths (IMP-11). The protocol
/// proposed 64; S2 uses 16 [M, fable51_w2_s2_2026-09-28.md §4]: the combined
/// four-loop controls `four-all` and `four-all-p5` drain at B = 1, 4, 8, 16
/// and 24 (29.8-29.9 k and 31.5 k natives) but enter the documented rank-13
/// (anchor + 1) flood on owner 0111110010 at B = 32 and 64, the same
/// non-drain signature as legacy Ready at W96. B is soundness-neutral and
/// determinism-neutral for a FIXED B (identity across widths), but it
/// changes the walk (which IDs are reserved together, hence transfers, IDs
/// and work volume): the identity oracle key is (request, B). B is recorded
/// in the result and the export (`epoch.schedule.b`); S3 must persist it in
/// CP6 and refuse a lockstep resume under another B.
pub(super) const LOCKSTEP_B: usize = 16;
/// Heartbeat spacing (observer events only; never a resolver input).
const HEARTBEAT_SECONDS: f64 = 5.0;
#[cfg(test)]
thread_local! {
    pub(super) static FORBID_LARGE_FINALIZATION: std::cell::Cell<bool> = const {std::cell::Cell::new(false)};
}
#[cfg(test)]
pub(super) fn assert_large_finalization_allowed() {
    FORBID_LARGE_FINALIZATION
        .with(|flag| assert!(!flag.get(), "CP6 entered a whole-state finalizer/census"));
}
/// Diagnostic seam, not a campaign knob: overrides the lockstep epoch size
/// B (1..=4096). It changes the walk (see `LOCKSTEP_B`); the value is
/// recorded (`epoch.schedule.b`, `b_override`), the identity tools compare
/// it, and the campaign supervisors strip it from their children
/// (`shared_owner_campaign.py` DIAGNOSTIC_ONLY_ENVIRONMENT). Read once per
/// walk; an invalid value is refused, never ignored.
pub(crate) const LOCKSTEP_B_VARIABLE: &str = "RUSTRED_EPOCH_LOCKSTEP_B";

pub(super) fn lockstep_b() -> Result<usize, String> {
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

/// Test seam for `consumer_stop_parity`: the resolver breaks with
/// `resolver_range` when this thread's job has emitted `at_event` events (a
/// W1 walk runs its inspections inline on the calling thread); None clears.
#[cfg(test)]
pub(in crate::application::routed_campaign) fn force_resolver_break(at_event: Option<u64>) {
    resolve::FORCED_BREAK.with(|f| f.set(at_event.map(|at| (at, job::BreakReason::ResolverRange))));
}

/// Unsupported execution extensions must also be refused by the offline
/// verifier, whose request otherwise need not pass execution admission.
pub(super) fn admit_extensions(request: &OwnerDomainWalkRequest) -> Result<(), AppError> {
    request
        .validate_epoch_inspector_lookup()
        .map_err(AppError::input)?;
    if request.g2_activate_on_resume {
        return Err(AppError::input(
            "epoch publication does not support G2' activation; start a fresh Union campaign",
        ));
    }
    if !request.amendments.is_empty()
        && request
            .checkpoint
            .as_ref()
            .is_some_and(|options| !options.resume)
    {
        return Err(AppError::input(
            "epoch rescue amendments require resuming a durable CP6 campaign",
        ));
    }
    Ok(())
}

/// Epoch-specific admission (refused lanes F20 and S2 limits); runs before
/// any input is parsed.
pub(super) fn admit(request: &OwnerDomainWalkRequest) -> Result<(), AppError> {
    admit_extensions(request)?;
    if !request.amendments.is_empty() && request.checkpoint.is_none() {
        return Err(AppError::input(
            "epoch rescue requires a durable CP6 checkpoint",
        ));
    }
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
            "epoch publication requires TransferUnreserved scheduling (transfers are part of Epoch semantics; the lookahead is unused)",
        ));
    }
    if request
        .checkpoint
        .as_ref()
        .is_some_and(|c| c.interval_seconds == 0)
    {
        return Err(AppError::input("checkpoint interval must be positive"));
    }
    Ok(())
}

/// Why initial admission stopped, mapped to its stop reason (§9.2).
#[derive(Debug)]
enum AdmissionError {
    /// The scheduled-domain cap (F9): `domain_allowance`.
    DomainCap,
    FrontierCap,
    /// An input whose canonical image or native summary is refused (a
    /// deterministic input error): `error_stop`.
    Refused(String),
    /// An allocation failure: `ram_guard`.
    Alloc(String),
    /// An engine inconsistency (index, digest or verify disagreement): C5.
    Internal(String),
}

impl AdmissionError {
    fn index(error: &str) -> Self {
        if error.contains("allocation") {
            AdmissionError::Alloc(error.to_owned())
        } else {
            AdmissionError::Internal(error.to_owned())
        }
    }
    /// The run's error text and its stop (Err: engine-fatal).
    fn stop(self) -> (String, Result<StopReason, String>) {
        match self {
            AdmissionError::DomainCap => (
                "scheduled domain allowance".into(),
                Ok(StopReason::DomainAllowance),
            ),
            AdmissionError::FrontierCap => (
                "retained frontier allowance".into(),
                Ok(StopReason::FrontierAllowance),
            ),
            AdmissionError::Refused(m) => (m, Ok(StopReason::ErrorStop)),
            AdmissionError::Alloc(m) => (m, Ok(StopReason::RamGuard)),
            AdmissionError::Internal(m) => (m.clone(), Err(m)),
        }
    }
}

struct Sink<'a>(&'a mut RecordSink);
impl RecordOut for Sink<'_> {
    fn reserve(&mut self) -> Result<(), String> {
        self.0.reserve_one()
    }
    fn push(&mut self, record: records::typed::Record) -> Result<(), String> {
        self.0.push(record)
    }
}

enum End {
    Drained,
    Stopped(StopReason),
}

/// Retain the immutable exact-ID role on the persisted query-to-root map.
/// Undeclared inputs stay required even when their name mentions a helper.
fn input_row(query: &matching::input::Query, domain: Option<u32>) -> Value {
    json!({"id":query.id,"domain":domain,
        "role":if query.auxiliary {"auxiliary"} else {"required"},
        "role_declared":query.role_declared})
}

/// Initial admission (S_0): each query in document order resolves exactly as
/// a merge-time miss against the store (exact, orthant, minimum live ID) or
/// becomes a new ID; a new initial ID retires the live IDs it contains from
/// the lookup index (nothing transfers: every initial ID is protected).
/// Every retirement holds a `Verified` token (the future ID's `Planned`
/// image contains the retired image), as P2's reverse sets do, so a retired
/// entry is always contained in a newer live entry of its bucket (the S1
/// premise S4's tiers rely on).
fn admit_initial<const N: usize>(
    state: &mut EpochState<N>,
    domain: &Domain<N>,
) -> Result<u32, AdmissionError> {
    admit_initial_with(state, domain, || Ok(()))
}

/// The no-op checkpoint is inlined in production; tests can fail the
/// existing reservation seams without allocating unbounded memory.
fn admit_initial_with<const N: usize>(
    state: &mut EpochState<N>,
    domain: &Domain<N>,
    mut checkpoint: impl FnMut() -> Result<(), &'static str>,
) -> Result<u32, AdmissionError> {
    use AdmissionError as E;
    if state.poisoned {
        return Err(E::Internal("initial admission: state is poisoned".into()));
    }
    state
        .store
        .ensure_unique()
        .map_err(|e| E::Internal(e.into()))?;
    let image =
        CompactDomain::try_from_domain(domain).map_err(|e| E::Refused(format!("input: {e}")))?;
    let q = QueryImage::new(image).map_err(|e| E::Refused(format!("input: {e}")))?;
    let query = Query::new(q.core.clone(), domain.phase);
    let published = state.store.len();
    if let Some((id, _, _)) = state
        .store
        .lookup(&q, &query, published, &mut state.lookup, &mut state.verify)
        .map_err(E::Internal)?
    {
        return Ok(id);
    }
    if state.store.len() >= state.id_cap() {
        return Err(E::DomainCap);
    }
    let retire = state
        .store
        .contained_live(&q, &query, &mut state.lookup)
        .map_err(E::Internal)?;
    checkpoint().map_err(E::index)?;
    state.reserve_ids(1).map_err(E::index)?;
    checkpoint().map_err(E::index)?;
    state.store.reserve_exact(&[q.digest]).map_err(E::index)?;
    let prepared = state
        .store
        .prepare_initial(&image, &query, &mut checkpoint)
        .map_err(E::index)?;
    let mut expected = 0;
    for &old in &retire {
        let old_q = QueryImage::new(state.store.domains[old as usize])
            .map_err(|e| E::Internal(format!("retired {old}: {e}")))?;
        verify::verify(
            verify::Container::Planned {
                pos: 0,
                survivors: 1,
                image: &image,
            },
            &old_q,
            &mut state.verify,
        )
        .ok_or_else(|| {
            E::Internal(format!(
                "initial admission: retired {old} is not contained in planned image (verify)"
            ))
        })?;
        if state.is_live(old) {
            expected += 1;
        }
    }
    checkpoint().map_err(E::index)?;

    // No fallible capacity/geometry preparation remains. Like P3, any
    // invariant error or panic in this commit window forbids persistence.
    state.poisoned = true;
    let id = state
        .store
        .push(image, query.compact, q.digest)
        .map_err(E::Internal)?;
    state.admit_id(id);
    for &old in &retire {
        state.set_live(old, false);
    }
    let removed = state
        .store
        .index_initial(id, &query, &retire, prepared)
        .map_err(|e| E::Internal(e.into()))?;
    if removed != expected {
        return Err(E::Internal(
            "initial admission index retirement mismatch".into(),
        ));
    }
    state.poisoned = false;
    Ok(id)
}

/// §16 certification: exit 0 `locally_resolved` only when drained with 0
/// frontiers, 0 NativeError, 0 Exhausted, no input frontier, no admission
/// error and the closure monitor available (F15). Returns (certified,
/// monitor available).
fn certification<const N: usize>(
    state: &EpochState<N>,
    drained: bool,
    admission_error: bool,
    input_frontiers: usize,
) -> (bool, bool) {
    // Availability is scalar. In particular the CP6 drain controller must not
    // enter Tracker::json's open-target storage census just to name its stop.
    let closure = state.tracker.counters();
    let monitor_available = closure.unavailable.is_none()
        && state.tracker.node_count() == state.store.len()
        && closure.initial == state.p0 as usize;
    let counts = state.ledger.counts();
    let certified = drained
        && !admission_error
        && state.counters.frontiers == 0
        && counts.get(Tag::NativeError) == 0
        && counts.get(Tag::Exhausted) == 0
        && input_frontiers == 0
        && monitor_available;
    (certified, monitor_available)
}

/// The anchor validators over the final state (the restore-side rules of
/// §7/§11.4 run on the engine's own output before it is exported): every
/// record passes R1-R3, the lent-scope rule, the exact cover and the
/// anchor-edge-present rule against the edge run log.
fn anchor_self_check<const N: usize>(state: &EpochState<N>) -> Result<(), String> {
    #[cfg(test)]
    assert_large_finalization_allowed();
    use anchors::{AnchorRecord, AnchorView, union_cover};
    if state.anchors.len() == 0 {
        return Ok(());
    }
    let mut runs: std::collections::HashMap<u32, Vec<u32>> = std::collections::HashMap::new();
    let log = state.edges.log();
    let mut at = 0;
    while at + 1 < log.len() {
        let (source, n) = (log[at], log[at + 1] as usize);
        let targets = log.get(at + 2..at + 2 + n).ok_or("edge log truncated")?;
        if state.anchors.get(source).is_some() {
            runs.insert(source, targets.to_vec());
        }
        at += 2 + n;
    }
    let domains = &state.store.domains;
    let same_bucket = |a: u32, b: u32| {
        (a as usize) < domains.len()
            && (b as usize) < domains.len()
            && store::bucket_key(&domains[a as usize]) == store::bucket_key(&domains[b as usize])
    };
    let record_of = |id: u32| {
        state
            .anchors
            .get(id)
            .map(|r| (r.kind, r.d_band().map(|(_, cut)| cut)))
    };
    let cut_of = |id: u32| {
        state
            .anchors
            .get(id)
            .and_then(AnchorRecord::d_band)
            .map(|(_, cut)| cut)
    };
    let edges_of = |id: u32| runs.get(&id).cloned();
    let eligible = |id, _v0| g2::eligible(domains, &state.ledger, &state.anchors, id);
    for record in state.anchors.records() {
        let node = domains
            .get(record.node as usize)
            .ok_or("anchor record beyond the store")?;
        let cover = |r: &AnchorRecord| union_cover(node, domains, r, &cut_of);
        let view = AnchorView {
            p0: state.p0,
            published_len: domains.len(),
            arity: N,
            ledger: &state.ledger,
            same_bucket: &same_bucket,
            record_of: &record_of,
            edges_of: Some(&edges_of),
            merged_view: Some(&eligible),
            cover: &cover,
        };
        let epoch = match state.ledger.get(record.node) {
            Ok(
                ledger6::Entry6::Native { epoch, .. }
                | ledger6::Entry6::NativeFrontier { epoch }
                | ledger6::Entry6::NativeError { epoch, .. },
            ) => epoch,
            _ => return Err(format!("anchor record on unmerged node {}", record.node)),
        };
        record
            .validate(&view, epoch)
            .map_err(|v| format!("anchor record of node {}: {v:?}", record.node))?;
    }
    Ok(())
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
    let lockstep = request
        .effective_epoch_cut_size()
        .map_err(AppError::input)?;
    // Durable CP6 and memory-only execution are explicit entry boundaries.
    // No hidden temporary checkpoint backs a memory-only request.
    let export_dir: Option<std::path::PathBuf> = None;
    let mut load = RoutedCampaignRequest::new(String::new(), String::new());
    load.workers = request.workers;
    load.owner_base = request.matching.owner_base.clone();
    load.reduction_limits = request.matching.reduction_limits;
    super::finite_replay::configure_load(request, &mut load);
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
            "full_result_in_output_document":false,"full_state_in_checkpoint":false,
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
    if request.checkpoint.is_some() {
        return checkpoint::run(
            request,
            &reducer,
            &owners,
            queries,
            lockstep,
            started,
            prepared,
            cancellation,
            observer,
        );
    }
    let mut state = EpochState::<N>::new(
        request.max_domains,
        request.max_events,
        request.max_frontiers,
    );
    // Initial admission (mirrors the legacy input handling).
    let mut inputs = Vec::new();
    let mut input_frontiers = Vec::new();
    let mut error: Option<String> = None;
    // The stop an admission failure maps to (Err: engine-fatal).
    let mut admission_stop: Option<Result<StopReason, String>> = None;
    for query in queries {
        let phase = admission::query_phase(&reducer, request.route_domain_overcover, query);
        match admission::one(&mut state, query, phase, &mut inputs, &mut input_frontiers) {
            Ok(()) => {}
            Err(e) => {
                let (message, stop) = e.stop();
                error = Some(message);
                admission_stop = Some(stop);
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
        g2: request.g2_residual_anchors == super::OwnerDomainWalkG2ResidualAnchors::Union,
        finite_replay: request.finite_replay,
    };
    if config.g2 {
        g2::enable(&mut state);
    }
    let g2_store = state.g2_store.clone();
    let traversal_started = Instant::now();
    let mut dispatch = Dispatch::new();
    let context = inspector::Context {
        reducer: &reducer,
        request,
        overlap: &overlap,
        cancellation,
        g2: g2_store.as_deref(),
        finite_account: None,
    };
    let job = |bytes: &[u8]| inspector::inspect_job(&context, bytes);
    let mut heartbeat = Instant::now();
    // Wall seconds per phase (indications only; never an input).
    let mut timing = [0f64; 5];
    let (obligations, retirements) = request
        .epoch_preparation_limits()
        .map_err(AppError::input)?;
    let preparation = merge::preparation::Engine::new(
        budget.helpers,
        merge::preparation::Limits {
            obligations,
            retirements,
        },
    )
    .map_err(|e| AppError::input(format!("epoch preparation startup: {e:?}")))?;
    let outcome: Result<End, Fatal> = if let Some(stop) = admission_stop {
        // A failed initial admission never becomes a walk over its prefix.
        stop.map(End::Stopped)
            .map_err(|m| Fatal(format!("initial admission: {m}")))
    } else if config.frontier_stop && !input_frontiers.is_empty() {
        // Input frontiers stop the run before the first dispatch (§9.3).
        Ok(End::Stopped(StopReason::FrontierStop))
    } else {
        inspector::with_pool(threads, &job, |run_batch| -> Result<End, Fatal> {
            loop {
                // One lockstep epoch: dispatch, inspect, P1-P3. A panic
                // anywhere in it is engine-fatal (§6.4: caught at the merge
                // loop; `state.poisoned` stays set if P3 was running).
                let step = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                    || -> Result<Option<End>, Fatal> {
                        if cancellation.load(Ordering::Acquire) {
                            return Ok(Some(End::Stopped(StopReason::Paused)));
                        }
                        let jobs = match dispatch.refill(&mut state, lockstep) {
                            Refill::Drained => return Ok(Some(End::Drained)),
                            Refill::SequenceExhausted => return Ok(Some(End::Stopped(StopReason::Capacity))),
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
                        let results = run_batch(batch).map_err(Fatal)?;
                        timing[0] += clock.elapsed().as_secs_f64();
                        let clock = Instant::now();
                        let checked = merge::p1_check(&mut state, results, config)?;
                        timing[1] += clock.elapsed().as_secs_f64();
                        if let Some(stop) = checked.stop {
                            merge::discard_cut(&mut state, &checked, &mut |id, a| {
                                dispatch.requeue(id, a)
                            })?;
                            return Ok(Some(End::Stopped(stop)));
                        }
                        let clock = Instant::now();
                        let prepared = match preparation.prepare_observed(&state, &checked,
                            || cancellation.load(Ordering::Acquire)) {
                            Ok(prepared) => prepared,
                            Err(error) => {
                                let stop = match error {
                                    merge::preparation::Error::Fatal(error) => return Err(error),
                                    merge::preparation::Error::Stopped => StopReason::Paused,
                                    merge::preparation::Error::RamGuard(_) => StopReason::RamGuard,
                                };
                                merge::discard_cut(&mut state, &checked, &mut |id, a| dispatch.requeue(id, a))?;
                                return Ok(Some(End::Stopped(stop)));
                            }
                        };
                        state.preparation.add(&prepared.metrics);
                        let plan = prepared.plan;
                        plan.counters.apply_to(&mut state);
                        timing[2] += clock.elapsed().as_secs_f64();
                        let clock = Instant::now();
                        if let Err(stop) =
                            merge::p3_preflight(&mut state, &checked, &plan, &mut Sink(&mut sink))
                        {
                            let stop = match stop {
                                merge::PreflightError::Stop(stop) => stop,
                                merge::PreflightError::Engine(error) => return Err(Fatal(error.into())),
                            };
                            merge::discard_cut(&mut state, &checked, &mut |id, a| {
                                dispatch.requeue(id, a)
                            })?;
                            return Ok(Some(End::Stopped(stop)));
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
                        Ok(applied.stop.map(End::Stopped))
                    },
                ));
                match step {
                    Ok(Ok(None)) => {}
                    Ok(Ok(Some(end))) => return Ok(end),
                    Ok(Err(fatal)) => return Err(fatal),
                    Err(panic) => {
                        let text = panic
                            .downcast_ref::<&str>()
                            .map(|s| s.to_string())
                            .or_else(|| panic.downcast_ref::<String>().cloned())
                            .unwrap_or_else(|| "non-text panic".into());
                        return Err(Fatal(format!(
                            "panic in the merge loop at k = {} (poisoned = {}): {text}",
                            state.k, state.poisoned
                        )));
                    }
                }
            }
        })
        .map_err(|e| Fatal(format!("inspector pool: {e}")))
        .and_then(|inner| inner)
    };
    // The engine's own restore-side anchor validators on the final state; a
    // poisoned state (P3 interrupted) is never exported.
    let outcome = outcome.and_then(|end| {
        if state.poisoned {
            return Err(Fatal("P3 did not complete (poisoned state)".into()));
        }
        anchor_self_check(&state)
            .map(|()| end)
            .map_err(|e| Fatal(format!("anchor self-check: {e}")))
    });
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
        "encountered_numerator_rank":state.store.encountered_rank.json(),
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
    let drained = matches!(end, End::Drained);
    let (certified, monitor_available) = certification(
        &state,
        drained,
        parts.error.is_some(),
        parts.input_frontiers.len(),
    );
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
    document["encountered_numerator_rank"] = state.store.encountered_rank.json();
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
    document["epoch"] = json!({"stage":"S5","preparation":state.preparation,
        "preparation_configuration":{"helpers":parts.budget.helpers,
            "obligations":request.epoch_preparation_limits().expect("validated").0,
            "retirements":request.epoch_preparation_limits().expect("validated").1},
        "preparation_scope":"invocation-local accepted P2 plans; includes later P3 capacity refusal",
        "schedule":{"kind":"lockstep","depth":1,"b":parts.lockstep,
            "b_default":LOCKSTEP_B,"b_override":parts.lockstep != LOCKSTEP_B,
            "b_semantics":"identity holds for a fixed B; B changes the walk (identity key = request and B)"},
        "resolution":"canonical_in_merge","k":state.k,"watermark":state.watermark(),"p0":state.p0,
        "ledger6":counts.json(),"records_digest":state.edges.records_digest(),
        "edge_digest":state.edges.edge_digest(),"edge_runs":state.edges.runs(),
        "edges":state.edges.edges(),"self_edges":state.edges.self_edges(),
        "anchors":state.anchors.len(),"merged_view":state.merged_view.len(),
        "counters":{"merges":c.merges,"dispatched":c.dispatched,"survivors":c.survivors,
            "miss_requests":c.miss_requests,"antichain_folded":c.antichain_folded,
            "job_local_duplicates":c.job_duplicates,"known_reuse":c.known_reuse,
            "transfers":c.transfers,"retired_lookup_only":c.retired_lookup_only,
            "discarded":c.discarded,"requeued":c.requeued,"partials":c.partials,
            "g2_records":c.g2_records},
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
        RecordSink::Memory(rows) => {
            let mut rows = rows
                .iter()
                .map(records::typed::Record::project)
                .collect::<Result<Vec<_>, _>>()
                .map_err(AppError::input)?;
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
            OwnerDomainWalkRecords(Some(super::RecordSource::Epoch(Streamed::new(
                sidecar,
                annotations,
            ))))
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
