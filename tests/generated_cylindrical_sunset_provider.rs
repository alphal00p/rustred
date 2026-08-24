//! Authentic connected two-loop probe of the anchor-free cylindrical path.
//!
//! The equal-mass sunset is a validation topology only. Production receives
//! no recurrence, expected coefficient, preferred pivot, master count, or
//! loop-count dispatch. Every persistent Global pivot is compiled in its
//! authenticated ordinal order and submitted to the shared generated
//! `WhenBad` and ordered product-free coverage layers.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use rustred::{
    AdaptiveParametricRuleProvider, AdaptiveRuleSearchLimits, AffineDenominator,
    CertifiedConcreteRewriteProof, CertifiedFamilyRuleProvider, CertifiedFamilyRuleProviderLimits,
    CoefficientContext, ConcreteIntegralKey, ConcreteReduction, ConcreteRuleApplicationTrace,
    ConcreteRuleDecision, ConcreteRuleProvider, FamilySectorInventoryCompiler,
    FamilySectorInventoryLimits, GeneratedCylindricalCandidateAuthorityLimits,
    GeneratedCylindricalPersistentEliminationCertificate,
    GeneratedCylindricalPersistentEliminationLimits, GeneratedCylindricalRowSystemCertificate,
    GeneratedCylindricalRowSystemLimits, GeneratedCylindricalSectorCoverageCompiler,
    GeneratedCylindricalSectorCoverageLimits, GeneratedCylindricalSectorLeafDisposition,
    GeneratedCylindricalSectorRootStartCertificate, GeneratedCylindricalSectorRootStartLimits,
    GeneratedCylindricalSectorRuleProvider, GeneratedCylindricalSectorRuleProviderLimits,
    GeneratedSymbolicRowSpanConfig, IntegralFamily, IntegralOrderingPolicy,
    InternalSymmetrySearchLimits, MasterPolicyProvider, ParametricIbpConfig,
    ParametricIbpGenerator, ParametricReductionEngine, PowerShiftPolicy, ReductionEngineLimits,
    SectorMask, SectorRestrictions, WhenBadCompilerLimits,
    discover_bounded_vacuum_internal_symmetries,
};

const SUNSET_CYLINDRICAL_THROUGH_DEPTH: usize = 1;

fn timed_stage<T>(label: impl AsRef<str>, stage: impl FnOnce() -> T) -> T {
    let label = label.as_ref();
    eprintln!("SUNSET_STAGE_BEGIN {label}");
    let started = Instant::now();
    let output = stage();
    eprintln!(
        "SUNSET_STAGE_END {label} elapsed_ms={}",
        started.elapsed().as_millis()
    );
    output
}

