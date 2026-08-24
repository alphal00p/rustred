//! Selective, topology-independent `SolvejSector`-style depth growth.
//!
//! The sunset is a concrete oracle only.  Production receives no topology
//! label, recurrence, or master count.

use std::sync::Arc;

use rustred::{
    AffineDenominator, CoefficientContext, ConcreteIntegralKey, GeneratedFamilyDepthGrowthCompiler,
    GeneratedFamilyDepthGrowthConfig, GeneratedFamilyDepthGrowthError,
    GeneratedFamilyDepthGrowthFinalStatus, GeneratedFamilyDepthGrowthLimits,
    GeneratedFamilyDepthGrowthProvider, GeneratedFamilyDepthGrowthProviderError,
    GeneratedFamilyDepthGrowthSelectionPolicy, GeneratedFamilyDepthGrowthStage,
    GeneratedFamilyRuleSystemCompiler, GeneratedFamilyRuleSystemConfig,
    GeneratedFamilyRuleSystemLimits, IntegralFamily, IntegralOrderingPolicy,
    ParametricIbpGenerator, ParametricReductionEngine, PowerShiftPolicy, ReductionEngineLimits,
    SectorRestrictions,
};

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

fn key(powers: impl IntoIterator<Item = i64>) -> ConcreteIntegralKey {
    ConcreteIntegralKey::try_new(powers).unwrap()
}

fn compile_base(
    family: &IntegralFamily,
) -> (
    rustred::ParametricCoefficientContext,
    rustred::GeneratedFamilyRuleSystemCertificate,
) {
    compile_base_with_limits(family, GeneratedFamilyRuleSystemLimits::default())
}

fn compile_base_with_limits(
    family: &IntegralFamily,
    mut limits: GeneratedFamilyRuleSystemLimits,
) -> (
    rustred::ParametricCoefficientContext,
    rustred::GeneratedFamilyRuleSystemCertificate,
) {
    let context = ParametricIbpGenerator::try_new(family)
        .unwrap()
        .context()
        .clone();
    limits.discovery.adaptive.max_search_depth = 0;
    limits.live_leaf_queue.translation_radius = 0;
    limits.live_leaf_queue.max_translation_points = 1;
    let certificate = GeneratedFamilyRuleSystemCompiler::compile(
        family,
        &context,
        SectorRestrictions::unrestricted(family.denominator_count()).unwrap(),
        PowerShiftPolicy::FormalGeneric,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        GeneratedFamilyRuleSystemConfig::default(),
        limits,
    )
    .unwrap();
    (context, certificate)
}

fn no_growth_config() -> GeneratedFamilyDepthGrowthConfig {
    GeneratedFamilyDepthGrowthConfig {
        initial_depth: 0,
        maximum_depth: 0,
        selection: GeneratedFamilyDepthGrowthSelectionPolicy::AllResidualSubsectorFirst,
        stop_on_no_strict_improvement: false,
    }
}

#[test]
fn no_growth_is_replayable_and_never_infers_a_master() {
    let family = massive_tadpole("depth-growth-one-loop-no-growth");
    let (context, base) = compile_base(&family);
    let certificate = GeneratedFamilyDepthGrowthCompiler::compile(
        &family,
        &context,
        base,
        no_growth_config(),
        GeneratedFamilyDepthGrowthLimits::default(),
    )
    .unwrap();
    certificate.replay(&family, &context).unwrap();
    assert!(certificate.rounds().is_empty());
    let active = rustred::SectorMask::try_new([true]).unwrap();
    assert!(matches!(
        certificate.final_status(&active),
        Some(GeneratedFamilyDepthGrowthFinalStatus::CoveredByGeneratedRules { depth: 0 })
            | Some(GeneratedFamilyDepthGrowthFinalStatus::ExhaustedAtMaxDepth {
                latest_successful_depth: 0,
                ..
            })
    ));
    let provider = GeneratedFamilyDepthGrowthProvider::try_new(
        &family,
        &context,
        certificate,
        rustred::GeneratedFamilyRuleSystemProviderLimits::default(),
    )
    .unwrap();
    assert!(provider.terminals().is_empty());
}

