use super::*;

fn summary<const N: usize>(
    owner: [bool; N],
    lower: [u64; N],
    upper: [Option<u64>; N],
    rank: Option<u32>,
    powers: DomainPowerBounds,
) -> DomainPowerSummary<N> {
    DomainPowerSummary::try_new(owner, &lower, &upper, rank, powers).unwrap()
}

fn finite_membership(
    owner: [bool; 2],
    lower: [u64; 2],
    upper: [u64; 2],
    rank: Option<u32>,
    powers: DomainPowerBounds,
) -> usize {
    let mut bits = 0;
    for x in lower[0]..=upper[0] {
        for y in lower[1]..=upper[1] {
            let local = [x, y];
            let mut a = 0u128;
            let mut r = 0u128;
            for axis in 0..2 {
                if owner[axis] {
                    a += u128::from(local[axis]) + 1;
                } else {
                    r += u128::from(local[axis]);
                }
            }
            let d = a as i128 - r as i128;
            if rank.is_none_or(|cap| r <= u128::from(cap))
                && powers
                    .max_positive_power
                    .is_none_or(|cap| a <= u128::from(cap))
                && powers
                    .min_power_difference
                    .is_none_or(|cap| d >= i128::from(cap))
                && powers
                    .max_power_difference
                    .is_none_or(|cap| d <= i128::from(cap))
            {
                bits |= 1 << (3 * x + y);
            }
        }
    }
    bits
}

#[test]
fn exhaustive_finite_domain_summaries_and_all_distinct_set_pairs_match_enumeration() {
    let intervals: Vec<_> = (0..=2)
        .flat_map(|lo| (lo..=2).map(move |hi| (lo, hi)))
        .collect();
    let a_caps: Vec<_> = [None].into_iter().chain((0..=6).map(Some)).collect();
    let r_caps: Vec<_> = [None].into_iter().chain((0..=4).map(Some)).collect();
    let d_ends: Vec<_> = [None].into_iter().chain((-4..=4).map(Some)).collect();
    let mut checked = 0;
    for mask in 0..4 {
        let owner = [mask & 1 != 0, mask & 2 != 0];
        // Nine possible points. Group by independently enumerated point set:
        // every raw representation must yield the SAME tight summary. Then
        // checking all distinct-set pairs checks every original domain pair
        // without repeating equivalent comparisons quadratically.
        let mut representatives: Vec<Option<DomainPowerSummary<2>>> = vec![None; 512];
        for &(l0, u0) in &intervals {
            for &(l1, u1) in &intervals {
                for &max_positive_power in &a_caps {
                    for &rank in &r_caps {
                        for &min_power_difference in &d_ends {
                            for &max_power_difference in &d_ends {
                                if min_power_difference
                                    .zip(max_power_difference)
                                    .is_some_and(|(lo, hi)| lo > hi)
                                {
                                    continue;
                                }
                                let powers = DomainPowerBounds {
                                    max_positive_power,
                                    min_power_difference,
                                    max_power_difference,
                                };
                                let bits =
                                    finite_membership(owner, [l0, l1], [u0, u1], rank, powers);
                                let current =
                                    summary(owner, [l0, l1], [Some(u0), Some(u1)], rank, powers);
                                assert_eq!(current.is_empty(), bits == 0);
                                assert_eq!(current.owner(), &owner);
                                assert!(current.contains(&current));
                                match &representatives[bits] {
                                    Some(old) => assert_eq!(
                                        &current, old,
                                        "raw-distinct equal sets have different tight extrema: mask={mask}, bits={bits}"
                                    ),
                                    None => representatives[bits] = Some(current),
                                }
                                checked += 1;
                            }
                        }
                    }
                }
            }
        }
        for (a_bits, a) in representatives
            .iter()
            .enumerate()
            .filter_map(|(bits, s)| s.as_ref().map(|s| (bits, s)))
        {
            for (b_bits, b) in representatives
                .iter()
                .enumerate()
                .filter_map(|(bits, s)| s.as_ref().map(|s| (bits, s)))
            {
                assert_eq!(
                    a.contains(b),
                    b_bits & !a_bits == 0,
                    "containment differs at mask={mask}, container={a_bits}, candidate={b_bits}"
                );
            }
        }
    }
    assert_eq!(checked, 442_368);
}

#[test]
fn redundant_a_d_bounds_are_semantically_equal_without_rewriting_inputs() {
    let a = DomainPowerBounds {
        max_positive_power: Some(4),
        ..Default::default()
    };
    let d = DomainPowerBounds {
        max_power_difference: Some(4),
        ..Default::default()
    };
    assert!(!a.contains(&d) && !d.contains(&a));
    let x = summary([true; 2], [0; 2], [Some(1); 2], None, a);
    let y = summary([true; 2], [0; 2], [Some(1); 2], None, d);
    assert_eq!(x, y);
    assert!(x.contains(&y) && y.contains(&x));
    assert_eq!(a.max_power_difference, None);
    assert_eq!(d.max_positive_power, None);
}

