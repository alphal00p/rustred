use super::*;
use rustred::solver::DomainPowerBounds;
use std::collections::BTreeSet;

fn limits(points: usize) -> WindowLimits {
    WindowLimits {
        max_seed_points: points,
        max_seed_bytes: 1024 * 1024,
        max_positive_layers: 64,
        max_partition_checks: 4096,
    }
}
fn region<const N: usize>(
    support: [bool; N],
    a: u64,
    r: u32,
    lo: Option<i64>,
    hi: Option<i64>,
) -> RootRegionInput<N> {
    RootRegionInput {
        support,
        lower: vec![0; N],
        upper: vec![None; N],
        rank: Some(r),
        powers: DomainPowerBounds {
            max_positive_power: Some(a),
            min_power_difference: lo,
            max_power_difference: hi,
        },
    }
}
fn source<const N: usize>(region: RootRegionInput<N>, points: usize) -> PrefixWindows<N> {
    PrefixWindows::try_new(
        "test-origin",
        region,
        N,
        limits(points),
        &AtomicBool::new(false),
    )
    .unwrap()
}
fn collect<const N: usize>(
    source: &PrefixWindows<N>,
) -> (Vec<IntegralKey>, Vec<WindowTransition>, PrefixCursor) {
    let cancel = AtomicBool::new(false);
    let mut cursor = source.initial_cursor();
    let mut keys = Vec::new();
    let mut records = Vec::new();
    while let Some(window) = source.next_window(&cursor, &cancel).unwrap() {
        assert!(!window.targets().is_empty());
        assert!(window.targets().len() <= source.limits.max_seed_points);
        assert_eq!(Integer::from(window.targets().len()), *window.count());
        let validated = source
            .validate_transition(&cursor, window.transition(), &cancel)
            .unwrap();
        assert_eq!(&validated, window.successor());
        let roundtrip = WindowTransition::decode(&window.transition().encode().unwrap()).unwrap();
        assert_eq!(&roundtrip, window.transition());
        records.push(roundtrip);
        keys.extend_from_slice(window.targets());
        cursor = window.successor().clone();
        assert!(cursor.0.frames.len() <= source.arity);
    }
    assert!(cursor.is_exhausted());
    assert_eq!(cursor.emitted(), source.total_count());
    assert_eq!(Integer::from(keys.len()), *source.total_count());
    assert_eq!(keys.iter().collect::<BTreeSet<_>>().len(), keys.len());
    let restored = source
        .audit_restore_cursor(&cursor.encode().unwrap(), &records, &cancel)
        .unwrap();
    assert_eq!(cursor, restored);
    (keys, records, cursor)
}
fn whole<const N: usize>(region: &RootRegionInput<N>) -> BTreeSet<IntegralKey> {
    FiniteEntryDomain::new(
        region.support.to_vec(),
        EntryPowerBudget {
            max_positive_power: region
                .powers
                .max_positive_power
                .unwrap()
                .try_into()
                .unwrap(),
            max_numerator_rank: region.rank.unwrap(),
            min_power_difference: region
                .powers
                .min_power_difference
                .map(|v| v.try_into().unwrap()),
            max_power_difference: region
                .powers
                .max_power_difference
                .map(|v| v.try_into().unwrap()),
        },
    )
    .unwrap()
    .targets()
    .collect::<std::result::Result<_, _>>()
    .unwrap()
}

#[test]
fn native_prefix_windows_match_complete_envelopes_for_all_small_supports_and_sizes() {
    for bits in 0..8 {
        let support = std::array::from_fn::<_, 3, _>(|axis| bits & (1 << axis) != 0);
        for (a, r, lo, hi) in [
            (5, 3, None, None),
            (5, 3, Some(1), Some(2)),
            (3, 2, Some(-2), Some(3)),
        ] {
            let input = region(support, a, r, lo, hi);
            let expected = whole(&input);
            for size in [1, 2, 7, 1000] {
                let src = source(input.clone(), size);
                let (keys, _, _) = collect(&src);
                assert_eq!(keys.into_iter().collect::<BTreeSet<_>>(), expected);
            }
        }
    }
}

