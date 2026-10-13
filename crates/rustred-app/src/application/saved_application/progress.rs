//! Saved-application adapter for native safe-boundary observations.
use super::*;
use rustred::solver::{
    CandidateRoutedApplicationPhase, CandidateRoutedApplicationSnapshot,
    CandidateRoutedApplicationStatus, CandidateRoutedCacheCensus,
};
use serde_json::json;

impl<const N: usize> PreparedSavedApplication<N> {
    /// Optional structured progress on the calling thread. The ordinary API
    /// performs no progress clocks or formatting. Observations occur between
    /// native operations, not inside CAS, and do not promise a timed heartbeat.
    /// Core success only finishes `routed_reduction`; `request_finished` becomes
    /// true after terminal substitution succeeds or the request returns an error.
    /// This observes direct entry requests through `reduce_linear_combination`.
    /// The initial-entry `reduce_routed_linear_combination` API has no observed
    /// wrapper; these events do not cover that separate caller-side workflow.
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
