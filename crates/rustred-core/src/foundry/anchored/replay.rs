use crate::algebra::{Coefficient, IndexedCoefficientContext};

use super::error::AnchoredRuleError;
use super::limits::AnchoredRuleLimits;
use super::model::ExactReplayWitness;
use super::prepare::{PreparedProblem, check_limit, checked_add, try_vec};
use super::sparse::ReducedRuleRow;

pub(super) fn verify_exact_source_replay(
    context: &IndexedCoefficientContext,
    problem: &PreparedProblem,
    reduced: &ReducedRuleRow,
    limits: AnchoredRuleLimits,
) -> Result<ExactReplayWitness, AnchoredRuleError> {
    let mut budget = ReplayBudget {
        used: 0,
        limit: limits.max_replay_exact_operations,
    };
    let mut replayed: Vec<Option<Coefficient>> =
        try_vec("exact replay integral accumulators", problem.columns.len())?;
    replayed.resize_with(problem.columns.len(), || None);

    for contribution in &reduced.source_combination {
        let source = problem.sources.get(contribution.source_ordinal()).ok_or(
            AnchoredRuleError::ReducerInvariant {
                detail: "source replay ordinal is outside its chronology",
            },
        )?;
        if source.row_id != *contribution.row_id() {
            return Err(AnchoredRuleError::ReducerInvariant {
                detail: "source replay row identity differs from its chronology",
            });
        }
        for (column, source_coefficient) in &source.entries {
            budget.charge("anchored replay exact operations")?;
            let product = context.base().try_mul(
                contribution.coefficient(),
                source_coefficient,
                limits.indexed_algebra.exact_algebra,
            )?;
            let slot =
                replayed
                    .get_mut(*column as usize)
                    .ok_or(AnchoredRuleError::ReducerInvariant {
                        detail: "source replay integral column is outside the physical matrix",
                    })?;
            if let Some(accumulator) = slot {
                budget.charge("anchored replay exact operations")?;
                *accumulator = context.base().try_add(
                    accumulator,
                    &product,
                    limits.indexed_algebra.exact_algebra,
                )?;
            } else {
                *slot = Some(product);
            }
        }
    }

    let mut reduced_position = 0usize;
    for (column, replayed) in replayed.into_iter().enumerate() {
        let reduced_coefficient = if reduced_position < reduced.integral_entries.len()
            && reduced.integral_entries[reduced_position].0 == column
        {
            let value = &reduced.integral_entries[reduced_position].1;
            reduced_position += 1;
            value.clone()
        } else {
            context.base().zero()
        };
        let replayed = replayed.unwrap_or_else(|| context.base().zero());
        budget.charge("anchored replay exact operations")?;
        let residual = context.base().try_sub(
            &replayed,
            &reduced_coefficient,
            limits.indexed_algebra.exact_algebra,
        )?;
        if !residual.is_zero() {
            return Err(AnchoredRuleError::ReplayMismatch {
                integral_column: column,
            });
        }
    }
    if reduced_position != reduced.integral_entries.len() {
        return Err(AnchoredRuleError::ReducerInvariant {
            detail: "reduced row contains a physical column outside replay",
        });
    }

    Ok(ExactReplayWitness::new(
        reduced.source_combination.len(),
        problem.columns.len(),
        budget.used,
    ))
}

struct ReplayBudget {
    used: usize,
    limit: usize,
}

impl ReplayBudget {
    fn charge(&mut self, resource: &'static str) -> Result<(), AnchoredRuleError> {
        self.used = checked_add(resource, self.used, 1)?;
        check_limit(resource, self.used, self.limit)
    }
}
