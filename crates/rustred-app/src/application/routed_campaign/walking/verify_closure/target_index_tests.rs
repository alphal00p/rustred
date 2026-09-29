//! Candidate ordering is never containment authority, including below the
//! historical 256-target threshold. Keep the same exact/brute predicate and
//! full-scan/alias fallback used by native reinspection.
use super::*;

fn box_cell(owner: [bool; 2], lower: u64, upper: u64) -> Cell {
    Cell {
        owner: owner.to_vec(),
        lower: vec![lower, 0],
        upper: vec![Some(upper), Some(2)],
        rank: Some(2),
        powers: DomainPowerBounds::default(),
    }
}

/// The reinspection candidate order, using an optionally corrupted hint to
/// exercise the full fallback without trusting any fingerprint as authority.
fn indexed_choice(
    targets: &[(usize, Phase, Cell)],
    index: Option<&TargetIndex>,
    phase: Phase,
    inner: &Cell,
    recent: usize,
    containment: &Containment,
) -> Option<usize> {
    let fits = |(_, candidate_phase, outer): &(usize, Phase, Cell)| {
        *candidate_phase == phase && containment.contains(outer, inner)
    };
    if recent < targets.len() && fits(&targets[recent]) {
        Some(recent)
    } else {
        index
            .and_then(|index| index.find(phase, inner, targets, &fits))
            .or_else(|| {
                (recent + 1..targets.len())
                    .chain(0..recent.min(targets.len()))
                    .find(|&index| fits(&targets[index]))
            })
    }
}

#[test]
fn target_index_medium_fanout_matches_full_scan() {
    assert_eq!(WIDE_NODE_TARGETS, 16);
    for count in [0, 1, 15, 16, 17, 63, 64, 255, 256] {
        let mut targets: Vec<_> = (0..count)
            .map(|i| {
                (
                    i,
                    if i % 3 == 0 {
                        Phase::Apply
                    } else {
                        Phase::Route
                    },
                    box_cell(
                        if i % 2 == 0 {
                            [true, false]
                        } else {
                            [false, true]
                        },
                        100 + i as u64,
                        100 + i as u64,
                    ),
                )
            })
            .collect();
        if let Some(last) = targets.last_mut() {
            *last = (count - 1, Phase::Route, box_cell([true, false], 0, 4));
        }
        let index = (count >= WIDE_NODE_TARGETS).then(|| TargetIndex::new(&targets));
        let checks = Containment::new(4096, 1 << 24);
        for (phase, inner) in [
            (Phase::Route, box_cell([true, false], 0, 4)), // exact image
            (Phase::Route, box_cell([true, false], 1, 3)), // genuine inclusion
            (Phase::Route, box_cell([true, false], 9999, 9999)), // uncovered
            (Phase::Apply, box_cell([true, false], 1, 3)), // wrong phase
            (Phase::Route, box_cell([false, true], 1, 3)), // wrong owner
            (Phase::Route, box_cell([true, true], 3, 2)),  // empty: any owner
        ] {
            let linear = targets
                .iter()
                .any(|(_, target_phase, outer)| *target_phase == phase && outer.contains(&inner));
            for recent in [0, count / 2, count.saturating_sub(1), count] {
                let found =
                    indexed_choice(&targets, index.as_ref(), phase, &inner, recent, &checks);
                assert_eq!(
                    found.is_some(),
                    linear,
                    "count={count}, recent={recent}, {inner:?}"
                );
                if let Some(found) = found {
                    assert_eq!(targets[found].1, phase);
                    assert!(targets[found].2.contains(&inner));
                }
            }
        }
        assert_eq!(checks.disagreements(), 0);
        if count != 0 {
            assert!(checks.brute.load(Ordering::Relaxed) > 0);
        }
    }
}

