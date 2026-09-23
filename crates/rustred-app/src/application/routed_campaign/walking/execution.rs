//! Single publisher for stable domain admission and bounded retained reports.
#[cfg(test)]
use super::queue::Phase;
use super::{
    OwnerDomainWalkRequest,
    diagnostics::{OptionalCounts, OptionalRefusals},
    initial_orthants::InitialOrthants,
    inspection::{self, Effect, Event, Finished, NativeStats},
    mask,
    parallel::{self, Failure, Poll},
    power_bounds_json,
    queue::Queue,
    stats_json,
};
use rustred::solver::RoutedCandidateReducer;
use serde_json::{Value, json};
use std::ops::ControlFlow;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

mod admission;
mod delegation;
pub(super) use delegation::scheduling_policy_json;

pub(super) struct State<const N: usize> {
    pub queue: Queue<N>,
    pub records: Vec<Value>,
    pub events: usize,
    pub successors: usize,
    pub conditional: usize,
    pub job_local_reuse_hits: usize,
    pub pre_admitted_orthant_hits: usize,
    pub optional: OptionalCounts,
    pub frontiers: usize,
    pub completed: usize,
    /// Actual native records, including failed partial publisher retention.
    /// Delegation cursor advances never increment this counter.
    pub native_records: usize,
    pub routed: usize,
    pub route_masks: usize,
    pub error: Option<String>,
    pub parallel: Value,
    pub uncommitted: Vec<Value>,
    details: Vec<Value>,
    refusals: OptionalRefusals,
    admission: admission::Metrics,
}
impl<const N: usize> State<N> {
    pub fn new(queue: Queue<N>, frontiers: usize, error: Option<String>) -> Self {
        Self {
            queue,
            records: Vec::new(),
            events: 0,
            successors: 0,
            conditional: 0,
            job_local_reuse_hits: 0,
            pre_admitted_orthant_hits: 0,
            optional: OptionalCounts::default(),
            frontiers,
            completed: 0,
            native_records: 0,
            routed: 0,
            route_masks: 0,
            error,
            parallel: json!({}),
            uncommitted: Vec::new(),
            details: Vec::new(),
            refusals: OptionalRefusals::default(),
            admission: admission::Metrics::default(),
        }
    }
    fn progress(&self, event: &str, id: usize, telemetry: &Value) -> Value {
        let domain = self.queue.domains.get(id);
        let mut progress = json!({"event":event, "operation":"owner_domain_walk", "id":id,
            "owner":domain.map(|d| mask(&d.owner)), "phase":domain.map(|d| format!("{:?}", d.phase)),
            "power_bounds":domain.map(|d| power_bounds_json(d.powers)),
            "scheduled_nodes":self.queue.domains.len(), "completed_nodes":self.completed,
            "committed_domains":self.queue.next, "commit_domain":id,
            "queued_nodes":self.queue.domains.len().saturating_sub(self.queue.next),
            "deduplication_hits":self.queue.deduplicated, "exact_domain_hits":self.queue.exact_hits,
            "full_orthant_hits":self.queue.orthant_hits, "containment_checks":self.queue.containment_checks,
            "containment_maintenance_checks":self.queue.containment_maintenance_checks,
            "containment_retired_candidates":self.queue.containment_retired_candidates,
            "containment_summary_builds":self.queue.containment_summary_builds,
            "containment_semantic_hits":self.queue.containment_semantic_hits,
            "containment_semantic_retirements":self.queue.containment_semantic_retirements,
            "containment_candidates":self.queue.containment_candidate_count(),
            "containment_index_policy":self.queue.containment_index_policy(),
            "max_containment_checks":self.queue.containment_limit(), "containment_check_policy":"general_comparisons_only; null_is_unlimited; checked_counter",
            "job_local_reuse_hits":self.job_local_reuse_hits,
            "pre_admitted_orthant_hits":self.pre_admitted_orthant_hits,
            "max_scheduled_finite_rank":self.queue.max_finite_rank, "unbounded_rank_domains":self.queue.unbounded_rank_domains,
            "successors":self.successors, "conditional_successors":self.conditional,
            "frontiers":self.frontiers, "events":self.events, "committed_events":self.events,
            "routed_domains":self.routed, "route_masks":self.route_masks, "parallel":self.enrich(telemetry.clone())});
        self.add_delegation_progress(&mut progress);
        progress
    }
    fn enrich(&self, mut telemetry: Value) -> Value {
        telemetry["admission_preparation"] = self.admission.json();
        if let Some(id) = telemetry["first_failure"]["domain"]
            .as_u64()
            .and_then(|id| usize::try_from(id).ok())
            && let Some(domain) = self.queue.domains.get(id)
        {
            let f = &mut telemetry["first_failure"];
            f["owner"] = json!(mask(&domain.owner));
            f["lower"] = json!(domain.lower);
            f["upper"] = json!(domain.upper);
            f["rank"] = json!(domain.rank);
            f["power_bounds"] = power_bounds_json(domain.powers);
        }
        telemetry
    }
    fn accept(
        &mut self,
        event: Event<N>,
        request: &OwnerDomainWalkRequest,
    ) -> Result<(), &'static str> {
        let remaining = request
            .max_events
            .checked_sub(self.events)
            .ok_or("event counter invariant")?;
        let reuse = match &event.effect {
            Effect::KnownReuse {
                successor,
                conditional,
            } => Some((*successor, *conditional, false)),
            Effect::PreAdmittedOrthantReuse {
                successor,
                conditional,
            } => Some((*successor, *conditional, true)),
            _ => None,
        };
        if let Some((successor, conditional, initial)) = reuse {
            let (old_hits, hit_overflow) = if initial {
                (
                    self.pre_admitted_orthant_hits,
                    "pre-admitted orthant counter overflow",
                )
            } else {
                (
                    self.job_local_reuse_hits,
                    "job-local reuse counter overflow",
                )
            };
            // Preserve the exact accepted logical prefix even inside a run.
            // Compute all next counters before any publication (including the
            // aggregate queue counter); an overflow leaves a coherent prefix.
            let mut accepted = event.count;
            let mut refusal = None;
            for (available, reason) in [
                (remaining, "aggregate successor event allowance"),
                (
                    if successor {
                        usize::MAX - self.successors
                    } else {
                        usize::MAX
                    },
                    "successor counter overflow",
                ),
                (
                    if conditional {
                        usize::MAX - self.conditional
                    } else {
                        usize::MAX
                    },
                    "conditional successor counter overflow",
                ),
                (usize::MAX - old_hits, hit_overflow),
                (
                    usize::MAX - self.queue.deduplicated,
                    "reuse counter overflow",
                ),
            ] {
                if available < accepted {
                    accepted = available;
                    refusal = Some(reason);
                }
            }
            let successors = self
                .successors
                .checked_add(if successor { accepted } else { 0 })
                .ok_or("successor counter overflow")?;
            let conditional = self
                .conditional
                .checked_add(if conditional { accepted } else { 0 })
                .ok_or("conditional successor counter overflow")?;
            let hits = old_hits.checked_add(accepted).ok_or(hit_overflow)?;
            self.queue.count_known_reuse(accepted)?;
            self.events += accepted;
            self.successors = successors;
            self.conditional = conditional;
            if initial {
                self.pre_admitted_orthant_hits = hits;
            } else {
                self.job_local_reuse_hits = hits;
            }
            return refusal.map_or(Ok(()), Err);
        }
        self.charge_event(event.count, request)?;
        match event.effect {
            Effect::Count => {}
            Effect::KnownReuse { .. } | Effect::PreAdmittedOrthantReuse { .. } => {
                unreachable!("handled counted reuse")
            }
            Effect::Admit {
                domain,
                successor,
                conditional,
            } => {
                self.apply_admission(successor, conditional, |queue| queue.admit(domain))?;
            }
            Effect::Frontier {
                value,
                successor,
                conditional,
            } => {
                self.successors += usize::from(successor);
                self.conditional += usize::from(conditional);
                if self.frontiers == request.max_frontiers {
                    return Err("retained frontier allowance");
                }
                self.details
                    .try_reserve(1)
                    .map_err(|_| "frontier allocation")?;
                self.frontiers += 1;
                self.details.push(value);
            }
            Effect::Optional(d) => self.refusals.record(
                d.disposition,
                d.rank,
                d.powers,
                &d.lower,
                &d.upper,
                &d.shift,
                d.ordinal,
                &rustred::algebra::IndexedAlgebraError::ResourceLimit {
                    resource: d.resource,
                    requested: d.requested,
                    limit: d.limit,
                },
            )?,
        }
        Ok(())
    }
    fn charge_event(
        &mut self,
        count: usize,
        request: &OwnerDomainWalkRequest,
    ) -> Result<(), &'static str> {
        let remaining = request
            .max_events
            .checked_sub(self.events)
            .ok_or("event counter invariant")?;
        if count > remaining {
            self.events += remaining;
            return Err("aggregate successor event allowance");
        }
        self.events += count;
        Ok(())
    }
    fn apply_admission(
        &mut self,
        successor: bool,
        conditional: bool,
        admit: impl FnOnce(&mut Queue<N>) -> Result<(usize, bool), &'static str>,
    ) -> Result<(), &'static str> {
        self.successors += usize::from(successor);
        self.conditional += usize::from(conditional);
        admit(&mut self.queue)?;
        Ok(())
    }
    fn commit(&mut self, id: usize, finished: Finished) {
        let domain = &self.queue.domains[id];
        let native_cancelled = finished.error_kind == "cancelled";
        self.error = self.error.take().or(finished.error);
        let (stats, optional, truncated) = match finished.stats {
            NativeStats::Apply(stats) => {
                if let Err(error) = self.optional.add(stats) {
                    self.error.get_or_insert_with(|| error.into());
                }
                (
                    stats_json(stats),
                    Some(std::mem::take(&mut self.refusals)),
                    Some(stats),
                )
            }
            NativeStats::Route(stats) => {
                self.routed += 1;
                self.route_masks += stats.masks_examined;
                (route_stats(stats), None, None)
            }
        };
        // Capture real frontiers before moving the per-inspection details.
        // An outer publisher error means the native stream was NOT completely
        // admitted, even if a later buffered Finished itself has no error.
        let frontier_count = self.details.len();
        if let Some(ledger) = &mut self.queue.delegation {
            use super::delegation::NativeOutcome;
            let outcome = if self.error.is_none() {
                NativeOutcome::Completed {
                    unresolved_frontiers: frontier_count,
                }
            } else if native_cancelled || self.error.as_deref() == Some("cancelled") {
                NativeOutcome::Cancelled
            } else {
                NativeOutcome::Failed
            };
            if let Err(error) = ledger.publish_native(id, outcome) {
                self.error.get_or_insert_with(|| error.to_string());
            }
        }
        self.native_records += 1;
        self.completed += usize::from(self.error.is_none());
        let mut record = json!({"id":id, "phase":format!("{:?}", domain.phase), "owner":mask(&domain.owner),
            "lower":domain.lower, "upper":domain.upper, "rank":domain.rank,
            "power_bounds":power_bounds_json(domain.powers),
            "local_inspection_finished":self.error.is_none(), "stats":stats, "seconds":finished.seconds,
            "frontiers":std::mem::take(&mut self.details), "error":self.error});
        if self.queue.delegation.is_some() {
            record["record_kind"] = json!("native_inspection");
            record["local_classification_discharged"] =
                json!(self.error.is_none() && frontier_count == 0);
        }
        if let (Some(optional), Some(stats)) = (optional, truncated) {
            record["optional_refusal_provenance_truncated"] = json!(optional.truncated(stats));
            record["optional_refusal_provenance_scope"] = json!("first_per_phase_per_query");
            record["optional_refusals"] = json!(optional.records);
        } else {
            record["conservative_route_overcover"] = json!(true);
        }
        self.records.push(record);
        self.queue.next += 1;
    }
}
fn route_stats(s: rustred::solver::CandidateDomainRouteStats) -> Value {
    json!({"masks_examined":s.masks_examined, "masks_pruned":s.masks_pruned,
        "events":s.events, "apply_domains":s.apply_domains,
        "route_domains":s.route_domains, "zero_sectors":s.zero_sectors, "missing_routes":s.missing_routes,
        "coordinate_cells":s.coordinate_cells})
}
fn native_stats(stats: NativeStats) -> Value {
    match stats {
        NativeStats::Apply(s) => stats_json(s),
        NativeStats::Route(s) => route_stats(s),
    }
}

