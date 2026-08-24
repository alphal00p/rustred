//! Black-box validation of deterministic, generated-source sector coverage.
//!
//! Every candidate below is obtained from RustRed's freshly generated
//! one-loop IBP identity.  The second valid candidate uses an exact translated
//! source row, exercising the same provenance path as an adaptive LiteRed
//! stencil without embedding the resulting recurrence coefficient.

use std::sync::Arc;

use rustred::{
    AffineDenominator, CoefficientContext, GeneratedWhenBadCompilation, GeneratedWhenBadLimits,
    IndexShift, IntegralFamily, IntegralOrderingPolicy, ParametricCoefficientContext,
    ParametricElimination, ParametricEliminationLimits, ParametricEliminationOrdering,
    ParametricIbpConfig, ParametricIbpGenerator, ParametricReductionRuleCandidate,
    ParametricRelation, ParametricRowId, ParametricRuleLimits, ParametricSectorCoverageCompiler,
    ParametricSectorCoverageError, ParametricSectorCoverageLimits, ParametricSectorLeafDisposition,
    SectorMask, WhenBadLeafDisposition,
};

fn family() -> IntegralFamily {
    unit_family("parametric-sector-coverage-one-loop")
}

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

fn guarded_family() -> IntegralFamily {
    let coefficients = CoefficientContext::new(["a", "d", "m2"]);
    IntegralFamily::new(
        "parametric-sector-coverage-one-loop-guarded",
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
    context: &rustred::ParametricCoefficientContext,
    rows: &[ParametricRelation],
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

fn generated_candidates(
    family: &IntegralFamily,
) -> (
    rustred::ParametricCoefficientContext,
    ParametricReductionRuleCandidate,
    ParametricReductionRuleCandidate,
) {
    let generated = ParametricIbpGenerator::try_new(family)
        .unwrap()
        .generate()
        .unwrap();
    let context = generated.context().clone();
    let canonical_rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    assert_eq!(canonical_rows.len(), 1);
    let sector = SectorMask::try_new([true]).unwrap();
    let canonical = candidate(&context, &canonical_rows, sector.clone(), 2);

    let translation = IndexShift::try_new([1], 1).unwrap();
    let translated_rows = canonical_rows
        .iter()
        .enumerate()
        .map(|(source, row)| {
            row.translated(
                &context,
                &translation,
                ParametricRowId::Derived {
                    label: Arc::from(format!(
                        "one-loop-sector-coverage-stencil-offset-one-source-{source}"
                    )),
                },
                GeneratedWhenBadLimits::default().ibp.arithmetic_limits,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let translated = candidate(&context, &translated_rows, sector, 2);
    (context, canonical, translated)
}

fn source_stats(
    attempt: &rustred::SectorCoverageCandidateAttempt,
) -> rustred::GeneratedSourceAuthenticationStats {
    match attempt.compilation() {
        GeneratedWhenBadCompilation::Certified(certificate) => {
            certificate.source_authentication().stats()
        }
        GeneratedWhenBadCompilation::Unsupported(unsupported) => {
            unsupported.source_authentication().stats()
        }
    }
}

#[test]
fn overlapping_generated_candidates_use_persisted_first_match_priority() {
    let family = family();
    let (context, canonical, translated) = generated_candidates(&family);
    let sector = SectorMask::try_new([true]).unwrap();

    let translated_first = ParametricSectorCoverageCompiler::compile(
        &family,
        &context,
        sector.clone(),
        &[translated.clone(), canonical.clone()],
        ParametricSectorCoverageLimits::default(),
    )
    .unwrap();
    translated_first.replay(&family, &context).unwrap();
    assert_eq!(translated_first.candidate_attempts().len(), 2);
    assert_eq!(translated_first.stats().certified_candidates(), 2);
    assert_eq!(translated_first.stats().unsupported_candidates(), 0);
    assert_eq!(translated_first.stats().unique_predicates(), 1);
    assert_eq!(translated_first.stats().global_leaves(), 2);
    assert_eq!(translated_first.stats().descending_leaves(), 1);
    assert_eq!(translated_first.stats().uncovered_leaves(), 1);
    assert_eq!(
        source_stats(&translated_first.candidate_attempts()[0]).translated_rows(),
        1
    );
    assert_eq!(
        source_stats(&translated_first.candidate_attempts()[1]).original_rows(),
        1
    );

    assert!(matches!(
        translated_first
            .classification_for_indices(&context, &[1])
            .unwrap()
            .unwrap()
            .disposition(),
        ParametricSectorLeafDisposition::Uncovered,
    ));
    for power in [2, 3, 41, i64::MAX] {
        assert!(matches!(
            translated_first
                .classification_for_indices(&context, &[power])
                .unwrap()
                .unwrap()
                .disposition(),
            ParametricSectorLeafDisposition::DescendingRule {
                candidate_ordinal: 0,
                ..
            },
        ));
    }
    assert!(
        translated_first
            .classification_for_indices(&context, &[0])
            .unwrap()
            .is_none()
    );

    let canonical_first = ParametricSectorCoverageCompiler::compile(
        &family,
        &context,
        sector,
        &[canonical, translated],
        ParametricSectorCoverageLimits::default(),
    )
    .unwrap();
    canonical_first.replay(&family, &context).unwrap();
    assert_eq!(
        source_stats(&canonical_first.candidate_attempts()[0]).original_rows(),
        1
    );
    assert_eq!(
        source_stats(&canonical_first.candidate_attempts()[1]).translated_rows(),
        1
    );
    assert!(matches!(
        canonical_first
            .classification_for_indices(&context, &[2])
            .unwrap()
            .unwrap()
            .disposition(),
        ParametricSectorLeafDisposition::DescendingRule {
            candidate_ordinal: 0,
            ..
        },
    ));
}

#[test]
fn empty_input_is_uncovered_and_unsupported_input_never_becomes_a_master() {
    let family = family();
    let (context, canonical, _) = generated_candidates(&family);
    let active = SectorMask::try_new([true]).unwrap();
    let empty = ParametricSectorCoverageCompiler::compile(
        &family,
        &context,
        active,
        &[],
        ParametricSectorCoverageLimits::default(),
    )
    .unwrap();
    empty.replay(&family, &context).unwrap();
    assert_eq!(empty.partition().cases().len(), 1);
    for power in [1, 2, i64::MAX] {
        assert!(matches!(
            empty
                .classification_for_indices(&context, &[power])
                .unwrap()
                .unwrap()
                .disposition(),
            ParametricSectorLeafDisposition::Uncovered,
        ));
    }

    let rows = canonical.derivation().source_rows();
    let inactive = SectorMask::try_new([false]).unwrap();
    let unsupported_candidate = candidate(&context, rows, inactive.clone(), 0);
    let unsupported = ParametricSectorCoverageCompiler::compile(
        &family,
        &context,
        inactive,
        &[unsupported_candidate],
        ParametricSectorCoverageLimits::default(),
    )
    .unwrap();
    unsupported.replay(&family, &context).unwrap();
    assert_eq!(unsupported.stats().certified_candidates(), 0);
    assert_eq!(unsupported.stats().unsupported_candidates(), 1);
    assert_eq!(unsupported.stats().unsupported_leaves(), 1);
    assert!(matches!(
        unsupported.candidate_attempts()[0].compilation(),
        GeneratedWhenBadCompilation::Unsupported(_),
    ));
    for power in [0, -1, i64::MIN] {
        assert!(matches!(
            unsupported
                .classification_for_indices(&context, &[power])
                .unwrap()
                .unwrap()
                .disposition(),
            ParametricSectorLeafDisposition::Unsupported {
                candidate_ordinals
            } if candidate_ordinals.as_ref() == [0],
        ));
    }
}

#[test]
fn nonunit_base_guards_remain_local_and_global_leaves_match_local_truth() {
    let family = guarded_family();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let context = generated.context().clone();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let sector = SectorMask::try_new([true]).unwrap();
    let candidate = candidate(&context, &rows, sector.clone(), 2);
    let coverage = ParametricSectorCoverageCompiler::compile(
        &family,
        &context,
        sector,
        &[candidate],
        ParametricSectorCoverageLimits::default(),
    )
    .unwrap();
    coverage.replay(&family, &context).unwrap();

    let GeneratedWhenBadCompilation::Certified(generated_candidate) =
        coverage.candidate_attempts()[0].compilation()
    else {
        panic!("the generated one-loop active-sector rule must be certified")
    };
    let local = generated_candidate.admissibility();
    assert!(
        local.base_domain_guards().next().is_some(),
        "the non-unit denominator basis must survive as an authenticated base-only assumption"
    );
    assert_eq!(local.stats().base_domain_guards(), 1);
    assert_eq!(
        coverage.stats().unique_predicates(),
        1,
        "a base-only field guard must not manufacture a lattice case split"
    );

    for power in 1..=64 {
        let local_disposition = local
            .classification_for_indices(&context, &[power])
            .unwrap()
            .unwrap()
            .disposition();
        let global_disposition = coverage
            .classification_for_indices(&context, &[power])
            .unwrap()
            .unwrap()
            .disposition();
        match local_disposition {
            WhenBadLeafDisposition::CoveredByCandidate => assert!(matches!(
                global_disposition,
                ParametricSectorLeafDisposition::DescendingRule {
                    candidate_ordinal: 0,
                    ..
                }
            )),
            WhenBadLeafDisposition::ExceptionalDomain { .. }
            | WhenBadLeafDisposition::ExceptionalSectorLeak { .. } => {
                assert!(matches!(
                    global_disposition,
                    ParametricSectorLeafDisposition::Uncovered
                ))
            }
        }
    }
}

#[test]
fn family_context_sector_and_arity_are_bound_before_empty_or_candidate_coverage() {
    let family = family();
    let (context, canonical, _) = generated_candidates(&family);
    let active = SectorMask::try_new([true]).unwrap();

    let foreign_family = unit_family("parametric-sector-coverage-foreign-family");
    let foreign_generated = ParametricIbpGenerator::try_new(&foreign_family)
        .unwrap()
        .generate()
        .unwrap();
    assert!(matches!(
        ParametricSectorCoverageCompiler::compile(
            &foreign_family,
            foreign_generated.context(),
            active.clone(),
            std::slice::from_ref(&canonical),
            ParametricSectorCoverageLimits::default(),
        ),
        Err(ParametricSectorCoverageError::CandidateWrongFamily { ordinal: 0 })
    ));

    let foreign_context = ParametricCoefficientContext::try_new(
        family.coefficient_context(),
        "parametric-sector-coverage-foreign-context",
        1,
    )
    .unwrap();
    assert!(matches!(
        ParametricSectorCoverageCompiler::compile(
            &family,
            &foreign_context,
            active.clone(),
            std::slice::from_ref(&canonical),
            ParametricSectorCoverageLimits::default(),
        ),
        Err(ParametricSectorCoverageError::CandidateWrongContext { ordinal: 0 })
    ));

    let foreign_scope_generated = ParametricIbpGenerator::try_with_context(
        &family,
        foreign_context.clone(),
        ParametricIbpConfig::default(),
    )
    .unwrap()
    .generate()
    .unwrap();
    let foreign_scope_rows = foreign_scope_generated
        .ibp_li()
        .cloned()
        .collect::<Vec<_>>();
    let foreign_scope_candidate =
        candidate(&foreign_context, &foreign_scope_rows, active.clone(), 2);
    assert!(matches!(
        ParametricSectorCoverageCompiler::compile(
            &family,
            &context,
            active.clone(),
            std::slice::from_ref(&foreign_scope_candidate),
            ParametricSectorCoverageLimits::default(),
        ),
        Err(ParametricSectorCoverageError::CandidateWrongContext { ordinal: 0 })
    ));

    // A caller-owned K(n) namespace is a supported generation mode. Its
    // generated candidate must authenticate and replay in that exact scope.
    let custom_scope = ParametricSectorCoverageCompiler::compile(
        &family,
        &foreign_context,
        active.clone(),
        &[foreign_scope_candidate],
        ParametricSectorCoverageLimits::default(),
    )
    .unwrap();
    custom_scope.replay(&family, &foreign_context).unwrap();
    assert!(matches!(
        custom_scope
            .classification_for_indices(&foreign_context, &[2])
            .unwrap()
            .unwrap()
            .disposition(),
        ParametricSectorLeafDisposition::DescendingRule {
            candidate_ordinal: 0,
            ..
        }
    ));

    // With no candidate, the certificate makes no identity claim: it records
    // only that the supplied search set leaves this orthant uncovered.
    let empty_custom_scope = ParametricSectorCoverageCompiler::compile(
        &family,
        &foreign_context,
        active.clone(),
        &[],
        ParametricSectorCoverageLimits::default(),
    )
    .unwrap();
    empty_custom_scope
        .replay(&family, &foreign_context)
        .unwrap();
    assert!(matches!(
        empty_custom_scope
            .classification_for_indices(&foreign_context, &[1])
            .unwrap()
            .unwrap()
            .disposition(),
        ParametricSectorLeafDisposition::Uncovered
    ));

    assert!(matches!(
        ParametricSectorCoverageCompiler::compile(
            &family,
            &context,
            SectorMask::try_new([true, false]).unwrap(),
            &[],
            ParametricSectorCoverageLimits::default(),
        ),
        Err(ParametricSectorCoverageError::WrongArity {
            expected: 1,
            actual: 2
        })
    ));

    assert!(matches!(
        ParametricSectorCoverageCompiler::compile(
            &family,
            &context,
            SectorMask::try_new([false]).unwrap(),
            &[canonical],
            ParametricSectorCoverageLimits::default(),
        ),
        Err(ParametricSectorCoverageError::CandidateWrongSector { ordinal: 0 })
    ));
}

#[test]
fn candidate_and_aggregate_coverage_budgets_fail_closed() {
    let family = family();
    let (context, canonical, translated) = generated_candidates(&family);
    let sector = SectorMask::try_new([true]).unwrap();

    let mut limits = ParametricSectorCoverageLimits::default();
    limits.max_candidates = 1;
    assert!(matches!(
        ParametricSectorCoverageCompiler::compile(
            &family,
            &context,
            sector.clone(),
            &[canonical.clone(), translated],
            limits,
        ),
        Err(ParametricSectorCoverageError::ResourceLimit {
            resource: "sector-coverage candidates",
            requested: 2,
            limit: 1,
        }),
    ));

    let mut limits = ParametricSectorCoverageLimits::default();
    limits.max_unique_predicates = 0;
    assert!(matches!(
        ParametricSectorCoverageCompiler::compile(
            &family,
            &context,
            sector.clone(),
            std::slice::from_ref(&canonical),
            limits,
        ),
        Err(ParametricSectorCoverageError::ResourceLimit {
            resource: "unique sector-coverage predicates",
            requested: 1,
            limit: 0,
        }),
    ));

    let mut limits = ParametricSectorCoverageLimits::default();
    limits.max_candidate_leaf_match_attempts = 0;
    assert!(matches!(
        ParametricSectorCoverageCompiler::compile(
            &family,
            &context,
            sector.clone(),
            std::slice::from_ref(&canonical),
            limits,
        ),
        Err(ParametricSectorCoverageError::ResourceLimit {
            resource: "sector-coverage candidate leaf match attempts",
            requested: 1,
            limit: 0,
        }),
    ));

    let mut limits = ParametricSectorCoverageLimits::default();
    limits.max_total_canonical_rows = 0;
    assert!(matches!(
        ParametricSectorCoverageCompiler::compile(&family, &context, sector, &[canonical], limits,),
        Err(ParametricSectorCoverageError::ResourceLimit {
            resource: "sector-coverage canonical rows",
            requested: 1,
            limit: 0,
        }),
    ));
}

#[test]
fn compiler_fresh_composition_matches_public_authenticated_rebuild() {
    let family = family();
    let (context, canonical, translated) = generated_candidates(&family);
    let sector = SectorMask::try_new([true]).unwrap();
    let limits = ParametricSectorCoverageLimits::default();
    let fresh = ParametricSectorCoverageCompiler::compile(
        &family,
        &context,
        sector.clone(),
        &[canonical, translated],
        limits,
    )
    .unwrap();

    // This public entry point treats every supplied compilation as untrusted:
    // it fully replays it, normalizes it onto one shared row span, and only
    // then reaches the private compiler-fresh composition path.
    let rebuilt = ParametricSectorCoverageCompiler::compose_authenticated(
        &family,
        &context,
        sector,
        fresh
            .candidate_attempts()
            .iter()
            .map(|attempt| attempt.compilation().clone())
            .collect(),
        limits,
    )
    .unwrap();

    assert_eq!(fresh.schema(), rebuilt.schema());
    assert_eq!(fresh.stats(), rebuilt.stats());
    assert_eq!(fresh.partition(), rebuilt.partition());
    assert_eq!(fresh.classifications(), rebuilt.classifications());
    assert_eq!(
        fresh
            .candidate_attempts()
            .iter()
            .map(source_stats)
            .collect::<Vec<_>>(),
        rebuilt
            .candidate_attempts()
            .iter()
            .map(source_stats)
            .collect::<Vec<_>>(),
    );
    fresh.replay(&family, &context).unwrap();
    rebuilt.replay(&family, &context).unwrap();
}

#[test]
fn public_authenticated_composition_rejects_wrong_sector_payload() {
    let family = family();
    let (context, canonical, _) = generated_candidates(&family);
    let inactive = SectorMask::try_new([false]).unwrap();
    let wrong_sector_candidate = candidate(
        &context,
        canonical.derivation().source_rows(),
        inactive.clone(),
        0,
    );
    let wrong_sector = ParametricSectorCoverageCompiler::compile(
        &family,
        &context,
        inactive,
        &[wrong_sector_candidate],
        ParametricSectorCoverageLimits::default(),
    )
    .unwrap();

    assert!(matches!(
        ParametricSectorCoverageCompiler::compose_authenticated(
            &family,
            &context,
            SectorMask::try_new([true]).unwrap(),
            vec![wrong_sector.candidate_attempts()[0].compilation().clone()],
            ParametricSectorCoverageLimits::default(),
        ),
        Err(ParametricSectorCoverageError::CandidateWrongSector { ordinal: 0 })
    ));
}
