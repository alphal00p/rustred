use std::collections::BTreeSet;
use std::sync::Arc;

use symbolica::prelude::PolyVariable;

use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily, IntegralKey};
use crate::sector::OrderingPolicy;

use super::{ProductSkipReason as Skip, TerminalAliasError, TerminalAliasPlan};

fn family(rows: &[[i64; 3]]) -> IntegralFamily {
    let context = CoefficientContext::new(["d"]);
    let denominators = rows
        .iter()
        .map(|row| {
            AffineDenominator::new(
                context.integer(-1),
                row.iter().map(|&c| context.integer(c)).collect(),
            )
        })
        .collect();
    IntegralFamily::new(
        "terminal_product_test",
        vec!["k1".into(), "k2".into()],
        vec![],
        context.clone(),
        context.parameter("d").unwrap(),
        denominators,
        vec![],
        vec![context.zero(); 3],
    )
    .unwrap()
}

fn key(indices: [i64; 3]) -> IntegralKey {
    IntegralKey::try_new(indices).unwrap()
}

#[test]
fn equal_power_products_have_one_hop_descending_witnesses() {
    let family = family(&[[1, 0, 0], [0, 0, 1], [1, -2, 1]]);
    let raw = BTreeSet::from([key([1, 1, 0]), key([1, 0, 1]), key([0, 1, 1])]);
    let plan = TerminalAliasPlan::independent_tadpole_products(
        &family,
        &raw,
        OrderingPolicy::SpiredUncutV1,
    )
    .unwrap();
    assert_eq!(plan.raw_terminals(), &raw);
    assert_eq!(plan.statistics().eligible_products, 3);
    assert_eq!(plan.statistics().analyzed_denominators, 3);
    assert_eq!(plan.aliases().len(), 2);
    assert_eq!(plan.canonical_terminals().len(), 1);
    for (source, alias) in plan.aliases() {
        assert!(!plan.aliases().contains_key(alias.representative()));
        assert!(raw.contains(alias.representative()));
        assert_eq!(
            plan.representative(&family, source).unwrap(),
            alias.representative()
        );
        assert!(
            plan.ordering()
                .compare(alias.representative().powers(), source.powers())
                .unwrap()
                .is_lt()
        );
        assert!(
            alias
                .witness()
                .as_momentum()
                .unwrap()
                .nonzero_conditions()
                .iter()
                .all(|condition| condition.polynomial().is_constant())
        );
    }
}

#[test]
fn integer_scalar_square_is_proposed_but_nonunit_determinant_is_skipped() {
    let family = family(&[[4, 0, 0], [0, 0, 1], [1, -2, 1]]);
    let raw = BTreeSet::from([key([1, 1, 0]), key([1, 0, 1]), key([0, 1, 1])]);
    let plan =
        TerminalAliasPlan::independent_tadpole_products(&family, &raw, OrderingPolicy::default())
            .unwrap();
    assert_eq!(
        plan.statistics()
            .skipped
            .get(&Skip::NonUnimodularMomentumBasis),
        Some(&2)
    );
    assert_eq!(plan.statistics().eligible_products, 1);
    assert!(plan.aliases().is_empty());
    assert_eq!(plan.canonical_terminals(), &raw);
}

#[test]
fn nonsquare_quadratics_are_not_treated_as_momentum_lines() {
    let family = family(&[[1, 0, 1], [0, 0, 1], [1, -2, 1]]);
    let raw = BTreeSet::from([key([1, 1, 0]), key([0, 1, 1])]);
    let plan =
        TerminalAliasPlan::independent_tadpole_products(&family, &raw, OrderingPolicy::default())
            .unwrap();
    assert_eq!(
        plan.statistics().skipped.get(&Skip::NotLinearSquare),
        Some(&1)
    );
    assert!(plan.aliases().is_empty());
}

#[test]
fn truncated_scalar_root_cannot_create_a_false_square() {
    let family = family(&[[2, 0, 0], [0, 0, 1], [1, -2, 1]]);
    let raw = BTreeSet::from([key([1, 1, 0])]);
    let plan =
        TerminalAliasPlan::independent_tadpole_products(&family, &raw, OrderingPolicy::default())
            .unwrap();
    assert_eq!(
        plan.statistics().skipped.get(&Skip::NotLinearSquare),
        Some(&1)
    );
    assert_eq!(plan.statistics().eligible_products, 0);
    assert_eq!(plan.canonical_terminals(), &raw);
}

#[test]
fn product_helpers_reject_constant_context_rebinding() {
    let expected = CoefficientContext::new(["d"]);
    let foreign = CoefficientContext::new(["foreign_terminal_parameter"]);
    // Native polynomial equality equates constant values across maps; this
    // service must not use it as proof of coefficient-context ownership.
    assert_eq!(expected.zero(), foreign.zero());
    assert_eq!(
        super::products::integer(&foreign.zero(), &expected),
        Err(TerminalAliasError::InvalidCoefficientContext)
    );
}

#[test]
fn integer_square_replay_retains_sign_and_large_coefficients() {
    let family = family(&[[1, -4, 4], [0, 0, 1], [1, 0, 0]]);
    let variables = Arc::new(vec![PolyVariable::Temporary(0), PolyVariable::Temporary(1)]);
    let row = super::products::momentum(&family, 0, &variables)
        .unwrap()
        .unwrap();
    let context = family.coefficient_context();
    assert!(
        row == vec![context.one(), context.integer(-2)]
            || row == vec![context.integer(-1), context.integer(2)]
    );
}

#[test]
fn declaration_order_and_repeated_preparation_do_not_change_representatives() {
    let family = family(&[[1, 0, 0], [0, 0, 1], [1, -2, 1]]);
    let input = [key([2, 3, 0]), key([0, 2, 3]), key([2, 0, 3])];
    let first = TerminalAliasPlan::independent_tadpole_products(
        &family,
        &input.iter().cloned().collect(),
        OrderingPolicy::default(),
    )
    .unwrap();
    let second = TerminalAliasPlan::independent_tadpole_products(
        &family,
        &input.into_iter().rev().collect(),
        OrderingPolicy::default(),
    )
    .unwrap();
    assert_eq!(first.statistics(), second.statistics());
    assert_eq!(first.canonical_terminals(), second.canonical_terminals());
    for (key, alias) in first.aliases() {
        assert_eq!(
            alias.representative(),
            second.aliases()[key].representative()
        );
        assert_eq!(
            alias.witness().as_momentum().unwrap().momentum(),
            second.aliases()[key]
                .witness()
                .as_momentum()
                .unwrap()
                .momentum()
        );
    }
}
