use super::*;
use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;

fn standard(size: usize) -> OrderDescriptor {
    OrderDescriptor {
        pre_support_degree_rows: Vec::new(),
        support_weights: vec![0; size],
        support_priority: (0..size).collect(),
        degree_rows: vec![
            DegreeRow {
                active: vec![1; size],
                inactive: vec![1; size],
            },
            DegreeRow {
                active: vec![0; size],
                inactive: vec![1; size],
            },
        ],
        coordinate_priority: (0..size).collect(),
        coordinate_groups: CoordinateGroups::ActiveFirst,
        active_direction: Direction::Descending,
        inactive_direction: Direction::Descending,
    }
}

fn compile(descriptor: OrderDescriptor) -> CompiledOrder {
    CompiledOrder::compile(descriptor, Limits::default()).unwrap()
}

fn global_f(size: usize) -> OrderDescriptor {
    let mut descriptor = standard(size);
    descriptor.pre_support_degree_rows.push(DegreeRow {
        active: vec![1; size],
        inactive: vec![1; size],
    });
    descriptor
}

fn prefix_weighted() -> OrderDescriptor {
    let mut descriptor = weighted();
    descriptor.pre_support_degree_rows = vec![DegreeRow {
        active: vec![2, 0, 5],
        inactive: vec![0, 7, 3],
    }];
    descriptor
}

#[test]
fn absolute_prefix_precedes_support_and_is_not_excess() {
    let order = compile(global_f(3));
    assert_eq!(
        order.compare(&[2, 0, 0], &[1, 1, 1]).unwrap(),
        Comparison {
            ordering: Ordering::Less,
            component: Some(Component::PreSupportDegreeRow(0)),
        }
    ); // F: 2<3, whereas E: 1>0.
    assert_eq!(
        order.compare(&[3, 0, 0], &[1, 1, 0]).unwrap().ordering,
        Ordering::Greater
    );
    assert_eq!(
        compile(standard(3))
            .compare(&[3, 0, 0], &[1, 1, 0])
            .unwrap()
            .ordering,
        Ordering::Less
    );
    assert_eq!(
        order.compare(&[2, 0, 0], &[1, 1, 0]).unwrap().component,
        Some(Component::SupportCount)
    );
    let mut descriptor = global_f(3);
    descriptor.pre_support_degree_rows.push(DegreeRow {
        active: vec![0; 3],
        inactive: vec![1; 3],
    });
    assert_eq!(
        compile(descriptor)
            .compare(&[1, -1, 0], &[1, 1, 0])
            .unwrap(),
        Comparison {
            ordering: Ordering::Greater,
            component: Some(Component::PreSupportDegreeRow(1)),
        }
    );
    // Concrete physical rule151 witness: a pinch raises F7 to F9.
    let order = compile(global_f(10));
    assert_eq!(
        order
            .compare(
                &[-2, 3, 0, 2, 1, 0, 0, 0, 0, 1],
                &[0, 1, 1, 2, 2, 0, 0, 0, 0, 1]
            )
            .unwrap()
            .ordering,
        Ordering::Greater
    );
}

#[test]
fn empty_prefix_keeps_known_v1_bytes() {
    let order = compile(standard(1));
    let mut expected = b"RRDORD01".to_vec();
    for word in [1u64, 2] {
        expected.extend_from_slice(&word.to_le_bytes());
    }
    expected.extend_from_slice(&[0, 1, 1]);
    for word in [0u64, 0, 0, 1, 1, 0, 1] {
        expected.extend_from_slice(&word.to_le_bytes());
    }
    assert_eq!(order.canonical_bytes(), expected);
    assert_eq!(
        CompiledOrder::from_canonical_bytes(&expected, Limits::default()).unwrap(),
        order
    );
}

