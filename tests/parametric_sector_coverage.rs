use rustred::{
    AffineDenominator, CoefficientContext, GeneratedWhenBadCompilation, IntegralFamily,
    IntegralOrderingPolicy, ParametricCoefficientContext, ParametricElimination,
    ParametricEliminationLimits, ParametricEliminationOrdering, ParametricIbpGenerator,
    ParametricReductionRuleCandidate, ParametricRelation, ParametricRuleLimits,
    ParametricSectorCoverageCompiler, ParametricSectorCoverageError,
    ParametricSectorCoverageLimits, ParametricSectorLeafDisposition, SectorMask,
};

fn one_loop_family() -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        "sector-coverage-one-loop-oracle",
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

fn candidate_at_anchor(
    context: &ParametricCoefficientContext,
    rows: &[ParametricRelation],
    anchor: i64,
    sector: SectorMask,
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
fn empty_authenticated_search_is_explicitly_uncovered_never_a_master() {
    let family = one_loop_family();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let sector = SectorMask::try_new([true]).unwrap();
    let certificate = ParametricSectorCoverageCompiler::compile(
        &family,
        generated.context(),
        sector,
        &[],
        ParametricSectorCoverageLimits::default(),
    )
    .unwrap();

    assert!(certificate.candidate_attempts().is_empty());
    assert_eq!(certificate.classifications().len(), 1);
    assert_eq!(
        certificate.classifications()[0].disposition(),
        &ParametricSectorLeafDisposition::Uncovered
    );
    assert_eq!(certificate.stats().uncovered_leaves(), 1);
    assert!(matches!(
        certificate
            .classification_for_indices(generated.context(), &[1])
            .unwrap()
            .unwrap()
            .disposition(),
        ParametricSectorLeafDisposition::Uncovered
    ));
    assert!(
        certificate
            .classification_for_indices(generated.context(), &[0])
            .unwrap()
            .is_none()
    );
    certificate.replay(&family, generated.context()).unwrap();

    let foreign = ParametricCoefficientContext::try_new(
        family.coefficient_context(),
        "foreign-empty-coverage-scope",
        family.denominator_count(),
    )
    .unwrap();
    let foreign_empty = ParametricSectorCoverageCompiler::compile(
        &family,
        &foreign,
        SectorMask::try_new([true]).unwrap(),
        &[],
        ParametricSectorCoverageLimits::default(),
    )
    .unwrap();
    foreign_empty.replay(&family, &foreign).unwrap();
    assert!(matches!(
        foreign_empty
            .classification_for_indices(&foreign, &[1])
            .unwrap()
            .unwrap()
            .disposition(),
        ParametricSectorLeafDisposition::Uncovered
    ));
}

#[test]
fn generated_one_loop_rule_covers_positive_integer_oracle_points() {
    let family = one_loop_family();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let sector = SectorMask::try_new([true]).unwrap();
    let candidate = candidate_at_anchor(generated.context(), &rows, 2, sector.clone());
    let certificate = ParametricSectorCoverageCompiler::compile(
        &family,
        generated.context(),
        sector,
        &[candidate],
        ParametricSectorCoverageLimits::default(),
    )
    .unwrap();

    assert_eq!(certificate.stats().candidates(), 1);
    assert_eq!(certificate.stats().certified_candidates(), 1);
    assert!(matches!(
        certificate
            .classification_for_indices(generated.context(), &[1])
            .unwrap()
            .unwrap()
            .disposition(),
        ParametricSectorLeafDisposition::Uncovered
    ));
    for power in 2..=8 {
        assert!(matches!(
            certificate
                .classification_for_indices(generated.context(), &[power])
                .unwrap()
                .unwrap()
                .disposition(),
            ParametricSectorLeafDisposition::DescendingRule {
                candidate_ordinal: 0,
                ..
            }
        ));
    }
    // The generated recurrence is exceptional at the inhabited n=1 boundary.
    // This composition layer leaves it uncovered: an explicit later master
    // policy or another proof is required to classify it as a master.
    assert!(certificate.stats().uncovered_leaves() >= 1);
    certificate.replay(&family, generated.context()).unwrap();
}

#[test]
fn overlapping_generated_candidates_use_stable_input_priority() {
    let family = one_loop_family();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let sector = SectorMask::try_new([true]).unwrap();
    let candidate = candidate_at_anchor(generated.context(), &rows, 2, sector.clone());
    let certificate = ParametricSectorCoverageCompiler::compile(
        &family,
        generated.context(),
        sector,
        &[candidate.clone(), candidate],
        ParametricSectorCoverageLimits::default(),
    )
    .unwrap();

    assert_eq!(certificate.candidate_attempts().len(), 2);
    assert_eq!(certificate.stats().certified_candidates(), 2);
    assert!(matches!(
        certificate
            .classification_for_indices(generated.context(), &[3])
            .unwrap()
            .unwrap()
            .disposition(),
        ParametricSectorLeafDisposition::DescendingRule {
            candidate_ordinal: 0,
            ..
        }
    ));
    certificate.replay(&family, generated.context()).unwrap();
}

#[test]
fn generated_but_nonuniform_candidate_is_terminal_unsupported_not_master() {
    let family = one_loop_family();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let sector = SectorMask::try_new([true]).unwrap();
    // At a negative discovery anchor the exact elimination chooses I(n) over
    // I(n+1). Centering that pivot yields an outward shift in this positive
    // sector, so WhenBad correctly refuses a uniform descending rule.
    let candidate = candidate_at_anchor(generated.context(), &rows, -2, sector.clone());
    let certificate = ParametricSectorCoverageCompiler::compile(
        &family,
        generated.context(),
        sector,
        &[candidate],
        ParametricSectorCoverageLimits::default(),
    )
    .unwrap();

    assert!(matches!(
        certificate.candidate_attempts()[0].compilation(),
        GeneratedWhenBadCompilation::Unsupported(_)
    ));
    assert!(matches!(
        certificate.classifications()[0].disposition(),
        ParametricSectorLeafDisposition::Unsupported { candidate_ordinals }
            if candidate_ordinals.as_ref() == [0]
    ));
    certificate.replay(&family, generated.context()).unwrap();
}

#[test]
fn later_certified_candidate_covers_before_terminal_unsupported_is_chosen() {
    let family = one_loop_family();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let sector = SectorMask::try_new([true]).unwrap();
    let unsupported = candidate_at_anchor(generated.context(), &rows, -2, sector.clone());
    let certified = candidate_at_anchor(generated.context(), &rows, 2, sector.clone());
    let certificate = ParametricSectorCoverageCompiler::compile(
        &family,
        generated.context(),
        sector,
        &[unsupported, certified],
        ParametricSectorCoverageLimits::default(),
    )
    .unwrap();

    assert!(matches!(
        certificate.candidate_attempts()[0].compilation(),
        GeneratedWhenBadCompilation::Unsupported(_)
    ));
    assert!(matches!(
        certificate
            .classification_for_indices(generated.context(), &[2])
            .unwrap()
            .unwrap()
            .disposition(),
        ParametricSectorLeafDisposition::DescendingRule {
            candidate_ordinal: 1,
            ..
        }
    ));
    assert!(matches!(
        certificate
            .classification_for_indices(generated.context(), &[1])
            .unwrap()
            .unwrap()
            .disposition(),
        ParametricSectorLeafDisposition::Unsupported { candidate_ordinals }
            if candidate_ordinals.as_ref() == [0]
    ));
    certificate.replay(&family, generated.context()).unwrap();
}

#[test]
fn candidate_and_global_proof_budgets_fail_closed() {
    let family = one_loop_family();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let sector = SectorMask::try_new([true]).unwrap();
    let candidate = candidate_at_anchor(generated.context(), &rows, 2, sector.clone());

    let mut limits = ParametricSectorCoverageLimits::default();
    limits.max_candidates = 0;
    assert!(matches!(
        ParametricSectorCoverageCompiler::compile(
            &family,
            generated.context(),
            sector.clone(),
            std::slice::from_ref(&candidate),
            limits,
        ),
        Err(ParametricSectorCoverageError::ResourceLimit {
            resource: "sector-coverage candidates",
            requested: 1,
            limit: 0,
        })
    ));

    let mut limits = ParametricSectorCoverageLimits::default();
    limits.max_global_leaf_classifications = 1;
    assert!(matches!(
        ParametricSectorCoverageCompiler::compile(
            &family,
            generated.context(),
            sector,
            &[candidate],
            limits,
        ),
        Err(ParametricSectorCoverageError::SectorCase(_))
    ));

    let mut limits = ParametricSectorCoverageLimits::default();
    limits.sector_cases.exact_algebra.max_polynomial_terms -= 1;
    assert!(matches!(
        ParametricSectorCoverageCompiler::compile(
            &family,
            generated.context(),
            SectorMask::try_new([true]).unwrap(),
            &[],
            limits,
        ),
        Err(ParametricSectorCoverageError::InconsistentLimits {
            first: "generated WhenBad arithmetic exact algebra",
            second: "global sector-case exact algebra",
        })
    ));
}
