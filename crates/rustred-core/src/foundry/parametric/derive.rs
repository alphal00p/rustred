use crate::algebra::{IndexedCoefficientContext, IndexedPolynomial};
use crate::identity::ParametricRelation;
use crate::sector::OrderingPolicy;

use super::anchor::verify_anchor_agreement;
use super::error::ParametricRuleError;
use super::limits::ParametricRuleLimits;
use super::model::{
    ParametricGuardOrigin, ParametricNonZeroGuard, ParametricRule, ParametricRuleTerm,
};
use super::prepare::{
    PreparedProblem, check_cell_limit, check_limit, checked_add, checked_mul, prepare_problem,
    try_vec,
};
use super::replay::verify_exact_source_replay;
use super::sparse::{ReducedRuleRow, reduce_rows};

/// Derive one exact guarded rule over a representable fixed-sector interior.
///
/// Rows are eliminated directly over the authenticated indexed field `K(n)`
/// using Symbolica's public sparse reducer. The result is returned only after
/// exact indexed source replay, uniform structural descent proofs, complete
/// pivot/denominator guards, and an independent concrete anchored derivation
/// agree. This function does not claim exceptional-domain coverage or closure.
pub fn derive_sector_interior_rule(
    context: &IndexedCoefficientContext,
    relations: &[ParametricRelation],
    anchor: &[i64],
    ordering: OrderingPolicy,
    limits: ParametricRuleLimits,
) -> Result<ParametricRule, ParametricRuleError> {
    let problem = prepare_problem(context, relations, anchor, ordering, limits)?;
    let reduced = reduce_rows(context, &problem, limits)?;
    let replay = verify_exact_source_replay(context, &problem, &reduced, limits)?;
    build_rule(context, relations, problem, reduced, replay, limits)
}

fn build_rule(
    context: &IndexedCoefficientContext,
    relations: &[ParametricRelation],
    problem: PreparedProblem,
    reduced: ReducedRuleRow,
    replay: super::model::ParametricExactReplayWitness,
    limits: ParametricRuleLimits,
) -> Result<ParametricRule, ParametricRuleError> {
    let right_hand_side_terms = reduced.shift_entries.len().checked_sub(1).ok_or(
        ParametricRuleError::ReducerInvariant {
            detail: "a reduced indexed rule has no physical pivot entry",
        },
    )?;
    let witness_ordering_keys =
        checked_mul("live parametric ordering keys", right_hand_side_terms, 2)?;
    let live_ordering_keys = checked_add(
        "live parametric ordering keys",
        problem.columns.len(),
        witness_ordering_keys,
    )?;
    check_cell_limit(
        "live parametric ordering-key coordinate cells",
        live_ordering_keys,
        context.index_count(),
        limits.max_ordering_key_coordinate_cells,
    )?;

    // Every shift cloned into the pivot, RHS, pivot guards, and subsequent
    // guard origins is an Arc-backed handle to a value-canonical column
    // buffer. Their handle counts are bounded respectively by shift columns,
    // RHS columns, elimination pivots, and guard origins; none allocates new
    // i64 coordinate cells.
    let pivot = problem
        .columns
        .get(reduced.pivot_column)
        .ok_or(ParametricRuleError::ReducerInvariant {
            detail: "reduced indexed pivot is outside the ordered shifts",
        })?
        .shift
        .clone();
    let mut right_hand_side = try_vec(
        "parametric rule right-hand-side terms",
        right_hand_side_terms,
    )?;
    for (column, coefficient) in &reduced.shift_entries {
        if *column == reduced.pivot_column {
            continue;
        }
        let shift = problem
            .columns
            .get(*column)
            .ok_or(ParametricRuleError::ReducerInvariant {
                detail: "reduced rule column is outside the ordered shifts",
            })?
            .shift
            .clone();
        let descent = problem.ordering.prove_shift_strict_descent(
            &problem.domain,
            pivot.values(),
            shift.values(),
        )?;
        if !descent.verify() {
            return Err(ParametricRuleError::ReducerInvariant {
                detail: "the ordering owner returned an invalid parametric descent witness",
            });
        }
        let coefficient = context.bind_sealed(coefficient)?;
        let coefficient =
            context.neg_bound_with_limits(coefficient, limits.indexed_algebra.exact_algebra)?;
        right_hand_side.push(ParametricRuleTerm::new(shift, coefficient, descent));
    }
    if right_hand_side.is_empty() {
        return Err(ParametricRuleError::NoStrictlyDescendingRule);
    }

    let mut guards = GuardCollector::new(context, limits)?;
    for (source_ordinal, source) in problem.sources.into_iter().enumerate() {
        let participates = reduced
            .source_combination
            .iter()
            .any(|contribution| contribution.source_ordinal() == source_ordinal)
            || reduced
                .pivot_guards
                .iter()
                .any(|pivot| pivot.source_ordinal() == source_ordinal);
        if participates {
            for guard in source.guards {
                guards.insert(guard.polynomial, guard.origin)?;
            }
        }
    }

    for pivot_guard in &reduced.pivot_guards {
        let source_ordinal = pivot_guard.source_ordinal();
        let row_id = pivot_guard.row_id().clone();
        let pivot_column = pivot_guard.pivot_column();
        let pivot_shift = pivot_guard.pivot_shift().clone();
        guards.insert(
            pivot_guard.nonzero_polynomial().clone(),
            ParametricGuardOrigin::ReducerPivotNumerator {
                source_ordinal,
                row_id: row_id.clone(),
                pivot_column,
                pivot_shift: pivot_shift.clone(),
            },
        )?;
        let pivot_coefficient = context.bind_sealed(pivot_guard.coefficient())?;
        guards.insert(
            context.denominator_condition_from_bound(pivot_coefficient)?,
            ParametricGuardOrigin::ReducerPivotDenominator {
                source_ordinal,
                row_id,
                pivot_column,
                pivot_shift,
            },
        )?;
    }
    for term in &right_hand_side {
        let coefficient = context.bind_sealed(term.coefficient())?;
        guards.insert(
            context.denominator_condition_from_bound(coefficient)?,
            ParametricGuardOrigin::RuleCoefficientDenominator {
                shift: term.shift().clone(),
            },
        )?;
    }
    for contribution in &reduced.source_combination {
        let coefficient = context.bind_sealed(contribution.coefficient())?;
        guards.insert(
            context.denominator_condition_from_bound(coefficient)?,
            ParametricGuardOrigin::SourceCombinationDenominator {
                source_ordinal: contribution.source_ordinal(),
                row_id: contribution.row_id().clone(),
            },
        )?;
    }
    let nonzero_guards = guards.finish();

    let anchor_agreement = verify_anchor_agreement(
        context,
        relations,
        problem.anchor.powers(),
        problem.ordering,
        &pivot,
        &right_hand_side,
        &nonzero_guards,
        &reduced.source_combination,
        limits,
    )?;

    Ok(ParametricRule {
        family_fingerprint: problem.family_fingerprint,
        context_fingerprint: problem.context_fingerprint,
        domain: problem.domain,
        ordering: problem.ordering,
        pivot,
        right_hand_side,
        pivot_guards: reduced.pivot_guards,
        nonzero_guards,
        source_combination: reduced.source_combination,
        replay,
        anchor_agreement,
    })
}

