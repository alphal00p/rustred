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
    assert_eq!(index.storage(), index.storage_by_walk());
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

/// The running storage totals equal the full walk after every mutation:
/// narrow and exact (wide) envelopes, narrow tails widened to exact storage,
/// lossy and escaped slots, retirements that empty blocks and remove groups,
/// and preparations that are refused mid-way or abandoned after reserving.
#[test]
fn running_storage_totals_equal_the_full_walk_after_every_mutation() {
    let mut state = 0x5DEE_CE66_D1CE_4E5B_u64;
    let mut below = move |n: u64| {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 33) % n
    };
    let mut index = AggregateIndex::<2>::default();
    let (mut next_id, mut refused, mut abandoned, mut groups_removed) = (0, 0, 0, 0);
    let (mut wide_rows, mut lossy_peak, mut lossy_retired) = (0, 0, 0);
    // Two full blocks in a group no random insertion retires (signature 9):
    // the existing-group new-block path, then refusals at both of its
    // reservation checkpoints on a full tail.
    let full = signature(9);
    let point_summary = point(7);
    for _ in 0..64 {
        let insertion = index
            .prepare(full, Coordinates::of(&point_summary))
            .unwrap();
        index.insert_plain(insertion, next_id, Coordinates::of(&point_summary));
        next_id += 1;
        assert_counts(&index);
    }
    assert_eq!(index.block_lens(0), [32, 32]);
    for fail_at in 0..2 {
        let mut step = 0;
        let result = index.prepare_with(full, Coordinates::of(&point_summary), || {
            let fail = step == fail_at;
            step += 1;
            if fail {
                Err("injected refusal")
            } else {
                Ok(())
            }
        });
        assert!(result.is_err());
        refused += 1;
        assert_counts(&index);
    }
    for _ in 0..6_000 {
        // Coordinates: small (narrow codes) or beyond 65534 (exact storage).
        let mut value = || {
            if below(8) == 0 {
                70_000 + below(1_000)
            } else {
                below(300)
            }
        };
        let lower = [value(), value()];
        let upper = [
            (below(4) != 0).then(|| lower[0] + below(50)),
            (below(4) != 0).then(|| lower[1] + below(50)),
        ];
        let summary = DomainPowerSummary::try_new(
            [true; 2],
            &lower,
            &upper,
            None,
            DomainPowerBounds::default(),
        )
        .unwrap();
        // Some insertions carry no coordinates (an absent envelope).
        let coordinates = (below(10) != 0)
            .then(|| Coordinates::of(&summary))
            .flatten();
        let key = signature(u128::from(below(4)));
        match below(20) {
            0 => {
                // A preparation refused at a random reservation checkpoint;
                // half of them for a group that never exists (signatures
                // 10-13), whose path has four checkpoints.
                let key = if below(2) == 0 {
                    signature(10 + u128::from(below(4)))
                } else {
                    key
                };
                let fail_at = below(4);
                let mut step = 0;
                let result = index.prepare_with(key, coordinates, || {
                    let fail = step == fail_at;
                    step += 1;
                    if fail {
                        Err("injected refusal")
                    } else {
                        Ok(())
                    }
                });
                refused += usize::from(result.is_err());
            }
            1 => {
                // Storage reserved, then the admission failed a later queue
                // preflight and dropped its insertion.
                drop(index.prepare(key, coordinates).unwrap());
                abandoned += 1;
            }
            _ => {
                let insertion = index.prepare(key, coordinates).unwrap();
                let percent = [0, 2, 30][below(3) as usize];
                let (groups, lossy) = (index.groups(), index.storage().lossy);
                index.retire_each(&insertion, coordinates, |_| below(100) < percent);
                assert_counts(&index);
                groups_removed += groups - index.groups();
                lossy_retired += lossy - index.storage().lossy;
                let lanes = (below(5) != 0).then(|| Lanes {
                    lower: [0; 2],
                    upper: [0; 2],
                    power: [0; 6],
                    lossy: below(3) == 0,
                });
                index.insert(
                    insertion,
                    Entry {
                        id: next_id,
                        coordinates,
                        word: below(1 << 20),
                        lanes,
                    },
                );
                next_id += 1;
            }
        }
        assert_counts(&index);
        lossy_peak = lossy_peak.max(index.storage().lossy);
        wide_rows = wide_rows.max(
            index
                .groups
                .iter()
                .flat_map(|group| &group.meta)
                .filter(|meta| meta.envelope_capacity_bytes() > 0)
                .count(),
        );
    }
    println!(
        "storage_totals inserted={next_id} refused={refused} abandoned={abandoned} groups_removed={groups_removed} wide_rows_peak={wide_rows} lossy_peak={lossy_peak} lossy_retired={lossy_retired} final={:?}",
        index.storage()
    );
    // Every path of the totals was exercised.
    assert!(refused > 50 && abandoned > 100 && groups_removed > 0);
    assert!(
        index.block_lens(0).len() >= 2,
        "the full group was never retired"
    );
    assert!(wide_rows > 0 && lossy_peak > 0 && lossy_retired > 0);
}

