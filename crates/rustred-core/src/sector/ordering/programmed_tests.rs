//! Core adapters must share mathematical meaning, not only an order label.
use super::*;
use crate::solver::{Integral, IntegralOrder, SectorConfig, SectorSolver, SourceSystem};
use rustred_order::{
    CompiledOrder, CoordinateGroups, DegreeRow, Direction, Limits, OrderDescriptor,
};

fn descriptor(n: usize) -> OrderDescriptor {
    OrderDescriptor {
        pre_support_degree_rows: vec![],
        support_weights: (0..n).map(|axis| (axis + 1) as u64).collect(),
        support_priority: (0..n).rev().collect(),
        degree_rows: vec![DegreeRow {
            active: vec![1; n],
            inactive: vec![1; n],
        }],
        coordinate_priority: (0..n).rev().collect(),
        coordinate_groups: CoordinateGroups::InactiveFirst,
        active_direction: Direction::Descending,
        inactive_direction: Direction::Ascending,
    }
}

fn policy(n: usize) -> OrderingPolicy {
    OrderingPolicy::try_programmed(
        CompiledOrder::compile(descriptor(n), Limits::default()).unwrap(),
    )
    .unwrap()
}

#[test]
fn global_degree_adapter_rejects_growing_pinches_and_sector_first_proofs() {
    let mut d = descriptor(2);
    d.pre_support_degree_rows = vec![DegreeRow {
        active: vec![1; 2],
        inactive: vec![1; 2],
    }];
    let policy =
        OrderingPolicy::try_programmed(CompiledOrder::compile(d, Limits::default()).unwrap())
            .unwrap();
    assert!(!policy.is_support_primary());
    assert!(policy.has_total_excess_primary()); // Fixed support only.
    assert_eq!(
        OrderingPolicy::try_from_stable_id(&policy.stable_id()).unwrap(),
        policy
    );
    // A pinch with degree growth cannot borrow the old sector-first witness.
    assert!(
        OrderingPolicy::SpiredUncutV1
            .prove_strict_descent(&[1, 1], &[0, 3])
            .is_ok()
    );
    assert!(policy.prove_strict_descent(&[1, 1], &[0, 3]).is_err());
    let domain = crate::sector::SectorMonotoneDomain::try_new_for_rule(
        Mask::try_new([true, true]).unwrap(),
        [crate::sector::InteriorBounds::new(1, 1); 2],
        &[0, 0],
        &[[-1, 2]],
    )
    .unwrap();
    assert!(matches!(
        policy.prove_sector_monotone_shift_descent(&domain, &[0, 0], &[-1, 2]),
        Err(crate::sector::Error::OrderRequiresSupportPrimary)
    ));
    // A physical degree decrease can be simpler even when support grows.
    let witness = policy.prove_strict_descent(&[-3, 1], &[1, 1]).unwrap();
    assert!(witness.verify());
    assert_eq!(
        witness.decisive_component(),
        ComplexityComponent::PreSupportDegreeRow { ordinal: 0 }
    );
    let order = IntegralOrder::from_persisted_policy([true; 2], &policy).unwrap();
    for a in (-2..=2).flat_map(|i| (-2..=2).map(move |j| [i, j])) {
        for b in (-2..=2).flat_map(|i| (-2..=2).map(move |j| [i, j])) {
            let expected = policy
                .compare(&a.map(i64::from), &b.map(i64::from))
                .unwrap();
            assert_eq!(
                policy
                    .complexity_key(&a.map(i64::from))
                    .unwrap()
                    .cmp(&policy.complexity_key(&b.map(i64::from)).unwrap()),
                expected
            );
            assert_eq!(
                order
                    .compare(
                        &Integral::numeric(a).unwrap(),
                        &Integral::numeric(b).unwrap()
                    )
                    .reverse(),
                expected
            );
        }
    }
}

#[test]
fn programmed_policy_identity_is_content_based_and_has_no_packed_arity_limit() {
    for n in [1, 3, 21, 40] {
        let policy = policy(n);
        let identity = policy.stable_id().to_string();
        assert_eq!(
            OrderingPolicy::try_from_stable_id(&identity).unwrap(),
            policy
        );
        assert_eq!(policy.program().unwrap().arity(), n);
        assert!(policy.compare(&vec![1; n], &vec![1; n]).is_ok());
        assert!(policy.compare(&vec![1; n + 1], &vec![1; n + 1]).is_err());
        assert_ne!(policy, OrderingPolicy::SpiredUncutV1);
        assert!(OrderingPolicy::try_from_stable_id(&(identity.clone() + "00")).is_err());
        assert!(OrderingPolicy::try_from_stable_id(&identity[..identity.len() - 1]).is_err());
    }
}

