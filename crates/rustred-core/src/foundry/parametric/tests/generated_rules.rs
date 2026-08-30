use crate::sector::OrderingPolicy;

use super::super::{ParametricGuardOrigin, ParametricRuleLimits, derive_sector_interior_rule};
use super::support::{sunset_sources, tadpole_sources, two_source_ibp_li_sources};

#[test]
fn generated_tadpole_yields_a_genuine_guarded_parametric_rule() {
    let (base, context, relations, family_fingerprint) = tadpole_sources();
    let rule = derive_sector_interior_rule(
        &context,
        &relations,
        &[1],
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )
    .unwrap();

    assert_eq!(rule.family_fingerprint(), family_fingerprint);
    assert_eq!(rule.context_fingerprint(), context.fingerprint());
    assert_eq!(rule.anchor().powers(), &[1]);
    assert_eq!(rule.sector().active_bits(), &[true]);
    assert_eq!(rule.domain().bounds()[0].lower(), 1);
    assert_eq!(rule.domain().bounds()[0].upper(), i64::MAX - 1);
    assert_eq!(rule.pivot().values(), &[1]);
    assert_eq!(rule.right_hand_side().len(), 1);
    assert_eq!(rule.right_hand_side()[0].shift().values(), &[0]);
    assert!(rule.right_hand_side()[0].descent().verify());

    let n = context.index(0).unwrap();
    let two = context.integer(2);
    let d = context.lift(&base.parameter("d").unwrap()).unwrap();
    let m2 = context.lift(&base.parameter("m2").unwrap()).unwrap();
    let two_n = context.mul(&two, &n).unwrap();
    let numerator = context.sub(&two_n, &d).unwrap();
    let two_m2 = context.mul(&two, &m2).unwrap();
    let denominator = context.mul(&two_m2, &n).unwrap();
    let expected = context.div(&numerator, &denominator).unwrap();
    assert!(
        context
            .sub(rule.right_hand_side()[0].coefficient(), &expected)
            .unwrap()
            .is_zero()
    );
    let expected_source = context.div(&context.one(), &denominator).unwrap();
    assert_eq!(rule.source_combination().len(), 1);
    assert!(
        context
            .sub(rule.source_combination()[0].coefficient(), &expected_source,)
            .unwrap()
            .is_zero()
    );

    assert_eq!(rule.elimination_pivot_guards().len(), 1);
    assert_eq!(rule.pivot_guard().pivot_shift().values(), &[1]);
    assert!(rule.nonzero_guards().iter().any(|guard| {
        guard.origins().iter().any(|origin| {
            matches!(
                origin,
                ParametricGuardOrigin::ReducerPivotNumerator {
                    pivot_shift,
                    ..
                } if pivot_shift.values() == [1]
            )
        })
    }));
    assert!(rule.nonzero_guards().iter().any(|guard| {
        guard.origins().iter().any(|origin| {
            matches!(
                origin,
                ParametricGuardOrigin::RuleCoefficientDenominator { .. }
            )
        })
    }));
    assert!(rule.nonzero_guards().iter().any(|guard| {
        guard.origins().iter().any(|origin| {
            matches!(
                origin,
                ParametricGuardOrigin::SourceCombinationDenominator { .. }
            )
        })
    }));
    assert_eq!(rule.replay().source_rows_used(), 1);
    assert_eq!(rule.replay().shift_columns_checked(), 2);
    assert!(rule.replay().exact_operations() > 0);
    assert_eq!(rule.concrete_replay().right_hand_side_terms_checked(), 1);
    assert_eq!(rule.concrete_replay().source_contributions_checked(), 1);
    assert_eq!(rule.concrete_replay().source_terms_checked(), 2);
    assert_eq!(rule.concrete_replay().integral_keys_checked(), 4);
    assert_eq!(
        rule.concrete_replay().nonzero_guards_checked(),
        rule.nonzero_guards().len()
    );
    assert_eq!(rule.concrete_replay().anchor().powers(), &[1]);
    assert!(rule.concrete_replay().exact_operations() > 0);
}

