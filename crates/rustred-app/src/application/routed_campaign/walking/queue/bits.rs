//! Packed necessary-condition words tested before `DomainPowerSummary::contains`.
//!
//! One `u64` per indexed candidate (stored inline in its index block) records, for every implication that exact
//! inclusion forces, a one-bit "the candidate side has this property" flag
//! whose container side must then hold as well. A single subset test rejects
//! most comparisons before the O(N) tight-extrema comparison runs. The word is
//! a prefilter only: admission results, retirements, persisted counters and
//! replay tokens are unchanged, and `containment_checks` is still charged for
//! every callback whether or not the bit test rejects it.
//!
//! Bit layout (the first `MAX_ARITY` = 16 axes; any further axis is simply
//! not packed, which weakens but never invalidates the necessary condition):
//!   0..16   coordinate upper bound is +infinity, per axis
//!   16..32  coordinate lower bound is zero, per axis
//!   32      A (positive power) upper bound is +infinity
//!   33      R (numerator rank) upper bound is +infinity
//!   34      D (power difference) lower bound is -infinity
//!   35      D upper bound is +infinity
//!   36      A lower bound is zero
//!   37      R lower bound is zero
//!   38      D lower bound is -infinity or <= 0
//!
//! Implications used, for a nonempty container C and nonempty candidate Q with
//! C ⊇ Q (every C bound encloses the corresponding Q bound):
//!   Q.upper[i] = +inf     => C.upper[i] = +inf
//!   Q.lower[i] = 0        => C.lower[i] <= 0, i.e. C.lower[i] = 0
//!   Q.A/R upper = +inf    => C.A/R upper = +inf
//!   Q.A/R lower = 0       => C.A/R lower = 0
//!   Q.D lower = -inf      => C.D lower = -inf
//!   Q.D upper = +inf      => C.D upper = +inf
//!   Q.D lower <= 0 / -inf => C.D lower <= Q.D lower <= 0, or -inf
//! Every implication reads "candidate bit set => container bit set", so the
//! whole word is checked by `candidate & !container == 0`.
//!
//! An empty summary has word 0: it is contained by everything, and word 0
//! passes every container. As a container an empty summary passes only
//! all-zero candidates, which the exact comparison then rejects.
use rustred::solver::DomainPowerSummary;
use serde_json::{Value, json};

/// Coordinate axes packed into bits 0..16 and 16..32.
pub(super) const MAX_ARITY: usize = 16;

pub(super) fn word<const N: usize>(summary: &DomainPowerSummary<N>) -> u64 {
    let Some(extrema) = summary.extrema() else {
        return 0;
    };
    let mut word = 0_u64;
    for (axis, (&lower, &upper)) in extrema
        .lower()
        .iter()
        .zip(extrema.upper())
        .take(MAX_ARITY)
        .enumerate()
    {
        word |= u64::from(upper.is_none()) << axis;
        word |= u64::from(lower == 0) << (MAX_ARITY + axis);
    }
    let (a_lower, a_upper) = extrema.positive_power();
    let (r_lower, r_upper) = extrema.numerator_rank();
    let (d_lower, d_upper) = extrema.power_difference();
    word |= u64::from(a_upper.is_none()) << 32;
    word |= u64::from(r_upper.is_none()) << 33;
    word |= u64::from(d_lower.is_none()) << 34;
    word |= u64::from(d_upper.is_none()) << 35;
    word |= u64::from(a_lower == 0) << 36;
    word |= u64::from(r_lower == 0) << 37;
    word |= u64::from(d_lower.is_none_or(|d| d <= 0)) << 38;
    word
}

/// Necessary condition for `container.contains(candidate)`; never sufficient.
/// (The kernel evaluates it on whole blocks; this is the per-pair reference.)
#[cfg(test)]
#[inline]
pub(super) fn may_contain(container: u64, candidate: u64) -> bool {
    candidate & !container == 0
}

/// Production always filters. Tests can switch the tier off on one queue to
/// prove that results and persisted counters do not depend on it.
#[derive(Clone, Copy)]
pub(super) struct Prefilter {
    #[cfg(test)]
    enabled: bool,
}

