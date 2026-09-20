use std::cell::Cell;
use std::collections::HashSet;
use std::convert::Infallible;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex};

use crate::algebra::CoefficientContext;
use crate::solver::{Integral, Term};

use super::*;

fn trivial_sources() -> SourceSystem<3> {
    let context = CoefficientContext::new(["n0", "n1", "n2"]);
    SourceSystem::new(
        vec![vec![Term {
            integral: Integral::symbolic([0; 3]).unwrap(),
            coefficient: context.one().numerator,
        }]],
        [0, 1, 2],
    )
    .unwrap()
}

fn signature<const N: usize>(completed: SectorCompleted<N>) -> Result<String, Infallible> {
    let rules: Vec<_> = completed
        .solution
        .rules
        .into_iter()
        .map(|rule| {
            let rhs: Vec<_> = rule
                .candidate
                .rhs
                .into_iter()
                .map(|term| (term.integral, term.coefficient.to_string()))
                .collect();
            let guards: Vec<Vec<_>> = rule
                .exceptions
                .branches
                .into_iter()
                .map(|branch| {
                    branch
                        .into_iter()
                        .map(|factor| factor.to_string())
                        .collect()
                })
                .collect();
            (rule.candidate.case, rule.candidate.target, rhs, guards)
        })
        .collect();
    Ok(format!(
        "{}:{:?}:{rules:?}:{:?}",
        completed.ordinal, completed.sector, completed.solution.finite_residuals
    ))
}

#[test]
fn executor_and_immutable_sources_are_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<SectorExecutor>();
    assert_send_sync::<SourceSystem<6>>();
    let config = SectorConfig::<6>::default();
    let clone = config.clone();
    assert!(Arc::ptr_eq(&config.zero_sectors, &clone.zero_sectors));
}

#[test]
fn per_job_ordering_matches_standalone_search_and_keeps_original_ordinals() {
    let sources = SourceSystem::<3>::from_family(&crate::solver::tests::sunset()).unwrap();
    let sectors = [[true; 3], [true, false, true], [true; 3]];
    let permutations = [Some([2, 1, 0]), None, Some([1, 0, 2])];
    let census: Arc<[[bool; 3]]> = Arc::from([[false; 3]]);
    let configure = |ordinal, sector| {
        assert_eq!(sector, sectors[ordinal]);
        SectorConfig {
            permutation: permutations[ordinal],
            zero_sectors: census.clone(),
            ..Default::default()
        }
    };
    let expected: Vec<_> = sectors
        .iter()
        .enumerate()
        .map(|(ordinal, &sector)| {
            let solver = SectorSolver::new(&sources, sector, configure(ordinal, sector)).unwrap();
            signature(SectorCompleted {
                ordinal,
                sector,
                preconditioning: Duration::ZERO,
                solution: solver.solve_sector(SectorSolveOptions::default()).unwrap(),
            })
            .unwrap()
        })
        .collect();
    for workers in [1, 2, 3] {
        if workers > 1 && LicenseManager::max_threads(workers) < workers {
            continue;
        }
        let calls = [
            AtomicUsize::new(0),
            AtomicUsize::new(0),
            AtomicUsize::new(0),
        ];
        let actual = SectorExecutor::new(workers)
            .unwrap()
            .map_configured_with_observer(
                &sources,
                &sectors,
                |ordinal, sector| {
                    calls[ordinal].fetch_add(1, Ordering::SeqCst);
                    configure(ordinal, sector)
                },
                SectorSolveOptions::default(),
                |_, _, _| {},
                signature,
            )
            .unwrap();
        assert_eq!(actual, expected);
        assert!(calls.iter().all(|count| count.load(Ordering::SeqCst) == 1));
    }
}

#[test]
fn per_job_invalid_ordering_is_a_manifest_ordered_preparation_error() {
    let sources = trivial_sources();
    let sectors = [[false; 3], [true; 3]];
    let error = SectorExecutor::new(1)
        .unwrap()
        .map_configured_with_observer(
            &sources,
            &sectors,
            |_, _| SectorConfig {
                permutation: Some([0, 0, 1]),
                ..Default::default()
            },
            SectorSolveOptions::default(),
            |_, _, _| {},
            signature,
        )
        .unwrap_err();
    assert!(matches!(
        error,
        SectorExecutionError::Prepare {
            ordinal: 0,
            source: SolverError::InvalidInput(_),
            ..
        }
    ));
}

