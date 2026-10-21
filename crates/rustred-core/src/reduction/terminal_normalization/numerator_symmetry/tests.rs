use super::*;
use crate::algebra::CoefficientContext;
use crate::family::AffineDenominator;

fn family(masses: [i64; 3], shift: bool) -> Arc<IntegralFamily> {
    let context = CoefficientContext::new(["d"]);
    Arc::new(
        IntegralFamily::new(
            "numerator_symmetry_test",
            vec!["k".into(), "l".into()],
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            [
                ([1, 0, 0], masses[0]),
                ([0, 0, 1], masses[1]),
                ([1, 2, 1], masses[2]),
            ]
            .into_iter()
            .map(|(row, mass)| {
                AffineDenominator::new(
                    context.integer(-mass),
                    row.map(|value| context.integer(value)).into(),
                )
            })
            .collect(),
            vec![],
            vec![
                context.integer(i64::from(shift)),
                context.zero(),
                context.zero(),
            ],
        )
        .unwrap(),
    )
}

fn key(powers: [i64; 3]) -> IntegralKey {
    IntegralKey::try_new(powers).unwrap()
}

fn map(family: &IntegralFamily, matrix: [i64; 4]) -> MomentumMap {
    MomentumMap::new(
        symmetry::CoefficientMatrix::try_new(
            2,
            2,
            matrix.map(|value| family.coefficient_context().integer(value)),
        )
        .unwrap(),
        symmetry::CoefficientMatrix::try_new(2, 0, []).unwrap(),
        symmetry::CoefficientMatrix::try_new(0, 0, []).unwrap(),
    )
}

#[test]
fn two_line_support_retains_mass_constants_and_exact_pinch_identity() {
    let family = family([1, 1, 1], false);
    let source = key([1, 1, -1]);
    let witness = TerminalNumeratorSymmetryWitness::verify(
        family.clone(),
        &source,
        map(&family, [-1, 0, 0, 1]),
        Default::default(),
    )
    .unwrap();
    let context = family.coefficient_context();
    assert_eq!(
        witness.equation().terms,
        BTreeMap::from([
            (source.clone(), context.integer(2)),
            (key([0, 1, 0]), context.integer(-2)),
            (key([1, 0, 0]), context.integer(-2)),
            (key([1, 1, 0]), context.integer(-2)),
        ])
    );
    assert!(matches!(
        witness.verified_map().jacobian(),
        Jacobian::Unit {
            determinant_sign: -1
        }
    ));
    assert_eq!(witness.family_fingerprint(), family.fingerprint());
    witness.replay(family.clone(), Default::default()).unwrap();
    let plan = TerminalNumeratorSymmetryEquations::prepare(
        family,
        &BTreeSet::from([source.clone()]),
        Default::default(),
    )
    .unwrap();
    assert!(plan.equations(&source).contains(witness.equation()));
    assert!(plan.statistics().verified_generators > 0);
    assert!(
        plan.witnesses(&source)
            .iter()
            .all(|proof| proof.source() == &source)
    );
}

#[test]
fn complete_quartic_numerator_is_transported_before_collection() {
    let family = family([1, 1, 1], false);
    let source = key([1, 1, -2]);
    let witness = TerminalNumeratorSymmetryWitness::verify(
        family.clone(),
        &source,
        map(&family, [-1, 0, 0, 1]),
        Default::default(),
    )
    .unwrap();
    let context = family.coefficient_context();
    let row = &witness.equation().terms;
    // D2 -> 2 D0 + 2 D1 - D2 + 2. Expanding the FULL square
    // includes cross terms and removes the equal D2^2 source only afterwards.
    assert!(!row.contains_key(&source));
    assert_eq!(row.len(), 9);
    assert_eq!(row[&key([1, 1, 0])], context.integer(-4));
    assert_eq!(row[&key([1, 1, -1])], context.integer(4));
    assert_eq!(row[&key([0, 0, 0])], context.integer(-8));
    assert_eq!(row[&key([0, 1, -1])], context.integer(4));
    witness.replay(family, Default::default()).unwrap();
}

