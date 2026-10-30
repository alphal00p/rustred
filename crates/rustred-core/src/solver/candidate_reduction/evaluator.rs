//! The single exact one-step evaluator shared by candidate callers.
use super::model::{CandidateReductionError, PreparedRule, PreparedTerm};
use crate::algebra::{
    Coefficient, CoefficientPolynomial, IndexedCoefficientContext, IndexedPolynomial,
};
use crate::family::IntegralKey;
use crate::reduction::{
    ReductionError, ReductionLimits, ReductionRequest, ReductionStatistics,
    accumulate_master_in_request,
};
use crate::sector::OrderingPolicy;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct CandidateEvaluator<'a, const N: usize> {
    pub context: &'a IndexedCoefficientContext,
    pub root_sector: [bool; N],
    pub ordering: &'a OrderingPolicy,
    pub rules: &'a [PreparedRule<N>],
    pub whole_piece_alternatives: &'a [usize],
    pub source_conditions: &'a [IndexedPolynomial],
    pub zero_sectors: &'a BTreeSet<[bool; N]>,
    pub limits: ReductionLimits,
}
impl<'a, const N: usize> CandidateEvaluator<'a, N> {
    pub(crate) fn validate_target(
        &self,
        target: &IntegralKey,
    ) -> Result<(), CandidateReductionError> {
        self.validate_target_with_conditions(target, &mut |_| {})
    }

    pub(super) fn validate_target_with_conditions(
        &self,
        target: &IntegralKey,
        nonzero: &mut impl FnMut(&CoefficientPolynomial),
    ) -> Result<(), CandidateReductionError> {
        if target.powers().len() != self.context.index_count() {
            return Err(ReductionError::WrongArity {
                expected: self.context.index_count(),
                actual: target.powers().len(),
            }
            .into());
        }
        if target
            .powers()
            .iter()
            .zip(self.root_sector)
            .any(|(&n, active)| n > 0 && !active)
        {
            return Err(CandidateReductionError::OutsideRoot {
                target: target.clone(),
            });
        }
        for (ordinal, condition) in self.source_conditions.iter().enumerate() {
            let condition = self.context.specialize_polynomial_sealed(
                condition,
                target.powers(),
                self.limits.indexed_algebra,
            )?;
            if condition.is_zero() {
                return Err(CandidateReductionError::SourceConditionVanished {
                    target: target.clone(),
                    ordinal,
                });
            }
            nonzero(&condition);
        }
        Ok(())
    }

    pub(super) fn is_zero(&self, target: &IntegralKey) -> bool {
        self.zero_sectors.contains(&std::array::from_fn(|i| {
            target.powers().get(i).copied().unwrap_or(0) > 0
        }))
    }

    pub(super) fn apply(
        &self,
        target: &IntegralKey,
        request: &mut ReductionRequest,
        statistics: &mut ReductionStatistics,
    ) -> Result<BTreeMap<IntegralKey, Coefficient>, CandidateReductionError> {
        let (_, rule) = self.selected_rule(target)?;
        self.apply_selected(rule, target, request, statistics, &mut |_| {})
    }

    /// The ordinary one-step kernel, additionally returning its actual saved
    /// selection identities. Selection runs once; no equation is rematerialized
    /// and no applicability or guard check is replaced by this bookkeeping.
    pub(super) fn apply_with_rule_identity(
        &self,
        target: &IntegralKey,
        request: &mut ReductionRequest,
        statistics: &mut ReductionStatistics,
    ) -> Result<(BTreeMap<IntegralKey, Coefficient>, usize, usize), CandidateReductionError> {
        let (baseline, selected) = self.selected_rule(target)?;
        let terms = self.apply_selected(selected, target, request, statistics, &mut |_| {})?;
        Ok((terms, baseline.ordinal, selected.ordinal))
    }

