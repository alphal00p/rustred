//! Exact original-source replay for a finite batch of candidate rules.
//!
//! This deliberately stops before whole-sector cover, descent, terminal, or
//! artifact checks.  It is the identity half of a bounded finite-target audit:
//! callers must separately prove that concrete reduction reaches only rules
//! listed here and ends in their explicitly declared finite terminal set.

use std::time::{Duration, Instant};

use crate::sector::{Mask, OrderingPolicy};
use crate::solver::{
    IntegralOrder, SectorConfig, SectorDomainSolution, SectorRule, SectorSolution, SectorSolver,
};

use super::{SourcePortAudit, SourcePortAuditError, error, geometry, replay};

mod circuit;
pub use circuit::{ReplayCircuitLimits, ReplayedSourceCircuit, ReplayedSourceCircuitBatch};

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
            &solution.order,
            &solution.rules,
            0..solution.rules.len(),
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
        validate_ordinals(ordinals, solution.rules.len())?;
        self.replay_sector_rule_iter(
            sector,
            permutation,
            &solution.order,
            &solution.rules,
            ordinals.iter().copied(),
        )
    }

    /// Replay every rule in a genuine partial, nominated-domain search result.
    ///
    /// This performs exactly the same original-source identity and guard
    /// checks as [`Self::replay_sector_rules`], without converting a partial
    /// result into a completed sector. It neither certifies coverage of the
    /// requested cases nor accepts finite residuals as terminal integrals.
    /// Descent and recursive successor coverage remain separate obligations.
    pub fn replay_domain_rules(
        &self,
        sector: [bool; N],
        permutation: Option<[usize; N]>,
        solution: &SectorDomainSolution<N>,
    ) -> Result<SourcePortRuleReplayAudit<N>, SourcePortAuditError> {
        self.replay_sector_rule_iter(
            sector,
            permutation,
            &solution.order,
            &solution.rules,
            0..solution.rules.len(),
        )
    }

    /// Replay a strictly increasing subset of rules from a partial search.
    ///
    /// Ordinals are local to `solution.rules`. Omitted rules, requested-case
    /// coverage and finite residuals acquire no authority from this report.
    pub fn replay_domain_rule_batch(
        &self,
        sector: [bool; N],
        permutation: Option<[usize; N]>,
        solution: &SectorDomainSolution<N>,
        ordinals: &[usize],
    ) -> Result<SourcePortRuleReplayAudit<N>, SourcePortAuditError> {
        validate_ordinals(ordinals, solution.rules.len())?;
        self.replay_sector_rule_iter(
            sector,
            permutation,
            &solution.order,
            &solution.rules,
            ordinals.iter().copied(),
        )
    }

    fn replay_sector_rule_iter(
        &self,
        sector: [bool; N],
        permutation: Option<[usize; N]>,
        order: &IntegralOrder<N>,
        source_rules: &[SectorRule<N>],
        ordinals: impl ExactSizeIterator<Item = usize>,
    ) -> Result<SourcePortRuleReplayAudit<N>, SourcePortAuditError> {
        let (ordering, rules, elapsed) = self.with_replayed_sector_rule_iter(
            sector,
            permutation,
            order,
            source_rules,
            ordinals,
            false,
            |ordinal, _, _, checked| {
                Ok(SourcePortReplayedRule {
                    ordinal,
                    original_source_entries: checked.ordinary.contributions.len(),
                })
            },
        )?;
        Ok(SourcePortRuleReplayAudit {
            sector,
            ordering,
            rules,
            elapsed,
        })
    }

    fn with_replayed_sector_rule_iter<T>(
        &self,
        sector: [bool; N],
        permutation: Option<[usize; N]>,
        order: &IntegralOrder<N>,
        source_rules: &[SectorRule<N>],
        ordinals: impl ExactSizeIterator<Item = usize>,
        capture_pivot: bool,
        mut retain: impl FnMut(
            usize,
            &SectorRule<N>,
            geometry::ApplicationPartition,
            replay::Replay<N>,
        ) -> Result<T, SourcePortAuditError>,
    ) -> Result<(OrderingPolicy, Vec<T>, Duration), SourcePortAuditError> {
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

        let ordering = super::replay_ordering(sector, permutation, order)?;
        let config = SectorConfig {
            permutation,
            integral_order: order.program().cloned(),
            zero_sectors: self.zero_sectors.clone(),
            ..SectorConfig::default()
        };
        let (solver, preconditioner) =
            SectorSolver::new_with_provenance(&self.sources, sector, config).map_err(error)?;
        let mut rules = Vec::new();
        rules
            .try_reserve_exact(ordinals.len())
            .map_err(|_| error("rule-replay report allocation failed"))?;
        for ordinal in ordinals {
            let rule = &source_rules[ordinal];
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
            if capture_pivot {
                circuit::require_coordinate(rule, &stored, &sector)?;
            }
            let checked = replay::replay_rule_retaining(
                &self.sources,
                &self.original_row_ids,
                &self.original_sources,
                solver.basis(),
                order,
                &self.zero_sectors,
                rule,
                &stored.boxes,
                Some(&preconditioner),
                capture_pivot,
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
            rules.push(retain(ordinal, rule, stored, checked)?);
        }
        Ok((ordering, rules, started.elapsed()))
    }
}