type Layout = Vec<(Signature, Vec<Vec<usize>>)>;

/// The historical retirement layout rule on a physical layout: retire the
/// given IDs from every block of every group the insertion signature may
/// contain, then keep the non-empty rows (and a pinned tail) in order, and
/// swap-remove emptied groups other than the insertion's own. `retired` is
/// sorted.
fn reference_retire(before: &Layout, retired: &[usize], key: Signature, pin_tail: bool) -> Layout {
    let mut groups = before.clone();
    let mut position = 0;
    while position < groups.len() {
        let (signature, blocks) = &mut groups[position];
        if key.may_contain(*signature) {
            for block in blocks.iter_mut() {
                block.retain(|id| retired.binary_search(id).is_err());
            }
            let pin = *signature == key && pin_tail;
            let old_len = blocks.len();
            let mut row = 0;
            blocks.retain(|block| {
                row += 1;
                !block.is_empty() || (pin && row == old_len)
            });
        }
        if groups[position].1.is_empty() && groups[position].0 != key {
            groups.swap_remove(position);
        } else {
            position += 1;
        }
    }
    groups
}

/// Compaction moves rows only from the first empty row on (and only when an
/// earlier row was dropped), yet leaves the layout of the historical loop
/// that swapped every kept row into place: retirements of nothing, of single
/// IDs, of whole blocks mid-group and of whole groups, pinned tails,
/// new-block and new-group insertions, with the running totals equal to the
/// full walk. Insertions come in runs of one signature so that blocks hold
/// consecutive IDs and an ID window can empty blocks in the middle of a group.
#[test]
fn retire_compaction_keeps_the_historical_layout() {
    let mut state = 0x2545_F491_4F6C_DD1D_u64;
    let mut below = move |n: u64| {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 33) % n
    };
    fn mix(id: usize, step: usize) -> u64 {
        let mut x = (id as u64) ^ ((step as u64) << 32) ^ 0x9E37_79B9_7F4A_7C15;
        x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x ^= x >> 31;
        x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
        x ^ (x >> 29)
    }
    let mut index = AggregateIndex::<2>::default();
    let (mut moved, mut emptied_mid, mut pinned_empty, mut max_rows) = (0, 0, 0, 0);
    let mut key = signature(0);
    for step in 0..8_000_usize {
        if below(100) == 0 {
            key = signature(u128::from(below(4)));
        }
        // 1 in 40: an ID window from a random live ID (whole blocks); 1 in
        // 40: 5% of the examined IDs; otherwise nothing.
        let mode = below(40);
        let live = index.ids();
        let lo = if live.is_empty() {
            0
        } else {
            live[below(live.len() as u64) as usize]
        };
        let width = 32 + below(96) as usize;
        let insertion = index.prepare(key, None).unwrap();
        let pin_tail = !insertion.has_new_block();
        let before = index.layout();
        let mut retired = Vec::new();
        index.retire_each(&insertion, None, |id| {
            let retire = match mode {
                0 => id >= lo && id < lo + width,
                1 => mix(id, step) % 100 < 5,
                _ => false,
            };
            if retire {
                retired.push(id);
            }
            retire
        });
        assert!(retired.iter().all(|id| live.binary_search(id).is_ok()));
        retired.sort_unstable();
        let expected = reference_retire(&before, &retired, key, pin_tail);
        assert_eq!(index.layout(), expected);
        assert_counts(&index);
        // Statistics of the paths taken: kept rows that had to move, emptied
        // rows before a group's last row, and an emptied pinned tail.
        for (signature, blocks) in &before {
            if !key.may_contain(*signature) {
                continue;
            }
            let emptied: Vec<bool> = blocks
                .iter()
                .map(|block| block.iter().all(|id| retired.binary_search(id).is_ok()))
                .collect();
            if let Some(first) = emptied.iter().position(|&e| e) {
                moved += emptied[first..].iter().filter(|&&e| !e).count();
                emptied_mid += usize::from(first + 1 < emptied.len());
            }
            pinned_empty +=
                usize::from(*signature == key && pin_tail && emptied.last() == Some(&true));
            max_rows = max_rows.max(blocks.len());
        }
        index.insert_plain(insertion, step, None);
        assert_counts(&index);
    }
    println!(
        "retire_compaction inserted=8000 live={} moved_rows={moved} emptied_mid={emptied_mid} pinned_empty_tails={pinned_empty} max_rows={max_rows} groups={}",
        index.live,
        index.groups()
    );
    // A Python replay of this exact stream (kernel2 lane) gives 783 / 101 / 17.
    assert!(moved > 300 && emptied_mid > 50 && pinned_empty > 5);
}

