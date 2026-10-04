use super::*;
use crate::reduction::ReductionLimits;
use crate::solver::candidate_reduction::owner_test_support::*;
use crate::solver::{CandidateReductionError, CandidateRoutedError, RoutedCandidateLimits};
use std::sync::Arc;
use std::sync::atomic::Ordering;

pub(crate) fn budget() -> CandidateRoutedWorkBudget {
    CandidateRoutedWorkBudget {
        max_nodes: 10_000,
        max_rule_applications: 10_000,
        max_transport_calls: 10_000,
        max_transport_operations: 1_000_000,
        max_transport_endpoints: 1_000_000,
        max_coalescing_additions: 1_000_000,
    }
}

fn trace<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    targets: impl IntoIterator<Item = IntegralKey>,
    budget: CandidateRoutedWorkBudget,
) -> Result<CandidateRoutedCampaignReport<N>, CandidateRoutedCampaignError<N>> {
    reducer.trace_targets_inline_with_entry_admission_and_observer(
        targets,
        CandidateEntryAdmission::SavedGenerationScope,
        budget,
        &AtomicBool::new(false),
        |_| {},
    )
}

#[test]
fn inline_is_same_fifo_native_trace_with_fresh_per_request_counters() {
    let reducer = super::super::tests::diamond(Default::default(), Default::default());
    let targets = [key([6]), key([5]), key([4]), key([6])];
    let parallel = reducer
        .trace_targets_parallel_with_observer(targets.clone(), 1, &AtomicBool::new(false), |_| {})
        .unwrap();
    for _ in 0..2 {
        let result = trace(&reducer, targets.clone(), budget()).unwrap();
        assert_eq!(result.trace(), parallel.trace());
        let mut actual = result.snapshot().clone();
        let mut expected = parallel.snapshot().clone();
        actual.elapsed = Duration::ZERO;
        expected.elapsed = Duration::ZERO;
        assert_eq!(actual, expected);
    }
}

#[test]
fn inline_progress_stays_on_caller_and_mid_trace_cancel_is_incomplete() {
    // Every stored absolute toy index and shift fits the native Shift range.
    // A single legal root fans out to 400 strictly lower keys, so the callback
    // after 256 actual expansions still exercises mid-trace cancellation.
    let family = Arc::new(crate::solver::tests::sunset());
    let children: Vec<_> = (2..=21)
        .flat_map(|a| (2..=21).map(move |b| ([a, b, 1], 1)))
        .collect();
    let mut rules = vec![rule(&family, [22, 22, 1], &children)];
    rules.extend(
        children
            .iter()
            .map(|(child, _)| rule(&family, *child, &[([1, 1, 1], 1)])),
    );
    let owner = input([true; 3], Some(0), rules, &[[1, 1, 1]]);
    let reducer = RoutedCandidateReducer::try_new(
        programs(family, Some(0), vec![owner], Default::default()),
        [],
        Default::default(),
    )
    .unwrap();
    let caller = std::thread::current().id();
    let cancel = AtomicBool::new(false);
    let mut callbacks = 0;
    let error = reducer
        .trace_targets_inline_with_entry_admission_and_observer(
            [key([22, 22, 1])],
            CandidateEntryAdmission::SavedGenerationScope,
            budget(),
            &cancel,
            |snapshot| {
                assert_eq!(std::thread::current().id(), caller);
                callbacks += 1;
                if snapshot.completed_nodes >= 256 {
                    cancel.store(true, Ordering::Release);
                }
            },
        )
        .unwrap_err();
    assert!(callbacks >= 3);
    assert_eq!(error.reason(), &CandidateRoutedCampaignFailure::Cancelled);
    assert_eq!(error.snapshot().completed_nodes, 256);
    assert_eq!(error.snapshot().active_nodes, 0);
    assert!(!error.snapshot().finished);
    assert!(error.snapshot().rule_attempts > 0);
}

