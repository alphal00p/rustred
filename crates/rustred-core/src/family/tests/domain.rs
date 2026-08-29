use super::*;

#[test]
fn equal_family_input_denominators_merge_all_sources() {
    let context = CoefficientContext::new(["d", "m", "a", "nu", "s"]);
    let family = IntegralFamily::new(
        "merged-input-denominators",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.coefficient_fixture("d/s"),
        vec![AffineDenominator::new(
            context.coefficient_fixture("m/s"),
            vec![context.coefficient_fixture("a/s")],
        )],
        Vec::new(),
        vec![context.coefficient_fixture("nu/s")],
    )
    .unwrap();

    assert_eq!(family.domain().conditions().count(), 2);
    let condition = family
        .domain()
        .conditions()
        .find(|condition| condition.polynomial() == &context.parameter("s").unwrap().numerator)
        .unwrap();
    assert_eq!(
        condition.polynomial(),
        &context.parameter("s").unwrap().numerator
    );
    let expected_sources = BTreeSet::from([
        CoefficientLocation::Dimension,
        CoefficientLocation::DenominatorConstant { denominator: 0 },
        CoefficientLocation::DenominatorCoefficient {
            denominator: 0,
            coordinate: 0,
        },
        CoefficientLocation::PowerShift { denominator: 0 },
    ]);
    assert_eq!(condition.sources(), &expected_sources);
}

#[test]
fn determinant_guard_merges_into_one_canonical_condition_at_the_domain_tail() {
    let context = CoefficientContext::new(["d", "s", "a"]);
    let family = IntegralFamily::new(
        "merged-determinant-guard",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.coefficient_fixture("d/s"),
        vec![AffineDenominator::new(
            context.zero(),
            vec![context.coefficient_fixture("s/a")],
        )],
        Vec::new(),
        vec![context.zero()],
    )
    .unwrap();

    let conditions = family.domain().conditions().collect::<Vec<_>>();
    assert_eq!(conditions.len(), 2);
    assert_eq!(
        conditions[0].polynomial(),
        &context.parameter("a").unwrap().numerator
    );
    assert_eq!(
        conditions[1].polynomial(),
        &context.parameter("s").unwrap().numerator
    );
    assert_eq!(
        conditions[1].sources(),
        &BTreeSet::from([
            CoefficientLocation::Dimension,
            CoefficientLocation::BasisDeterminantNumerator,
        ])
    );
}

#[test]
fn external_derivative_contractions_include_gram_constants() {
    let context = CoefficientContext::new(["d", "m2", "c", "s", "nu"]);
    let m2 = context.parameter("m2").unwrap();
    let c = context.parameter("c").unwrap();
    let s = context.parameter("s").unwrap();
    let family = IntegralFamily::new(
        "one-loop-one-leg",
        vec!["k".into()],
        vec!["p".into()],
        context.clone(),
        context.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(m2.clone(), vec![context.one(), context.zero()]),
            AffineDenominator::new(c.clone(), vec![context.zero(), context.one()]),
        ],
        vec![vec![s.clone()]],
        vec![context.parameter("nu").unwrap(), context.zero()],
    )
    .unwrap();

    let k_d0 = family
        .derivative_contraction(0, 0, ContractionMomentum::Loop(0))
        .unwrap();
    assert_eq!(k_d0.constant(), &(-(&context.integer(2) * &m2)));
    assert_eq!(
        k_d0.denominator_coefficients(),
        &[context.integer(2), context.zero()]
    );

    let p_d0 = family
        .derivative_contraction(0, 0, ContractionMomentum::External(0))
        .unwrap();
    assert_eq!(p_d0.constant(), &(-(&context.integer(2) * &c)));
    assert_eq!(
        p_d0.denominator_coefficients(),
        &[context.zero(), context.integer(2)]
    );

    let p_d1 = family
        .derivative_contraction(1, 0, ContractionMomentum::External(0))
        .unwrap();
    assert_eq!(p_d1.constant(), &s);
    assert_eq!(
        p_d1.denominator_coefficients(),
        &[context.zero(), context.zero()]
    );
    family.verify_exact_replay().unwrap();
}
