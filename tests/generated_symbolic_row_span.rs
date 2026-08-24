use std::sync::Arc;

use rustred::{
    AffineDenominator, CoefficientContext, CutConstraint, GENERATED_SECTOR_DISCOVERY_V2_SCHEMA,
    GeneratedSectorDiscoveryCompiler, GeneratedSectorDiscoveryLimits, GeneratedSourceAuthenticator,
    GeneratedSourceRowMode, GeneratedSymbolicRowSpanCompiler, GeneratedSymbolicRowSpanConfig,
    GeneratedSymbolicRowSpanError, GeneratedSymbolicRowSpanLineage,
    GeneratedSymbolicRowSpanStrategy, GeneratedWhenBadError, GeneratedWhenBadLimits, IndexShift,
    IntegralFamily, IntegralOrderingPolicy, InternalSymmetryReplayError,
    InternalSymmetrySearchLimits, ParametricCoefficientContext, ParametricElimination,
    ParametricEliminationLimits, ParametricEliminationOrdering, ParametricIbpGenerator,
    ParametricReductionRuleCandidate, ParametricRelation, ParametricRowId, ParametricRuleLimits,
    SectorMask, SectorPattern, SectorRestrictions, discover_bounded_vacuum_internal_symmetries,
};

fn equal_mass_sunset(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let zero = coefficients.zero();
    let one = coefficients.one();
    let minus_m2 = coefficients.parse("-m2").unwrap();
    IntegralFamily::new(
        name,
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

fn scaled_equal_mass_sunset(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["a", "d", "m2"]);
    let zero = coefficients.zero();
    let a = coefficients.parameter("a").unwrap();
    let two_a = coefficients.parse("2*a").unwrap();
    let minus_m2 = coefficients.parse("-m2").unwrap();
    IntegralFamily::new(
        name,
        vec!["k1".into(), "k2".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                minus_m2.clone(),
                vec![a.clone(), zero.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                minus_m2.clone(),
                vec![zero.clone(), zero.clone(), a.clone()],
            ),
            AffineDenominator::new(minus_m2, vec![a.clone(), two_a, a]),
        ],
        Vec::new(),
        vec![zero.clone(), zero.clone(), zero],
    )
    .unwrap()
}

fn default_context(family: &IntegralFamily) -> ParametricCoefficientContext {
    ParametricIbpGenerator::try_new(family)
        .unwrap()
        .context()
        .clone()
}

fn bounded_config() -> GeneratedSymbolicRowSpanConfig {
    let mut config = GeneratedSymbolicRowSpanConfig::default();
    config.strategy = GeneratedSymbolicRowSpanStrategy::BoundedVacuumInternal {
        search: InternalSymmetrySearchLimits::default(),
        require_exhaustive: true,
    };
    config
}

fn candidate(
    context: &ParametricCoefficientContext,
    rows: &[ParametricRelation],
) -> ParametricReductionRuleCandidate {
    let ordering = ParametricEliminationOrdering::try_new(
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        [2, 2, 2],
    )
    .unwrap();
    let elimination = ParametricElimination::build(
        context,
        rows,
        ordering,
        ParametricEliminationLimits::default(),
    )
    .unwrap();
    ParametricReductionRuleCandidate::try_from_elimination_pivot(
        context,
        rows,
        &elimination,
        0,
        SectorMask::try_new([true, true, true]).unwrap(),
        ParametricRuleLimits::default(),
    )
    .unwrap()
}

#[test]
fn bounded_compiler_augments_only_with_replayable_whole_rows() {
    let family = equal_mass_sunset("generated-row-span-bounded");
    let context = default_context(&family);
    let certificate = GeneratedSymbolicRowSpanCompiler::compile(
        &family,
        &context,
        GeneratedWhenBadLimits::default().ibp,
        bounded_config(),
    )
    .unwrap();

    let stats = certificate.stats();
    assert_eq!(stats.canonical_rows(), 4);
    assert!(stats.verified_symmetries() > 1);
    assert!(stats.nonidentity_symmetries() > 0);
    assert_eq!(
        stats.transport_attempts(),
        stats.canonical_rows() * stats.nonidentity_symmetries()
    );
    assert_eq!(
        stats.transport_attempts(),
        stats.retained_transports() + stats.exact_duplicate_transports()
    );
    assert_eq!(certificate.rows().len(), certificate.lineages().len());
    assert_eq!(stats.augmented_rows(), certificate.rows().len());
    assert!(stats.retained_transports() > 0);

    for (row, lineage) in certificate.rows().iter().zip(certificate.lineages()) {
        if let GeneratedSymbolicRowSpanLineage::VerifiedWholeRowSymmetryTransport {
            transport,
            ..
        } = lineage
        {
            assert!(
                row.has_identical_guard_provenance(transport.transported_relation()),
                "the retained source must be the certified whole-row image"
            );
            transport.replay(&family, &context).unwrap();
        }
    }
    certificate.replay(&family, &context).unwrap();
}

#[test]
fn source_authentication_distinguishes_transport_and_its_exact_translation() {
    let family = equal_mass_sunset("generated-row-span-authentication");
    let context = default_context(&family);
    let config = bounded_config();
    let row_span = GeneratedSymbolicRowSpanCompiler::compile(
        &family,
        &context,
        GeneratedWhenBadLimits::default().ibp,
        config,
    )
    .unwrap();
    let transported = row_span
        .lineages()
        .iter()
        .zip(row_span.rows())
        .find_map(|(lineage, row)| {
            matches!(
                lineage,
                GeneratedSymbolicRowSpanLineage::VerifiedWholeRowSymmetryTransport { .. }
            )
            .then_some(row.clone())
        })
        .unwrap();

    let direct_candidate = candidate(&context, std::slice::from_ref(&transported));
    assert!(matches!(
        GeneratedSourceAuthenticator::authenticate(
            &family,
            &context,
            &direct_candidate,
            GeneratedWhenBadLimits::default(),
        ),
        Err(GeneratedWhenBadError::UnauthenticatedRetainedSourceRow {
            retained_ordinal: 0
        })
    ));

    let mut limits = GeneratedWhenBadLimits::default();
    limits.row_span = config;
    let authenticated =
        GeneratedSourceAuthenticator::authenticate(&family, &context, &direct_candidate, limits)
            .unwrap();
    assert_eq!(authenticated.witnesses().len(), 1);
    assert_eq!(
        authenticated.witnesses()[0].mode(),
        GeneratedSourceRowMode::VerifiedWholeRowSymmetryTransport
    );
    assert!(authenticated.witnesses()[0].symmetry_ordinal().is_some());
    assert!(
        authenticated.witnesses()[0]
            .symmetry_permutation()
            .is_some()
    );
    authenticated
        .replay(&family, &context, &direct_candidate)
        .unwrap();

    let mut witness_limited = limits;
    witness_limited.max_symmetry_witness_components = 0;
    assert!(matches!(
        GeneratedSourceAuthenticator::authenticate(
            &family,
            &context,
            &direct_candidate,
            witness_limited,
        ),
        Err(GeneratedWhenBadError::ResourceLimit {
            resource: "generated-source symmetry witness components",
            ..
        })
    ));

    let translation = IndexShift::try_new([1, -1, 2], 3).unwrap();
    let translated = transported
        .translated(
            &context,
            &translation,
            ParametricRowId::Derived {
                label: Arc::from("translated-verified-whole-row"),
            },
            limits.ibp.arithmetic_limits,
        )
        .unwrap();
    let translated_candidate = candidate(&context, &[translated]);
    let authenticated = GeneratedSourceAuthenticator::authenticate(
        &family,
        &context,
        &translated_candidate,
        limits,
    )
    .unwrap();
    assert_eq!(
        authenticated.witnesses()[0].mode(),
        GeneratedSourceRowMode::ExactTranslationOfVerifiedWholeRowSymmetryTransport
    );
    assert_eq!(authenticated.witnesses()[0].translation(), &translation);
    authenticated
        .replay(&family, &context, &translated_candidate)
        .unwrap();
}

#[test]
fn termwise_fragment_is_not_authenticated_as_a_symmetry_transport() {
    let family = equal_mass_sunset("generated-row-span-termwise-forgery");
    let context = default_context(&family);
    let config = bounded_config();
    let row_span = GeneratedSymbolicRowSpanCompiler::compile(
        &family,
        &context,
        GeneratedWhenBadLimits::default().ibp,
        config,
    )
    .unwrap();
    let transported = row_span
        .lineages()
        .iter()
        .zip(row_span.rows())
        .find_map(|(lineage, row)| lineage.symmetry_ordinal().map(|_| row))
        .unwrap();
    assert!(transported.terms().len() > 1);

    let mut fragment = ParametricRelation::new(
        family.fingerprint(),
        ParametricRowId::Derived {
            label: Arc::from("forged-termwise-symmetry-quotient"),
        },
        &context,
    );
    let (shift, coefficient) = transported.terms().iter().next().unwrap();
    fragment
        .add_term(&context, shift.clone(), coefficient.clone())
        .unwrap();
    let fragment_candidate = candidate(&context, &[fragment]);
    let mut limits = GeneratedWhenBadLimits::default();
    limits.row_span = config;
    assert!(matches!(
        GeneratedSourceAuthenticator::authenticate(&family, &context, &fragment_candidate, limits,),
        Err(GeneratedWhenBadError::UnauthenticatedRetainedSourceRow {
            retained_ordinal: 0
        })
    ));
}

#[test]
fn transported_row_with_forged_guard_origins_is_rejected() {
    let family = scaled_equal_mass_sunset("generated-row-span-guard-forgery");
    let context = default_context(&family);
    let config = bounded_config();
    let row_span = GeneratedSymbolicRowSpanCompiler::compile(
        &family,
        &context,
        GeneratedWhenBadLimits::default().ibp,
        config,
    )
    .unwrap();
    let transported = row_span
        .lineages()
        .iter()
        .zip(row_span.rows())
        .find_map(|(lineage, row)| {
            (lineage.symmetry_ordinal().is_some() && !row.guarded_nonzero_conditions().is_empty())
                .then_some(row)
        })
        .expect("the non-unit scalar-product basis must retain guarded transports");

    let mut forged =
        ParametricRelation::new(family.fingerprint(), transported.row_id().clone(), &context);
    for polynomial in transported.nonzero_conditions() {
        forged
            .add_nonzero_condition(&context, polynomial.clone())
            .unwrap();
    }
    for (shift, coefficient) in transported.terms() {
        forged
            .add_term(&context, shift.clone(), coefficient.clone())
            .unwrap();
    }
    assert_eq!(&forged, transported);
    assert!(!forged.has_identical_guard_provenance(transported));

    let forged_candidate = candidate(&context, &[forged]);
    let mut limits = GeneratedWhenBadLimits::default();
    limits.row_span = config;
    assert!(matches!(
        GeneratedSourceAuthenticator::authenticate(&family, &context, &forged_candidate, limits,),
        Err(GeneratedWhenBadError::UnauthenticatedRetainedSourceRow {
            retained_ordinal: 0
        })
    ));
}

#[test]
fn explicit_verified_inputs_are_replayed_and_bound_to_the_family() {
    let family = equal_mass_sunset("generated-row-span-explicit");
    let context = default_context(&family);
    let restrictions = SectorRestrictions::try_new(
        CutConstraint::try_from_positions(3, [0]).unwrap(),
        SectorPattern::any(3).unwrap(),
    )
    .unwrap();
    let report = discover_bounded_vacuum_internal_symmetries(
        &family,
        &restrictions,
        InternalSymmetrySearchLimits::default(),
    )
    .unwrap();
    let limits = GeneratedSymbolicRowSpanConfig::default().limits;
    let certificate = GeneratedSymbolicRowSpanCompiler::compile_with_verified_symmetries(
        &family,
        &context,
        GeneratedWhenBadLimits::default().ibp,
        report.symmetries(),
        limits,
    )
    .unwrap();
    assert!(certificate.config().strategy.uses_verified_inputs());
    assert!(certificate.stats().retained_transports() > 0);
    assert!(
        certificate
            .symmetries()
            .iter()
            .all(|symmetry| symmetry.restrictions() == &restrictions)
    );
    certificate.replay(&family, &context).unwrap();

    let unrestricted = SectorRestrictions::unrestricted(3).unwrap();
    let restricted_nonidentity = report
        .symmetries()
        .iter()
        .find(|symmetry| symmetry.denominator_permutation() != [0, 1, 2])
        .unwrap();
    assert!(matches!(
        restricted_nonidentity.replay(&family, &unrestricted, limits.transport.symmetry),
        Err(InternalSymmetryReplayError::RestrictionsMismatch)
    ));

    let foreign = equal_mass_sunset("generated-row-span-explicit-foreign");
    let foreign_context = default_context(&foreign);
    assert!(matches!(
        GeneratedSymbolicRowSpanCompiler::compile_with_verified_symmetries(
            &foreign,
            &foreign_context,
            GeneratedWhenBadLimits::default().ibp,
            report.symmetries(),
            limits,
        ),
        Err(GeneratedSymbolicRowSpanError::SymmetryReplay(_))
            | Err(GeneratedSymbolicRowSpanError::Transport(_))
    ));
}

#[test]
fn transport_and_search_budgets_fail_closed_before_augmentation() {
    let family = equal_mass_sunset("generated-row-span-resources");
    let context = default_context(&family);
    let mut config = bounded_config();
    config.limits.max_transport_attempts = 0;
    assert!(matches!(
        GeneratedSymbolicRowSpanCompiler::compile(
            &family,
            &context,
            GeneratedWhenBadLimits::default().ibp,
            config,
        ),
        Err(GeneratedSymbolicRowSpanError::ResourceLimit {
            resource: "generated row-span transport attempts",
            ..
        })
    ));

    let mut search = InternalSymmetrySearchLimits::default();
    search.max_enumerated_matrices = 0;
    config = bounded_config();
    config.strategy = GeneratedSymbolicRowSpanStrategy::BoundedVacuumInternal {
        search,
        require_exhaustive: true,
    };
    assert!(matches!(
        GeneratedSymbolicRowSpanCompiler::compile(
            &family,
            &context,
            GeneratedWhenBadLimits::default().ibp,
            config,
        ),
        Err(GeneratedSymbolicRowSpanError::IncompleteRequiredSearch)
    ));

    config = bounded_config();
    config.limits.max_verified_symmetries = 0;
    assert!(matches!(
        GeneratedSymbolicRowSpanCompiler::compile(
            &family,
            &context,
            GeneratedWhenBadLimits::default().ibp,
            config,
        ),
        Err(GeneratedSymbolicRowSpanError::IncompleteRequiredSearch)
    ));
}

#[test]
fn two_loop_automatic_discovery_can_use_the_augmented_symbolic_row_span() {
    let family = equal_mass_sunset("generated-row-span-two-loop-discovery");
    let context = default_context(&family);
    let mut limits = GeneratedSectorDiscoveryLimits::default();
    limits.adaptive.max_search_depth = 0;
    limits.coverage.generated_when_bad.row_span = bounded_config();
    let certificate = GeneratedSectorDiscoveryCompiler::compile(
        &family,
        &context,
        SectorMask::try_new([true, true, true]).unwrap(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        limits,
    )
    .unwrap();
    assert_eq!(certificate.schema(), GENERATED_SECTOR_DISCOVERY_V2_SCHEMA);
    assert_eq!(certificate.stats().canonical_rows(), 4);
    assert!(certificate.stats().source_rows() > certificate.stats().canonical_rows());
    assert!(certificate.stats().transported_rows() > 0);
    assert_eq!(
        certificate.row_span().stats().augmented_rows(),
        certificate.stats().source_rows()
    );
    certificate.replay(&family, &context).unwrap();
}
