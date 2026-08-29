use std::sync::Arc;

use symbolica::domains::{Set, rational::Q};
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use crate::algebra::{CoefficientContext, IndexedCoefficientContext};
use crate::family::{AffineDenominator, IntegralFamily, IntegralKey};
use crate::identity::{ParametricIbpGenerator, ParametricRelation, RowId};
use crate::sector::OrderingPolicy;

use super::derive::derive_strictly_descending_rule;
use super::error::AnchoredRuleError;
use super::limits::AnchoredRuleLimits;
use super::model::GuardOrigin;
use super::prepare::{OrderedIntegral, PreparedProblem, PreparedSourceRow, prepare_problem};
use super::replay::verify_exact_source_replay;
use super::sparse::reduce_rows;

mod resource_limits;

#[test]
fn symbolica_sparse_reducer_chronology_and_pivots_are_pinned() {
    type Rational = <Q as Set>::Element;
    let half = Q.to_element(1.into(), 2.into(), true);
    let mut reducer = SparseRowReducer::new(4, Q, LuLMode::Full);

    let first: Vec<Rational> = vec![2.into(), 4.into(), 1.into()];
    assert_eq!(reducer.add_row(&first, &[0, 1, 2]), Some(0));
    let second: Vec<Rational> = vec![4.into(), 10.into(), 1.into()];
    assert_eq!(reducer.add_row(&second, &[0, 1, 3]), Some(1));

    assert_eq!(reducer.pivots(), &vec![Some(0), Some(1), None, None]);
    let (_, first_columns, first_values) = reducer.u().row_iter().next().unwrap();
    assert_eq!(first_columns, &[0, 1, 2]);
    assert_eq!(first_values, &[1.into(), 2.into(), half.clone()]);
    let (_, second_columns, second_values) = reducer.u().row_iter().nth(1).unwrap();
    assert_eq!(second_columns, &[1, 2, 3]);
    assert_eq!(second_values, &[1.into(), (-1).into(), half]);

    let (_, lower_columns, lower_values) = reducer.l().last_row().unwrap();
    assert_eq!(lower_columns, &[0, 1]);
    let expected_lower: Vec<Rational> = vec![4.into(), 2.into()];
    assert_eq!(lower_values, expected_lower);
}

