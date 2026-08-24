//! Natural generated controls for the current affine-start completeness boundary.
//!
//! Concrete loop counts occur only in these validation fixtures.  Production
//! receives a generic family, sector, generated live-leaf queue, and resource
//! limits.  In particular, this test must not turn the synthetic sunset locus
//! used by the compositor oracle into a claim about queue discovery.

use std::sync::Arc;

use rustred::{
    AffineDenominator, CoefficientContext, GeneratedSectorDiscoveryCompiler,
    GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafQueueCertificate,
    GeneratedSectorLiveLeafQueueCompiler, GeneratedSectorLiveLeafQueueLimits, IntegralFamily,
    IntegralOrderingPolicy, ParametricCoefficientContext, ParametricIbpGenerator,
    ResidualUnitAffineIndexMapCertificate, ResidualUnitAffineIndexMapError,
    ResidualUnitAffineIndexMapLimits, ResidualUnitAffineIndexMapUnsupported, SectorMask,
    SymbolicPolynomialPredicateKind,
};

fn tadpole(name: &str, power_shift: bool) -> IntegralFamily {
    let coefficients = if power_shift {
        CoefficientContext::new(["d", "m2", "nu"])
    } else {
        CoefficientContext::new(["d", "m2"])
    };
    let shifts = if power_shift {
        vec![coefficients.parameter("nu").unwrap()]
    } else {
        vec![coefficients.zero()]
    };
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
        shifts,
    )
    .unwrap()
}

fn sunset(name: &str) -> IntegralFamily {
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

fn depth_zero_queue(
    family: &IntegralFamily,
    sector: SectorMask,
) -> (
    ParametricCoefficientContext,
    GeneratedSectorLiveLeafQueueCertificate,
) {
    let context = ParametricIbpGenerator::try_new(family)
        .unwrap()
        .context()
        .clone();
    let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
    discovery_limits.adaptive.max_search_depth = 0;
    let discovery = GeneratedSectorDiscoveryCompiler::compile(
        family,
        &context,
        sector,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        discovery_limits,
    )
    .unwrap();
    let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
    queue_limits.translation_radius = 0;
    queue_limits.max_translation_points = 1;
    let queue =
        GeneratedSectorLiveLeafQueueCompiler::compile(family, &context, &discovery, queue_limits)
            .unwrap();
    queue.replay(family, &context).unwrap();
    (context, queue)
}

fn equality_ordinals(item: &rustred::GeneratedSectorLiveLeafWorkItem) -> Vec<usize> {
    item.extraction()
        .unresolved_predicates()
        .iter()
        .filter(|predicate| predicate.kind() == SymbolicPolynomialPredicateKind::EqualZero)
        .map(|predicate| predicate.predicate_ordinal())
        .collect()
}

#[test]
fn natural_tadpoles_distinguish_literal_and_base_dependent_starts() {
    let ordinary = tadpole("generated-affine-boundary-ordinary-tadpole", false);
    let (_context, queue) = depth_zero_queue(&ordinary, SectorMask::try_new([true]).unwrap());
    assert_eq!(queue.discovery().stats().canonical_rows(), 1);
    assert_eq!(queue.discovery().coverage().partition().cases().len(), 2);
    assert_eq!(queue.work_items().len(), 1);
    assert_eq!(
        queue.work_items()[0].extraction().assignment().entries(),
        [(0, 1)]
    );
    assert!(equality_ordinals(&queue.work_items()[0]).is_empty());

    let shifted = tadpole("generated-affine-boundary-shifted-tadpole", true);
    let (context, queue) = depth_zero_queue(&shifted, SectorMask::try_new([true]).unwrap());
    assert_eq!(queue.discovery().stats().canonical_rows(), 1);
    assert_eq!(queue.work_items().len(), 1);
    let item = &queue.work_items()[0];
    assert!(item.extraction().assignment().is_empty());
    let equalities = equality_ordinals(item);
    assert_eq!(equalities.len(), 1);
    assert!(matches!(
        ResidualUnitAffineIndexMapCertificate::compile(
            &context,
            Arc::new(item.extraction().clone()),
            equalities[0],
            0,
            ResidualUnitAffineIndexMapLimits::default(),
        ),
        Err(ResidualUnitAffineIndexMapError::Unsupported {
            reason: ResidualUnitAffineIndexMapUnsupported::NotAssociateToSingleIntegerAffineRow { .. },
            ..
        })
    ));
}

#[test]
fn natural_sunset_leaves_require_product_locus_branch_decomposition() {
    for (bits, expected_leaves, expected_equalities) in
        [("011", 3, 2), ("101", 3, 2), ("110", 4, 3), ("111", 4, 3)]
    {
        let family = sunset(&format!("generated-affine-boundary-sunset-{bits}"));
        let (context, queue) =
            depth_zero_queue(&family, SectorMask::try_from_bit_string(bits).unwrap());
        assert_eq!(
            queue.discovery().stats().canonical_rows(),
            4,
            "sector {bits}"
        );
        assert_eq!(
            queue.discovery().coverage().partition().cases().len(),
            expected_leaves,
            "sector {bits}"
        );
        assert_eq!(queue.work_items().len(), 1, "sector {bits}");
        let item = &queue.work_items()[0];
        assert!(item.extraction().assignment().is_empty(), "sector {bits}");
        let equalities = equality_ordinals(item);
        assert_eq!(equalities.len(), expected_equalities, "sector {bits}");

        for predicate_ordinal in equalities {
            assert!(matches!(
                ResidualUnitAffineIndexMapCertificate::compile(
                    &context,
                    Arc::new(item.extraction().clone()),
                    predicate_ordinal,
                    0,
                    ResidualUnitAffineIndexMapLimits::default(),
                ),
                Err(ResidualUnitAffineIndexMapError::Unsupported {
                    reason:
                        ResidualUnitAffineIndexMapUnsupported::UnconsumedEqualityPredicates {
                            additional
                        },
                    ..
                }) if additional + 1 == expected_equalities
            ));
        }
    }
}
