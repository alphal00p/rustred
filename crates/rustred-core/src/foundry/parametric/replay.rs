use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext};

use super::error::ParametricRuleError;
use super::limits::ParametricRuleLimits;
use super::model::ParametricExactReplayWitness;
use super::prepare::{PreparedProblem, check_limit, checked_add, try_vec};
use super::sparse::ReducedRuleRow;

pub(super) fn verify_exact_source_replay(
    context: &IndexedCoefficientContext,
    problem: &PreparedProblem,
    reduced: &ReducedRuleRow,
    limits: ParametricRuleLimits,
) -> Result<ParametricExactReplayWitness, ParametricRuleError> {
    let mut budget = ReplayBudget {
        used: 0,
        limit: limits.max_replay_exact_operations,
    };
    let mut replayed: Vec<Option<IndexedCoefficient>> =
        try_vec("exact indexed replay accumulators", problem.columns.len())?;
    replayed.resize_with(problem.columns.len(), || None);

    for contribution in &reduced.source_combination {
        let source = problem.sources.get(contribution.source_ordinal()).ok_or(
            ParametricRuleError::ReducerInvariant {
                detail: "indexed source replay ordinal is outside its chronology",
            },
        )?;
        if source.row_id != *contribution.row_id() {
            return Err(ParametricRuleError::ReducerInvariant {
                detail: "indexed source replay row identity differs from its chronology",
            });
        }
        for (column, source_coefficient) in &source.entries {
            budget.charge("parametric replay exact operations")?;
            let contribution = context.bind_sealed(contribution.coefficient())?;
            let source_coefficient = context.bind_sealed(source_coefficient)?;
            let product = context.mul_bound_with_limits(
                contribution,
                source_coefficient,
                limits.indexed_algebra.exact_algebra,
            )?;
            let slot = replayed.get_mut(*column as usize).ok_or(
                ParametricRuleError::ReducerInvariant {
                    detail: "indexed source replay column is outside the physical matrix",
                },
            )?;
            if let Some(accumulator) = slot {
                budget.charge("parametric replay exact operations")?;
                let accumulator = context.bind_sealed(accumulator)?;
                let product = context.bind_sealed(&product)?;
                *slot = Some(context.add_bound_with_limits(
                    accumulator,
                    product,
                    limits.indexed_algebra.exact_algebra,
                )?);
            } else {
                *slot = Some(product);
            }
        }
    }

    let mut reduced_position = 0usize;
    for (column, replayed) in replayed.into_iter().enumerate() {
        let reduced_coefficient = if reduced_position < reduced.shift_entries.len()
            && reduced.shift_entries[reduced_position].0 == column
        {
            let value = reduced.shift_entries[reduced_position].1.clone();
            reduced_position += 1;
            value
        } else {
            context.zero()
        };
        let replayed = replayed.unwrap_or_else(|| context.zero());
        budget.charge("parametric replay exact operations")?;
        let replayed = context.bind_sealed(&replayed)?;
        let reduced_coefficient = context.bind_sealed(&reduced_coefficient)?;
        let residual = context.sub_bound_with_limits(
            replayed,
            reduced_coefficient,
            limits.indexed_algebra.exact_algebra,
        )?;
        if !residual.is_zero() {
            return Err(ParametricRuleError::ReplayMismatch {
                shift_column: column,
            });
        }
    }
    if reduced_position != reduced.shift_entries.len() {
        return Err(ParametricRuleError::ReducerInvariant {
            detail: "reduced indexed row contains a shift column outside replay",
        });
    }

    Ok(ParametricExactReplayWitness::new(
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
    fn charge(&mut self, resource: &'static str) -> Result<(), ParametricRuleError> {
        self.used = checked_add(resource, self.used, 1)?;
        check_limit(resource, self.used, self.limit)
    }
}
