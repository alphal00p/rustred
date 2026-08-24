//! End-to-end concrete application of the family-wide generated certificate.
//!
//! Both topologies are fixtures only. The provider consumes the exact output
//! of `GeneratedFamilyRuleSystemCompiler`; no recurrence, loop count, or
//! topology name enters production construction.

use std::sync::Arc;

use rustred::{
    AffineDenominator, CoefficientContext, ConcreteIntegralKey, ConcreteRuleDecision,
    ConcreteRuleProvider, GeneratedFamilyPipelineStage, GeneratedFamilyRuleSystemCompiler,
    GeneratedFamilyRuleSystemConfig, GeneratedFamilyRuleSystemLimits,
    GeneratedFamilyRuleSystemProvider, GeneratedFamilyRuleSystemProviderError,
    GeneratedFamilyRuleSystemProviderLimits, GeneratedSymbolicRowSpanStrategy, IntegralFamily,
    IntegralOrderingPolicy, InternalSymmetrySearchLimits, MasterPolicyTerminal,
    ParametricIbpGenerator, ParametricReductionEngine, ParametricSectorRuleProviderError,
    PowerShiftPolicy, ReductionEngineError, ReductionEngineLimits, SectorRestrictions,
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

fn massless_tadpole(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d"]);
    IntegralFamily::new(
        name,
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            coefficients.zero(),
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

fn family_certificate(
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
    // A radius-zero queue is sufficient for these provider-routing tests and
    // keeps the connected two-loop fixture bounded without changing discovery.
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

#[test]
fn one_loop_family_certificate_drives_the_vakint_scalar_oracle() {
    let family = massive_tadpole("family-provider-one-loop-vakint");
    let (context, certificate) =
        family_certificate(&family, GeneratedFamilyRuleSystemLimits::default());
    let provider = GeneratedFamilyRuleSystemProvider::try_with_selected(
        &family,
        &context,
        certificate,
        [key([1])],
        GeneratedFamilyRuleSystemProviderLimits::default(),
    )
    .unwrap();
    provider.replay().unwrap();
    assert_eq!(provider.build_stats().retained_generated_sectors(), 1);
    assert_eq!(provider.build_stats().master_terminals(), 1);
    assert_eq!(provider.build_stats().sector_transcripts(), 2);
    assert_eq!(provider.build_stats().excluded_sectors(), 0);
    assert_eq!(provider.build_stats().proved_zero_sectors(), 1);
    assert_eq!(
        provider.build_stats().candidate_attempts(),
        provider
            .certificate()
            .stats()
            .generated_candidate_attempts()
    );
    assert_eq!(
        provider.build_stats().global_leaves(),
        provider.certificate().stats().generated_global_leaves()
    );
    assert_eq!(
        provider.build_stats().live_leaf_work_items(),
        provider.certificate().stats().queued_exceptional_leaves()
    );
    assert_eq!(
        provider.ordering(),
        IntegralOrderingPolicy::RustRedUnshiftedV1
    );
    assert_eq!(
        provider.inventory_power_shift_policy(),
        PowerShiftPolicy::FormalGeneric
    );
    assert_eq!(provider.inventory_restrictions().arity(), 1);

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
        assert_eq!(
            result.terms().get(&key([1])).unwrap(),
            &family.coefficient_context().parse(expected).unwrap()
        );
    }

    // The all-inactive sector is analytically zero even if a caller later
    // (incorrectly) selects the same key. The zero layer must preempt it.
    engine
        .provider_mut()
        .insert_selected_master(key([0]))
        .unwrap();
    let zero = engine.reduce(&key([0])).unwrap();
    assert!(zero.terms().is_empty());
    assert!(zero.selected_masters().is_empty());
}

#[test]
fn constructor_rejects_retained_family_interruptions_with_sector_and_stage() {
    let family = massive_tadpole("family-provider-reject-interruption");
    let mut family_limits = GeneratedFamilyRuleSystemLimits::default();
    family_limits.discovery.max_candidate_layers = 0;
    let (context, certificate) = family_certificate(&family, family_limits);
    let error = match GeneratedFamilyRuleSystemProvider::try_new(
        &family,
        &context,
        certificate,
        GeneratedFamilyRuleSystemProviderLimits::default(),
    ) {
        Ok(_) => panic!("interrupted family transcript was accepted"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        GeneratedFamilyRuleSystemProviderError::InterruptedResource {
            sector,
            stage: GeneratedFamilyPipelineStage::Discovery,
            ..
        } if sector.to_bit_string() == "1"
    ));

    // A caller terminal stream cannot mask the typed retained-certificate
    // interruption, even when its known size exceeds the provider cap.
    let family = massive_tadpole("family-provider-interruption-precedes-terminal-cap");
    let mut family_limits = GeneratedFamilyRuleSystemLimits::default();
    family_limits.discovery.max_candidate_layers = 0;
    let (context, certificate) = family_certificate(&family, family_limits);
    let mut provider_limits = GeneratedFamilyRuleSystemProviderLimits::default();
    provider_limits.max_input_terminals = 0;
    assert!(matches!(
        GeneratedFamilyRuleSystemProvider::try_with_selected(
            &family,
            &context,
            certificate,
            [key([1])],
            provider_limits,
        ),
        Err(GeneratedFamilyRuleSystemProviderError::InterruptedResource {
            sector,
            stage: GeneratedFamilyPipelineStage::Discovery,
            ..
        }) if sector.to_bit_string() == "1"
    ));

    let family = massive_tadpole("family-provider-reject-failure");
    let mut family_limits = GeneratedFamilyRuleSystemLimits::default();
    family_limits
        .discovery
        .adaptive
        .rule
        .arithmetic
        .max_source_terms -= 1;
    let (context, certificate) = family_certificate(&family, family_limits);
    let error = match GeneratedFamilyRuleSystemProvider::try_new(
        &family,
        &context,
        certificate,
        GeneratedFamilyRuleSystemProviderLimits::default(),
    ) {
        Ok(_) => panic!("failed family transcript was accepted"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        GeneratedFamilyRuleSystemProviderError::InterruptedFailure {
            sector,
            stage: GeneratedFamilyPipelineStage::Discovery,
            ..
        } if sector.to_bit_string() == "1"
    ));
}

#[test]
fn aggregate_limits_are_checked_before_cloning_generated_sector_payloads() {
    let family = massive_tadpole("family-provider-aggregate-limit");
    let (context, certificate) =
        family_certificate(&family, GeneratedFamilyRuleSystemLimits::default());
    let mut limits = GeneratedFamilyRuleSystemProviderLimits::default();
    limits.max_retained_generated_sectors = 0;
    assert!(matches!(
        GeneratedFamilyRuleSystemProvider::try_new(&family, &context, certificate, limits),
        Err(GeneratedFamilyRuleSystemProviderError::ResourceLimit {
            resource: "family provider generated sectors",
            requested: 1,
            limit: 0,
        })
    ));
}

#[test]
fn input_terminal_cap_counts_duplicate_declarations_before_orbit_deduplication() {
    let family = massive_tadpole("family-provider-input-terminal-cap");
    let (context, certificate) =
        family_certificate(&family, GeneratedFamilyRuleSystemLimits::default());
    let mut limits = GeneratedFamilyRuleSystemProviderLimits::default();
    limits.max_input_terminals = 1;
    let duplicate = key([1]);
    assert!(matches!(
        GeneratedFamilyRuleSystemProvider::try_with_selected(
            &family,
            &context,
            certificate,
            [duplicate.clone(), duplicate],
            limits,
        ),
        Err(GeneratedFamilyRuleSystemProviderError::ResourceLimit {
            resource: "family provider input terminal declarations",
            requested: 2,
            limit: 1,
        })
    ));
}

#[test]
fn nested_queue_retention_limits_are_preflighted_from_borrowed_payloads() {
    let family = massive_tadpole("family-provider-nested-borrowed-preflight");
    let (context, certificate) =
        family_certificate(&family, GeneratedFamilyRuleSystemLimits::default());
    let requested = certificate.stats().generated_global_leaves();
    assert!(requested > 0);
    let mut limits = GeneratedFamilyRuleSystemProviderLimits::default();
    // The outer family work-item and leaf caps remain permissive. The nested
    // conditional root-leaf cap is checked directly on borrowed queue
    // metadata, before any coverage/queue clone is made for installation.
    limits.conditional_rules.max_total_root_leaves = 0;
    assert!(matches!(
        GeneratedFamilyRuleSystemProvider::try_new(&family, &context, certificate, limits),
        Err(GeneratedFamilyRuleSystemProviderError::Provider(
            rustred::CertifiedZeroSectorRuleProviderError::Inner(
                rustred::CertifiedSymmetryCanonicalizingRuleProviderError::Inner(
                    rustred::MasterPolicyError::Inner(
                        rustred::GeneratedSectorConditionalRuleProviderError::ResourceLimit {
                            resource: "conditional root leaves",
                            requested: actual,
                            limit: 0,
                        }
                    )
                )
            )
        )) if actual == requested
    ));
}

#[test]
fn explicit_master_policy_is_mutable_but_never_inferred() {
    let family = massive_tadpole("family-provider-explicit-master-policy");
    let (context, certificate) =
        family_certificate(&family, GeneratedFamilyRuleSystemLimits::default());
    let mut provider = GeneratedFamilyRuleSystemProvider::try_new(
        &family,
        &context,
        certificate,
        GeneratedFamilyRuleSystemProviderLimits::default(),
    )
    .unwrap();
    assert!(provider.terminals().is_empty());
    assert!(matches!(
        provider.decision_for(&key([1])).unwrap(),
        ConcreteRuleDecision::Terminal(rustred::ConcreteTerminalStatus::Uncovered)
    ));
    provider
        .insert_terminal(key([1]), MasterPolicyTerminal::Selected)
        .unwrap();
    assert!(matches!(
        provider.decision_for(&key([1])).unwrap(),
        ConcreteRuleDecision::Terminal(rustred::ConcreteTerminalStatus::SelectedMaster)
    ));
    provider.replay().unwrap();
}

#[test]
fn no_generated_work_uses_an_exact_identity_symmetry_layer() {
    let family = massless_tadpole("family-provider-no-generated-work");
    let (context, certificate) =
        family_certificate(&family, GeneratedFamilyRuleSystemLimits::default());
    assert!(certificate.row_span_arc().is_none());
    let mut provider = GeneratedFamilyRuleSystemProvider::try_new(
        &family,
        &context,
        certificate,
        GeneratedFamilyRuleSystemProviderLimits::default(),
    )
    .unwrap();
    assert!(provider.symmetry_provider().row_span_arc().is_none());
    assert!(
        provider
            .symmetry_provider()
            .compatible_symmetry_ordinals()
            .is_empty()
    );
    assert!(matches!(
        provider.decision_for(&key([1])).unwrap(),
        ConcreteRuleDecision::ProvedZero(_)
    ));
    provider.replay().unwrap();
}

#[test]
fn bounded_family_symmetries_canonicalize_master_policy_and_short_circuit_inner_rules() {
    let family = equal_mass_sunset("family-provider-bounded-symmetry");
    let mut family_limits = GeneratedFamilyRuleSystemLimits::default();
    family_limits.discovery.adaptive.max_search_depth = 0;
    family_limits
        .discovery
        .coverage
        .generated_when_bad
        .row_span
        .strategy = GeneratedSymbolicRowSpanStrategy::BoundedVacuumInternal {
        search: InternalSymmetrySearchLimits::default(),
        require_exhaustive: true,
    };
    let (context, certificate) = family_certificate(&family, family_limits);
    assert_eq!(certificate.row_span_arc().unwrap().symmetries().len(), 6);
    let source = key([2, 1, 1]);
    let canonical = key([1, 1, 2]);
    let mut provider = GeneratedFamilyRuleSystemProvider::try_with_selected(
        &family,
        &context,
        certificate,
        [source.clone(), canonical.clone()],
        GeneratedFamilyRuleSystemProviderLimits::default(),
    )
    .unwrap();

    let shared_row_span = provider.certificate().row_span_arc().unwrap();
    assert!(Arc::ptr_eq(
        provider.symmetry_provider().row_span_arc().unwrap(),
        shared_row_span,
    ));
    assert_eq!(provider.terminals().len(), 1);
    assert_eq!(
        provider.terminals().get(&canonical),
        Some(&MasterPolicyTerminal::Selected)
    );
    assert_eq!(provider.build_stats().master_terminals(), 1);

    let ConcreteRuleDecision::CertifiedRewrite(rewrite) = provider.decision_for(&source).unwrap()
    else {
        panic!("a noncanonical selected master must first receive a symmetry rewrite")
    };
    assert_eq!(rewrite.rhs().first_key_value().unwrap().0, &canonical);
    rewrite
        .replay(
            &family,
            &context,
            IntegralOrderingPolicy::RustRedUnshiftedV1,
        )
        .unwrap();
    assert_eq!(provider.symmetry_provider().stats().symmetry_rewrites(), 1);
    assert_eq!(provider.symmetry_provider().stats().delegated_queries(), 0);
    assert!(matches!(
        provider.decision_for(&canonical).unwrap(),
        ConcreteRuleDecision::Terminal(rustred::ConcreteTerminalStatus::SelectedMaster)
    ));

    provider.insert_selected_master(key([3, 1, 1])).unwrap();
    assert!(!provider.terminals().contains_key(&key([3, 1, 1])));
    assert!(!provider.terminals().contains_key(&key([1, 3, 1])));
    assert!(provider.terminals().contains_key(&key([1, 1, 3])));
    assert!(matches!(
        provider.insert_certified_master(key([1, 3, 1]), "different-master-proof"),
        Err(GeneratedFamilyRuleSystemProviderError::Provider(
            rustred::CertifiedZeroSectorRuleProviderError::Inner(
                rustred::CertifiedSymmetryCanonicalizingRuleProviderError::Inner(
                    rustred::MasterPolicyError::ConflictingTerminal { integral }
                )
            )
        )) if integral == key([1, 1, 3])
    ));
    assert_eq!(
        provider.terminals().get(&key([1, 1, 3])),
        Some(&MasterPolicyTerminal::Selected),
        "a conflicting orbit-image declaration must not mutate canonical policy state"
    );
    assert!(provider.remove_master(&key([1, 3, 1])).unwrap());
    assert!(!provider.terminals().contains_key(&key([1, 1, 3])));
    provider.replay().unwrap();
}

#[test]
fn connected_two_loop_depth_zero_generates_top_rule_then_reports_exact_boundary_blocker() {
    let family = equal_mass_sunset("family-provider-connected-two-loop-vakint");
    let mut family_limits = GeneratedFamilyRuleSystemLimits::default();
    family_limits.discovery.adaptive.max_search_depth = 0;
    let (context, certificate) = family_certificate(&family, family_limits);
    let mut provider = GeneratedFamilyRuleSystemProvider::try_with_selected(
        &family,
        &context,
        certificate,
        [key([1, 1, 1]), key([0, 1, 1])],
        GeneratedFamilyRuleSystemProviderLimits::default(),
    )
    .unwrap();
    provider.replay().unwrap();
    let shared_row_span = provider
        .certificate()
        .row_span_arc()
        .expect("connected generated sectors retain one family row span");
    assert_eq!(provider.sector_provider().certificates().len(), 4);
    for coverage in provider.sector_provider().certificates().values() {
        assert!(Arc::ptr_eq(coverage.row_span_arc(), shared_row_span));
    }
    assert_eq!(provider.conditional_provider().queues().len(), 4);
    for queue in provider.conditional_provider().queues() {
        assert!(Arc::ptr_eq(
            queue.discovery().row_span_arc(),
            shared_row_span
        ));
    }
    let ConcreteRuleDecision::Reduction(first_rule) =
        provider.decision_for(&key([2, 1, 1])).unwrap()
    else {
        panic!("top-sector J211 must have a generated first-step rule")
    };
    assert_eq!(
        first_rule.rhs().keys().cloned().collect::<Vec<_>>(),
        [
            key([0, 1, 2]),
            key([1, 0, 2]),
            key([1, 1, 1]),
            key([1, 1, 2]),
        ]
    );
    for (powers, expected_unsupported, expected_conditional_candidates) in [
        ([0, 1, 2], Some(&[0usize, 2][..]), 0),
        ([1, 0, 2], Some(&[1usize, 3][..]), 0),
        ([1, 1, 2], None, 0),
    ] {
        let sector = rustred::SectorMask::try_from_indices(&powers).unwrap();
        let coverage = provider
            .sector_provider()
            .certificates()
            .get(&sector)
            .unwrap();
        let classification = coverage
            .classification_for_indices(&context, &powers)
            .unwrap()
            .unwrap();
        let conditional_candidates = provider
            .conditional_provider()
            .rule_provenance(&sector)
            .map(|rules| rules.count())
            .unwrap();
        assert_eq!(conditional_candidates, expected_conditional_candidates);
        match (classification.disposition(), expected_unsupported) {
            (
                rustred::ParametricSectorLeafDisposition::Unsupported { candidate_ordinals },
                Some(expected),
            ) => assert_eq!(candidate_ordinals.as_ref(), expected),
            (
                rustred::ParametricSectorLeafDisposition::DescendingRule {
                    candidate_ordinal: 2,
                },
                None,
            ) => {}
            (actual, expected) => panic!(
                "unexpected depth-zero classification for {powers:?}: {actual:?}, expected unsupported {expected:?}"
            ),
        }
    }
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );

    match engine.reduce(&key([2, 1, 1])) {
        Ok(result) => {
            result.require_complete().unwrap();
            assert_eq!(result.terms().len(), 1);
            assert_eq!(
                result.terms().get(&key([1, 1, 1])).unwrap(),
                &family.coefficient_context().parse("(d-3)/(3*m2)").unwrap(),
                "generated connected two-loop reduction differs from the frozen Vakint oracle"
            );
        }
        Err(ReductionEngineError::Provider(GeneratedFamilyRuleSystemProviderError::Provider(
            rustred::CertifiedZeroSectorRuleProviderError::Inner(
                rustred::CertifiedSymmetryCanonicalizingRuleProviderError::Inner(
                    rustred::MasterPolicyError::Inner(
                        rustred::GeneratedSectorConditionalRuleProviderError::Inner(
                            ParametricSectorRuleProviderError::UnsupportedLeaf { sector, .. },
                        ),
                    ),
                ),
            ),
        ))) => {
            assert_eq!(sector.to_bit_string(), "011");
            // This is the honest current blocker: the bounded family-wide
            // generated certificate produced the top-sector first step, but
            // retained an exceptional factorized-sector leaf. No topology-
            // specific recurrence or inferred master was inserted.
        }
        Err(other) => panic!("unexpected connected two-loop provider outcome: {other:?}"),
    }
}
