use crate::algebra::{IndexedCoefficientContext, IndexedPolynomial};
use crate::identity::ParametricRelation;
use crate::sector::OrderingPolicy;

use super::anchor::{AnchorSelection, verify_anchor_agreement};
use super::boundary::{build_sector_monotone_admission, preflight_sector_monotone_rhs_shift};
use super::error::ParametricRuleError;
use super::limits::ParametricRuleLimits;
use super::model::{
    ParametricGuardOrigin, ParametricNonZeroGuard, ParametricRule, ParametricRuleTerm,
};
use super::prepare::{
    PreparedProblem, check_cell_limit, check_limit, checked_add, checked_mul, prepare_problem,
    prepare_sector_monotone_problem, try_vec,
};
use super::replay::verify_exact_source_replay;
use super::sparse::{ReducedRuleRow, reduce_rows, reduce_rows_for_target};

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
    build_rule(
        context,
        relations,
        problem,
        reduced,
        replay,
        limits,
        AnchorSelection::FirstDescending,
        RuleAdmission::Interior,
    )
}

/// Derive the exact RREF rule for one requested free-index shift over a
/// representable fixed-sector interior.
///
/// The target must occur among the generated physical columns and be a
/// forward pivot. RustRed computes the complete pivot reachability before
/// invoking Symbolica's deterministic serial back-substitution, retains every
/// required physical pivot guard, replays the exact indexed source
/// combination, proves uniform descent, and compares against an independently
/// targeted anchored derivation. Back-substitution admits physical pivots only
/// and treats the identity-augmentation columns as free right-hand-side
/// coefficients, so dependent source rows cannot obstruct an otherwise valid
/// target rule. This function does not claim exceptional-domain coverage or
/// closure.
pub fn derive_sector_interior_rule_for_target(
    context: &IndexedCoefficientContext,
    relations: &[ParametricRelation],
    anchor: &[i64],
    target_shift: &[i64],
    ordering: OrderingPolicy,
    limits: ParametricRuleLimits,
) -> Result<ParametricRule, ParametricRuleError> {
    if target_shift.len() != context.index_count() {
        return Err(ParametricRuleError::WrongTargetShiftArity {
            expected: context.index_count(),
            actual: target_shift.len(),
        });
    }
    let problem = prepare_problem(context, relations, anchor, ordering, limits)?;
    let target_column = problem
        .columns
        .iter()
        .position(|column| column.shift.values() == target_shift)
        .ok_or(ParametricRuleError::TargetShiftAbsent)?;
    let reduced = reduce_rows_for_target(context, &problem, target_column, limits)?;
    let replay = verify_exact_source_replay(context, &problem, &reduced, limits)?;
    build_rule(
        context,
        relations,
        problem,
        reduced,
        replay,
        limits,
        AnchorSelection::Targeted,
        RuleAdmission::Interior,
    )
}

