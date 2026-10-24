use super::*;

pub(super) fn weights(result: &WeightedScalarNumeratorLowering) -> BTreeMap<Vec<i64>, Atom> {
    result
        .terms()
        .iter()
        .map(|term| {
            assert_eq!(term.common_mass_squared_power(), 0);
            (
                term.integral().powers().to_vec(),
                (term.coefficient().to_expression() * term.scalar_spectator()).together(),
            )
        })
        .collect()
}

#[test]
fn expanded_inverse_propagator_power_cancels_before_entry_admission() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let head = dot_head();
    let k = symbol("weighted_inverse_k").to_atom();
    let square = dot(head, k.clone(), k.clone());
    let service =
        FamilyScalarNumeratorService::try_new(artifact.family(), head, vec![k], Default::default())
            .unwrap();
    let base = IntegralKey::try_new([2]).unwrap();
    for mass in [Atom::one(), Atom::num(3), symbol("weighted_m2").to_atom()] {
        let numerator = (&square - &mass).pow(5).expand();
        let unweighted = service.lower(&numerator, &base).unwrap();
        assert!(unweighted.terms().len() > 1);
        let actual = service.lower_weighted(&numerator, &base, &mass).unwrap();
        assert_eq!(weights(&actual), BTreeMap::from([(vec![-3], Atom::one())]));
        assert_eq!(
            actual.terms()[0].coefficient(),
            &artifact.coefficient_context().one()
        );
        assert_eq!(actual.terms()[0].scalar_spectator(), &Atom::one());
        if !matches!(mass.as_view(), symbolica::atom::AtomView::Num(_)) {
            assert!(actual.nonzero_conditions().contains(&mass));
        }
        // An entry-only consumer need admit just the surviving shifted key;
        // the spurious intermediate powers have already cancelled exactly.
        assert!(
            actual
                .terms()
                .iter()
                .all(|term| term.integral().powers() == [-3])
        );
    }
}

#[test]
fn homogeneous_difference_keeps_only_the_degree_five_denominator_keys() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let head = dot_head();
    let k1 = symbol("weighted_difference_k1").to_atom();
    let k2 = symbol("weighted_difference_k2").to_atom();
    let numerator = (dot(head, k1.clone(), k1.clone()) - dot(head, k2.clone(), k2.clone()))
        .pow(5)
        .expand();
    let service = FamilyScalarNumeratorService::try_new(
        artifact.family(),
        head,
        vec![k1, k2],
        Default::default(),
    )
    .unwrap();
    let mass = symbol("weighted_difference_m2").to_atom();
    let actual = service
        .lower_weighted(&numerator, &IntegralKey::try_new([1, 1, 1]).unwrap(), &mass)
        .unwrap();
    let expected = [1, -5, 10, -10, 5, -1]
        .into_iter()
        .enumerate()
        .map(|(j, coefficient)| (vec![-4 + j as i64, 1 - j as i64, 1], Atom::num(coefficient)))
        .collect();
    assert_eq!(weights(&actual), expected);
    assert!(
        actual
            .terms()
            .iter()
            .all(|term| term.scalar_spectator() == &Atom::one())
    );
    assert!(
        actual
            .terms()
            .iter()
            .all(|term| term.integral().powers().iter().sum::<i64>() == -2)
    );
}

#[test]
fn weighted_lowering_preserves_distinct_physical_mass_contributions() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let head = dot_head();
    let k = symbol("weighted_mass_k").to_atom();
    let mass = symbol("weighted_mass_m2").to_atom();
    let denominator = dot(head, k.clone(), k.clone()) - &mass;
    let numerator = (denominator.clone().pow(2)
        + Atom::num(3) * &mass * &denominator
        + Atom::num(5) * mass.clone().pow(2))
    .expand();
    let service =
        FamilyScalarNumeratorService::try_new(artifact.family(), head, vec![k], Default::default())
            .unwrap();
    let actual = service
        .lower_weighted(&numerator, &IntegralKey::try_new([3]).unwrap(), &mass)
        .unwrap();
    assert_eq!(
        weights(&actual),
        BTreeMap::from([
            (vec![1], Atom::one()),
            (vec![2], Atom::num(3) * &mass),
            (vec![3], Atom::num(5) * mass.pow(2)),
        ])
    );
}

