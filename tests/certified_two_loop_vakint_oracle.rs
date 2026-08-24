use std::collections::BTreeMap;

use rustred::{
    AdaptiveParametricRuleProvider, AdaptiveRuleSearchLimits, AffineDenominator,
    CertifiedFamilyRuleProvider, CertifiedFamilyRuleProviderError,
    CertifiedFamilyRuleProviderLimits, Coefficient, CoefficientContext, ConcreteIntegralKey,
    ConcreteRuleApplicationTrace, ConcreteRuleProvider, IntegralFamily, IntegralOrderingPolicy,
    InternalSymmetrySearchLimits, MasterPolicyProvider, ParametricCoefficientContext,
    ParametricIbpConfig, ParametricIbpGenerator, ParametricReductionEngine, ParametricRelation,
    ReductionEngineLimits, SectorRestrictions, discover_bounded_vacuum_internal_symmetries,
};

fn vakint_equal_mass_two_loop_family() -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let zero = coefficients.zero();
    let one = coefficients.one();
    let minus_m2 = coefficients.parse("-m2").unwrap();
    IntegralFamily::new(
        "vakint-equal-mass-two-loop-vacuum-parametric-oracle",
        vec!["k1".into(), "k2".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                minus_m2.clone(),
                vec![one.clone(), zero.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                minus_m2.clone(),
                vec![zero.clone(), zero.clone(), one.clone()],
            ),
            AffineDenominator::new(minus_m2, vec![one.clone(), coefficients.integer(2), one]),
        ],
        Vec::new(),
        vec![zero.clone(), zero.clone(), zero],
    )
    .unwrap()
}

#[test]
fn certified_provider_rejects_fabricated_same_family_ibp_row() {
    let family = vakint_equal_mass_two_loop_family();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let mut rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    rows[0] = ParametricRelation::new(
        family.fingerprint(),
        rows[0].row_id().clone(),
        generated.context(),
    );
    let adaptive = AdaptiveParametricRuleProvider::try_new(
        generated.context(),
        &rows,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        AdaptiveRuleSearchLimits::default(),
    )
    .unwrap();
    let error = match CertifiedFamilyRuleProvider::try_new(
        family,
        SectorRestrictions::unrestricted(3).unwrap(),
        [],
        adaptive,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        CertifiedFamilyRuleProviderLimits::default(),
    ) {
        Ok(_) => panic!("fabricated same-family row was accepted"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        CertifiedFamilyRuleProviderError::UnauthenticatedSourceRows { row: Some(0) }
    ));
}

#[test]
fn certified_provider_enforces_concrete_row_and_term_bounds_before_retention() {
    let family = vakint_equal_mass_two_loop_family();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();

    let adaptive = AdaptiveParametricRuleProvider::try_new(
        generated.context(),
        &rows,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        AdaptiveRuleSearchLimits::default(),
    )
    .unwrap();
    let mut limits = CertifiedFamilyRuleProviderLimits::default();
    limits.rewrite.concrete_elimination.max_rows = 0;
    let mut provider = CertifiedFamilyRuleProvider::try_new(
        family.clone(),
        SectorRestrictions::unrestricted(3).unwrap(),
        [],
        adaptive,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        limits,
    )
    .unwrap();
    assert!(matches!(
        provider.decision_for(&key([2, 1, 1])),
        Err(CertifiedFamilyRuleProviderError::ResourceLimit {
            resource: "concrete quotient source rows",
            requested: 1,
            limit: 0,
        })
    ));

    let adaptive = AdaptiveParametricRuleProvider::try_new(
        generated.context(),
        &rows,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        AdaptiveRuleSearchLimits::default(),
    )
    .unwrap();
    let mut limits = CertifiedFamilyRuleProviderLimits::default();
    limits.rewrite.max_quotient_terms = 0;
    let mut provider = CertifiedFamilyRuleProvider::try_new(
        family,
        SectorRestrictions::unrestricted(3).unwrap(),
        [],
        adaptive,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        limits,
    )
    .unwrap();
    assert!(matches!(
        provider.decision_for(&key([2, 1, 1])),
        Err(CertifiedFamilyRuleProviderError::ResourceLimit {
            resource: "concrete quotient terms",
            requested,
            limit: 0,
        }) if requested > 0
    ));
}

fn key(powers: [i64; 3]) -> ConcreteIntegralKey {
    ConcreteIntegralKey::try_new(powers).unwrap()
}

fn expected(
    context: &CoefficientContext,
    master: [i64; 3],
    coefficient: &str,
) -> BTreeMap<ConcreteIntegralKey, Coefficient> {
    BTreeMap::from([(key(master), context.parse(coefficient).unwrap())])
}

