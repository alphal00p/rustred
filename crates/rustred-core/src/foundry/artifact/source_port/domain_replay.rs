//! Rule-only cold validation for explicitly partial owner overlays.

use crate::solver::SectorDomainSolution;

use super::{
    SourcePortAudit, SourcePortAuditError, SourcePortRuleReplayAudit, error, geometry,
    replay_ordering,
};

impl<const N: usize> SourcePortAudit<N> {
    /// Identity, guards and strict descent, but no case or recursive cover.
    /// Kept crate-private: the consuming owner restore must also bind lineage,
    /// reject residual terminals and check its resource limits.
    pub(crate) fn replay_descending_domain_rules(
        &self,
        sector: [bool; N],
        permutation: Option<[usize; N]>,
        solution: &SectorDomainSolution<N>,
    ) -> Result<SourcePortRuleReplayAudit<N>, SourcePortAuditError> {
        let started = std::time::Instant::now();
        let mut report = self.replay_domain_rules(sector, permutation, solution)?;
        let ordering = replay_ordering(sector, permutation, &solution.order)?;
        for (ordinal, rule) in solution.rules.iter().enumerate() {
            let partition = geometry::application_partition(
                rule,
                self.sources.index_variables(),
                &sector,
                &[],
            )?;
            if partition.boxes.is_empty() {
                return Err(error(format!(
                    "partial rule {ordinal} has an empty declared application domain"
                )));
            }
            geometry::prove_descent(
                rule,
                &partition.boxes,
                &partition.affine_exclusions,
                &sector,
                ordering.clone(),
                self.sources.index_variables(),
            )
            .map_err(|issue| {
                issue.with_message_context(|| format!("partial rule {ordinal} strict descent"))
            })?;
        }
        report.elapsed = started.elapsed();
        Ok(report)
    }
}