#[test]
fn weighted_lowering_extracts_base_d_and_numeric_signs_but_keeps_spectators() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let c = artifact.coefficient_context();
    let head = dot_head();
    let k = symbol("weighted_spectator_k").to_atom();
    let spectator = symbol("weighted_spectator_function").call(symbol("weighted_external_x"));
    let d = c.parameter("d").unwrap().to_expression();
    let inverse = dot(head, k.clone(), k.clone()) - Atom::one();
    let numerator = (-(d + Atom::one()) * &spectator * inverse.pow(3)).expand();
    let service =
        FamilyScalarNumeratorService::try_new(artifact.family(), head, vec![k], Default::default())
            .unwrap();
    let actual = service
        .lower_weighted(
            &numerator,
            &IntegralKey::try_new([2]).unwrap(),
            &Atom::one(),
        )
        .unwrap();
    assert_eq!(actual.terms().len(), 1);
    let term = &actual.terms()[0];
    let expected = c
        .try_neg(
            &c.try_add(&c.parameter("d").unwrap(), &c.one(), Default::default())
                .unwrap(),
            Default::default(),
        )
        .unwrap();
    assert_eq!(term.coefficient(), &expected);
    assert_eq!(term.scalar_spectator(), &spectator);
    assert!(c.contains(term.coefficient()));
    assert_eq!(term.integral().powers(), [-1]);
}

#[test]
fn weighted_zero_keeps_original_parameter_and_spectator_poles() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let head = dot_head();
    let k = symbol("weighted_zero_k").to_atom();
    let service =
        FamilyScalarNumeratorService::try_new(artifact.family(), head, vec![k], Default::default())
            .unwrap();
    let d = artifact
        .coefficient_context()
        .parameter("d")
        .unwrap()
        .to_expression();
    let external = symbol("weighted_zero_external").to_atom();
    for variable in [d, external] {
        let pole = &variable - Atom::one();
        let numerator = (variable.clone().pow(2) - Atom::one()) / &pole - &variable - Atom::one();
        let actual = service
            .lower_weighted(
                &numerator,
                &IntegralKey::try_new([1]).unwrap(),
                &Atom::one(),
            )
            .unwrap();
        assert!(actual.is_zero());
        assert!(actual.nonzero_conditions().contains(&pole));
    }
    let zero = service
        .lower_weighted(
            &Atom::Zero,
            &IntegralKey::try_new([1]).unwrap(),
            &Atom::one(),
        )
        .unwrap();
    assert!(zero.is_zero());
}

#[test]
fn weighted_mass_and_collection_resources_fail_closed() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let head = dot_head();
    let k = symbol("weighted_limits_k").to_atom();
    let square = dot(head, k.clone(), k.clone());
    let base = IntegralKey::try_new([2]).unwrap();
    let service = FamilyScalarNumeratorService::try_new(
        artifact.family(),
        head,
        vec![k.clone()],
        Default::default(),
    )
    .unwrap();
    assert!(matches!(
        service.lower_weighted(&square, &base, &Atom::Zero),
        Err(ScalarNumeratorError::InvalidWeightedInput { .. })
    ));
    assert!(matches!(
        service.lower_weighted(&square, &base, &square),
        Err(ScalarNumeratorError::InvalidWeightedInput { .. })
    ));
    let bounded = FamilyScalarNumeratorService::try_new(
        artifact.family(),
        head,
        vec![k.clone()],
        ScalarNumeratorLimits {
            diagnostics_enabled: true,
            max_polynomial_terms: 32,
            ..Default::default()
        },
    )
    .unwrap();
    let scalar =
        (symbol("weighted_limits_x").to_atom() + symbol("weighted_limits_y").to_atom()).pow(1000);
    assert!(matches!(
        bounded.lower_weighted(&(scalar * &square), &base, &Atom::one()),
        Err(ScalarNumeratorError::ResourceLimit { .. })
            | Err(ScalarNumeratorError::ResourceCountOverflow { .. })
    ));
    let mut limits = ScalarNumeratorLimits::default().with_diagnostics();
    // Whole-expression grammar/change-of-basis admission now shares this
    // request budget with collection (7 prior operations plus the 3-node
    // translated dot visited by native numeric distribution).
    limits.exact_algebra.max_term_operations = 10;
    let shared =
        FamilyScalarNumeratorService::try_new(artifact.family(), head, vec![k], limits).unwrap();
    assert!(
        shared
            .lower_weighted(&(&square - Atom::one()), &base, &Atom::one())
            .is_ok()
    );
    assert!(matches!(
        shared.lower_weighted(&square, &base, &Atom::one()),
        Err(ScalarNumeratorError::ResourceLimit {
            resource: "weighted collection work",
            ..
        })
    ));
}
