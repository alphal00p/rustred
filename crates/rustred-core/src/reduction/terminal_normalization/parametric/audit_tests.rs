//! Independent adversarial checks of the Schwinger-parameter proof contract.
//! No oracle values or reduction rules enter these fixtures.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::algebra::CoefficientContext;
use crate::family::symanzik::SymanzikPolynomials;
use crate::family::{AffineDenominator, IntegralFamily, IntegralKey};
use crate::sector::OrderingPolicy;

use super::super::{TerminalAliasError, TerminalAliasPlan, VacuumParametricLimits};
use super::{geometry, verify};

fn key(powers: [i64; 6]) -> IntegralKey {
    IntegralKey::try_new(powers).unwrap()
}

fn family(name: &str, rows: [[i64; 3]; 6]) -> IntegralFamily {
    family_with_conditions(name, rows, 1, [0; 6])
}

fn family_with_conditions(
    name: &str,
    rows: [[i64; 3]; 6],
    mass_squared: i64,
    shifts: [i64; 6],
) -> IntegralFamily {
    let context = CoefficientContext::new(["d"]);
    let denominators = rows
        .iter()
        .map(|q| {
            let coefficients = (0..3)
                .flat_map(|i| (i..3).map(move |j| q[i] * q[j] * if i == j { 1 } else { 2 }))
                .map(|n| context.integer(n))
                .collect();
            AffineDenominator::new(context.integer(-mass_squared), coefficients)
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
        shifts.into_iter().map(|n| context.integer(n)).collect(),
    )
    .unwrap()
}

fn nonprimitive_family() -> IntegralFamily {
    family(
        "audit-parametric-nonprimitive-products",
        [
            [1, 0, 0],
            [0, 2, 0],
            [0, 0, 1],
            [1, 2, 0],
            [1, 0, 1],
            [0, 1, 1],
        ],
    )
}

fn prepare(family: &IntegralFamily, raw: &BTreeSet<IntegralKey>) -> TerminalAliasPlan {
    TerminalAliasPlan::vacuum_parametric_equivalences(
        family,
        raw,
        OrderingPolicy::SpiredUncutV1,
        VacuumParametricLimits::default(),
    )
    .unwrap()
}

#[test]
fn noninvolutive_parameter_permutation_preserves_each_power_and_full_scale() {
    // Both products have U = 4*x*y*z. The source/target power colors force
    // a three-cycle, not an involution, in sorted active-slot coordinates.
    // Neither product has an independently unimodular momentum basis.
    let family = nonprimitive_family();
    let raw = BTreeSet::from([key([1, 2, 3, 0, 0, 0]), key([2, 0, 3, 1, 0, 0])]);
    let plan = prepare(&family, &raw);
    assert_eq!(plan.raw_terminals(), &raw);
    assert_eq!(plan.aliases().len(), 1);
    assert_eq!(plan.canonical_terminals().len(), 1);
    let (source, alias) = plan.aliases().first_key_value().unwrap();
    assert!(alias.witness().as_momentum().is_none());
    let proof = alias.witness().as_parametric().unwrap();
    assert_eq!(proof.loop_count(), 3);
    let source_slots = proof.source_slots();
    let representative_slots = proof.representative_slots();
    let permutation = proof.parameter_permutation();
    assert_eq!(source_slots.len(), 3);
    assert_eq!(representative_slots.len(), 3);
    assert_eq!(permutation.len(), 3);
    assert_eq!(
        permutation.iter().copied().collect::<BTreeSet<_>>(),
        BTreeSet::from([0, 1, 2])
    );
    assert!((0..3).any(|i| permutation[permutation[i]] != i));
    for (local, &source_slot) in source_slots.iter().enumerate() {
        let target_slot = representative_slots[permutation[local]];
        assert_eq!(
            source.powers()[source_slot],
            alias.representative().powers()[target_slot]
        );
    }
    assert!(raw.contains(alias.representative()));
    assert!(!plan.aliases().contains_key(alias.representative()));
    assert_eq!(
        OrderingPolicy::SpiredUncutV1
            .compare(alias.representative().powers(), source.powers())
            .unwrap(),
        std::cmp::Ordering::Less
    );
    assert_eq!(
        source.powers().iter().sum::<i64>(),
        alias.representative().powers().iter().sum::<i64>()
    );
}

#[test]
fn matching_monomial_support_does_not_erase_the_overall_determinant_scale() {
    let family = nonprimitive_family();
    // First basis has determinant 2, second determinant 1. Their U
    // polynomials have the same monomial support but coefficients 4 and 1.
    let raw = BTreeSet::from([key([1, 1, 1, 0, 0, 0]), key([1, 0, 1, 0, 0, 1])]);
    let plan = prepare(&family, &raw);
    assert!(plan.aliases().is_empty());
    assert_eq!(plan.canonical_terminals(), &raw);
}

#[test]
fn rational_unit_jacobian_equivalence_retains_the_exact_twenty_five_coefficient() {
    for (target_scale, expected_aliases) in [(5, 1), (6, 0)] {
        let family = family(
            "audit-parametric-rational-shear",
            [
                [5, 0, 0],
                [0, 1, 0],
                [0, 0, 1],
                [target_scale, 1, 0],
                [1, 0, 1],
                [0, 1, 1],
            ],
        );
        let raw = BTreeSet::from([key([1, 1, 1, 0, 0, 0]), key([0, 1, 1, 1, 0, 0])]);
        // The old product lane deliberately excludes these nonunit bases.
        let routing = TerminalAliasPlan::independent_tadpole_products(
            &family,
            &raw,
            OrderingPolicy::SpiredUncutV1,
        )
        .unwrap();
        assert!(routing.aliases().is_empty());
        // U_source = 25*x*y*z. For scale 5, the rational shear k1 ->
        // k1+k2/5 has determinant one; for scale 6, U_target = 36*x*y*z.
        let plan = prepare(&family, &raw);
        assert_eq!(plan.aliases().len(), expected_aliases);
        assert_eq!(
            plan.canonical_terminals().len(),
            raw.len() - expected_aliases
        );
        for alias in plan.aliases().values() {
            assert!(alias.witness().as_parametric().is_some());
            assert!(alias.witness().as_momentum().is_none());
        }
    }
}

#[test]
fn equal_total_power_does_not_identify_circuit_and_coloop_dots() {
    let family = family(
        "audit-parametric-power-colors",
        [
            [1, 0, 0],
            [0, 1, 0],
            [0, 0, 1],
            [1, 1, 0],
            [1, 0, 1],
            [0, 1, 1],
        ],
    );
    let raw = BTreeSet::from([key([2, 1, 1, 1, 0, 0]), key([1, 1, 2, 1, 0, 0])]);
    let plan = prepare(&family, &raw);
    assert!(plan.aliases().is_empty());
    assert_eq!(plan.canonical_terminals(), &raw);
}

#[test]
fn zero_symanzik_polynomials_never_authorize_a_terminal_alias() {
    let family = family(
        "audit-parametric-singular-support",
        [
            [1, 0, 0],
            [0, 1, 0],
            [1, 1, 0],
            [0, 0, 1],
            [1, 0, 1],
            [0, 1, 1],
        ],
    );
    let raw = BTreeSet::from([key([2, 1, 1, 0, 0, 0]), key([1, 2, 1, 0, 0, 0])]);
    let plan = prepare(&family, &raw);
    assert!(plan.aliases().is_empty());
    assert_eq!(plan.canonical_terminals(), &raw);
}

#[test]
fn numerator_indices_are_not_silently_dropped_from_the_parameter_integrand() {
    let family = nonprimitive_family();
    let raw = BTreeSet::from([key([1, 1, 1, -1, 0, 0]), key([1, 1, 1, 0, -1, 0])]);
    let plan = prepare(&family, &raw);
    assert!(plan.aliases().is_empty());
    assert_eq!(plan.canonical_terminals(), &raw);
}

#[test]
fn preparation_limits_retain_raw_keys_instead_of_creating_partial_authority() {
    let family = nonprimitive_family();
    let raw = BTreeSet::from([key([1, 2, 3, 0, 0, 0]), key([2, 0, 3, 1, 0, 0])]);
    for budget in 0..6 {
        let mut limits = VacuumParametricLimits::default();
        match budget {
            0 => limits.max_supports = 0,
            1 => limits.max_canonicalizations = 0,
            2 => limits.max_graph_vertices = 0,
            3 => limits.max_graph_edges = 0,
            4 => limits.symanzik.max_polynomial_terms = 0,
            5 => limits.symanzik.exact_algebra.max_polynomial_terms = 0,
            _ => unreachable!(),
        }
        let plan = TerminalAliasPlan::vacuum_parametric_equivalences(
            &family,
            &raw,
            OrderingPolicy::SpiredUncutV1,
            limits,
        )
        .unwrap();
        assert!(plan.aliases().is_empty(), "budget {budget}");
        assert_eq!(plan.raw_terminals(), &raw);
        assert_eq!(plan.canonical_terminals(), &raw);
    }
}

#[test]
fn nonunit_mass_and_nonzero_power_offsets_remain_outside_the_proof_domain() {
    let rows = [
        [1, 0, 0],
        [0, 2, 0],
        [0, 0, 1],
        [1, 2, 0],
        [1, 0, 1],
        [0, 1, 1],
    ];
    let raw = BTreeSet::from([key([1, 2, 3, 0, 0, 0]), key([2, 0, 3, 1, 0, 0])]);
    for (mass, shifts) in [(2, [0; 6]), (1, [1, 0, 0, 0, 0, 0])] {
        let family = family_with_conditions("audit-parametric-domain", rows, mass, shifts);
        let plan = prepare(&family, &raw);
        assert!(plan.aliases().is_empty());
        assert_eq!(plan.canonical_terminals(), &raw);
    }
}

#[test]
fn parameter_proofs_remain_bound_to_the_original_family_and_declarations() {
    let family = nonprimitive_family();
    let raw = BTreeSet::from([key([1, 2, 3, 0, 0, 0]), key([2, 0, 3, 1, 0, 0])]);
    let plan = prepare(&family, &raw);
    let foreign = family_with_conditions(
        "audit-parametric-foreign-family",
        [
            [1, 0, 0],
            [0, 2, 0],
            [0, 0, 1],
            [1, 3, 0],
            [1, 0, 1],
            [0, 1, 1],
        ],
        1,
        [0; 6],
    );
    assert!(plan.representative(&foreign, raw.first().unwrap()).is_err());
    assert!(
        plan.representative(&family, &key([9, 1, 1, 0, 0, 0]))
            .is_err()
    );
    assert_eq!(plan.raw_terminals(), &raw);
    for (source, alias) in plan.aliases() {
        assert_eq!(
            plan.representative(&family, source).unwrap(),
            alias.representative()
        );
        assert!(!plan.aliases().contains_key(alias.representative()));
    }
}

#[test]
fn exact_replay_independently_rejects_scale_and_constant_context_mismatches() {
    let family = nonprimitive_family();
    let symanzik =
        SymanzikPolynomials::try_from_family_with_limits(&family, Default::default()).unwrap();
    let variables = Arc::new(
        (0..3)
            .map(symbolica::prelude::PolyVariable::Temporary)
            .collect(),
    );
    let mut momenta = vec![None; 6];
    let mut statistics = Default::default();
    let source = geometry::prepare(
        &family,
        &[0, 1, 2],
        symanzik.u().raw(),
        &variables,
        &mut momenta,
        &mut statistics,
    )
    .unwrap()
    .unwrap();
    let target = geometry::prepare(
        &family,
        &[0, 2, 5],
        symanzik.u().raw(),
        &variables,
        &mut momenta,
        &mut statistics,
    )
    .unwrap()
    .unwrap();
    let source_key = key([1, 1, 1, 0, 0, 0]);
    assert!(matches!(
        verify::prove(
            &family,
            &source_key,
            &key([1, 0, 1, 0, 0, 1]),
            source.clone(),
            target,
            vec![0, 1, 2],
        ),
        Err(TerminalAliasError::InvalidParametricWitness)
    ));
    assert_eq!(source.u.nterms(), 1);
    let mut foreign = (*source).clone();
    foreign.u.coefficients[0] = CoefficientContext::new(["foreign_dimension"]).integer(4);
    assert!(matches!(
        verify::prove(
            &family,
            &source_key,
            &source_key,
            Arc::new(foreign),
            source,
            vec![0, 1, 2],
        ),
        Err(TerminalAliasError::InvalidParametricWitness)
    ));
}
