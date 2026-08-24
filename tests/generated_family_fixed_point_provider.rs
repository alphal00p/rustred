//! Black-box application of the generic residual fixed-point provider.
//!
//! The topology definitions and expected coefficients below are test fixtures
//! and frozen Vakint oracles.  Production construction receives only an
//! `IntegralFamily`, generic policies, and resource limits: no loop count,
//! topology name, recurrence, or expected normal form enters rule generation.

use rustred::{
    AffineDenominator, CertifiedSymmetryCanonicalizingRuleProviderError,
    CertifiedZeroSectorRuleProviderError, CoefficientContext, ConcreteIntegralKey,
    ConcreteRuleDecision, ConcreteRuleProvider, ConcreteTerminalStatus,
    GeneratedFamilyFixedPointCompiler, GeneratedFamilyFixedPointConfig,
    GeneratedFamilyFixedPointLimits, GeneratedFamilyFixedPointProvider,
    GeneratedFamilyFixedPointProviderError, GeneratedFamilyFixedPointProviderLimits,
    GeneratedFamilyFixedPointSelectionPolicy, GeneratedFamilyRuleSystemCompiler,
    GeneratedFamilyRuleSystemConfig, GeneratedFamilyRuleSystemLimits,
    GeneratedSectorConditionalRuleProviderError, GeneratedSymbolicRowSpanStrategy, IntegralFamily,
    IntegralOrderingPolicy, InternalSymmetrySearchLimits, MasterPolicyError,
    ParametricIbpGenerator, ParametricReductionEngine, ParametricSectorRuleProviderError,
    PowerShiftPolicy, ReductionEngineError, ReductionEngineLimits, SectorRestrictions,
};

fn key(powers: impl IntoIterator<Item = i64>) -> ConcreteIntegralKey {
    ConcreteIntegralKey::try_new(powers).unwrap()
}

fn massive_tadpole(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        name,
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            coefficients.parse("-m2").unwrap(),
            vec![coefficients.one()],
        )],
        Vec::new(),
        vec![coefficients.zero()],
    )
    .unwrap()
}

/// Connected equal-mass sunset with `D3 = (k0+k1)^2-m2`.
fn equal_mass_sunset(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let zero = coefficients.zero();
    let one = coefficients.one();
    let mass = coefficients.parse("-m2").unwrap();
    IntegralFamily::new(
        name,
        vec!["k0".into(), "k1".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(mass.clone(), vec![one.clone(), zero.clone(), zero.clone()]),
            AffineDenominator::new(mass.clone(), vec![zero.clone(), zero.clone(), one.clone()]),
            AffineDenominator::new(mass, vec![one.clone(), coefficients.integer(2), one]),
        ],
        Vec::new(),
        vec![zero.clone(), zero.clone(), zero],
    )
    .unwrap()
}

fn fixed_point_certificate(
    family: &IntegralFamily,
    row_span_strategy: GeneratedSymbolicRowSpanStrategy,
    residual_anchor_local_depth: usize,
) -> (
    rustred::ParametricCoefficientContext,
    rustred::GeneratedFamilyFixedPointCertificate,
) {
    let context = ParametricIbpGenerator::try_new(family)
        .unwrap()
        .context()
        .clone();
    let mut base_limits = GeneratedFamilyRuleSystemLimits::default();
    base_limits.discovery.adaptive.max_search_depth = 0;
    base_limits.live_leaf_queue.translation_radius = 0;
    base_limits.live_leaf_queue.max_translation_points = 1;
    base_limits
        .discovery
        .coverage
        .generated_when_bad
        .row_span
        .strategy = row_span_strategy;
    let base = GeneratedFamilyRuleSystemCompiler::compile(
        family,
        &context,
        SectorRestrictions::unrestricted(family.denominator_count()).unwrap(),
        PowerShiftPolicy::FormalGeneric,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        GeneratedFamilyRuleSystemConfig::default(),
        base_limits,
    )
    .unwrap();
    let certificate = GeneratedFamilyFixedPointCompiler::compile(
        family,
        &context,
        base,
        GeneratedFamilyFixedPointConfig {
            base_search_depth: 0,
            maximum_rounds: 1,
            residual_frontier_depth: 1,
            residual_anchor_local_depth,
            maximum_local_depth: 1,
            selection: GeneratedFamilyFixedPointSelectionPolicy::ResidualSubsectorFirstPrefix {
                max_selected_sectors: 1,
            },
            stop_on_no_strict_improvement: false,
        },
        GeneratedFamilyFixedPointLimits::default(),
    )
    .unwrap();
    (context, certificate)
}