pub(super) fn run<const N: usize>(
    state: &mut State<N>,
    reducer: &RoutedCandidateReducer<N>,
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
) {
    run_with_initial_orthants(state, reducer, request, cancellation, observer, true);
}

/// Private off/on reference seam, not a user-selectable applicability policy.
fn run_with_initial_orthants<const N: usize>(
    state: &mut State<N>,
    reducer: &RoutedCandidateReducer<N>,
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
    enabled: bool,
) {
    if state.error.is_some() {
        return;
    }
    // Capture actual initial admissions only. This immutable borrowed snapshot
    // is shared across scoped workers and never observes later queue growth.
    let initial = if enabled {
        InitialOrthants::from_initial(&state.queue.domains, cancellation)
    } else {
        InitialOrthants::empty()
    };
    if request.workers == 1 {
        return serial(state, reducer, request, cancellation, observer, &initial);
    }
    let budget = admission::WorkerBudget::new(request.workers, state.queue.containment_limit());
    state.admission = admission::Metrics::new(budget);
    let admission = match admission::Engine::new(budget) {
        Ok(engine) => engine,
        Err(error) => {
            state.error = Some(error.clone());
            state.parallel = state.enrich(json!({"workers":0, "active_workers":0,
                "first_failure":{"kind":"admission_worker_spawn", "detail":error}}));
            return;
        }
    };
    let mut dispatched = 0;
    let mut started_id = None;
    let mut heartbeat = Instant::now();
    let (_, snapshot, mut leftovers) = parallel::with_pool(
        budget.inspection,
        |domain, stop, emit| inspection::inspect(reducer, domain, request, stop, &initial, emit),
        |pool| {
            loop {
                if cancellation.load(Ordering::Acquire) {
                    pool.fail(Failure {
                        id: Some(state.queue.next),
                        phase: state.queue.domains.get(state.queue.next).map(|d| d.phase),
                        kind: "cancelled",
                        detail: "cancelled".into(),
                    });
                }
                if let Some(failure) = pool.failure() {
                    state.error = Some(failure.detail);
                    observer(state.progress("domain_progress", state.queue.next, &pool.snapshot()));
                    break;
                }
                if state.current_is_delegated() {
                    let id = state.queue.next;
                    if let Err(error) = state.commit_delegated() {
                        pool.fail(Failure {
                            id: Some(id),
                            phase: Some(state.queue.domains[id].phase),
                            kind: "delegation_publication",
                            detail: error,
                        });
                    } else {
                        observer(state.progress("domain_delegated", id, &pool.snapshot()));
                    }
                    continue; // Check cancellation between every delegated record.
                }
                while dispatched < state.queue.domains.len() {
                    if let Some(ledger) = &state.queue.delegation {
                        if dispatched >= ledger.dispatch_fence() {
                            break;
                        }
                        if ledger.delegated_to(dispatched).is_some() {
                            dispatched += 1;
                            continue;
                        }
                        if !ledger.can_dispatch(dispatched) {
                            pool.fail(Failure {
                                id: Some(dispatched),
                                phase: Some(state.queue.domains[dispatched].phase),
                                kind: "delegation_dispatch_invariant",
                                detail: "native dispatch has no reserved responsibility".into(),
                            });
                            break;
                        }
                    }
                    if !pool.dispatch(dispatched, state.queue.domains[dispatched].clone()) {
                        break;
                    }
                    if let Err(error) = state.note_native_started(dispatched) {
                        pool.fail(Failure {
                            id: Some(dispatched),
                            phase: Some(state.queue.domains[dispatched].phase),
                            kind: "delegation_native_start",
                            detail: error,
                        });
                        break;
                    }
                    dispatched += 1;
                }
                let id = state.queue.next;
                if id == state.queue.domains.len() {
                    break;
                }
                if started_id != Some(id) {
                    observer(state.progress("domain_started", id, &pool.snapshot()));
                    started_id = Some(id);
                    continue; // observe caller cancellation before publishing
                }
                match pool.poll(id) {
                    Poll::Events(chunk) => {
                        if let Err(error) = admission.commit_chunk(
                            state,
                            request,
                            chunk,
                            cancellation,
                            &pool.stop,
                            &mut |state| {
                                if heartbeat.elapsed() >= Duration::from_millis(250) {
                                    observer(state.progress(
                                        "domain_progress",
                                        id,
                                        &pool.snapshot(),
                                    ));
                                    heartbeat = Instant::now();
                                }
                            },
                        ) {
                            pool.fail(Failure {
                                id: Some(id),
                                phase: Some(state.queue.domains[id].phase),
                                kind: if error == "cancelled" {
                                    "cancelled"
                                } else {
                                    "coordinator_admission"
                                },
                                detail: error.into(),
                            });
                        }
                    }
                    Poll::Finished(finished) => state.commit(id, finished),
                    Poll::Waiting => pool.wait(id),
                }
                if heartbeat.elapsed() >= Duration::from_millis(250) {
                    observer(state.progress("domain_progress", state.queue.next, &pool.snapshot()));
                    heartbeat = Instant::now();
                }
                if state.error.is_some() {
                    pool.fail(Failure {
                        id: Some(id),
                        phase: state.queue.domains.get(id).map(|d| d.phase),
                        kind: "coordinator_accounting",
                        detail: state.error.clone().unwrap(),
                    });
                }
            }
            if pool.failure().is_some() {
                while !pool.wait_drained() {
                    observer(state.progress("domain_draining", state.queue.next, &pool.snapshot()));
                }
            }
        },
    );
    if state.error.is_none()
        && let Some(detail) = snapshot["first_failure"]["detail"].as_str()
    {
        state.error = Some(detail.to_owned());
    }
    state.parallel = state.enrich(snapshot);
    // Every native visitor has returned before reporting. Preserve the current
    // publisher's admitted prefix and keep later attempts explicitly separate.
    let before_retention = state.native_records;
    retain_leftovers(state, &mut leftovers);
    let retained_publisher = state.native_records - before_retention;
    for key in [
        "finished_uncommitted_domains",
        "dispatched_uncommitted_domains",
    ] {
        if let Some(value) = state.parallel[key].as_u64() {
            state.parallel[key] = json!(value - retained_publisher as u64);
        }
    }
}