/// Derive one target-directed recurrence on a sector-monotone parent box.
///
/// The exact `K(n)` row retains its ordinary fixed-sector interior proof and
/// additionally receives a maximal representable parent-sector box. Every RHS
/// term carries an exhaustive term-local partition: its same-sector cell uses
/// the ordinary strict shift ordering, while deterministic first-pinched
/// cylinders descend by propagator count. Positive inactive-line shifts are a
/// typed refinement boundary. The anchor is checked inside this larger box
/// and still agrees with an independent anchored derivation. Proper-subsector
/// cells are unresolved dependencies; this function certifies neither their
/// rule availability nor closure.
pub fn derive_sector_monotone_rule_for_target(
    context: &IndexedCoefficientContext,
    relations: &[ParametricRelation],
    anchor: &[i64],
    target_shift: &[i64],
    ordering: OrderingPolicy,
    limits: ParametricRuleLimits,
) -> Result<ParametricRule, ParametricRuleError> {
    if target_shift.len() != context.index_count() {
        return Err(ParametricRuleError::WrongTargetShiftArity {
            expected: context.index_count(),
            actual: target_shift.len(),
        });
    }
    let problem = prepare_sector_monotone_problem(context, relations, anchor, ordering, limits)?;
    let target_column = problem
        .columns
        .iter()
        .position(|column| column.shift.values() == target_shift)
        .ok_or(ParametricRuleError::TargetShiftAbsent)?;
    let reduced = reduce_rows_for_target(context, &problem, target_column, limits)?;
    let replay = verify_exact_source_replay(context, &problem, &reduced, limits)?;
    build_rule(
        context,
        relations,
        problem,
        reduced,
        replay,
        limits,
        AnchorSelection::Targeted,
        RuleAdmission::SectorMonotone,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuleAdmission {
    Interior,
    SectorMonotone,
}

fn build_rule(
    context: &IndexedCoefficientContext,
    relations: &[ParametricRelation],
    problem: PreparedProblem,
    reduced: ReducedRuleRow,
    replay: super::model::ParametricExactReplayWitness,
    limits: ParametricRuleLimits,
    anchor_selection: AnchorSelection,
    admission: RuleAdmission,
) -> Result<ParametricRule, ParametricRuleError> {
    let right_hand_side_terms = reduced.shift_entries.len().checked_sub(1).ok_or(
        ParametricRuleError::ReducerInvariant {
            detail: "a reduced indexed rule has no physical pivot entry",
        },
    )?;
    let ordering_keys_per_term = if admission == RuleAdmission::SectorMonotone {
        // The ordinary interior proof and the term-local same-sector proof
        // each retain source/target keys. Always-pinched terms retain one key
        // pair directly instead of a same-sector witness.
        4
    } else {
        2
    };
    let witness_ordering_keys = checked_mul(
        "live parametric ordering keys",
        right_hand_side_terms,
        ordering_keys_per_term,
    )?;
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
    if admission == RuleAdmission::SectorMonotone {
        // One prepared fixed-sector interior, one maximal parent-sector box,
        // and at most one retained same-sector subdomain per RHS term.
        let domain_count = checked_add(
            "live sector-monotone domain containers",
            right_hand_side_terms,
            2,
        )?;
        let endpoints_per_domain = checked_mul(
            "live sector-monotone domain bound endpoint cells",
            context.index_count(),
            2,
        )?;
        let live_domain_endpoints = checked_mul(
            "live sector-monotone domain bound endpoint cells",
            domain_count,
            endpoints_per_domain,
        )?;
        check_limit(
            "live sector-monotone domain bound endpoint cells",
            live_domain_endpoints,
            limits.max_domain_bound_endpoint_cells,
        )?;
    }

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
    if admission == RuleAdmission::SectorMonotone {
        let mut right_hand_side_ordinal = 0usize;
        for (column, _) in &reduced.shift_entries {
            if *column == reduced.pivot_column {
                continue;
            }
            let shift = &problem
                .columns
                .get(*column)
                .ok_or(ParametricRuleError::ReducerInvariant {
                    detail: "reduced rule column is outside the ordered shifts",
                })?
                .shift;
            preflight_sector_monotone_rhs_shift(
                problem.domain.sector(),
                right_hand_side_ordinal,
                shift,
            )?;
            right_hand_side_ordinal = checked_add(
                "sector-monotone RHS term ordinal",
                right_hand_side_ordinal,
                1,
            )?;
        }
        if right_hand_side_ordinal != right_hand_side_terms {
            return Err(ParametricRuleError::ReducerInvariant {
                detail: "sector-monotone RHS preflight count differs from reduced row",
            });
        }
    }
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

    let sector_monotone_admission = if admission == RuleAdmission::SectorMonotone {
        let admission = build_sector_monotone_admission(
            problem.domain.sector(),
            &pivot,
            &right_hand_side,
            problem.ordering,
            limits,
        )?;
        if !admission.domain().contains(problem.anchor.powers())? {
            return Err(ParametricRuleError::PointOutsideSectorMonotoneDomain);
        }
        Some(admission)
    } else {
        None
    };

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
        anchor_selection,
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
        sector_monotone_admission,
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
