//! Production-boundary one-loop `WhenBad` validation.
//!
//! These fixtures contain no recurrence coefficients.  Both candidates are
//! built by eliminating RustRed's freshly generated parametric IBP row for
//! the massive one-loop vacuum family.  The active-sector fixture is the
//! exact Symbolica counterpart of the range split in Vakint/alphaLoop's
//! checked-in `IntegrateUV1L`: `n = 1` is the master boundary and `n >= 2`
//! is covered by the descending recurrence.  The inactive-sector fixture
//! deliberately asks the same authenticated pivot to ascend on the numerator
//! orthant and therefore must remain `Unsupported`.

use rustred::{
    AffineDenominator, CoefficientContext, GeneratedWhenBadCompilation, GeneratedWhenBadCompiler,
    GeneratedWhenBadLimits, GeneratedWhenBadSourceAuthentication, IntegralFamily,
    IntegralOrderingPolicy, ParametricElimination, ParametricEliminationLimits,
    ParametricEliminationOrdering, ParametricIbpGenerator, ParametricReductionRuleCandidate,
    ParametricRelation, ParametricRuleLimits, SectorMask, WhenBadLeafDisposition,
    WhenBadUnsupportedReason,
};

fn one_loop_massive_vacuum() -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        "generated-when-bad-one-loop-orthant",
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

fn generated_candidate(
    rows: &[ParametricRelation],
    context: &rustred::ParametricCoefficientContext,
    sector: SectorMask,
    anchor: i64,
) -> ParametricReductionRuleCandidate {
    let elimination = ParametricElimination::build(
        context,
        rows,
        ParametricEliminationOrdering::try_new(
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            [anchor],
        )
        .unwrap(),
        ParametricEliminationLimits::default(),
    )
    .unwrap();
    ParametricReductionRuleCandidate::try_from_elimination_pivot(
        context,
        rows,
        &elimination,
        0,
        sector,
        ParametricRuleLimits::default(),
    )
    .unwrap()
}

#[test]
fn generated_one_loop_rule_certifies_exactly_n_ge_two_on_the_full_active_orthant() {
    let family = one_loop_massive_vacuum();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    assert_eq!(rows.len(), 1);
    let candidate = generated_candidate(
        &rows,
        generated.context(),
        SectorMask::try_new([true]).unwrap(),
        2,
    );

    let compilation = GeneratedWhenBadCompiler::compile(
        &family,
        generated.context(),
        &candidate,
        GeneratedWhenBadLimits::default(),
    )
    .unwrap();
    let GeneratedWhenBadCompilation::Certified(certificate) = compilation else {
        panic!("the generated one-loop active-sector recurrence must be certifiable");
    };
    certificate.replay(&family, generated.context()).unwrap();
    assert_eq!(
        certificate.source_authentication().source_authentication(),
        GeneratedWhenBadSourceAuthentication::CanonicalIbpLiAndExactTranslations,
    );

    let admissibility = certificate.admissibility();
    assert_eq!(
        admissibility.partition().orthant().sector().active_bits(),
        &[true]
    );
    // The inherited pivot guard and the solved coefficient denominator are
    // opposite associates. Both provenance-bearing conditions are retained,
    // while the bounded K*-associate proof routes them through one locus.
    assert_eq!(admissibility.domain_conditions().len(), 2);
    assert_eq!(admissibility.partition().cases().len(), 2);
    assert_eq!(admissibility.partition().stats().split_count(), 1);
    assert_eq!(admissibility.classifications().len(), 2);
    assert!(matches!(
        admissibility.classifications()[0].disposition(),
        WhenBadLeafDisposition::ExceptionalDomain { condition: 0 },
    ));
    assert!(matches!(
        admissibility.classifications()[1].disposition(),
        WhenBadLeafDisposition::CoveredByCandidate,
    ));
    assert_eq!(admissibility.stats().index_domain_guards(), 2);
    assert_eq!(admissibility.stats().base_domain_guards(), 0);
    assert_eq!(admissibility.stats().leak_events(), 0);

    assert!(matches!(
        admissibility
            .classification_for_indices(generated.context(), &[1])
            .unwrap()
            .unwrap()
            .disposition(),
        WhenBadLeafDisposition::ExceptionalDomain { condition: 0 },
    ));
    for power in [2, 3, 17, i64::MAX] {
        assert!(matches!(
            admissibility
                .classification_for_indices(generated.context(), &[power])
                .unwrap()
                .unwrap()
                .disposition(),
            WhenBadLeafDisposition::CoveredByCandidate,
        ));
    }
    assert!(
        admissibility
            .classification_for_indices(generated.context(), &[0])
            .unwrap()
            .is_none(),
        "the active-sector certificate must not claim the inactive orthant",
    );
}

#[test]
fn authenticated_generated_candidate_is_unsupported_when_it_ascends_uniformly() {
    let family = one_loop_massive_vacuum();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let candidate = generated_candidate(
        &rows,
        generated.context(),
        SectorMask::try_new([false]).unwrap(),
        0,
    );

    let compilation = GeneratedWhenBadCompiler::compile(
        &family,
        generated.context(),
        &candidate,
        GeneratedWhenBadLimits::default(),
    )
    .unwrap();
    let GeneratedWhenBadCompilation::Unsupported(unsupported) = compilation else {
        panic!("an outward recurrence on n <= 0 must not be certified");
    };
    unsupported.replay(&family, generated.context()).unwrap();
    assert_eq!(
        unsupported.source_authentication().source_authentication(),
        GeneratedWhenBadSourceAuthentication::CanonicalIbpLiAndExactTranslations,
    );
    assert!(matches!(
        unsupported.admissibility().reason(),
        WhenBadUnsupportedReason::NonUniformSameSectorDescent {
            rhs_ordinal: 0,
            delta: 1,
            ..
        },
    ));
}