/// An image restored with empty rows mid-group (the validator admits them)
/// loses them at the next eligible retirement, even one that retires
/// nothing, exactly as the historical compaction did; an empty pinned tail
/// is kept for the insertion.
#[test]
fn retire_drops_restored_empty_rows_even_without_retirements() {
    let stored = |len_and_ids: &[&[usize]]| {
        let mut live = 0;
        let blocks = len_and_ids
            .iter()
            .map(|ids| {
                let mut slots = [0; BLOCK_SIZE];
                slots[..ids.len()].copy_from_slice(ids);
                live += ids.len();
                StoredBlock {
                    ids: slots,
                    len: ids.len(),
                    envelope: Vec::new(),
                }
            })
            .collect();
        StoredIndex {
            groups: vec![StoredGroup {
                signature: signature(1),
                blocks,
                live,
            }],
            live,
        }
    };
    let restore = |image| AggregateIndex::<2>::restore(image, 20, |_| Ok((0, None))).unwrap();
    for (key, pin_tail) in [(signature(3), false), (signature(1), true)] {
        let mut index = restore(stored(&[&[0, 1, 2], &[], &[5, 6], &[]]));
        assert_eq!(index.block_lens(0), [3, 0, 2, 0]);
        assert_counts(&index);
        let insertion = index.prepare(key, None).unwrap();
        assert_eq!(!insertion.has_new_block(), pin_tail);
        let before = index.layout();
        assert_eq!(index.retire_each(&insertion, None, |_| false), 0);
        let expected = reference_retire(&before, &[], key, pin_tail);
        assert_eq!(index.layout(), expected);
        assert_eq!(
            index.block_lens(0),
            if pin_tail { vec![3, 2, 0] } else { vec![3, 2] }
        );
        assert_counts(&index);
        index.insert_plain(insertion, 7, None);
        assert_counts(&index);
    }
}

