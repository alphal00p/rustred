use std::collections::BTreeSet;

use symbolica::domains::SelfRing;

use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily, IntegralKey};
use crate::sector::{
    OrderingPolicy,
    symmetry::{DenominatorAction, Jacobian},
};

use super::super::{ProductSkipReason as Skip, TerminalAliasPlan};

fn family<const L: usize, const K: usize>(momenta: &[[i64; L]; K]) -> IntegralFamily {
    let context = CoefficientContext::new(["d"]);
    let denominators = momenta
        .iter()
        .map(|q| {
            let coefficients = (0..L)
                .flat_map(|i| (i..L).map(move |j| q[i] * q[j] * if i == j { 1 } else { 2 }))
                .map(|n| context.integer(n))
                .collect();
            AffineDenominator::new(context.integer(-1), coefficients)
        })
        .collect();
    IntegralFamily::new(
        "corank_one_test",
        (0..L).map(|i| format!("k{i}")).collect(),
        vec![],
        context.clone(),
        context.parameter("d").unwrap(),
        denominators,
        vec![],
        vec![context.zero(); K],
    )
    .unwrap()
}

fn standard() -> IntegralFamily {
    family(&[
        [1, 0, 0],
        [0, 1, 0],
        [0, 0, 1],
        [1, 1, 0],
        [1, 0, 1],
        [0, 1, 1],
    ])
}

fn key(values: [i64; 6]) -> IntegralKey {
    IntegralKey::try_new(values).unwrap()
}

fn prepare(family: &IntegralFamily, keys: &[[i64; 6]]) -> TerminalAliasPlan {
    TerminalAliasPlan::vacuum_routing_equivalences(
        family,
        &keys.iter().copied().map(key).collect(),
        OrderingPolicy::SpiredUncutV1,
    )
    .unwrap()
}

#[test]
fn matching_circuits_with_coloops_and_dots_have_exact_descending_routes() {
    let family = standard();
    let raw = [
        [1, 1, 1, 1, 0, 0],
        [1, 1, 1, 0, 1, 0],
        [1, 1, 1, 0, 0, 1],
        [2, 1, 1, 1, 0, 0],
        [1, 1, 2, 0, 1, 0],
    ];
    let plan = prepare(&family, &raw);
    assert_eq!(plan.statistics().eligible_corank_one, 5);
    assert_eq!(plan.statistics().analyzed_corank_one_supports, 3);
    assert_eq!(plan.aliases().len(), 3);
    assert_eq!(plan.canonical_terminals().len(), 2);
    assert!(
        !plan
            .statistics()
            .skipped
            .contains_key(&Skip::ActiveLineCount)
    );
    for (source, alias) in plan.aliases() {
        assert!(
            plan.ordering()
                .compare(alias.representative().powers(), source.powers())
                .unwrap()
                .is_lt()
        );
        assert!(!plan.aliases().contains_key(alias.representative()));
        assert!(matches!(
            alias.witness().as_momentum().unwrap().jacobian(),
            Jacobian::Unit { .. }
        ));
        for (slot, &power) in source.powers().iter().enumerate().filter(|(_, n)| **n > 0) {
            let DenominatorAction::Monomial { target, scale } =
                &alias.witness().as_momentum().unwrap().row_actions()[slot]
            else {
                panic!("active square must replay monomially")
            };
            assert!(family.coefficient_context().contains(scale));
            assert!(scale.is_one());
            assert_eq!(power, alias.representative().powers()[*target]);
        }
    }
}

#[test]
fn circuit_and_coloop_powers_are_not_interchangeable() {
    let family = standard();
    let plan = prepare(
        &family,
        &[[1, 1, 2, 1, 0, 0], [2, 1, 1, 1, 0, 0], [2, 1, 1, 0, 1, 0]],
    );
    assert_eq!(plan.aliases().len(), 1);
    assert_eq!(plan.canonical_terminals().len(), 2);
    assert_eq!(
        plan.representative(&family, &key([1, 1, 2, 1, 0, 0]))
            .unwrap(),
        &key([1, 1, 2, 1, 0, 0])
    );
}

#[test]
fn equal_support_size_does_not_erase_primitive_coefficient_magnitudes() {
    let family = family(&[
        [1, 0, 0],
        [0, 1, 0],
        [0, 0, 1],
        [1, 1, 0],
        [1, 0, 2],
        [0, 1, 1],
    ]);
    let plan = prepare(&family, &[[1, 1, 1, 1, 0, 0], [1, 1, 1, 0, 1, 0]]);
    assert_eq!(plan.statistics().eligible_corank_one, 2);
    assert!(plan.aliases().is_empty());
}

#[test]
fn nonprimitive_bases_are_allowed_when_final_routing_is_unimodular() {
    let family = family(&[
        [2, 0, 0],
        [0, 2, 0],
        [0, 0, 2],
        [2, 2, 0],
        [2, 0, 2],
        [0, 2, 2],
    ]);
    let plan = prepare(
        &family,
        &[[1, 1, 1, 1, 0, 0], [1, 1, 1, 0, 1, 0], [1, 1, 1, 0, 0, 1]],
    );
    assert_eq!(plan.aliases().len(), 2);
    assert_eq!(plan.statistics().eligible_corank_one, 3);
}