    /// The same ordered dispatch and strict descent as `apply`, retaining the
    /// generic conditions needed for exact coefficient back-substitution.
    pub(super) fn apply_with_conditions(
        &self,
        target: &IntegralKey,
        request: &mut ReductionRequest,
        statistics: &mut ReductionStatistics,
    ) -> Result<(BTreeMap<IntegralKey, Coefficient>, Vec<Coefficient>), CandidateReductionError>
    {
        self.apply_with_selection(target, None, request, statistics)
    }

    /// Apply an already selected native rule after a completed-result miss.
    /// Selection and its exact applicability checks are shared with ordinary
    /// application; no coefficient specialization occurs on a cache hit.
    pub(super) fn apply_selected_with_conditions(
        &self,
        target: &IntegralKey,
        baseline: &PreparedRule<N>,
        rule: &PreparedRule<N>,
        request: &mut ReductionRequest,
        statistics: &mut ReductionStatistics,
    ) -> Result<(BTreeMap<IntegralKey, Coefficient>, Vec<Coefficient>), CandidateReductionError>
    {
        self.apply_with_selection(target, Some((baseline, rule)), request, statistics)
    }

    fn apply_with_selection(
        &self,
        target: &IntegralKey,
        selected: Option<(&PreparedRule<N>, &PreparedRule<N>)>,
        request: &mut ReductionRequest,
        statistics: &mut ReductionStatistics,
    ) -> Result<(BTreeMap<IntegralKey, Coefficient>, Vec<Coefficient>), CandidateReductionError>
    {
        let mut conditions = Vec::new();
        let mut nonzero = |condition: &CoefficientPolynomial| {
            if !condition.is_constant() {
                let condition: Coefficient = condition.clone().into();
                if !conditions.contains(&condition) {
                    conditions.push(condition);
                }
            }
        };
        self.validate_target_with_conditions(target, &mut nonzero)?;
        // Post-baseline alternatives require both the baseline coverage and
        // their own applicability. Rejected candidates contribute no guards.
        let rule = match selected {
            Some((baseline, rule)) => {
                self.applicable_with_conditions(baseline, target, &mut nonzero)?;
                if !std::ptr::eq(baseline, rule) {
                    self.applicable_with_conditions(rule, target, &mut nonzero)?;
                }
                rule
            }
            None => self.selected_rule_with_conditions(target, &mut nonzero)?.1,
        };
        let terms = self.apply_selected(rule, target, request, statistics, &mut nonzero)?;
        Ok((terms, conditions))
    }

    pub(super) fn selected_rule(
        &self,
        target: &IntegralKey,
    ) -> Result<(&'a PreparedRule<N>, &'a PreparedRule<N>), CandidateReductionError> {
        self.dispatch(target, |rule| self.applicable(rule, target))
    }

    /// The same dispatch as `selected_rule`, reporting the guards of the
    /// selected baseline and then of the selected alternative, exactly as
    /// rechecking both with `applicable_with_conditions` would. Rejected
    /// candidates contribute no guards, so each candidate's guards are held
    /// back until its applicability is known.
    fn selected_rule_with_conditions(
        &self,
        target: &IntegralKey,
        nonzero: &mut impl FnMut(&CoefficientPolynomial),
    ) -> Result<(&'a PreparedRule<N>, &'a PreparedRule<N>), CandidateReductionError> {
        let mut candidate_guards = Vec::new();
        self.dispatch(target, |rule| {
            candidate_guards.clear();
            let applicable = self.applicable_with_conditions(rule, target, &mut |condition| {
                candidate_guards.push(condition.clone())
            })?;
            if applicable {
                candidate_guards.iter().for_each(&mut *nonzero);
            }
            Ok(applicable)
        })
    }

