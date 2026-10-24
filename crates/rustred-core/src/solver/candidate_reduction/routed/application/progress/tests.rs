use super::super::{CandidateRoutedCachePolicy, RoutedCandidateApplication};
use super::*;
use crate::reduction::ReductionLimits;
use crate::solver::candidate_reduction::owner_test_support::*;
use crate::solver::{
    CandidateCacheRepresentation, CandidateEntryAdmission, RoutedCandidateReducer,
};
use std::sync::Arc;

fn application(capacity: usize) -> RoutedCandidateApplication<1> {
    let family = Arc::new(crate::solver::tests::tadpole());
    let owners = programs(
        family.clone(),
        None,
        vec![input(
            [true],
            None,
            vec![
                rule(&family, [4], &[([2], 1)]),
                rule(&family, [3], &[([2], -3), ([1], 2)]),
                rule(&family, [2], &[([1], 2)]),
            ],
            &[[1], [5]],
        )],
        ReductionLimits {
            diagnostics_enabled: true,
            max_cached_integrals: capacity,
            ..Default::default()
        },
    );
    let mut application = RoutedCandidateReducer::try_new(owners, [], Default::default())
        .unwrap()
        .application();
    application
        .set_cache_policy(CandidateRoutedCachePolicy::EvictLeastRecentlyUsed)
        .unwrap();
    application
}

#[test]
fn observations_preserve_exact_work_and_reset_for_warm_requests() {
    for policy in [
        CandidateRoutedCachePolicy::EvictLeastRecentlyUsed,
        CandidateRoutedCachePolicy::EvictAgedLeastFrequentlyUsed,
    ] {
        for representation in [
            CandidateCacheRepresentation::Sparse,
            CandidateCacheRepresentation::Factorized,
        ] {
            let mut observed = application(2);
            let mut plain = application(2);
            observed.set_cache_policy(policy).unwrap();
            plain.set_cache_policy(policy).unwrap();
            observed.set_cache_representation(representation).unwrap();
            plain.set_cache_representation(representation).unwrap();
            let one = observed.family().coefficient_context().one();
            let inputs = [(key([3]), one.clone()), (key([4]), one)];
            let expected = plain.reduce_linear_combination(inputs.clone()).unwrap();
            let mut reference = application(20);
            reference.set_cache_representation(representation).unwrap();
            let reference = reference.reduce_linear_combination(inputs.clone()).unwrap();
            let mut events = Vec::new();
            let actual = observed
                .reduce_linear_combination_with_entry_admission_and_observer(
                    inputs.clone(),
                    CandidateEntryAdmission::SavedGenerationScope,
                    Duration::ZERO,
                    |snapshot| events.push(*snapshot),
                )
                .unwrap();
            assert_eq!(actual, expected);
            assert_eq!(actual.terms(), reference.terms());
            assert_eq!(actual.nonzero_conditions(), reference.nonzero_conditions());
            assert_eq!(
                events.first().unwrap().status,
                CandidateRoutedApplicationStatus::Running
            );
            let last = events.last().unwrap();
            assert_eq!(last.status, CandidateRoutedApplicationStatus::Succeeded);
            assert_eq!(last.statistics, *actual.statistics());
            assert!(last.statistics.cache.evicted_entries > 0);
            assert!(last.statistics.cache.recomputed_nodes > 0);
            assert_eq!(last.pending_frames, 0);
            assert_eq!(last.active_nodes, 0);
            assert_eq!(last.pinned.unwrap(), CandidateRoutedCacheCensus::default());
            assert!(events.iter().any(|event| event.pinned.unwrap().entries > 0));
            for (i, event) in events.iter().enumerate() {
                assert_eq!(event.sequence, i as u64 + 1);
                assert_eq!(event.statistics.operational_nodes, event.unique_nodes);
                assert_eq!(
                    event.cached.excluding(event.pinned.unwrap()),
                    event.eligible.unwrap()
                );
            }
            let mut warm = Vec::new();
            observed
                .reduce_linear_combination_with_entry_admission_and_observer(
                    [inputs[1].clone()],
                    CandidateEntryAdmission::SavedGenerationScope,
                    Duration::from_secs(3600),
                    |snapshot| warm.push(*snapshot),
                )
                .unwrap();
            assert_eq!(warm.len(), 2);
            assert_eq!(warm[0].sequence, 1);
            assert_eq!(warm[1].statistics.work.rule_applications(), 0);
            assert_eq!(warm[1].statistics.cache, Default::default());
        }
    }
}

