//! Independent exact-routing regressions; no supplied master values are used.

use std::collections::BTreeSet;

use symbolica::domains::SelfRing;

use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily, IntegralKey};
use crate::sector::OrderingPolicy;
use crate::sector::symmetry::{DenominatorAction, Jacobian};

use super::super::{ProductSkipReason, TerminalAliasPlan};

fn key(powers: [i64; 6]) -> IntegralKey {
    IntegralKey::try_new(powers).unwrap()
}

fn family(name: &str, rows: [[i64; 3]; 6]) -> IntegralFamily {
    let context = CoefficientContext::new(["d"]);
    let denominators = rows
        .iter()
        .map(|q| {
            let coefficients = (0..3)
                .flat_map(|i| (i..3).map(move |j| q[i] * q[j] * if i == j { 1 } else { 2 }))
                .map(|n| context.integer(n))
                .collect();
            AffineDenominator::new(context.integer(-1), coefficients)
        })
        .collect();
    IntegralFamily::new(
        name,
        vec!["k1".into(), "k2".into(), "k3".into()],
        vec![],
        context.clone(),
        context.parameter("d").unwrap(),
        denominators,
        vec![],
        vec![context.zero(); 6],
    )
    .unwrap()
}

fn check_aliases(family: &IntegralFamily, plan: &TerminalAliasPlan) {
    for (source, alias) in plan.aliases() {
        let target = alias.representative();
        assert!(plan.raw_terminals().contains(target));
        assert!(!plan.aliases().contains_key(target));
        assert_eq!(
            OrderingPolicy::SpiredUncutV1
                .compare(target.powers(), source.powers())
                .unwrap(),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            source.powers().iter().sum::<i64>(),
            target.powers().iter().sum::<i64>()
        );
        assert!(matches!(
            alias.witness().as_momentum().unwrap().jacobian(),
            Jacobian::Unit { .. }
        ));
        for (slot, &power) in source.powers().iter().enumerate().filter(|(_, n)| **n > 0) {
            let DenominatorAction::Monomial {
                target: mapped,
                scale,
            } = &alias.witness().as_momentum().unwrap().row_actions()[slot]
            else {
                panic!("accepted active row has no exact monomial action");
            };
            assert!(family.coefficient_context().contains(scale));
            assert!(scale.is_one());
            assert_eq!(target.powers()[*mapped], power);
        }
    }
}

#[test]
fn fractional_circuit_coordinates_are_normalized_jointly_and_replayed() {
    // Inverting the first support basis gives k1 = (2*k2)/2 + (k1-k2).
    // Its primitive relation is (2,-1,0,-2), not a vector of rounded entries.
    let family = family(
        "audit-rational-circuit",
        [
            [1, 0, 0],
            [0, 2, 0],
            [0, 0, 2],
            [1, -1, 0],
            [1, 0, -1],
            [0, 1, -1],
        ],
    );
    let raw = BTreeSet::from([key([1, 1, 1, 1, 0, 0]), key([1, 1, 1, 0, 1, 0])]);
    let plan = TerminalAliasPlan::vacuum_routing_equivalences(
        &family,
        &raw,
        OrderingPolicy::SpiredUncutV1,
    )
    .unwrap();
    assert_eq!(plan.statistics().eligible_corank_one, 2);
    assert_eq!(plan.aliases().len(), 1);
    assert_eq!(plan.canonical_terminals().len(), 1);
    assert_eq!(plan.raw_terminals(), &raw);
    check_aliases(&family, &plan);
}

#[test]
fn a_nonunit_support_basis_does_not_forbid_a_unit_jacobian_routing() {
    // Both active supports generate a lattice of index two. Exchanging k2/k3
    // is nevertheless an integer determinant-minus-one map between them.
    let family = family(
        "audit-nonprimitive-support",
        [
            [2, 0, 0],
            [0, 1, 0],
            [0, 0, 1],
            [2, -1, 0],
            [2, 0, -1],
            [0, 1, -1],
        ],
    );
    let raw = BTreeSet::from([key([1, 1, 1, 1, 0, 0]), key([1, 1, 1, 0, 1, 0])]);
    let plan = TerminalAliasPlan::vacuum_routing_equivalences(
        &family,
        &raw,
        OrderingPolicy::SpiredUncutV1,
    )
    .unwrap();
    assert_eq!(plan.aliases().len(), 1);
    check_aliases(&family, &plan);
}

#[test]
fn matching_primitive_circuits_do_not_authorize_a_fractional_lattice_map() {
    let family = family(
        "audit-circuit-is-not-lattice-proof",
        [
            [2, 0, 0],
            [0, 1, 0],
            [0, 0, 1],
            [2, -1, 0],
            [1, 0, -1],
            [0, 1, -1],
        ],
    );
    let raw = BTreeSet::from([key([1, 1, 1, 1, 0, 0]), key([1, 1, 1, 0, 0, 1])]);
    let plan = TerminalAliasPlan::vacuum_routing_equivalences(
        &family,
        &raw,
        OrderingPolicy::SpiredUncutV1,
    )
    .unwrap();
    assert_eq!(plan.statistics().eligible_corank_one, 2);
    assert!(plan.aliases().is_empty());
    assert_eq!(plan.canonical_terminals(), &raw);
    assert_eq!(
        plan.statistics().skipped[&ProductSkipReason::NonIntegralMomentumMap],
        1
    );
}

#[test]
fn dots_on_coloops_are_not_confused_with_dots_on_the_circuit() {
    let family = family(
        "audit-circuit-power-colors",
        [
            [1, 0, 0],
            [0, 1, 0],
            [0, 0, 1],
            [1, -1, 0],
            [1, 0, -1],
            [0, 1, -1],
        ],
    );
    let raw = BTreeSet::from([
        key([1, 1, 2, 1, 0, 0]),  // dotted coloop k3
        key([2, 1, 1, 0, 1, 0]),  // dotted circuit line k1
        key([1, 1, 1, 1, 0, -1]), // unsupported numerator
    ]);
    let plan = TerminalAliasPlan::vacuum_routing_equivalences(
        &family,
        &raw,
        OrderingPolicy::SpiredUncutV1,
    )
    .unwrap();
    assert_eq!(plan.statistics().eligible_corank_one, 2);
    assert!(plan.aliases().is_empty());
    assert_eq!(plan.canonical_terminals(), &raw);
    assert_eq!(
        plan.statistics().skipped[&ProductSkipReason::NumeratorPowers],
        1
    );
}