#[test]
fn fixed_active_baseline_one_and_negative_powers_scatter_exactly() {
    let src = source(region([true, false], 5, 4, Some(2), Some(2)), 1);
    let (keys, _, _) = collect(&src);
    assert_eq!(
        keys.into_iter()
            .map(|key| key.powers().to_vec())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([vec![2, 0], vec![3, -1], vec![4, -2], vec![5, -3]])
    );
    let b = residual(Some(9), Some(6), Some(3), Some(7), 4, 2)
        .unwrap()
        .unwrap();
    assert_eq!(
        b,
        EntryPowerBudget {
            max_positive_power: 5,
            max_numerator_rank: 4,
            min_power_difference: Some(1),
            max_power_difference: Some(5)
        }
    );
    assert!(
        residual(Some(1), Some(6), None, None, 2, 0)
            .unwrap()
            .is_none()
    );
    assert!(
        residual(Some(9), Some(1), None, None, 0, 2)
            .unwrap()
            .is_none()
    );
}

#[test]
fn all_fixed_singleton_uses_original_arity_without_dummy_axis() {
    let mut input = region([true, false, true], 8, 4, Some(1), Some(8));
    input.lower = vec![2, 3, 1];
    input.upper = vec![Some(2), Some(3), Some(1)];
    let src = source(input, 1);
    let (keys, _, cursor) = collect(&src);
    assert_eq!(keys[0].powers(), &[3, -3, 2]);
    assert_eq!(cursor.windows(), 1);
}

#[test]
fn original_and_conditioned_empty_domains_are_not_clamped_into_points() {
    let src = source(region([true, true], 1, 0, None, None), 1);
    let (keys, records, cursor) = collect(&src);
    assert!(keys.is_empty() && records.is_empty() && cursor.is_exhausted());
    let src = source(region([true, false], 2, 1, None, None), 1);
    assert_eq!(src.node(&[5], &AtomicBool::new(false)).unwrap().count, 0);
    assert_eq!(src.node(&[0, 5], &AtomicBool::new(false)).unwrap().count, 0);
}

#[test]
fn arbitrary_nonuniform_or_shifted_geometry_refuses_instead_of_filtering() {
    let cancel = AtomicBool::new(false);
    let mut input = region([true, false], 5, 4, None, None);
    input.upper[0] = Some(1);
    assert_eq!(
        PrefixWindows::try_new("q", input, 2, limits(2), &cancel)
            .err()
            .unwrap()
            .kind,
        ErrorKind::UnsupportedGeometry
    );
    let mut input = region([true, false], 5, 4, None, None);
    input.lower[0] = 1;
    assert_eq!(
        PrefixWindows::try_new("q", input, 2, limits(2), &cancel)
            .err()
            .unwrap()
            .kind,
        ErrorKind::UnsupportedGeometry
    );
    let mut input = region([true, false], 5, 4, None, None);
    input.lower[0] = 2;
    input.upper[0] = Some(1);
    assert_eq!(
        PrefixWindows::try_new("q", input, 2, limits(2), &cancel)
            .err()
            .unwrap()
            .kind,
        ErrorKind::Input
    );
}