#[test]
fn held_out_tadpole_specialization_matches_the_exact_recurrence() {
    let (base, context, relations, _) = tadpole_sources();
    let rule = derive_sector_interior_rule(
        &context,
        &relations,
        &[1],
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )
    .unwrap();

    let (actual, denominator_guard) = context
        .specialize(
            rule.right_hand_side()[0].coefficient(),
            &[2],
            Default::default(),
        )
        .unwrap();
    assert!(denominator_guard.is_some());
    let expected_numerator = base
        .try_sub(
            &base.integer(4),
            &base.parameter("d").unwrap(),
            Default::default(),
        )
        .unwrap();
    let expected_denominator = base
        .try_mul(
            &base.integer(4),
            &base.parameter("m2").unwrap(),
            Default::default(),
        )
        .unwrap();
    let expected = base
        .try_div(
            &expected_numerator,
            &expected_denominator,
            Default::default(),
        )
        .unwrap();
    assert_eq!(actual, expected);
    assert!(rule.nonzero_guards().iter().all(|guard| {
        !context
            .specialize_polynomial(guard.polynomial(), &[2], Default::default())
            .unwrap()
            .is_zero()
    }));
}

#[test]
fn generated_equal_mass_sunset_selected_source_span_is_descending_and_replayed() {
    let (base, context, relations) = sunset_sources();
    // This deliberately authenticates one caller-selected generated source
    // span. It is a connected two-loop rule sentinel, not a complete-source
    // search or a sector-closure claim.
    let rule = derive_sector_interior_rule(
        &context,
        &relations[1..=1],
        &[2, 3, 4],
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )
    .unwrap();

    assert_eq!(rule.sector().active_bits(), &[true, true, true]);
    assert_eq!(rule.domain().bounds()[0].lower(), 2);
    assert_eq!(rule.domain().bounds()[0].upper(), i64::MAX);
    assert_eq!(rule.domain().bounds()[1].lower(), 2);
    assert_eq!(rule.domain().bounds()[1].upper(), i64::MAX - 1);
    assert_eq!(rule.domain().bounds()[2].lower(), 2);
    assert_eq!(rule.domain().bounds()[2].upper(), i64::MAX - 1);
    assert_eq!(rule.pivot().values(), &[0, 1, 0]);

    let shifts: Vec<&[i64]> = rule
        .right_hand_side()
        .iter()
        .map(|term| term.shift().values())
        .collect();
    assert_eq!(
        shifts,
        vec![
            &[0, 0, 1][..],
            &[0, 1, -1][..],
            &[0, 0, 0][..],
            &[0, -1, 1][..],
            &[-1, 1, 0][..],
            &[-1, 0, 1][..],
        ]
    );
    assert!(
        rule.right_hand_side()
            .iter()
            .all(|term| term.descent().verify())
    );

    let n1 = context.index(1).unwrap();
    let n2 = context.index(2).unwrap();
    let s = context.lift(&base.parameter("s").unwrap()).unwrap();
    let s_n1 = context.mul(&s, &n1).unwrap();
    let one_over_s = context.div(&context.one(), &s).unwrap();
    let n2_over_s_n1 = context.div(&n2, &s_n1).unwrap();
    let expected = [
        context.div(&n2, &n1).unwrap(),
        one_over_s.clone(),
        context.div(&context.sub(&n2, &n1).unwrap(), &s_n1).unwrap(),
        context
            .neg_with_limits(&n2_over_s_n1, Default::default())
            .unwrap(),
        context
            .neg_with_limits(&one_over_s, Default::default())
            .unwrap(),
        n2_over_s_n1,
    ];
    for (term, expected) in rule.right_hand_side().iter().zip(&expected) {
        assert!(context.sub(term.coefficient(), expected).unwrap().is_zero());
    }

    assert_eq!(rule.elimination_pivot_guards().len(), 1);
    assert_eq!(rule.pivot_guard().pivot_shift().values(), &[0, 1, 0]);
    assert_eq!(rule.nonzero_guards().len(), 3);
    assert_eq!(rule.replay().source_rows_used(), 1);
    assert_eq!(rule.replay().shift_columns_checked(), 7);
    assert_eq!(rule.replay().exact_operations(), 14);
    assert_eq!(rule.concrete_replay().anchor().powers(), &[2, 3, 4]);
    assert_eq!(rule.concrete_replay().right_hand_side_terms_checked(), 6);
    assert_eq!(rule.concrete_replay().source_contributions_checked(), 1);
    assert_eq!(
        rule.concrete_replay().integral_keys_checked(),
        rule.concrete_replay().source_terms_checked() + 7
    );
    assert_eq!(rule.concrete_replay().nonzero_guards_checked(), 3);
}

