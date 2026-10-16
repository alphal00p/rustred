//! Input-controlled, process-local finite-demand measurement.
//! No checkpoint, publication, coefficient back-substitution or durable closure.
use super::super::{input::Selection, prepare, snapshot_json, walking::finite_scope};
use super::windows::{PrefixWindows, WindowLimits};
use crate::{AppError, AppErrorKind, OwnerDomainWalkRequest, RoutedCampaignRequest};
use rustred::family::IntegralKey;
use rustred::reduction::ReductionLimits;
use rustred::solver::{
    CandidateEntryAdmission, CandidateRoutedCampaignReport, CandidateRoutedCampaignSnapshot,
    CandidateRoutedCompletedTrace, CandidateRoutedCompletedTraceLimits,
    CandidateRoutedSelectionIdentity, CandidateRoutedTraceCensusLimits, CandidateRoutedWorkBudget,
    FiniteRootAdmission, RoutedCandidateReducer,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};
use symbolica::domains::integer::Integer;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Mode {
    SeparateWindows,
    SharedWindows,
    Compare,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SeedLimits {
    pub max_seed_points: usize,
    pub max_seed_bytes: usize,
    pub max_positive_layers: usize,
    pub max_partition_checks: usize,
}
impl From<SeedLimits> for WindowLimits {
    fn from(v: SeedLimits) -> Self {
        Self {
            max_seed_points: v.max_seed_points,
            max_seed_bytes: v.max_seed_bytes,
            max_positive_layers: v.max_positive_layers,
            max_partition_checks: v.max_partition_checks,
        }
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkLimits {
    pub max_nodes: usize,
    pub max_rule_applications: usize,
    pub max_transport_calls: usize,
    pub max_transport_operations: usize,
    pub max_transport_endpoints: usize,
    pub max_coalescing_additions: usize,
}
impl From<WorkLimits> for CandidateRoutedWorkBudget {
    fn from(v: WorkLimits) -> Self {
        Self {
            max_nodes: v.max_nodes,
            max_rule_applications: v.max_rule_applications,
            max_transport_calls: v.max_transport_calls,
            max_transport_operations: v.max_transport_operations,
            max_transport_endpoints: v.max_transport_endpoints,
            max_coalescing_additions: v.max_coalescing_additions,
        }
    }
}

/// Opt-in diagnostic aggregate policy, applied only after the original scope
/// and amendment chain have authenticated. No local algebra/scratch override.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReductionAggregateOverrides {
    pub max_rule_applications: usize,
    pub max_pending_frames: usize,
    pub max_coalescing_additions: usize,
}
impl ReductionAggregateOverrides {
    pub(super) fn apply(self, mut limits: ReductionLimits) -> ReductionLimits {
        limits.max_rule_applications = self.max_rule_applications;
        limits.max_pending_frames = self.max_pending_frames;
        limits.max_coalescing_additions = self.max_coalescing_additions;
        limits
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CensusLimits {
    #[serde(with = "super::lifecycle::policy::nullable_usize")]
    pub max_unique_selections: usize,
    #[serde(with = "super::lifecycle::policy::nullable_usize")]
    pub max_identity_payload_bytes: usize,
}
impl From<CensusLimits> for CandidateRoutedTraceCensusLimits {
    fn from(v: CensusLimits) -> Self {
        Self {
            max_unique_selections: v.max_unique_selections,
            max_identity_payload_bytes: v.max_identity_payload_bytes,
        }
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RetentionLimits {
    pub max_completed_work: usize,
    pub max_merge_payload_bytes: usize,
}
impl From<RetentionLimits> for CandidateRoutedCompletedTraceLimits {
    fn from(v: RetentionLimits) -> Self {
        Self {
            max_completed_work: v.max_completed_work,
            max_merge_payload_bytes: v.max_merge_payload_bytes,
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Policy {
    pub mode: Mode,
    pub workers: usize,
    pub windows: SeedLimits,
    /// Reset at each bounded native invocation, NOT a campaign-wide work cap.
    pub per_batch_work: WorkLimits,
    /// None preserves the source defaults and their native min-intersections.
    /// Zero is an explicit zero grant. MAX removes only that aggregate stop;
    /// checked counters, local native limits and external RSS/time guards remain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prepared_reduction_aggregates: Option<ReductionAggregateOverrides>,
    pub cumulative_census: CensusLimits,
    /// Per live capability: the whole shared side, but only the fresh current
    /// window in the separate control. Evidence grants bind both whole sides.
    pub cumulative_retention: RetentionLimits,
    pub max_total_rows: usize,
    pub max_source_bytes: usize,
    /// Per side, logical retained exact evidence payload (not allocator/RSS).
    /// Native session retention and final JSON allocations are additional.
    pub max_evidence_payload_bytes: usize,
}
pub(crate) struct Request<'a> {
    /// Read-only source/provenance carrier. Its checkpoint options are NOT run.
    pub source: &'a OwnerDomainWalkRequest,
    pub policy: &'a Policy,
}
pub(crate) struct ResultDocument {
    pub completed: bool,
    pub document: Value,
}

fn check(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Acquire) {
        Err("finite demand cancelled".into())
    } else {
        Ok(())
    }
}

pub(crate) fn run_with_progress(
    request: Request<'_>,
    cancel: &AtomicBool,
    observer: impl Fn(Value),
) -> Result<ResultDocument, AppError> {
    if !(1..=64).contains(&request.policy.workers) {
        return Err(AppError::input("finite demand workers must be in 1..=64"));
    }
    rustred::campaign::ParallelExecution::preflight_requested_core_budget(request.policy.workers)
        .map_err(|e| AppError::input(e.to_string()))?;
    let (selection, arity, load_limits) = Selection::parse_with_byte_limit(
        &request.source.matching.selection_json,
        request.policy.max_source_bytes,
    )?;
    let scope = finite_scope::parse(
        request.source,
        arity,
        request.policy.max_total_rows,
        request.policy.max_source_bytes,
    )?;
    crate::ensure_runtime_arity(arity)?;
    macro_rules! dispatch { ($($n:literal),*) => { match rustred::campaign_storage_arity(arity) {
        $($n => run::<$n>(&request,&selection,load_limits,scope,arity,cancel,&observer),)*
        _ => Err(AppError::input("finite demand arity is not compiled")),
    }} }
    rustred::with_app_runtime_arities!(dispatch)
}

#[derive(Default)]
struct Evidence {
    terminals: BTreeSet<IntegralKey>,
    zeros: BTreeSet<IntegralKey>,
    selections: BTreeMap<CandidateRoutedSelectionIdentity, u64>,
    payload: usize,
    identity_payload: usize,
    completed_totals: BTreeMap<&'static str, u64>,
    observed_totals: BTreeMap<&'static str, u64>,
}
fn add_counts<const N: usize>(
    totals: &mut BTreeMap<&'static str, u64>,
    s: &CandidateRoutedCampaignSnapshot<N>,
) -> Result<(), String> {
    for (name, n) in [
        ("input_occurrences", s.input_targets),
        ("requested_targets", s.requested_targets),
        ("operational_nodes", s.scheduled_nodes),
        ("physical_keys_per_batch", s.reachable_integrals),
        ("completed_baseline_hits", s.completed_baseline_hits),
        ("deduplication_hits", s.deduplication_hits),
        ("rule_attempts", s.rule_attempts),
        ("rule_applications", s.rule_applications),
        ("transport_calls", s.transport_calls),
        ("transport_operations", s.transport_operations),
        ("transport_endpoints", s.transport_endpoints),
        ("coalescing_additions", s.coalescing_additions),
    ] {
        let n = u64::try_from(n).map_err(|_| "work counter exceeds u64")?;
        let slot = totals.entry(name).or_default();
        *slot = slot.checked_add(n).ok_or("work counter sum overflow")?;
    }
    Ok(())
}
impl Evidence {
    fn absorb<const N: usize>(
        &mut self,
        report: &CandidateRoutedCampaignReport<N>,
        expected: usize,
        family: &str,
        policy: &Policy,
        cancel: &AtomicBool,
    ) -> Result<(), String> {
        check(cancel)?;
        let s = report.snapshot();
        if !s.finished
            || s.scheduled_nodes != s.completed_nodes
            || s.queued_nodes != 0
            || s.active_nodes != 0
            || s.failed_nodes != 0
            || s.first_failure.is_some()
            || s.reserved_coalescing_additions != 0
            || s.input_targets != expected
            || s.requested_targets != expected
            || s.missing_owners != 0
            || s.missing_rules != 0
            || !report.trace().frontier().is_empty()
            || report.trace().family_fingerprint() != family
        {
            return Err(
                "finite demand trace is incomplete, frontier-bearing or incorrectly bound".into(),
            );
        }
        let census = report
            .complete_census()
            .ok_or("finite demand lacks complete native census")?;
        let multiplicity = census
            .selections()
            .values()
            .try_fold(0u64, |a, b| a.checked_add(*b))
            .ok_or("census multiplicity overflow")?;
        if multiplicity
            != u64::try_from(s.rule_applications).map_err(|_| "rule count exceeds u64")?
        {
            return Err("native census multiplicity does not equal rule applications".into());
        }
        let mut bytes = self.payload;
        for (stored, delta) in [
            (&self.terminals, report.trace().declared_terminals()),
            (&self.zeros, report.trace().visited_zeros()),
        ] {
            for key in delta {
                check(cancel)?;
                if !stored.contains(key) {
                    bytes = bytes
                        .checked_add(std::mem::size_of::<IntegralKey>())
                        .and_then(|v| v.checked_add(key.powers().len().checked_mul(8)?))
                        .ok_or("evidence payload overflow")?;
                }
            }
        }
        let mut identities = self.selections.len();
        let mut identity_bytes = self.identity_payload;
        for (key, n) in census.selections() {
            check(cancel)?;
            if let Some(old) = self.selections.get(key) {
                old.checked_add(*n)
                    .ok_or("cumulative multiplicity overflow")?;
            } else {
                identities = identities.checked_add(1).ok_or("identity count overflow")?;
                let added = std::mem::size_of::<(CandidateRoutedSelectionIdentity, u64)>()
                    .checked_add(key.owner.len())
                    .ok_or("identity bytes overflow")?;
                identity_bytes = identity_bytes
                    .checked_add(added)
                    .ok_or("identity bytes overflow")?;
                bytes = bytes
                    .checked_add(added)
                    .ok_or("evidence payload overflow")?;
            }
        }
        if bytes > policy.max_evidence_payload_bytes
            || identities > policy.cumulative_census.max_unique_selections
            || identity_bytes > policy.cumulative_census.max_identity_payload_bytes
        {
            return Err("cumulative exact evidence allowance exceeded".into());
        }
        add_counts(&mut self.completed_totals, s)?;
        self.terminals
            .extend(report.trace().declared_terminals().iter().cloned());
        self.zeros
            .extend(report.trace().visited_zeros().iter().cloned());
        for (key, n) in census.selections() {
            *self.selections.entry(key.clone()).or_default() += n;
        }
        self.payload = bytes;
        self.identity_payload = identity_bytes;
        Ok(())
    }
    fn json(&self) -> Value {
        json!({"terminals":self.terminals.iter().map(IntegralKey::powers).collect::<Vec<_>>(),
            "visited_zeros":self.zeros.iter().map(IntegralKey::powers).collect::<Vec<_>>(),
            "selections":self.selections.iter().map(|(key,n)|json!({"owner":key.owner,
                "batch_ordinal":key.batch_ordinal,"baseline_rule_ordinal":key.baseline_rule_ordinal,
                "selected_rule_ordinal":key.selected_rule_ordinal,"multiplicity":n})).collect::<Vec<_>>(),
            "logical_payload_bytes":self.payload,"completed_work_totals":self.completed_totals,
            "observed_work_totals_including_failed_batch":self.observed_totals})
    }
}
struct Side {
    evidence: Evidence,
    rows: Vec<Value>,
    error: Option<String>,
    wall: f64,
    native_seconds: f64,
    call_seconds: f64,
}
impl Side {
    fn json(&self) -> Value {
        json!({"complete":self.error.is_none(),"error":self.error,
        "whole_side_seconds":self.wall,"native_snapshot_seconds_sum":self.native_seconds,
        "call_including_retention_seconds":self.call_seconds,
        "rows":self.rows,"evidence":self.evidence.json()})
    }
}

fn side<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    scope: &finite_scope::Scope,
    windows: &[PrefixWindows<N>],
    policy: &Policy,
    shared: bool,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
) -> Side {
    let started = Instant::now();
    let mut evidence = Evidence::default();
    let mut native_seconds = 0.0;
    let mut call_seconds = 0.0;
    let mut rows = scope
        .rows
        .iter()
        .enumerate()
        .map(|(ordinal, row)| {
            json!({"ordinal":ordinal,
        "id":row.query.id,"source":row.source,"complete":false,"status":"pending"})
        })
        .collect::<Vec<_>>();
    let mut session: Option<CandidateRoutedCompletedTrace<'_, N>> = None;
    let label = if shared {
        "shared_windows"
    } else {
        "separate_windows"
    };
    let family = reducer.programs().context().family().fingerprint();
    let outcome = (|| -> Result<(), String> {
        for (ordinal, (row, source)) in scope.rows.iter().zip(windows).enumerate() {
            check(cancel)?;
            let mut cursor = source.initial_cursor();
            let mut row_result = json!({"ordinal":ordinal,"id":row.query.id,"source":row.source,
                "expected_occurrences":source.total_count().to_string(),"complete":false,"windows":0,
                "completed_occurrences":"0","status":"pending"});
            let row_outcome = (|| -> Result<(), String> {
                while let Some(window) = source
                    .next_window(&cursor, cancel)
                    .map_err(|e| e.to_string())?
                {
                    check(cancel)?;
                    let admission = FiniteRootAdmission::try_new([window.region().clone()], 1)
                        .map_err(|e| e.to_string())?;
                    let progress = |s: &CandidateRoutedCampaignSnapshot<N>| {
                        let mut native = snapshot_json(s);
                        native["completed_baseline_hits"] = json!(s.completed_baseline_hits);
                        observer(json!({"event":"finite_demand_native",
                        "side":label,"row":ordinal,"window":cursor.windows(),"native":native}))
                    };
                    let call_started = Instant::now();
                    let report = if shared {
                        if let Some(session) = session.as_mut() {
                            session.trace_batch(
                                window.targets().iter().cloned(),
                                CandidateEntryAdmission::ExplicitFinite(&admission),
                                policy.workers,
                                cancel,
                                progress,
                            )
                        } else {
                            reducer
                                .trace_targets_completed_with_entry_admission_and_observer(
                                    window.targets().iter().cloned(),
                                    CandidateEntryAdmission::ExplicitFinite(&admission),
                                    policy.workers,
                                    policy.per_batch_work.into(),
                                    policy.cumulative_census.into(),
                                    policy.cumulative_retention.into(),
                                    cancel,
                                    progress,
                                )
                                .map(|(completed, report)| {
                                    session = Some(completed);
                                    report
                                })
                        }
                    } else {
                        // Same complete-only kernel/merge contract; no baseline survives this window.
                        reducer
                            .trace_targets_completed_with_entry_admission_and_observer(
                                window.targets().iter().cloned(),
                                CandidateEntryAdmission::ExplicitFinite(&admission),
                                policy.workers,
                                policy.per_batch_work.into(),
                                policy.cumulative_census.into(),
                                policy.cumulative_retention.into(),
                                cancel,
                                progress,
                            )
                            .map(|(_, report)| report)
                    };
                    call_seconds += call_started.elapsed().as_secs_f64();
                    let report = match report {
                        Ok(report) => report,
                        Err(error) => {
                            native_seconds += error.report().snapshot().elapsed.as_secs_f64();
                            add_counts(&mut evidence.observed_totals, error.report().snapshot())?;
                            return Err(error.to_string());
                        }
                    };
                    native_seconds += report.snapshot().elapsed.as_secs_f64();
                    add_counts(&mut evidence.observed_totals, report.snapshot())?;
                    evidence.absorb(&report, window.targets().len(), family, policy, cancel)?;
                    // In-memory only; durable adoption must later be atomic with closed evidence.
                    cursor = window.successor().clone();
                    row_result["windows"] = json!(cursor.windows());
                    row_result["completed_occurrences"] = json!(cursor.emitted().to_string());
                    observer(
                        json!({"event":"finite_demand_window_complete","side":label,"row":ordinal,
                        "windows":cursor.windows(),"completed_occurrences":cursor.emitted().to_string()}),
                    );
                }
                check(cancel)?;
                if !cursor.is_exhausted() || cursor.emitted() != source.total_count() {
                    return Err("finite prefix windows did not exhaust exact native count".into());
                }
                row_result["complete"] = json!(true);
                row_result["status"] = json!("complete");
                row_result["empty"] = json!(source.total_count() == &Integer::from(0));
                Ok(())
            })();
            rows[ordinal] = row_result;
            row_outcome?;
        }
        if let Some(completed) = &session {
            if completed.declared_terminals() != &evidence.terminals
                || completed.visited_zeros() != &evidence.zeros
                || completed.census().selections() != &evidence.selections
                || u64::try_from(completed.input_occurrences()).ok()
                    != evidence.completed_totals.get("input_occurrences").copied()
                || u64::try_from(completed.completed_work()).ok()
                    != evidence.completed_totals.get("operational_nodes").copied()
            {
                return Err(
                    "cumulative native capability and complete-window evidence differ".into(),
                );
            }
        }
        check(cancel)
    })();
    // The separate control drops each one-window capability inside its call.
    // Include the shared capability's final teardown in the whole-side clock.
    drop(session);
    Side {
        evidence,
        rows,
        error: outcome.err(),
        wall: started.elapsed().as_secs_f64(),
        native_seconds,
        call_seconds,
    }
}

fn run<const N: usize>(
    request: &Request<'_>,
    selection: &Selection,
    load_limits: crate::CandidateOwnerLoadLimits,
    scope: finite_scope::Scope,
    arity: usize,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<ResultDocument, AppError> {
    let started = Instant::now();
    let policy = request.policy;
    let mut load = RoutedCampaignRequest::new(String::new(), String::new());
    load.owner_base = request.source.matching.owner_base.clone();
    load.workers = policy.workers;
    load.reduction_limits = request.source.matching.reduction_limits;
    // run_with_progress already authenticated the ORIGINAL chain. Never mutate
    // that request or its digest to express this separate execution policy.
    if let Some(overrides) = policy.prepared_reduction_aggregates {
        load.reduction_limits = overrides.apply(load.reduction_limits);
    }
    // Only concrete aggregate slots change. Native local algebra/expansion stays unchanged.
    load.trace_limits.max_input_targets = policy.windows.max_seed_points;
    load.trace_limits.max_unique_nodes = policy.per_batch_work.max_nodes;
    load.trace_limits.max_transport_calls = policy.per_batch_work.max_transport_calls;
    load.trace_limits.max_transport_operations = policy.per_batch_work.max_transport_operations;
    load.trace_limits.max_transport_endpoints = policy.per_batch_work.max_transport_endpoints;
    let mut digests = Vec::new();
    let reducer = prepare::prepare_with_fingerprints::<N>(
        &load,
        selection,
        load_limits,
        cancel,
        observer,
        Some(&mut |values| {
            digests = values;
            Ok(())
        }),
    )?
    .ok_or_else(|| {
        AppError::new(
            AppErrorKind::Cancelled,
            "finite demand cancelled during preparation",
        )
    })?;
    let prepared_seconds = started.elapsed().as_secs_f64();
    let mut windows = Vec::new();
    let mut metadata = Vec::new();
    let mut occurrences = Integer::from(0);
    let preflight = Instant::now();
    for (ordinal, row) in scope.rows.iter().enumerate() {
        check(cancel).map_err(|e| AppError::new(AppErrorKind::Cancelled, e))?;
        let region = finite_scope::region(&reducer, request.source, row)?;
        let origin = json!({"request":scope.request_binding,"source":scope.sources[row.source],
            "ordinal":ordinal,"id":row.query.id,"auxiliary":row.query.auxiliary,
            "role_declared":row.query.role_declared})
        .to_string();
        let source = PrefixWindows::try_new(&origin, region, arity, policy.windows.into(), cancel)
            .map_err(|e| {
                AppError::new(
                    match e.kind {
                        super::windows::ErrorKind::Resource => AppErrorKind::Limit,
                        super::windows::ErrorKind::Cancelled => AppErrorKind::Cancelled,
                        super::windows::ErrorKind::Native => AppErrorKind::Execution,
                        _ => AppErrorKind::Input,
                    },
                    format!("finite demand query {}: {e}", row.query.id),
                )
            })?;
        occurrences += source.total_count();
        metadata.push(json!({"ordinal":ordinal,"id":row.query.id,"source":row.source,
            "auxiliary":row.query.auxiliary,"role_declared":row.query.role_declared,
            "owner":row.query.owner,"lower":row.query.lower,"upper":row.query.upper,
            "rank":row.query.rank,"powers":super::super::matching::input::power_bounds_json(row.query.powers),
            "native_count":source.total_count().to_string(),"empty":source.total_count()==&Integer::from(0)}));
        windows.push(source);
    }
    let preflight_seconds = preflight.elapsed().as_secs_f64();
    observer(
        json!({"event":"finite_demand_preflight_complete","rows":scope.rows.len(),
        "input_occurrences":occurrences.to_string(),"distinct_start_claim":false}),
    );
    let separate = matches!(policy.mode, Mode::SeparateWindows | Mode::Compare)
        .then(|| side(&reducer, &scope, &windows, policy, false, cancel, observer));
    let shared = if separate.as_ref().is_none_or(|v| v.error.is_none())
        && matches!(policy.mode, Mode::SharedWindows | Mode::Compare)
    {
        Some(side(
            &reducer, &scope, &windows, policy, true, cancel, observer,
        ))
    } else {
        None
    };
    let parity = match (&separate, &shared) {
        (Some(a), Some(b)) if a.error.is_none() && b.error.is_none() => Some(json!({
            "terminals":a.evidence.terminals==b.evidence.terminals,
            "visited_zeros":a.evidence.zeros==b.evidence.zeros,
            "composite_rule_identity_sets":a.evidence.selections.keys().eq(b.evidence.selections.keys()),
            "multiplicities_expected_equal":false})),
        _ => None,
    };
    let complete = check(cancel).is_ok()
        && match policy.mode {
            Mode::SeparateWindows => separate.as_ref().is_some_and(|v| v.error.is_none()),
            Mode::SharedWindows => shared.as_ref().is_some_and(|v| v.error.is_none()),
            Mode::Compare => parity.as_ref().is_some_and(|v| {
                v["terminals"] == true
                    && v["visited_zeros"] == true
                    && v["composite_rule_identity_sets"] == true
            }),
        };
    let mut document = json!({"schema":"rustred.finite-demand-diagnostic.v1","complete":complete,
        "authority":"process-local diagnostic only; no checkpoint, publication or durable closure authority",
        "family_closure_claim":false,"coefficient_backsubstitution":false,"work_checkpoint":false,
        "scope_request_binding":scope.request_binding,"sources":scope.sources,
        "scope_request_binding_is_execution_identity":false,
        "selection_blake3":blake3::hash(request.source.matching.selection_json.as_bytes()).to_hex().to_string(),
        "prepared_owner_payload_blake3":digests,"family_fingerprint":reducer.programs().context().family().fingerprint(),
        "physical_arity":arity,"storage_capacity":N,"policy":policy,"rows":metadata,
        "input_occurrences":occurrences.to_string(),"distinct_start_claim":false,
        "prepared_seconds":prepared_seconds,"preflight_seconds":preflight_seconds,
        "source_reduction_limits":format!("{:?}",request.source.matching.reduction_limits),
        "effective_reduction_limits":format!("{:?}",load.reduction_limits),
        "local_reduction_limits":format!("{:?}",load.reduction_limits),
        "effective_batch_aggregate_limits":{
            "max_input_targets":load.trace_limits.max_input_targets.min(policy.per_batch_work.max_nodes),
            "max_unique_nodes":load.trace_limits.max_unique_nodes.min(policy.per_batch_work.max_nodes),
            "max_rule_applications":load.reduction_limits.max_rule_applications.min(policy.per_batch_work.max_rule_applications),
            "max_pending_frames":load.reduction_limits.max_pending_frames.min(policy.per_batch_work.max_nodes),
            "max_coalescing_additions":load.reduction_limits.max_coalescing_additions.min(policy.per_batch_work.max_coalescing_additions),
            "max_transport_calls":load.trace_limits.max_transport_calls.min(policy.per_batch_work.max_transport_calls),
            "max_transport_operations":load.trace_limits.max_transport_operations.min(policy.per_batch_work.max_transport_operations),
            "max_transport_endpoints":load.trace_limits.max_transport_endpoints.min(policy.per_batch_work.max_transport_endpoints)},
        "local_expansion_limits":format!("{:?}",load.trace_limits.expansion),
        "fixed_comparison_order":["separate_windows","shared_windows"],"same_prepared_reducer":true,
        "separate":separate.as_ref().map(Side::json),"shared":shared.as_ref().map(Side::json),"exact_set_parity":parity});
    observer(json!({"event":"finite_demand_finished","complete":complete,"rows":scope.rows.len()}));
    // Cancellation during final evidence construction/observer never publishes success.
    let complete = complete && check(cancel).is_ok();
    document["complete"] = json!(complete);
    document["elapsed_seconds"] = json!(started.elapsed().as_secs_f64());
    Ok(ResultDocument {
        completed: complete,
        document,
    })
}

#[cfg(test)]
mod aggregate_policy_tests;
