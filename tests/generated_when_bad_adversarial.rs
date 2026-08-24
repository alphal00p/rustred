use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use rustred::{
    AffineDenominator, CoefficientContext, GeneratedSourceAuthenticator, GeneratedSourceRowMode,
    GeneratedWhenBadError, GeneratedWhenBadLimits, IndexShift, IntegralFamily,
    IntegralOrderingPolicy, ParametricCoefficientContext, ParametricCoefficientError,
    ParametricElimination, ParametricEliminationLimits, ParametricEliminationOrdering,
    ParametricIbpGenerator, ParametricReductionRuleCandidate, ParametricRelation,
    ParametricRelationError, ParametricRowId, ParametricRuleLimits, SectorMask,
};

fn unit_family(name: &str) -> IntegralFamily {
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

fn guarded_family(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["a", "d", "m2"]);
    IntegralFamily::new(
        name,
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            coefficients.parse("-m2").unwrap(),
            vec![coefficients.parameter("a").unwrap()],
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
fn family_and_context_fingerprints_are_checked_before_row_matching() {
    let family = unit_family("generated-auth-family-binding");
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let candidate = candidate(generated.context(), &rows);
    let foreign_family = unit_family("generated-auth-foreign-family");
    assert!(matches!(
        GeneratedSourceAuthenticator::authenticate(
            &foreign_family,
            generated.context(),
            &candidate,
            GeneratedWhenBadLimits::default(),
        ),
        Err(GeneratedWhenBadError::WrongFamily)
    ));

    let foreign_context = ParametricCoefficientContext::try_new(
        family.coefficient_context(),
        "generated-auth-foreign-context",
        1,
    )
    .unwrap();
    assert!(matches!(
        GeneratedSourceAuthenticator::authenticate(
            &family,
            &foreign_context,
            &candidate,
            GeneratedWhenBadLimits::default(),
        ),
        Err(GeneratedWhenBadError::WrongContext)
    ));
    candidate.replay_retained(generated.context()).unwrap();
}

#[test]
fn a_zero_offset_relabel_is_accepted_only_after_exact_translation_replay() {
    let family = unit_family("generated-auth-zero-translation");
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let zero = IndexShift::try_new([0], 1).unwrap();
    let relabeled = generated.ordinary_ibp()[0]
        .translated(
            generated.context(),
            &zero,
            ParametricRowId::Derived {
                label: Arc::from("untrusted-zero-offset-label"),
            },
            GeneratedWhenBadLimits::default().ibp.arithmetic_limits,
        )
        .unwrap();
    assert_ne!(relabeled.row_id(), generated.ordinary_ibp()[0].row_id());
    let candidate = candidate(generated.context(), &[relabeled]);
    let source = GeneratedSourceAuthenticator::authenticate(
        &family,
        generated.context(),
        &candidate,
        GeneratedWhenBadLimits::default(),
    )
    .unwrap();
    assert_eq!(source.witnesses().len(), 1);
    assert_eq!(
        source.witnesses()[0].mode(),
        GeneratedSourceRowMode::ExactTranslation
    );
    assert_eq!(source.witnesses()[0].translation().values(), &[0]);
    source
        .replay(&family, generated.context(), &candidate)
        .unwrap();
}

#[test]
fn mathematically_identical_translation_with_forged_guard_origins_is_rejected() {
    let family = guarded_family("generated-auth-guard-provenance");
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let offset = IndexShift::try_new([2], 1).unwrap();
    let row_id = ParametricRowId::Derived {
        label: Arc::from("translated-but-guard-origins-forged"),
    };
    let exact = generated.ordinary_ibp()[0]
        .translated(
            generated.context(),
            &offset,
            row_id.clone(),
            GeneratedWhenBadLimits::default().ibp.arithmetic_limits,
        )
        .unwrap();
    assert!(
        !exact.guarded_nonzero_conditions().is_empty(),
        "the non-unit denominator basis must provide a provenance-bearing guard"
    );

    // Rebuild the same mathematical sparse row and polynomial guard set
    // through the public explicit-condition API. This deliberately replaces
    // generator/translation origins by forged explicit-condition origins.
    let mut forged = ParametricRelation::new(family.fingerprint(), row_id, generated.context());
    for polynomial in exact.nonzero_conditions() {
        forged
            .add_nonzero_condition(generated.context(), polynomial.clone())
            .unwrap();
    }
    for (shift, coefficient) in exact.terms() {
        forged
            .add_term(generated.context(), shift.clone(), coefficient.clone())
            .unwrap();
    }
    assert_eq!(
        forged, exact,
        "only provenance should differ in this attack"
    );
    assert!(!forged.has_identical_guard_provenance(&exact));

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
fn aggregate_source_limits_fail_closed_and_leave_the_candidate_replayable() {
    let family = unit_family("generated-auth-limits");
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let candidate = candidate(generated.context(), &rows);

    let checks: [(fn(&mut GeneratedWhenBadLimits), &str); 7] = [
        (
            |limits| limits.max_canonical_rows = 0,
            "canonical generated IBP/LI rows",
        ),
        (
            |limits| limits.max_retained_rows = 0,
            "retained parametric source rows",
        ),
        (
            |limits| limits.max_canonical_terms = 0,
            "canonical generated IBP/LI terms",
        ),
        (
            |limits| limits.max_retained_terms = 0,
            "retained parametric source terms",
        ),
        (
            |limits| limits.max_match_attempts = 0,
            "generated-source match attempts",
        ),
        (
            |limits| limits.max_translation_components = 0,
            "generated-source translation components",
        ),
        (
            |limits| limits.max_source_manifest_bytes = 0,
            "candidate source manifest bytes",
        ),
    ];
    for (configure, expected_resource) in checks {
        let mut limits = GeneratedWhenBadLimits::default();
        configure(&mut limits);
        assert!(matches!(
            GeneratedSourceAuthenticator::authenticate(
                &family,
                generated.context(),
                &candidate,
                limits,
            ),
            Err(GeneratedWhenBadError::ResourceLimit { resource, .. })
                if resource == expected_resource
        ));
        candidate.replay_retained(generated.context()).unwrap();
    }
}

#[test]
fn near_extreme_exact_translation_is_panic_contained_at_default_limits() {
    let family = unit_family("generated-auth-extreme-translation");
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    // i64::MIN itself cannot always be negated while centering a candidate;
    // MIN+1 still exercises 63-bit translation coefficients and has a checked
    // additive inverse.
    let offset = IndexShift::try_new([i64::MIN + 1], 1).unwrap();
    let translated = catch_unwind(AssertUnwindSafe(|| {
        generated.ordinary_ibp()[0].translated(
            generated.context(),
            &offset,
            ParametricRowId::Derived {
                label: Arc::from("near-i64-min-translation"),
            },
            GeneratedWhenBadLimits::default().ibp.arithmetic_limits,
        )
    }))
    .expect("checked translation must not unwind through the public API")
    .unwrap();
    let candidate = candidate(generated.context(), &[translated]);
    let source = catch_unwind(AssertUnwindSafe(|| {
        GeneratedSourceAuthenticator::authenticate(
            &family,
            generated.context(),
            &candidate,
            GeneratedWhenBadLimits::default(),
        )
    }))
    .expect("generated-source authentication must not unwind")
    .unwrap();
    assert_eq!(source.witnesses()[0].translation(), &offset);
    assert_eq!(
        source.witnesses()[0].mode(),
        GeneratedSourceRowMode::ExactTranslation
    );
    source
        .replay(&family, generated.context(), &candidate)
        .unwrap();
}

#[test]
fn probe_translation_integer_growth_obeys_configured_bit_budget() {
    let family = unit_family("generated-auth-translation-bit-probe");
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let offset = IndexShift::try_new([i64::MIN + 1], 1).unwrap();
    let translated = generated.ordinary_ibp()[0]
        .translated(
            generated.context(),
            &offset,
            ParametricRowId::Derived {
                label: Arc::from("translation-bit-budget-probe"),
            },
            GeneratedWhenBadLimits::default().ibp.arithmetic_limits,
        )
        .unwrap();
    let candidate = candidate(generated.context(), &[translated]);
    let mut limits = GeneratedWhenBadLimits::default();
    limits.ibp.arithmetic_limits.max_specialization_integer_bits = 1;
    assert!(matches!(
        GeneratedSourceAuthenticator::authenticate(
            &family,
            generated.context(),
            &candidate,
            limits,
        ),
        Err(GeneratedWhenBadError::Relation(
            ParametricRelationError::Coefficient(ParametricCoefficientError::ResourceLimit {
                resource: "parametric translation integer bits",
                limit: 1,
                ..
            })
        ))
    ));
}