fn equal_mass_sunset() -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let zero = coefficients.zero();
    let one = coefficients.one();
    let minus_m2 = coefficients.parse("-m2").unwrap();
    IntegralFamily::new(
        "generated-cylindrical-provider-connected-sunset",
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

fn key(powers: [i64; 3]) -> ConcreteIntegralKey {
    ConcreteIntegralKey::try_new(powers).unwrap()
}

#[test]
fn all_generated_global_pivots_cover_sunset_j211_and_numeric_quotient_closes_it() {
    let family = equal_mass_sunset();
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let sector = SectorMask::try_new([true, true, true]).unwrap();
    let ordering = IntegralOrderingPolicy::RustRedUnshiftedV1;

    let inventory = timed_stage("inventory/compile", || {
        Arc::new(
            FamilySectorInventoryCompiler::compile(
                &family,
                SectorRestrictions::unrestricted(family.denominator_count()).unwrap(),
                PowerShiftPolicy::FormalGeneric,
                ordering,
                FamilySectorInventoryLimits::default(),
            )
            .unwrap(),
        )
    });
    let root = timed_stage("sector-root/compile", || {
        Arc::new(
            GeneratedCylindricalSectorRootStartCertificate::compile(
                &family,
                &context,
                inventory,
                sector.clone(),
                ParametricIbpConfig::default(),
                GeneratedSymbolicRowSpanConfig::default(),
                SUNSET_CYLINDRICAL_THROUGH_DEPTH,
                GeneratedCylindricalSectorRootStartLimits::default(),
            )
            .unwrap(),
        )
    });
    assert!(root.assignment().is_empty());
    assert_eq!(root.row_span().rows().len(), 4, "L(L+E)=4 native IBPs");

    let rows = timed_stage("row-system/compile", || {
        Arc::new(
            GeneratedCylindricalRowSystemCertificate::compile_from_sector_root(
                &family,
                &context,
                root,
                GeneratedCylindricalRowSystemLimits::default(),
            )
            .unwrap(),
        )
    });
    let persistent = timed_stage("persistent/compile", || {
        Arc::new(
            GeneratedCylindricalPersistentEliminationCertificate::compile(
                &family,
                &context,
                rows,
                GeneratedCylindricalPersistentEliminationLimits::default(),
            )
            .unwrap(),
        )
    });
    let persistent_stats = persistent.stats();
    eprintln!(
        "SUNSET_PERSISTENT_STATS retained_source_rows={} elimination_builds={} pivot_rows={} elimination_source_rows={}",
        persistent_stats.retained_source_rows(),
        persistent_stats.elimination_builds(),
        persistent_stats.pivot_rows(),
        persistent_stats.elimination_source_rows(),
    );

    let pivot_ordinals = persistent
        .guarded_pivots()
        .map(|pivot| pivot.ordinal())
        .collect::<Vec<_>>();
    assert!(
        !pivot_ordinals.is_empty(),
        "sunset source retained no pivots"
    );
    assert!(
        pivot_ordinals
            .iter()
            .enumerate()
            .all(|(ordinal, &pivot)| ordinal == pivot),
        "guarded pivots must be visited in their authenticated deterministic order"
    );
    eprintln!(
        "SUNSET_PIVOT_STATS pivot_count={} pivot_ordinals={pivot_ordinals:?}",
        pivot_ordinals.len()
    );

    let coverage = timed_stage("coverage/compile-all-persistent-pivots", || {
        GeneratedCylindricalSectorCoverageCompiler::compile_from_persistent_source(
            &family,
            &context,
            Arc::clone(&persistent),
            GeneratedCylindricalCandidateAuthorityLimits::default(),
            WhenBadCompilerLimits::default(),
            GeneratedCylindricalSectorCoverageLimits::default(),
        )
        .unwrap()
    });
    let coverage_stats = coverage.stats();
    let certified_attempts = coverage_stats.certified_attempts();
    let unsupported_attempts = coverage_stats.unsupported_attempts();
    assert!(Arc::ptr_eq(
        coverage
            .batch_provenance()
            .expect("sunset coverage must retain exhaustive source provenance")
            .source(),
        &persistent,
    ));
    assert_eq!(coverage.stats().attempts(), pivot_ordinals.len());
    assert_eq!(coverage.stats().certified_attempts(), certified_attempts);
    assert_eq!(
        coverage.stats().unsupported_attempts(),
        unsupported_attempts
    );

    let target = key([2, 1, 1]);
    let (candidate_ordinal, candidate) = timed_stage("coverage/classify-j211", || {
        let classification = coverage
            .classification_for_indices(&context, target.powers())
            .unwrap()
            .expect("J(2,1,1) lies in the sunset top-sector orthant");
        let GeneratedCylindricalSectorLeafDisposition::DescendingRule {
            candidate_ordinal,
            candidate,
        } = classification
        else {
            panic!(
                "the exhaustive generated cylindrical source must cover sunset J(2,1,1), got {classification:?}"
            )
        };
        (candidate_ordinal, Arc::clone(candidate))
    });

    let mut provider = timed_stage("provider/construct", || {
        GeneratedCylindricalSectorRuleProvider::try_new(
            &family,
            &context,
            ordering,
            [coverage],
            GeneratedCylindricalSectorRuleProviderLimits::default(),
        )
        .unwrap()
    });

    let direct = timed_stage("application/direct", || {
        ConcreteReduction::apply_generated_cylindrical(
            Arc::clone(&candidate),
            &context,
            target.powers(),
        )
        .unwrap()
    });
    let ConcreteRuleDecision::Reduction(reduction) = timed_stage("application/provider", || {
        provider.decision_for(&target).unwrap()
    }) else {
        panic!("covered sunset J(2,1,1) did not publish its concrete reduction")
    };
    assert_eq!(reduction.source(), &target);
    assert_eq!(reduction.pivot_ordinal(), candidate_ordinal);
    assert_eq!(reduction.rhs(), direct.rhs());
    assert_eq!(reduction.descent_witnesses(), direct.descent_witnesses());
    assert!(
        reduction
            .specialized_relation()
            .has_identical_guard_provenance(direct.specialized_relation())
    );
    assert_eq!(
        reduction.rhs(),
        &BTreeMap::from([
            (
                key([0, 1, 2]),
                family.coefficient_context().parse("-1/(2*m2)").unwrap()
            ),
            (
                key([1, 0, 2]),
                family.coefficient_context().parse("1/(2*m2)").unwrap()
            ),
            (
                key([1, 1, 1]),
                family.coefficient_context().parse("(d-3)/(2*m2)").unwrap()
            ),
            (
                key([1, 1, 2]),
                family.coefficient_context().parse("-1/2").unwrap()
            ),
        ]),
        "generated raw relation differs from the frozen Vakint/LiteRed sunset oracle",
    );
    assert!(reduction.verify_descent(ordering));
    assert!(std::ptr::eq(
        reduction
            .generated_cylindrical_certificate()
            .expect("covered sunset rule must retain cylindrical provenance"),
        candidate.as_ref(),
    ));
    assert!(timed_stage("application/replay", || {
        reduction.replay_application(&family, &context).unwrap()
    }));
    eprintln!(
        "covered generated cylindrical sunset J(2,1,1): pivot={}, rhs={:?}, guards={:?}, certified_attempts={}, unsupported_attempts={}",
        candidate_ordinal,
        reduction.rhs(),
        reduction.required_nonzero(),
        certified_attempts,
        unsupported_attempts,
    );
    timed_stage("provider/final-replay", || provider.replay().unwrap());

    // LiteRed applies zero and self-symmetry relations at numeric prepare
    // points before choosing a pivot. Reuse the same authenticated persistent
    // translated-row source and require that generic quotient/re-elimination
    // closes the raw relation, rather than recursively canonicalizing its RHS
    // into an active-key cycle.
    let generated = ParametricIbpGenerator::try_with_context(
        &family,
        context.clone(),
        ParametricIbpConfig::default(),
    )
    .unwrap()
    .generate()
    .unwrap();
    let canonical_rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let restrictions = SectorRestrictions::unrestricted(family.denominator_count()).unwrap();
    let symmetry_report = discover_bounded_vacuum_internal_symmetries(
        &family,
        &restrictions,
        InternalSymmetrySearchLimits::default(),
    )
    .unwrap();
    assert!(symmetry_report.completion().is_exhaustive_within_bounds());
    assert_eq!(symmetry_report.symmetries().len(), 6);
    let mut adaptive_limits = AdaptiveRuleSearchLimits::default();
    // The accepted proof below must be the persistent-source arm. Keep only a
    // minimal ordinary fallback so the test cannot silently pass through the
    // older cumulative depth-one scout implementation.
    adaptive_limits.max_search_depth = 0;
    let adaptive = AdaptiveParametricRuleProvider::try_new(
        generated.context(),
        &canonical_rows,
        ordering,
        adaptive_limits,
    )
    .unwrap();
    let quotient_provider =
        CertifiedFamilyRuleProvider::try_new_with_persistent_cylindrical_source(
            family.clone(),
            restrictions,
            symmetry_report.symmetries().iter().cloned(),
            adaptive,
            Arc::clone(&persistent),
            ordering,
            CertifiedFamilyRuleProviderLimits::default(),
        )
        .unwrap();
    assert!(Arc::ptr_eq(
        quotient_provider
            .persistent_cylindrical_source()
            .expect("the provider must retain its exact persistent source"),
        &persistent,
    ));
    let quotient_provider =
        MasterPolicyProvider::with_selected(quotient_provider, [key([1, 1, 1])]).unwrap();
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        ordering,
        quotient_provider,
        ReductionEngineLimits::default(),
    );
    let closed = timed_stage("numeric-quotient/reduce-j211", || {
        engine.reduce(&target).unwrap()
    });
    closed.require_complete().unwrap();
    assert_eq!(
        closed.terms(),
        &BTreeMap::from([(
            key([1, 1, 1]),
            family.coefficient_context().parse("(d-3)/(3*m2)").unwrap(),
        )]),
        "persistent-source numeric quotient differs from the frozen Vakint sunset oracle",
    );
    let retained_quotient = closed
        .application_traces()
        .iter()
        .find_map(|trace| match trace {
            ConcreteRuleApplicationTrace::CertifiedRewrite(rewrite)
                if matches!(
                    rewrite.proof(),
                    CertifiedConcreteRewriteProof::GeneratedCylindricalNumericQuotientElimination {
                        persistent_source,
                        ..
                    } if Arc::ptr_eq(persistent_source, &persistent)
                ) =>
            {
                Some(rewrite.clone())
            }
            _ => None,
        })
        .expect("J(2,1,1) must retain a persistent cylindrical numeric-quotient proof");
    assert_eq!(
        retained_quotient.rhs(),
        &BTreeMap::from([(
            key([1, 1, 1]),
            family.coefficient_context().parse("(d-3)/(3*m2)").unwrap(),
        )]),
    );
    drop(engine);
    timed_stage("numeric-quotient/replay-after-provider-drop", || {
        retained_quotient
            .replay(&family, &context, ordering)
            .unwrap()
    });
}