#[test]
fn neither_coordinate_nor_aggregate_extrema_alone_determine_inclusion() {
    let narrow = summary(
        [true; 2],
        [0; 2],
        [Some(2); 2],
        None,
        DomainPowerBounds {
            max_positive_power: Some(4),
            ..Default::default()
        },
    );
    let broad = summary(
        [true; 2],
        [0; 2],
        [Some(2); 2],
        None,
        DomainPowerBounds {
            max_positive_power: Some(6),
            ..Default::default()
        },
    );
    assert_eq!(
        narrow.extrema().unwrap().lower(),
        broad.extrema().unwrap().lower()
    );
    assert_eq!(
        narrow.extrema().unwrap().upper(),
        broad.extrema().unwrap().upper()
    );
    assert!(broad.contains(&narrow));
    assert!(!narrow.contains(&broad));

    let left = summary(
        [true; 2],
        [0; 2],
        [Some(3), Some(1)],
        None,
        Default::default(),
    );
    let right = summary(
        [true; 2],
        [0; 2],
        [Some(1), Some(3)],
        None,
        Default::default(),
    );
    let l = left.extrema().unwrap();
    let r = right.extrema().unwrap();
    assert_eq!(l.positive_power(), r.positive_power());
    assert_eq!(l.numerator_rank(), r.numerator_rank());
    assert_eq!(l.power_difference(), r.power_difference());
    assert!(!left.contains(&right) && !right.contains(&left));
}

#[test]
fn coupled_rank_and_difference_imply_an_unsupplied_positive_cap() {
    let candidate = summary(
        [true, false],
        [0; 2],
        [None; 2],
        Some(3),
        DomainPowerBounds {
            max_power_difference: Some(2),
            ..Default::default()
        },
    );
    let container = summary(
        [true, false],
        [0; 2],
        [None; 2],
        None,
        DomainPowerBounds {
            max_positive_power: Some(5),
            ..Default::default()
        },
    );
    assert_eq!(candidate.extrema().unwrap().positive_power(), (1, Some(5)));
    assert_eq!(candidate.extrema().unwrap().numerator_rank(), (0, Some(3)));
    assert!(container.contains(&candidate));
    assert!(!candidate.contains(&container));
}

#[test]
fn infinities_and_finite_wide_rank_are_never_confused() {
    let owner = [true, false];
    let unbounded = summary(owner, [0; 2], [None; 2], None, Default::default());
    let d5 = summary(
        owner,
        [0; 2],
        [None; 2],
        None,
        DomainPowerBounds {
            min_power_difference: Some(5),
            max_power_difference: Some(5),
            ..Default::default()
        },
    );
    let d6 = summary(
        owner,
        [0; 2],
        [None; 2],
        None,
        DomainPowerBounds {
            min_power_difference: Some(6),
            max_power_difference: Some(6),
            ..Default::default()
        },
    );
    assert_eq!(
        unbounded.extrema().unwrap().power_difference(),
        (None, None)
    );
    assert_eq!(d5.extrema().unwrap().positive_power(), (5, None));
    assert_eq!(d5.extrema().unwrap().numerator_rank(), (0, None));
    assert_eq!(d5.extrema().unwrap().power_difference(), (Some(5), Some(5)));
    assert!(unbounded.contains(&d5) && unbounded.contains(&d6));
    assert!(!d5.contains(&d6) && !d6.contains(&d5));
    assert!(!d5.contains(&unbounded));

    let rank32 = u64::from(u32::MAX) + 1;
    let finite = summary([false], [0], [Some(rank32)], None, Default::default());
    let capped = summary([false], [0], [None], Some(u32::MAX), Default::default());
    let infinite = summary([false], [0], [None], None, Default::default());
    assert_eq!(
        finite.extrema().unwrap().numerator_rank(),
        (0, Some(u128::from(rank32)))
    );
    assert!(finite.contains(&capped) && infinite.contains(&finite));
    assert!(!capped.contains(&finite) && !finite.contains(&infinite));
}

#[test]
fn empty_sets_and_distinct_nonempty_supports_have_explicit_semantics() {
    let empty = summary(
        [true],
        [0],
        [None],
        None,
        DomainPowerBounds {
            max_positive_power: Some(0),
            ..Default::default()
        },
    );
    let other_empty = summary([false], [1], [None], Some(0), Default::default());
    let positive = summary([true], [0], [None], None, Default::default());
    let inactive = summary([false], [0], [None], None, Default::default());
    assert!(empty.is_empty() && empty.extrema().is_none());
    assert!(other_empty.is_empty());
    assert!(empty.contains(&other_empty) && other_empty.contains(&empty));
    assert!(positive.contains(&empty) && inactive.contains(&empty));
    assert!(!empty.contains(&positive) && !empty.contains(&inactive));
    assert!(!positive.contains(&inactive) && !inactive.contains(&positive));
}

