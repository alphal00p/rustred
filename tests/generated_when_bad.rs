use std::sync::Arc;

use rustred::{
    AffineDenominator, CoefficientContext, GeneratedSourceAuthenticator, GeneratedSourceRowMode,
    GeneratedWhenBadCompilation, GeneratedWhenBadCompiler, GeneratedWhenBadError,
    GeneratedWhenBadLimits, GeneratedWhenBadSourceAuthentication, IndexShift, IntegralFamily,
    IntegralOrderingPolicy, ParametricCoefficientContext, ParametricElimination,
    ParametricEliminationLimits, ParametricEliminationOrdering, ParametricIbpGenerator,
    ParametricReductionRuleCandidate, ParametricRelation, ParametricRowId, ParametricRuleLimits,
    SectorMask,
};

fn family() -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        "generated-when-bad-one-loop",
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

fn candidate(
    context: &ParametricCoefficientContext,
    rows: &[ParametricRelation],
) -> ParametricReductionRuleCandidate {
    let elimination = ParametricElimination::build(
        context,
        rows,
        ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [2])
            .unwrap(),
        ParametricEliminationLimits::default(),
    )
    .unwrap();
    ParametricReductionRuleCandidate::try_from_elimination_pivot(
        context,
        rows,
        &elimination,
        0,
        SectorMask::try_new([true]).unwrap(),
        ParametricRuleLimits::default(),
    )
    .unwrap()
}

#[test]
fn production_wrapper_regenerates_and_binds_canonical_ibp_sources() {
    let family = family();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let candidate = candidate(generated.context(), &rows);

    let compiled = GeneratedWhenBadCompiler::compile(
        &family,
        generated.context(),
        &candidate,
        GeneratedWhenBadLimits::default(),
    )
    .unwrap();
    let source = match &compiled {
        GeneratedWhenBadCompilation::Certified(certificate) => {
            certificate.replay(&family, generated.context()).unwrap();
            certificate.source_authentication()
        }
        GeneratedWhenBadCompilation::Unsupported(unsupported) => {
            unsupported.replay(&family, generated.context()).unwrap();
            unsupported.source_authentication()
        }
    };
    assert_eq!(
        source.source_authentication(),
        GeneratedWhenBadSourceAuthentication::CanonicalIbpLiAndExactTranslations
    );
    assert_eq!(source.stats().retained_rows(), rows.len());
    assert_eq!(source.stats().original_rows(), rows.len());
    assert!(
        source
            .witnesses()
            .iter()
            .all(|witness| witness.mode() == GeneratedSourceRowMode::CanonicalOriginal)
    );
}

#[test]
fn exact_symbolica_translation_is_authenticated_without_trusting_its_label() {
    let family = family();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let offset = IndexShift::try_new([3], 1).unwrap();
    let translated = generated.ordinary_ibp()[0]
        .translated(
            generated.context(),
            &offset,
            ParametricRowId::Derived {
                label: Arc::from("deliberately-not-an-adaptive-label"),
            },
            GeneratedWhenBadLimits::default().ibp.arithmetic_limits,
        )
        .unwrap();
    let rows = vec![translated];
    let candidate = candidate(generated.context(), &rows);
    let certificate = GeneratedSourceAuthenticator::authenticate(
        &family,
        generated.context(),
        &candidate,
        GeneratedWhenBadLimits::default(),
    )
    .unwrap();
    assert_eq!(certificate.stats().translated_rows(), 1);
    assert_eq!(
        certificate.witnesses()[0].mode(),
        GeneratedSourceRowMode::ExactTranslation
    );
    assert_eq!(certificate.witnesses()[0].translation().values(), &[3]);
    certificate
        .replay(&family, generated.context(), &candidate)
        .unwrap();
}

#[test]
fn self_consistent_forged_identity_is_rejected_at_production_boundary() {
    let family = family();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let mut forged = ParametricRelation::new(
        family.fingerprint(),
        ParametricRowId::Derived {
            label: Arc::from("forged-I(n)=0"),
        },
        generated.context(),
    );
    forged
        .add_term(
            generated.context(),
            IndexShift::try_new([0], 1).unwrap(),
            generated.context().one(),
        )
        .unwrap();
    let candidate = candidate(generated.context(), &[forged]);
    assert!(matches!(
        GeneratedSourceAuthenticator::authenticate(
            &family,
            generated.context(),
            &candidate,
            GeneratedWhenBadLimits::default(),
        ),
        Err(GeneratedWhenBadError::UnauthenticatedRetainedSourceRow {
            retained_ordinal: 0
        })
    ));
}

#[test]
fn aggregate_match_attempt_budget_fails_before_row_authentication() {
    let family = family();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let candidate = candidate(generated.context(), &rows);
    let mut limits = GeneratedWhenBadLimits::default();
    limits.max_match_attempts = 0;
    assert!(matches!(
        GeneratedSourceAuthenticator::authenticate(
            &family,
            generated.context(),
            &candidate,
            limits,
        ),
        Err(GeneratedWhenBadError::ResourceLimit {
            resource: "generated-source match attempts",
            requested: 1,
            limit: 0,
        })
    ));
}

#[test]
fn row_translation_and_manifest_budgets_are_preflighted_before_replay() {
    let family = family();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let candidate = candidate(generated.context(), &rows);

    let mut limits = GeneratedWhenBadLimits::default();
    limits.max_retained_rows = 0;
    assert!(matches!(
        GeneratedSourceAuthenticator::authenticate(
            &family,
            generated.context(),
            &candidate,
            limits,
        ),
        Err(GeneratedWhenBadError::ResourceLimit {
            resource: "retained parametric source rows",
            requested: 1,
            limit: 0,
        })
    ));

    let mut limits = GeneratedWhenBadLimits::default();
    limits.max_translation_components = 0;
    assert!(matches!(
        GeneratedSourceAuthenticator::authenticate(
            &family,
            generated.context(),
            &candidate,
            limits,
        ),
        Err(GeneratedWhenBadError::ResourceLimit {
            resource: "generated-source translation components",
            requested: 1,
            limit: 0,
        })
    ));

    let mut limits = GeneratedWhenBadLimits::default();
    limits.max_source_manifest_bytes = 0;
    assert!(matches!(
        GeneratedSourceAuthenticator::authenticate(
            &family,
            generated.context(),
            &candidate,
            limits,
        ),
        Err(GeneratedWhenBadError::ResourceLimit {
            resource: "candidate source manifest bytes",
            requested,
            limit: 0,
        }) if requested > 0
    ));
}

#[test]
fn canonical_row_count_is_preflighted_without_generating_the_rows() {
    let family = family();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let candidate = candidate(generated.context(), &rows);
    let mut limits = GeneratedWhenBadLimits::default();
    limits.max_canonical_rows = 0;
    assert!(matches!(
        GeneratedSourceAuthenticator::authenticate(
            &family,
            generated.context(),
            &candidate,
            limits,
        ),
        Err(GeneratedWhenBadError::ResourceLimit {
            resource: "canonical generated IBP/LI rows",
            requested: 1,
            limit: 0,
        })
    ));
}
