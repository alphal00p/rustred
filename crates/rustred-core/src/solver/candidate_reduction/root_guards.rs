//! One request-owned guard union for an isolated fresh starting root only.
use super::{CandidateDecomposition, CandidateReducer, CandidateReductionError};
use crate::algebra::Coefficient;
use crate::family::IntegralKey;
use crate::reduction::{GuardedConditionBudget, guard_union::GuardUnion};
use std::sync::Arc;

impl<const N: usize> CandidateReducer<N> {
    pub(super) fn reduce_with_root_guard_union(
        &mut self,
        target: &IntegralKey,
    ) -> Result<
        (
            CandidateDecomposition,
            Vec<Coefficient>,
            GuardedConditionBudget,
        ),
        CandidateReductionError,
    > {
        let limits = self.limits;
        let mut conditions = Vec::new();
        let mut lease = GuardedConditionBudget::new(Arc::clone(&self.cache_budget), limits);
        let result = {
            let mut union = GuardUnion::try_new(&mut conditions)?;
            let mut retain = |local: &[Coefficient]| {
                union
                    .extend_with(local, |guard| lease.retain(guard, limits))
                    .map_err(CandidateReductionError::from)
            };
            // The caller has proved an empty memo. Preorder local vectors
            // equal legacy local-then-sorted-child unions after first-seen
            // deduplication. Every inner hit was first visited in this same
            // request; no external/imported guardless node can enter here.
            self.reduce_unit_mass_impl_with_root_guards(target, true, Some(&mut retain))?
        };
        // The request lease covers guards concurrently with native memo and
        // outer roots through the whole solve, and releases on error/unwind.
        // Returned output buffers are transient until the root cache admits
        // the complete guarded record under its ordinary retained census.
        Ok((result, conditions, lease))
    }
}
