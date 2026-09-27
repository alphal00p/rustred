use super::super::blocks::{Block, Meta};
use super::*;

fn point(x: u64) -> DomainPowerSummary<2> {
    DomainPowerSummary::try_new(
        [true; 2],
        &[x, 100 - x],
        &[Some(x), Some(100 - x)],
        None,
        DomainPowerBounds::default(),
    )
    .unwrap()
}

fn whole_diagonal() -> DomainPowerSummary<2> {
    DomainPowerSummary::try_new(
        [true; 2],
        &[0, 0],
        &[Some(100); 2],
        None,
        DomainPowerBounds {
            max_positive_power: Some(102),
            min_power_difference: Some(102),
            max_power_difference: Some(102),
        },
    )
    .unwrap()
}

fn plain(id: usize, coordinates: Option<Coordinates<'_>>) -> Entry<'_, 2> {
    Entry {
        id,
        coordinates,
        word: 0,
        lanes: None,
    }
}

fn insert(index: &mut AggregateIndex<2>, summary: &DomainPowerSummary<2>, id: usize) {
    let coordinates = Coordinates::of(summary);
    let insertion = index.prepare(Signature::of(summary), coordinates).unwrap();
    index.insert_plain(insertion, id, coordinates);
    assert_counts(index);
}

fn assert_counts(index: &AggregateIndex<2>) {
    assert_eq!(index.live, index.ids().len());
    for group in &index.groups {
        assert_eq!(
            group.live,
            group
                .meta
                .iter()
                .map(|meta| meta.len as usize)
                .sum::<usize>()
        );
        assert_eq!(group.meta.len(), group.blocks.len());
    }
}

#[test]
fn coordinate_filters_are_necessary_for_native_inclusion_in_both_directions() {
    let mut summaries = Vec::new();
    for owner in [[true, false], [false, true], [true; 2], [false; 2]] {
        for lower in [[0, 0], [1, 0], [0, 1], [1, 1]] {
            for upper in [[Some(1); 2], [None; 2], [Some(2), None], [None, Some(2)]] {
                for rank in [Some(0), Some(2), None] {
                    for (a, d) in [(None, None), (Some(3), Some(1)), (Some(0), None)] {
                        summaries.push(
                            DomainPowerSummary::try_new(
                                owner,
                                &lower,
                                &upper,
                                rank,
                                DomainPowerBounds {
                                    max_positive_power: a,
                                    min_power_difference: d,
                                    max_power_difference: None,
                                },
                            )
                            .unwrap(),
                        );
                    }
                }
            }
        }
    }
    let mut implications = 0;
    for candidate in &summaries {
        let (mut meta, mut block) =
            Meta::<2>::prepare(Coordinates::of(candidate), &mut || Ok(())).unwrap();
        meta.push(&mut block[0], plain(0, Coordinates::of(candidate)), None);
        for query in &summaries {
            let probe = Probe::unfiltered(Coordinates::of(query));
            if candidate.contains(query) {
                assert!(meta.may_contain(&probe));
                implications += 1;
            }
            if query.contains(candidate) {
                assert!(meta.may_be_contained(&probe));
            }
        }
    }
    assert!(implications > 10_000);
}

#[test]
fn blocks_reject_disjoint_coordinates_before_native_calls_and_keep_reverse_charges() {
    let mut index = AggregateIndex::<2>::default();
    let summaries: Vec<_> = (0..65).map(point).collect();
    for (id, summary) in summaries.iter().enumerate() {
        insert(&mut index, summary, id);
    }
    assert_eq!(index.groups(), 1);
    assert_eq!(index.block_lens(0), [32, 32, 1]);
    let query = point(32);
    let mut calls = Vec::new();
    assert_eq!(
        index
            .find_each(Signature::of(&query), Coordinates::of(&query), |id| {
                calls.push(id);
                Ok(summaries[id].contains(&query))
            })
            .unwrap(),
        Some(32)
    );
    assert_eq!(calls, [32]);
    let work = index.work();
    assert_eq!(work.blocks_visited, 2);
    assert_eq!(work.blocks_rejected, 1);

    // All 65 aggregate-eligible IDs remain charged/preflighted even though the
    // coordinate envelope avoids the first and last blocks' native calls.
    assert_eq!(index.maintenance_len(Signature::of(&query)).unwrap(), 65);
    let insertion = index
        .prepare(Signature::of(&query), Coordinates::of(&query))
        .unwrap();
    let mut examined = Vec::new();
    assert_eq!(
        index.retire_each(&insertion, Coordinates::of(&query), |id| {
            examined.push(id);
            query.contains(&summaries[id])
        }),
        1
    );
    assert_eq!(examined, (32..64).collect::<Vec<_>>());
    index.insert_plain(insertion, 65, Coordinates::of(&query));
    assert_counts(&index);
    assert!(!index.is_live(Signature::of(&query), 32));
    assert!(index.is_live(Signature::of(&query), 65));
    assert_eq!(index.ids().len(), 65);
}

#[test]
fn block_watermarks_keep_the_first_live_id_at_31_32_33_boundaries() {
    let mut index = AggregateIndex::<2>::default();
    let summary = point(5);
    for id in 0..66 {
        insert(&mut index, &summary, id);
    }
    for first in [0, 31, 32, 33, 63, 64, 65, 66] {
        assert_eq!(
            index
                .find_from_each(
                    Signature::of(&summary),
                    Coordinates::of(&summary),
                    first,
                    |_| Ok(true)
                )
                .unwrap(),
            (first < 66).then_some(first)
        );
    }
    let insertion = index
        .prepare(Signature::of(&summary), Coordinates::of(&summary))
        .unwrap();
    assert_eq!(
        index.retire_each(&insertion, Coordinates::of(&summary), |id| id == 32),
        1
    );
    index.insert_plain(insertion, 66, Coordinates::of(&summary));
    assert_counts(&index);
    assert_eq!(
        index
            .find_from_each(
                Signature::of(&summary),
                Coordinates::of(&summary),
                32,
                |_| Ok(true)
            )
            .unwrap(),
        Some(33)
    );
    assert!(!index.is_live(Signature::of(&summary), 32));
}

#[test]
fn all_retired_tail_and_full_tail_insertions_use_only_prepared_storage() {
    for count in [1, 31, 32, 33, 64, 65] {
        let mut index = AggregateIndex::<2>::default();
        let summaries: Vec<_> = (0..count).map(point).collect();
        for (id, summary) in summaries.iter().enumerate() {
            insert(&mut index, summary, id);
        }
        let query = whole_diagonal();
        let insertion = index
            .prepare(Signature::of(&query), Coordinates::of(&query))
            .unwrap();
        assert_eq!(insertion.has_new_block(), count % 32 == 0);
        assert_eq!(
            index.retire_each(&insertion, Coordinates::of(&query), |id| query
                .contains(&summaries[id])),
            count as usize
        );
        assert_eq!(index.live, 0);
        assert_counts(&index);
        // The old nonfull tail is kept by its original block position; if it
        // was full, the replacement block was allocated before retirement.
        assert_eq!(index.groups[0].blocks.len(), usize::from(count % 32 != 0));
        index.insert_plain(insertion, count as usize, Coordinates::of(&query));
        assert_eq!(index.ids(), [count as usize]);
        assert_eq!(index.groups[0].blocks.len(), 1);
        assert_eq!(index.live, 1);
        assert_counts(&index);
        for id in 0..count as usize {
            assert!(!index.is_live(Signature::of(&query), id));
        }
        assert!(index.is_live(Signature::of(&query), count as usize));
    }
}

#[test]
fn envelope_allocation_refusals_leave_live_ids_keys_and_retirement_unchanged() {
    let summary = point(0);
    for existing in [0, 32] {
        let mut index = AggregateIndex::<2>::default();
        for id in 0..existing {
            insert(&mut index, &summary, id);
        }
        let checkpoints = if existing == 0 { 4 } else { 2 };
        for fail_at in 0..checkpoints {
            let mut step = 0;
            let result =
                index.prepare_with(Signature::of(&summary), Coordinates::of(&summary), || {
                    let fail = step == fail_at;
                    step += 1;
                    if fail {
                        Err("injected block allocation refusal")
                    } else {
                        Ok(())
                    }
                });
            assert!(matches!(result, Err("injected block allocation refusal")));
            assert_eq!(index.ids(), (0..existing).collect::<Vec<_>>());
            assert_eq!(index.live, existing);
            assert_eq!(index.groups(), usize::from(existing != 0));
            assert_counts(&index);
        }
    }
}

#[test]
fn stale_outward_envelopes_and_infinite_upper_coordinates_only_create_false_positives() {
    let make = |upper: [Option<u64>; 2]| {
        DomainPowerSummary::try_new(
            [true; 2],
            &[0; 2],
            &upper,
            None,
            DomainPowerBounds::default(),
        )
        .unwrap()
    };
    let candidates = [make([None, Some(1)]), make([Some(1), None])];
    let query = DomainPowerSummary::try_new(
        [true; 2],
        &[5; 2],
        &[Some(5); 2],
        None,
        DomainPowerBounds::default(),
    )
    .unwrap();
    let (mut meta, mut block) =
        Meta::<2>::prepare(Coordinates::of(&candidates[0]), &mut || Ok(())).unwrap();
    meta.push(
        &mut block[0],
        plain(0, Coordinates::of(&candidates[0])),
        None,
    );
    meta.push(
        &mut block[0],
        plain(1, Coordinates::of(&candidates[1])),
        None,
    );
    let probe = |summary| Probe::unfiltered(Coordinates::of(summary));
    assert!(meta.may_contain(&probe(&query))); // Loose hull, not proof.
    assert!(
        candidates
            .iter()
            .all(|candidate| !candidate.contains(&query))
    );
    assert_eq!(meta.retain(&mut block[0], 0b10), 1);
    assert_eq!((meta.first, meta.last, meta.len), (1, 1, 1));
    assert!(meta.may_contain(&probe(&candidates[1])));
    let finite_query = make([Some(1); 2]);
    assert!(meta.may_be_contained(&probe(&finite_query))); // Stale min.
    assert!(!finite_query.contains(&candidates[1]));
    let maximum = make([Some(u64::MAX); 2]);
    assert!(!maximum.contains(&candidates[1])); // Infinity is not u64::MAX.
}

#[test]
fn empty_domains_bypass_coordinates_in_both_directions() {
    let empty = DomainPowerSummary::try_new(
        [true; 2],
        &[0; 2],
        &[Some(1); 2],
        None,
        DomainPowerBounds {
            max_positive_power: Some(0),
            ..Default::default()
        },
    )
    .unwrap();
    let nonempty = point(0);
    let mut index = AggregateIndex::<2>::default();
    insert(&mut index, &empty, 0);
    assert_eq!(
        index
            .find_each(
                Signature::of(&nonempty),
                Coordinates::of(&nonempty),
                |_| panic!("empty aggregate cannot contain nonempty domain")
            )
            .unwrap(),
        None
    );
    insert(&mut index, &nonempty, 1);
    assert_eq!(
        index
            .find_each(Signature::of(&empty), Coordinates::of(&empty), |_| Ok(true))
            .unwrap(),
        Some(0)
    );
    let insertion = index
        .prepare(Signature::of(&nonempty), Coordinates::of(&nonempty))
        .unwrap();
    assert_eq!(
        index.retire_each(&insertion, Coordinates::of(&nonempty), |id| id == 0),
        1
    );
    index.insert_plain(insertion, 2, Coordinates::of(&nonempty));
    assert_eq!(
        index
            .find_each(Signature::of(&empty), Coordinates::of(&empty), |_| Ok(true))
            .unwrap(),
        Some(1)
    );
}

#[test]
fn cancellation_is_polled_when_every_coordinate_block_rejects() {
    let mut index = AggregateIndex::<2>::default();
    for id in 0..65 {
        insert(&mut index, &point(id as u64), id);
    }
    let query = point(99);
    let mut checkpoints = 0;
    assert_eq!(
        index.find_controlled_each(
            Signature::of(&query),
            Coordinates::of(&query),
            0,
            || {
                checkpoints += 1;
                if checkpoints == 3 {
                    Err("cancelled speculative lookup")
                } else {
                    Ok(())
                }
            },
            |_| panic!("all completed block tests reject before native containment")
        ),
        Err("cancelled speculative lookup")
    );
    assert_eq!(index.ids(), (0..65).collect::<Vec<_>>());
    assert_eq!(index.work().blocks_rejected, 1);
}

#[test]
fn complete_reused_and_novel_proposals_preserve_native_ids_and_every_retirement_set() {
    let mut proposals: Vec<_> = (0..96).map(point).collect();
    proposals.extend((0..96).rev().map(point)); // Actual reused requests, not only admissions.
    for (lo, hi) in [(10, 70), (20, 80), (5, 90), (0, 100)] {
        proposals.push(
            DomainPowerSummary::try_new(
                [true; 2],
                &[lo, 100 - hi],
                &[Some(hi), Some(100 - lo)],
                None,
                DomainPowerBounds {
                    max_positive_power: Some(102),
                    min_power_difference: Some(102),
                    max_power_difference: Some(102),
                },
            )
            .unwrap(),
        );
        proposals.extend((0..=100).map(point));
    }
    let mut index = AggregateIndex::<2>::default();
    let mut stored: Vec<DomainPowerSummary<2>> = Vec::new();
    let mut live = std::collections::BTreeSet::<usize>::new();
    let mut reused = 0;
    for query in proposals {
        let signature = Signature::of(&query);
        let coordinates = Coordinates::of(&query);
        let expected = live.iter().copied().find(|&id| stored[id].contains(&query));
        let actual = index
            .find_each(signature, coordinates, |id| Ok(stored[id].contains(&query)))
            .unwrap();
        assert_eq!(actual, expected);
        if actual.is_some() {
            reused += 1;
        } else {
            let retired: Vec<_> = live
                .iter()
                .copied()
                .filter(|&id| query.contains(&stored[id]))
                .collect();
            let insertion = index.prepare(signature, coordinates).unwrap();
            let mut actual_retired = Vec::new();
            assert_eq!(
                index.retire_each(&insertion, coordinates, |id| {
                    let remove = query.contains(&stored[id]);
                    if remove {
                        actual_retired.push(id);
                    }
                    remove
                }),
                retired.len()
            );
            actual_retired.sort_unstable();
            assert_eq!(actual_retired, retired);
            for id in retired {
                live.remove(&id);
            }
            let id = stored.len();
            index.insert_plain(insertion, id, coordinates);
            live.insert(id);
            stored.push(query);
        }
        assert_eq!(index.ids(), live.iter().copied().collect::<Vec<_>>());
        assert_counts(&index);
    }
    assert!(stored.len() >= 100);
    assert!(reused > 400);
    assert_eq!(live.len(), 1);
    assert!(index.work().blocks_rejected > 0);
}

#[test]
fn block_storage_reports_real_capacity_and_releases_removed_blocks() {
    let mut index = AggregateIndex::<2>::default();
    for id in 0..65 {
        insert(&mut index, &point(id as u64), id);
    }
    let block_bytes = std::mem::size_of::<Block<2>>();
    let before = index.block_storage();
    assert_eq!(before.live_ids, 65);
    assert_eq!(before.blocks, 3);
    assert_eq!(before.id_slots, 96);
    assert!(before.block_capacity_bytes >= 3 * block_bytes);
    assert!(before.envelope_capacity_bytes >= 3 * std::mem::size_of::<Meta<2>>());
    println!("coordinate_block_storage_before={before:?}");

    let query = whole_diagonal();
    let insertion = index
        .prepare(Signature::of(&query), Coordinates::of(&query))
        .unwrap();
    assert_eq!(
        index.retire_each(&insertion, Coordinates::of(&query), |_| true),
        65
    );
    index.insert_plain(insertion, 65, Coordinates::of(&query));
    let after = index.block_storage();
    assert_eq!(after.live_ids, 1);
    assert_eq!(after.blocks, 1);
    assert_eq!(after.id_slots, 32);
    // Removed boxed blocks are freed; the row and pointer vectors keep their
    // reserved capacity.
    assert_eq!(
        after.block_capacity_bytes + 2 * block_bytes,
        before.block_capacity_bytes
    );
    assert_eq!(
        after.envelope_capacity_bytes,
        before.envelope_capacity_bytes
    );
    println!(
        "coordinate_block_storage_after={after:?}; removed boxed blocks are dropped; outer capacity remains reserved"
    );
    println!(
        "coordinate_block_layout arity2 block_bytes={} row_bytes={}; arity15 block_bytes={} row_bytes={} (allocator/group/hash overhead excluded)",
        block_bytes,
        std::mem::size_of::<Meta<2>>(),
        std::mem::size_of::<Block<15>>(),
        std::mem::size_of::<Meta<15>>()
    );
}