fn retain_leftovers<const N: usize>(state: &mut State<N>, leftovers: &mut Vec<(usize, Finished)>) {
    leftovers.sort_by_key(|(id, _)| *id);
    let publisher_id = state.queue.next;
    for (id, finished) in leftovers.drain(..) {
        if id == publisher_id && state.error.is_some() {
            state.commit(id, finished);
        } else {
            let domain = &state.queue.domains[id];
            state.uncommitted.push(json!({"id":id, "phase":format!("{:?}", domain.phase),
                "owner":mask(&domain.owner), "lower":domain.lower, "upper":domain.upper, "rank":domain.rank,
                "power_bounds":power_bounds_json(domain.powers),
                "stats":native_stats(finished.stats), "error":finished.error, "seconds":finished.seconds,
                "committed":false}));
        }
    }
    if (!state.details.is_empty() || !state.refusals.records.is_empty())
        && let Some(domain) = state.queue.domains.get(publisher_id)
    {
        // A panicked worker has no Finished/stats. Retain already-published
        // diagnostics without inventing a completed inspection or zero stats.
        state.uncommitted.push(json!({"id":publisher_id, "phase":format!("{:?}", domain.phase),
            "owner":mask(&domain.owner), "lower":domain.lower, "upper":domain.upper, "rank":domain.rank,
            "power_bounds":power_bounds_json(domain.powers),
            "stats":null, "committed":false, "partial_publisher":true,
            "frontiers":std::mem::take(&mut state.details),
            "optional_refusals":std::mem::take(&mut state.refusals.records),
            "partial_native_statistics_unavailable":true}));
    }
}