impl Prefilter {
    pub const fn new() -> Self {
        Self {
            #[cfg(test)]
            enabled: true,
        }
    }

    #[cfg(test)]
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Whether the kernel prefilter (bit words and lanes) runs; always true
    /// in production.
    #[inline]
    pub fn enabled(self) -> bool {
        #[cfg(test)]
        return self.enabled;
        #[cfg(not(test))]
        true
    }
}

/// Coordinator-side session telemetry for the filter tier; never persisted and
/// never an admission or checkpoint authority. Speculative helper work is
/// reported separately by the admission engine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in super::super) struct SessionCounters {
    pub forward_callbacks: usize,
    pub forward_bit_rejections: usize,
    pub reverse_callbacks: usize,
    pub reverse_bit_rejections: usize,
    /// Commits whose reverse retirement applied a helper-prepared set that
    /// decided the IDs below its snapshot watermark.
    pub prepared_retirements_applied: usize,
    /// Commits whose prepared set was empty only because the phase/owner
    /// bucket was absent at the snapshot (created by an earlier commit of the
    /// same batch): every retirement was decided at commit time.
    pub prepared_retirements_trivial: usize,
    /// Commits that had a prepared lookup but retired with the serial scan.
    pub prepared_retire_fallbacks: usize,
    /// Kernel attribution (coordinator scans only): forward lookups and
    /// reverse retirements that ran an index scan, the candidates of theirs
    /// that reached the exact predicate, and the scans' wall nanoseconds.
    /// `callbacks - bit_rejections - tests` were rejected by the u8 lanes.
    pub forward_scans: usize,
    pub forward_tests: usize,
    pub forward_scan_nanos: u64,
    pub reverse_scans: usize,
    pub reverse_tests: usize,
    /// Wall of the whole reverse retirement call (see `kernel_json`).
    pub reverse_scan_nanos: u64,
    /// Examined reverse candidates that a helper-prepared set decided: the
    /// retirement traversal and block kernel still visit them, but they reach
    /// no callback, so `reverse_callbacks` does not count them.
    pub reverse_prepared_candidates: usize,
}

impl SessionCounters {
    /// One forward candidate that reached the exact predicate.
    #[inline]
    pub fn forward_test(&mut self) {
        self.forward_callbacks = self.forward_callbacks.saturating_add(1);
        self.forward_tests = self.forward_tests.saturating_add(1);
    }

    /// One reverse candidate that reached the exact predicate.
    #[inline]
    pub fn reverse_test(&mut self) {
        self.reverse_callbacks = self.reverse_callbacks.saturating_add(1);
        self.reverse_tests = self.reverse_tests.saturating_add(1);
    }

    /// One coordinator forward scan and its wall time.
    #[inline]
    pub fn forward_scan(&mut self, started: std::time::Instant) {
        self.forward_scans = self.forward_scans.saturating_add(1);
        self.forward_scan_nanos = self
            .forward_scan_nanos
            .saturating_add(started.elapsed().as_nanos() as u64);
    }

    /// One coordinator reverse retirement scan and its wall time.
    #[inline]
    pub fn reverse_scan(&mut self, started: std::time::Instant) {
        self.reverse_scans = self.reverse_scans.saturating_add(1);
        self.reverse_scan_nanos = self
            .reverse_scan_nanos
            .saturating_add(started.elapsed().as_nanos() as u64);
    }

    /// `count` forward callbacks rejected by the kernel prefilter, `words` of
    /// them by the bit word.
    #[inline]
    pub fn forward_run(&mut self, count: usize, words: usize) {
        self.forward_callbacks = self.forward_callbacks.saturating_add(count);
        self.forward_bit_rejections = self.forward_bit_rejections.saturating_add(words);
    }

    #[inline]
    pub fn reverse_run(&mut self, count: usize, words: usize) {
        self.reverse_callbacks = self.reverse_callbacks.saturating_add(count);
        self.reverse_bit_rejections = self.reverse_bit_rejections.saturating_add(words);
    }

    /// `count` examined reverse candidates decided by a helper-prepared set.
    #[inline]
    pub fn reverse_decided(&mut self, count: usize) {
        self.reverse_prepared_candidates = self.reverse_prepared_candidates.saturating_add(count);
    }