#[test]
fn elimination_retains_recursive_pivots_and_chronological_source_weights() {
    let base = CoefficientContext::new(Vec::<String>::new());
    let context = IndexedCoefficientContext::try_new(&base, "pivot-chain", 1).unwrap();
    let ordering = OrderingPolicy::default();
    let integral = |power| {
        let key = IntegralKey::try_new([power]).unwrap();
        OrderedIntegral {
            complexity: ordering.complexity_key(key.powers()).unwrap(),
            key,
        }
    };
    let problem = PreparedProblem {
        family_fingerprint: Arc::new("pivot-chain-family".to_owned()),
        anchor: IntegralKey::try_new([1]).unwrap(),
        ordering,
        columns: vec![integral(3), integral(2), integral(1)],
        sources: vec![
            PreparedSourceRow {
                row_id: RowId::Derived {
                    label: Arc::from("first"),
                },
                entries: vec![(0, base.integer(2))],
                guards: Vec::new(),
            },
            PreparedSourceRow {
                row_id: RowId::Derived {
                    label: Arc::from("second"),
                },
                entries: vec![
                    (0, base.integer(4)),
                    (1, base.integer(10)),
                    (2, base.integer(6)),
                ],
                guards: Vec::new(),
            },
        ],
    };
    let limits = AnchoredRuleLimits::default();
    let reduced = reduce_rows(&context, &problem, limits).unwrap();

    assert_eq!(reduced.pivot_column, 1);
    assert_eq!(
        reduced
            .pivot_guards
            .iter()
            .map(|guard| guard.source_ordinal())
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert_eq!(reduced.pivot_guards[0].coefficient(), &base.integer(2));
    assert_eq!(reduced.pivot_guards[1].coefficient(), &base.integer(10));
    assert_eq!(
        reduced
            .source_combination
            .iter()
            .map(|contribution| contribution.source_ordinal())
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    let minus_one_fifth = base
        .try_div(&base.integer(-1), &base.integer(5), Default::default())
        .unwrap();
    let one_tenth = base
        .try_div(&base.one(), &base.integer(10), Default::default())
        .unwrap();
    assert_eq!(
        reduced.source_combination[0].coefficient(),
        &minus_one_fifth
    );
    assert_eq!(reduced.source_combination[1].coefficient(), &one_tenth);
    let replay = verify_exact_source_replay(&context, &problem, &reduced, limits).unwrap();
    assert_eq!(replay.source_rows_used(), 2);
    assert_eq!(replay.integral_columns_checked(), 3);
}

#[test]
fn generated_tadpole_row_yields_guarded_strict_descent_and_exact_replay() {
    let (base, context, relations, family_fingerprint) = tadpole_sources();
    let rule = derive_strictly_descending_rule(
        &context,
        &relations,
        &[1],
        OrderingPolicy::default(),
        AnchoredRuleLimits::default(),
    )
    .unwrap();

    assert_eq!(rule.family_fingerprint(), family_fingerprint);
    assert_eq!(rule.anchor().powers(), &[1]);
    assert_eq!(rule.pivot().powers(), &[2]);
    assert_eq!(rule.right_hand_side().len(), 1);
    assert_eq!(rule.right_hand_side()[0].integral().powers(), &[1]);
    assert!(rule.right_hand_side()[0].descent().verify());

    let d = base.parameter("d").unwrap();
    let m2 = base.parameter("m2").unwrap();
    let two = base.integer(2);
    let pivot = base.try_mul(&two, &m2, Default::default()).unwrap();
    assert!(
        base.try_sub(rule.pivot_guard().coefficient(), &pivot, Default::default())
            .unwrap()
            .is_zero()
    );
    assert_eq!(rule.pivot_guard().nonzero_polynomial(), &pivot.numerator);

    let expected_numerator = base.try_sub(&two, &d, Default::default()).unwrap();
    let expected_rule = base
        .try_div(&expected_numerator, &pivot, Default::default())
        .unwrap();
    assert!(
        base.try_sub(
            rule.right_hand_side()[0].coefficient(),
            &expected_rule,
            Default::default()
        )
        .unwrap()
        .is_zero()
    );
    let expected_source_weight = base
        .try_div(&base.one(), &pivot, Default::default())
        .unwrap();
    assert_eq!(rule.source_combination().len(), 1);
    assert_eq!(rule.source_combination()[0].source_ordinal(), 0);
    assert!(
        base.try_sub(
            rule.source_combination()[0].coefficient(),
            &expected_source_weight,
            Default::default()
        )
        .unwrap()
        .is_zero()
    );
    assert!(rule.nonzero_guards().iter().any(|guard| {
        guard.polynomial() == &pivot.numerator
            && guard.origins().iter().any(|origin| {
                matches!(
                    origin,
                    GuardOrigin::ReducerPivotNumerator {
                        pivot_column: 0,
                        ..
                    }
                )
            })
    }));
    assert_eq!(rule.replay().source_rows_used(), 1);
    assert_eq!(rule.replay().integral_columns_checked(), 2);
    assert!(rule.replay().exact_operations() > 0);
}

#[test]
fn anchored_rule_retains_specialized_source_domain_conditions() {
    let base = CoefficientContext::new(["d", "m2", "x"]);
    let x = base.parameter("x").unwrap();
    let shifted_power = base.try_div(&base.one(), &x, Default::default()).unwrap();
    let family = IntegralFamily::new(
        "foundry-guarded-tadpole",
        vec!["k".into()],
        Vec::new(),
        base.clone(),
        base.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            base.parameter("m2").unwrap(),
            vec![base.one()],
        )],
        Vec::new(),
        vec![shifted_power],
    )
    .unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let batch = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..batch.len())
        .map(|ordinal| batch.generate(ordinal))
        .collect();
    let relations = batch.complete(rows).unwrap().into_relations();
    assert!(!relations[0].nonzero_conditions().is_empty());

    let rule = derive_strictly_descending_rule(
        generator.context(),
        &relations,
        &[1],
        OrderingPolicy::default(),
        AnchoredRuleLimits::default(),
    )
    .unwrap();
    assert!(rule.nonzero_guards().iter().any(|guard| {
        guard.polynomial() == &x.numerator
            && guard
                .origins()
                .iter()
                .any(|origin| matches!(origin, GuardOrigin::SourceCondition { .. }))
    }));
}

