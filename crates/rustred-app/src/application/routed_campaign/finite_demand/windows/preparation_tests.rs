use super::*;
use rustred::solver::{CandidateRoutedWorkerPool, DomainPowerBounds};

fn fixture(origin: &str) -> PrefixWindows<3> {
    PrefixWindows::try_new(
        origin,
        RootRegionInput {
            support: [true, true, false],
            lower: vec![0; 3],
            upper: vec![None; 3],
            rank: Some(3),
            powers: DomainPowerBounds {
                max_positive_power: Some(8),
                min_power_difference: Some(1),
                max_power_difference: Some(6),
            },
        },
        3,
        WindowLimits {
            max_seed_points: 4,
            max_seed_bytes: 4096,
            max_positive_layers: 16,
            max_partition_checks: 1000,
        },
        &AtomicBool::new(false),
    )
    .unwrap()
}

#[test]
fn deferred_parallel_preparation_preserves_every_target_and_cursor_byte() {
    let source = fixture("same-source");
    let cancel = AtomicBool::new(false);
    let mut cursor = source.initial_cursor();
    let mut sequential = Vec::new();
    while let Some(window) = source.next_window(&cursor, &cancel).unwrap() {
        cursor = window.successor().clone();
        sequential.push(window);
    }
    for workers in [1, 4] {
        if !crate::test_gates::workers_or_skip("parallel window preparation", workers) {
            continue;
        }
        let pool = CandidateRoutedWorkerPool::try_new(workers).unwrap();
        let mut proposed = source.initial_cursor();
        let mut plans = Vec::new();
        while let Some(plan) = source.plan_window(&proposed, &cancel).unwrap() {
            proposed = plan.successor().clone();
            plans.push(plan);
        }
        assert_eq!(proposed.encode().unwrap(), cursor.encode().unwrap());
        let results = pool
            .map_ordered(plans, |plan| source.materialize_window(plan, &cancel))
            .unwrap();
        assert_eq!(results.len(), sequential.len());
        for (actual, expected) in results.into_iter().zip(&sequential) {
            let actual = actual.unwrap();
            assert_eq!(actual.targets(), expected.targets());
            assert_eq!(
                actual.transition().encode().unwrap(),
                expected.transition().encode().unwrap()
            );
            assert_eq!(
                actual.successor().encode().unwrap(),
                expected.successor().encode().unwrap()
            );
        }
    }
}

#[test]
fn materialization_rejects_foreign_source_and_cancellation_without_adoption() {
    let source = fixture("original");
    let other = fixture("other");
    let cancel = AtomicBool::new(false);
    let initial = source.initial_cursor();
    let saved = initial.encode().unwrap();
    let plan = source.plan_window(&initial, &cancel).unwrap().unwrap();
    assert_eq!(
        other.materialize_window(plan, &cancel).err().unwrap().kind,
        ErrorKind::Cursor
    );
    let plan = source.plan_window(&initial, &cancel).unwrap().unwrap();
    cancel.store(true, Ordering::Release);
    assert_eq!(
        source.materialize_window(plan, &cancel).err().unwrap().kind,
        ErrorKind::Cancelled
    );
    assert_eq!(initial.encode().unwrap(), saved);
    cancel.store(false, Ordering::Release);
    assert!(source.next_window(&initial, &cancel).unwrap().is_some());
}