#[test]
fn cursor_replay_rejects_skipped_repeated_changed_and_wrong_source_transitions() {
    let src = source(region([true, false, true], 6, 2, None, None), 2);
    let (_, records, final_cursor) = collect(&src);
    assert!(records.len() > 3);
    let cancel = AtomicBool::new(false);
    let initial = src.initial_cursor();
    assert_eq!(
        src.validate_transition(&initial, &records[1], &cancel)
            .err()
            .unwrap()
            .kind,
        ErrorKind::Cursor
    );
    assert!(
        src.audit_restore_cursor(
            &final_cursor.encode().unwrap(),
            records.iter().skip(1),
            &cancel
        )
        .is_err()
    );
    assert!(
        src.audit_restore_cursor(
            &final_cursor.encode().unwrap(),
            records.iter().chain(&records[..1]),
            &cancel
        )
        .is_err()
    );
    let mut bad = records[0].clone();
    bad.count += 1;
    assert_eq!(
        src.validate_transition(&initial, &bad, &cancel)
            .err()
            .unwrap()
            .kind,
        ErrorKind::Cursor
    );
    let mut bad = records[0].clone();
    bad.after.prefix[0] += 1;
    assert_eq!(
        src.validate_transition(&initial, &bad, &cancel)
            .err()
            .unwrap()
            .kind,
        ErrorKind::Cursor
    );
    let mut bad = records[0].clone();
    bad.version += 1;
    assert!(src.validate_transition(&initial, &bad, &cancel).is_err());
    for field in 0..3 {
        let mut bad = records[0].clone();
        let frame = &mut bad.after.frames[0];
        match field {
            0 => frame.current += 1,
            1 => frame.completed_children += 1,
            _ => frame.parent_count += 1,
        }
        assert_eq!(
            src.validate_transition(&initial, &bad, &cancel)
                .err()
                .unwrap()
                .kind,
            ErrorKind::Cursor
        );
    }
    let other = source(region([true, false, true], 7, 2, None, None), 2);
    assert!(other.next_window(&initial, &cancel).is_err());
    let changed_origin = PrefixWindows::try_new(
        "another-origin",
        region([true, false, true], 6, 2, None, None),
        3,
        limits(2),
        &cancel,
    )
    .unwrap();
    assert!(changed_origin.next_window(&initial, &cancel).is_err());
    let changed_limits = source(region([true, false, true], 6, 2, None, None), 3);
    assert!(changed_limits.next_window(&initial, &cancel).is_err());
    let mut saved = final_cursor.0.clone();
    saved.version += 1;
    assert!(
        src.audit_restore_cursor(&encode(&saved).unwrap(), &records, &cancel)
            .is_err()
    );
    let mut bytes = final_cursor.encode().unwrap();
    bytes.push(0);
    assert!(src.audit_restore_cursor(&bytes, &records, &cancel).is_err());
}

#[test]
fn resume_replays_only_count_metadata_and_preserves_next_exact_window() {
    let src = source(region([true, false, true], 7, 3, None, None), 3);
    let cancel = AtomicBool::new(false);
    let first = src
        .next_window(&src.initial_cursor(), &cancel)
        .unwrap()
        .unwrap();
    let restored = src
        .audit_restore_cursor(
            &first.successor().encode().unwrap(),
            [first.transition()],
            &cancel,
        )
        .unwrap();
    let direct = src
        .next_window(first.successor(), &cancel)
        .unwrap()
        .unwrap();
    let resumed = src.next_window(&restored, &cancel).unwrap().unwrap();
    assert_eq!(direct.targets(), resumed.targets());
    assert_eq!(direct.transition(), resumed.transition());
}

#[test]
fn resource_and_cancelled_proposals_never_advance_the_caller_cursor() {
    let cancel = AtomicBool::new(false);
    let input = region([true, false, true], 6, 3, None, None);
    for caps in [
        WindowLimits {
            max_seed_points: 0,
            ..limits(1)
        },
        WindowLimits {
            max_seed_bytes: 0,
            ..limits(1)
        },
        WindowLimits {
            max_partition_checks: 0,
            ..limits(1)
        },
    ] {
        let src = PrefixWindows::try_new("q", input.clone(), 3, caps, &cancel).unwrap();
        let cursor = src.initial_cursor();
        let before = cursor.clone();
        assert_eq!(
            src.next_window(&cursor, &cancel).err().unwrap().kind,
            ErrorKind::Resource
        );
        assert_eq!(cursor, before);
    }
    let src = source(input.clone(), 1);
    let cursor = src.initial_cursor();
    cancel.store(true, Ordering::Release);
    assert_eq!(
        src.next_window(&cursor, &cancel).err().unwrap().kind,
        ErrorKind::Cancelled
    );
    assert_eq!(cursor, src.initial_cursor());
    cancel.store(false, Ordering::Release);
    let caps = WindowLimits {
        max_positive_layers: 0,
        ..limits(1)
    };
    assert_eq!(
        PrefixWindows::try_new("q", input, 3, caps, &cancel)
            .err()
            .unwrap()
            .kind,
        ErrorKind::Resource
    );
}

