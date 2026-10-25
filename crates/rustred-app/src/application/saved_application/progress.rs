//! Saved-application adapter for native safe-boundary observations.
use super::*;
use rustred::solver::{
    CandidateRoutedActivity, CandidateRoutedApplicationEnvelope, CandidateRoutedApplicationEvent,
    CandidateRoutedApplicationObserver, CandidateRoutedApplicationPhase,
    CandidateRoutedApplicationSnapshot, CandidateRoutedApplicationStatus,
    CandidateRoutedCacheCensus, CandidateRoutedCompletionSource, CandidateRoutedKeyDegrees,
    CandidateRoutedObserverStatus, CandidateRoutedTerminalProgress, CandidateRoutedWorkRef,
};
use serde_json::json;

impl<const N: usize> PreparedSavedApplication<N> {
    /// Observe direct native reduction and the saved terminal substitutions
    /// through one optional caller-thread observer. Native `CoreFinished` is
    /// local to the core; the saved `RequestFinished` follows substitutions.
    /// Callback faults or detachment preserve the reduction's typed result.
    /// Cancellation is checked between native operations and substitution terms.
    pub fn reduce_linear_combination_with_observation(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        observer: &mut CandidateRoutedApplicationObserver<'_>,
        cancel: Option<&AtomicBool>,
    ) -> Result<SavedApplicationReduction, AppError> {
        begin_observation(observer, self.family().fingerprint());
        let inputs = match self.collect_reduction_inputs(terms) {
            Ok(inputs) => inputs,
            Err(error) => {
                let mut envelope = CandidateRoutedApplicationEnvelope::new(
                    CandidateRoutedApplicationEvent::RequestFinished,
                    self.family().fingerprint(),
                );
                envelope.status = CandidateRoutedApplicationStatus::Failed;
                envelope.request_finished = true;
                envelope.caller_phase = Some("entry_admission");
                observer.emit(envelope);
                return Err(error);
            }
        };
        let reduced = self
            .application
            .reduce_linear_combination_with_observation(
                inputs,
                Self::entry_admission(&self.entry_domain, self.options.entry_coverage_policy),
                observer,
                cancel,
            )
            .map_err(native_failure);
        self.finish_observed_reduction(reduced, observer, cancel)
    }

    /// Observe the existing verified initial-entry routing, native reduction,
    /// and saved terminal substitutions without a second traversal.
    pub fn reduce_routed_linear_combination_with_observation(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        observer: &mut CandidateRoutedApplicationObserver<'_>,
        cancel: Option<&AtomicBool>,
    ) -> Result<SavedApplicationReduction, AppError> {
        begin_observation(observer, self.family().fingerprint());
        let reduced = self
            .application
            .reduce_routed_linear_combination_with_observation(
                terms,
                Self::entry_admission(&self.entry_domain, self.options.entry_coverage_policy),
                observer,
                cancel,
            )
            .map_err(native_failure);
        self.finish_observed_reduction(reduced, observer, cancel)
    }

    pub(super) fn finish_observed_reduction(
        &self,
        reduced: Result<CandidateRoutedReduction, AppError>,
        observer: &mut CandidateRoutedApplicationObserver<'_>,
        cancel: Option<&AtomicBool>,
    ) -> Result<SavedApplicationReduction, AppError> {
        match reduced {
            Ok(reduced) => {
                self.apply_terminal_substitutions_observed(reduced, None, Some(observer), cancel)
            }
            Err(error) => {
                let mut envelope = saved_envelope(
                    observer,
                    self.family().fingerprint(),
                    CandidateRoutedApplicationEvent::RequestFinished,
                    "routed_reduction",
                );
                envelope.status = CandidateRoutedApplicationStatus::Failed;
                envelope.request_finished = true;
                observer.emit(envelope);
                Err(error)
            }
        }
    }