#[test]
fn target_index_false_hints_and_empty_owner_miss_keep_full_fallback() {
    let inner = box_cell([true, false], 1, 2);
    let mut targets: Vec<_> = (0..16)
        .map(|i| (i, Phase::Route, box_cell([false, true], 100, 101)))
        .collect();
    targets[15] = (15, Phase::Route, box_cell([true, false], 0, 4));
    let checks = Containment::new(4096, 1 << 20);
    let mut index = TargetIndex::new(&targets);
    // Force an exact-fingerprint collision: a non-containing target must
    // still fail the exact predicate, then the owner hint finds the cover.
    index
        .exact
        .insert(TargetIndex::fingerprint(Phase::Route, &inner, true), 0);
    assert_eq!(
        indexed_choice(&targets, Some(&index), Phase::Route, &inner, 0, &checks),
        Some(15)
    );
    // Missing/colliding owner hints may lose speed, never coverage.
    index.owners.insert(
        TargetIndex::fingerprint(Phase::Route, &inner, false),
        vec![0],
    );
    assert!(
        index
            .find(Phase::Route, &inner, &targets, |(_, phase, outer)| {
                *phase == Phase::Route && checks.contains(outer, &inner)
            })
            .is_none()
    );
    assert_eq!(
        indexed_choice(&targets, Some(&index), Phase::Route, &inner, 0, &checks),
        Some(15)
    );
    targets[0].1 = Phase::Apply; // recent cannot short-circuit the empty case
    let index = TargetIndex::new(&targets);
    let empty = box_cell([true, true], 3, 2);
    assert!(
        index
            .find(Phase::Route, &empty, &targets, |(_, phase, outer)| {
                *phase == Phase::Route && checks.contains(outer, &empty)
            })
            .is_none()
    );
    assert!(indexed_choice(&targets, Some(&index), Phase::Route, &empty, 0, &checks).is_some());
    assert_eq!(checks.disagreements(), 0);
}

#[test]
fn target_index_no_direct_cover_keeps_alias_chain_and_missing_edge_checks() {
    let cells: Vec<_> = (0..17)
        .map(|i| {
            box_cell(
                [true, false],
                if i == 0 || i == 16 { 0 } else { 100 },
                if i == 0 {
                    0
                } else if i == 16 {
                    4
                } else {
                    101
                },
            )
        })
        .collect();
    let domains: Vec<_> = cells
        .iter()
        .map(|cell| {
            CompactDomain::<2>::try_from_domain(&Domain {
                phase: Phase::Route,
                owner: [true, false],
                lower: cell.lower.clone(),
                upper: cell.upper.clone(),
                rank: cell.rank,
                powers: cell.powers,
            })
            .unwrap()
        })
        .collect();
    let targets: Vec<_> = cells[..16]
        .iter()
        .enumerate()
        .map(|(id, cell)| (id, Phase::Route, cell.clone()))
        .collect();
    let mut nodes = vec![Node::MISSING; 17];
    nodes[0] = Node {
        kind: Kind::Alias,
        link: 16,
        ..Node::MISSING
    };
    nodes[16].kind = Kind::Native;
    let inner = box_cell([true, false], 1, 2);
    let index = TargetIndex::new(&targets);
    let checks = Containment::new(4096, 1 << 20);
    assert!(indexed_choice(&targets, Some(&index), Phase::Route, &inner, 0, &checks).is_none());
    assert!(alias_chain_covers(
        &nodes,
        &domains,
        &checks,
        &targets,
        Phase::Route,
        &inner
    ));
    // Dropping the load-bearing target or corrupting its representative must
    // remain uncovered; changing candidate order cannot manufacture an edge.
    assert!(!alias_chain_covers(
        &nodes,
        &domains,
        &checks,
        &targets[1..],
        Phase::Route,
        &inner
    ));
    nodes[0].link = usize::MAX;
    assert!(!alias_chain_covers(
        &nodes,
        &domains,
        &checks,
        &targets,
        Phase::Route,
        &inner
    ));
    assert!(indexed_choice(&[], None, Phase::Route, &inner, 0, &checks).is_none());
    assert_eq!(checks.disagreements(), 0);
}
