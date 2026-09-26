//! Single publisher for stable domain admission and bounded retained reports.
#[cfg(test)]
use super::queue::Phase;
use super::{
    OwnerDomainWalkPublicationPolicy, OwnerDomainWalkRequest,
    diagnostics::{OptionalCounts, OptionalRefusals},
    initial_orthants::InitialOrthants,
    initial_overlap::{InitialOverlapBuildReport, InitialOverlapIndex},
    inspection::{self, Effect, Event, Finished, NativeStats},
    mask,
    parallel::{self, Failure, Poll},
    physical_parts::{Progress as PhysicalProgress, Ticket},
    power_bounds_json,
    queue::Queue,
    stats_json,
};
use rustred::solver::RoutedCandidateReducer;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::ops::ControlFlow;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

mod admission;
mod delegation;
pub(super) mod owner_batches;
mod publication;
mod replay;
pub(super) mod streams;
pub(super) use delegation::scheduling_policy_json;

/// Cheap summary of the persisted walk state that can change between saves.
/// Equal stamps mean the retained generation already holds this state.
/// Deliberately excluded: `parallel` telemetry and session timing (diagnostics
/// only); in-flight details, refusals and replay prefixes always arrive with
/// accepted events, which are stamped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ChangeStamp {
    pub domains: usize,
    pub published: usize,
    pub events: usize,
    pub closure_revision: u64,
    pub ledger_reserved_through: usize,
    pub ledger_transfers: usize,
    pub records_total: usize,
    /// Interrupted-inspection receipts persisted for the final report; a
    /// cooperative stop must save them even when nothing else moved.
    pub uncommitted: usize,
    /// Completed physical parts of the current subdivided parent; part 0 can
    /// finish without accepting any event.
    pub physical_parts_completed: usize,
    /// The retained manifest's `paused` flag must follow the run.
    pub paused: bool,
}