    pub fn json(&self) -> Value {
        json!({
            "forward_callbacks": self.forward_callbacks,
            "forward_bit_rejections": self.forward_bit_rejections,
            "reverse_callbacks": self.reverse_callbacks,
            "reverse_bit_rejections": self.reverse_bit_rejections,
            "prepared_retirements_applied": self.prepared_retirements_applied,
            "prepared_retirements_trivial": self.prepared_retirements_trivial,
            "prepared_retire_fallbacks": self.prepared_retire_fallbacks,
            "forward_lane_rejections": self.forward_lane_rejections(),
            "forward_exact_tests": self.forward_tests,
            "reverse_lane_rejections": self.reverse_lane_rejections(),
            "reverse_exact_tests": self.reverse_tests,
            "counter_scope": "coordinator_commit_path_current_process_session; not_persisted; speculative_helper_work_reported_by_admission_preparation",
            "bit_layout": "0-15 upper=inf per axis; 16-31 lower=0 per axis; 32 A upper=inf; 33 R upper=inf; 34 D lower=-inf; 35 D upper=inf; 36 A lower=0; 37 R lower=0; 38 D lower<=0 or -inf",
            "results_and_persisted_counters_unchanged": true
        })
    }

    fn forward_lane_rejections(&self) -> usize {
        self.forward_callbacks
            .saturating_sub(self.forward_bit_rejections)
            .saturating_sub(self.forward_tests)
    }

    fn reverse_lane_rejections(&self) -> usize {
        self.reverse_callbacks
            .saturating_sub(self.reverse_bit_rejections)
            .saturating_sub(self.reverse_tests)
    }

    /// Kernel attribution for `parallel.coordinator_duty.admission_kernel`:
    /// scans, candidates per scan and wall ns per check-equivalent (one
    /// logical candidate, the `containment_checks` unit) on the coordinator.
    /// Forward: pure index scans. Reverse: the whole retirement call, whose
    /// traversal examines helper-decided candidates too, so its per-candidate
    /// figure divides by every examined candidate, not by the callbacks.
    pub fn kernel_json(&self) -> Value {
        let per = |numerator: f64, denominator: usize| {
            (denominator > 0).then(|| numerator / denominator as f64)
        };
        let reverse_examined = self
            .reverse_callbacks
            .saturating_add(self.reverse_prepared_candidates);
        json!({
            "kernel": "struct_of_arrays_u8_lanes_v1",
            "forward_scans": self.forward_scans,
            "forward_candidates": self.forward_callbacks,
            "forward_word_rejections": self.forward_bit_rejections,
            "forward_lane_rejections": self.forward_lane_rejections(),
            "forward_exact_tests": self.forward_tests,
            "forward_scan_seconds": self.forward_scan_nanos as f64 * 1e-9,
            "forward_candidates_per_scan": per(self.forward_callbacks as f64, self.forward_scans),
            "forward_ns_per_candidate": per(self.forward_scan_nanos as f64, self.forward_callbacks),
            "reverse_scans": self.reverse_scans,
            "reverse_candidates": self.reverse_callbacks,
            "reverse_word_rejections": self.reverse_bit_rejections,
            "reverse_lane_rejections": self.reverse_lane_rejections(),
            "reverse_exact_tests": self.reverse_tests,
            "reverse_scan_seconds": self.reverse_scan_nanos as f64 * 1e-9,
            "reverse_prepared_candidates": self.reverse_prepared_candidates,
            "reverse_examined_candidates": reverse_examined,
            "reverse_retire_ns_per_examined_candidate": per(self.reverse_scan_nanos as f64, reverse_examined),
            // Kept short: the final event must stay below 8 KiB. Forward:
            // the serial and revalidation index scans. Reverse: the whole
            // retirement call (traversal and kernel over every examined
            // candidate, exact tests, compaction, empty block/group removal,
            // the ledger transfer per retirement; not the insertion).
            "scope": "commit scans this session, wall ns, nested in ordered_commit_seconds; reverse = whole retire call over examined (callbacks + helper-decided); not persisted"
        })
    }
}