#[test]
fn serial_executor_is_inline_reusable_and_retains_only_callback_values() {
    let sources = trivial_sources();
    let executor = SectorExecutor::new(1).unwrap();
    assert_eq!(executor.workers(), 1);
    assert!(executor.pool.is_none());
    let caller = std::thread::current().id();
    let sectors = [[true; 3], [false, true, true], [true; 3]];
    for _ in 0..2 {
        let result = executor
            .map(
                &sources,
                &sectors,
                &SectorConfig::default(),
                SectorSolveOptions::default(),
                |completed| {
                    assert_eq!(std::thread::current().id(), caller);
                    assert_eq!(completed.sector, sectors[completed.ordinal]);
                    assert_eq!(completed.solution.rules.len(), 1);
                    let ordinal = completed.ordinal;
                    drop(completed);
                    // Cell is Send but not Sync. Summaries need not be Sync
                    // and no exact solution is kept in the result collection.
                    Ok::<_, Infallible>(Cell::new(ordinal))
                },
            )
            .unwrap();
        assert_eq!(
            result.into_iter().map(Cell::into_inner).collect::<Vec<_>>(),
            [0, 1, 2]
        );
    }
    let empty = executor
        .map(
            &sources,
            &[],
            &SectorConfig::default(),
            SectorSolveOptions::default(),
            |_| -> Result<(), Infallible> { panic!("empty manifest cannot invoke callbacks") },
        )
        .unwrap();
    assert!(empty.is_empty());
}

#[test]
fn structural_scheduling_does_not_change_manifest_result_order() {
    let sources = trivial_sources();
    let sectors = [
        [false; 3],
        [true; 3],
        [true, false, true],
        [false, true, true],
    ];
    for (policy, expected_arrivals) in [
        (SectorScheduling::InputOrder, vec![0, 1, 2, 3]),
        (SectorScheduling::ActiveFirst, vec![1, 3, 2, 0]),
    ] {
        let executor = SectorExecutor::new(1).unwrap().with_scheduling(policy);
        assert_eq!(executor.scheduling(), policy);
        let arrivals = Mutex::new(Vec::new());
        let summaries = executor
            .map(
                &sources,
                &sectors,
                &SectorConfig::default(),
                SectorSolveOptions::default(),
                |completed| {
                    arrivals.lock().unwrap().push(completed.ordinal);
                    Ok::<_, Infallible>(completed.ordinal)
                },
            )
            .unwrap();
        assert_eq!(summaries, [0, 1, 2, 3]);
        assert_eq!(*arrivals.lock().unwrap(), expected_arrivals);
    }
}

#[test]
fn manifest_first_failure_wins_even_when_later_jobs_finish_first() {
    let sources = trivial_sources();
    let sectors = [[false; 3], [true; 3], [false, true, true]];
    for workers in [1, 2] {
        if workers > 1 && LicenseManager::max_threads(workers) < workers {
            continue;
        }
        let calls = AtomicUsize::new(0);
        let error = SectorExecutor::new(workers)
            .unwrap()
            .map(
                &sources,
                &sectors,
                &SectorConfig::default(),
                SectorSolveOptions::default(),
                |completed| -> Result<(), usize> {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Err(completed.ordinal)
                },
            )
            .unwrap_err();
        assert_eq!(error.ordinal(), 0);
        assert_eq!(*error.sector(), sectors[0]);
        assert!(matches!(
            error,
            SectorExecutionError::Consume {
                ordinal: 0,
                source: 0,
                ..
            }
        ));
        assert_eq!(calls.load(Ordering::SeqCst), sectors.len());
    }
}