    /// Optional structured progress on the calling thread. The ordinary API
    /// performs no progress clocks or formatting. Observations occur between
    /// native operations, not inside CAS, and do not promise a timed heartbeat.
    /// Core success only finishes `routed_reduction`; `request_finished` becomes
    /// true after terminal substitution succeeds or the request returns an error.
    /// This legacy JSON method observes direct entry requests only. Initial
    /// routing is observed by `reduce_routed_linear_combination_with_observation`.
    /// This legacy callback retains its panic/unwind contract; use the typed
    /// observer methods when callback-failure isolation is required.
    pub fn reduce_linear_combination_with_progress(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        interval: Duration,
        mut observe: impl FnMut(Value),
    ) -> Result<SavedApplicationReduction, AppError> {
        let mut progress = SavedProgress::new(interval, &mut observe);
        progress.emit("running", false, None);
        let result = self.reduce_linear_combination_progress(terms, Some(&mut progress));
        progress.emit(
            if result.is_ok() {
                "succeeded"
            } else {
                "failed"
            },
            true,
            result.as_ref().err().map(|error| error.to_string()),
        );
        result
    }
}

pub(super) fn check_cancellation(cancel: Option<&AtomicBool>) -> Result<(), AppError> {
    if cancel.is_some_and(|cancel| cancel.load(std::sync::atomic::Ordering::Relaxed)) {
        Err(native_failure(
            rustred::solver::CandidateRoutedError::Cancelled,
        ))
    } else {
        Ok(())
    }
}

pub(super) fn begin_observation(
    observer: &mut CandidateRoutedApplicationObserver<'_>,
    family: &str,
) {
    observer.clear_native_summary();
    let mut envelope = CandidateRoutedApplicationEnvelope::new(
        CandidateRoutedApplicationEvent::ActivityStarted,
        family,
    );
    envelope.caller_phase = Some("entry_admission");
    observer.emit(envelope);
}

fn saved_envelope<'a>(
    observer: &CandidateRoutedApplicationObserver<'_>,
    family: &'a str,
    event: CandidateRoutedApplicationEvent,
    phase: &'static str,
) -> CandidateRoutedApplicationEnvelope<'a> {
    let mut envelope = CandidateRoutedApplicationEnvelope::new(event, family);
    envelope.snapshot = observer.last_snapshot();
    envelope.metrics = observer.last_metrics();
    envelope.caller_phase = Some(phase);
    envelope.activity = match phase {
        "terminal_substitution" => CandidateRoutedActivity::TerminalSubstitution,
        "routed_reduction" => CandidateRoutedActivity::Expansion,
        _ => CandidateRoutedActivity::Admission,
    };
    envelope
}

pub(super) struct SavedObservation<'a, 'b> {
    observer: &'a mut CandidateRoutedApplicationObserver<'b>,
    terminal: CandidateRoutedTerminalProgress,
    last: Instant,
}

impl<'a, 'b> SavedObservation<'a, 'b> {
    pub fn new(
        observer: &'a mut CandidateRoutedApplicationObserver<'b>,
        family: &str,
        rows: usize,
    ) -> Option<Self> {
        if observer.status() != CandidateRoutedObserverStatus::Active {
            return None;
        }
        let mut observation = Self {
            observer,
            terminal: CandidateRoutedTerminalProgress {
                rows,
                output_terms: Some(0),
                ..Default::default()
            },
            last: Instant::now(),
        };
        observation.emit(
            family,
            CandidateRoutedApplicationEvent::TerminalSubstitutionStarted,
            CandidateRoutedApplicationStatus::Running,
            false,
        );
        Some(observation)
    }

    pub fn terminal_row(&mut self, family: &str, additions: usize, output_terms: usize) {
        self.terminal.completed_rows += 1;
        self.terminal.additions = additions;
        self.terminal.output_terms = Some(output_terms);
        if self.observer.status() != CandidateRoutedObserverStatus::Active {
            return;
        }
        let interval = self.observer.options().interval;
        if !interval.is_zero() && !self.terminal.completed_rows.is_multiple_of(256) {
            return;
        }
        if self.last.elapsed() >= interval {
            self.emit(
                family,
                CandidateRoutedApplicationEvent::TerminalSubstitutionProgress,
                CandidateRoutedApplicationStatus::Running,
                false,
            );
        }
    }

    pub fn finish(&mut self, family: &str, result: &Result<SavedApplicationReduction, AppError>) {
        if let Ok(reduction) = result {
            self.terminal.output_terms = Some(reduction.terms().len());
            self.terminal.additions = reduction.terminal_substitution_additions;
        } else {
            // A failed request returns no substituted output map.
            self.terminal.output_terms = None;
        }
        self.emit(
            family,
            CandidateRoutedApplicationEvent::RequestFinished,
            if result.is_ok() {
                CandidateRoutedApplicationStatus::Succeeded
            } else {
                CandidateRoutedApplicationStatus::Failed
            },
            true,
        );
    }

