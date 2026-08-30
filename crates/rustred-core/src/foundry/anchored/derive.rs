use crate::algebra::{
    CoefficientPolynomial, CoefficientPolynomialPart, IndexedCoefficientContext,
    validate_polynomial_on_map,
};
use crate::identity::ParametricRelation;
use crate::sector::OrderingPolicy;

use super::error::AnchoredRuleError;
use super::limits::AnchoredRuleLimits;
use super::model::{AnchoredNonZeroGuard, AnchoredRule, AnchoredRuleTerm, GuardOrigin};
use super::prepare::{
    PreparedProblem, check_cell_limit, check_limit, checked_add, checked_mul, clone_integral_key,
    prepare_problem, try_vec,
};
use super::replay::verify_exact_source_replay;
use super::sparse::{ReducedRuleRow, reduce_rows, reduce_rows_for_target};

/// Derive one exact guarded rule at `anchor`, using physical integral columns
/// in hardest-first order.
///
/// Source rows are inserted into Symbolica's public `SparseRowReducer` in the
/// supplied chronology. A rule is returned only when every right-hand-side
/// integral is proved strictly lower by `ordering` and the reducer row has
/// been replayed exactly as a combination of the original specialized rows.
/// This function does not generalize the anchor or claim sector closure.
pub fn derive_strictly_descending_rule(
    context: &IndexedCoefficientContext,
    relations: &[ParametricRelation],
    anchor: &[i64],
    ordering: OrderingPolicy,
    limits: AnchoredRuleLimits,
) -> Result<AnchoredRule, AnchoredRuleError> {
    let problem = prepare_problem(context, relations, anchor, ordering, limits)?;
    let reduced = reduce_rows(context, &problem, limits)?;
    let replay = verify_exact_source_replay(context, &problem, &reduced, limits)?;
    build_rule(context, problem, reduced, replay, limits)
}

/// Derive the exact RREF rule for one requested physical integral at
/// `anchor`.
///
/// The target must occur among the specialized source-row columns and be a
/// forward pivot. RustRed computes the complete pivot reachability before
/// invoking Symbolica's deterministic serial back-substitution, retains every
/// required pivot guard, and exactly replays the resulting source-row
/// combination. Back-substitution admits physical pivots only and treats the
/// identity-augmentation columns as free right-hand-side coefficients, so
/// dependent source rows cannot obstruct an otherwise valid target rule.
/// This function does not generalize the anchor or claim sector closure.
pub fn derive_strictly_descending_rule_for_target(
    context: &IndexedCoefficientContext,
    relations: &[ParametricRelation],
    anchor: &[i64],
    target_integral: &[i64],
    ordering: OrderingPolicy,
    limits: AnchoredRuleLimits,
) -> Result<AnchoredRule, AnchoredRuleError> {
    if target_integral.len() != context.index_count() {
        return Err(AnchoredRuleError::WrongTargetIntegralArity {
            expected: context.index_count(),
            actual: target_integral.len(),
        });
    }
    let problem = prepare_problem(context, relations, anchor, ordering, limits)?;
    let target_column = problem
        .columns
        .iter()
        .position(|column| column.key.powers() == target_integral)
        .ok_or(AnchoredRuleError::TargetIntegralAbsent)?;
    let reduced = reduce_rows_for_target(context, &problem, target_column, limits)?;
    let replay = verify_exact_source_replay(context, &problem, &reduced, limits)?;
    build_rule(context, problem, reduced, replay, limits)
}

