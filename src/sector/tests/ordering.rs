use std::cmp::Ordering;
use std::collections::{BTreeSet, HashSet};

use super::super::ordering::{RUSTRED_UNSHIFTED_ORDER_V1_ID, RUSTRED_UNSHIFTED_ORDER_V1_SCHEMA};
use super::super::{ComplexityComponent, Error, OrderingPolicy};
use super::support::all_indices;

#[test]
fn complexity_key_is_injective_strict_and_has_stable_manifest() {
    let policy = OrderingPolicy::RustRedUnshiftedV1;
    assert_eq!(policy.stable_id(), RUSTRED_UNSHIFTED_ORDER_V1_ID);
    assert_eq!(policy.key_schema(), RUSTRED_UNSHIFTED_ORDER_V1_SCHEMA);
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