#[test]
fn generated_ibps_zero_sectors_and_discovered_symmetries_match_vakint_two_loop_oracle() {
    let family = vakint_equal_mass_two_loop_family();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    assert_eq!(rows.len(), 4, "L(L+E)=4 ordinary generated IBPs");

    let restrictions = SectorRestrictions::unrestricted(3).unwrap();
    let symmetry_report = discover_bounded_vacuum_internal_symmetries(
        &family,
        &restrictions,
        InternalSymmetrySearchLimits::default(),
    )
    .unwrap();
    assert!(symmetry_report.completion().is_exhaustive_within_bounds());
    assert_eq!(symmetry_report.symmetries().len(), 6);

    let mut adaptive_limits = AdaptiveRuleSearchLimits::default();
    adaptive_limits.max_search_depth = 1;
    let adaptive = AdaptiveParametricRuleProvider::try_new(
        generated.context(),
        &rows,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        adaptive_limits,
    )
    .unwrap();
    let provider = CertifiedFamilyRuleProvider::try_new(
        family.clone(),
        restrictions,
        symmetry_report.symmetries().iter().cloned(),
        adaptive,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        CertifiedFamilyRuleProviderLimits::default(),
    )
    .unwrap();
    let provider =
        MasterPolicyProvider::with_selected(provider, [key([1, 1, 1]), key([0, 1, 1])]).unwrap();
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );

    // Frozen independent oracle: Vakint's alphaLoop FORM implementation.
    // These equations are fixtures only; no production recurrence is copied.
    for (source, master, coefficient) in [
        ([0, 2, 1], [0, 1, 1], "(d-2)/(2*m2)"),
        ([0, 2, 2], [0, 1, 1], "(d-2)^2/(4*m2^2)"),
        ([-1, 1, 1], [0, 1, 1], "m2"),
        ([-2, 1, 1], [0, 1, 1], "m2^2*(1+4/d)"),
        ([2, 1, 1], [1, 1, 1], "(d-3)/(3*m2)"),
    ] {
        let result = engine.reduce(&key(source)).unwrap();
        result.require_complete().unwrap();
        assert_eq!(
            result.terms(),
            &expected(family.coefficient_context(), master, coefficient),
            "wrong generated reduction for J{source:?}"
        );
        assert!(!result.application_traces().is_empty());
        for trace in result.application_traces() {
            match trace {
                ConcreteRuleApplicationTrace::Parametric(proof) => assert!(
                    proof
                        .replay_application(&family, generated.context())
                        .unwrap()
                ),
                ConcreteRuleApplicationTrace::ConditionalParametric(proof) => {
                    proof.replay(&family, proof.parametric_context()).unwrap()
                }
                ConcreteRuleApplicationTrace::CertifiedRewrite(proof) => proof
                    .replay(
                        &family,
                        generated.context(),
                        IntegralOrderingPolicy::RustRedUnshiftedV1,
                    )
                    .unwrap(),
                ConcreteRuleApplicationTrace::ProvedZero(proof) => proof.replay(&family).unwrap(),
            }
        }
    }

    let zero = engine.reduce(&key([0, 0, 1])).unwrap();
    assert!(zero.terms().is_empty());
    assert!(matches!(
        zero.application_traces(),
        [ConcreteRuleApplicationTrace::ProvedZero(_)]
    ));

    // The results own their proofs. In particular this analytic zero-sector
    // certificate remains replayable after the provider and engine are gone.
    drop(engine);
    match &zero.application_traces()[0] {
        ConcreteRuleApplicationTrace::ProvedZero(proof) => proof.replay(&family).unwrap(),
        _ => unreachable!(),
    }
}

#[test]
fn certified_numeric_provider_replays_with_a_nondefault_parametric_context() {
    let family = vakint_equal_mass_two_loop_family();
    let parametric_context = ParametricCoefficientContext::try_new(
        family.coefficient_context(),
        "two-loop-custom-numeric-provider",
        family.denominator_count(),
    )
    .unwrap();
    let generated = ParametricIbpGenerator::try_with_context(
        &family,
        parametric_context.clone(),
        ParametricIbpConfig::default(),
    )
    .unwrap()
    .generate()
    .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let restrictions = SectorRestrictions::unrestricted(3).unwrap();
    let symmetries = discover_bounded_vacuum_internal_symmetries(
        &family,
        &restrictions,
        InternalSymmetrySearchLimits::default(),
    )
    .unwrap();
    let mut adaptive_limits = AdaptiveRuleSearchLimits::default();
    adaptive_limits.max_search_depth = 1;
    let adaptive = AdaptiveParametricRuleProvider::try_new(
        generated.context(),
        &rows,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        adaptive_limits,
    )
    .unwrap();
    let provider = CertifiedFamilyRuleProvider::try_new(
        family.clone(),
        restrictions,
        symmetries.symmetries().iter().cloned(),
        adaptive,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        CertifiedFamilyRuleProviderLimits::default(),
    )
    .unwrap();
    let provider =
        MasterPolicyProvider::with_selected(provider, [key([1, 1, 1]), key([0, 1, 1])]).unwrap();
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );
    let result = engine.reduce(&key([2, 1, 1])).unwrap();
    result.require_complete().unwrap();
    let retained = result
        .application_traces()
        .iter()
        .find_map(|trace| match trace {
            ConcreteRuleApplicationTrace::CertifiedRewrite(rewrite)
                if rewrite.parametric_context().is_some() =>
            {
                Some(rewrite.clone())
            }
            _ => None,
        })
        .expect("numeric provider must retain a generated concrete rewrite");
    assert_eq!(
        retained.parametric_context().unwrap().fingerprint(),
        parametric_context.fingerprint()
    );
    drop(engine);
    retained
        .replay(
            &family,
            &parametric_context,
            IntegralOrderingPolicy::RustRedUnshiftedV1,
        )
        .unwrap();
}