#[test]
fn unequal_mass_and_unequal_power_swaps_are_rejected() {
    let unequal_mass = family([1, 2, 1], false);
    assert!(
        TerminalNumeratorSymmetryWitness::verify(
            unequal_mass.clone(),
            &key([1, 1, -1]),
            map(&unequal_mass, [0, 1, 1, 0]),
            Default::default(),
        )
        .is_err()
    );
    let equal_mass = family([1, 1, 1], false);
    assert!(
        TerminalNumeratorSymmetryWitness::verify(
            equal_mass.clone(),
            &key([1, 2, -1]),
            map(&equal_mass, [0, 1, 1, 0]),
            Default::default(),
        )
        .is_err()
    );
    // Dots on individually fixed lines are valid; rejecting one invalid swap
    // must not suppress the independent reflection symmetry.
    let proof = TerminalNumeratorSymmetryWitness::verify(
        equal_mass.clone(),
        &key([1, 2, -1]),
        map(&equal_mass, [-1, 0, 0, 1]),
        Default::default(),
    )
    .unwrap();
    proof
        .replay(equal_mass.clone(), Default::default())
        .unwrap();
    let plan = TerminalNumeratorSymmetryEquations::prepare(
        equal_mass,
        &BTreeSet::from([key([1, 2, -1])]),
        Default::default(),
    )
    .unwrap();
    assert!(plan.statistics().rejected_power_permutations > 0);
    assert!(plan.statistics().equations > 0);
}

#[test]
fn mass_constants_are_not_replaced_by_the_unit_mass_formula() {
    let family = family([2, 3, 7], false);
    let proof = TerminalNumeratorSymmetryWitness::verify(
        family.clone(),
        &key([1, 1, -1]),
        map(&family, [-1, 0, 0, 1]),
        Default::default(),
    )
    .unwrap();
    // D2' = 2D0+2D1-D2 + 2m0²+2m1²-2m2² = ... -4.
    assert_eq!(
        proof.equation().terms[&key([1, 1, 0])],
        family.coefficient_context().integer(4)
    );
}

#[test]
fn nonunit_jacobians_and_analytic_shifts_never_gain_authority() {
    let ordinary = family([1, 1, 1], false);
    assert!(
        TerminalNumeratorSymmetryWitness::verify(
            ordinary.clone(),
            &key([1, 1, -1]),
            map(&ordinary, [2, 0, 0, 1]),
            Default::default(),
        )
        .is_err()
    );
    let shifted = family([1, 1, 1], true);
    assert!(
        TerminalNumeratorSymmetryWitness::verify(
            shifted.clone(),
            &key([1, 1, -1]),
            map(&shifted, [-1, 0, 0, 1]),
            Default::default(),
        )
        .is_err()
    );
    let plan = TerminalNumeratorSymmetryEquations::prepare(
        shifted,
        &BTreeSet::from([key([1, 1, -1])]),
        Default::default(),
    )
    .unwrap();
    assert_eq!(plan.statistics().equations, 0);
    assert_eq!(
        plan.statistics().skipped[&TerminalNumeratorSymmetrySkipReason::AnalyticPowerShifts],
        1
    );
}

#[test]
fn unknown_keys_are_not_recursively_promoted_and_preparation_is_deterministic() {
    let family = family([1, 1, 1], false);
    let keys = BTreeSet::from([key([1, 1, -1]), key([1, 1, -2])]);
    let a = TerminalNumeratorSymmetryEquations::prepare(family.clone(), &keys, Default::default())
        .unwrap();
    let b = TerminalNumeratorSymmetryEquations::prepare(family.clone(), &keys, Default::default())
        .unwrap();
    assert_eq!(a.input_keys(), &keys);
    assert_eq!(a.statistics(), b.statistics());
    for key in &keys {
        assert_eq!(a.equations(key), b.equations(key));
        for witness in a.witnesses(key) {
            witness.replay(family.clone(), Default::default()).unwrap();
        }
    }
    assert!(a.equations(&key([0, 1, 0])).is_empty());
}
