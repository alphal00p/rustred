use super::*;
use crate::reduction::ReductionLimits;
use crate::solver::candidate_reduction::owner_test_support::*;
use std::sync::Arc;
use std::sync::atomic::Ordering;

fn diamond(
    limits: ReductionLimits,
    routing: super::super::RoutedCandidateLimits,
) -> RoutedCandidateReducer<1> {
    let family = Arc::new(crate::solver::tests::tadpole());
    let owner = input(
        [true],
        Some(0),
        vec![
            rule(&family, [6], &[([4], 1), ([3], 1)]),
            rule(&family, [5], &[([4], 1), ([2], 1)]),
            rule(&family, [4], &[([1], 1)]),
            rule(&family, [3], &[([1], 1)]),
            rule(&family, [2], &[([1], 1)]),
        ],
        &[[1]],
    );
    RoutedCandidateReducer::try_new(programs(family, Some(0), vec![owner], limits), [], routing)
        .unwrap()
}

#[test]
fn shared_diamond_matches_legacy_and_parallel_union_without_repeated_work() {
    let reducer = diamond(Default::default(), Default::default());
    let targets = [key([6]), key([5]), key([4]), key([6])];
    let legacy = reducer.trace_targets(targets.clone()).unwrap();
    let cancel = AtomicBool::new(false);
    for workers in [1, 2, 6] {
        for _ in 0..3 {
            let mut observed = Vec::new();
            let result = reducer
                .trace_targets_parallel_with_observer(
                    targets.clone(),
                    workers,
                    &cancel,
                    |snapshot| observed.push(snapshot.clone()),
                )
                .unwrap();
            assert_eq!(result.trace(), &legacy);
            assert_eq!(result.trace().rule_applications(), 5);
            assert_eq!(result.snapshot().deduplication_hits, 5);
            assert_eq!(result.snapshot().scheduled_nodes, 9);
            assert_eq!(result.snapshot().completed_nodes, 9);
            assert_eq!(result.snapshot().active_nodes, 0);
            assert!(result.snapshot().finished);
            assert!(observed.last().unwrap().finished);
            assert!(observed.iter().all(|s| s.active_nodes <= workers
                && s.completed_nodes + s.failed_nodes + s.active_nodes + s.queued_nodes
                    == s.scheduled_nodes));
        }
    }
}

