//! First connected two-loop probe of the automatic symbolic sector search.
//!
//! The equal-mass sunset is a concrete validation family only.  Production
//! receives no recurrence, pivot, expected master count, or loop-specific
//! dispatch: the complete input is an `IntegralFamily`, its authenticated
//! `K(n)` context, a sector, an ordering, and bounded search policies.

use rustred::{
    AffineDenominator, CoefficientContext, GeneratedSectorDiscoveryCompiler,
    GeneratedSectorDiscoveryLimits, IntegralFamily, IntegralOrderingPolicy, ParametricIbpGenerator,
    SectorMask,
};

fn equal_mass_sunset() -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let zero = coefficients.zero();
    let one = coefficients.one();
    let minus_m2 = coefficients.parse("-m2").unwrap();
    IntegralFamily::new(
        "generated-two-loop-sector-discovery-sunset",
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

#[test]
fn depth_one_top_sector_search_is_bounded_and_replayable() {
    let family = equal_mass_sunset();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    assert_eq!(generated.ibp_li().count(), 4);
    let context = generated.context().clone();
    let sector = SectorMask::try_new([true, true, true]).unwrap();
    let mut limits = GeneratedSectorDiscoveryLimits::default();
    limits.adaptive.max_search_depth = 1;
    // This cap is deliberately far below the old >65,536-split Cartesian
    // partition.  Ordered direct-formula composition, exact product-locus
    // compression, and contradiction pruning must finish within it.
    limits.coverage.sector_cases.max_splits = 4_096;
    limits.coverage.sector_cases.max_live_cases = 4_097;
    limits.coverage.max_global_leaf_classifications = 4_097;
    let discovery = GeneratedSectorDiscoveryCompiler::compile(
        &family,
        &context,
        sector,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        limits,
    )
    .unwrap();
    discovery.replay(&family, &context).unwrap();
    assert_eq!(discovery.stats().canonical_rows(), 4);
    assert!(discovery.stats().canonical_terms() > 4);
    assert!(discovery.stats().candidate_attempts() > 0);
    assert!(discovery.stats().certified_candidates() > 0);
    assert!(discovery.stats().descending_leaves() > 0);
    assert!(
        discovery.stats().proved_empty_locus_leaves() > 0,
        "the connected sunset must exercise exact coordinate contradiction pruning: {:?}",
        discovery.stats()
    );
    assert!(
        discovery.coverage().partition().stats().split_count()
            <= limits.coverage.sector_cases.max_splits
    );
    assert_eq!(
        discovery.coverage().stats().coordinate_pruned_leaves()
            + discovery.coverage().stats().divisibility_pruned_leaves(),
        discovery.stats().proved_empty_locus_leaves()
    );
    for indices in [[1, 1, 1], [2, 1, 1], [2, 2, 1], [3, 1, 1]] {
        let classification = discovery
            .coverage()
            .classification_for_indices(&context, &indices)
            .unwrap()
            .unwrap();
        assert!(!matches!(
            classification.disposition(),
            rustred::ParametricSectorLeafDisposition::ProvedEmptyLocus { .. }
        ));
    }
}