#[test]
fn absolute_prefix_matches_independent_key_and_is_total_on_cube() {
    let points = cube(2, -2, 2);
    for weights in [(vec![1, 1], vec![1, 1]), (vec![0, 3], vec![4, 1])] {
        let mut descriptor = standard(2);
        descriptor.pre_support_degree_rows.push(DegreeRow {
            active: weights.0,
            inactive: weights.1,
        });
        let row = descriptor.pre_support_degree_rows[0].clone();
        let order = compile(descriptor);
        let key = |p: &[i64]| {
            p.iter()
                .enumerate()
                .map(|(axis, &n)| {
                    i128::from(if n > 0 {
                        row.active[axis]
                    } else {
                        row.inactive[axis]
                    }) * i128::from(n).abs()
                })
                .sum::<i128>()
        };
        for a in &points {
            for b in &points {
                let ab = order.compare(a, b).unwrap().ordering;
                assert_eq!(ab, key(a).cmp(&key(b)).then_with(|| legacy_uncut(a, b)));
                assert_eq!(ab, order.compare(b, a).unwrap().ordering.reverse());
                assert_eq!(ab == Ordering::Equal, a == b);
                for c in &points {
                    if ab == Ordering::Less
                        && order.compare(b, c).unwrap().ordering == Ordering::Less
                    {
                        assert_eq!(order.compare(a, c).unwrap().ordering, Ordering::Less);
                    }
                }
            }
        }
    }
}

