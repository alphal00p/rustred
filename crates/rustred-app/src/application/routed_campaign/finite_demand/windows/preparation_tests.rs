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

#[test]
fn planning_resource_overrides_preserve_partition_bindings_and_resumable_cursor_bytes() {
    let cancel = AtomicBool::new(false);
    let region = RootRegionInput {
        support: [true],
        lower: vec![0],
        upper: vec![Some(5)],
        rank: Some(0),
        powers: DomainPowerBounds {
            max_positive_power: Some(6),
            min_power_difference: Some(1),
            max_power_difference: Some(6),
        },
    };
    let original = WindowLimits {
        max_seed_points: 1,
        max_seed_bytes: 4096,
        max_positive_layers: 1,
        max_partition_checks: 1,
    };
    assert!(
        PrefixWindows::<1>::try_new("bound-source", region.clone(), 1, original, &cancel).is_err()
    );
    let make = |layers, checks| {
        PrefixWindows::<1>::try_new_with_planning_limits(
            "bound-source",
            region.clone(),
            1,
            original,
            WindowPlanningLimits {
                max_positive_layers: layers,
                max_partition_checks: checks,
            },
            &cancel,
        )
        .unwrap()
    };
    let capped = make(64, 0);
    let cursor = capped.initial_cursor();
    assert_eq!(
        capped.next_window(&cursor, &cancel).err().unwrap().kind,
        ErrorKind::Resource
    );
    let ordinary = make(64, 1000);
    let uncapped = make(usize::MAX, usize::MAX);
    assert_eq!(
        ordinary.source_policy_binding(),
        uncapped.source_policy_binding()
    );
    assert_eq!(
        ordinary.source_policy_binding(),
        PrefixWindows::<1>::source_binding("bound-source", &region, 1, original).unwrap()
    );
    let mut position = cursor.clone();
    let mut count = 0;
    while let Some(expected) = ordinary.next_window(&position, &cancel).unwrap() {
        let actual = uncapped.next_window(&position, &cancel).unwrap().unwrap();
        assert_eq!(expected.targets(), actual.targets());
        assert_eq!(
            expected.transition().encode().unwrap(),
            actual.transition().encode().unwrap()
        );
        assert_eq!(
            expected.successor().encode().unwrap(),
            actual.successor().encode().unwrap()
        );
        position = expected.successor().clone();
        count += 1;
    }
    assert_eq!(count, 6);
    assert!(position.is_exhausted());
    assert_eq!(cursor, capped.initial_cursor());
}
