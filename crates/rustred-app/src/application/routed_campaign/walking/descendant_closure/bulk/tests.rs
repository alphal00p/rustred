use super::super::{FLAG_INSPECTED, Tracker};
use std::sync::atomic::AtomicBool;

fn same(reference: &Tracker, batch: &Tracker) {
    assert_eq!(reference.unavailable, batch.unavailable);
    assert_eq!(reference.revision, batch.revision);
    assert_eq!(reference.snapshot_revision, batch.snapshot_revision);
    assert_eq!(reference.inspected, batch.inspected);
    assert_eq!(reference.flags, batch.flags);
    assert_eq!(reference.initial_closed, batch.initial_closed);
    assert_eq!(reference.total_closed, batch.total_closed);
    assert_eq!(reference.open_targets, batch.open_targets);
    assert_eq!(reference.edges.folded(), batch.edges.folded());
    assert_eq!(
        reference.edges.iter().collect::<Vec<_>>(),
        batch.edges.iter().collect::<Vec<_>>()
    );
}

fn apply(
    reference: &mut Tracker,
    batch: &mut Tracker,
    source: usize,
    targets: &[u32],
    inspected: bool,
    success: bool,
) {
    for &target in targets {
        reference.edge(source, target as usize);
    }
    reference.finish(source, inspected, success);
    batch.finish_with_sorted_targets(source, targets, inspected, success);
    same(reference, batch);
}

#[test]
fn source_batches_match_scalar_for_open_retries_duplicates_and_sealing() {
    let mut reference = Tracker::new(6);
    let mut batch = Tracker::new(6);
    batch.reserve_edge_batch(12);
    same(&reference, &batch);
    apply(&mut reference, &mut batch, 0, &[1, 1, 2, 4], true, false);
    apply(&mut reference, &mut batch, 0, &[1, 2, 2, 4], true, false);
    apply(&mut reference, &mut batch, 0, &[0, 2, 3, 3, 5], true, true);
    assert!(!batch.open_targets.contains_key(&0));
    apply(&mut reference, &mut batch, 1, &[], false, false);
    apply(&mut reference, &mut batch, 2, &[3, 4, 5], true, true);
    assert!(!batch.open_targets.contains_key(&2));
    apply(&mut reference, &mut batch, 3, &[], false, true);
}

#[test]
fn restored_open_source_retains_its_existing_deduplication() {
    let mut old = Tracker::new(4);
    old.edge(0, 1);
    old.edge(0, 3);
    old.finish(0, true, false);
    let flags: Vec<_> = old.node_flags().collect();
    let edges: Vec<_> = old.edges.iter().collect();
    let restore = || {
        let mut result = Tracker::from_parts(old.counters(), &flags, &edges).unwrap();
        result.restore(4, 4).unwrap();
        result
    };
    let mut reference = restore();
    let mut batch = restore();
    apply(&mut reference, &mut batch, 0, &[1, 2, 2, 3], true, true);
    assert_eq!(batch.edge_count(), 3);
}

#[test]
fn empty_invalid_and_sealed_sources_keep_scalar_diagnostic_precedence() {
    for initially_sealed in [false, true] {
        for source in [0, 3] {
            for targets in [&[][..], &[0][..], &[3][..], &[0, 3][..]] {
                let mut reference = Tracker::new(3);
                let mut batch = Tracker::new(3);
                if initially_sealed {
                    reference.finish(0, true, true);
                    batch.finish(0, true, true);
                }
                apply(&mut reference, &mut batch, source, targets, true, true);
                let first_reason = batch.unavailable.clone();
                batch.reserve_edge_batch(9);
                if first_reason.is_some() {
                    batch.finish_with_sorted_targets(1, &[0, 2], true, true);
                    assert_eq!(batch.unavailable, first_reason);
                    same(&reference, &batch);
                }
            }
        }
    }
}

#[test]
fn revision_overflow_distinguishes_edges_from_finish() {
    for revision in [u64::MAX, u64::MAX - 1, u64::MAX - 2, u64::MAX - 3] {
        for targets in [&[][..], &[0][..], &[0, 0, 1][..], &[0, 1, 2][..]] {
            let mut reference = Tracker::new(3);
            let mut batch = Tracker::new(3);
            reference.revision = revision;
            batch.revision = revision;
            apply(&mut reference, &mut batch, 0, targets, true, true);
        }
    }
    let mut batch = Tracker::new(1);
    batch.revision = u64::MAX;
    batch.finish_with_sorted_targets(0, &[], true, true);
    assert_eq!(
        batch.inspected, 1,
        "finish marks inspection before overflow"
    );
    assert!(
        batch.flags.is_empty(),
        "overflow disables the optional graph"
    );
}

#[test]
fn batch_graph_has_identical_cycle_and_frontier_closure() {
    let mut reference = Tracker::new(4);
    let mut batch = Tracker::new(4);
    apply(&mut reference, &mut batch, 0, &[1], true, true);
    apply(&mut reference, &mut batch, 1, &[0, 2], true, true);
    apply(&mut reference, &mut batch, 2, &[], true, false);
    apply(&mut reference, &mut batch, 3, &[3], true, true);
    reference.refresh(&AtomicBool::new(false), true);
    batch.refresh(&AtomicBool::new(false), true);
    same(&reference, &batch);
    assert_eq!(batch.total_closed, 1);
    assert_eq!(batch.flags[2] & FLAG_INSPECTED, FLAG_INSPECTED);
    apply(&mut reference, &mut batch, 2, &[], true, true);
    reference.refresh(&AtomicBool::new(false), true);
    batch.refresh(&AtomicBool::new(false), true);
    same(&reference, &batch);
    assert_eq!(batch.total_closed, 4);
}

#[test]
fn unsorted_batch_is_refused_instead_of_silently_losing_edges() {
    let mut batch = Tracker::new(3);
    batch.finish_with_sorted_targets(0, &[2, 1], true, true);
    assert_eq!(
        batch.unavailable.as_deref(),
        Some("dependency batch targets are not sorted")
    );
    assert_eq!(batch.inspected, 0);
    assert_eq!(batch.edge_count(), 0);
}

#[test]
fn failed_append_disables_the_partial_monitor_and_retains_prefix_revision() {
    for sealed in [false, true] {
        let mut batch = Tracker::new(4);
        let revision = batch.revision;
        batch.finish_with_sorted_targets_using(
            0,
            &[1, 2, 3],
            true,
            sealed,
            |edges, source, targets| {
                edges.push(source, targets[0])?;
                Err(())
            },
        );
        assert_eq!(batch.revision, revision + 1);
        assert_eq!(batch.inspected, 0);
        assert_eq!(batch.edge_count(), 0);
        assert!(batch.flags.is_empty());
        assert!(batch.open_targets.is_empty());
        assert_eq!(batch.closed(0), None);
        assert_eq!(
            batch.unavailable.as_deref(),
            Some("dependency edge allocation unavailable")
        );
        batch.finish_with_sorted_targets(0, &[u32::MAX], true, true);
        assert_eq!(
            batch.unavailable.as_deref(),
            Some("dependency edge allocation unavailable")
        );
        assert_eq!(batch.revision, revision + 1);
    }
}
