//! Family-wide current-path two-loop sunset acceptance against Vakint.
//!
//! The equal-mass sunset is a validation topology only. Production receives no
//! recurrence, expected coefficient, master count, loop dispatch, or preferred
//! pivot. All four sectors reuse one generated symbolic row span, while their
//! persistent eliminations and concrete applications remain sector-local.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use rustred::{
    AdaptiveParametricRuleProvider, AdaptiveRuleSearchLimits, AdaptiveRuleSearchStats,
    AffineDenominator, CertifiedConcreteRewrite, CertifiedConcreteRewriteProof,
    CertifiedFamilyRuleProvider, CertifiedFamilyRuleProviderLimits, CertifiedZeroReduction,
    Coefficient, CoefficientContext, ConcreteIntegralKey, ConcreteRuleApplicationTrace,
    GeneratedCylindricalFamilySourceSetCompiler, GeneratedCylindricalFamilySourceSetLimits,
    GeneratedSymbolicRowSpanConfig, IntegralFamily, IntegralOrderingPolicy,
    InternalSymmetrySearchLimits, MasterPolicyProvider, ParametricIbpConfig,
    ParametricIbpGenerator, ParametricReductionEngine, PowerShiftPolicy, ReductionEngineLimits,
    SectorMask, SectorRestrictions, discover_bounded_vacuum_internal_symmetries,
};

const ORDERING: IntegralOrderingPolicy = IntegralOrderingPolicy::RustRedUnshiftedV1;
const THROUGH_DEPTH: usize = 1;