    fn emit(
        &mut self,
        family: &str,
        event: CandidateRoutedApplicationEvent,
        status: CandidateRoutedApplicationStatus,
        finished: bool,
    ) {
        if self.observer.status() != CandidateRoutedObserverStatus::Active {
            return;
        }
        let mut envelope = saved_envelope(self.observer, family, event, "terminal_substitution");
        envelope.status = status;
        envelope.request_finished = finished;
        envelope.terminal = Some(self.terminal);
        self.observer.emit(envelope);
        self.last = Instant::now();
    }
}

/// Copy bounded native observation metadata into the saved progress document.
/// The v1 basic fields retain their meanings; the additional scope/event fields
/// distinguish core completion from saved substitutions and caller-owned work.
/// `metrics.output_terms` remains native; substituted output counts are in
/// `terminal.output_terms`. No coefficients, atoms, ETA or closure proof are
/// computed. Degree values outside JSON's integer range use decimal strings.
pub fn saved_application_observation_document(
    envelope: &CandidateRoutedApplicationEnvelope<'_>,
) -> Value {
    use CandidateRoutedApplicationEvent as Event;
    let scope = match envelope.event {
        _ if envelope.activity == CandidateRoutedActivity::Caller => "caller",
        Event::CallerProgress => "caller",
        _ if envelope.caller_phase.is_some_and(|phase| {
            !matches!(
                phase,
                "entry_admission" | "routed_reduction" | "terminal_substitution"
            )
        }) =>
        {
            "caller"
        }
        Event::TerminalSubstitutionStarted
        | Event::TerminalSubstitutionProgress
        | Event::RequestFinished => "saved_application",
        _ if matches!(
            envelope.caller_phase,
            Some("entry_admission" | "routed_reduction" | "terminal_substitution")
        ) =>
        {
            "saved_application"
        }
        _ => "native_core",
    };
    let event_kind = match envelope.event {
        Event::Snapshot => "snapshot",
        Event::ActivityStarted => "activity_started",
        Event::NodeCompleted(_) => "node_completed",
        Event::RootCollected => "root_collected",
        Event::CycleDetected => "cycle_detected",
        Event::CoreFinished => "core_finished",
        Event::TerminalSubstitutionStarted => "terminal_substitution_started",
        Event::TerminalSubstitutionProgress => "terminal_substitution_progress",
        Event::RequestFinished => "request_finished",
        Event::CallerProgress => "caller_progress",
    };
    let completion_source = match envelope.event {
        Event::NodeCompleted(source) => Some(match source {
            CandidateRoutedCompletionSource::Computed => "computed",
            CandidateRoutedCompletionSource::RamHit => "ram_hit",
            CandidateRoutedCompletionSource::BackingHit => "backing_hit",
            CandidateRoutedCompletionSource::Zero => "zero",
            CandidateRoutedCompletionSource::Terminal => "terminal",
            CandidateRoutedCompletionSource::UnresolvedPreserved => "unresolved_preserved",
        }),
        _ => None,
    };
    let activity = match envelope.activity {
        CandidateRoutedActivity::Admission => "admission",
        CandidateRoutedActivity::Expansion => "expansion",
        CandidateRoutedActivity::OwnerPreparation => "owner_preparation",
        CandidateRoutedActivity::Transport => "transport",
        CandidateRoutedActivity::RuleEvaluation => "rule_evaluation",
        CandidateRoutedActivity::CoefficientCombination => "coefficient_combination",
        CandidateRoutedActivity::RootMaterialization => "root_materialization",
        CandidateRoutedActivity::RootCollection => "root_collection",
        CandidateRoutedActivity::TerminalSubstitution => "terminal_substitution",
        CandidateRoutedActivity::Caller => "caller",
    };
    let status = match envelope.status {
        CandidateRoutedApplicationStatus::Running => "running",
        CandidateRoutedApplicationStatus::Succeeded => "succeeded",
        CandidateRoutedApplicationStatus::Failed => "failed",
    };
    let metrics = envelope.metrics;
    let complexity = envelope.complexity;
    let terminal = envelope.terminal;
    json!({
        "schema":"rustred.saved-application-progress.v1",
        "event":"application_progress", "sequence":envelope.sequence,
        "elapsed_seconds":envelope.elapsed.as_secs_f64(),
        "phase":envelope.caller_phase.unwrap_or("routed_reduction"),
        "status":status, "request_finished":envelope.request_finished,
        "core":envelope.snapshot.as_ref().map(core_json),
        "core_summary_scope":envelope.snapshot.map(|_| "latest_native_request"),
        "core_summary_live":envelope.snapshot.is_some() && scope == "native_core",
        "core_summary_age_seconds":envelope.native_summary_age.map(|age| age.as_secs_f64()),
        "terminal_rows":terminal.map_or(0, |value| value.rows),
        "completed_terminal_rows":terminal.map_or(0, |value| value.completed_rows),
        "terminal_substitution_additions":terminal.map_or(0, |value| value.additions),
        "error":null, "closure_claim":false,
        "observation_scope":scope, "event_kind":event_kind,
        "caller_phase":envelope.caller_phase, "activity":activity,
        "completion_source":completion_source,
        "family_fingerprint":envelope.family_fingerprint,
        "root":envelope.root.map(work_json), "current":envelope.current.map(work_json),
        "degrees":envelope.degrees.map(degrees_json),
        "metrics":{
            "completed_unique_operational_nodes":metrics.completed_unique_operational_nodes,
            "completion_attempts":metrics.completion_attempts,
            "completed_roots":metrics.completed_roots, "requested_roots":metrics.requested_roots,
            "output_terms":metrics.output_terms,
            "observed_descent_checks":metrics.observed_descent_checks,
            "max_seen_degrees":degrees_json(metrics.max_seen_degrees),
        },
        "frontier":envelope.frontier.map(|value| json!({
            "frames_examined":value.frames_examined,
            "total_pending_frames":value.total_pending_frames,
            "unique_nodes":value.unique_nodes, "unique_integrals":value.unique_integrals,
            "exact":value.exact, "truncated":!value.exact,
        })),
        "active_path":envelope.active_path.iter().copied().map(work_json).collect::<Vec<_>>(),
        "path_total_nodes":envelope.path_total_nodes,
        "path_truncated":envelope.active_path.len() < envelope.path_total_nodes,
        "cycle_backedge":envelope.cycle_backedge,
        "complexity":{
            "terms":complexity.terms, "coefficient_terms":complexity.coefficient_terms,
            "coefficient_bytes":complexity.coefficient_bytes, "factorized":complexity.factorized,
            "contribution_index":complexity.contribution_index,
            "contribution_total":complexity.contribution_total,
        },
        "terminal":terminal.map(|value| json!({
            "rows":value.rows, "completed_rows":value.completed_rows,
            "additions":value.additions, "output_terms":value.output_terms,
        })),
        "global_remaining_work_known":false, "eta_seconds":null, "percent_complete":null,
        "arithmetic_internals_known":false,
    })
}