#[test]
fn wide_aggregate_and_signed_extrema_survive_without_loss() {
    let active = summary(
        [true; 2],
        [u64::MAX; 2],
        [Some(u64::MAX); 2],
        None,
        Default::default(),
    );
    let twice = 2 * (u128::from(u64::MAX) + 1);
    assert_eq!(
        active.extrema().unwrap().positive_power(),
        (twice, Some(twice))
    );
    assert_eq!(
        active.extrema().unwrap().power_difference(),
        (Some(twice as i128), Some(twice as i128))
    );
    let inactive = summary(
        [false; 2],
        [u64::MAX; 2],
        [Some(u64::MAX); 2],
        None,
        Default::default(),
    );
    let rank = 2 * u128::from(u64::MAX);
    assert_eq!(
        inactive.extrema().unwrap().numerator_rank(),
        (rank, Some(rank))
    );
    assert_eq!(
        inactive.extrema().unwrap().power_difference(),
        (Some(-(rank as i128)), Some(-(rank as i128)))
    );
    assert!(active.contains(&active) && inactive.contains(&inactive));
}

#[test]
fn malformed_and_unrepresentable_inputs_fail_before_empty_shortcuts() {
    let empty_cap = DomainPowerBounds {
        max_positive_power: Some(0),
        ..Default::default()
    };
    assert_eq!(
        DomainPowerSummary::<0>::try_new([], &[], &[], None, empty_cap),
        Err(DomainPowerError::InvalidArity)
    );
    assert_eq!(
        DomainPowerSummary::try_new([true], &[], &[None], None, empty_cap),
        Err(DomainPowerError::InvalidArity)
    );
    assert_eq!(
        DomainPowerSummary::try_new([true], &[2], &[Some(1)], None, empty_cap),
        Err(DomainPowerError::InvertedCoordinate { axis: 0 })
    );
    assert_eq!(
        DomainPowerSummary::try_new(
            [false],
            &[1],
            &[None],
            Some(0),
            DomainPowerBounds {
                min_power_difference: Some(3),
                max_power_difference: Some(2),
                ..Default::default()
            }
        ),
        Err(DomainPowerError::InvertedDifferenceBounds)
    );
    assert_eq!(
        DomainPowerSummary::try_new(
            [true, false],
            &[0, u64::MAX],
            &[None, Some(u64::MAX)],
            None,
            DomainPowerBounds {
                max_power_difference: Some(2),
                ..Default::default()
            }
        ),
        Err(DomainPowerError::OutOfRange("projected coordinate upper"))
    );
}

#[test]
fn unbounded_rank_box_only_comparisons_equal_raw_coordinate_inclusion() {
    let intervals = [
        (0, Some(0)),
        (0, Some(2)),
        (1, Some(2)),
        (0, None),
        (2, None),
    ];
    for mask in 0..4 {
        let owner = [mask & 1 != 0, mask & 2 != 0];
        for &(l0, u0) in &intervals {
            for &(l1, u1) in &intervals {
                let a = summary(owner, [l0, l1], [u0, u1], None, Default::default());
                for &(r0, v0) in &intervals {
                    for &(r1, v1) in &intervals {
                        let b = summary(owner, [r0, r1], [v0, v1], None, Default::default());
                        let raw = l0 <= r0
                            && l1 <= r1
                            && upper_contains(u0, v0)
                            && upper_contains(u1, v1);
                        assert_eq!(a.contains(&b), raw);
                    }
                }
            }
        }
    }
}

#[test]
fn redundant_rank_labels_do_not_defeat_exact_semantic_containment() {
    let rank0 = summary(
        [true, false],
        [0, 0],
        [Some(2), Some(0)],
        Some(0),
        Default::default(),
    );
    let rank7 = summary(
        [true, false],
        [0, 0],
        [Some(2), Some(0)],
        Some(7),
        Default::default(),
    );
    let rank_none = summary(
        [true, false],
        [0, 0],
        [Some(2), Some(0)],
        None,
        Default::default(),
    );
    assert_eq!(rank0, rank7);
    assert_eq!(rank0, rank_none);
    assert!(rank0.contains(&rank7) && rank7.contains(&rank_none));
    let actually_larger = summary(
        [true, false],
        [0, 0],
        [Some(2), Some(1)],
        Some(7),
        Default::default(),
    );
    assert!(!rank0.contains(&actually_larger));
}