#[test]
fn generic_native_range_and_checked_arithmetic_refusals_do_not_wrap() {
    let input = region([true, false], u64::MAX, 1, None, None);
    assert_eq!(
        PrefixWindows::try_new("q", input, 2, limits(1), &AtomicBool::new(false))
            .err()
            .unwrap()
            .kind,
        ErrorKind::UnsupportedGeometry
    );
    assert_eq!(
        residual(Some(5), Some(2), Some(i64::MAX), None, 0, 0)
            .err()
            .unwrap()
            .kind,
        ErrorKind::UnsupportedGeometry
    );
    assert_eq!(
        residual(Some(5), Some(2), None, None, i128::MIN, 0)
            .err()
            .unwrap()
            .kind,
        ErrorKind::Resource
    );
    let src = source(region([true], 3, 0, None, None), 1);
    assert_eq!(
        src.scatter(&[i64::MAX as u64], &[]).err().unwrap().kind,
        ErrorKind::Resource
    );
}

#[test]
fn very_large_count_retains_only_lazy_sibling_frames_and_one_bounded_window() {
    let src = source(region([false; 4], 0, 1_000_000, None, None), 2);
    assert!(*src.total_count() > Integer::from(usize::MAX));
    let window = src
        .next_window(&src.initial_cursor(), &AtomicBool::new(false))
        .unwrap()
        .unwrap();
    assert_eq!(window.targets()[0].powers(), &[0, 0, 0, 0]);
    assert!(window.targets().len() <= 2);
    assert!(window.successor().0.frames.len() <= 4);
    assert!(window.successor().encode().unwrap().len() < 1024);
}

#[test]
fn overlapping_query_occurrences_remain_distinct_from_union_keys() {
    let a = region([true, false], 4, 2, None, None);
    let b = region([true, false], 3, 1, None, None);
    let (left, _, _) = collect(&source(a.clone(), 2));
    let (right, _, _) = collect(&source(b.clone(), 1));
    let occurrences = left.len() + right.len();
    let union = left.into_iter().chain(right).collect::<BTreeSet<_>>();
    let expected = whole(&a)
        .union(&whole(&b))
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(union, expected);
    assert!(occurrences > union.len());
}

#[cfg(feature = "capacity-dispatch")]
#[test]
fn physical_two_coordinate_source_matches_capacity_four_without_padding_compositions() {
    let original = region([true, false], 5, 3, Some(1), Some(3));
    let (expected, _, _) = collect(&source(original.clone(), 2));
    let mut padded = region([true, false, false, false], 5, 3, Some(1), Some(3));
    padded.upper[2..].fill(Some(0));
    let src = PrefixWindows::try_new(
        "padded",
        padded.clone(),
        2,
        limits(2),
        &AtomicBool::new(false),
    )
    .unwrap();
    let (actual, _, _) = collect(&src);
    assert_eq!(actual, expected);
    assert!(actual.iter().all(|key| key.powers().len() == 2));
    padded.upper[3] = None;
    assert_eq!(
        PrefixWindows::try_new("bad-pad", padded, 2, limits(2), &AtomicBool::new(false))
            .err()
            .unwrap()
            .kind,
        ErrorKind::Input
    );
}