#[test]
fn failure_snapshot_precedes_pin_cleanup_and_cache_remains_reusable() {
    for policy in [
        CandidateRoutedCachePolicy::EvictLeastRecentlyUsed,
        CandidateRoutedCachePolicy::EvictAgedLeastFrequentlyUsed,
    ] {
        let mut app = application(1);
        app.set_cache_policy(policy).unwrap();
        let one = app.family().coefficient_context().one();
        let mut events = Vec::new();
        assert!(
            app.reduce_linear_combination_with_entry_admission_and_observer(
                [(key([3]), one.clone())],
                CandidateEntryAdmission::SavedGenerationScope,
                Duration::from_secs(3600),
                |event| events.push(*event),
            )
            .is_err()
        );
        let last = events.last().unwrap();
        assert_eq!(last.status, CandidateRoutedApplicationStatus::Failed);
        assert_eq!(last.pinned.unwrap().entries, 1);
        assert_eq!(last.eligible.unwrap().entries, 0);
        assert!(last.active_nodes > 0);
        assert!(last.pinned.unwrap().coefficient_bytes > 0);
        app.reduce_linear_combination([(key([5]), one)]).unwrap();
        assert!(app.cache_work_statistics().evicted_entries > 0);
    }
}

#[test]
fn observer_panic_releases_pins_on_running_and_failed_snapshots() {
    for policy in [
        CandidateRoutedCachePolicy::EvictLeastRecentlyUsed,
        CandidateRoutedCachePolicy::EvictAgedLeastFrequentlyUsed,
    ] {
        for panic_on_failure in [false, true] {
            let mut app = application(1);
            app.set_cache_policy(policy).unwrap();
            let one = app.family().coefficient_context().one();
            let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                app.reduce_linear_combination_with_entry_admission_and_observer(
                    [(key([3]), one.clone())],
                    CandidateEntryAdmission::SavedGenerationScope,
                    Duration::ZERO,
                    |event| {
                        if (panic_on_failure
                            && event.status == CandidateRoutedApplicationStatus::Failed)
                            || (!panic_on_failure && event.pinned.unwrap().entries > 0)
                        {
                            panic!("test observer failure");
                        }
                    },
                )
            }));
            assert!(panicked.is_err());
            app.reduce_linear_combination([(key([5]), one)]).unwrap();
            assert!(app.cache_work_statistics().evicted_entries > 0);
        }
    }
}

#[test]
fn fail_on_limit_does_not_invent_pin_census_and_admission_errors_finish() {
    let mut app = application(20);
    app.set_cache_policy(CandidateRoutedCachePolicy::FailOnLimit)
        .unwrap();
    let one = app.family().coefficient_context().one();
    let mut events = Vec::new();
    app.reduce_linear_combination_with_entry_admission_and_observer(
        [(key([3]), one.clone())],
        CandidateEntryAdmission::SavedGenerationScope,
        Duration::ZERO,
        |event| events.push(*event),
    )
    .unwrap();
    assert!(
        events
            .iter()
            .all(|event| event.pinned.is_none() && event.eligible.is_none())
    );
    events.clear();
    assert!(
        app.reduce_linear_combination_with_entry_admission_and_observer(
            [(key([1, 2]), one)],
            CandidateEntryAdmission::SavedGenerationScope,
            Duration::ZERO,
            |event| events.push(*event),
        )
        .is_err()
    );
    assert_eq!(events.len(), 2);
    assert_eq!(events[1].status, CandidateRoutedApplicationStatus::Failed);
    assert_eq!(events[1].unique_nodes, 0);
    assert_eq!(events[1].statistics.work.rule_applications(), 0);
}

#[test]
fn cadence_skips_clocks_until_the_work_gate_and_force_emits() {
    let mut observe = |_: &CandidateRoutedApplicationSnapshot| {};
    let mut progress = Progress::new(Duration::from_secs(3600), &mut observe);
    assert!(progress.due(true).is_some());
    for _ in 0..255 {
        assert!(progress.due(false).is_none());
    }
    assert_eq!(progress.remaining, 1);
    assert!(progress.due(false).is_none());
    assert_eq!(progress.remaining, 256);
    assert_eq!(progress.sequence, 1);
    assert!(progress.due(true).is_some());
    assert_eq!(progress.sequence, 2);
}
