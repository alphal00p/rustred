//! Operational settings and explicit partial application, independent of stored scope.
use crate::AppError;
use rustred::persistence::BinaryIoLimits;
use rustred::reduction::ReductionLimits;
use rustred::solver::{CandidateRoutedCachePolicy, RoutedCandidateLimits, UnresolvedPolicy};

/// Recorded coverage is independent of requiring a complete terminal result.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SavedApplicationEntryCoveragePolicy {
    /// Preserve retains its historical advisory behavior; Reject enforces the
    /// recorded starting union. This is the backward-compatible default.
    #[default]
    FollowUnresolvedPolicy,
    /// Attempt valid inputs outside the recorded union without changing it.
    /// Combining this with Reject requires an exact fully reduced result.
    Advisory,
}

/// Operational allowances never enlarge the recorded mathematical entry scope.
/// Partial application makes that scope advisory without changing its records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavedApplicationOptions {
    /// Preserve exact uncovered leaves and make recorded entry coverage advisory.
    /// This does not relax formula applicability, source guards, or resource limits.
    pub unresolved_policy: UnresolvedPolicy,
    pub entry_coverage_policy: SavedApplicationEntryCoveragePolicy,
    pub reduction_limits: ReductionLimits,
    pub trace_limits: RoutedCandidateLimits,
    pub binary_limits: BinaryIoLimits,
    pub cache_representation: rustred::solver::CandidateCacheRepresentation,
    /// Completed results may be discarded and recomputed. Live dependencies
    /// remain protected; this allowance is not a process-wide memory limit.
    pub cache_policy: CandidateRoutedCachePolicy,
    /// Existing bounded deterministic route-verification pool. This does not
    /// change reduction scope or create parallel formula-application workers.
    pub preparation_workers: usize,
}
impl Default for SavedApplicationOptions {
    fn default() -> Self {
        Self {
            unresolved_policy: UnresolvedPolicy::Reject,
            entry_coverage_policy: SavedApplicationEntryCoveragePolicy::default(),
            reduction_limits: Default::default(),
            trace_limits: Default::default(),
            binary_limits: Default::default(),
            cache_representation: Default::default(),
            cache_policy: CandidateRoutedCachePolicy::EvictLeastRecentlyUsed,
            preparation_workers: 1,
        }
    }
}
impl SavedApplicationOptions {
    pub(super) fn validate_preparation(&self) -> Result<(), AppError> {
        if self.preparation_workers == 0 {
            return Err(AppError::input("preparation_workers must be positive"));
        }
        rustred::campaign::ParallelExecution::preflight_requested_core_budget(
            self.preparation_workers,
        )
        .map_err(|error| AppError::input(error.to_string()))
    }
}
