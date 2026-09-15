use std::cmp::Ordering;

use crate::sector::{
    ComplexityComponent, CoordinatePriority, CoordinatePriorityLimits, Error, InteriorBounds,
    MAX_PACKED_ORDERING_PRIORITY_ARITY, Mask, OrderingPolicy, SectorInteriorDomain,
    SectorMonotoneDomain, SectorMonotonePointClass,
};
use crate::solver::{Integral, IntegralOrder};

fn policy_for_slots<const N: usize>(slots: [usize; N]) -> OrderingPolicy {
    let mut ranks = [0; N];
    for (rank, slot) in slots.into_iter().enumerate() {
        ranks[slot] = rank;
    }
    let priority =
        CoordinatePriority::try_new(N, &ranks, CoordinatePriorityLimits::default()).unwrap();
    OrderingPolicy::try_spired_with_coordinate_priority(&priority).unwrap()
}

fn small_points() -> Vec<[i16; 3]> {
    let mut points = Vec::new();
    for a in -2..=2 {
        for b in -2..=2 {
            for c in -2..=2 {
                points.push([a, b, c]);
            }
        }
    }
    points
}

#[test]
fn spired_identity_round_trips_and_cannot_alias_the_original_order() {
    let natural = policy_for_slots([0, 1, 2]);
    assert_eq!(natural, OrderingPolicy::SpiredUncutV1);
    assert_eq!(
        natural.static_stable_id(),
        Some("rustred.spired-uncut-sector-order.v1")
    );
    let custom = policy_for_slots([2, 0, 1]);
    for policy in [natural, custom] {
        assert_eq!(
            OrderingPolicy::try_from_stable_id(&policy.stable_id()).unwrap(),
            policy
        );
        assert_ne!(policy.stable_id(), OrderingPolicy::default().stable_id());
    }
    assert_eq!(custom.coordinate_priority_arity(), Some(3));
    assert_eq!(
        custom
            .try_coordinate_priority()
            .unwrap()
            .unwrap()
            .rank_by_slot(),
        &[1, 2, 0]
    );
    assert!(matches!(
        custom.complexity_key(&[1, 2]),
        Err(Error::WrongArity {
            expected: 3,
            actual: 2
        })
    ));
    let noncanonical = format!(
        "rustred.spired-uncut-sector-order.v1;priority={}",
        CoordinatePriority::try_new(3, &[0, 1, 2], CoordinatePriorityLimits::default())
            .unwrap()
            .try_stable_id(CoordinatePriorityLimits::default())
            .unwrap()
    );
    assert!(OrderingPolicy::try_from_stable_id(&noncanonical).is_err());
}

#[test]
fn maximum_packed_spired_priority_and_unpacked_natural_arity_are_supported() {
    let size = MAX_PACKED_ORDERING_PRIORITY_ARITY;
    let ranks = (0..size).rev().collect::<Vec<_>>();
    let priority =
        CoordinatePriority::try_new(size, &ranks, CoordinatePriorityLimits::default()).unwrap();
    let policy = OrderingPolicy::try_spired_with_coordinate_priority(&priority).unwrap();
    assert_eq!(
        OrderingPolicy::try_from_stable_id(&policy.stable_id()).unwrap(),
        policy
    );
    assert_eq!(
        policy
            .try_coordinate_priority()
            .unwrap()
            .unwrap()
            .rank_by_slot(),
        ranks
    );
    assert!(
        OrderingPolicy::SpiredUncutV1
            .complexity_key(&[1; 40])
            .is_ok()
    );
}

