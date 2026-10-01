use std::cmp::Ordering;

use super::{Admitted, model::*};
use crate::solver::{Power, SolverError};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Features([u64; 12]);

impl Features {
    pub(super) fn read<const N: usize>(
        candidate: &Admitted<N>,
        sector: &[bool; N],
    ) -> Result<Self, SolverError> {
        let mut values = Self::default();
        let rule = &candidate.rule;
        for term in &rule.candidate.rhs {
            let mut numerator = 0u64;
            let mut positive = 0u64;
            for (axis, &active) in sector.iter().enumerate() {
                let target = rule.candidate.target[axis];
                let rhs = term.integral[axis];
                let excursion = excursion(target, rhs, active)?;
                if active {
                    add(&mut positive, excursion)?;
                } else {
                    add(&mut numerator, excursion)?;
                }
            }
            values.0[RuleQualityFeature::MaxNumeratorShiftExcursion as usize] = values
                .get(RuleQualityFeature::MaxNumeratorShiftExcursion)
                .max(numerator);
            add(
                &mut values.0[RuleQualityFeature::TotalNumeratorShiftExcursion as usize],
                numerator,
            )?;
            values.0[RuleQualityFeature::MaxPositiveShiftExcursion as usize] = values
                .get(RuleQualityFeature::MaxPositiveShiftExcursion)
                .max(positive);
            add(
                &mut values.0[RuleQualityFeature::TotalPositiveShiftExcursion as usize],
                positive,
            )?;
            add(
                &mut values.0[RuleQualityFeature::CoefficientMonomials as usize],
                count(term.coefficient.numerator.nterms())?,
            )?;
            add(
                &mut values.0[RuleQualityFeature::CoefficientMonomials as usize],
                count(term.coefficient.denominator.nterms())?,
            )?;
        }
        values.0[RuleQualityFeature::ExceptionalCases as usize] = count(candidate.children.len())?;
        values.0[RuleQualityFeature::AffineExceptionalCases as usize] = count(
            candidate
                .children
                .iter()
                .filter(|c| c.affine().is_some())
                .count(),
        )?;
        values.0[RuleQualityFeature::GuardBranches as usize] =
            count(rule.exceptions.branches.len())?;
        for branch in &rule.exceptions.branches {
            add(
                &mut values.0[RuleQualityFeature::GuardPredicates as usize],
                count(branch.len())?,
            )?;
        }
        values.0[RuleQualityFeature::RhsTerms as usize] = count(rule.candidate.rhs.len())?;
        values.0[RuleQualityFeature::SourceRows as usize] = count(rule.candidate.sources.len())?;
        values.0[RuleQualityFeature::SearchRows as usize] = count(rule.candidate.stats.rows)?;
        Ok(values)
    }

    pub(super) fn get(&self, feature: RuleQualityFeature) -> u64 {
        self.0[feature as usize]
    }

    pub(super) fn triggered(&self, trigger: &RulePortfolioTrigger) -> bool {
        match trigger {
            RulePortfolioTrigger::Always => true,
            RulePortfolioTrigger::AnyAtLeast(thresholds) => {
                thresholds.iter().any(|t| self.get(t.feature) >= t.minimum)
            }
        }
    }

    pub(super) fn compare(&self, other: &Self, priorities: &[RuleQualityPriority]) -> Ordering {
        for priority in priorities {
            let order = self.get(priority.feature).cmp(&other.get(priority.feature));
            let order = if priority.descending {
                order.reverse()
            } else {
                order
            };
            if order != Ordering::Equal {
                return order;
            }
        }
        Ordering::Equal
    }
}

// Offset proxies, not affine rank optimization or route prediction. Numeric
// coordinates use exact differences of the corresponding one-axis degrees.
pub(super) fn excursion(target: Power, rhs: Power, active: bool) -> Result<u64, SolverError> {
    if target.is_symbolic() != rhs.is_symbolic() {
        return Err(invalid(
            "rule quality received mismatched symbolic target/RHS coordinates",
        ));
    }
    let target = i64::from(target.value());
    let rhs_value = i64::from(rhs.value());
    let amount = if rhs.is_symbolic() {
        if active {
            rhs_value - target
        } else {
            target - rhs_value
        }
    } else if active {
        rhs_value.max(0) - target.max(0)
    } else {
        (-rhs_value).max(0) - (-target).max(0)
    };
    Ok(amount.max(0) as u64)
}

fn count(value: usize) -> Result<u64, SolverError> {
    u64::try_from(value).map_err(|_| invalid("rule quality count overflow"))
}

fn add(target: &mut u64, value: u64) -> Result<(), SolverError> {
    *target = target
        .checked_add(value)
        .ok_or_else(|| invalid("rule quality count overflow"))?;
    Ok(())
}