fn build_rule(
    context: &IndexedCoefficientContext,
    problem: PreparedProblem,
    reduced: ReducedRuleRow,
    replay: super::model::ExactReplayWitness,
    limits: AnchoredRuleLimits,
) -> Result<AnchoredRule, AnchoredRuleError> {
    let right_hand_side_terms = reduced.integral_entries.len().checked_sub(1).ok_or(
        AnchoredRuleError::ReducerInvariant {
            detail: "a reduced rule has no physical pivot entry",
        },
    )?;
    let rule_denominator_key_origins = reduced
        .integral_entries
        .iter()
        .filter(|(column, coefficient)| {
            *column != reduced.pivot_column && !coefficient.denominator.is_constant()
        })
        .count();
    // At the assembly peak, prepared column keys and the anchor remain live
    // beside the new pivot, every RHS key, and every RHS key copied into a
    // nonconstant coefficient-denominator guard origin.
    let live_integral_keys = checked_add(
        "live anchored integral-key count",
        checked_add(
            "live anchored integral-key count",
            checked_add("live anchored integral-key count", problem.columns.len(), 2)?,
            right_hand_side_terms,
        )?,
        rule_denominator_key_origins,
    )?;
    check_cell_limit(
        "live anchored integral-key power cells",
        live_integral_keys,
        context.index_count(),
        limits.max_integral_key_power_cells,
    )?;
    let witness_ordering_keys =
        checked_mul("live anchored ordering-key count", right_hand_side_terms, 2)?;
    let live_ordering_keys = checked_add(
        "live anchored ordering-key count",
        problem.columns.len(),
        witness_ordering_keys,
    )?;
    // Prepared columns retain one ComplexityKey apiece; each RHS descent
    // witness retains distinct source and target keys. Every key owns its
    // sector-bit and index-excess coordinate buffers.
    let live_ordering_buffers = checked_mul(
        "live anchored ordering-key coordinate buffers",
        live_ordering_keys,
        2,
    )?;
    check_cell_limit(
        "live anchored ordering-key coordinate cells",
        live_ordering_buffers,
        context.index_count(),
        limits.max_ordering_key_coordinate_cells,
    )?;

    let pivot = clone_integral_key(&problem.columns[reduced.pivot_column].key)?;
    let mut right_hand_side =
        try_vec("anchored rule right-hand-side terms", right_hand_side_terms)?;
    for (column, coefficient) in &reduced.integral_entries {
        if *column == reduced.pivot_column {
            continue;
        }
        let integral = &problem
            .columns
            .get(*column)
            .ok_or(AnchoredRuleError::ReducerInvariant {
                detail: "reduced rule column is outside the ordered integrals",
            })?
            .key;
        let integral = clone_integral_key(integral)?;
        let descent = problem
            .ordering
            .prove_strict_descent(pivot.powers(), integral.powers())?;
        if !descent.verify() {
            return Err(AnchoredRuleError::ReducerInvariant {
                detail: "the ordering owner returned an invalid descent witness",
            });
        }
        let coefficient = context
            .base()
            .try_neg(coefficient, limits.indexed_algebra.exact_algebra)?;
        right_hand_side.push(AnchoredRuleTerm::new(integral, coefficient, descent));
    }
    if right_hand_side.is_empty() {
        return Err(AnchoredRuleError::NoStrictlyDescendingRule);
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
        let pivot_source = pivot_guard.source_ordinal();
        let pivot_row = pivot_guard.row_id().clone();
        let pivot_column = pivot_guard.pivot_column();
        guards.insert(
            pivot_guard.nonzero_polynomial().clone(),
            GuardOrigin::ReducerPivotNumerator {
                source_ordinal: pivot_source,
                row_id: pivot_row.clone(),
                pivot_column,
            },
        )?;
        guards.insert(
            pivot_guard.coefficient().denominator.clone(),
            GuardOrigin::ReducerPivotDenominator {
                source_ordinal: pivot_source,
                row_id: pivot_row,
                pivot_column,
            },
        )?;
    }
    for term in &right_hand_side {
        if !term.coefficient().denominator.is_constant() {
            guards.insert(
                term.coefficient().denominator.clone(),
                GuardOrigin::RuleCoefficientDenominator {
                    integral: clone_integral_key(term.integral())?,
                },
            )?;
        }
    }
    for contribution in &reduced.source_combination {
        guards.insert(
            contribution.coefficient().denominator.clone(),
            GuardOrigin::SourceCombinationDenominator {
                source_ordinal: contribution.source_ordinal(),
                row_id: contribution.row_id().clone(),
            },
        )?;
    }

    Ok(AnchoredRule {
        family_fingerprint: problem.family_fingerprint,
        anchor: problem.anchor,
        ordering: problem.ordering,
        pivot,
        right_hand_side,
        pivot_guards: reduced.pivot_guards,
        nonzero_guards: guards.finish(),
        source_combination: reduced.source_combination,
        replay,
    })
}

struct GuardCollector<'context> {
    context: &'context IndexedCoefficientContext,
    limits: AnchoredRuleLimits,
    guards: Vec<AnchoredNonZeroGuard>,
    origins: usize,
}

impl<'context> GuardCollector<'context> {
    fn new(
        context: &'context IndexedCoefficientContext,
        limits: AnchoredRuleLimits,
    ) -> Result<Self, AnchoredRuleError> {
        Ok(Self {
            context,
            limits,
            guards: try_vec("anchored rule nonzero guards", 0)?,
            origins: 0,
        })
    }

    fn insert(
        &mut self,
        polynomial: CoefficientPolynomial,
        origin: GuardOrigin,
    ) -> Result<(), AnchoredRuleError> {
        validate_polynomial_on_map(
            &polynomial,
            self.context.base().variables(),
            CoefficientPolynomialPart::Numerator,
            self.limits.indexed_algebra.exact_algebra,
        )?;
        if polynomial.is_zero() {
            return Err(AnchoredRuleError::ReducerInvariant {
                detail: "a required anchored nonzero guard is identically zero",
            });
        }
        if polynomial.is_constant() {
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
            self.origins = checked_add("anchored rule guard origins", self.origins, 1)?;
            check_limit(
                "anchored rule guard origins",
                self.origins,
                self.limits.max_guard_origins,
            )?;
            guard.origins.try_reserve_exact(1).map_err(|_| {
                AnchoredRuleError::AllocationFailure {
                    resource: "anchored rule guard origins",
                    requested: guard.origins.len().saturating_add(1),
                }
            })?;
            guard.origins.push(origin);
            return Ok(());
        }
        let requested = checked_add("anchored rule nonzero guards", self.guards.len(), 1)?;
        check_limit(
            "anchored rule nonzero guards",
            requested,
            self.limits.max_rule_guards,
        )?;
        self.origins = checked_add("anchored rule guard origins", self.origins, 1)?;
        check_limit(
            "anchored rule guard origins",
            self.origins,
            self.limits.max_guard_origins,
        )?;
        self.guards
            .try_reserve_exact(1)
            .map_err(|_| AnchoredRuleError::AllocationFailure {
                resource: "anchored rule nonzero guards",
                requested,
            })?;
        let mut origins = try_vec("anchored rule guard origins", 1)?;
        origins.push(origin);
        self.guards.push(AnchoredNonZeroGuard {
            polynomial,
            origins,
        });
        Ok(())
    }

    fn finish(self) -> Vec<AnchoredNonZeroGuard> {
        self.guards
    }
}