#[test]
fn all_small_numeric_comparisons_match_the_source_port_with_sign_reversal() {
    let points = small_points();
    for slots in [[0, 1, 2], [2, 0, 1], [1, 2, 0], [2, 1, 0]] {
        let policy = policy_for_slots(slots);
        let source_order = IntegralOrder::new([true, false, true], [false; 3])
            .with_permutation(slots)
            .unwrap();
        let keys = points
            .iter()
            .map(|point| policy.complexity_key(&point.map(i64::from)).unwrap())
            .collect::<Vec<_>>();
        for (i, left) in points.iter().enumerate() {
            for (j, right) in points.iter().enumerate() {
                let expected = source_order
                    .compare(
                        &Integral::numeric(*left).unwrap(),
                        &Integral::numeric(*right).unwrap(),
                    )
                    .reverse();
                assert_eq!(
                    keys[i].cmp(&keys[j]),
                    expected,
                    "{slots:?}: {left:?} / {right:?}"
                );
                assert_eq!(expected == Ordering::Equal, left == right);
            }
        }
    }
}

#[test]
fn symbolic_shift_keys_match_the_source_port_in_every_sector() {
    let shifts: Vec<_> = small_points()
        .into_iter()
        .filter(|point| point.iter().all(|value| (-1..=1).contains(value)))
        .collect();
    for bits in 0..8 {
        let active = std::array::from_fn(|slot| bits & (1 << slot) != 0);
        let sector = Mask::try_new(active).unwrap();
        for slots in [[0, 1, 2], [2, 0, 1], [1, 2, 0]] {
            let policy = policy_for_slots(slots);
            let source_order = IntegralOrder::new(active, [false; 3])
                .with_permutation(slots)
                .unwrap();
            let keys = shifts
                .iter()
                .map(|shift| {
                    policy
                        .shift_complexity_key(&sector, &shift.map(i64::from))
                        .unwrap()
                })
                .collect::<Vec<_>>();
            for (i, left) in shifts.iter().enumerate() {
                for (j, right) in shifts.iter().enumerate() {
                    let expected = source_order
                        .compare(
                            &Integral::symbolic(*left).unwrap(),
                            &Integral::symbolic(*right).unwrap(),
                        )
                        .reverse();
                    assert_eq!(
                        keys[i].cmp(&keys[j]),
                        expected,
                        "{bits}: {slots:?}: {left:?} / {right:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn numerator_degree_precedes_dot_degree_when_absolute_degree_ties() {
    let source = [1, -1];
    let target = [2, 0];
    let witness = OrderingPolicy::SpiredUncutV1
        .prove_strict_descent(&source, &target)
        .unwrap();
    assert!(witness.verify());
    assert_eq!(
        witness.decisive_component(),
        ComplexityComponent::NumeratorPower
    );
    assert_eq!(
        OrderingPolicy::default().prove_strict_descent(&source, &target),
        Err(Error::NotStrictDescent)
    );
}

#[test]
fn denominator_ties_precede_numerator_ties_even_if_priority_starts_inactive() {
    let left = [-1, 2, -2, 1];
    let right = [-2, 1, -1, 2];
    assert_eq!(
        OrderingPolicy::SpiredUncutV1
            .compare(&left, &right)
            .unwrap(),
        Ordering::Less
    );
    assert_eq!(
        policy_for_slots([0, 2, 3, 1])
            .compare(&left, &right)
            .unwrap(),
        Ordering::Greater
    );
}

#[test]
fn actual_vac3_numerator_transfer_is_uniformly_descending_on_its_coordinate_face() {
    let source = [0, -12, 1, -14, 1, 1];
    let target = [-1, -12, 1, -13, 1, 1];
    let policy = OrderingPolicy::SpiredUncutV1;
    let witness = policy.prove_strict_descent(&source, &target).unwrap();
    assert!(witness.verify());
    assert_eq!(
        witness.decisive_component(),
        ComplexityComponent::IndexExcess { position: 0 }
    );
    assert_eq!(
        OrderingPolicy::default().prove_strict_descent(&source, &target),
        Err(Error::NotStrictDescent)
    );

    let bounds = [
        InteriorBounds::new(0, 0),
        InteriorBounds::new(i64::MIN, 0),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(i64::MIN, -1),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(1, 1),
    ];
    let domain = SectorInteriorDomain::try_new(
        Mask::try_new([false, false, true, false, true, true]).unwrap(),
        bounds,
    )
    .unwrap();
    let uniform = policy
        .prove_shift_strict_descent(&domain, &[0; 6], &[-1, 0, 0, 1, 0, 0])
        .unwrap();
    assert!(uniform.verify());
    assert_eq!(
        uniform.decisive_component(),
        ComplexityComponent::IndexExcess { position: 0 }
    );
}

#[test]
fn shift_witness_decisive_component_uses_the_same_numerator_precedence() {
    let domain = SectorInteriorDomain::try_new(
        Mask::try_new([true, false]).unwrap(),
        [InteriorBounds::new(2, 100), InteriorBounds::new(-100, -2)],
    )
    .unwrap();
    let witness = OrderingPolicy::SpiredUncutV1
        .prove_shift_strict_descent(&domain, &[0, -1], &[1, 0])
        .unwrap();
    assert!(witness.verify());
    assert_eq!(
        witness.decisive_component(),
        ComplexityComponent::NumeratorPower
    );
    for anchor in [[2, -2], [100, -100], [37, -42]] {
        let source = [anchor[0], anchor[1] - 1];
        let target = [anchor[0] + 1, anchor[1]];
        assert_eq!(
            OrderingPolicy::SpiredUncutV1
                .compare(&target, &source)
                .unwrap(),
            Ordering::Less
        );
    }
}

#[test]
fn source_port_order_extends_beyond_compact_powers_without_negation_overflow() {
    let policy = OrderingPolicy::SpiredUncutV1;
    for (source, target) in [
        ([1, i64::MAX], [2, i64::MAX - 1]),
        ([0, i64::MIN], [-1, i64::MIN + 1]),
        ([i64::MAX, i64::MIN], [i64::MAX - 1, i64::MIN]),
    ] {
        assert!(
            policy
                .prove_strict_descent(&source, &target)
                .unwrap()
                .verify()
        );
    }
    let key = policy.complexity_key(&[i64::MIN; 40]).unwrap();
    assert_eq!(key.numerators(), 40 * u128::from(i64::MIN.unsigned_abs()));
    let sector = Mask::try_new([false, true]).unwrap();
    let extreme = policy
        .shift_complexity_key(&sector, &[i64::MIN, i64::MAX])
        .unwrap();
    assert_eq!(extreme.shift_at(0).unwrap(), i64::MIN);
    assert_eq!(extreme.shift_at(1).unwrap(), i64::MAX);
}

#[test]
fn pinch_descent_and_inactive_activation_remain_uniformly_checked() {
    let policy = OrderingPolicy::SpiredUncutV1;
    let sector = Mask::try_new([true, true]).unwrap();
    let domain = SectorMonotoneDomain::try_maximal_for_rule(sector, &[0, 0], &[[-1, 0]]).unwrap();
    let witness = policy
        .prove_sector_monotone_shift_descent(&domain, &[0, 0], &[-1, 0])
        .unwrap();
    assert!(witness.verify());
    assert!(matches!(
        witness.classify(&[1, 2]).unwrap(),
        Some(SectorMonotonePointClass::ProperSubsector { .. })
    ));
    assert_eq!(
        witness.classify(&[2, 2]).unwrap(),
        Some(SectorMonotonePointClass::SameSector)
    );
    assert!(witness.same_sector_descent().unwrap().verify());

    let unsafe_domain = SectorMonotoneDomain::try_maximal_for_rule(
        Mask::try_new([false, true]).unwrap(),
        &[0, 0],
        &[[1, -2]],
    )
    .unwrap();
    assert!(
        policy
            .prove_sector_monotone_shift_descent(&unsafe_domain, &[0, 0], &[1, -2])
            .is_err()
    );
    let safe_domain = SectorMonotoneDomain::try_new_for_rule(
        Mask::try_new([false, true]).unwrap(),
        [
            InteriorBounds::new(i64::MIN, -1),
            InteriorBounds::new(1, i64::MAX),
        ],
        &[0, 0],
        &[[1, -2]],
    )
    .unwrap();
    assert!(
        policy
            .prove_sector_monotone_shift_descent(&safe_domain, &[0, 0], &[1, -2])
            .unwrap()
            .verify()
    );
}