fn serial<const N: usize>(
    state: &mut State<N>,
    reducer: &RoutedCandidateReducer<N>,
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
    initial: &InitialOrthants<N>,
) {
    let mut attempted = 0usize;
    let mut native = 0usize;
    let mut returned = 0usize;
    let mut failure = None;
    let mut heartbeat = Instant::now();
    while state.error.is_none() && state.queue.next < state.queue.domains.len() {
        if cancellation.load(Ordering::Acquire) {
            state.error = Some("cancelled".into());
            failure = Some(Failure {
                id: Some(state.queue.next),
                phase: state.queue.domains.get(state.queue.next).map(|d| d.phase),
                kind: "cancelled",
                detail: "cancelled".into(),
            });
            break;
        }
        let id = state.queue.next;
        if state.current_is_delegated() {
            if let Err(error) = state.commit_delegated() {
                state.error = Some(error);
                break;
            }
            observer(state.progress(
                "domain_delegated",
                id,
                &json!({"workers":1, "active_workers":0, "attempted_events":attempted}),
            ));
            continue;
        }
        if let Err(error) = state.note_native_started(id) {
            state.error = Some(error);
            break;
        }
        let domain = state.queue.domains[id].clone();
        observer(state.progress(
            "domain_started",
            id,
            &json!({"workers":1, "active_workers":1, "attempted_events":attempted}),
        ));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            inspection::inspect(
                reducer,
                &domain,
                request,
                cancellation,
                initial,
                &mut |event| {
                    let error = if let Some(next) = attempted.checked_add(event.count) {
                        attempted = next;
                        state.accept(event, request).err()
                    } else {
                        Some("attempted event counter overflow")
                    };
                    if let Some(error) = error {
                        state.error = Some(error.into());
                        failure = Some(Failure {
                            id: Some(id),
                            phase: Some(domain.phase),
                            kind: "coordinator_admission",
                            detail: error.into(),
                        });
                        return ControlFlow::Break(());
                    }
                    if heartbeat.elapsed() >= Duration::from_millis(250) {
                        observer(state.progress(
                            "domain_progress",
                            id,
                            &json!({"workers":1, "active_workers":1, "attempted_events":attempted}),
                        ));
                        heartbeat = Instant::now();
                    }
                    ControlFlow::Continue(())
                },
            )
        }));
        match result {
            Ok(finished) => {
                if failure.is_none()
                    && let Some(error) = &finished.error
                {
                    failure = Some(Failure {
                        id: Some(id),
                        phase: Some(domain.phase),
                        kind: finished.error_kind,
                        detail: error.clone(),
                    });
                }
                if let Some(next) = native.checked_add(finished.native_operations()) {
                    native = next;
                } else {
                    state.error.get_or_insert_with(|| {
                        "attempted native operation counter overflow".into()
                    });
                }
                returned += 1;
                state.commit(id, finished);
                if failure.is_none()
                    && let Some(error) = &state.error
                {
                    failure = Some(Failure {
                        id: Some(id),
                        phase: Some(domain.phase),
                        kind: "coordinator_accounting",
                        detail: error.clone(),
                    });
                }
            }
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }
    state.parallel = state.enrich(json!({"workers":1, "active_workers":0, "attempted_events":attempted,
        "returned_inspections":returned, "attempted_native_operations":native,
        "native_attempt_counters_scope":"returned_inspections_including_uncommitted_and_cancelled",
        "worker_buffered_events":0, "worker_buffered_logical_bytes":0, "peak_worker_buffered_logical_bytes":0,
        "backpressured_workers":0, "backpressure_seconds":0.0,
        "first_failure":failure.as_ref().map(Failure::json)}));
}

#[cfg(test)]
#[path = "execution/initial_orthants_tests.rs"]
mod initial_orthants_tests;
#[cfg(test)]
mod tests;