struct GuardCollector<'context> {
    context: &'context IndexedCoefficientContext,
    limits: ParametricRuleLimits,
    guards: Vec<ParametricNonZeroGuard>,
    origins: usize,
}

impl<'context> GuardCollector<'context> {
    fn new(
        context: &'context IndexedCoefficientContext,
        limits: ParametricRuleLimits,
    ) -> Result<Self, ParametricRuleError> {
        Ok(Self {
            context,
            limits,
            guards: try_vec("parametric rule nonzero guards", 0)?,
            origins: 0,
        })
    }

    fn insert(
        &mut self,
        polynomial: IndexedPolynomial,
        origin: ParametricGuardOrigin,
    ) -> Result<(), ParametricRuleError> {
        self.context.validate_polynomial_context(&polynomial)?;
        if polynomial.is_zero() {
            return Err(ParametricRuleError::ReducerInvariant {
                detail: "a required parametric nonzero guard is identically zero",
            });
        }
        if polynomial.is_nonzero_constant() {
            return Ok(());
        }
        if let Some(guard) = self
            .guards
            .iter_mut()
            .find(|guard| guard.polynomial == polynomial)
        {
            if guard.origins.contains(&origin) {
                return Ok(());
            }
            self.origins = checked_add("parametric rule guard origins", self.origins, 1)?;
            check_limit(
                "parametric rule guard origins",
                self.origins,
                self.limits.max_guard_origins,
            )?;
            guard.origins.try_reserve_exact(1).map_err(|_| {
                ParametricRuleError::AllocationFailure {
                    resource: "parametric rule guard origins",
                    requested: guard.origins.len().saturating_add(1),
                }
            })?;
            guard.origins.push(origin);
            return Ok(());
        }
        let requested = checked_add("parametric rule nonzero guards", self.guards.len(), 1)?;
        check_limit(
            "parametric rule nonzero guards",
            requested,
            self.limits.max_rule_guards,
        )?;
        self.origins = checked_add("parametric rule guard origins", self.origins, 1)?;
        check_limit(
            "parametric rule guard origins",
            self.origins,
            self.limits.max_guard_origins,
        )?;
        self.guards
            .try_reserve_exact(1)
            .map_err(|_| ParametricRuleError::AllocationFailure {
                resource: "parametric rule nonzero guards",
                requested,
            })?;
        let mut origins = try_vec("parametric rule guard origins", 1)?;
        origins.push(origin);
        self.guards.push(ParametricNonZeroGuard {
            polynomial,
            origins,
        });
        Ok(())
    }

    fn finish(self) -> Vec<ParametricNonZeroGuard> {
        self.guards
    }
}
