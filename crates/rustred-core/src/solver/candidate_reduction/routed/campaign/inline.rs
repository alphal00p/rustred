//! Bounded caller-thread execution of the existing exact FIFO kernel.
//! No new algebra, thread pool, shared cache or certification authority.
use super::*;

/// Additional aggregate request allowances, intersected with the admitted
/// reducer's existing limits. Native per-formula/scratch limits are unchanged.
/// Attempts, conservative reservations and failed prefixes consume allowances.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateRoutedWorkBudget {
    pub max_nodes: usize,
    pub max_rule_applications: usize,
    pub max_transport_calls: usize,
    pub max_transport_operations: usize,
    pub max_transport_endpoints: usize,
    pub max_coalescing_additions: usize,
}

impl<const N: usize> RoutedCandidateReducer<N> {
    /// Run one fresh FIFO request on the calling thread, without spawning a
    /// worker or cloning prepared algebra. This is the same checked processing
    /// kernel as `trace_targets_parallel_with_entry_admission_and_observer`.
    ///
    /// Cancellation is checked between native operations, not inside Symbolica.
    /// The observer runs at admission, after each 256 local expansions and at
    /// return. A panic in the observer unwinds on the caller; no worker detaches.
    /// A finished trace can still have frontiers and is not a certificate.
    pub fn trace_targets_inline_with_entry_admission_and_observer(
        &self,
        targets: impl IntoIterator<Item = IntegralKey>,
        admission: CandidateEntryAdmission<'_, N>,
        budget: CandidateRoutedWorkBudget,
        cancellation: &AtomicBool,
        mut observer: impl FnMut(&CandidateRoutedCampaignSnapshot<N>),
    ) -> Result<CandidateRoutedCampaignReport<N>, CandidateRoutedCampaignError<N>> {
        let mut shared = Shared::new(self, 1, cancellation);
        shared.limits.max_input_targets = shared.limits.max_input_targets.min(budget.max_nodes);
        shared.limits.max_unique_nodes = shared.limits.max_unique_nodes.min(budget.max_nodes);
        shared.limits.max_transport_calls = shared
            .limits
            .max_transport_calls
            .min(budget.max_transport_calls);
        shared.limits.max_transport_operations = shared
            .limits
            .max_transport_operations
            .min(budget.max_transport_operations);
        shared.limits.max_transport_endpoints = shared
            .limits
            .max_transport_endpoints
            .min(budget.max_transport_endpoints);
        shared.reduction.max_pending_frames =
            shared.reduction.max_pending_frames.min(budget.max_nodes);
        shared.reduction.max_rule_applications = shared
            .reduction
            .max_rule_applications
            .min(budget.max_rule_applications);
        shared.reduction.max_coalescing_additions = shared
            .reduction
            .max_coalescing_additions
            .min(budget.max_coalescing_additions);
        if let Err(error) = shared.prepare_with_entry_admission(self, targets, admission) {
            shared.fail(error);
        } else {
            observer(&shared.snapshot());
            worker::run_with_progress(self, &shared, || observer(&shared.snapshot()));
        }
        let result = shared.into_result();
        observer(match &result {
            Ok(report) => report.snapshot(),
            Err(error) => error.snapshot(),
        });
        result
    }
}

#[cfg(test)]
mod tests;
