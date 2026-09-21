use super::super::{CandidateRoutedError, CandidateRoutedTraceReport};
use crate::family::IntegralKey;
use std::time::Duration;

/// Identity in one immutable campaign snapshot. Owner and phase are part of
/// identity: an equal physical key in another phase is not interchangeable.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CandidateRoutedWork<const N: usize> {
    Route(IntegralKey),
    Apply {
        owner_sector: [bool; N],
        target: IntegralKey,
    },
}
impl<const N: usize> CandidateRoutedWork<N> {
    pub fn target(&self) -> &IntegralKey {
        match self {
            Self::Route(target) | Self::Apply { target, .. } => target,
        }
    }
}

/// Coherent scheduler snapshot. Completed nodes have finished their *local*
/// expansion; this does not mean their descendants or any input ray are solved.
/// Counters are aggregate over all inputs, not sums of independent traces.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CandidateRoutedCampaignSnapshot<const N: usize> {
    pub elapsed: Duration,
    pub workers: usize,
    pub input_targets: usize,
    pub requested_targets: usize,
    pub scheduled_nodes: usize,
    pub queued_nodes: usize,
    pub active_nodes: usize,
    pub completed_nodes: usize,
    pub failed_nodes: usize,
    pub deduplication_hits: usize,
    pub reachable_integrals: usize,
    pub rule_attempts: usize,
    pub rule_applications: usize,
    pub transport_calls: usize,
    pub transport_operations: usize,
    pub transport_endpoints: usize,
    pub coalescing_additions: usize,
    pub reserved_coalescing_additions: usize,
    pub declared_terminals: usize,
    pub visited_zeros: usize,
    pub missing_owners: usize,
    pub missing_rules: usize,
    pub max_numerator_rank: u128,
    pub max_dot_excess: u128,
    /// At most `workers` descriptors; no formulas/coefficients are cloned.
    pub active: Vec<CandidateRoutedWork<N>>,
    /// First typed failure, exposed while other native calls are still draining.
    /// Later failed nodes do not replace this cause. None does not imply closure.
    pub first_failure: Option<CandidateRoutedCampaignFailure>,
    /// Originating node when failure was first reported by a worker; absent for
    /// preflight, external cancellation, spawn or final invariant failures.
    pub first_failure_work: Option<CandidateRoutedWork<N>>,
    /// True only after every scheduled node completed without error/cancel.
    /// Even true does not mean zero frontier or parametric family coverage.
    pub finished: bool,
}

/// A finite shared trace, without coefficient back-substitution or a closure
/// certificate. Sorted terminal/frontier sets are scheduling-independent.
#[derive(Clone, Debug)]
pub struct CandidateRoutedCampaignReport<const N: usize> {
    pub(super) trace: CandidateRoutedTraceReport<N>,
    pub(super) snapshot: CandidateRoutedCampaignSnapshot<N>,
}
impl<const N: usize> CandidateRoutedCampaignReport<N> {
    pub fn trace(&self) -> &CandidateRoutedTraceReport<N> {
        &self.trace
    }
    pub fn snapshot(&self) -> &CandidateRoutedCampaignSnapshot<N> {
        &self.snapshot
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CandidateRoutedCampaignFailure {
    Trace(CandidateRoutedError),
    Cancelled,
    WorkerPanicked,
}
impl From<CandidateRoutedError> for CandidateRoutedCampaignFailure {
    fn from(error: CandidateRoutedError) -> Self {
        Self::Trace(error)
    }
}
impl From<super::super::super::CandidateReductionError> for CandidateRoutedCampaignFailure {
    fn from(error: super::super::super::CandidateReductionError) -> Self {
        Self::Trace(error.into())
    }
}

/// An incomplete trace, including its observable partial frontier. This type
/// never promotes pending work, an error, or cancellation into a terminal.
#[derive(Clone, Debug)]
pub struct CandidateRoutedCampaignError<const N: usize> {
    pub(super) reason: CandidateRoutedCampaignFailure,
    pub(super) partial: CandidateRoutedCampaignReport<N>,
}
impl<const N: usize> CandidateRoutedCampaignError<N> {
    pub fn reason(&self) -> &CandidateRoutedCampaignFailure {
        &self.reason
    }
    pub fn partial_report(&self) -> &CandidateRoutedCampaignReport<N> {
        &self.partial
    }
    pub fn snapshot(&self) -> &CandidateRoutedCampaignSnapshot<N> {
        &self.partial.snapshot
    }
}
impl<const N: usize> std::fmt::Display for CandidateRoutedCampaignError<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "incomplete routed campaign: {:?}", self.reason)
    }
}
impl<const N: usize> std::error::Error for CandidateRoutedCampaignError<N> {}