fn equal_mass_sunset() -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let zero = coefficients.zero();
    let one = coefficients.one();
    let minus_m2 = coefficients.parse("-m2").unwrap();
    IntegralFamily::new(
        "generated-cylindrical-sunset-family-oracle",
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

fn expected(
    context: &CoefficientContext,
    master: [i64; 3],
    coefficient: &str,
) -> BTreeMap<ConcreteIntegralKey, Coefficient> {
    BTreeMap::from([(key(master), context.parse(coefficient).unwrap())])
}

fn permutations(powers: [i64; 3]) -> BTreeSet<[i64; 3]> {
    let [a, b, c] = powers;
    BTreeSet::from([
        [a, b, c],
        [a, c, b],
        [b, a, c],
        [b, c, a],
        [c, a, b],
        [c, b, a],
    ])
}

#[test]
fn shared_generated_sources_reduce_the_sunset_family_matrix_and_all_s3_images() {
    let family = equal_mass_sunset();
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let restrictions = SectorRestrictions::unrestricted(family.denominator_count()).unwrap();
    let source_set = GeneratedCylindricalFamilySourceSetCompiler::compile(
        &family,
        &context,
        restrictions.clone(),
        PowerShiftPolicy::FormalGeneric,
        ORDERING,
        ParametricIbpConfig::default(),
        GeneratedSymbolicRowSpanConfig::default(),
        THROUGH_DEPTH,
        GeneratedCylindricalFamilySourceSetLimits::default(),
    )
    .unwrap();
    source_set.replay(&family, &context).unwrap();

    let expected_solve_order = [
        SectorMask::try_new([false, true, true]).unwrap(),
        SectorMask::try_new([true, false, true]).unwrap(),
        SectorMask::try_new([true, true, false]).unwrap(),
        SectorMask::try_new([true, true, true]).unwrap(),
    ];
    assert_eq!(source_set.solve_order(), expected_solve_order);
    let inventory = Arc::clone(source_set.inventory_arc());
    let shared_row_span = Arc::clone(
        source_set
            .row_span_arc()
            .expect("a nonempty solve order has one generated row span"),
    );
    assert_eq!(shared_row_span.rows().len(), 4, "L(L+E)=4 native IBPs");
    let sources = source_set.persistent_sources().to_vec();
    assert_eq!(sources.len(), expected_solve_order.len());
    let canonical_boundary_source = Arc::clone(&sources[0]);
    let connected_top_source = Arc::clone(&sources[3]);
    for source in &sources {
        let start = source.row_system().start();
        let root = start
            .sector_root_start()
            .expect("family-wide fixture uses only sector-root sources");
        assert!(Arc::ptr_eq(root.inventory_arc(), &inventory));
        assert!(Arc::ptr_eq(root.row_span_arc(), &shared_row_span));
        assert!(start.assignment().is_empty());
    }

    let generated = ParametricIbpGenerator::try_with_context(
        &family,
        context.clone(),
        ParametricIbpConfig::default(),
    )
    .unwrap()
    .generate()
    .unwrap();
    let canonical_rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let symmetry_report = discover_bounded_vacuum_internal_symmetries(
        &family,
        &restrictions,
        InternalSymmetrySearchLimits::default(),
    )
    .unwrap();
    assert!(symmetry_report.completion().is_exhaustive_within_bounds());
    assert_eq!(symmetry_report.symmetries().len(), 6);

    let mut adaptive_limits = AdaptiveRuleSearchLimits::default();
    adaptive_limits.max_search_depth = 0;
    // Depth zero is inclusive of the central scout point. Make every adaptive
    // work/output surface hostile so any fallback attempt fails instead of
    // hiding behind the persistent/symmetry proof whitelist below.
    adaptive_limits.max_enumerated_offsets_per_integral = 0;
    adaptive_limits.max_offset_enumeration_steps_per_layer = 0;
    adaptive_limits.max_offset_components_per_integral = 0;
    adaptive_limits.max_scout_points_per_integral = 0;
    adaptive_limits.max_pivot_candidates_per_integral = 0;
    adaptive_limits.max_cached_decisions = 0;
    adaptive_limits.elimination.max_source_rows = 0;
    adaptive_limits.elimination.max_columns = 0;
    adaptive_limits.elimination.max_pivots = 0;
    adaptive_limits.rule.max_rhs_terms = 0;
    adaptive_limits.rule.max_source_rows_for_replay = 0;
    let adaptive = AdaptiveParametricRuleProvider::try_new(
        generated.context(),
        &canonical_rows,
        ORDERING,
        adaptive_limits,
    )
    .unwrap();
    let provider = CertifiedFamilyRuleProvider::try_new_with_persistent_cylindrical_sources(
        family.clone(),
        restrictions,
        symmetry_report.symmetries().iter().cloned(),
        adaptive,
        source_set.persistent_sources().iter().cloned(),
        ORDERING,
        CertifiedFamilyRuleProviderLimits::default(),
    )
    .unwrap();
    assert_eq!(provider.persistent_cylindrical_sources().len(), 4);
    assert_eq!(provider.adaptive().limits(), adaptive_limits);
    let provider =
        MasterPolicyProvider::with_selected(provider, [key([0, 1, 1]), key([1, 1, 1])]).unwrap();
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        ORDERING,
        provider,
        ReductionEngineLimits::default(),
    );

    let fixtures = [
        ([0, 2, 1], [0, 1, 1], "(d-2)/(2*m2)", true),
        ([0, 2, 2], [0, 1, 1], "(d-2)^2/(4*m2^2)", true),
        ([-1, 1, 1], [0, 1, 1], "m2", false),
        ([-2, 1, 1], [0, 1, 1], "m2^2*(1+4/d)", false),
        ([2, 1, 1], [1, 1, 1], "(d-3)/(3*m2)", true),
    ];
    let mut retained_rewrites = Vec::<CertifiedConcreteRewrite>::new();
    for (representative, master, coefficient, requires_m2_guard) in fixtures {
        for source in permutations(representative) {
            let result = engine.reduce(&key(source)).unwrap();
            result.require_complete().unwrap();
            assert_eq!(
                result.terms(),
                &expected(family.coefficient_context(), master, coefficient),
                "wrong current-path reduction for J{source:?}",
            );
            assert_eq!(result.selected_masters(), &BTreeSet::from([key(master)]));
            if requires_m2_guard {
                assert!(result.required_nonzero().iter().any(|condition| {
                    condition
                        .polynomial()
                        .to_expression()
                        .to_string()
                        .contains("m2")
                }));
            }
            let mut saw_persistent = false;
            for trace in result.application_traces() {
                let ConcreteRuleApplicationTrace::CertifiedRewrite(rewrite) = trace else {
                    panic!("J{source:?} used a non-certified application path: {trace:?}")
                };
                match rewrite.proof() {
                    CertifiedConcreteRewriteProof::Symmetry { .. } => {}
                    CertifiedConcreteRewriteProof::GeneratedCylindricalNumericQuotientElimination {
                        persistent_source,
                        ..
                    } => {
                        let expected_source = if master == [0, 1, 1] {
                            &canonical_boundary_source
                        } else {
                            &connected_top_source
                        };
                        assert!(Arc::ptr_eq(expected_source, persistent_source));
                        assert!(!Arc::ptr_eq(&sources[1], persistent_source));
                        assert!(!Arc::ptr_eq(&sources[2], persistent_source));
                        saw_persistent = true;
                    }
                    other => panic!("J{source:?} used an adaptive fallback proof: {other:?}"),
                }
                retained_rewrites.push(rewrite.clone());
            }
            assert!(saw_persistent, "J{source:?} never used a persistent source");
        }
    }

    let mut retained_zeros = Vec::<CertifiedZeroReduction>::new();
    for source in permutations([0, 0, 1]) {
        let result = engine.reduce(&key(source)).unwrap();
        result.require_complete().unwrap();
        assert!(result.terms().is_empty());
        assert!(result.terminal_statuses().is_empty());
        assert!(!result.application_traces().is_empty());
        for trace in result.application_traces() {
            match trace {
                ConcreteRuleApplicationTrace::ProvedZero(proof) => {
                    retained_zeros.push(proof.clone())
                }
                ConcreteRuleApplicationTrace::CertifiedRewrite(rewrite)
                    if matches!(
                        rewrite.proof(),
                        CertifiedConcreteRewriteProof::Symmetry { .. }
                    ) =>
                {
                    retained_rewrites.push(rewrite.clone())
                }
                other => panic!("zero J{source:?} used an unexpected path: {other:?}"),
            }
        }
    }

    assert_eq!(
        engine.provider().inner().adaptive().stats(),
        AdaptiveRuleSearchStats::default()
    );
    drop(engine);
    // Replay must be proof-owned: no external source-set, source, row-span,
    // or inventory handle remains alive below this boundary.
    drop(sources);
    drop(canonical_boundary_source);
    drop(connected_top_source);
    drop(shared_row_span);
    drop(inventory);
    drop(source_set);
    assert!(!retained_rewrites.is_empty());
    assert!(!retained_zeros.is_empty());
    for rewrite in retained_rewrites {
        rewrite.replay(&family, &context, ORDERING).unwrap();
    }
    for zero in retained_zeros {
        zero.replay(&family).unwrap();
    }
}