fn work_json(work: CandidateRoutedWorkRef<'_>) -> Value {
    match work {
        CandidateRoutedWorkRef::Route(key) => json!({"kind":"route", "powers":key.powers()}),
        CandidateRoutedWorkRef::Apply {
            owner_sector,
            target,
        } => json!({"kind":"apply", "owner_sector":owner_sector, "powers":target.powers()}),
    }
}

fn degrees_json(degrees: CandidateRoutedKeyDegrees) -> Value {
    fn unsigned(value: u128) -> Value {
        u64::try_from(value).map_or_else(|_| json!(value.to_string()), |value| json!(value))
    }
    fn signed(value: i128) -> Value {
        i64::try_from(value).map_or_else(|_| json!(value.to_string()), |value| json!(value))
    }
    json!({
        "numerator_rank":unsigned(degrees.numerator_rank),
        "positive_power_sum":unsigned(degrees.positive_power_sum),
        "power_difference":signed(degrees.power_difference),
        "support":degrees.support, "dot_excess":unsigned(degrees.dot_excess),
    })
}

pub(super) struct SavedProgress<'a> {
    observe: &'a mut dyn FnMut(Value),
    pub interval: Duration,
    started: Instant,
    last: Instant,
    sequence: u64,
    phase: &'static str,
    core: Option<CandidateRoutedApplicationSnapshot>,
    terminal_rows: usize,
    completed_terminal_rows: usize,
    terminal_additions: usize,
}