#[test]
fn aggregate_shared_work_succeeds_where_independent_totals_exceed_allowance() {
    let reducer = diamond(
        ReductionLimits {
            max_rule_applications: 5,
            ..Default::default()
        },
        Default::default(),
    );
    let result = reducer
        .trace_targets_parallel_with_observer(
            [key([6]), key([5]), key([4])],
            6,
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
    assert_eq!(result.snapshot().rule_attempts, 5);
    assert_eq!(result.snapshot().rule_applications, 5);
}

#[test]
fn missing_rules_and_owners_are_frontiers_not_errors_or_terminals() {
    let family = Arc::new(crate::solver::tests::sunset());
    let reducer = RoutedCandidateReducer::try_new(
        programs(
            family,
            Some(0),
            vec![input([true; 3], Some(0), vec![], &[])],
            Default::default(),
        ),
        [],
        Default::default(),
    )
    .unwrap();
    let result = reducer
        .trace_targets_parallel_with_observer(
            [key([1, 1, 1]), key([0, 1, 1]), key([0, 1, 1])],
            3,
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
    assert!(result.snapshot().finished);
    assert_eq!(result.snapshot().missing_rules, 1);
    assert_eq!(result.snapshot().missing_owners, 1);
    assert_eq!(result.trace().frontier().len(), 2);
    assert!(result.trace().declared_terminals().is_empty());
}

#[test]
fn children_above_input_rank_survive_and_late_invalid_input_prevents_work() {
    let family = Arc::new(crate::solver::tests::sunset());
    let owners = vec![
        input(
            [true; 3],
            Some(0),
            vec![rule(&family, [2, 1, 1], &[([0, 3, 1], 1)])],
            &[],
        ),
        input(
            [false, true, true],
            Some(0),
            vec![rule(&family, [0, 3, 1], &[([-1, 1, 1], 1)])],
            &[[-1, 1, 1]],
        ),
    ];
    let reducer = RoutedCandidateReducer::try_new(
        programs(family, Some(0), owners, Default::default()),
        [],
        Default::default(),
    )
    .unwrap();
    let cancel = AtomicBool::new(false);
    let result = reducer
        .trace_targets_parallel_with_observer([key([2, 1, 1])], 3, &cancel, |_| {})
        .unwrap();
    assert_eq!(result.trace().max_negative_index_degree(), 1);
    assert!(
        result
            .trace()
            .declared_terminals()
            .contains(&key([-1, 1, 1]))
    );
    let error = reducer
        .trace_targets_parallel_with_observer([key([2, 1, 1]), key([-1, 1, 1])], 3, &cancel, |_| {})
        .unwrap_err();
    assert_eq!(error.snapshot().rule_attempts, 0);
    assert_eq!(error.snapshot().scheduled_nodes, 0);
    assert!(!error.snapshot().finished);
}

#[test]
fn cancellation_is_typed_and_never_success() {
    let reducer = diamond(Default::default(), Default::default());
    let cancel = AtomicBool::new(true);
    let error = reducer
        .trace_targets_parallel_with_observer([key([6])], 2, &cancel, |_| {})
        .unwrap_err();
    assert_eq!(error.reason(), &CandidateRoutedCampaignFailure::Cancelled);
    assert!(!error.snapshot().finished);
    cancel.store(false, Ordering::Release);
    let error = reducer
        .trace_targets_parallel_with_observer([key([6])], 2, &cancel, |_| {
            cancel.store(true, Ordering::Release)
        })
        .unwrap_err();
    assert_eq!(error.reason(), &CandidateRoutedCampaignFailure::Cancelled);
    assert_eq!(error.snapshot().active_nodes, 0);
    assert_eq!(error.snapshot().rule_attempts, 0);
}

#[test]
fn aggregate_node_pending_rule_and_input_limits_fail_closed() {
    use super::super::RoutedCandidateLimits;
    for (reduction, routing) in [
        (
            ReductionLimits::default(),
            RoutedCandidateLimits {
                max_unique_nodes: 2,
                ..Default::default()
            },
        ),
        (
            ReductionLimits {
                max_pending_frames: 1,
                ..Default::default()
            },
            RoutedCandidateLimits::default(),
        ),
        (
            ReductionLimits {
                max_rule_applications: 1,
                ..Default::default()
            },
            RoutedCandidateLimits::default(),
        ),
        (
            ReductionLimits::default(),
            RoutedCandidateLimits {
                max_input_targets: 1,
                ..Default::default()
            },
        ),
    ] {
        let reducer = diamond(reduction, routing);
        let error = reducer
            .trace_targets_parallel_with_observer(
                [key([6]), key([5])],
                3,
                &AtomicBool::new(false),
                |_| {},
            )
            .unwrap_err();
        assert!(!error.snapshot().finished);
        assert_eq!(error.snapshot().active_nodes, 0);
        assert!(matches!(
            error.reason(),
            CandidateRoutedCampaignFailure::Trace(
                super::super::CandidateRoutedError::ResourceLimit { .. }
            )
        ));
    }
}

#[test]
fn coalescing_is_aggregate_and_local_cancellation_precedes_dedup() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let make = |cap| {
        let owner = input(
            [true],
            Some(0),
            vec![
                rule(&family, [4], &[([1], 1), ([1], -1)]),
                rule(&family, [3], &[([1], 1), ([1], 1)]),
            ],
            &[[1]],
        );
        RoutedCandidateReducer::try_new(
            programs(
                family.clone(),
                Some(0),
                vec![owner],
                ReductionLimits {
                    max_coalescing_additions: cap,
                    ..Default::default()
                },
            ),
            [],
            Default::default(),
        )
        .unwrap()
    };
    let cancel = AtomicBool::new(false);
    for workers in [1, 4] {
        let result = make(2)
            .trace_targets_parallel_with_observer([key([4]), key([3])], workers, &cancel, |_| {})
            .unwrap();
        assert_eq!(result.snapshot().coalescing_additions, 2);
        assert_eq!(result.snapshot().reserved_coalescing_additions, 0);
        assert_eq!(result.trace().rule_applications(), 2);
        assert_eq!(result.trace().declared_terminals().len(), 1);
        assert!(
            make(1)
                .trace_targets_parallel_with_observer(
                    [key([4]), key([3])],
                    workers,
                    &cancel,
                    |_| {}
                )
                .is_err()
        );
        let cancelled = make(1)
            .trace_targets_parallel_with_observer([key([4])], workers, &cancel, |_| {})
            .unwrap();
        assert!(cancelled.trace().declared_terminals().is_empty());
        assert_eq!(cancelled.trace().reachable_integrals(), 1);
    }
}

#[test]
fn workers_and_empty_campaign_are_explicit() {
    let reducer = diamond(Default::default(), Default::default());
    for workers in [0, 65] {
        assert!(
            reducer
                .trace_targets_parallel_with_observer([], workers, &AtomicBool::new(false), |_| {})
                .is_err()
        );
    }
    let result = reducer
        .trace_targets_parallel_with_observer([], 1, &AtomicBool::new(false), |_| {})
        .unwrap();
    assert!(result.snapshot().finished);
    assert_eq!(result.snapshot().scheduled_nodes, 0);
}

#[test]
fn illegal_seen_successor_is_checked_before_deduplication() {
    let family = Arc::new(crate::solver::tests::sunset());
    let owner = input(
        [true, false, true],
        None,
        vec![rule(&family, [2, 0, 1], &[([0, 1, 1], 1)])],
        &[],
    );
    let reducer = RoutedCandidateReducer::try_new(
        programs(family, None, vec![owner], Default::default()),
        [],
        Default::default(),
    )
    .unwrap();
    assert!(
        reducer
            .trace_targets_parallel_with_observer(
                [key([0, 1, 1]), key([2, 0, 1])],
                2,
                &AtomicBool::new(false),
                |_| {}
            )
            .is_err()
    );
}

#[test]
fn native_affine_route_and_pinch_match_legacy_with_shared_transport_budgets() {
    use super::super::{CandidateOwnerRoute, RoutedCandidateLimits};
    use crate::sector::Mask;
    use crate::sector::symmetry::{self, CoefficientMatrix, MomentumMap, integral_transport};
    use crate::solver::{CandidateOwnerContext, CandidateOwnerPrograms, CandidateOwnerScope};
    let family = Arc::new(crate::solver::tests::sunset());
    let c = family.coefficient_context();
    let map = symmetry::verify(
        &family,
        &family,
        MomentumMap::new(
            CoefficientMatrix::try_new(2, 2, [c.zero(), c.one(), c.one(), c.one()]).unwrap(),
            CoefficientMatrix::try_new(2, 0, []).unwrap(),
            CoefficientMatrix::try_new(0, 0, []).unwrap(),
        ),
        Default::default(),
    )
    .unwrap();
    let owner_mask = Mask::try_new([false, true, false]).unwrap();
    let transport = Arc::new(
        integral_transport::compile(
            &family,
            family.clone(),
            Arc::new(map),
            Mask::try_new([true, false, false]).unwrap(),
            owner_mask.clone(),
            Default::default(),
        )
        .unwrap(),
    );
    let crate::sector::zero::Decision::ProvedZero(zero) =
        crate::sector::zero::Analyzer::try_unrestricted(&family)
            .unwrap()
            .analyze(&Mask::try_new([false; 3]).unwrap())
            .unwrap()
    else {
        panic!("zero fixture")
    };
    let ctx = Arc::new(
        CandidateOwnerContext::try_new(
            family,
            CandidateOwnerScope {
                max_numerator_rank: Some(10),
                finite_case_policy: Default::default(),
            },
            vec![zero],
            Default::default(),
        )
        .unwrap(),
    );
    let programs = Arc::new(
        CandidateOwnerPrograms::try_new(
            ctx,
            [input(
                [false, true, false],
                Some(10),
                vec![],
                &[[0, 1, 0], [-1, 1, 0], [0, 1, -1]],
            )],
        )
        .unwrap(),
    );
    let route = CandidateOwnerRoute {
        owner_sector: owner_mask,
        transport,
    };
    let reducer =
        RoutedCandidateReducer::try_new(programs.clone(), [route.clone()], Default::default())
            .unwrap();
    let target = key([1, 0, -1]);
    let old = reducer
        .trace_targets([target.clone(), target.clone()])
        .unwrap();
    assert_eq!(old.visited_zeros().len(), 1);
    assert_eq!(old.declared_terminals().len(), 3);
    for workers in [1, 4] {
        let result = reducer
            .trace_targets_parallel_with_observer(
                [target.clone(), target.clone()],
                workers,
                &AtomicBool::new(false),
                |_| {},
            )
            .unwrap();
        assert_eq!(result.trace(), &old);
        assert_eq!(result.snapshot().transport_calls, 1);
        for caps in [
            RoutedCandidateLimits {
                max_transport_calls: 1,
                ..Default::default()
            },
            RoutedCandidateLimits {
                max_transport_operations: old.transport_operation_bound(),
                ..Default::default()
            },
            RoutedCandidateLimits {
                max_transport_endpoints: old.transport_endpoint_bound(),
                ..Default::default()
            },
        ] {
            let limited =
                RoutedCandidateReducer::try_new(programs.clone(), [route.clone()], caps).unwrap();
            let error = limited
                .trace_targets_parallel_with_observer(
                    [target.clone(), key([2, 0, -1])],
                    workers,
                    &AtomicBool::new(false),
                    |_| {},
                )
                .unwrap_err();
            assert!(!error.snapshot().finished);
            assert!(error.snapshot().transport_operations <= caps.max_transport_operations);
            assert!(error.snapshot().transport_endpoints <= caps.max_transport_endpoints);
            assert_eq!(error.snapshot().active_nodes, 0);
        }
    }
}

#[test]
fn observer_panic_stops_and_joins_workers() {
    let reducer = diamond(Default::default(), Default::default());
    let cancel = AtomicBool::new(false);
    let mut calls = 0;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        reducer.trace_targets_parallel_with_observer([key([6])], 3, &cancel, |_| {
            calls += 1;
            if calls == 2 {
                panic!("observer fixture");
            }
        })
    }));
    assert!(result.is_err());
    assert_eq!(calls, 2);
    assert!(
        reducer
            .trace_targets_parallel_with_observer([key([6])], 1, &cancel, |_| {})
            .is_ok()
    );
}

