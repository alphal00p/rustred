//! Cold identity replay for a prospective, source-bound partial search.

use std::sync::Arc;

use super::{BoundOwnerOverlay, BoundOwnerSearch};
use crate::foundry::artifact::{
    SourcePortAudit, SourcePortAuditError, SourcePortLimits, SourcePortRuleReplayAudit,
};
use crate::solver::candidate_reduction::preparation::shared::validate_solution_order;

impl<const N: usize> BoundOwnerSearch<N> {
    /// Independently replay a bound partial result against regenerated ordinary
    /// IBPs using this owner's family, zero evidence, root and mathematical order.
    ///
    /// Foreign lineage or owner metadata is rejected before source regeneration.
    /// The returned report checks identities and their declared guards only. It
    /// neither installs the overlay nor proves descent, requested-case coverage,
    /// recursive RHS closure, or that residuals are acceptable terminal integrals.
    /// In particular, callers must not interpret a successful empty report as a
    /// repaired domain.
    pub fn replay_overlay_rules(
        &self,
        overlay: &BoundOwnerOverlay<N>,
        limits: SourcePortLimits,
    ) -> Result<SourcePortRuleReplayAudit<N>, SourcePortAuditError> {
        if !Arc::ptr_eq(&self.lineage, &overlay.lineage)
            || self.sector != overlay.sector
            || self.owner.root != overlay.root
            || self.owner.ordering != overlay.ordering
        {
            return Err(SourcePortAuditError::message(
                "rule-replay overlay belongs to another program lineage or owner order",
            ));
        }
        validate_solution_order(&self.sector, &self.owner.ordering, &overlay.solution.order)
            .map_err(|issue| SourcePortAuditError::message(issue.to_string()))?;
        let zero_sectors = self
            .context
            .shared
            .zero_sectors
            .iter()
            .copied()
            .collect::<Vec<_>>()
            .into();
        let audit = SourcePortAudit::try_new_with_root_sector(
            self.context.family(),
            zero_sectors,
            self.owner.root,
        )?
        .with_limits(limits);
        audit.replay_domain_rules(
            self.sector,
            self.order.permutation().copied(),
            &overlay.solution,
        )
    }
}