#[test]
fn generated_ordinary_and_li_rows_yield_a_genuine_two_source_rule() {
    let (base, context, relations) = two_source_ibp_li_sources();
    let rule = derive_sector_interior_rule(
        &context,
        &relations,
        &[1, 2, 3, 1],
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )
    .unwrap();

    assert_eq!(rule.pivot().values(), &[0, 0, -1, 1]);
    assert_eq!(rule.right_hand_side().len(), 2);
    assert_eq!(rule.right_hand_side()[0].shift().values(), &[0, -1, 1, 0]);
    assert_eq!(rule.right_hand_side()[1].shift().values(), &[0, -1, 0, 1]);
    assert!(
        rule.right_hand_side()
            .iter()
            .all(|term| term.descent().verify())
    );

    let n0 = context.index(0).unwrap();
    let n1 = context.index(1).unwrap();
    let n2 = context.index(2).unwrap();
    let n3 = context.index(3).unwrap();
    let d = context.lift(&base.parameter("d").unwrap()).unwrap();
    let two_n0 = context.mul(&context.integer(2), &n0).unwrap();
    let mut a = context.sub(&d, &two_n0).unwrap();
    for index in [&n1, &n2, &n3] {
        a = context.sub(&a, index).unwrap();
    }
    let expected_first_rhs = context.div(&n2, &n3).unwrap();
    assert!(
        context
            .sub(rule.right_hand_side()[0].coefficient(), &expected_first_rhs,)
            .unwrap()
            .is_zero()
    );
    assert!(
        context
            .sub(rule.right_hand_side()[1].coefficient(), &context.one())
            .unwrap()
            .is_zero()
    );

    assert_eq!(
        rule.source_combination()
            .iter()
            .map(|source| source.source_ordinal())
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    let source_numerator = context.sub(&n1, &n2).unwrap();
    let source_denominator = context.mul(&a, &n3).unwrap();
    let expected_ordinary_weight = context.div(&source_numerator, &source_denominator).unwrap();
    let expected_li_weight = context.div(&context.integer(-1), &n3).unwrap();
    assert!(
        context
            .sub(
                rule.source_combination()[0].coefficient(),
                &expected_ordinary_weight,
            )
            .unwrap()
            .is_zero()
    );
    assert!(
        context
            .sub(
                rule.source_combination()[1].coefficient(),
                &expected_li_weight,
            )
            .unwrap()
            .is_zero()
    );
    assert_eq!(
        rule.elimination_pivot_guards()
            .iter()
            .map(|guard| guard.source_ordinal())
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert_eq!(rule.replay().source_rows_used(), 2);
    assert_eq!(rule.concrete_replay().source_contributions_checked(), 2);
    assert!(rule.concrete_replay().source_terms_checked() > 0);
    assert_eq!(rule.concrete_replay().anchor().powers(), &[1, 2, 3, 1]);
}
