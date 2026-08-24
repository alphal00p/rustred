//! Focused factored-product regression for the generic depth-two coverage path.
//!
//! The equal-mass sunset is only a concrete validation topology.  Production
//! receives no topology name, loop-count switch, recurrence, or master list.

use std::sync::Arc;

use rustred::{
    AdaptiveParametricRuleProvider, AffineDenominator, CoefficientContext, ConcreteIntegralKey,
    GeneratedFamilyRuleSystemLimits, GeneratedSectorSearchAnchorRequest,
    GeneratedSymbolicRowSpanCompiler, IntegralFamily, IntegralOrderingPolicy,
    ParametricIbpGenerator, ParametricSectorCoverageCompiler, SectorMask,
};

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

#[test]
#[ignore = "multi-minute natural regression for sunset sector 011 at adaptive depth two"]
fn sunset_011_depth_two_coverage_uses_factored_product_without_native_expansion() {
    let family = equal_mass_sunset("sunset-011-depth-two-factored-product");
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();

    // These are precisely the depth-growth discovery defaults, except that
    // the round has raised the local adaptive depth from zero to two.
    let mut limits = GeneratedFamilyRuleSystemLimits::default().discovery;
    limits.adaptive.max_search_depth = 2;

    let exact = limits
        .coverage
        .generated_when_bad
        .when_bad
        .arithmetic
        .exact_algebra;
    assert_eq!(exact.max_polynomial_terms, 4_000_000);
    assert_eq!(
        limits
            .coverage
            .max_product_reconstruction_native_output_term_bound,
        1 << 22
    );

    let row_span = Arc::new(
        GeneratedSymbolicRowSpanCompiler::compile(
            &family,
            &context,
            limits.coverage.generated_when_bad.ibp,
            limits.coverage.generated_when_bad.row_span,
        )
        .unwrap(),
    );
    assert_eq!(row_span.rows().len(), 4);

    let sector = SectorMask::try_new([false, true, true]).unwrap();
    let request = GeneratedSectorSearchAnchorRequest::new(key([0, 1, 1]), 2);

    // Mirror GeneratedSectorDiscoveryCompiler's one-anchor aggregate clamp.
    let mut adaptive_limits = limits.adaptive;
    adaptive_limits.max_search_depth = request.maximum_local_depth();
    adaptive_limits.max_pivot_candidates_per_integral = adaptive_limits
        .max_pivot_candidates_per_integral
        .min(limits.coverage.max_candidates);

    let rows = row_span.rows().to_vec();
    let mut adaptive = AdaptiveParametricRuleProvider::try_new(
        &context,
        &rows,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        adaptive_limits,
    )
    .unwrap();
    let layers = adaptive
        .candidate_layers_for_quotient(request.anchor())
        .unwrap();
    assert_eq!(layers.len(), 3);
    let layer_counts = layers.iter().map(Vec::len).collect::<Vec<_>>();
    let candidates = layers.into_iter().flatten().collect::<Vec<_>>();
    assert!(candidates.len() <= limits.coverage.max_candidates);

    let coverage = ParametricSectorCoverageCompiler::compile_with_row_span(
        &family,
        &context,
        sector,
        &candidates,
        row_span,
        limits.coverage,
    )
    .unwrap_or_else(|error| {
        panic!("depth-two sector-011 coverage failed after layers {layer_counts:?}: {error:#?}")
    });

    let stats = coverage.stats();
    let partition_stats = coverage.partition().stats();
    eprintln!(
        "sunset-011-depth2 partition: terms={}, bytes={}, splits={}, leaves={}, leaf_predicates={}",
        partition_stats.retained_polynomial_terms(),
        partition_stats.retained_polynomial_bytes(),
        partition_stats.split_count(),
        partition_stats.leaf_count(),
        partition_stats.total_leaf_predicates(),
    );
    assert!(stats.factored_product_zero_disjunctions() > 0);
    assert!(
        stats.factored_product_zero_factor_references()
            >= 2 * stats.factored_product_zero_disjunctions()
    );
    assert!(
        coverage
            .structural_loci()
            .iter()
            .all(|polynomial| polynomial.term_count() <= exact.max_polynomial_terms),
        "factor-only routing must not retain an oversized expanded product",
    );
}
