//! Rebind an independently checked persisted partial result to this snapshot.

use super::{
    BoundOwnerOverlay, BoundOwnerSearch, CandidateOwnerPrograms, OwnerDomainAttemptLimits,
    OwnerFeedbackError, OwnerOverlayLimits, OwnerOverlayUsage, validate_solution_order,
};
use crate::foundry::artifact::{SourcePortAudit, SourcePortLimits, SourcePortRuleReplayAudit};
use crate::solver::{FiniteCasePolicy, SectorDomainSolution};
use std::sync::Arc;

impl<const N: usize> CandidateOwnerPrograms<N> {
    /// Cheap immutable lineage binding, not source replay or coverage authority.
    pub fn owns_domain_overlay(&self, overlay: &BoundOwnerOverlay<N>) -> bool {
        Arc::ptr_eq(&self.lineage, &overlay.lineage)
            && self.owners.get(&overlay.sector).is_some_and(|owner| {
                owner.root == overlay.root && owner.ordering == overlay.ordering
            })
    }
}

impl<const N: usize> BoundOwnerSearch<N> {
    /// Restore rules-only partial transport after exact cold source replay and
    /// strict-descent checks, assigning this immutable owner's current lineage.
    ///
    /// The caller must have checked durable family/base-owner identity before
    /// decoding the native algebra. This method independently regenerates the
    /// owner's sources and verifies every retained equation and declared guard;
    /// it does not trust a serialized source certificate. Search-policy metadata
    /// is prospective provenance, not evidence that a historical search occurred.
    ///
    /// No finite residual is accepted, no batch is published here, and successful
    /// replay never claims requested-case or recursive RHS coverage. Subsequent
    /// append/application must still discharge all reachable obligations.
    pub fn restore_replayed_residual_free_overlay(
        &self,
        solution: SectorDomainSolution<N>,
        attempt_limits: OwnerDomainAttemptLimits,
        overlay_limits: OwnerOverlayLimits,
        replay_limits: SourcePortLimits,
    ) -> Result<(BoundOwnerOverlay<N>, SourcePortRuleReplayAudit<N>), OwnerFeedbackError<N>> {
        if !solution.finite_residuals.is_empty() {
            return Err(OwnerFeedbackError::InvalidInput(
                "restored rules-only owner overlay must not retain finite residual terminals"
                    .into(),
            ));
        }
        if solution.requested_cases.is_empty()
            || solution
                .requested_cases
                .iter()
                .any(|case| !case.is_in_sector(&self.sector))
        {
            return Err(OwnerFeedbackError::InvalidInput(
                "restored overlay needs nonempty nominated cases in its owner sector".into(),
            ));
        }
        if solution.finite_case_policy == FiniteCasePolicy::RetainRankFinite
            && solution.max_numerator_rank.is_none()
        {
            return Err(OwnerFeedbackError::InvalidInput(
                "restored finite-retention metadata requires an explicit rank".into(),
            ));
        }
        super::limits::check(
            solution.requested_cases.len(),
            attempt_limits.max_requested_cases,
            "requested domains",
        )?;
        validate_solution_order(&self.sector, &self.owner.ordering, &solution.order)?;
        OwnerOverlayUsage::default().admit_solution(&solution, overlay_limits)?;
        let zeros = self
            .context
            .shared
            .zero_sectors
            .iter()
            .copied()
            .collect::<Vec<_>>()
            .into();
        let audit = SourcePortAudit::try_new_with_root_sector(
            self.context.family(),
            zeros,
            self.owner.root,
        )
        .map_err(OwnerFeedbackError::Replay)?
        .with_limits(replay_limits);
        let report = audit
            .replay_descending_domain_rules(
                self.sector,
                self.order.permutation().copied(),
                &solution,
            )
            .map_err(OwnerFeedbackError::Replay)?;
        let overlay = BoundOwnerOverlay {
            lineage: self.lineage.clone(),
            sector: self.sector,
            root: self.owner.root,
            ordering: self.owner.ordering.clone(),
            policy: self.policy,
            attempt_limits,
            solution,
        };
        Ok((overlay, report))
    }
}
