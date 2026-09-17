//! Exact original-source replay for a finite batch of candidate rules.
//!
//! This deliberately stops before whole-sector cover, descent, terminal, or
//! artifact checks.  It is the identity half of a bounded finite-target audit:
//! callers must separately prove that concrete reduction reaches only rules
//! listed here and ends in their explicitly declared finite terminal set.

use std::time::{Duration, Instant};

use crate::sector::{CoordinatePriority, CoordinatePriorityLimits, Mask, OrderingPolicy};
use crate::solver::{SectorConfig, SectorSolution, SectorSolver};

use super::{SourcePortAudit, SourcePortAuditError, error, geometry, replay};

/// One candidate-rule ordinal whose exact identity and complete declared
/// application domain replay against the regenerated original ordinary IBPs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourcePortReplayedRule {
    pub ordinal: usize,
    pub original_source_entries: usize,
}

/// Identity-only replay result for one finite batch of candidate rules.
///
/// This is not a sector-cover or family-closure certificate.  In particular,
/// it makes no statement about omitted integer points, rule descent, or the
/// independence/minimality of finite terminals.
#[derive(Debug)]
pub struct SourcePortRuleReplayAudit<const N: usize> {
    pub sector: [bool; N],
    pub ordering: OrderingPolicy,
    pub rules: Vec<SourcePortReplayedRule>,
    pub elapsed: Duration,
}

impl<const N: usize> SourcePortAudit<N> {
    /// Replay a finite batch of candidate identities without running the
    /// whole-sector predicate-cover proof.
    ///
    /// Every returned ordinal has been regenerated from its selected
    /// preconditioned trace, expressed as exact weights of the original
    /// ordinary IBPs, and multiplied back to the candidate equation in exact
    /// generic coefficient algebra.  RHS poles, activation boundaries, and
    /// source-weight poles are recomputed.  If they require any exceptional
    /// branch absent from the candidate's declared domain, the entire batch is
    /// rejected rather than silently narrowing the rule.
    pub fn replay_sector_rules(
        &self,
        sector: [bool; N],
        permutation: Option<[usize; N]>,
        solution: &SectorSolution<N>,
    ) -> Result<SourcePortRuleReplayAudit<N>, SourcePortAuditError> {
        self.replay_sector_rule_iter(
            sector,
            permutation,
            solution,
            0..solution.rules.len(),
            solution.rules.len(),
        )
    }

    /// Replay only a strictly increasing set of sector-local rule ordinals.
    /// This is the bounded-target integration seam: the concrete reachability
    /// walk may select the rules it actually used without paying for unrelated
    /// formulas in the same sector.
    pub fn replay_sector_rule_batch(
        &self,
        sector: [bool; N],
        permutation: Option<[usize; N]>,
        solution: &SectorSolution<N>,
        ordinals: &[usize],
    ) -> Result<SourcePortRuleReplayAudit<N>, SourcePortAuditError> {
        if ordinals.windows(2).any(|pair| pair[0] >= pair[1])
            || ordinals
                .last()
                .is_some_and(|&ordinal| ordinal >= solution.rules.len())
        {
            return Err(error(
                "rule-replay ordinals must be strictly increasing and in range",
            ));
        }
        self.replay_sector_rule_iter(
            sector,
            permutation,
            solution,
            ordinals.iter().copied(),
            ordinals.len(),
        )
    }

