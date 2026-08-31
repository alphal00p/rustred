use std::cmp::Ordering;
use std::collections::{BTreeSet, HashSet};

use super::super::ordering::RUSTRED_UNSHIFTED_ORDER_V1_ID;
use super::super::{
    ComplexityComponent, CoordinatePriority, CoordinatePriorityError, CoordinatePriorityLimits,
    Error, OrderingPolicy,
};
use super::support::all_indices;

#[test]
fn complexity_key_is_injective_strict_and_has_stable_id_and_display() {
    let policy = OrderingPolicy::RustRedUnshiftedV1;
    assert_eq!(policy.stable_id(), RUSTRED_UNSHIFTED_ORDER_V1_ID);
    assert_eq!(
        OrderingPolicy::try_from_stable_id(policy.stable_id()).unwrap(),
        policy
    );
    assert!(matches!(
        OrderingPolicy::try_from_stable_id("rustred.unknown-order.v9"),
        Err(Error::UnknownOrderingPolicy { .. })
    ));

    let points = all_indices(3, -3, 3);
    let keys = points
        .iter()
        .map(|point| policy.complexity_key(point).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(keys.iter().collect::<HashSet<_>>().len(), points.len());
    assert_eq!(
        keys.iter().cloned().collect::<BTreeSet<_>>().len(),
        points.len()
    );

    for (left_position, left) in points.iter().enumerate() {
        for (right_position, right) in points.iter().enumerate() {
            let comparison = policy.compare(left, right).unwrap();
            assert_eq!(comparison, keys[left_position].cmp(&keys[right_position]));
            assert_eq!(comparison == Ordering::Equal, left == right);
            assert_eq!(comparison, policy.compare(right, left).unwrap().reverse());
        }
    }

    let key = policy.complexity_key(&[2, 0, -3]).unwrap();
    assert_eq!(key.propagators(), 1);
    assert_eq!(key.sector().to_string(), "100");
    assert_eq!(key.corner_distance(), 4);
    assert_eq!(key.dots(), 1);
    assert_eq!(key.numerators(), 3);
    assert_eq!(key.index_excess(), &[1, 0, 3]);
    assert_eq!(
        key.to_string(),
        "rustred.unshifted-sector-order.v1|arity=3|propagators=1|sector=100|corner=4|dots=1|numerators=3|excess=[1,0,3]"
    );
}

#[test]
fn descent_witness_identifies_the_first_strict_component() {
    let policy = OrderingPolicy::default();

    let dot_descent = policy.prove_strict_descent(&[3, 1], &[2, 1]).unwrap();
    assert_eq!(
        dot_descent.decisive_component(),
        ComplexityComponent::CornerDistance
    );
    assert!(dot_descent.verify());

    let sector_descent = policy.prove_strict_descent(&[1, 1], &[1, 0]).unwrap();
    assert_eq!(
        sector_descent.decisive_component(),
        ComplexityComponent::PropagatorCount
    );
    assert!(sector_descent.verify());

    let coordinate_descent = policy.prove_strict_descent(&[3, 2], &[2, 3]).unwrap();
    assert_eq!(
        coordinate_descent.decisive_component(),
        ComplexityComponent::IndexExcess { position: 0 }
    );
    assert!(coordinate_descent.verify());

    assert_eq!(
        policy.prove_strict_descent(&[1, 1], &[1, 1]),
        Err(Error::NotStrictDescent)
    );
    assert_eq!(
        policy.prove_strict_descent(&[1, 1], &[2, 1]),
        Err(Error::NotStrictDescent)
    );
}

#[test]
fn coordinate_priorities_are_exact_bijections_with_stable_full_vector_identities() {
    let limits = CoordinatePriorityLimits::default();
    let natural = CoordinatePriority::try_natural(4, limits).unwrap();
    assert_eq!(natural.arity(), 4);
    assert_eq!(natural.rank_by_slot(), [0, 1, 2, 3]);
    assert_eq!(
        natural.try_stable_id(limits).unwrap(),
        "rustred.coordinate-priority.v1;k=4;rank-by-slot=0,1,2,3"
    );

    let changed = CoordinatePriority::try_new(4, &[2, 0, 3, 1], limits).unwrap();
    let id = "rustred.coordinate-priority.v1;k=4;rank-by-slot=2,0,3,1";
    assert_eq!(changed.rank_by_slot(), [2, 0, 3, 1]);
    assert_eq!(changed.to_string(), id);
    assert_eq!(changed.try_stable_id(limits).unwrap(), id);
    assert_eq!(
        CoordinatePriority::try_from_stable_id(id, limits).unwrap(),
        changed
    );
    assert_ne!(changed, natural);
    assert!(matches!(
        OrderingPolicy::try_from_stable_id(id),
        Err(Error::UnknownOrderingPolicy { .. })
    ));
}

#[test]
fn malformed_coordinate_priorities_and_unbounded_requests_fail_closed() {
    let limits = CoordinatePriorityLimits {
        max_arity: 4,
        max_stable_id_bytes: 128,
    };
    assert_eq!(
        CoordinatePriority::try_new(0, &[], limits),
        Err(CoordinatePriorityError::Empty)
    );
    assert_eq!(
        CoordinatePriority::try_new(3, &[0, 1], limits),
        Err(CoordinatePriorityError::WrongArity {
            expected: 3,
            actual: 2,
        })
    );
    assert_eq!(
        CoordinatePriority::try_new(3, &[0, 1, 3], limits),
        Err(CoordinatePriorityError::RankOutOfRange {
            slot: 2,
            rank: 3,
            arity: 3,
        })
    );
    assert_eq!(
        CoordinatePriority::try_new(3, &[0, 1, 1], limits),
        Err(CoordinatePriorityError::DuplicateRank { slot: 2, rank: 1 })
    );
    assert_eq!(
        CoordinatePriority::try_natural(5, limits),
        Err(CoordinatePriorityError::ResourceLimit {
            resource: "arity",
            requested: 5,
            limit: 4,
        })
    );

    for malformed in [
        "",
        "rustred.coordinate-priority.v2;k=3;rank-by-slot=0,1,2",
        "rustred.coordinate-priority.v1;k=03;rank-by-slot=0,1,2",
        "rustred.coordinate-priority.v1;k=3;rank-by-slot=0,01,2",
        "rustred.coordinate-priority.v1;k=3;rank-by-slot=0,1,2;extra=1",
    ] {
        assert!(matches!(
            CoordinatePriority::try_from_stable_id(malformed, limits),
            Err(CoordinatePriorityError::MalformedStableId { .. })
        ));
    }
    assert_eq!(
        CoordinatePriority::try_from_stable_id(
            "rustred.coordinate-priority.v1;k=3;rank-by-slot=0,1",
            limits,
        ),
        Err(CoordinatePriorityError::WrongArity {
            expected: 3,
            actual: 2,
        })
    );
    assert_eq!(
        CoordinatePriority::try_from_stable_id(
            "rustred.coordinate-priority.v1;k=3;rank-by-slot=0,1,1",
            limits,
        ),
        Err(CoordinatePriorityError::DuplicateRank { slot: 2, rank: 1 })
    );

    let small_identity_limit = CoordinatePriorityLimits {
        max_arity: 4,
        max_stable_id_bytes: 8,
    };
    assert_eq!(
        CoordinatePriority::try_natural(3, small_identity_limit),
        Err(CoordinatePriorityError::ResourceLimit {
            resource: "stable identity bytes",
            requested: 53,
            limit: 8,
        })
    );

    let exact_boundary = CoordinatePriorityLimits {
        max_arity: 3,
        max_stable_id_bytes: 53,
    };
    let boundary = CoordinatePriority::try_new(3, &[2, 0, 1], exact_boundary).unwrap();
    assert_eq!(boundary.try_stable_id(exact_boundary).unwrap().len(), 53);
    assert_eq!(
        CoordinatePriority::try_new(
            3,
            &[2, 0, 1],
            CoordinatePriorityLimits {
                max_stable_id_bytes: 52,
                ..exact_boundary
            },
        ),
        Err(CoordinatePriorityError::ResourceLimit {
            resource: "stable identity bytes",
            requested: 53,
            limit: 52,
        })
    );
}
