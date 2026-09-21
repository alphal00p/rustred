//! The single exact one-step evaluator shared by candidate callers.
use super::model::{CandidateReductionError, PreparedRule};
use crate::algebra::{Coefficient, IndexedCoefficientContext, IndexedPolynomial};
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
    pub ordering: OrderingPolicy,
    pub rules: &'a [PreparedRule<N>],
    pub source_conditions: &'a [IndexedPolynomial],
    pub zero_sectors: &'a BTreeSet<[bool; N]>,
    pub limits: ReductionLimits,
}
impl<const N: usize> CandidateEvaluator<'_, N> {
    pub(crate) fn validate_target(
        &self,
        target: &IntegralKey,
    ) -> Result<(), CandidateReductionError> {
        if target.powers().len() != N {
            return Err(ReductionError::WrongArity {
                expected: N,
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
            if self
                .context
                .specialize_polynomial_sealed(
                    condition,
                    target.powers(),
                    self.limits.indexed_algebra,
                )?
                .is_zero()
            {
                return Err(CandidateReductionError::SourceConditionVanished {
                    target: target.clone(),
                    ordinal,
                });
            }
        }
        Ok(())
    }

    pub(super) fn is_zero(&self, target: &IntegralKey) -> bool {
        self.zero_sectors
            .contains(&std::array::from_fn(|i| target.powers()[i] > 0))
    }

    pub(super) fn apply(
        &self,
        target: &IntegralKey,
        request: &mut ReductionRequest,
        statistics: &mut ReductionStatistics,
    ) -> Result<BTreeMap<IntegralKey, Coefficient>, CandidateReductionError> {
        'rules: for rule in self.rules {
            if rule
                .fixed
                .iter()
                .zip(target.powers())
                .any(|(fixed, &n)| fixed.is_some_and(|v| i64::from(v) != n))
            {
                continue;
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
                    continue 'rules;
                }
            }
            // One branch is one AND-conjunction; a rule is excluded if ANY
            // entire branch vanishes. Never split its factors into exclusions.
            for branch in &rule.exceptions {
                let mut all_zero = true;
                for condition in branch {
                    if !self
                        .context
                        .specialize_polynomial_sealed(
                            condition,
                            target.powers(),
                            self.limits.indexed_algebra,
                        )?
                        .is_zero()
                    {
                        all_zero = false;
                        break;
                    }
                }
                if all_zero {
                    continue 'rules;
                }
            }
            // Test every original denominator before coefficient cancellation
            // or RHS coalescing. An undefined formula is not a zero identity.
            for term in &rule.rhs {
                if self
                    .context
                    .specialize_polynomial_sealed(
                        &term.denominator,
                        target.powers(),
                        self.limits.indexed_algebra,
                    )?
                    .is_zero()
                {
                    continue 'rules;
                }
            }
            let mut result = BTreeMap::new();
            for term in &rule.rhs {
                let (coefficient, _) = self.context.specialize_sealed(
                    &term.coefficient,
                    target.powers(),
                    self.limits.indexed_algebra,
                )?;
                if coefficient.is_zero() {
                    continue;
                }
                let mut child = [0_i64; N];
                for (axis, ((out, &n), &shift)) in child
                    .iter_mut()
                    .zip(target.powers())
                    .zip(&term.shift)
                    .enumerate()
                {
                    *out = n.checked_add(shift).ok_or_else(|| {
                        CandidateReductionError::IndexOverflow {
                            target: target.clone(),
                            axis,
                            rule: rule.ordinal,
                        }
                    })?;
                }
                let child = IntegralKey::try_new(child).map_err(ReductionError::IntegralKey)?;
                self.validate_target(&child)?;
                if self.is_zero(&child) {
                    continue;
                }
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
            return Ok(result);
        }
        Err(CandidateReductionError::Uncovered {
            target: target.clone(),
        })
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