#[test]
fn live_failures_precede_later_jobs_without_changing_manifest_error_selection() {
    // Non-Display, non-Clone errors are borrowed, never stringified or cloned
    // by the executor. The application chooses how to display them.
    struct OpaqueError;
    let sources = trivial_sources();
    let sectors = [[false; 3], [true; 3]];
    let events = Mutex::new(Vec::new());
    let error = SectorExecutor::new(1)
        .unwrap()
        .map_configured_with_error_observer(
            &sources,
            &sectors,
            |ordinal, _| {
                events.lock().unwrap().push(("start", ordinal));
                SectorConfig {
                    permutation: (ordinal == 1).then_some([0, 0, 1]),
                    ..Default::default()
                }
            },
            SectorSolveOptions::default(),
            |_, _, _| {},
            |error| {
                events.lock().unwrap().push(("failed", error.ordinal()));
                assert_eq!(*error.sector(), sectors[error.ordinal()]);
            },
            |_| -> Result<(), OpaqueError> { Err(OpaqueError) },
        )
        .unwrap_err();
    assert!(matches!(
        error,
        SectorExecutionError::Consume { ordinal: 0, .. }
    ));
    assert_eq!(
        *events.lock().unwrap(),
        [("start", 1), ("failed", 1), ("start", 0), ("failed", 0)]
    );
}

#[test]
fn live_search_failures_are_reported_once_per_job_across_worker_counts() {
    let sources = trivial_sources();
    let sectors = [[false; 3], [true; 3], [false, true, true]];
    for workers in [1, 2, 6] {
        if workers > 1 && LicenseManager::max_threads(workers) < workers {
            continue;
        }
        let failures = Mutex::new(Vec::new());
        let error = SectorExecutor::new(workers)
            .unwrap()
            .map_configured_with_error_observer(
                &sources,
                &sectors,
                |_, _| SectorConfig::default(),
                SectorSolveOptions {
                    max_symbolic_cases: Some(0),
                    ..Default::default()
                },
                |_, _, _| {},
                |error| {
                    assert!(matches!(error, SectorExecutionError::Solve { .. }));
                    failures.lock().unwrap().push(error.ordinal());
                },
                |_| -> Result<(), Infallible> { panic!("search must fail") },
            )
            .unwrap_err();
        assert_eq!(error.ordinal(), 0);
        let mut failures = failures.into_inner().unwrap();
        failures.sort_unstable();
        assert_eq!(failures, [0, 1, 2]);
    }
}

#[test]
#[should_panic(expected = "failure observer panic")]
fn failure_observer_panics_propagate_without_becoming_job_errors() {
    let _ = SectorExecutor::new(1)
        .unwrap()
        .map_configured_with_error_observer(
            &trivial_sources(),
            &[[true; 3]],
            |_, _| SectorConfig::default(),
            SectorSolveOptions::default(),
            |_, _, _| {},
            |_| panic!("failure observer panic"),
            |_| -> Result<(), ()> { Err(()) },
        );
}

#[test]
fn preparation_and_search_errors_keep_their_original_ordinal() {
    let sources = trivial_sources();
    let executor = SectorExecutor::new(1).unwrap();
    let sectors = [[false; 3], [false, true, true]];
    let callback = |_: SectorCompleted<3>| -> Result<(), Infallible> {
        panic!("failed preparation/search cannot invoke the consumer")
    };
    let prepare_error = executor
        .map(
            &sources,
            &sectors,
            &SectorConfig {
                deltas: [true, false, false],
                ..Default::default()
            },
            SectorSolveOptions::default(),
            callback,
        )
        .unwrap_err();
    assert!(matches!(
        prepare_error,
        SectorExecutionError::Prepare { ordinal: 0, .. }
    ));
    let search_error = executor
        .map(
            &sources,
            &sectors,
            &SectorConfig::default(),
            SectorSolveOptions {
                max_symbolic_cases: Some(0),
                ..Default::default()
            },
            callback,
        )
        .unwrap_err();
    assert!(matches!(
        search_error,
        SectorExecutionError::Solve { ordinal: 0, .. }
    ));
}

#[test]
fn native_thread_limits_are_errors_not_silent_multicore_fallback() {
    assert!(matches!(
        SectorExecutor::new(0),
        Err(SectorExecutorBuildError::ZeroWorkers)
    ));
    if LicenseManager::max_threads(2) < 2 {
        assert!(matches!(
            SectorExecutor::new(2),
            Err(SectorExecutorBuildError::NativeThreadLimit {
                requested: 2,
                allowed: 1
            })
        ));
    }
}