#[test]
fn policies_and_aggregate_residual_limits_fail_closed_before_depth_work() {
    let family = massive_tadpole("depth-growth-policy-limits");
    let (context, base) = compile_base(&family);

    let error = GeneratedFamilyDepthGrowthCompiler::compile(
        &family,
        &context,
        base.clone(),
        GeneratedFamilyDepthGrowthConfig {
            initial_depth: 1,
            maximum_depth: 1,
            selection: GeneratedFamilyDepthGrowthSelectionPolicy::AllResidualSubsectorFirst,
            stop_on_no_strict_improvement: false,
        },
        GeneratedFamilyDepthGrowthLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        GeneratedFamilyDepthGrowthError::InitialDepthMismatch {
            expected: 1,
            actual: 0,
            ..
        }
    ));

    let error = GeneratedFamilyDepthGrowthCompiler::compile(
        &family,
        &context,
        base.clone(),
        GeneratedFamilyDepthGrowthConfig {
            initial_depth: 0,
            maximum_depth: 1,
            selection: GeneratedFamilyDepthGrowthSelectionPolicy::ResidualSubsectorFirstPrefix {
                max_sectors_per_round: 0,
            },
            stop_on_no_strict_improvement: false,
        },
        GeneratedFamilyDepthGrowthLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        GeneratedFamilyDepthGrowthError::InvalidConfig { .. }
    ));

    let mut limits = GeneratedFamilyDepthGrowthLimits::default();
    limits.max_rounds = 0;
    let error = GeneratedFamilyDepthGrowthCompiler::compile(
        &family,
        &context,
        base.clone(),
        GeneratedFamilyDepthGrowthConfig {
            maximum_depth: 1,
            ..no_growth_config()
        },
        limits,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        GeneratedFamilyDepthGrowthError::ResourceLimit {
            resource: "depth-growth rounds",
            requested: 1,
            limit: 0,
        }
    ));

    let mut limits = GeneratedFamilyDepthGrowthLimits::default();
    limits.max_sector_attempts = 0;
    let error = GeneratedFamilyDepthGrowthCompiler::compile(
        &family,
        &context,
        base.clone(),
        GeneratedFamilyDepthGrowthConfig {
            maximum_depth: 1,
            ..no_growth_config()
        },
        limits,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        GeneratedFamilyDepthGrowthError::ResourceLimit {
            resource: "depth-growth sector attempts",
            requested: 1,
            limit: 0,
        }
    ));

    let normal = GeneratedFamilyDepthGrowthCompiler::compile(
        &family,
        &context,
        base.clone(),
        no_growth_config(),
        GeneratedFamilyDepthGrowthLimits::default(),
    )
    .unwrap();
    assert!(normal.stats().retained_residual_leaves() > 0);
    assert!(normal.stats().retained_residual_predicates() > 0);

    let mut limits = GeneratedFamilyDepthGrowthLimits::default();
    limits.max_retained_residual_leaves = normal.stats().retained_residual_leaves() - 1;
    assert!(matches!(
        GeneratedFamilyDepthGrowthCompiler::compile(
            &family,
            &context,
            base.clone(),
            no_growth_config(),
            limits,
        ),
        Err(GeneratedFamilyDepthGrowthError::ResourceLimit {
            resource: "depth-growth retained residual leaves",
            ..
        })
    ));

    let mut limits = GeneratedFamilyDepthGrowthLimits::default();
    limits.max_retained_residual_predicates = normal.stats().retained_residual_predicates() - 1;
    assert!(matches!(
        GeneratedFamilyDepthGrowthCompiler::compile(
            &family,
            &context,
            base,
            no_growth_config(),
            limits,
        ),
        Err(GeneratedFamilyDepthGrowthError::ResourceLimit {
            resource: "depth-growth retained residual predicates",
            ..
        })
    ));
}

#[test]
fn latest_materials_retain_the_exact_family_shared_row_span() {
    let family = massive_tadpole("depth-growth-shared-row-span");
    let (context, base) = compile_base(&family);
    let certificate = GeneratedFamilyDepthGrowthCompiler::compile(
        &family,
        &context,
        base,
        no_growth_config(),
        GeneratedFamilyDepthGrowthLimits::default(),
    )
    .unwrap();
    let shared = certificate.base().row_span_arc().unwrap();
    for material in certificate.latest_successful_materials() {
        assert!(Arc::ptr_eq(material.discovery().row_span_arc(), shared));
        assert!(Arc::ptr_eq(
            material.discovery().coverage().row_span_arc(),
            shared
        ));
        assert!(Arc::ptr_eq(
            material.live_leaf_queue().discovery().row_span_arc(),
            shared
        ));
    }
}