/// The watermark shortcuts (a forward scan from ID 0 skips no block, a scan
/// whose watermark is above every block skips the group, a reverse run wholly
/// below the prepared watermark needs no split search) keep the minimum-ID
/// result, never test an ID below the watermark, and hand every examined
/// candidate to exactly one of: the prepared set (old IDs), the prefilter or
/// the commit-time predicate (new IDs), with the historical layout.
#[test]
fn watermark_shortcuts_keep_minimum_ids_and_the_prepared_split() {
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    let mut below = move |n: u64| {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 33) % n
    };
    struct Record<F> {
        decided: usize,
        rejected: Vec<usize>,
        tested: Vec<usize>,
        predicate: F,
    }
    impl<F: FnMut(usize) -> bool> Retire for Record<F> {
        fn rejected(&mut self, run: &[u32], _: u32) {
            self.rejected.extend(run.iter().map(|&id| id as usize));
        }
        fn test(&mut self, id: usize) -> bool {
            self.tested.push(id);
            (self.predicate)(id)
        }
        fn decided(&mut self, count: usize) {
            self.decided += count;
        }
    }
    let mut index = AggregateIndex::<2>::default();
    let mut words = Vec::new();
    let (mut shortcuts, mut splits) = (0, 0);
    for next_id in 0..4_000_usize {
        let key = signature(u128::from(below(4)));
        let before = index.layout();
        // Forward: brute-force minimum over the groups that may contain key.
        for first in [0, below(next_id as u64 + 1) as usize, next_id, next_id + 3] {
            let hit = |id: usize| id % 5 == 2;
            let mut asked = Vec::new();
            let found = index
                .find_from_each(key, None, first, |id| {
                    asked.push(id);
                    Ok(hit(id))
                })
                .unwrap();
            let expected = before
                .iter()
                .filter(|(signature, _)| signature.may_contain(key))
                .flat_map(|(_, blocks)| blocks.iter().flatten().copied())
                .filter(|&id| id >= first && hit(id))
                .min();
            assert_eq!(found, expected);
            assert!(asked.iter().all(|&id| id >= first));
            shortcuts += usize::from(first >= next_id && next_id > 0);
        }
        // Reverse with a word prefilter and a prepared set below `first_new`.
        let insertion = index.prepare(key, None).unwrap();
        let pin_tail = !insertion.has_new_block();
        let probe_word = below(16);
        let first_new = below(next_id as u64 + 1) as usize;
        let passes = |id: usize, words: &[u64]| words[id] & !probe_word == 0;
        let examined: Vec<usize> = before
            .iter()
            .filter(|(signature, _)| key.may_contain(*signature))
            .flat_map(|(_, blocks)| blocks.iter().flatten().copied())
            .collect();
        let prepared: Vec<usize> = examined
            .iter()
            .copied()
            .filter(|&id| id < first_new && passes(id, &words) && id % 3 == 0)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut record = Record {
            decided: 0,
            rejected: Vec::new(),
            tested: Vec::new(),
            predicate: |id: usize| id % 4 == 1,
        };
        let mut retired = Vec::new();
        index.retire_prepared(
            &insertion,
            &Probe::new(None, probe_word, None, true),
            &prepared,
            first_new,
            &mut record,
            |id| retired.push(id),
        );
        let (old, new): (Vec<usize>, Vec<usize>) = examined.iter().partition(|&&id| id < first_new);
        assert_eq!(record.decided, old.len());
        let mut handed: Vec<usize> = record
            .rejected
            .iter()
            .chain(&record.tested)
            .copied()
            .collect();
        handed.sort_unstable();
        let mut new_sorted = new.clone();
        new_sorted.sort_unstable();
        assert_eq!(handed, new_sorted);
        assert!(record.rejected.iter().all(|&id| !passes(id, &words)));
        assert!(record.tested.iter().all(|&id| passes(id, &words)));
        let mut expected: Vec<usize> = prepared
            .iter()
            .copied()
            .chain(
                new.iter()
                    .copied()
                    .filter(|&id| passes(id, &words) && id % 4 == 1),
            )
            .collect();
        expected.sort_unstable();
        retired.sort_unstable();
        assert_eq!(retired, expected);
        assert_eq!(
            index.layout(),
            reference_retire(&before, &retired, key, pin_tail)
        );
        splits += usize::from(!old.is_empty() && !new.is_empty());
        let word = below(16);
        words.push(word);
        index.insert(
            insertion,
            Entry {
                id: next_id,
                coordinates: None,
                word,
                lanes: None,
            },
        );
        assert_counts(&index);
    }
    println!("watermark_shortcuts group_skips={shortcuts} mixed_splits={splits}");
    assert!(shortcuts > 1_000 && splits > 1_000);
}