pub(super) struct State<const N: usize> {
    pub closure: RefCell<super::descendant_closure::Tracker>,
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
    pub route_joint_support_masks_pruned: usize,
    pub error: Option<String>,
    pub parallel: Value,
    pub uncommitted: Vec<Value>,
    pub initial_overlap_report: Option<InitialOverlapBuildReport>,
    pub initial_domain_count: usize,
    pub initial_entry_domains_inspected: usize,
    pub checkpoint_paused: bool,
    pub physical_inspections_published: usize,
    pub subdivided_logical_inspections: usize,
    physical_enabled: bool,
    pub(super) physical_progress: Option<PhysicalProgress>,
    pub(super) details: Vec<Value>,
    pub(super) refusals: OptionalRefusals,
    admission: admission::Metrics,
    replay: Option<replay::Replay>,
    pub(super) streams: streams::Streams,
}
impl<const N: usize> State<N> {
    fn parts(
        &self,
        id: usize,
        request: &OwnerDomainWalkRequest,
    ) -> Option<[std::sync::Arc<super::queue::Domain<N>>; 2]> {
        (id < self.initial_domain_count)
            .then(|| request.apply_subdivision?.parts(&self.queue.domains[id]))
            .flatten()
    }
    fn publisher_ticket(&self, request: &OwnerDomainWalkRequest) -> Ticket {
        let id = self.queue.next;
        let part =
            (id < self.queue.domains.len() && self.parts(id, request).is_some()).then(|| {
                self.physical_progress
                    .as_ref()
                    .map_or(0, |p| p.completed.len() as u8)
            });
        Ticket { parent: id, part }
    }
    pub fn new(queue: Queue<N>, frontiers: usize, error: Option<String>) -> Self {
        let initial_domain_count = queue.domains.len();
        let mut closure = super::descendant_closure::Tracker::new(initial_domain_count);
        if frontiers != 0 || error.is_some() {
            closure.disable("initial input obligations were not completely admitted");
        }
        Self {
            closure: RefCell::new(closure),
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
            route_joint_support_masks_pruned: 0,
            error,
            parallel: json!({}),
            uncommitted: Vec::new(),
            initial_overlap_report: None,
            initial_domain_count,
            initial_entry_domains_inspected: 0,
            checkpoint_paused: false,
            physical_inspections_published: 0,
            subdivided_logical_inspections: 0,
            physical_enabled: false,
            physical_progress: None,
            replay: None,
            streams: streams::Streams::default(),
            details: Vec::new(),
            refusals: OptionalRefusals::default(),
            admission: admission::Metrics::default(),
        }
    }
    pub(super) fn change_stamp(&self) -> ChangeStamp {
        let ledger = self.queue.delegation.as_ref();
        ChangeStamp {
            domains: self.queue.domains.len(),
            published: self.published_count(),
            events: self.events,
            closure_revision: self.closure.borrow().revision(),
            ledger_reserved_through: ledger.map_or(0, |l| l.reservation_scan()),
            ledger_transfers: ledger.map_or(0, |l| l.transfers()),
            records_total: self.records.len(),
            uncommitted: self.uncommitted.len(),
            physical_parts_completed: self
                .physical_progress
                .as_ref()
                .map_or(0, |progress| progress.completed.len()),
            paused: self.checkpoint_paused,
        }
    }
    pub(super) fn checkpoint_progress_metadata(&self) -> Value {
        json!({"physical_enabled":self.physical_enabled,
            "replay":self.replay.as_ref().map(replay::Replay::snapshot),
            "physical_inspections_published":self.physical_inspections_published,
            "subdivided_logical_inspections":self.subdivided_logical_inspections})
    }
    #[cfg(test)]
    fn checkpoint_progress(&self) -> Value {
        let mut metadata = self.checkpoint_progress_metadata();
        metadata["physical_parent"] = json!(self.physical_progress);
        metadata
    }
    pub(super) fn restore_checkpoint_progress(&mut self, mut value: Value) -> Result<(), String> {
        self.physical_enabled = value["physical_enabled"]
            .as_bool()
            .ok_or("missing physical policy")?;
        self.replay = if value["replay"].is_null() {
            None
        } else {
            Some(replay::Replay::restore(value["replay"].clone())?)
        };
        self.physical_progress = serde_json::from_value(
            value
                .as_object_mut()
                .ok_or("invalid checkpoint progress")?
                .remove("physical_parent")
                .ok_or("missing physical parent progress")?,
        )
        .map_err(|e| e.to_string())?;
        if self
            .physical_progress
            .as_ref()
            .is_some_and(|p| p.parent != self.queue.next || p.completed.len() > 1)
        {
            return Err("invalid physical checkpoint prefix".into());
        }
        self.physical_inspections_published = value["physical_inspections_published"]
            .as_u64()
            .and_then(|v| usize::try_from(v).ok())
            .ok_or("invalid physical publication count")?;
        self.subdivided_logical_inspections = value["subdivided_logical_inspections"]
            .as_u64()
            .and_then(|v| usize::try_from(v).ok())
            .ok_or("invalid subdivision count")?;
        Ok(())
    }
    fn progress(&self, event: &str, id: usize, telemetry: &Value) -> Value {
        let domain = self.queue.domains.get(id);
        let mut progress = json!({"event":event, "operation":"owner_domain_walk", "id":id,
            "initial_entry_domains_total":self.initial_domain_count,
            "initial_entry_domains_published":self.initial_published(),
            "initial_entry_domains_inspected":self.initial_entry_domains_inspected,
            "pending_descendant_domains":self.pending_descendants(),
            "owner":domain.map(|d| mask(&d.owner)), "phase":domain.map(|d| format!("{:?}", d.phase)),
            "power_bounds":domain.map(|d| power_bounds_json(d.powers)),
            "scheduled_nodes":self.queue.domains.len(), "completed_nodes":self.completed,
            "committed_domains":self.published_count(), "commit_domain":id,
            "contiguous_publication_watermark":self.queue.next,
            "queued_nodes":self.queue.domains.len().saturating_sub(self.published_count()),
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
            "routed_domains":self.routed, "route_masks":self.route_masks,
            "parallel":self.enrich(telemetry.clone())});
        progress["route_joint_support_masks_pruned"] = json!(self.route_joint_support_masks_pruned);
        self.add_delegation_progress(&mut progress);
        self.add_ready_progress(&mut progress);
        progress["descendant_closure"] = self.closure_json();
        progress
    }
    pub(super) fn closure_json(&self) -> Value {
        self.closure
            .borrow()
            .json(self.queue.domains.len(), self.initial_domain_count)
    }
    pub(super) fn refresh_closure(&self, cancellation: &AtomicBool, force: bool) {
        self.closure.borrow_mut().refresh(cancellation, force);
    }
    fn dependency_source(&self) -> usize {
        self.streams
            .active
            .map_or(self.queue.next, |ticket| ticket.parent)
    }
    fn dependency(&self, target: usize) {
        let mut closure = self.closure.borrow_mut();
        closure.discovered(self.queue.domains.len());
        closure.edge(self.dependency_source(), target);
    }
    fn enrich(&self, mut telemetry: Value) -> Value {
        if self.physical_enabled {
            for key in ["first_failure", "non_cancellation_failure"] {
                if let Some(raw) = telemetry[key]["domain"]
                    .as_u64()
                    .and_then(|v| usize::try_from(v).ok())
                {
                    let ticket = Ticket::decode(raw, true);
                    telemetry[key]["domain"] = json!(ticket.parent);
                    telemetry[key]["physical_part"] = json!(ticket.part);
                }
            }
            telemetry["inspection_count_scope"] =
                json!("physical_calls; domain_publications_are_logical_parents");
            telemetry["physical_inspections_published"] =
                json!(self.physical_inspections_published);
            telemetry["subdivided_logical_inspections"] =
                json!(self.subdivided_logical_inspections);
            telemetry["physical_publisher_part"] =
                json!(self.physical_progress.as_ref().map(|p| p.completed.len()));
        }
        telemetry["admission_preparation"] = self.admission.json();
        for key in ["first_failure", "non_cancellation_failure"] {
            if let Some(id) = telemetry[key]["domain"]
                .as_u64()
                .and_then(|id| usize::try_from(id).ok())
                && let Some(domain) = self.queue.domains.get(id)
            {
                let f = &mut telemetry[key];
                f["owner"] = json!(mask(&domain.owner));
                f["lower"] = json!(domain.lower);
                f["upper"] = json!(domain.upper);
                f["rank"] = json!(domain.rank);
                f["power_bounds"] = power_bounds_json(domain.powers);
            }
        }
        telemetry
    }
    fn set_parallel(&mut self, snapshot: Value, previous: &Value) {
        self.parallel = self.enrich(snapshot);
        accumulate_attempts(&mut self.parallel, previous, &mut self.error);
    }
    fn accept(
        &mut self,
        event: Event<N>,
        request: &OwnerDomainWalkRequest,
    ) -> Result<(), &'static str> {
        let token = self
            .replay
            .as_ref()
            .map(|_| replay::token(&event))
            .transpose()?;
        let count = event.count;
        self.accept_with_limits(event, request.max_events, request.max_frontiers)?;
        self.record_accepted(token, count)
    }
    fn record_accepted(
        &mut self,
        token: Option<replay::Token>,
        count: usize,
    ) -> Result<(), &'static str> {
        if let (Some(replay), Some(token)) = (&mut self.replay, token) {
            replay.record_accepted(token, count)?;
        }
        Ok(())
    }
    fn filter_replay(&mut self, event: &mut Event<N>) -> Result<bool, &'static str> {
        self.replay
            .as_mut()
            .map_or(Ok(true), |replay| replay.filter(event))
    }
    fn finish_replay(&mut self) -> Result<(), &'static str> {
        if let Some(replay) = &self.replay {
            replay.finish()?;
        }
        if self.replay.is_some() && !self.ready() {
            self.replay = Some(replay::Replay::default());
        }
        Ok(())
    }

    /// Numeric policy only: owner-local publication must not clone a request
    /// containing the complete saved-owner selection for every native callback.
    fn accept_with_limits(
        &mut self,
        event: Event<N>,
        max_events: usize,
        max_frontiers: usize,
    ) -> Result<(), &'static str> {
        let remaining = max_events
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
                ..
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
                if accepted != 0
                    && let Effect::PreAdmittedOrthantReuse { target, .. } = event.effect
                {
                    self.dependency(target);
                }
            } else {
                self.job_local_reuse_hits = hits;
            }
            return refusal.map_or(Ok(()), Err);
        }
        self.charge_event_with_limit(event.count, max_events)?;
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
                if self.frontiers == max_frontiers {
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
        self.charge_event_with_limit(count, request.max_events)
    }

    fn charge_event_with_limit(
        &mut self,
        count: usize,
        max_events: usize,
    ) -> Result<(), &'static str> {
        let remaining = max_events
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
        #[cfg(test)]
        if super::queue::positive_reuse_trace::enabled() {
            assert!(
                !self.ready() && !self.physical_enabled,
                "spectator requires Ordered unsplit publication"
            );
            super::queue::positive_reuse_trace::set_source(
                self.queue.next,
                &self.queue.domains[self.queue.next],
            );
        }
        let (target, _) = admit(&mut self.queue)?;
        self.dependency(target);
        Ok(())
    }
    fn commit(&mut self, id: usize, finished: Finished) {
        self.commit_result(
            id,
            finished.stats,
            finished.error,
            finished.error_kind,
            finished.seconds,
            None,
        );
    }
    fn commit_result(
        &mut self,
        id: usize,
        native_stats: NativeStats,
        native_error: Option<String>,
        error_kind: &str,
        seconds: f64,
        physical_parts: Option<Vec<Value>>,
    ) {
        let domain = &self.queue.domains[id];
        let partial_scope = match native_stats {
            NativeStats::ApplyPartial(_, scope) => Some(scope),
            _ => None,
        };
        let native_cancelled = error_kind == "cancelled";
        self.error = self.error.take().or(native_error);
        let (stats, optional, truncated) = match native_stats {
            NativeStats::Apply(stats) | NativeStats::ApplyPartial(stats, _) => {
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
                if let Some(total) = self
                    .route_joint_support_masks_pruned
                    .checked_add(stats.joint_support_masks_pruned)
                {
                    self.route_joint_support_masks_pruned = total;
                } else {
                    self.error
                        .get_or_insert_with(|| "joint support mask counter overflow".into());
                }
                (route_stats(stats), None, None)
            }
        };
        // Capture real frontiers before moving the per-inspection details.
        // An outer publisher error means the native stream was NOT completely
        // admitted, even if a later buffered Finished itself has no error.
        let frontier_count = self.details.len();
        if let Some(ledger) = &mut self.queue.delegation {
            use super::delegation::NativeOutcome;
            if let Some(scope) = partial_scope {
                if let Err(error) = ledger.record_initial_overlap(id, scope.anchor_id) {
                    self.error.get_or_insert_with(|| error.to_string());
                }
            }
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
        } else if partial_scope.is_some() {
            self.error.get_or_insert_with(|| {
                "partial initial inspection has no responsibility ledger".into()
            });
        }
        {
            let mut closure = self.closure.borrow_mut();
            closure.discovered(self.queue.domains.len());
            if let Some(scope) = partial_scope {
                closure.edge(id, scope.anchor_id);
            }
            closure.finish(
                id,
                self.error.is_none(),
                self.error.is_none() && frontier_count == 0,
            );
        }
        self.native_records += 1;
        self.physical_inspections_published += physical_parts.as_ref().map_or(1, Vec::len);
        self.completed += usize::from(self.error.is_none());
        self.initial_entry_domains_inspected +=
            usize::from(id < self.initial_domain_count && self.error.is_none());
        let mut record = json!({"id":id, "phase":format!("{:?}", domain.phase), "owner":mask(&domain.owner),
            "lower":domain.lower, "upper":domain.upper, "rank":domain.rank,
            "power_bounds":power_bounds_json(domain.powers),
            "local_inspection_finished":self.error.is_none(), "stats":stats, "seconds":seconds,
            "error":self.error});
        if self.ready()
            && let Some(replay) = &self.replay
        {
            record["accepted_events"] = json!(replay.accepted_events());
        }
        record["frontiers"] = Value::Array(std::mem::take(&mut self.details));
        if self.queue.delegation.is_some() {
            record["record_kind"] = json!("native_inspection");
            record["local_classification_discharged"] =
                json!(self.error.is_none() && frontier_count == 0);
        }
        if let Some(scope) = partial_scope {
            record["record_kind"] = json!("partial_initial_overlap_inspection");
            record["native_inspection_scope"] = json!("low_D_residual_only");
            record["local_inspection_finished"] = json!(false);
            record["residual_inspection_finished"] = json!(self.error.is_none());
            // Filled from the typed ledger at finalization, never inferred from
            // residual Finished alone or the existence of an initial anchor.
            record["local_classification_discharged"] = json!(false);
            record["initial_overlap"] = json!({"anchor_id":scope.anchor_id,"cut":scope.cut,
                "covered_slice":"original_intersect_D_ge_cut",
                "residual_power_bounds":power_bounds_json(scope.residual_powers),
                "coordinates_and_rank_unchanged":true,
                "authority":"same_snapshot_phase_owner_native_summary"});
        }
        if let (Some(optional), Some(stats)) = (optional, truncated) {
            record["optional_refusal_provenance_truncated"] = json!(optional.truncated(stats));
            record["optional_refusal_provenance_scope"] = json!("first_per_phase_per_query");
            record["optional_refusals"] = Value::Array(optional.records);
        } else {
            record["conservative_route_overcover"] = json!(true);
        }
        if let Some(parts) = physical_parts {
            self.subdivided_logical_inspections += 1;
            record["record_kind"] = json!("subdivided_native_inspection");
            record["native_inspection_scope"] = json!("disjoint_exact_source_partition");
            record["stats_scope"] = json!("checked_sum_of_actual_physical_calls");
            record["physical_inspections"] = json!(parts.len());
            record["physical_parts_expected"] = json!(2);
            record["unreturned_physical_parts"] = json!(
                (0..2)
                    .filter(|part| !parts.iter().any(|p| p["part"] == *part))
                    .collect::<Vec<_>>()
            );
            record["physical_seconds_sum"] = json!(seconds);
            record["physical_seconds_scope"] =
                json!("sum_of_physical_call_wall_seconds; not_parent_wall_or_CPU");
            record["seconds"] = Value::Null;
            record["optional_refusal_provenance_scope"] =
                json!("first_per_phase_per_physical_part");
            record["optional_refusals"] = json!(
                parts
                    .iter()
                    .flat_map(|p| p["optional_refusals"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .cloned())
                    .collect::<Vec<_>>()
            );
            record["optional_refusal_provenance_truncated"] = json!(
                parts
                    .iter()
                    .any(|p| p["optional_refusal_provenance_truncated"] == true)
            );
            record["physical_parts"] = Value::Array(parts);
        }
        self.records.push(record);
        if self.ready() {
            self.streams.initial_published += usize::from(id < self.initial_domain_count);
            self.queue.next = self
                .queue
                .delegation
                .as_ref()
                .expect("ready ledger")
                .cursor();
        } else {
            self.queue.next += 1;
        }
    }

    fn physical_receipt(
        &mut self,
        part: u8,
        finished: Finished,
        committed: bool,
        request: &OwnerDomainWalkRequest,
    ) -> Value {
        let id = self.queue.next;
        let parts = request
            .apply_subdivision
            .expect("physical policy")
            .parts(&self.queue.domains[id])
            .expect("valid physical source partition");
        let source = &parts[usize::from(part)];
        let mut parent_limits = request.applied_limits;
        parent_limits.matching = request.matching.match_limits;
        let admitted_limits =
            super::physical_parts::limits_json(super::physical_parts::limits(parent_limits, part));
        let truncated = match finished.stats {
            NativeStats::Apply(stats) => self.refusals.truncated(stats),
            _ => true,
        };
        for diagnostic in &mut self.refusals.records {
            diagnostic["physical_part"] = json!(part);
        }
        for frontier in &mut self.details {
            frontier["physical_part"] = json!(part);
        }
        let mut receipt = json!({"part":part,"lower":source.lower,"upper":source.upper,"rank":source.rank,
            "admitted_limits":admitted_limits,
            "power_bounds":power_bounds_json(source.powers),"stats":native_stats(finished.stats),
            "seconds":finished.seconds,"error":finished.error,"error_kind":finished.error_kind,
            "stream_committed":committed,"optional_refusal_provenance_truncated":truncated});
        receipt["frontiers"] = Value::Array(std::mem::take(&mut self.details));
        receipt["optional_refusals"] = Value::Array(std::mem::take(&mut self.refusals).records);
        receipt
    }

    fn commit_physical(
        &mut self,
        ticket: Ticket,
        finished: Finished,
        request: &OwnerDomainWalkRequest,
    ) {
        if let Err(error) = self.finish_replay() {
            self.error.get_or_insert_with(|| error.into());
            self.uncommitted
                .push(json!({"id":ticket.parent,"physical_part":ticket.part,
                "committed":false,"stats":native_stats(finished.stats),"seconds":finished.seconds,
                "native_error":finished.error,"checkpoint_replay_error":error}));
            return;
        }
        let Some(part) = ticket.part else {
            self.commit(ticket.parent, finished);
            self.complete_stream(ticket);
            return;
        };
        let failed = self.error.is_some() || finished.error.is_some();
        let receipt = self.physical_receipt(part, finished, !failed, request);
        let progress = self
            .physical_progress
            .get_or_insert_with(|| PhysicalProgress {
                parent: ticket.parent,
                completed: Vec::new(),
            });
        if progress.parent != ticket.parent || progress.completed.len() != usize::from(part) {
            self.error = Some("physical publication order invariant".into());
            return;
        }
        progress.completed.push(receipt);
        if part == 1 || failed {
            self.commit_physical_parent();
        }
    }

    fn commit_physical_parent(&mut self) {
        let mut progress = self.physical_progress.take().expect("physical parent");
        let stats = match super::physical_parts::sum_stats(&progress.completed) {
            Ok(stats) => stats,
            Err(error) => {
                self.error.get_or_insert(error);
                self.physical_progress = Some(progress);
                return;
            }
        };
        let mut seconds = 0.0;
        let mut error = None;
        let mut cancelled = false;
        for part in &mut progress.completed {
            seconds += part["seconds"].as_f64().unwrap_or(0.0);
            if let Some(detail) = part["error"].as_str() {
                error.get_or_insert_with(|| detail.to_owned());
            }
            cancelled |= part["error_kind"] == "cancelled";
            // Preserve the normal parent frontier report without retaining a
            // second copy of every payload in its physical-call receipts.
            // Pending part0 checkpoints still retain the full source receipt.
            if let Some(frontiers) = part["frontiers"].as_array_mut() {
                let frontiers = std::mem::take(frontiers);
                let count = frontiers.len();
                self.details.extend(frontiers);
                part["frontier_count"] = json!(count);
                part["frontiers_published_on_parent"] = json!(true);
            }
        }
        self.commit_result(
            progress.parent,
            NativeStats::Apply(stats),
            error,
            if cancelled { "cancelled" } else { "none" },
            seconds,
            Some(progress.completed),
        );
    }
}
fn route_stats(s: rustred::solver::CandidateDomainRouteStats) -> Value {
    json!({"masks_examined":s.masks_examined, "masks_pruned":s.masks_pruned,
        "joint_support_masks_pruned":s.joint_support_masks_pruned,
        "events":s.events, "apply_domains":s.apply_domains,
        "route_domains":s.route_domains, "zero_sectors":s.zero_sectors, "missing_routes":s.missing_routes,
        "coordinate_cells":s.coordinate_cells})
}
fn native_stats(stats: NativeStats) -> Value {
    match stats {
        NativeStats::Apply(s) | NativeStats::ApplyPartial(s, _) => stats_json(s),
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
    run_configured(
        state,
        reducer,
        request,
        cancellation,
        observer,
        enabled,
        false,
        &mut |_| Ok(()),
    );
}

pub(super) fn run_checkpointed<const N: usize>(
    state: &mut State<N>,
    reducer: &RoutedCandidateReducer<N>,
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
    maybe_save: &mut dyn FnMut(&State<N>) -> Result<(), String>,
) {
    run_configured(
        state,
        reducer,
        request,
        cancellation,
        observer,
        true,
        true,
        maybe_save,
    );
}

fn run_configured<const N: usize>(
    state: &mut State<N>,
    reducer: &RoutedCandidateReducer<N>,
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
    enabled: bool,
    checkpointing: bool,
    maybe_save: &mut dyn FnMut(&State<N>) -> Result<(), String>,
) {
    if state.error.is_some() {
        return;
    }
    #[cfg(test)]
    if super::queue::positive_reuse_trace::enabled() {
        assert_eq!(
            request.publication_policy,
            super::OwnerDomainWalkPublicationPolicy::Ordered
        );
        assert!(request.workers > 1 && request.apply_subdivision.is_none());
        super::queue::positive_reuse_trace::begin_run(&mut state.queue);
    }
    state.physical_enabled = request.apply_subdivision.is_some();
    if request
        .apply_subdivision
        .is_some_and(|policy| policy.axis >= N)
    {
        state.error = Some("Apply subdivision axis exceeds native dimension".into());
        return;
    }
    if let Some(progress) = &state.physical_progress {
        let valid = state.parts(progress.parent, request).is_some_and(|parts| {
            progress.completed.len() == 1
                && progress.completed[0]["part"] == 0
                && progress.completed[0]["stream_committed"] == true
                && progress.completed[0]["error"].is_null()
                && progress.completed[0]["lower"] == json!(parts[0].lower)
                && progress.completed[0]["upper"] == json!(parts[0].upper)
                && progress.completed[0]["rank"] == json!(parts[0].rank)
                && progress.completed[0]["power_bounds"] == power_bounds_json(parts[0].powers)
                && super::physical_parts::sum_stats(&progress.completed).is_ok()
        });
        if !valid {
            state.error = Some("checkpoint physical parent geometry or completion mismatch".into());
            return;
        }
    }
    if checkpointing && !state.ready() && state.replay.is_none() {
        state.replay = Some(replay::Replay::default());
    }
    // Capture actual initial admissions only. This immutable borrowed snapshot
    // is shared across scoped workers and never observes later queue growth.
    let initial = if enabled {
        InitialOrthants::from_initial(
            &state.queue.domains[..state.initial_domain_count],
            cancellation,
        )
    } else {
        InitialOrthants::empty()
    };
    let overlap = if request.reuse_initial_d_bands {
        let Some(prefix) = state
            .queue
            .delegation
            .as_ref()
            .and_then(|l| l.initial_prefix())
        else {
            state.error = Some("initial D-band reuse requires a protected initial prefix".into());
            return;
        };
        InitialOverlapIndex::from_initial(&state.queue.domains[..prefix], cancellation)
    } else {
        InitialOverlapIndex::empty()
    };
    if request.reuse_initial_d_bands {
        state.initial_overlap_report = Some(overlap.build_report());
        observer(
            json!({"event":"initial_overlap_prepared","operation":"owner_domain_walk",
            "initial_overlap_index":super::index_report::render(
                state.initial_overlap_report, super::index_report::Scope::GlobalInitial)}),
        );
    }
    if request.workers == 1 {
        return serial(
            state,
            reducer,
            request,
            cancellation,
            observer,
            &initial,
            &overlap,
            checkpointing,
            maybe_save,
        );
    }
    let physical_enabled = state.physical_enabled;
    run_pool(
        state,
        request,
        cancellation,
        observer,
        checkpointing,
        maybe_save,
        |raw, domain, stop, emit| match Ticket::decode(raw, physical_enabled).part {
            Some(part) => {
                inspection::inspect_part(reducer, domain, request, part, stop, &initial, emit)
            }
            None => inspection::inspect(reducer, domain, request, stop, &initial, &overlap, emit),
        },
    );
    #[cfg(test)]
    if super::queue::positive_reuse_trace::enabled() {
        super::queue::positive_reuse_trace::end_run(&state.queue);
    }
}

// Both publication policies share this one native pool and admission loop.
// Injecting the native visitor also permits deterministic concurrency tests.
fn run_pool<const N: usize>(
    state: &mut State<N>,
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
    checkpointing: bool,
    maybe_save: &mut dyn FnMut(&State<N>) -> Result<(), String>,
    inspect: impl Fn(
        usize,
        &super::queue::Domain<N>,
        &AtomicBool,
        &mut dyn FnMut(Event<N>) -> ControlFlow<()>,
    ) -> Finished
    + Sync,
) {
    let previous_parallel = state.parallel.clone();
    let budget = super::worker_budget::WorkerBudget::for_request(request);
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
    let mut dispatched = state.queue.next;
    let mut dispatch_part = state
        .physical_progress
        .as_ref()
        .map_or(0, |p| p.completed.len() as u8);
    let mut parent_dispatched = false;
    let mut started_id = None;
    let mut heartbeat = Instant::now();
    let physical_enabled = state.physical_enabled;
    let ready = state.ready();
    let mut ready_streams = publication::ReadyStreams::default();
    let (_, snapshot, mut leftovers) =
        parallel::with_ticket_pool(budget.inspection, inspect, |pool| {
            loop {
                state.refresh_closure(cancellation, false);
                let publisher = state.publisher_ticket(request);
                let publisher_raw = match publisher.encode(physical_enabled) {
                    Ok(id) => id,
                    Err(error) => {
                        state.error = Some(error.into());
                        break;
                    }
                };
                if cancellation.load(Ordering::Acquire) {
                    pool.fail(Failure {
                        id: Some(publisher_raw),
                        phase: state.queue.domains.get(state.queue.next).map(|d| d.phase),
                        kind: "cancelled",
                        detail: "cancelled".into(),
                    });
                }
                if let Some(failure) = pool.failure() {
                    state.error.get_or_insert(failure.detail);
                    observer(state.progress("domain_progress", state.queue.next, &pool.snapshot()));
                    break;
                }
                if !ready && state.current_is_delegated() {
                    let id = state.queue.next;
                    if let Err(error) = state.commit_delegated() {
                        pool.fail(Failure {
                            id: Some(
                                Ticket {
                                    parent: id,
                                    part: None,
                                }
                                .encode(physical_enabled)
                                .expect("admitted ticket"),
                            ),
                            phase: Some(state.queue.domains[id].phase),
                            kind: "delegation_publication",
                            detail: error,
                        });
                    } else {
                        observer(state.progress("domain_delegated", id, &pool.snapshot()));
                        if let Err(error) = maybe_save(state) {
                            state.error = Some(error);
                            break;
                        }
                    }
                    continue; // Check cancellation between every delegated record.
                }
                // Locally finished later jobs are not committed. Bounded
                // escrow frees their worker slots without reordering effects.
                if !ready {
                    pool.reclaim_finished(publisher_raw);
                }
                while dispatched < state.queue.domains.len() {
                    if cancellation.load(Ordering::Acquire) || pool.failure().is_some() {
                        break;
                    }
                    if !parent_dispatched && let Some(ledger) = &state.queue.delegation {
                        if ready && ledger.is_published(dispatched) {
                            dispatched += 1;
                            continue;
                        }
                        if dispatched >= ledger.dispatch_fence() {
                            break;
                        }
                        if ledger.delegated_to(dispatched).is_some() {
                            if ready {
                                if let Err(error) = state.commit_delegated_id(dispatched) {
                                    state.error = Some(error);
                                    break;
                                }
                                observer(state.progress(
                                    "domain_delegated",
                                    dispatched,
                                    &pool.snapshot(),
                                ));
                                if let Err(error) = maybe_save(state) {
                                    state.error = Some(error);
                                    break;
                                }
                            }
                            dispatched += 1;
                            continue;
                        }
                        if !ledger.can_dispatch(dispatched) {
                            pool.fail(Failure {
                                id: Some(
                                    Ticket {
                                        parent: dispatched,
                                        part: None,
                                    }
                                    .encode(physical_enabled)
                                    .expect("admitted ticket"),
                                ),
                                phase: Some(state.queue.domains[dispatched].phase),
                                kind: "delegation_dispatch_invariant",
                                detail: "native dispatch has no reserved responsibility".into(),
                            });
                            break;
                        }
                    }
                    let parts = state.parts(dispatched, request);
                    let ticket = Ticket {
                        parent: dispatched,
                        part: parts.as_ref().map(|_| dispatch_part),
                    };
                    let raw = match ticket.encode(physical_enabled) {
                        Ok(raw) => raw,
                        Err(error) => {
                            pool.fail(Failure {
                                id: None,
                                phase: None,
                                kind: "counter_overflow",
                                detail: error.into(),
                            });
                            break;
                        }
                    };
                    let source = parts.as_ref().map_or_else(
                        || state.queue.domains[dispatched].clone(),
                        |parts| parts[usize::from(dispatch_part)].clone(),
                    );
                    if !pool.dispatch(raw, source) {
                        break;
                    }
                    if ready {
                        ready_streams.dispatched(raw);
                    }
                    if !parent_dispatched && let Err(error) = state.note_native_started(dispatched)
                    {
                        pool.fail(Failure {
                            id: Some(raw),
                            phase: Some(state.queue.domains[dispatched].phase),
                            kind: "delegation_native_start",
                            detail: error,
                        });
                        break;
                    }
                    parent_dispatched = true;
                    if parts.is_some() && dispatch_part == 0 {
                        dispatch_part = 1;
                    } else {
                        dispatched += 1;
                        dispatch_part = 0;
                        parent_dispatched = false;
                    }
                }
                if let Some(error) = state.error.clone() {
                    pool.fail(Failure {
                        id: Some(publisher_raw),
                        phase: None,
                        kind: "coordinator_dispatch",
                        detail: error,
                    });
                    break;
                }
                let watermark = state.queue.next;
                if watermark == state.queue.domains.len() && (!ready || ready_streams.is_empty()) {
                    break;
                }
                if !ready && started_id != Some(watermark) {
                    observer(state.progress("domain_started", watermark, &pool.snapshot()));
                    started_id = Some(watermark);
                    continue; // observe caller cancellation before publishing
                }
                let (publisher_raw, poll) = if ready {
                    ready_streams
                        .poll(pool)
                        .unwrap_or((publisher_raw, Poll::Waiting))
                } else {
                    (publisher_raw, pool.poll(publisher_raw))
                };
                let publisher = Ticket::decode(publisher_raw, physical_enabled);
                let id = publisher.parent;
                if ready && !matches!(poll, Poll::Waiting) {
                    if let Err(error) = state.activate_stream(publisher, checkpointing) {
                        state.error = Some(error.into());
                        pool.fail(Failure {
                            id: Some(publisher_raw),
                            phase: Some(state.queue.domains[id].phase),
                            kind: "publication_context",
                            detail: error.into(),
                        });
                        break;
                    }
                }
                match poll {
                    Poll::Events(chunk) => {
                        if let Err(error) = admission.commit_chunk(
                            state,
                            request,
                            chunk,
                            cancellation,
                            &pool.stop,
                            &mut |state| {
                                if heartbeat.elapsed() >= Duration::from_millis(250) {
                                    state.refresh_closure(cancellation, false);
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
                                id: Some(publisher_raw),
                                phase: Some(state.queue.domains[id].phase),
                                kind: if error == "cancelled" {
                                    "cancelled"
                                } else {
                                    "coordinator_admission"
                                },
                                detail: error.into(),
                            });
                        } else {
                            state.set_parallel(pool.snapshot(), &previous_parallel);
                            if let Err(error) = maybe_save(state) {
                                pool.fail(Failure {
                                    id: Some(publisher_raw),
                                    phase: Some(state.queue.domains[id].phase),
                                    kind: "checkpoint_write",
                                    detail: error,
                                });
                            }
                        }
                    }
                    Poll::Finished(finished) => {
                        state.commit_physical(publisher, finished, request);
                        if ready {
                            ready_streams.finished(publisher_raw);
                        }
                        state.set_parallel(pool.snapshot(), &previous_parallel);
                        if state.error.is_none()
                            && let Err(error) = maybe_save(state)
                        {
                            pool.fail(Failure {
                                id: Some(publisher_raw),
                                phase: Some(state.queue.domains[id].phase),
                                kind: "checkpoint_write",
                                detail: error,
                            });
                        }
                    }
                    Poll::Waiting => {
                        if ready {
                            if ready_streams.is_empty() && state.error.is_none() {
                                state.error = Some(
                                    "Ready worklist has no dispatchable or active responsibility"
                                        .into(),
                                );
                            } else {
                                ready_streams.wait(pool);
                            }
                        } else {
                            pool.wait(publisher_raw);
                        }
                    }
                }
                if heartbeat.elapsed() >= Duration::from_millis(250) {
                    observer(state.progress("domain_progress", state.queue.next, &pool.snapshot()));
                    heartbeat = Instant::now();
                    state.set_parallel(pool.snapshot(), &previous_parallel);
                    if let Err(error) = maybe_save(state) {
                        pool.fail(Failure {
                            id: Some(publisher_raw),
                            phase: Some(state.queue.domains[id].phase),
                            kind: "checkpoint_write",
                            detail: error,
                        });
                    }
                }
                if state.error.is_some() {
                    pool.fail(Failure {
                        id: Some(publisher_raw),
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
        });
    if state.error.is_none()
        && let Some(detail) = snapshot["first_failure"]["detail"].as_str()
    {
        state.error = Some(detail.to_owned());
    }
    state.set_parallel(snapshot, &previous_parallel);
    if state
        .error
        .as_deref()
        .is_none_or(|error| error == "cancelled")
        && let Some(detail) = state.parallel["non_cancellation_failure"]["detail"].as_str()
    {
        state.error = Some(detail.to_owned());
    }
    if checkpointing
        && state.parallel["non_cancellation_failure"].is_null()
        && resumable_cancellation(
            cancellation.load(Ordering::Acquire),
            state.parallel["first_failure"]["kind"].as_str(),
            state.error.as_deref(),
        )
    {
        state.error = None;
        state.checkpoint_paused = true;
        for (raw, finished) in leftovers {
            let ticket = Ticket::decode(raw, physical_enabled);
            state.uncommitted.push(json!({"id":ticket.parent,"physical_part":ticket.part,
                "committed":false,"resume_reinspects_unfinished_part":true,
                "stats":native_stats(finished.stats),"error":finished.error,"seconds":finished.seconds}));
        }
        return;
    }
    // Every native visitor has returned before reporting. Preserve the current
    // publisher's admitted prefix and keep later attempts explicitly separate.
    let before_retention = state.native_records;
    let publisher_returns = leftovers
        .iter()
        .filter(|(raw, _)| Ticket::decode(*raw, physical_enabled).parent == state.queue.next)
        .count();
    if ready {
        state.retain_ready_leftovers(&mut leftovers);
    } else if physical_enabled {
        retain_physical_leftovers(state, &mut leftovers, request);
    } else {
        retain_leftovers(state, &mut leftovers);
    }
    let retained_publisher = if physical_enabled && state.native_records > before_retention {
        publisher_returns
    } else {
        state.native_records - before_retention
    };
    for key in [
        "finished_uncommitted_domains",
        "dispatched_uncommitted_domains",
    ] {
        if let Some(value) = state.parallel[key].as_u64() {
            state.parallel[key] = json!(value - retained_publisher as u64);
        }
    }
}

fn resumable_cancellation(
    cancelled: bool,
    first_failure_kind: Option<&str>,
    error: Option<&str>,
) -> bool {
    cancelled
        && first_failure_kind == Some("cancelled")
        && error.is_none_or(|error| error == "cancelled")
}

fn accumulate_attempts(current: &mut Value, previous: &Value, error: &mut Option<String>) {
    for key in [
        "attempted_events",
        "returned_inspections",
        "attempted_native_operations",
        "attempted_rule_checks",
        "attempted_predicates",
        "attempted_optional_coefficient_refusals",
    ] {
        if let (Some(now), Some(before)) = (current[key].as_u64(), previous[key].as_u64()) {
            match now.checked_add(before) {
                Some(total) => current[key] = json!(total),
                None => {
                    error.get_or_insert_with(|| "resumed attempted-work counter overflow".into());
                }
            }
        }
    }
    if previous["returned_inspections"]
        .as_u64()
        .is_some_and(|n| n > 0)
    {
        current["native_attempt_counters_scope"] = json!(
            "returned_physical_calls_across_resume_attempts_including_uncommitted_and_cancelled"
        );
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
            let partial_scope = finished.initial_overlap_scope();
            let mut record = json!({"id":id, "phase":format!("{:?}", domain.phase),
                "owner":mask(&domain.owner), "lower":domain.lower, "upper":domain.upper, "rank":domain.rank,
                "power_bounds":power_bounds_json(domain.powers),
                "stats":native_stats(finished.stats), "error":finished.error, "seconds":finished.seconds,
                "committed":false});
            if let Some(scope) = partial_scope {
                record["native_inspection_scope"] = json!("low_D_residual_only");
                record["initial_overlap"] = json!({"anchor_id":scope.anchor_id,"cut":scope.cut,
                    "covered_slice":"original_intersect_D_ge_cut",
                    "residual_power_bounds":power_bounds_json(scope.residual_powers),
                    "coordinates_and_rank_unchanged":true,
                    "responsibility_published":false});
            }
            state.uncommitted.push(record);
        }
    }
    if (!state.details.is_empty() || !state.refusals.records.is_empty())
        && let Some(domain) = state.queue.domains.get(publisher_id)
    {
        // A panicked worker has no Finished/stats. Retain already-published
        // diagnostics without inventing a completed inspection or zero stats.
        let mut record = json!({"id":publisher_id, "phase":format!("{:?}", domain.phase),
            "owner":mask(&domain.owner), "lower":domain.lower, "upper":domain.upper, "rank":domain.rank,
            "power_bounds":power_bounds_json(domain.powers),
            "stats":null, "committed":false, "partial_publisher":true,
            "partial_native_statistics_unavailable":true});
        record["frontiers"] = Value::Array(std::mem::take(&mut state.details));
        record["optional_refusals"] = Value::Array(std::mem::take(&mut state.refusals.records));
        state.uncommitted.push(record);
    }
}

fn retain_physical_leftovers<const N: usize>(
    state: &mut State<N>,
    leftovers: &mut Vec<(usize, Finished)>,
    request: &OwnerDomainWalkRequest,
) {
    leftovers.sort_by_key(|(raw, _)| *raw);
    let publisher = state.publisher_ticket(request);
    let current_returned = leftovers
        .iter()
        .any(|(raw, _)| Ticket::decode(*raw, true) == publisher);
    let mut parent_parts = state
        .physical_progress
        .take()
        .map_or_else(Vec::new, |p| p.completed);
    let mut ordinary = Vec::new();
    if !current_returned
        && publisher.part.is_some()
        && state.error.is_some()
        && (!state.details.is_empty() || !state.refusals.records.is_empty())
    {
        let mut record = json!({"id":publisher.parent,"physical_part":publisher.part,
            "committed":false,"stats":null,"partial_native_statistics_unavailable":true});
        record["frontiers"] = Value::Array(std::mem::take(&mut state.details));
        record["optional_refusals"] = Value::Array(std::mem::take(&mut state.refusals).records);
        state.uncommitted.push(record);
    }
    for (raw, finished) in leftovers.drain(..) {
        let ticket = Ticket::decode(raw, true);
        if ticket.parent != publisher.parent || ticket.part.is_none() {
            if ticket.part.is_none() {
                ordinary.push((ticket.parent, finished));
            } else {
                state
                    .uncommitted
                    .push(json!({"id":ticket.parent,"physical_part":ticket.part,
                    "committed":false,"stats":native_stats(finished.stats),"error":finished.error,
                    "seconds":finished.seconds}));
            }
            continue;
        }
        let part = ticket.part.expect("physical part");
        if ticket == publisher {
            parent_parts.push(state.physical_receipt(part, finished, false, request));
        } else {
            let sources = state
                .parts(ticket.parent, request)
                .expect("physical sources");
            let source = &sources[usize::from(part)];
            parent_parts.push(json!({"part":part,"lower":source.lower,"upper":source.upper,
                "rank":source.rank,"power_bounds":power_bounds_json(source.powers),
                "stats":native_stats(finished.stats),"error":finished.error,"error_kind":finished.error_kind,
                "seconds":finished.seconds,"stream_committed":false,"frontiers":[],
                "optional_refusals":[],"optional_refusal_provenance_truncated":true}));
        }
    }
    if !parent_parts.is_empty() && state.error.is_some() {
        state.physical_progress = Some(PhysicalProgress {
            parent: publisher.parent,
            completed: parent_parts,
        });
        state.commit_physical_parent();
    }
    retain_leftovers(state, &mut ordinary);
}

fn serial<const N: usize>(
    state: &mut State<N>,
    reducer: &RoutedCandidateReducer<N>,
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
    initial: &InitialOrthants<N>,
    overlap: &InitialOverlapIndex<N>,
    checkpointing: bool,
    maybe_save: &mut dyn FnMut(&State<N>) -> Result<(), String>,
) {
    let previous_parallel = state.parallel.clone();
    let mut attempted = 0usize;
    let mut native = 0usize;
    let mut returned = 0usize;
    let mut failure = None;
    let mut heartbeat = Instant::now();
    while state.error.is_none() && state.queue.next < state.queue.domains.len() {
        state.refresh_closure(cancellation, false);
        if cancellation.load(Ordering::Acquire) {
            if checkpointing {
                state.checkpoint_paused = true;
            } else {
                state.error = Some("cancelled".into());
            }
            failure = Some(Failure {
                id: state
                    .publisher_ticket(request)
                    .encode(state.physical_enabled)
                    .ok(),
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
            if let Err(error) = maybe_save(state) {
                state.error = Some(error);
                break;
            }
            continue;
        }
        let ticket = state.publisher_ticket(request);
        if let Err(error) = state.activate_stream(ticket, checkpointing) {
            state.error = Some(error.into());
            break;
        }
        let raw = match ticket.encode(state.physical_enabled) {
            Ok(raw) => raw,
            Err(error) => {
                state.error = Some(error.into());
                break;
            }
        };
        let already_started = ticket.part == Some(1)
            && state
                .queue
                .delegation
                .as_ref()
                .is_none_or(|ledger| !ledger.can_dispatch(id));
        if !already_started && let Err(error) = state.note_native_started(id) {
            state.error = Some(error);
            break;
        }
        let domain = ticket.part.map_or_else(
            || state.queue.domains[id].clone(),
            |part| state.parts(id, request).expect("physical partition")[usize::from(part)].clone(),
        );
        observer(state.progress(
            "domain_started",
            id,
            &json!({"workers":1, "active_workers":1, "attempted_events":attempted}),
        ));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut emit = |mut event: Event<N>| {
                let error = if let Some(next) = attempted.checked_add(event.count) {
                    attempted = next;
                    match state.filter_replay(&mut event) {
                        Ok(true) => state.accept(event, request).err(),
                        Ok(false) => None,
                        Err(error) => Some(error),
                    }
                } else {
                    Some("attempted event counter overflow")
                };
                if let Some(error) = error {
                    state.error = Some(error.into());
                    failure = Some(Failure {
                        id: Some(raw),
                        phase: Some(domain.phase),
                        kind: "coordinator_admission",
                        detail: error.into(),
                    });
                    return ControlFlow::Break(());
                }
                if heartbeat.elapsed() >= Duration::from_millis(250) {
                    state.refresh_closure(cancellation, false);
                    observer(state.progress(
                        "domain_progress",
                        id,
                        &json!({"workers":1, "active_workers":1, "attempted_events":attempted}),
                    ));
                    heartbeat = Instant::now();
                    state.set_parallel(
                        json!({"workers":1,"active_workers":1,"attempted_events":attempted,
                            "returned_inspections":returned,"attempted_native_operations":native}),
                        &previous_parallel,
                    );
                    if let Err(error) = maybe_save(state) {
                        state.error = Some(error);
                        return ControlFlow::Break(());
                    }
                }
                ControlFlow::Continue(())
            };
            match ticket.part {
                Some(part) => inspection::inspect_part(
                    reducer,
                    &domain,
                    request,
                    part,
                    cancellation,
                    initial,
                    &mut emit,
                ),
                None => inspection::inspect(
                    reducer,
                    &domain,
                    request,
                    cancellation,
                    initial,
                    overlap,
                    &mut emit,
                ),
            }
        }));
        match result {
            Ok(finished) => {
                if failure.is_none()
                    && let Some(error) = &finished.error
                {
                    failure = Some(Failure {
                        id: Some(raw),
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
                if checkpointing
                    && finished.error_kind == "cancelled"
                    && resumable_cancellation(
                        cancellation.load(Ordering::Acquire),
                        failure.as_ref().map(|f| f.kind),
                        state.error.as_deref(),
                    )
                {
                    state.error = None;
                    state.checkpoint_paused = true;
                    state.uncommitted.push(json!({"id":id,"physical_part":ticket.part,"committed":false,
                        "resume_reinspects_unfinished_part":true,"stats":native_stats(finished.stats),
                        "error":finished.error,"seconds":finished.seconds}));
                    break;
                }
                state.commit_physical(ticket, finished, request);
                state.set_parallel(
                    json!({"workers":1,"active_workers":0,"attempted_events":attempted,
                    "returned_inspections":returned,"attempted_native_operations":native}),
                    &previous_parallel,
                );
                if state.error.is_none()
                    && let Err(error) = maybe_save(state)
                {
                    state.error = Some(error);
                }
                if failure.is_none()
                    && let Some(error) = &state.error
                {
                    failure = Some(Failure {
                        id: Some(raw),
                        phase: Some(domain.phase),
                        kind: "coordinator_accounting",
                        detail: error.clone(),
                    });
                }
            }
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }
    state.set_parallel(json!({"workers":1, "active_workers":0, "attempted_events":attempted,
        "returned_inspections":returned, "attempted_native_operations":native,
        "native_attempt_counters_scope":"returned_inspections_including_uncommitted_and_cancelled",
        "worker_buffered_events":0, "worker_buffered_logical_bytes":0, "peak_worker_buffered_logical_bytes":0,
        "backpressured_workers":0, "backpressure_seconds":0.0,
        "first_failure":failure.as_ref().map(Failure::json)}), &previous_parallel);
}

#[cfg(test)]
mod closure_tests;
#[cfg(test)]
#[path = "execution/initial_orthants_tests.rs"]
mod initial_orthants_tests;
#[cfg(test)]
mod ready_native_tests;
#[cfg(test)]
mod ready_tests;
#[cfg(test)]
mod subdivision_tests;
#[cfg(test)]
mod tests;