#[test]
fn snapshot_continues_during_active_work_and_worker_panic_is_incomplete() {
    let reducer = diamond(Default::default(), Default::default());
    let cancel = AtomicBool::new(false);
    let shared = scheduler::Shared::new(&reducer, 1, &cancel);
    shared.prepare(&reducer, [key([6])]).unwrap();
    let node = shared.take().unwrap();
    // Artificial held local work tests progress independently of native timing.
    let (snapshot, done) = shared.wait_snapshot(Duration::from_millis(1));
    assert!(!done);
    assert_eq!(snapshot.active_nodes, 1);
    assert_eq!(snapshot.completed_nodes, 0);
    worker::run_one(&shared, node, |_| panic!("worker fixture"));
    let (snapshot, done) = shared.wait_snapshot(Duration::from_millis(1));
    assert!(done);
    assert_eq!(snapshot.active_nodes, 0);
    assert_eq!(snapshot.failed_nodes, 1);
    let error = shared.into_result().unwrap_err();
    assert_eq!(
        error.reason(),
        &CandidateRoutedCampaignFailure::WorkerPanicked
    );
}

#[test]
fn shared_scheduler_cannot_return_success_with_unfinished_work() {
    let reducer = diamond(Default::default(), Default::default());
    let cancel = AtomicBool::new(false);
    let shared = scheduler::Shared::new(&reducer, 1, &cancel);
    shared.prepare(&reducer, [key([6])]).unwrap();
    assert!(shared.into_result().is_err());
}