#[test]
fn exact_replay_rejects_a_tampered_native_candidate() {
    let (_, context, relations, _) = tadpole_sources();
    let limits = AnchoredRuleLimits::default();
    let problem = prepare_problem(
        &context,
        &relations,
        &[1],
        OrderingPolicy::default(),
        limits,
    )
    .unwrap();
    let mut reduced = reduce_rows(&context, &problem, limits).unwrap();
    reduced.integral_entries[1].1 = context.base().zero();
    assert_eq!(
        verify_exact_source_replay(&context, &problem, &reduced, limits),
        Err(AnchoredRuleError::ReplayMismatch { integral_column: 1 })
    );
}

#[test]
fn structural_and_replay_limits_fail_with_typed_one_below_errors() {
    let (_, context, relations, _) = tadpole_sources();
    let mut limits = AnchoredRuleLimits {
        max_input_nonzero_entries: 2,
        ..AnchoredRuleLimits::default()
    };
    assert_eq!(
        derive_strictly_descending_rule(
            &context,
            &relations,
            &[1],
            OrderingPolicy::default(),
            limits,
        ),
        Err(AnchoredRuleError::ResourceLimit {
            resource: "prospective anchored source entries",
            requested: 3,
            limit: 2,
        })
    );

    limits = AnchoredRuleLimits {
        max_native_decomposition_nonzero_entries: 3,
        ..AnchoredRuleLimits::default()
    };
    assert_eq!(
        derive_strictly_descending_rule(
            &context,
            &relations,
            &[1],
            OrderingPolicy::default(),
            limits,
        ),
        Err(AnchoredRuleError::ResourceLimit {
            resource: "Symbolica sparse U/L nonzero entries",
            requested: 4,
            limit: 3,
        })
    );

    let exact = derive_strictly_descending_rule(
        &context,
        &relations,
        &[1],
        OrderingPolicy::default(),
        AnchoredRuleLimits::default(),
    )
    .unwrap();
    let one_below = exact.replay().exact_operations() - 1;
    limits = AnchoredRuleLimits {
        max_replay_exact_operations: one_below,
        ..AnchoredRuleLimits::default()
    };
    assert_eq!(
        derive_strictly_descending_rule(
            &context,
            &relations,
            &[1],
            OrderingPolicy::default(),
            limits,
        ),
        Err(AnchoredRuleError::ResourceLimit {
            resource: "anchored replay exact operations",
            requested: one_below + 1,
            limit: one_below,
        })
    );
}

#[test]
fn empty_sources_and_wrong_anchor_arity_are_typed() {
    let (_, context, relations, _) = tadpole_sources();
    assert_eq!(
        derive_strictly_descending_rule(
            &context,
            &[],
            &[1],
            OrderingPolicy::default(),
            AnchoredRuleLimits::default(),
        ),
        Err(AnchoredRuleError::EmptySourceRows)
    );
    assert_eq!(
        derive_strictly_descending_rule(
            &context,
            &relations,
            &[1, 2],
            OrderingPolicy::default(),
            AnchoredRuleLimits::default(),
        ),
        Err(AnchoredRuleError::WrongAnchorArity {
            expected: 1,
            actual: 2,
        })
    );
}

fn tadpole_sources() -> (
    CoefficientContext,
    IndexedCoefficientContext,
    Vec<ParametricRelation>,
    String,
) {
    let base = CoefficientContext::new(["d", "m2"]);
    let family = IntegralFamily::new(
        "foundry-tadpole",
        vec!["k".into()],
        Vec::new(),
        base.clone(),
        base.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            base.parameter("m2").unwrap(),
            vec![base.one()],
        )],
        Vec::new(),
        vec![base.zero()],
    )
    .unwrap();
    let family_fingerprint = family.fingerprint().to_owned();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let batch = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..batch.len())
        .map(|ordinal| batch.generate(ordinal))
        .collect();
    let relations = batch.complete(rows).unwrap().into_relations();
    assert_eq!(
        relations[0].row_id(),
        &RowId::OrdinaryIbp {
            contraction_momentum: 0,
            differentiated_loop: 0,
        }
    );
    (
        base,
        generator.context().clone(),
        relations,
        family_fingerprint,
    )
}