#[test]
fn programmed_numeric_adapter_and_complexity_keys_agree_in_all_supports() {
    let policy = policy(3);
    let order = IntegralOrder::from_persisted_policy([true; 3], &policy).unwrap();
    let points: Vec<[i16; 3]> = (-1..=2)
        .flat_map(|a| (-1..=2).flat_map(move |b| (-1..=2).map(move |c| [a, b, c])))
        .collect();
    for a in &points {
        for b in &points {
            let expected = policy
                .compare(&a.map(i64::from), &b.map(i64::from))
                .unwrap();
            assert_eq!(
                policy
                    .complexity_key(&a.map(i64::from))
                    .unwrap()
                    .cmp(&policy.complexity_key(&b.map(i64::from)).unwrap()),
                expected
            );
            assert_eq!(
                order
                    .compare(
                        &Integral::numeric(*a).unwrap(),
                        &Integral::numeric(*b).unwrap()
                    )
                    .reverse(),
                expected
            );
        }
    }
}

#[test]
fn programmed_symbolic_shift_adapter_agrees_with_translated_numeric_points() {
    let policy = policy(3);
    for bits in 0..8 {
        let support = std::array::from_fn(|axis| bits & (1 << axis) != 0);
        let mask = Mask::try_new(support).unwrap();
        let native = IntegralOrder::from_persisted_policy(support, &policy).unwrap();
        for a in [[0, 0, 0], [1, -1, 0], [-1, 0, 1], [0, 1, -1]] {
            for b in [[0, 0, 0], [1, 0, -1], [-1, 1, 0]] {
                let expected = policy
                    .shift_complexity_key(&mask, &a.map(i64::from))
                    .unwrap()
                    .cmp(
                        &policy
                            .shift_complexity_key(&mask, &b.map(i64::from))
                            .unwrap(),
                    );
                assert_eq!(
                    native
                        .compare(
                            &Integral::symbolic(a).unwrap(),
                            &Integral::symbolic(b).unwrap()
                        )
                        .reverse(),
                    expected
                );
                let translated = |shift: [i16; 3]| {
                    std::array::from_fn::<_, 3, _>(|axis| {
                        if support[axis] {
                            4 + i64::from(shift[axis])
                        } else {
                            -4 + i64::from(shift[axis])
                        }
                    })
                };
                assert_eq!(
                    policy.compare(&translated(a), &translated(b)).unwrap(),
                    expected
                );
            }
        }
    }
}

#[test]
fn weighted_degree_can_increase_total_excess_but_keeps_exact_descent() {
    let mut d = descriptor(2);
    d.degree_rows = vec![DegreeRow {
        active: vec![4, 1],
        inactive: vec![4, 1],
    }];
    let policy =
        OrderingPolicy::try_programmed(CompiledOrder::compile(d, Limits::default()).unwrap())
            .unwrap();
    // E increases 1 -> 3 while weighted excess decreases 4 -> 3.
    let witness = policy.prove_strict_descent(&[2, 1], &[1, 4]).unwrap();
    assert!(witness.verify());
    assert!(!policy.has_total_excess_primary());
    assert!(
        OrderingPolicy::SpiredUncutV1
            .prove_strict_descent(&[2, 1], &[1, 4])
            .is_err()
    );
}

#[test]
fn programmed_source_solver_rejects_cuts_removed_cuts_and_legacy_ties() {
    let context = crate::algebra::CoefficientContext::new(["d"]);
    let family = crate::family::IntegralFamily::new(
        "ordering-admission",
        vec!["p".into(), "q".into()],
        vec![],
        context.clone(),
        context.parameter("d").unwrap(),
        [[1, 0, 0], [0, 0, 1], [1, 2, 1]]
            .into_iter()
            .map(|coefficients| {
                crate::family::AffineDenominator::new(
                    context.integer(-1),
                    coefficients
                        .into_iter()
                        .map(|c| context.integer(c))
                        .collect(),
                )
            })
            .collect(),
        vec![],
        vec![context.zero(); 3],
    )
    .unwrap();
    let sources = SourceSystem::<3>::from_family(&family).unwrap();
    for (deltas, removed, permutation) in [
        ([true, false, false], [false; 3], None),
        ([true, false, false], [true, false, false], None),
        ([false; 3], [true, false, false], None),
        ([false; 3], [false; 3], Some([2, 1, 0])),
    ] {
        let result = SectorSolver::new(
            &sources,
            [true; 3],
            SectorConfig {
                integral_order: policy(3).program().cloned(),
                deltas,
                removed_deltas: removed,
                permutation,
                ..Default::default()
            },
        );
        assert!(matches!(
            result,
            Err(crate::solver::SolverError::InvalidInput(_))
        ));
    }
}

#[test]
fn order_transport_commutes_with_both_core_and_source_port_comparisons() {
    let original = policy(3);
    let permutation = [2, 0, 1];
    let transported = original
        .program()
        .unwrap()
        .transport(&permutation, Limits::default())
        .unwrap();
    let transported = OrderingPolicy::try_programmed(transported).unwrap();
    let map = |point: [i64; 3]| permutation.map(|axis| point[axis]);
    // Transport is defined by target[axis] = source[permutation[axis]].
    for a in [[1, 2, 3], [-1, 2, 1], [1, 0, 3], [0, -2, 1]] {
        for b in [[3, 1, 2], [0, 3, 1], [1, 2, -2]] {
            assert_eq!(
                transported.compare(&map(a), &map(b)).unwrap(),
                original.compare(&a, &b).unwrap()
            );
        }
    }
}
