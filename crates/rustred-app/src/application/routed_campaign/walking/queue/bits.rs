//! Packed necessary-condition words tested before `DomainPowerSummary::contains`.
//!
//! One `u64` per indexed candidate records, for every implication that exact
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

    /// True when the bit tier alone proves `container` cannot contain
    /// `candidate`. The caller has already charged the comparison.
    #[inline]
    pub fn rejects(self, container: u64, candidate: u64) -> bool {
        #[cfg(test)]
        if !self.enabled {
            return false;
        }
        !may_contain(container, candidate)
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
    /// decided every candidate below the snapshot watermark.
    pub prepared_retirements_applied: usize,
    /// Commits that applied the trivial empty set of a phase/owner bucket
    /// absent at the snapshot: correct, but no helper comparison decided
    /// anything, so not evidence of helper effectiveness.
    pub prepared_retirements_trivial: usize,
    /// Commits that had a prepared lookup but retired with the serial scan.
    pub prepared_retire_fallbacks: usize,
}

impl SessionCounters {
    #[inline]
    pub fn forward(&mut self, rejected: bool) {
        self.forward_callbacks = self.forward_callbacks.saturating_add(1);
        self.forward_bit_rejections = self
            .forward_bit_rejections
            .saturating_add(usize::from(rejected));
    }

    #[inline]
    pub fn reverse(&mut self, rejected: bool) {
        self.reverse_callbacks = self.reverse_callbacks.saturating_add(1);
        self.reverse_bit_rejections = self
            .reverse_bit_rejections
            .saturating_add(usize::from(rejected));
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
            "counter_scope": "coordinator_commit_path_current_process_session; not_persisted; speculative_helper_work_reported_by_admission_preparation",
            "bit_layout": "0-15 upper=inf per axis; 16-31 lower=0 per axis; 32 A upper=inf; 33 R upper=inf; 34 D lower=-inf; 35 D upper=inf; 36 A lower=0; 37 R lower=0; 38 D lower<=0 or -inf",
            "results_and_persisted_counters_unchanged": true
        })
    }
}