fn validate_ordinals(ordinals: &[usize], rule_count: usize) -> Result<(), SourcePortAuditError> {
    if ordinals.windows(2).any(|pair| pair[0] >= pair[1])
        || ordinals
            .last()
            .is_some_and(|&ordinal| ordinal >= rule_count)
    {
        return Err(error(
            "rule-replay ordinals must be strictly increasing and in range",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::super::tests::{solved_tadpole, tadpole};
    use super::*;
    use crate::solver::{Case, CoordinateCase, SectorSolveOptions, SourceSystem};

    fn partial_tadpole(case: Case<1>, depth: u32) -> (SourcePortAudit<1>, SectorDomainSolution<1>) {
        let family = tadpole();
        let zeros: Arc<[[bool; 1]]> = Arc::from([[false]]);
        let source = SourceSystem::from_family(&family).unwrap();
        let solver = SectorSolver::new(
            &source,
            [true],
            SectorConfig {
                zero_sectors: zeros.clone(),
                ..Default::default()
            },
        )
        .unwrap();
        let solution = solver
            .solve_domains(
                vec![case],
                SectorSolveOptions {
                    numerical_depth: depth,
                    ..Default::default()
                },
            )
            .unwrap();
        (SourcePortAudit::try_new(&family, zeros).unwrap(), solution)
    }

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

    #[test]
    fn partial_domain_rules_share_exact_replay_without_whole_sector_conversion() {
        let (audit, solution) = partial_tadpole(Case::generic(), 0);
        assert_eq!(solution.requested_cases, [Case::generic()]);
        // A residual is deliberately present: identity-only replay must not
        // depend on accepting it as a master or proving requested-case cover.
        assert_eq!(solution.finite_residuals.len(), 1);
        let report = audit.replay_domain_rules([true], None, &solution).unwrap();
        assert_eq!(report.rules.len(), 1);
        assert!(report.rules[0].original_source_entries > 0);
        let batch = audit
            .replay_domain_rule_batch([true], None, &solution, &[0])
            .unwrap();
        assert_eq!(batch.rules, report.rules);
        let empty = audit
            .replay_domain_rule_batch([true], None, &solution, &[])
            .unwrap();
        assert!(empty.rules.is_empty());
        for invalid in [vec![0, 0], vec![1], vec![1, 0]] {
            assert!(
                audit
                    .replay_domain_rule_batch([true], None, &solution, &invalid)
                    .unwrap_err()
                    .to_string()
                    .contains("ordinals must be strictly increasing and in range")
            );
        }
    }

    #[test]
    fn partial_fixed_case_replays_a_shared_numerical_source_trace() {
        let case = CoordinateCase::new([Some(2)]).unwrap().into();
        let (audit, solution) = partial_tadpole(case, 1);
        assert_eq!(solution.stats.symbolic_cases, 0);
        assert_eq!(solution.stats.numerical_cases, 1);
        assert!(!solution.rules.is_empty());
        let report = audit.replay_domain_rules([true], None, &solution).unwrap();
        assert_eq!(report.rules.len(), solution.rules.len());
        assert!(
            report
                .rules
                .iter()
                .all(|rule| rule.original_source_entries > 0)
        );
    }

    #[test]
    fn partial_domain_replay_rejects_mutated_rhs_and_missing_guards() {
        let (audit, mut solution) = partial_tadpole(Case::generic(), 0);
        solution.rules[0].candidate.rhs[0].coefficient =
            -solution.rules[0].candidate.rhs[0].coefficient.clone();
        let issue = audit
            .replay_domain_rules([true], None, &solution)
            .unwrap_err();
        assert!(issue.to_string().contains("rule 0 original-source replay"));

        let (audit, mut solution) = partial_tadpole(Case::generic(), 0);
        solution.rules[0].exceptions = Default::default();
        let issue = audit
            .replay_domain_rule_batch([true], None, &solution, &[0])
            .unwrap_err();
        assert!(
            issue
                .to_string()
                .contains("omits 1 exceptional guard branches")
        );
    }

    #[test]
    fn partial_domain_replay_keeps_sector_zero_and_order_validation() {
        let (audit, mut solution) = partial_tadpole(Case::generic(), 0);
        let issue = audit
            .replay_domain_rules([false], None, &solution)
            .unwrap_err();
        assert!(issue.to_string().contains("declared zero"));
        solution.order = IntegralOrder::new([false], [false]);
        let issue = audit
            .replay_domain_rules([true], None, &solution)
            .unwrap_err();
        assert!(
            issue
                .to_string()
                .contains("differs from the solved mathematical order")
        );
    }
}