#[test]
fn one_loop_fixed_point_provider_matches_vakint_scalar_normal_forms() {
    let family = massive_tadpole("fixed-point-provider-one-loop-vakint");
    let (context, certificate) =
        fixed_point_certificate(&family, GeneratedSymbolicRowSpanStrategy::Disabled, 0);
    let provider = GeneratedFamilyFixedPointProvider::try_with_selected(
        &family,
        &context,
        certificate,
        [key([1])],
        GeneratedFamilyFixedPointProviderLimits::default(),
    )
    .unwrap();
    provider.replay().unwrap();
    assert_eq!(provider.terminals().len(), 1);
    assert_eq!(provider.build_stats().master_terminals(), 1);

    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );
    for (power, expected) in [
        (1, "1"),
        (2, "(d-2)/(2*m2)"),
        (3, "(d-4)*(d-2)/(8*m2^2)"),
        (4, "(d-6)*(d-4)*(d-2)/(48*m2^3)"),
    ] {
        let result = engine.reduce(&key([power])).unwrap();
        result.require_complete().unwrap();
        assert_eq!(result.terms().len(), 1);
        assert_eq!(
            result.terms().get(&key([1])).unwrap(),
            &family.coefficient_context().parse(expected).unwrap(),
            "generated one-loop fixed-point reduction differs from the frozen Vakint oracle at power {power}",
        );
    }
}

#[test]
fn fixed_point_provider_never_infers_an_unselected_master_and_preflights_materials() {
    let family = massive_tadpole("fixed-point-provider-no-inferred-master");
    let (context, certificate) =
        fixed_point_certificate(&family, GeneratedSymbolicRowSpanStrategy::Disabled, 0);

    let mut zero_material_limit = GeneratedFamilyFixedPointProviderLimits::default();
    zero_material_limit.max_retained_generated_sectors = 0;
    let limited = GeneratedFamilyFixedPointProvider::try_new(
        &family,
        &context,
        certificate.clone(),
        zero_material_limit,
    );
    assert!(matches!(
        limited,
        Err(GeneratedFamilyFixedPointProviderError::ResourceLimit {
            resource: "fixed-point provider generated sectors",
            requested: 1,
            limit: 0,
        })
    ));

    let mut provider = GeneratedFamilyFixedPointProvider::try_new(
        &family,
        &context,
        certificate,
        GeneratedFamilyFixedPointProviderLimits::default(),
    )
    .unwrap();
    provider.replay().unwrap();
    assert!(provider.terminals().is_empty());
    assert!(matches!(
        provider.decision_for(&key([1])).unwrap(),
        ConcreteRuleDecision::Terminal(ConcreteTerminalStatus::Uncovered)
    ));
}

#[test]
fn sunset_fixed_point_provider_closes_numerator_and_reports_exact_dot_blocker() {
    let family = equal_mass_sunset("fixed-point-provider-connected-sunset-vakint");
    let (context, certificate) = fixed_point_certificate(
        &family,
        GeneratedSymbolicRowSpanStrategy::BoundedVacuumInternal {
            search: InternalSymmetrySearchLimits::default(),
            require_exhaustive: true,
        },
        1,
    );
    assert_eq!(
        certificate
            .base()
            .row_span_arc()
            .unwrap()
            .symmetries()
            .len(),
        6
    );
    let provider = GeneratedFamilyFixedPointProvider::try_with_selected(
        &family,
        &context,
        certificate,
        [key([1, 1, 1]), key([0, 1, 1])],
        GeneratedFamilyFixedPointProviderLimits::default(),
    )
    .unwrap();
    provider.replay().unwrap();
    assert_eq!(provider.terminals().len(), 2);

    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );
    let numerator = engine.reduce(&key([-1, 1, 1])).unwrap();
    numerator.require_complete().unwrap();
    assert_eq!(numerator.terms().len(), 1);
    assert_eq!(
        numerator.terms().get(&key([0, 1, 1])).unwrap(),
        &family.coefficient_context().parse("m2").unwrap(),
        "generic fixed-point numerator reduction differs from the frozen Vakint oracle",
    );

    // This bounded one-round certificate does derive the top-sector first
    // step for J(2,1,1), but that rule reaches J(0,1,2).  The residual search
    // inspected all 24 candidates through local depth one at that request and
    // selected none.  Preserve the exact authenticated blocker instead of
    // weakening it to `Uncovered`, inferring a master, or claiming the Vakint
    // dot oracle before symbolic-start/persistent elimination closes it.
    let dot_error = engine.reduce(&key([2, 1, 1])).unwrap_err();
    assert!(matches!(
        dot_error,
        ReductionEngineError::Provider(GeneratedFamilyFixedPointProviderError::Provider(
            CertifiedZeroSectorRuleProviderError::Inner(
                CertifiedSymmetryCanonicalizingRuleProviderError::Inner(
                    MasterPolicyError::Inner(
                        GeneratedSectorConditionalRuleProviderError::Inner(
                            ParametricSectorRuleProviderError::UnsupportedLeaf {
                                sector,
                                candidate_ordinals,
                            },
                        ),
                    ),
                ),
            ),
        )) if sector.to_bit_string() == "011" && candidate_ordinals.as_ref() == [0, 2]
    ));
}