#[test]
fn absolute_prefix_mixed_retained_and_shift_views_agree() {
    let order = compile(prefix_weighted());
    let points = cube(3, -2, 2);
    let support = [true, false, true];
    let symbolic = [true, true, false];
    let mixed_base = [10, -10, 0];
    let shift_base = [10, -10, 10];
    let retained = |p: &[i64]| {
        (
            p.iter().map(|&n| n > 0).collect::<Vec<_>>(),
            p.iter()
                .map(|&n| {
                    if n > 0 {
                        (n - 1) as u64
                    } else {
                        n.unsigned_abs()
                    }
                })
                .collect::<Vec<_>>(),
        )
    };
    for a in &points {
        for b in &points {
            let (sa, ea) = retained(a);
            let (sb, eb) = retained(b);
            assert_eq!(
                order.compare_excess(&sa, &ea, &sb, &eb).unwrap(),
                order.compare(a, b).unwrap()
            );
            let instantiate = |base: &[i64], offsets: &[i64]| {
                base.iter()
                    .zip(offsets)
                    .map(|(n, d)| n + d)
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                order.compare_mixed(&support, &symbolic, a, b).unwrap(),
                order
                    .compare(&instantiate(&mixed_base, a), &instantiate(&mixed_base, b))
                    .unwrap()
            );
            let expected = order
                .compare(&instantiate(&shift_base, a), &instantiate(&shift_base, b))
                .unwrap();
            assert_eq!(order.compare_shifts(&support, a, b).unwrap(), expected);
            let signed = |p: &[i64]| {
                p.iter()
                    .zip(support)
                    .map(|(&n, active)| {
                        if active {
                            i128::from(n)
                        } else {
                            -i128::from(n)
                        }
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                order
                    .compare_shift_excess(&support, &signed(a), &signed(b))
                    .unwrap(),
                expected
            );
        }
    }
}

#[test]
fn prefix_v2_codec_identity_limits_and_hostile_counts() {
    let order = compile(prefix_weighted());
    let bytes = order.canonical_bytes();
    assert_eq!(&bytes[..8], b"RRDORD02");
    assert_eq!(
        CompiledOrder::from_canonical_bytes(bytes, Limits::default()).unwrap(),
        order
    );
    assert_ne!(order, compile(weighted()));
    for n in 0..bytes.len() {
        assert!(CompiledOrder::from_canonical_bytes(&bytes[..n], Limits::default()).is_err());
    }
    let mut extra = bytes.to_vec();
    extra.push(0);
    assert!(CompiledOrder::from_canonical_bytes(&extra, Limits::default()).is_err());
    for index in [8, 16, 24] {
        let mut bad = bytes.to_vec();
        bad[index..index + 8].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(CompiledOrder::from_canonical_bytes(&bad, Limits::default()).is_err());
    }
    for index in [0, 32, 33, 34] {
        let mut bad = bytes.to_vec();
        bad[index] = 255;
        assert!(CompiledOrder::from_canonical_bytes(&bad, Limits::default()).is_err());
    }
    let mut empty_v2 = bytes.to_vec();
    empty_v2[24..32].copy_from_slice(&0u64.to_le_bytes());
    assert!(CompiledOrder::from_canonical_bytes(&empty_v2, Limits::default()).is_err());
    let limits = Limits {
        max_encoded_bytes: bytes.len() - 1,
        ..Limits::default()
    };
    assert!(CompiledOrder::compile(prefix_weighted(), limits).is_err());
    assert!(CompiledOrder::from_canonical_bytes(bytes, limits).is_err());
    assert!(order.transport(&[2, 0, 1], limits).is_err());
    let limits = Limits {
        max_comparison_terms: 3 * (2 * 2 + 8),
        ..Limits::default()
    };
    assert!(CompiledOrder::compile(weighted(), limits).is_ok());
    assert!(CompiledOrder::compile(prefix_weighted(), limits).is_err());
}

#[test]
fn prefix_transport_noninvolutive_covariance_and_inverse() {
    let order = compile(prefix_weighted());
    let p = [2, 0, 1];
    let transported = order.transport(&p, Limits::default()).unwrap();
    assert_eq!(
        transported
            .transport(&[1, 2, 0], Limits::default())
            .unwrap(),
        order
    );
    let points = cube(3, -1, 2);
    for a in &points {
        for b in &points {
            assert_eq!(
                order.compare(a, b).unwrap().ordering,
                transported
                    .compare(&map(a, &p), &map(b, &p))
                    .unwrap()
                    .ordering
            );
            let support = [true, false, true];
            assert_eq!(
                order.compare_shifts(&support, a, b).unwrap().ordering,
                transported
                    .compare_shifts(&map(&support, &p), &map(a, &p), &map(b, &p))
                    .unwrap()
                    .ordering
            );
            let symbolic = [true, false, false];
            assert_eq!(
                order
                    .compare_mixed(&support, &symbolic, a, b)
                    .unwrap()
                    .ordering,
                transported
                    .compare_mixed(
                        &map(&support, &p),
                        &map(&symbolic, &p),
                        &map(a, &p),
                        &map(b, &p)
                    )
                    .unwrap()
                    .ordering
            );
        }
    }
}

#[test]
fn prefix_coverage_extremes_and_effective_primary() {
    assert!(compile(standard(3)).is_support_primary());
    assert!(!compile(global_f(3)).is_support_primary());
    assert!(compile(standard(3)).has_total_excess_primary());
    assert!(compile(global_f(3)).has_total_excess_primary());
    assert!(!compile(prefix_weighted()).has_total_excess_primary());
    let mut rank_first = global_f(3);
    rank_first.pre_support_degree_rows[0].active.fill(0);
    assert!(!compile(rank_first).has_total_excess_primary());
    let mut prefix_only = global_f(3);
    prefix_only.degree_rows.clear();
    let order = compile(prefix_only);
    assert!(order.has_total_excess_primary());
    assert_eq!(
        CompiledOrder::from_canonical_bytes(order.canonical_bytes(), Limits::default()).unwrap(),
        order
    );
    let mut invalid = global_f(3);
    invalid.pre_support_degree_rows[0].active.pop();
    assert!(CompiledOrder::compile(invalid, Limits::default()).is_err());
    let mut zero = global_f(3);
    zero.pre_support_degree_rows[0].active.fill(0);
    zero.pre_support_degree_rows[0].inactive.fill(0);
    assert_eq!(
        CompiledOrder::compile(zero, Limits::default()).unwrap_err(),
        Error::ZeroDegreeRow { row: 0 }
    );
    let mut extreme = standard(1);
    extreme.pre_support_degree_rows.push(DegreeRow {
        active: vec![u64::MAX - 1],
        inactive: vec![1],
    });
    let order = compile(extreme.clone());
    assert_eq!(
        order.compare(&[i64::MIN], &[i64::MAX]).unwrap().ordering,
        Ordering::Less
    );
    assert_eq!(
        order
            .compare_excess(&[false], &[1u64 << 63], &[true], &[i64::MAX as u64 - 1])
            .unwrap(),
        order.compare(&[i64::MIN], &[i64::MAX]).unwrap()
    );
    for active in [true, false] {
        assert_ne!(
            order
                .compare_shifts(&[active], &[i64::MIN], &[i64::MAX])
                .unwrap()
                .ordering,
            Ordering::Equal
        );
        assert_eq!(
            order
                .compare_mixed(&[active], &[true], &[i64::MIN], &[i64::MAX])
                .unwrap(),
            order
                .compare_shifts(&[active], &[i64::MIN], &[i64::MAX])
                .unwrap()
        );
    }
    extreme.pre_support_degree_rows[0].active[0] = u64::MAX;
    assert_eq!(
        CompiledOrder::compile(extreme, Limits::default()).unwrap_err(),
        Error::WeightSumOverflow
    );
    for size in [1, 10, 15, 21, 80] {
        assert_eq!(compile(global_f(size)).arity(), size);
    }
}

fn weighted() -> OrderDescriptor {
    let mut d = standard(3);
    d.support_weights = vec![7, 1, 3];
    d.support_priority = vec![2, 0, 1];
    d.degree_rows = vec![
        DegreeRow {
            active: vec![3, 1, 0],
            inactive: vec![0, 5, 2],
        },
        DegreeRow {
            active: vec![0, 2, 4],
            inactive: vec![3, 0, 1],
        },
    ];
    d.coordinate_priority = vec![1, 2, 0];
    d.coordinate_groups = CoordinateGroups::InactiveFirst;
    d.active_direction = Direction::Ascending;
    d
}

fn cube(size: usize, low: i64, high: i64) -> Vec<Vec<i64>> {
    let mut points = vec![Vec::new()];
    for _ in 0..size {
        points = points
            .into_iter()
            .flat_map(|p| {
                (low..=high).map(move |v| {
                    let mut next = p.clone();
                    next.push(v);
                    next
                })
            })
            .collect();
    }
    points
}

fn legacy_uncut(left: &[i64], right: &[i64]) -> Ordering {
    let support = |p: &[i64]| p.iter().map(|&v| v > 0).collect::<Vec<_>>();
    let excess = |n: i64| {
        if n > 0 {
            i128::from(n) - 1
        } else {
            -i128::from(n)
        }
    };
    let ls = support(left);
    let rs = support(right);
    let mut cmp = ls
        .iter()
        .filter(|&&v| v)
        .count()
        .cmp(&rs.iter().filter(|&&v| v).count());
    if cmp == Ordering::Equal {
        cmp = ls.cmp(&rs);
    }
    if cmp == Ordering::Equal {
        cmp = left
            .iter()
            .map(|&n| excess(n))
            .sum::<i128>()
            .cmp(&right.iter().map(|&n| excess(n)).sum());
    }
    if cmp == Ordering::Equal {
        cmp = left
            .iter()
            .filter(|&&n| n <= 0)
            .map(|&n| excess(n))
            .sum::<i128>()
            .cmp(&right.iter().filter(|&&n| n <= 0).map(|&n| excess(n)).sum());
    }
    for active in [true, false] {
        for axis in 0..left.len() {
            if cmp == Ordering::Equal && ls[axis] == active {
                cmp = excess(right[axis]).cmp(&excess(left[axis]));
            }
        }
    }
    cmp
}

#[test]
fn uncut_reference_order_matches_on_complete_cube() {
    let order = compile(standard(3));
    let points = cube(3, -2, 2);
    for left in &points {
        for right in &points {
            assert_eq!(
                order.compare(left, right).unwrap().ordering,
                legacy_uncut(left, right)
            );
        }
    }
}

#[test]
fn genuinely_weighted_degree_order_changes_earlier_than_coordinate_ties() {
    let legacy = compile(standard(2));
    let mut descriptor = standard(2);
    descriptor.degree_rows = vec![DegreeRow {
        active: vec![3, 1],
        inactive: vec![3, 1],
    }];
    let order = compile(descriptor);
    let parent = [2, 1];
    let child = [1, 3];
    assert_eq!(
        legacy.compare(&child, &parent).unwrap().ordering,
        Ordering::Greater
    );
    assert_eq!(
        order.compare(&child, &parent).unwrap(),
        Comparison {
            ordering: Ordering::Less,
            component: Some(Component::DegreeRow(0)),
        }
    );
    // This real order intentionally does not imply non-increasing unweighted
    // total excess; a caller must not reuse the legacy E-envelope proof.
    assert!(child.iter().map(|n| n - 1).sum::<i64>() > parent.iter().map(|n| n - 1).sum());
}

#[test]
fn support_count_precedes_weights_and_support_bits_precede_degrees() {
    let order = compile(weighted());
    assert_eq!(
        order.compare(&[100, 0, 0], &[0, 1, 1]).unwrap().component,
        Some(Component::SupportCount)
    );
    assert_eq!(
        order.compare(&[1, 0, 0], &[0, 100, 0]).unwrap(),
        Comparison {
            ordering: Ordering::Greater,
            component: Some(Component::SupportWeight),
        }
    );
    let mut d = weighted();
    d.support_weights.fill(0);
    let order = compile(d);
    assert_eq!(
        order.compare(&[1, 0, 0], &[0, 0, 100]).unwrap(),
        Comparison {
            ordering: Ordering::Less,
            component: Some(Component::SupportAxis(2)),
        }
    );
}

#[test]
fn totality_transitivity_and_strict_pinches_on_finite_cube() {
    let mut d = weighted();
    d.support_weights.pop();
    d.support_priority = vec![1, 0];
    d.coordinate_priority = vec![1, 0];
    for row in &mut d.degree_rows {
        row.active.pop();
        row.inactive.pop();
    }
    let order = compile(d);
    let points = cube(2, -2, 2);
    for a in &points {
        for b in &points {
            let ab = order.compare(a, b).unwrap().ordering;
            assert_eq!(ab, order.compare(b, a).unwrap().ordering.reverse());
            assert_eq!(ab == Ordering::Equal, a == b);
            for c in &points {
                if ab == Ordering::Less && order.compare(b, c).unwrap().ordering == Ordering::Less {
                    assert_eq!(order.compare(a, c).unwrap().ordering, Ordering::Less);
                }
            }
        }
        for axis in 0..2 {
            if a[axis] > 0 {
                let mut pinched = a.clone();
                pinched[axis] = 0;
                assert_eq!(order.compare(&pinched, a).unwrap().ordering, Ordering::Less);
            }
        }
    }
}

#[test]
fn shifts_match_concrete_translations_inside_one_support() {
    let order = compile(weighted());
    let support = [true, false, true];
    let base = [10, -10, 8];
    let shifts = cube(3, -2, 2);
    for a in &shifts {
        for b in &shifts {
            let left: Vec<_> = base.iter().zip(a).map(|(x, d)| x + d).collect();
            let right: Vec<_> = base.iter().zip(b).map(|(x, d)| x + d).collect();
            assert_eq!(
                order.compare_shifts(&support, a, b).unwrap(),
                order.compare(&left, &right).unwrap()
            );
        }
    }
}

#[test]
fn mixed_symbolic_offsets_cancel_only_after_actual_support_comparison() {
    let order = compile(weighted());
    let sector = [true, false, true];
    let symbolic = [true, true, false];
    let base = [20, -20, 0];
    let points = cube(3, -2, 2);
    for a in &points {
        for b in &points {
            let left: Vec<_> = base.iter().zip(a).map(|(x, d)| x + d).collect();
            let right: Vec<_> = base.iter().zip(b).map(|(x, d)| x + d).collect();
            assert_eq!(
                order.compare_mixed(&sector, &symbolic, a, b).unwrap(),
                order.compare(&left, &right).unwrap()
            );
        }
    }
}

#[test]
fn reversed_coordinate_ties_and_groups_are_runtime_data() {
    let mut d = standard(4);
    d.degree_rows.truncate(1);
    let a = [2, 1, -1, 0];
    let b = [1, 2, 0, -1];
    for (groups, expected_axis) in [
        (CoordinateGroups::ActiveFirst, 0),
        (CoordinateGroups::InactiveFirst, 2),
        (CoordinateGroups::Interleaved, 2),
    ] {
        d.coordinate_groups = groups;
        d.coordinate_priority = vec![2, 0, 1, 3];
        let descending = compile(d.clone()).compare(&a, &b).unwrap();
        assert_eq!(
            descending.component,
            Some(Component::Coordinate(expected_axis))
        );
        d.active_direction = Direction::Ascending;
        d.inactive_direction = Direction::Ascending;
        let ascending = compile(d.clone()).compare(&a, &b).unwrap();
        assert_eq!(ascending.ordering, descending.ordering.reverse());
        d.active_direction = Direction::Descending;
        d.inactive_direction = Direction::Descending;
    }
}

#[test]
fn canonical_encoding_content_identity_and_builder_run_once() {
    let mut calls = 0;
    let order = CompiledOrder::from_builder(Limits::default(), || {
        calls += 1;
        weighted()
    })
    .unwrap();
    assert_eq!(calls, 1);
    let restored =
        CompiledOrder::from_canonical_bytes(order.canonical_bytes(), Limits::default()).unwrap();
    assert_eq!(order, restored);
    assert!(!Arc::ptr_eq(&order.0, &restored.0));
    assert_eq!(order.cmp(&restored), Ordering::Equal);
    let digest = |value: &CompiledOrder| {
        let mut h = DefaultHasher::new();
        value.hash(&mut h);
        h.finish()
    };
    assert_eq!(digest(&order), digest(&restored));
    assert_eq!(order.canonical_bytes(), restored.canonical_bytes());
    assert_eq!(
        std::mem::size_of::<CompiledOrder>(),
        std::mem::size_of::<usize>()
    );
    assert_eq!(Arc::strong_count(&order.0), 1);
    for _ in 0..100 {
        order.compare(&[1, 0, -1], &[1, -1, 0]).unwrap();
    }
    assert_eq!(Arc::strong_count(&order.0), 1);
}

#[test]
fn decoder_refuses_truncation_extra_bytes_unknown_flags_and_huge_dimensions() {
    let order = compile(weighted());
    let bytes = order.canonical_bytes();
    for n in 0..bytes.len() {
        assert!(CompiledOrder::from_canonical_bytes(&bytes[..n], Limits::default()).is_err());
    }
    let mut extra = bytes.to_vec();
    extra.push(0);
    assert_eq!(
        CompiledOrder::from_canonical_bytes(&extra, Limits::default()).unwrap_err(),
        Error::InvalidEncoding
    );
    for index in [0, 24, 25, 26] {
        let mut bad = bytes.to_vec();
        bad[index] = 255;
        assert!(CompiledOrder::from_canonical_bytes(&bad, Limits::default()).is_err());
    }
    for index in [8, 16] {
        let mut bad = bytes.to_vec();
        bad[index..index + 8].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(CompiledOrder::from_canonical_bytes(&bad, Limits::default()).is_err());
    }
}

#[test]
fn missing_sign_coverage_bad_permutations_and_zero_rows_fail_closed() {
    let mut d = standard(3);
    for row in &mut d.degree_rows {
        row.inactive[1] = 0;
    }
    assert_eq!(
        CompiledOrder::compile(d, Limits::default()).unwrap_err(),
        Error::UncoveredCoordinate {
            axis: 1,
            active: false
        }
    );
    let mut d = standard(3);
    for row in &mut d.degree_rows {
        row.active[2] = 0;
    }
    assert_eq!(
        CompiledOrder::compile(d, Limits::default()).unwrap_err(),
        Error::UncoveredCoordinate {
            axis: 2,
            active: true
        }
    );
    let mut d = standard(3);
    d.support_priority = vec![0, 0, 2];
    assert_eq!(
        CompiledOrder::compile(d, Limits::default()).unwrap_err(),
        Error::InvalidPermutation
    );
    let mut d = standard(3);
    d.coordinate_priority = vec![0, 1, 3];
    assert_eq!(
        CompiledOrder::compile(d, Limits::default()).unwrap_err(),
        Error::InvalidPermutation
    );
    let mut d = standard(3);
    d.degree_rows[0].active.fill(0);
    d.degree_rows[0].inactive.fill(0);
    assert_eq!(
        CompiledOrder::compile(d, Limits::default()).unwrap_err(),
        Error::ZeroDegreeRow { row: 0 }
    );
}

#[test]
fn checked_l1_bound_covers_extreme_concrete_and_signed_shift_values() {
    let mut d = standard(1);
    d.support_weights[0] = u64::MAX;
    d.degree_rows = vec![DegreeRow {
        active: vec![u64::MAX - 1],
        inactive: vec![1],
    }];
    let order = compile(d.clone());
    order.compare(&[i64::MIN], &[i64::MAX]).unwrap();
    for active in [false, true] {
        assert_ne!(
            order
                .compare_shifts(&[active], &[i64::MIN], &[i64::MAX])
                .unwrap()
                .ordering,
            Ordering::Equal
        );
    }
    d.degree_rows[0].active[0] = u64::MAX;
    assert_eq!(
        CompiledOrder::compile(d, Limits::default()).unwrap_err(),
        Error::WeightSumOverflow
    );
    let mut d = standard(2);
    d.support_weights = vec![u64::MAX, 1];
    assert_eq!(
        CompiledOrder::compile(d, Limits::default()).unwrap_err(),
        Error::WeightSumOverflow
    );
}

#[test]
fn resource_caps_bound_data_and_work_without_a_family_arity_cap() {
    for size in [1, 16, 21, 34, 80] {
        let order = compile(standard(size));
        assert_eq!(order.arity(), size);
        assert_eq!(
            order.compare(&vec![1; size], &vec![1; size]).unwrap(),
            Comparison {
                ordering: Ordering::Equal,
                component: None
            }
        );
    }
    let order = compile(weighted());
    assert!(
        CompiledOrder::from_canonical_bytes(
            order.canonical_bytes(),
            Limits {
                max_encoded_bytes: order.canonical_bytes().len() - 1,
                ..Limits::default()
            }
        )
        .is_err()
    );
    assert!(
        CompiledOrder::compile(
            weighted(),
            Limits {
                max_comparison_terms: 1,
                ..Limits::default()
            }
        )
        .is_err()
    );
    assert_eq!(
        CompiledOrder::compile(standard(0), Limits::default()).unwrap_err(),
        Error::EmptyArity
    );
    let mut d = standard(1);
    d.degree_rows.clear();
    assert_eq!(
        CompiledOrder::compile(d, Limits::default()).unwrap_err(),
        Error::NoDegreeRows
    );
}

fn map<T: Copy>(values: &[T], source_for_target: &[usize]) -> Vec<T> {
    source_for_target
        .iter()
        .map(|&source| values[source])
        .collect()
}

#[test]
fn nonuniform_transport_commutes_for_support_degrees_ties_and_shifts() {
    let order = compile(weighted());
    let permutation = [2, 0, 1];
    let target = order.transport(&permutation, Limits::default()).unwrap();
    let points = cube(3, -1, 2);
    for left in &points {
        for right in &points {
            let expected = order.compare(left, right).unwrap().ordering;
            assert_eq!(
                target
                    .compare(&map(left, &permutation), &map(right, &permutation))
                    .unwrap()
                    .ordering,
                expected
            );
            for support in [[true, false, true], [false, true, false]] {
                let expected = order
                    .compare_shifts(&support, left, right)
                    .unwrap()
                    .ordering;
                assert_eq!(
                    target
                        .compare_shifts(
                            &map(&support, &permutation),
                            &map(left, &permutation),
                            &map(right, &permutation)
                        )
                        .unwrap()
                        .ordering,
                    expected
                );
            }
        }
    }
    assert_ne!(order, target);
    // Merely reusing the original descriptor with transformed powers does not
    // commute in general; a label is not a transport proof.
    assert!(points.iter().any(|left| points.iter().any(|right| {
        order.compare(left, right).unwrap().ordering
            != order
                .compare(&map(left, &permutation), &map(right, &permutation))
                .unwrap()
                .ordering
    })));
}

#[test]
fn transport_composition_inverse_and_identity_preserve_canonical_data() {
    let order = compile(weighted());
    let p = [2, 0, 1];
    let q = [1, 0, 2];
    let composition = map(&p, &q);
    let sequential = order
        .transport(&p, Limits::default())
        .unwrap()
        .transport(&q, Limits::default())
        .unwrap();
    assert_eq!(
        sequential,
        order.transport(&composition, Limits::default()).unwrap()
    );
    assert_eq!(
        order
            .transport(&p, Limits::default())
            .unwrap()
            .transport(&[1, 2, 0], Limits::default())
            .unwrap(),
        order
    );
    assert_eq!(
        order.transport(&[0, 1, 2], Limits::default()).unwrap(),
        order
    );
    assert!(order.transport(&[0, 0, 2], Limits::default()).is_err());
    assert!(order.transport(&[0, 1], Limits::default()).is_err());
}

#[test]
fn input_arity_mismatch_cannot_reach_indexing_or_shift_evaluation() {
    let order = compile(weighted());
    assert!(order.compare(&[1], &[1, 2, 3]).is_err());
    assert!(
        order
            .compare_shifts(&[true], &[1, 2, 3], &[1, 2, 3])
            .is_err()
    );
    assert!(
        order
            .compare_mixed(&[true; 3], &[true; 2], &[1; 3], &[2; 3])
            .is_err()
    );
}

#[test]
fn borrowed_excess_keys_match_the_same_program_without_raw_power_copies() {
    let order = compile(weighted());
    let points = cube(3, -2, 2);
    for a in &points {
        let sa: Vec<_> = a.iter().map(|&n| n > 0).collect();
        let ea: Vec<_> = a
            .iter()
            .map(|&n| {
                if n > 0 {
                    (n - 1) as u64
                } else {
                    n.unsigned_abs()
                }
            })
            .collect();
        for b in &points {
            let sb: Vec<_> = b.iter().map(|&n| n > 0).collect();
            let eb: Vec<_> = b
                .iter()
                .map(|&n| {
                    if n > 0 {
                        (n - 1) as u64
                    } else {
                        n.unsigned_abs()
                    }
                })
                .collect();
            assert_eq!(
                order.compare_excess(&sa, &ea, &sb, &eb).unwrap(),
                order.compare(a, b).unwrap()
            );
            let support = [true, false, true];
            let shifts = |values: &[i64]| {
                values
                    .iter()
                    .zip(support)
                    .map(|(&n, active)| {
                        if active {
                            i128::from(n)
                        } else {
                            -i128::from(n)
                        }
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                order
                    .compare_shift_excess(&support, &shifts(a), &shifts(b))
                    .unwrap(),
                order.compare_shifts(&support, a, b).unwrap()
            );
        }
    }
    assert!(
        order
            .compare_excess(&[true; 3], &[u64::MAX; 3], &[true; 3], &[0; 3])
            .is_err()
    );
    assert!(
        order
            .compare_shift_excess(&[false; 3], &[i128::MIN; 3], &[0; 3])
            .is_err()
    );
}

#[test]
fn total_excess_capability_is_an_explicit_positive_uniform_first_row() {
    assert!(compile(standard(3)).has_total_excess_primary());
    assert!(!compile(weighted()).has_total_excess_primary());
    let mut d = standard(3);
    d.degree_rows[0].active.fill(7);
    d.degree_rows[0].inactive.fill(7);
    assert!(compile(d.clone()).has_total_excess_primary());
    d.degree_rows[0].inactive[1] = 8;
    assert!(!compile(d).has_total_excess_primary());
}