    /// The first applicable partition rule, optionally replaced by the first
    /// applicable whole-piece alternative.
    fn dispatch(
        &self,
        target: &IntegralKey,
        mut applicable: impl FnMut(&'a PreparedRule<N>) -> Result<bool, CandidateReductionError>,
    ) -> Result<(&'a PreparedRule<N>, &'a PreparedRule<N>), CandidateReductionError> {
        let mut baseline = None;
        for rule in self.rules {
            if rule.dispatch_policy == crate::solver::RuleDispatchPolicy::Partition
                && applicable(rule)?
            {
                baseline = Some(rule);
                break;
            }
        }
        let Some(baseline) = baseline else {
            return Err(CandidateReductionError::Uncovered {
                target: target.clone(),
            });
        };
        // A singleton is a whole baseline piece. Optional alternatives cannot
        // create coverage where the ordinary batch had no applicable rule.
        for &index in self.whole_piece_alternatives {
            let alternative = &self.rules[index];
            if applicable(alternative)? {
                return Ok((baseline, alternative));
            }
        }
        Ok((baseline, baseline))
    }

    fn applicable(
        &self,
        rule: &PreparedRule<N>,
        target: &IntegralKey,
    ) -> Result<bool, CandidateReductionError> {
        self.applicable_with_conditions(rule, target, &mut |_| {})
    }

    fn applicable_with_conditions(
        &self,
        rule: &PreparedRule<N>,
        target: &IntegralKey,
        nonzero: &mut impl FnMut(&CoefficientPolynomial),
    ) -> Result<bool, CandidateReductionError> {
        if rule
            .fixed
            .iter()
            .zip(target.powers())
            .any(|(fixed, &n)| fixed.is_some_and(|v| i64::from(v) != n))
        {
            return Ok(false);
        }
        for equality in &rule.equalities {
            if !self
                .context
                .specialize_polynomial_sealed(
                    equality,
                    target.powers(),
                    self.limits.indexed_algebra,
                )?
                .is_zero()
            {
                return Ok(false);
            }
        }
        // One branch is one AND-conjunction; a rule is excluded if ANY
        // entire branch vanishes. Never split its factors into exclusions.
        for branch in &rule.exceptions {
            let mut all_zero = true;
            for condition in branch {
                let condition = self.context.specialize_polynomial_sealed(
                    condition,
                    target.powers(),
                    self.limits.indexed_algebra,
                )?;
                if !condition.is_zero() {
                    all_zero = false;
                    // One nonzero witness suffices to avoid this AND-zero
                    // branch. Retaining every factor as nonzero would split
                    // the original conjunction into stronger exclusions.
                    nonzero(&condition);
                    break;
                }
            }
            if all_zero {
                return Ok(false);
            }
        }
        // Test every original denominator before coefficient cancellation
        // or RHS coalescing. An undefined formula is not a zero identity.
        for term in &rule.rhs {
            let denominator = self.context.specialize_polynomial_sealed(
                &term.denominator,
                target.powers(),
                self.limits.indexed_algebra,
            )?;
            if denominator.is_zero() {
                return Ok(false);
            }
            nonzero(&denominator);
        }
        Ok(true)
    }

    fn apply_selected(
        &self,
        rule: &PreparedRule<N>,
        target: &IntegralKey,
        request: &mut ReductionRequest,
        statistics: &mut ReductionStatistics,
        nonzero: &mut impl FnMut(&CoefficientPolynomial),
    ) -> Result<BTreeMap<IntegralKey, Coefficient>, CandidateReductionError> {
        let mut result = BTreeMap::new();
        for term in &rule.rhs {
            let Some((child, coefficient)) = self.specialize_term(rule, term, target, nonzero)?
            else {
                continue;
            };
            if self
                .ordering
                .compare(child.powers(), target.powers())
                .map_err(ReductionError::Ordering)?
                != Ordering::Less
            {
                return Err(CandidateReductionError::NonDescending {
                    target: target.clone(),
                    child,
                    rule: rule.ordinal,
                });
            }
            accumulate_master_in_request(
                self.context.base(),
                &mut result,
                &child,
                coefficient,
                self.limits,
                request,
                statistics,
            )?;
        }
        Ok(result)
    }

    /// Exact arithmetic beneath dispatch, terminal stopping, and descent.
    /// This operation does not replay a candidate's source provenance.
    pub(super) fn applicable_identity(
        &self,
        rule: &PreparedRule<N>,
        target: &IntegralKey,
        request: &mut ReductionRequest,
        statistics: &mut ReductionStatistics,
    ) -> Result<
        Option<(BTreeMap<IntegralKey, Coefficient>, Vec<Coefficient>)>,
        CandidateReductionError,
    > {
        let mut conditions = Vec::new();
        let mut nonzero = |condition: &CoefficientPolynomial| {
            if !condition.is_constant() {
                let condition: Coefficient = condition.clone().into();
                if !conditions.contains(&condition) {
                    conditions.push(condition);
                }
            }
        };
        self.validate_target_with_conditions(target, &mut nonzero)?;
        if !self.applicable_with_conditions(rule, target, &mut nonzero)? {
            return Ok(None);
        }
        request.record_rule_application(self.limits.diagnostic_limit(self.limits.max_rule_applications))?;
        let mut terms = BTreeMap::from([(target.clone(), self.context.base().one())]);
        for term in &rule.rhs {
            let Some((child, coefficient)) =
                self.specialize_term(rule, term, target, &mut nonzero)?
            else {
                continue;
            };
            let coefficient = self
                .context
                .base()
                .try_neg(&coefficient, self.limits.exact_algebra)?;
            accumulate_master_in_request(
                self.context.base(),
                &mut terms,
                &child,
                coefficient,
                self.limits,
                request,
                statistics,
            )?;
        }
        Ok(Some((terms, conditions)))
    }

    fn specialize_term(
        &self,
        rule: &PreparedRule<N>,
        term: &PreparedTerm<N>,
        target: &IntegralKey,
        nonzero: &mut impl FnMut(&CoefficientPolynomial),
    ) -> Result<Option<(IntegralKey, Coefficient)>, CandidateReductionError> {
        let (coefficient, denominator) = self.context.specialize_sealed(
            &term.coefficient,
            target.powers(),
            self.limits.indexed_algebra,
        )?;
        if let Some(denominator) = denominator {
            nonzero(&denominator);
        }
        if coefficient.is_zero() {
            return Ok(None);
        }
        let mut child = [0_i64; N];
        for (axis, ((out, &n), &shift)) in child
            .iter_mut()
            .zip(target.powers())
            .zip(&term.shift)
            .enumerate()
        {
            *out = n
                .checked_add(shift)
                .ok_or_else(|| CandidateReductionError::IndexOverflow {
                    target: target.clone(),
                    axis,
                    rule: rule.ordinal,
                })?;
        }
        let child = IntegralKey::try_new(child[..self.context.index_count()].iter().copied())
            .map_err(ReductionError::IntegralKey)?;
        self.validate_target_with_conditions(&child, nonzero)?;
        if self.is_zero(&child) {
            return Ok(None);
        }
        Ok(Some((child, coefficient)))
    }
}
pub(super) fn validate_entry_rank(
    target: &IntegralKey,
    max_numerator_rank: Option<u32>,
) -> Result<(), CandidateReductionError> {
    if let Some(limit) = max_numerator_rank {
        let rank = target
            .powers()
            .iter()
            .filter(|&&n| n < 0)
            .try_fold(0_u128, |sum, &n| {
                sum.checked_add(u128::from(n.unsigned_abs()))
            })
            .ok_or_else(|| {
                CandidateReductionError::InvalidInput(
                    "candidate entry numerator rank overflow".into(),
                )
            })?;
        if rank > u128::from(limit) {
            return Err(CandidateReductionError::OutsideNumeratorRank {
                target: target.clone(),
                rank,
                limit,
            });
        }
    }
    Ok(())
}