#[test]
fn first_typed_failure_and_work_are_visible_before_native_drain_finishes() {
    let reducer = diamond(Default::default(), Default::default());
    let cancel = AtomicBool::new(false);
    let shared = scheduler::Shared::new(&reducer, 2, &cancel);
    shared.prepare(&reducer, [key([6]), key([5])]).unwrap();
    let failed = shared.take().unwrap();
    let draining = shared.take().unwrap();
    let original =
        CandidateRoutedCampaignFailure::Trace(super::super::CandidateRoutedError::ResourceLimit {
            resource: "typed failure fixture",
            requested: 7,
            limit: 6,
        });
    shared.finish(failed.clone(), Err(original.clone()));
    let (snapshot, done) = shared.wait_snapshot(Duration::from_millis(1));
    assert!(!done);
    assert!(!snapshot.finished);
    assert_eq!(snapshot.active_nodes, 1);
    assert_eq!(snapshot.failed_nodes, 1);
    assert_eq!(snapshot.first_failure.as_ref(), Some(&original));
    assert_eq!(snapshot.first_failure_work.as_ref(), Some(&failed));
    // Later local cancellation/panic reports must not obscure the first cause.
    shared.finish(
        draining,
        Err(CandidateRoutedCampaignFailure::WorkerPanicked),
    );
    let error = shared.into_result().unwrap_err();
    assert_eq!(error.reason(), &original);
    assert_eq!(error.snapshot().first_failure.as_ref(), Some(&original));
    assert_eq!(error.snapshot().first_failure_work.as_ref(), Some(&failed));
    assert_eq!(error.snapshot().failed_nodes, 2);
    assert_eq!(error.snapshot().active_nodes, 0);
}

#[test]
fn external_cancellation_has_no_fabricated_worker_origin() {
    let reducer = diamond(Default::default(), Default::default());
    let cancel = AtomicBool::new(false);
    let shared = scheduler::Shared::new(&reducer, 1, &cancel);
    shared.prepare(&reducer, [key([6])]).unwrap();
    let node = shared.take().unwrap();
    cancel.store(true, Ordering::Release);
    assert!(shared.check().is_err());
    let snapshot = shared.snapshot();
    assert_eq!(
        snapshot.first_failure,
        Some(CandidateRoutedCampaignFailure::Cancelled)
    );
    assert!(snapshot.first_failure_work.is_none());
    shared.finish(node, Err(CandidateRoutedCampaignFailure::WorkerPanicked));
    let error = shared.into_result().unwrap_err();
    assert_eq!(error.reason(), &CandidateRoutedCampaignFailure::Cancelled);
    assert!(error.snapshot().first_failure_work.is_none());
}