    fn replay_sector_rule_iter(
        &self,
        sector: [bool; N],
        permutation: Option<[usize; N]>,
        solution: &SectorSolution<N>,
        ordinals: impl IntoIterator<Item = usize>,
        ordinal_count: usize,
    ) -> Result<SourcePortRuleReplayAudit<N>, SourcePortAuditError> {
        self.limits.validate()?;
        let started = Instant::now();
        if !Mask::try_new(sector)
            .map_err(error)?
            .is_subsector_of(&self.root_sector)
            .map_err(error)?
        {
            return Err(error(
                "rule-replay sector is outside the declared root-sector scope",
            ));
        }
        if self.zero_sectors.contains(&sector) {
            return Err(error("a rule-replay sector was also declared zero"));
        }

        let ordering = match permutation {
            None => OrderingPolicy::SpiredUncutV1,
            Some(slots) => {
                crate::solver::IntegralOrder::new(sector, [false; N])
                    .with_permutation(slots)
                    .map_err(error)?;
                let mut ranks = [0; N];
                for (rank, slot) in slots.into_iter().enumerate() {
                    ranks[slot] = rank;
                }
                let priority =
                    CoordinatePriority::try_new(N, &ranks, CoordinatePriorityLimits::default())
                        .map_err(error)?;
                OrderingPolicy::try_spired_with_coordinate_priority(&priority).map_err(error)?
            }
        };
        let config = SectorConfig {
            permutation,
            zero_sectors: self.zero_sectors.clone(),
            ..SectorConfig::default()
        };
        let (solver, preconditioner) =
            SectorSolver::new_with_provenance(&self.sources, sector, config).map_err(error)?;
        let mut order = crate::solver::IntegralOrder::new(sector, [false; N]);
        if let Some(slots) = permutation {
            order = order.with_permutation(slots).map_err(error)?;
        }

        let mut rules = Vec::new();
        rules
            .try_reserve_exact(ordinal_count)
            .map_err(|_| error("rule-replay report allocation failed"))?;
        for ordinal in ordinals {
            let rule = &solution.rules[ordinal];
            let stored =
                geometry::application_partition(rule, self.sources.index_variables(), &sector, &[])
                    .map_err(|issue| {
                        issue.with_message_context(|| format!("rule {ordinal} declared domain"))
                    })?;
            if stored.boxes.is_empty() {
                return Err(error(format!(
                    "rule {ordinal} has an empty declared application domain"
                )));
            }
            let checked = replay::replay_rule(
                &self.sources,
                &self.original_row_ids,
                &self.original_sources,
                solver.basis(),
                &order,
                &self.zero_sectors,
                rule,
                &stored.boxes,
                Some(&preconditioner),
            )
            .map_err(|issue| {
                issue.with_message_context(|| format!("rule {ordinal} original-source replay"))
            })?;
            if !checked.additional_exceptions.is_empty() {
                return Err(error(format!(
                    "rule {ordinal} omits {} exceptional guard branches required by original-source replay",
                    checked.additional_exceptions.len()
                )));
            }
            rules.push(SourcePortReplayedRule {
                ordinal,
                original_source_entries: checked.ordinary.contributions.len(),
            });
        }
        Ok(SourcePortRuleReplayAudit {
            sector,
            ordering,
            rules,
            elapsed: started.elapsed(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::solved_tadpole;

    #[test]
    fn finite_rule_batch_replays_exact_identity_and_rejects_mutated_rhs() {
        let (audit, solution) = solved_tadpole();
        let report = audit.replay_sector_rules([true], None, &solution).unwrap();
        assert_eq!(report.sector, [true]);
        assert_eq!(report.rules.len(), 1);
        assert_eq!(report.rules[0].ordinal, 0);
        assert!(report.rules[0].original_source_entries > 0);

        let selected = audit
            .replay_sector_rule_batch([true], None, &solution, &[0])
            .unwrap();
        assert_eq!(selected.rules, report.rules);
        assert!(
            audit
                .replay_sector_rule_batch([true], None, &solution, &[0, 0])
                .is_err()
        );

        let (audit, mut solution) = solved_tadpole();
        solution.rules[0].candidate.rhs[0].coefficient =
            -solution.rules[0].candidate.rhs[0].coefficient.clone();
        let error = audit
            .replay_sector_rules([true], None, &solution)
            .unwrap_err();
        assert!(
            error.to_string().contains("rule 0 original-source replay"),
            "{error}"
        );
    }

    #[test]
    fn finite_rule_batch_rejects_an_omitted_replay_guard() {
        let (audit, mut solution) = solved_tadpole();
        solution.rules[0].exceptions = Default::default();
        let error = audit
            .replay_sector_rules([true], None, &solution)
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("omits 1 exceptional guard branches"),
            "{error}"
        );
    }
}