#[test]
fn combined_lane_preserves_old_products_and_keeps_unsupported_keys() {
    let family = standard();
    let keys = [
        [1, 1, 1, 0, 0, 0],
        [1, 1, 0, 0, 1, 0],
        [1, 1, 1, 1, 0, 0],
        [1, 1, 1, 0, 1, 0],
        [1, 1, 1, 1, -1, 0],
        [1, 1, 1, 1, 1, 0],
    ];
    let raw: BTreeSet<_> = keys.into_iter().map(key).collect();
    let old = TerminalAliasPlan::independent_tadpole_products(
        &family,
        &raw,
        OrderingPolicy::SpiredUncutV1,
    )
    .unwrap();
    let new = prepare(&family, &keys);
    assert_eq!(new.raw_terminals(), &raw);
    assert_eq!(
        new.statistics().eligible_products,
        old.statistics().eligible_products
    );
    assert_eq!(new.statistics().eligible_corank_one, 2);
    assert_eq!(new.aliases().len(), old.aliases().len() + 1);
    for (source, alias) in old.aliases() {
        assert_eq!(
            new.aliases()[source].representative(),
            alias.representative()
        );
        assert_eq!(
            new.aliases()[source]
                .witness()
                .as_momentum()
                .unwrap()
                .momentum(),
            alias.witness().as_momentum().unwrap().momentum()
        );
    }
    assert_eq!(new.statistics().skipped[&Skip::NumeratorPowers], 1);
    assert_eq!(new.statistics().skipped[&Skip::ActiveLineCount], 1);
}

#[test]
fn repeated_preparation_and_input_order_preserve_full_witnesses() {
    let family = standard();
    let mut keys = [
        [2, 1, 1, 1, 0, 0],
        [1, 1, 2, 0, 1, 0],
        [1, 1, 1, 1, 0, 0],
        [1, 1, 1, 0, 1, 0],
    ];
    let first = prepare(&family, &keys);
    keys.reverse();
    for _ in 0..3 {
        let next = prepare(&family, &keys);
        assert_eq!(next.statistics(), first.statistics());
        assert_eq!(next.canonical_terminals(), first.canonical_terminals());
        for (source, alias) in first.aliases() {
            assert_eq!(
                next.aliases()[source].representative(),
                alias.representative()
            );
            assert_eq!(
                next.aliases()[source]
                    .witness()
                    .as_momentum()
                    .unwrap()
                    .momentum(),
                alias.witness().as_momentum().unwrap().momentum()
            );
        }
    }
}

#[test]
fn large_integer_shears_use_native_exact_algebra() {
    let family = family(&[
        [1, 1_000_000_000, 0],
        [0, 1, 0],
        [0, 0, 1],
        [1, 1_000_000_001, 0],
        [1, 1_000_000_000, 1],
        [0, 1, 1],
    ]);
    let plan = prepare(&family, &[[1, 1, 1, 1, 0, 0], [1, 1, 1, 0, 1, 0]]);
    assert_eq!(plan.statistics().eligible_corank_one, 2);
    assert_eq!(plan.aliases().len(), 1);
}

#[test]
fn rank_deficient_active_support_does_not_become_a_circuit_alias() {
    let family = family(&[
        [1, 0, 0, 0],
        [0, 1, 0, 0],
        [0, 0, 1, 0],
        [1, 1, 0, 0],
        [1, 0, 1, 0],
        [0, 0, 0, 1],
        [0, 1, 1, 0],
        [1, 0, 0, 1],
        [0, 1, 0, 1],
        [0, 0, 1, 1],
    ]);
    let raw = BTreeSet::from([IntegralKey::try_new([1, 1, 1, 1, 1, 0, 0, 0, 0, 0]).unwrap()]);
    let plan = TerminalAliasPlan::vacuum_routing_equivalences(
        &family,
        &raw,
        OrderingPolicy::SpiredUncutV1,
    )
    .unwrap();
    assert_eq!(plan.statistics().eligible_corank_one, 0);
    assert_eq!(plan.statistics().skipped[&Skip::SingularMomentumBasis], 1);
    assert_eq!(plan.canonical_terminals(), &raw);
}

#[test]
fn vacuum_subproducts_route_without_a_full_family_isp_permutation() {
    let family = family(&[
        [1, 0, 0, 0],
        [0, 1, 0, 0],
        [0, 0, 1, 0],
        [1, 0, -1, 0],
        [0, 0, 0, 1],
        [0, 1, -1, 0],
        [1, 0, -1, 1],
        [1, -1, 0, 0],
        // Deliberately asymmetric unused ISP: the active subproducts still
        // route identically, but a full-family permutation is unavailable.
        [0, 1, 0, 2],
        [0, 0, 1, -1],
    ]);
    // Each support is a sunset times two one-loop factors, expressed through
    // different parent coordinates. No lower master value enters discovery.
    let raw = BTreeSet::from([
        IntegralKey::try_new([0, 0, 1, 1, 0, 1, 1, 1, 0, 0]).unwrap(),
        IntegralKey::try_new([0, 0, 1, 1, 1, 0, 1, 1, 0, 0]).unwrap(),
    ]);
    let plan = TerminalAliasPlan::vacuum_routing_equivalences(
        &family,
        &raw,
        OrderingPolicy::SpiredUncutV1,
    )
    .unwrap();
    assert_eq!(plan.statistics().eligible_corank_one, 2);
    assert_eq!(plan.aliases().len(), 1);
    assert!(plan.aliases().values().all(|alias| {
        alias
            .witness()
            .as_momentum()
            .unwrap()
            .row_actions()
            .iter()
            .any(|action| matches!(action, DenominatorAction::Affine))
    }));
}