#[test]
fn inline_min_caps_charge_failed_attempts_and_conservative_reservations() {
    let reducer = super::super::tests::diamond(Default::default(), Default::default());
    for cap in [0, 1] {
        let error = trace(
            &reducer,
            [key([6])],
            CandidateRoutedWorkBudget {
                max_rule_applications: cap,
                ..budget()
            },
        )
        .unwrap_err();
        assert_eq!(error.snapshot().rule_attempts, cap);
        assert!(!error.snapshot().finished);
    }
    let original = super::super::tests::diamond(
        ReductionLimits {
            max_rule_applications: 1,
            ..Default::default()
        },
        Default::default(),
    );
    assert!(trace(&original, [key([6])], budget()).is_err());
    let original = super::super::tests::diamond(
        Default::default(),
        RoutedCandidateLimits {
            max_unique_nodes: 1,
            ..Default::default()
        },
    );
    assert!(trace(&original, [key([6])], budget()).is_err());
    assert!(
        trace(
            &reducer,
            [key([6])],
            CandidateRoutedWorkBudget {
                max_nodes: 1,
                ..budget()
            }
        )
        .is_err()
    );
    let family = Arc::new(crate::solver::tests::tadpole());
    let owner = input(
        [true],
        Some(0),
        vec![rule(&family, [4], &[([1], 1), ([1], -1)])],
        &[[1]],
    );
    let reducer = RoutedCandidateReducer::try_new(
        programs(family, Some(0), vec![owner], Default::default()),
        [],
        Default::default(),
    )
    .unwrap();
    let error = trace(
        &reducer,
        [key([4])],
        CandidateRoutedWorkBudget {
            max_coalescing_additions: 0,
            ..budget()
        },
    )
    .unwrap_err();
    assert_eq!(error.snapshot().rule_attempts, 1);
    assert_eq!(error.snapshot().rule_applications, 0);
    assert_eq!(error.snapshot().active_nodes, 0);
}

#[test]
fn inline_original_pole_survives_zero_coefficient_as_native_frontier() {
    let mut reducer = super::super::tests::diamond(Default::default(), Default::default());
    let programs = Arc::get_mut(&mut reducer.programs).unwrap();
    let c = programs.context.coefficient_context().clone();
    let pole = c
        .numerator_condition_with_limits(
            &c.sub(&c.index(0).unwrap(), &c.integer(4)).unwrap(),
            Default::default(),
        )
        .unwrap();
    let owner = Arc::get_mut(programs.owners.get_mut(&[true]).unwrap()).unwrap();
    let batch = Arc::get_mut(&mut owner.batches[0]).unwrap();
    batch.rules[2].rhs[0].coefficient = c.zero();
    batch.rules[2].rhs[0].denominator = pole;
    let result = trace(&reducer, [key([6])], budget()).unwrap();
    assert!(result.snapshot().finished);
    assert_eq!(result.snapshot().missing_rules, 1);
    assert!(!result.trace().frontier().is_empty());
    assert!(result.snapshot().rule_applications > 0);
}

#[test]
fn inline_late_descent_error_is_not_a_frontier_or_success() {
    let mut reducer = super::super::tests::diamond(Default::default(), Default::default());
    let programs = Arc::get_mut(&mut reducer.programs).unwrap();
    let owner = Arc::get_mut(programs.owners.get_mut(&[true]).unwrap()).unwrap();
    Arc::get_mut(&mut owner.batches[0]).unwrap().rules[2].rhs[0].shift = [1];
    let error = trace(&reducer, [key([6])], budget()).unwrap_err();
    assert!(matches!(
        error.reason(),
        CandidateRoutedCampaignFailure::Trace(CandidateRoutedError::Candidate(
            CandidateReductionError::NonDescending { .. }
        ))
    ));
    assert!(error.snapshot().rule_applications > 0);
    assert!(!error.snapshot().finished);
}

#[test]
fn inline_kernel_late_panic_retains_completed_prefix_and_no_active_worker() {
    let reducer = super::super::tests::diamond(Default::default(), Default::default());
    let cancel = AtomicBool::new(false);
    let shared = Shared::new(&reducer, 1, &cancel);
    shared.prepare(&reducer, [key([6])]).unwrap();
    let first = shared.take().unwrap();
    worker::run_one(&shared, first, |_| {
        shared.schedule(scheduler::Work::Apply {
            owner_sector: [true],
            target: key([6]),
        })
    });
    let second = shared.take().unwrap();
    worker::run_one(&shared, second, |_| panic!("late inline kernel fixture"));
    let error = shared.into_result().unwrap_err();
    assert_eq!(
        error.reason(),
        &CandidateRoutedCampaignFailure::WorkerPanicked
    );
    assert_eq!(error.snapshot().completed_nodes, 1);
    assert_eq!(error.snapshot().failed_nodes, 1);
    assert_eq!(error.snapshot().active_nodes, 0);
    assert!(!error.snapshot().finished);
}

#[test]
fn inline_observer_panic_has_no_detached_worker_or_retained_attempt_state() {
    let reducer = super::super::tests::diamond(Default::default(), Default::default());
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            reducer.trace_targets_inline_with_entry_admission_and_observer(
                [key([6])],
                CandidateEntryAdmission::SavedGenerationScope,
                budget(),
                &AtomicBool::new(false),
                |_| panic!("caller observer"),
            )
        }))
        .is_err()
    );
    assert!(
        trace(&reducer, [key([6])], budget())
            .unwrap()
            .snapshot()
            .finished
    );
}
