use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily, IntegralKey};
use crate::sector::symmetry::permutation::compile;
use crate::sector::symmetry::{CoefficientMatrix, Limits, MomentumMap, verify};
use crate::sector::{CoordinatePriority, CoordinatePriorityLimits, OrderingPolicy};

use super::{CanonicalizationLimits, Canonicalizer, CoordinatePriorityActionLimits, Error};

fn sunset_family_with_identity(identity: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d"]);
    let zero = coefficients.zero();
    let one = coefficients.one();
    let minus_one = coefficients.integer(-1);
    IntegralFamily::new(
        identity,
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

fn sunset_family() -> IntegralFamily {
    sunset_family_with_identity("canonical-symmetry-sunset")
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

fn sunset_swap_c2() -> Canonicalizer {
    let family = sunset_family();
    let generator = compile(
        &family,
        verify(
            &family,
            &family,
            vacuum_map(family.coefficient_context(), [0, 1, 1, 0]),
            Limits::default(),
        )
        .unwrap(),
    )
    .unwrap();
    Canonicalizer::try_new(
        OrderingPolicy::default(),
        [generator],
        CanonicalizationLimits::default(),
    )
    .unwrap()
}

#[test]
fn authenticated_sunset_generators_close_to_exact_s3() {
    let expected_family = sunset_family();
    let owner = sunset_s3();
    assert_eq!(owner.family_fingerprint(), expected_family.fingerprint());
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
fn canonicalizer_rejects_same_arity_generators_from_distinct_families() {
    let first = sunset_family_with_identity("canonical-symmetry-family-first");
    let second = sunset_family_with_identity("canonical-symmetry-family-second");
    assert_ne!(first.fingerprint(), second.fingerprint());

    let generators = [&first, &second].map(|family| {
        let map = vacuum_map(family.coefficient_context(), [0, 1, 1, 0]);
        compile(
            family,
            verify(family, family, map, Limits::default()).unwrap(),
        )
        .unwrap()
    });
    assert_eq!(
        Canonicalizer::try_new(
            OrderingPolicy::default(),
            generators,
            CanonicalizationLimits::default(),
        )
        .unwrap_err(),
        Error::OrbitInvariant {
            detail: "authenticated symmetry generators belong to different families",
        }
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

#[test]
fn coordinate_priority_transport_follows_exact_source_for_target_composition() {
    let owner = sunset_s3();
    let priority_limits = CoordinatePriorityLimits::default();
    let action_limits = CoordinatePriorityActionLimits::default();
    let priority = CoordinatePriority::try_new(3, &[2, 0, 1], priority_limits).unwrap();

    // Group element 3 is [1, 2, 0], so out[j] = input[[1, 2, 0][j]].
    assert_eq!(owner.group_elements().nth(3).unwrap(), [1, 2, 0]);
    assert_eq!(
        owner
            .transport_coordinate_priority(&priority, 3, action_limits)
            .unwrap()
            .rank_by_slot(),
        [0, 1, 2]
    );

    // Applying [1,0,2] first and [0,2,1] second composes to [1,2,0].
    let after_left = owner
        .transport_coordinate_priority(&priority, 2, action_limits)
        .unwrap();
    let after_both = owner
        .transport_coordinate_priority(&after_left, 1, action_limits)
        .unwrap();
    let direct = owner
        .transport_coordinate_priority(&priority, 3, action_limits)
        .unwrap();
    assert_eq!(after_both, direct);

    let identity = owner
        .transport_coordinate_priority(&priority, 0, action_limits)
        .unwrap();
    assert_eq!(identity, priority);

    let canonical = owner
        .coordinate_priority_orbit(&priority, action_limits)
        .unwrap()
        .canonical()
        .rank_by_slot()
        .to_vec();
    for group_element in 0..owner.group_order() {
        let transported = owner
            .transport_coordinate_priority(&priority, group_element, action_limits)
            .unwrap();
        assert_eq!(
            owner
                .coordinate_priority_orbit(&transported, action_limits)
                .unwrap()
                .canonical()
                .rank_by_slot(),
            canonical
        );
    }
    let canonical_priority = CoordinatePriority::try_new(3, &canonical, priority_limits).unwrap();
    assert_eq!(
        owner
            .coordinate_priority_orbit(&canonical_priority, action_limits)
            .unwrap()
            .canonical()
            .rank_by_slot(),
        canonical
    );
}

#[test]
fn exact_coordinate_priority_quotient_is_a_deterministic_partition() {
    let action_limits = CoordinatePriorityActionLimits::default();

    let full = sunset_s3()
        .coordinate_priority_quotient(action_limits)
        .unwrap();
    assert_eq!(full.arity(), 3);
    assert_eq!(full.priority_count(), 6);
    assert_eq!(full.group_order(), 6);
    assert_eq!(full.class_count(), 1);
    assert_eq!(
        full.representatives()
            .map(CoordinatePriority::rank_by_slot)
            .collect::<Vec<_>>(),
        vec![&[0, 1, 2][..]]
    );
    assert_eq!(full.classes()[0].orbit_size(), 6);

    let owner = sunset_swap_c2();
    let first = owner.coordinate_priority_quotient(action_limits).unwrap();
    let second = owner.coordinate_priority_quotient(action_limits).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.priority_count(), 6);
    assert_eq!(first.group_order(), 2);
    assert_eq!(first.class_count(), 3);
    assert_eq!(
        first
            .representatives()
            .map(CoordinatePriority::rank_by_slot)
            .collect::<Vec<_>>(),
        vec![&[0, 1, 2][..], &[0, 2, 1][..], &[1, 2, 0][..]]
    );
    let members = first
        .classes()
        .iter()
        .flat_map(|class| class.images())
        .map(|priority| <[usize; 3]>::try_from(priority.rank_by_slot()).unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(members.len(), 6);
    assert!(first.classes().iter().all(|class| class.orbit_size() == 2));
}

#[test]
fn coordinate_priority_actions_reject_wrong_arity_ordinals_and_resource_overruns() {
    let owner = sunset_s3();
    let priority_limits = CoordinatePriorityLimits::default();
    let action_limits = CoordinatePriorityActionLimits::default();
    let short = CoordinatePriority::try_natural(2, priority_limits).unwrap();
    assert_eq!(
        owner.coordinate_priority_orbit(&short, action_limits),
        Err(Error::WrongPriorityArity {
            expected: 3,
            actual: 2,
        })
    );

    let priority = CoordinatePriority::try_natural(3, priority_limits).unwrap();
    assert_eq!(
        owner.transport_coordinate_priority(&priority, 6, action_limits),
        Err(Error::UnknownGroupElement {
            ordinal: 6,
            group_order: 6,
        })
    );
    assert_eq!(
        owner.coordinate_priority_orbit(
            &priority,
            CoordinatePriorityActionLimits {
                max_orbit_images: 5,
                ..action_limits
            },
        ),
        Err(Error::ResourceLimit {
            resource: "priority orbit images",
            requested: 6,
            limit: 5,
        })
    );
    assert_eq!(
        owner
            .coordinate_priority_orbit(
                &priority,
                CoordinatePriorityActionLimits {
                    max_orbit_images: 6,
                    ..action_limits
                },
            )
            .unwrap()
            .orbit_size(),
        6
    );
    assert_eq!(
        owner.coordinate_priority_quotient(CoordinatePriorityActionLimits {
            max_quotient_priorities: 5,
            ..action_limits
        }),
        Err(Error::ResourceLimit {
            resource: "priority quotient priorities",
            requested: 6,
            limit: 5,
        })
    );
    assert_eq!(
        owner.transport_coordinate_priority(
            &priority,
            0,
            CoordinatePriorityActionLimits {
                max_retained_rank_entries: 2,
                ..action_limits
            },
        ),
        Err(Error::ResourceLimit {
            resource: "priority retained rank entries",
            requested: 3,
            limit: 2,
        })
    );

    // A C2 quotient retains six three-rank members, one transient two-member
    // orbit, and the permutation/candidate cursors. The conservative exact
    // admission boundary is 6*3 + 2*3 + 2*3 = 30 rank entries.
    let c2 = sunset_swap_c2();
    let exact_small_group_limit = CoordinatePriorityActionLimits {
        max_retained_rank_entries: 30,
        ..action_limits
    };
    let quotient = c2
        .coordinate_priority_quotient(exact_small_group_limit)
        .unwrap();
    assert_eq!(quotient.class_count(), 3);
    for class in quotient.classes() {
        let identity_image = class
            .images()
            .iter()
            .find(|image| *image == class.source())
            .unwrap();
        assert_eq!(
            class.source().rank_by_slot().as_ptr(),
            identity_image.rank_by_slot().as_ptr()
        );
    }
    assert_eq!(
        c2.coordinate_priority_quotient(CoordinatePriorityActionLimits {
            max_retained_rank_entries: 29,
            ..action_limits
        }),
        Err(Error::ResourceLimit {
            resource: "priority retained rank entries",
            requested: 30,
            limit: 29,
        })
    );
}