impl<'a> SavedProgress<'a> {
    fn new(interval: Duration, observe: &'a mut dyn FnMut(Value)) -> Self {
        let started = Instant::now();
        Self {
            observe,
            interval,
            started,
            last: started,
            sequence: 0,
            phase: "entry_admission",
            core: None,
            terminal_rows: 0,
            completed_terminal_rows: 0,
            terminal_additions: 0,
        }
    }

    pub fn routed(&mut self, snapshot: &CandidateRoutedApplicationSnapshot) {
        self.phase = "routed_reduction";
        self.core = Some(*snapshot);
        // A completed native reduction still has terminal substitutions ahead.
        self.emit("running", false, None);
    }

    pub fn terminal_start(&mut self, rows: usize) {
        self.phase = "terminal_substitution";
        self.terminal_rows = rows;
        self.emit("running", false, None);
    }

    pub fn terminal_row(&mut self, additions: usize) {
        self.completed_terminal_rows += 1;
        self.terminal_additions = additions;
        if !self.interval.is_zero() && self.completed_terminal_rows % 256 != 0 {
            return;
        }
        if self.last.elapsed() >= self.interval {
            self.emit("running", false, None);
        }
    }

    fn emit(&mut self, status: &str, finished: bool, error: Option<String>) {
        let now = Instant::now();
        self.sequence = self.sequence.saturating_add(1);
        self.last = now;
        (self.observe)(json!({
            "schema":"rustred.saved-application-progress.v1",
            "event":"application_progress", "sequence":self.sequence,
            "elapsed_seconds":now.duration_since(self.started).as_secs_f64(),
            "phase":self.phase, "status":status, "request_finished":finished,
            "core":self.core.as_ref().map(core_json),
            "terminal_rows":self.terminal_rows,
            "completed_terminal_rows":self.completed_terminal_rows,
            "terminal_substitution_additions":self.terminal_additions,
            "error":error, "closure_claim":false,
        }));
    }
}

fn census(value: CandidateRoutedCacheCensus) -> Value {
    json!({"entries":value.entries,"coefficient_terms":value.coefficient_terms,
        "coefficient_bytes":value.coefficient_bytes})
}

fn core_json(snapshot: &CandidateRoutedApplicationSnapshot) -> Value {
    let stats = snapshot.statistics;
    let phase = match snapshot.phase {
        CandidateRoutedApplicationPhase::Admission => "admission",
        CandidateRoutedApplicationPhase::Expansion => "expansion",
        CandidateRoutedApplicationPhase::Combination => "combination",
        CandidateRoutedApplicationPhase::RootCollection => "root_collection",
    };
    let status = match snapshot.status {
        CandidateRoutedApplicationStatus::Running => "running",
        CandidateRoutedApplicationStatus::Succeeded => "succeeded",
        CandidateRoutedApplicationStatus::Failed => "failed",
    };
    json!({"sequence":snapshot.sequence,"elapsed_seconds":snapshot.elapsed.as_secs_f64(),
        "phase":phase,"status":status,"input_terms":stats.input_terms,
        "requested_targets":stats.requested_targets,"unique_nodes":snapshot.unique_nodes,
        "active_nodes":snapshot.active_nodes,"pending_frames":snapshot.pending_frames,
        "rule_applications":stats.work.rule_applications(),"cache_hits":stats.work.cache_hits(),
        "coalescing_additions":stats.work.coalescing_additions(),
        "transport_calls":stats.transport_calls,"transport_operations":stats.transport_operations,
        "transport_endpoints":stats.transport_endpoints,
        "evicted_entries":stats.cache.evicted_entries,
        "evicted_coefficient_terms":stats.cache.evicted_coefficient_terms,
        "evicted_coefficient_bytes":stats.cache.evicted_coefficient_bytes,
        "recomputed_nodes":stats.cache.recomputed_nodes,
        "cached":census(snapshot.cached),"pinned":snapshot.pinned.map(census),
        "eligible":snapshot.eligible.map(census)})
}