#[test]
fn provider_rejects_replayed_base_and_round_interruptions() {
    let family = massive_tadpole("depth-growth-provider-base-interruption");
    let mut family_limits = GeneratedFamilyRuleSystemLimits::default();
    family_limits.discovery.max_candidate_layers = 0;
    let (context, base) = compile_base_with_limits(&family, family_limits);
    let certificate = GeneratedFamilyDepthGrowthCompiler::compile(
        &family,
        &context,
        base,
        no_growth_config(),
        GeneratedFamilyDepthGrowthLimits::default(),
    )
    .unwrap();
    let error = match GeneratedFamilyDepthGrowthProvider::try_new(
        &family,
        &context,
        certificate,
        rustred::GeneratedFamilyRuleSystemProviderLimits::default(),
    ) {
        Ok(_) => panic!("interrupted base family was accepted"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        GeneratedFamilyDepthGrowthProviderError::BaseResourceLimited { .. }
    ));

    let family = massive_tadpole("depth-growth-provider-round-interruption");
    let mut family_limits = GeneratedFamilyRuleSystemLimits::default();
    family_limits.discovery.max_candidate_layers = 1;
    let (context, base) = compile_base_with_limits(&family, family_limits);
    let certificate = GeneratedFamilyDepthGrowthCompiler::compile(
        &family,
        &context,
        base,
        GeneratedFamilyDepthGrowthConfig {
            maximum_depth: 1,
            ..no_growth_config()
        },
        GeneratedFamilyDepthGrowthLimits::default(),
    )
    .unwrap();
    let error = match GeneratedFamilyDepthGrowthProvider::try_new(
        &family,
        &context,
        certificate,
        rustred::GeneratedFamilyRuleSystemProviderLimits::default(),
    ) {
        Ok(_) => panic!("interrupted depth round was accepted"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        GeneratedFamilyDepthGrowthProviderError::RoundResourceLimited {
            depth: 1,
            stage: GeneratedFamilyDepthGrowthStage::Discovery,
            ..
        }
    ));
}

#[test]
#[ignore = "V4 explicit depth-two coverage exceeds its independent 65,536-split and 4,000,000 leaf-predicate safety limits; re-enable after Coverage V5/MTBDD ownership integration"]
fn automatic_depth_two_growth_closes_connected_sunset_j211() {
    let family = equal_mass_sunset("depth-growth-connected-sunset");
    let (context, base) = compile_base(&family);
    // The subsector-first prefix is generic. For this concrete three-line
    // family the first three residual sectors are 011, 101, and 110; the
    // top sector stays at its already successful depth-zero material.
    let provider = GeneratedFamilyDepthGrowthCompiler::compile_with_selected_provider(
        &family,
        &context,
        base,
        GeneratedFamilyDepthGrowthConfig {
            initial_depth: 0,
            maximum_depth: 2,
            selection: GeneratedFamilyDepthGrowthSelectionPolicy::ResidualSubsectorFirstPrefix {
                max_sectors_per_round: 2,
            },
            stop_on_no_strict_improvement: false,
        },
        GeneratedFamilyDepthGrowthLimits::default(),
        [key([1, 1, 1]), key([0, 1, 1])],
        rustred::GeneratedFamilyRuleSystemProviderLimits::default(),
    )
    .unwrap();
    let certificate = provider.certificate();
    assert_eq!(certificate.rounds().len(), 2);
    for round in certificate.rounds() {
        let attempted = round
            .attempts()
            .iter()
            .map(|attempt| attempt.sector().to_bit_string())
            .collect::<Vec<_>>();
        assert_eq!(attempted, ["011", "101"]);
    }
    let materials = certificate.latest_successful_materials();
    for sector in ["011", "101"] {
        assert_eq!(
            materials
                .iter()
                .find(|material| material.sector().to_bit_string() == sector)
                .unwrap()
                .depth(),
            2
        );
    }
    assert_eq!(
        materials
            .iter()
            .find(|material| material.sector().to_bit_string() == "111")
            .unwrap()
            .depth(),
        0
    );

    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );
    let result = engine.reduce(&key([2, 1, 1])).unwrap();
    result.require_complete().unwrap();
    assert_eq!(result.terms().len(), 1);
    assert_eq!(
        result.terms().get(&key([1, 1, 1])).unwrap(),
        &family.coefficient_context().parse("(d-3)/(3*m2)").unwrap()
    );
}