#[test]
fn exact_tadpole_rules_guards_and_residuals_match_across_worker_counts() {
    let family = crate::solver::tests::tadpole();
    let sources = SourceSystem::<1>::from_family(&family).unwrap();
    let sectors = [[true]; 6];
    let config = SectorConfig {
        zero_sectors: Arc::from([[false]]),
        ..Default::default()
    };
    let serial = SectorExecutor::new(1)
        .unwrap()
        .map(
            &sources,
            &sectors,
            &config,
            SectorSolveOptions::default(),
            signature,
        )
        .unwrap();
    for workers in [2, 4, 6] {
        if LicenseManager::max_threads(workers) < workers {
            continue;
        }
        let executor = SectorExecutor::new(workers).unwrap();
        for _ in 0..2 {
            let parallel = executor
                .map(
                    &sources,
                    &sectors,
                    &config,
                    SectorSolveOptions::default(),
                    signature,
                )
                .unwrap();
            assert_eq!(parallel, serial, "workers={workers}");
        }
    }
}

#[test]
fn observers_and_consumers_use_a_bounded_number_of_owned_workers() {
    if LicenseManager::max_threads(2) < 2 {
        return;
    }
    let sources = trivial_sources();
    let executor = SectorExecutor::new(2).unwrap();
    let sectors = [[true; 3]; 4];
    let barrier = Barrier::new(2);
    let active = AtomicUsize::new(0);
    let maximum = AtomicUsize::new(0);
    let threads = Mutex::new(HashSet::new());
    let observed = AtomicUsize::new(0);
    let summaries = executor
        .map_with_observer(
            &sources,
            &sectors,
            &SectorConfig::default(),
            SectorSolveOptions::default(),
            |ordinal, mask, event| {
                assert_eq!(mask, sectors[ordinal]);
                if let SectorEvent::CaseStarted { .. } = event {
                    observed.fetch_add(1, Ordering::SeqCst);
                }
                assert_eq!(rayon::current_num_threads(), 2);
            },
            |completed| {
                let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                maximum.fetch_max(now, Ordering::SeqCst);
                threads.lock().unwrap().insert(std::thread::current().id());
                barrier.wait();
                active.fetch_sub(1, Ordering::SeqCst);
                Ok::<_, Infallible>(completed.ordinal)
            },
        )
        .unwrap();
    assert_eq!(summaries, [0, 1, 2, 3]);
    assert_eq!(observed.load(Ordering::SeqCst), 4);
    assert_eq!(maximum.load(Ordering::SeqCst), 2);
    assert_eq!(threads.lock().unwrap().len(), 2);
}

#[test]
fn nested_rayon_work_reuses_the_owned_pool_instead_of_spawning_another_pool() {
    if LicenseManager::max_threads(2) < 2 {
        return;
    }
    let sources = trivial_sources();
    let executor = SectorExecutor::new(2).unwrap();
    let threads = Mutex::new(HashSet::new());
    executor
        .map(
            &sources,
            &[[true; 3]; 4],
            &SectorConfig::default(),
            SectorSolveOptions::default(),
            |completed| {
                let nested: Vec<_> = (0..8)
                    .into_par_iter()
                    .map(|_| {
                        assert_eq!(rayon::current_num_threads(), 2);
                        std::thread::current().id()
                    })
                    .collect();
                threads.lock().unwrap().extend(nested);
                Ok::<_, Infallible>(completed.ordinal)
            },
        )
        .unwrap();
    assert!(!threads.lock().unwrap().is_empty());
    assert!(threads.lock().unwrap().len() <= 2);
}

#[test]
fn diagnostic_mask_is_distinct_from_the_manifest_ordinal() {
    let error = SectorExecutionError::<3, Infallible>::Solve {
        ordinal: 397,
        sector: [true, false, true],
        source: SectorSolveError::CaseBudget {
            solved: 2,
            pending: 1,
        },
    };
    let text = error.to_string();
    assert!(text.starts_with("sector 397 search: symbolic-case budget exhausted"));
    assert!(text.ends_with("\nsector_mask=101"));
    assert_eq!(error.ordinal(), 397);
    assert_eq!(error.sector(), &[true, false, true]);
}
