use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily, IntegralKey};
use crate::sector::OrderingPolicy;
use crate::sector::symmetry::permutation::compile;
use crate::sector::symmetry::{CoefficientMatrix, Limits, MomentumMap, verify};

use super::{CanonicalizationLimits, Canonicalizer, Error};

fn sunset_family() -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d"]);
    let zero = coefficients.zero();
    let one = coefficients.one();
    let minus_one = coefficients.integer(-1);
    IntegralFamily::new(
        "canonical-symmetry-sunset",
        vec!["k1".into(), "k2".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                minus_one.clone(),
                vec![one.clone(), zero.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                minus_one.clone(),
                vec![zero.clone(), zero.clone(), one.clone()],
            ),
            AffineDenominator::new(minus_one, vec![one.clone(), coefficients.integer(2), one]),
        ],
        Vec::new(),
        vec![zero.clone(), zero.clone(), zero],
    )
    .unwrap()
}

fn vacuum_map(coefficients: &CoefficientContext, entries: [i64; 4]) -> MomentumMap {
    MomentumMap::new(
        CoefficientMatrix::try_new(
            2,
            2,
            entries.into_iter().map(|entry| coefficients.integer(entry)),
        )
        .unwrap(),
        CoefficientMatrix::try_new(2, 0, []).unwrap(),
        CoefficientMatrix::try_new(0, 0, []).unwrap(),
    )
}

fn sunset_s3() -> Canonicalizer {
    let family = sunset_family();
    let coefficients = family.coefficient_context();
    let generators = [
        // k1 <-> k2: D0 <-> D1.
        vacuum_map(coefficients, [0, 1, 1, 0]),
        // k1 -> k1, k2 -> -k1-k2: D1 <-> D2.
        vacuum_map(coefficients, [1, 0, -1, -1]),
    ]
    .into_iter()
    .map(|map| {
        compile(
            &family,
            verify(&family, &family, map, Limits::default()).unwrap(),
        )
        .unwrap()
    })
    .collect::<Vec<_>>();
    Canonicalizer::try_new(
        OrderingPolicy::default(),
        generators,
        CanonicalizationLimits::default(),
    )
    .unwrap()
}

#[test]
fn authenticated_sunset_generators_close_to_exact_s3() {
    let owner = sunset_s3();
    assert_eq!(owner.arity(), 3);
    assert_eq!(owner.generator_count(), 2);
    assert_eq!(owner.group_order(), 6);
    assert_eq!(
        owner.group_elements().collect::<Vec<_>>(),
        vec![
            &[0, 1, 2][..],
            &[0, 2, 1][..],
            &[1, 0, 2][..],
            &[1, 2, 0][..],
            &[2, 0, 1][..],
            &[2, 1, 0][..],
        ]
    );
}

#[test]
fn canonical_images_follow_the_persisted_integral_order() {
    let owner = sunset_s3();
    for (source, expected) in [
        ([3, 1, 2], [1, 2, 3]),
        ([4, 0, 2], [0, 2, 4]),
        ([2, -3, 0], [0, -3, 2]),
        ([0, 0, 5], [0, 0, 5]),
    ] {
        let source = IntegralKey::try_new(source).unwrap();
        let canonical = owner.canonicalize(&source).unwrap();
        assert_eq!(canonical.canonical().powers(), expected);
        assert!(canonical.verify());
        assert!(canonical.no_harder().verify());
        assert!(canonical.route().verify(&source, canonical.canonical()));
    }
}

#[test]
fn orbit_routes_are_exact_deterministic_and_count_stabilizers() {
    let owner = sunset_s3();
    let source = IntegralKey::try_new([2, 2, 1]).unwrap();
    let first = owner.orbit(&source).unwrap();
    let second = owner.orbit(&source).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.orbit_size(), 3);
    assert_eq!(first.group_order(), 6);
    assert!(
        first
            .images()
            .iter()
            .all(|image| image.routing_multiplicity() == 2)
    );
    assert_eq!(first.canonical().integral().powers(), [1, 2, 2]);
}

#[test]
fn reducer_chain_proves_raw_then_canonical_descent() {
    let owner = sunset_s3();
    let parent = IntegralKey::try_new([2, 2, 3]).unwrap();
    let raw_child = IntegralKey::try_new([2, 1, 3]).unwrap();
    let chain = owner
        .canonicalize_descending_child(&parent, &raw_child)
        .unwrap();
    assert_eq!(chain.child().canonical().powers(), [1, 2, 3]);
    assert!(chain.verify());
}

#[test]
fn construction_limits_fail_before_unbounded_group_retention() {
    let family = sunset_family();
    let map = vacuum_map(family.coefficient_context(), [0, 1, 1, 0]);
    let generator = compile(
        &family,
        verify(&family, &family, map, Limits::default()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        Canonicalizer::try_new(
            OrderingPolicy::default(),
            [generator],
            CanonicalizationLimits {
                max_group_order: 1,
                ..CanonicalizationLimits::default()
            },
        )
        .unwrap_err(),
        Error::ResourceLimit {
            resource: "group order",
            requested: 2,
            limit: 1,
        }
    );
}
