use crate::algebra::{CoefficientContext, IndexedCoefficientContext};
use crate::family::{AffineDenominator, IntegralFamily};
use crate::identity::{IdentityConditionSource, ParametricIbpGenerator, ParametricRelation};
use crate::sector::OrderingPolicy;

use super::super::derive::derive_strictly_descending_rule;
use super::super::error::AnchoredRuleError;
use super::super::limits::AnchoredRuleLimits;
use super::super::model::GuardOrigin;
use super::super::prepare::{PreparedProblem, prepare_problem};

#[test]
fn aggregate_index_cell_limits_have_exact_two_index_boundaries() {
    let (_, context, relations) = two_index_guarded_sources();
    let anchor = [1, 1];
    let defaults = AnchoredRuleLimits::default();
    let problem = prepare_problem(
        &context,
        &relations,
        &anchor,
        OrderingPolicy::default(),
        defaults,
    )
    .unwrap();
    let rule = derive_strictly_descending_rule(
        &context,
        &relations,
        &anchor,
        OrderingPolicy::default(),
        defaults,
    )
    .unwrap();

    let arity = context.index_count();
    assert_eq!(arity, 2);
    let physical_nonzeros = problem
        .sources
        .iter()
        .map(|source| source.entries.len())
        .sum::<usize>();
    let denominator_origins = rule
        .right_hand_side()
        .iter()
        .filter(|term| !term.coefficient().denominator.is_constant())
        .count();
    let preparation_integral_keys = physical_nonzeros * 2;
    let rule_integral_keys =
        problem.columns.len() + 2 + rule.right_hand_side().len() + denominator_origins;
    let integral_cells = preparation_integral_keys.max(rule_integral_keys) * arity;
    let ordering_cells = (problem.columns.len() + 2 * rule.right_hand_side().len()) * 2 * arity;
    let guard_cells = prepared_guard_index_cells(&problem);
    assert_eq!(
        (
            physical_nonzeros,
            problem.columns.len(),
            rule.right_hand_side().len(),
            denominator_origins,
            integral_cells,
            ordering_cells,
            guard_cells,
        ),
        (3, 3, 1, 1, 14, 20, 10),
    );

    let exact = AnchoredRuleLimits {
        max_integral_key_power_cells: integral_cells,
        max_guard_provenance_index_cells: guard_cells,
        max_ordering_key_coordinate_cells: ordering_cells,
        ..defaults
    };
    derive_strictly_descending_rule(
        &context,
        &relations,
        &anchor,
        OrderingPolicy::default(),
        exact,
    )
    .unwrap();

    for (limits, resource, requested) in [
        (
            AnchoredRuleLimits {
                max_integral_key_power_cells: integral_cells - 1,
                ..exact
            },
            "live anchored integral-key power cells",
            integral_cells,
        ),
        (
            AnchoredRuleLimits {
                max_guard_provenance_index_cells: guard_cells - 1,
                ..exact
            },
            "anchored guard provenance index cells",
            guard_cells,
        ),
        (
            AnchoredRuleLimits {
                max_ordering_key_coordinate_cells: ordering_cells - 1,
                ..exact
            },
            "live anchored ordering-key coordinate cells",
            ordering_cells,
        ),
    ] {
        assert_eq!(
            derive_strictly_descending_rule(
                &context,
                &relations,
                &anchor,
                OrderingPolicy::default(),
                limits,
            ),
            Err(AnchoredRuleError::ResourceLimit {
                resource,
                requested,
                limit: requested - 1,
            })
        );
    }
}

fn two_index_guarded_sources() -> (
    CoefficientContext,
    IndexedCoefficientContext,
    Vec<ParametricRelation>,
) {
    let base = CoefficientContext::new(["d", "x"]);
    let x = base.parameter("x").unwrap();
    let shifted_power = base.try_div(&base.one(), &x, Default::default()).unwrap();
    let family = IntegralFamily::new(
        "foundry-two-index-census",
        vec!["k".into()],
        vec!["p".into()],
        base.clone(),
        base.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(base.zero(), vec![base.one(), base.zero()]),
            AffineDenominator::new(base.zero(), vec![base.zero(), base.one()]),
        ],
        vec![vec![base.one()]],
        vec![shifted_power, base.zero()],
    )
    .unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let batch = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..batch.len())
        .map(|ordinal| batch.generate(ordinal))
        .collect();
    let relations = batch.complete(rows).unwrap().into_relations();
    (base, generator.context().clone(), relations)
}

fn prepared_guard_index_cells(problem: &PreparedProblem) -> usize {
    problem
        .sources
        .iter()
        .flat_map(|source| &source.guards)
        .map(|guard| match &guard.origin {
            GuardOrigin::SourceCondition {
                condition_sources, ..
            } => condition_sources.iter().map(condition_source_cells).sum(),
            GuardOrigin::SourceCoefficientDenominator { shift, .. } => shift.len(),
            _ => 0,
        })
        .sum()
}

fn condition_source_cells(source: &IdentityConditionSource) -> usize {
    match source {
        IdentityConditionSource::RelationInputTermDenominator { shift, .. }
        | IdentityConditionSource::RelationCollectedTermDenominator { shift, .. } => shift.len(),
        IdentityConditionSource::RelationTranslation { offset, .. }
        | IdentityConditionSource::IndexTranslation { offset } => offset.len(),
        _ => 0,
    }
}
