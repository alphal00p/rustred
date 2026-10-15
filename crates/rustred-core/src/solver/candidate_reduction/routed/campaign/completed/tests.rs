use super::*;
use crate::solver::candidate_reduction::owner_test_support::*;
use crate::solver::{DomainPowerBounds, FiniteRootAdmission, RootRegionInput};
use std::sync::Arc;

fn budget() -> CandidateRoutedWorkBudget {
    CandidateRoutedWorkBudget {
        max_nodes: 10_000,
        max_rule_applications: 10_000,
        max_transport_calls: 10_000,
        max_transport_operations: 1_000_000,
        max_transport_endpoints: 1_000_000,
        max_coalescing_additions: 1_000_000,
    }
}
fn limits() -> CandidateRoutedCompletedTraceLimits {
    CandidateRoutedCompletedTraceLimits {
        max_completed_work: 10_000,
        max_merge_payload_bytes: 10_000_000,
    }
}
fn begin<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    targets: Vec<IntegralKey>,
    workers: usize,
) -> Result<
    (
        CandidateRoutedCompletedTrace<'_, N>,
        CandidateRoutedCampaignReport<N>,
    ),
    CandidateRoutedCompletedTraceError<N>,
> {
    reducer.trace_targets_completed_with_entry_admission_and_observer(
        targets,
        CandidateEntryAdmission::SavedGenerationScope,
        workers,
        budget(),
        Default::default(),
        limits(),
        &AtomicBool::new(false),
        |_| {},
    )
}

#[test]
fn completed_parallel_census_matches_inline_and_legacy_at_one_and_four_workers() {
    let reducer = super::super::tests::diamond(Default::default(), Default::default());
    let targets = vec![key([6]), key([5]), key([4]), key([6])];
    let inline = reducer
        .trace_targets_inline_with_entry_admission_and_census_observer(
            targets.clone(),
            CandidateEntryAdmission::SavedGenerationScope,
            budget(),
            Default::default(),
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
    for workers in [1, 4] {
        let plain = reducer
            .trace_targets_parallel_with_observer(
                targets.clone(),
                workers,
                &AtomicBool::new(false),
                |_| {},
            )
            .unwrap();
        let census = reducer
            .trace_targets_parallel_with_entry_admission_and_census_observer(
                targets.clone(),
                CandidateEntryAdmission::SavedGenerationScope,
                workers,
                budget(),
                Default::default(),
                &AtomicBool::new(false),
                |_| {},
            )
            .unwrap();
        let (closed, report) = begin(&reducer, targets.clone(), workers).unwrap();
        assert_eq!(plain.trace(), census.trace());
        assert_eq!(inline.trace(), report.trace());
        assert_eq!(inline.complete_census(), census.complete_census());
        assert_eq!(inline.complete_census(), Some(closed.census()));
        assert_eq!(closed.completed_work(), 9);
        assert_eq!(closed.input_occurrences(), 4);
        assert_eq!(closed.completed_batches(), 1);
    }
}

#[test]
fn completed_sequential_batches_equal_union_without_reexecuting_shared_descendants() {
    let reducer = super::super::tests::diamond(Default::default(), Default::default());
    let targets = vec![key([6]), key([5]), key([4]), key([6])];
    for workers in [1, 4] {
        let (whole, _) = begin(&reducer, targets.clone(), workers).unwrap();
        let (mut closed, first) = begin(&reducer, vec![key([6])], workers).unwrap();
        assert_eq!(first.snapshot().scheduled_nodes, 5);
        let second = closed
            .trace_batch(
                [key([5])],
                CandidateEntryAdmission::SavedGenerationScope,
                workers,
                &AtomicBool::new(false),
                |_| {},
            )
            .unwrap();
        assert_eq!(second.snapshot().scheduled_nodes, 3);
        assert_eq!(second.snapshot().completed_baseline_hits, 2);
        assert!(second.trace().declared_terminals().is_empty());
        let descendant_root = closed
            .trace_batch(
                [key([4])],
                CandidateEntryAdmission::SavedGenerationScope,
                workers,
                &AtomicBool::new(false),
                |_| {},
            )
            .unwrap();
        assert_eq!(descendant_root.snapshot().scheduled_nodes, 1); // Route phase was not present.
        assert_eq!(descendant_root.snapshot().completed_baseline_hits, 1);
        assert_eq!(descendant_root.snapshot().requested_targets, 1);
        let baseline_only = closed
            .trace_batch(
                [key([6]), key([6])],
                CandidateEntryAdmission::SavedGenerationScope,
                workers,
                &AtomicBool::new(false),
                |_| {},
            )
            .unwrap();
        assert_eq!(baseline_only.snapshot().scheduled_nodes, 0);
        assert_eq!(baseline_only.snapshot().rule_applications, 0);
        assert_eq!(baseline_only.snapshot().input_targets, 2);
        assert_eq!(baseline_only.snapshot().requested_targets, 1);
        assert_eq!(baseline_only.snapshot().completed_baseline_hits, 1);
        assert!(baseline_only.trace().declared_terminals().is_empty());
        assert_eq!(closed.completed_work(), whole.completed_work());
        assert_eq!(closed.census(), whole.census());
        assert_eq!(closed.declared_terminals(), whole.declared_terminals());
        assert_eq!(closed.visited_zeros(), whole.visited_zeros());
        assert_eq!(closed.input_occurrences(), 5); // Not globally distinct inputs.
        assert_eq!(closed.completed_batches(), 4);
    }
}

#[test]
fn completed_baseline_hit_still_requires_current_starting_admission() {
    let reducer = super::super::tests::diamond(Default::default(), Default::default());
    let (mut closed, _) = begin(&reducer, vec![key([6])], 1).unwrap();
    let admission = FiniteRootAdmission::try_new(
        [RootRegionInput {
            support: [true],
            lower: vec![4],
            upper: vec![Some(4)],
            rank: Some(0),
            powers: DomainPowerBounds::default(),
        }],
        1,
    )
    .unwrap();
    let error = closed
        .trace_batch(
            [key([6])],
            CandidateEntryAdmission::ExplicitFinite(&admission),
            4,
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap_err();
    assert!(matches!(
        error,
        CandidateRoutedCompletedTraceError::Trace(_)
    ));
    assert_eq!(error.report().snapshot().scheduled_nodes, 0);
    assert_eq!(error.report().snapshot().completed_baseline_hits, 0);
    assert_eq!(closed.completed_batches(), 1);
    assert_eq!(closed.input_occurrences(), 1);
    assert!(closed.contains_work(&CandidateRoutedWork::Route(key([6]))));
}

#[test]
fn completed_local_parent_done_with_missing_descendant_cannot_seal() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let reducer = RoutedCandidateReducer::try_new(
        programs(
            family.clone(),
            Some(0),
            vec![input(
                [true],
                Some(0),
                vec![rule(&family, [6], &[([4], 1)])],
                &[],
            )],
            Default::default(),
        ),
        [],
        Default::default(),
    )
    .unwrap();
    let error = begin(&reducer, vec![key([6])], 4).unwrap_err();
    assert!(matches!(
        error,
        CandidateRoutedCompletedTraceError::Frontier(_)
    ));
    assert!(error.report().snapshot().finished);
    assert_eq!(error.report().snapshot().completed_nodes, 3);
    assert_eq!(error.report().snapshot().missing_rules, 1);
    assert!(error.report().complete_census().is_none());
    assert!(error.report().trace().declared_terminals().is_empty());
}

#[test]
fn completed_failed_descendant_cannot_seal_even_after_selected_rule_prefix() {
    let mut reducer = super::super::tests::diamond(Default::default(), Default::default());
    let programs = Arc::get_mut(&mut reducer.programs).unwrap();
    let owner = Arc::get_mut(programs.owners.get_mut(&[true]).unwrap()).unwrap();
    Arc::get_mut(&mut owner.batches[0]).unwrap().rules[2].rhs[0].shift = [1];
    let error = begin(&reducer, vec![key([6])], 1).unwrap_err();
    assert!(matches!(
        error,
        CandidateRoutedCompletedTraceError::Trace(_)
    ));
    assert!(!error.report().snapshot().finished);
    assert!(error.report().snapshot().completed_nodes >= 2);
    assert!(error.report().complete_census().is_none());
}

#[test]
fn completed_frontier_failure_preserves_all_prior_authority() {
    let reducer = super::super::tests::diamond(Default::default(), Default::default());
    let (mut closed, _) = begin(&reducer, vec![key([6])], 1).unwrap();
    let census = closed.census().clone();
    let terminals = closed.declared_terminals().clone();
    let error = closed
        .trace_batch(
            [key([5]), key([7])],
            CandidateEntryAdmission::SavedGenerationScope,
            4,
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap_err();
    assert!(matches!(
        error,
        CandidateRoutedCompletedTraceError::Frontier(_)
    ));
    assert_eq!(closed.census(), &census);
    assert_eq!(closed.declared_terminals(), &terminals);
    assert_eq!(closed.completed_work(), 5);
    assert_eq!(closed.completed_batches(), 1);
    assert_eq!(closed.input_occurrences(), 1);
    assert!(!closed.contains_work(&CandidateRoutedWork::Route(key([5]))));
}

#[test]
fn completed_final_observer_cancellation_preserves_prior_membership_and_census() {
    let reducer = super::super::tests::diamond(Default::default(), Default::default());
    let (mut closed, _) = begin(&reducer, vec![key([6])], 1).unwrap();
    let census = closed.census().clone();
    let cancel = AtomicBool::new(false);
    let mut finished = 0;
    let error = closed
        .trace_batch(
            [key([5])],
            CandidateEntryAdmission::SavedGenerationScope,
            1,
            &cancel,
            |snapshot| {
                if snapshot.finished {
                    finished += 1;
                    if finished == 2 {
                        cancel.store(true, Ordering::Release);
                    }
                }
            },
        )
        .unwrap_err();
    assert_eq!(finished, 2);
    assert!(
        matches!(error, CandidateRoutedCompletedTraceError::Trace(ref e)
        if *e.reason() == CandidateRoutedCampaignFailure::Cancelled)
    );
    assert!(error.report().complete_census().is_none());
    assert_eq!(closed.census(), &census);
    assert_eq!(closed.completed_work(), 5);
    assert_eq!(closed.completed_batches(), 1);
    assert!(!closed.contains_work(&CandidateRoutedWork::Route(key([5]))));
}

#[test]
fn completed_cumulative_work_and_selection_quotas_do_not_commit_partial_delta() {
    let reducer = super::super::tests::diamond(Default::default(), Default::default());
    for (retention, census_limits) in [
        (
            CandidateRoutedCompletedTraceLimits {
                max_completed_work: 5,
                ..limits()
            },
            CandidateRoutedTraceCensusLimits::default(),
        ),
        (
            limits(),
            CandidateRoutedTraceCensusLimits {
                max_unique_selections: 3,
                ..Default::default()
            },
        ),
    ] {
        let (mut closed, _) = reducer
            .trace_targets_completed_with_entry_admission_and_observer(
                [key([6])],
                CandidateEntryAdmission::SavedGenerationScope,
                1,
                budget(),
                census_limits,
                retention,
                &AtomicBool::new(false),
                |_| {},
            )
            .unwrap();
        let before = closed.census().clone();
        let error = closed
            .trace_batch(
                [key([5])],
                CandidateEntryAdmission::SavedGenerationScope,
                4,
                &AtomicBool::new(false),
                |_| {},
            )
            .unwrap_err();
        assert!(matches!(
            error,
            CandidateRoutedCompletedTraceError::Retention { .. }
        ));
        assert!(error.report().complete_census().is_none());
        assert_eq!(closed.census(), &before);
        assert_eq!(closed.completed_work(), 5);
        assert_eq!(closed.completed_batches(), 1);
        assert!(!closed.contains_work(&CandidateRoutedWork::Route(key([5]))));
    }
}

#[test]
fn completed_merge_payload_refusal_and_input_counter_overflow_are_fail_closed() {
    let reducer = super::super::tests::diamond(Default::default(), Default::default());
    assert!(matches!(
        reducer.trace_targets_completed_with_entry_admission_and_observer(
            [key([6])],
            CandidateEntryAdmission::SavedGenerationScope,
            1,
            budget(),
            Default::default(),
            CandidateRoutedCompletedTraceLimits {
                max_merge_payload_bytes: 0,
                ..limits()
            },
            &AtomicBool::new(false),
            |_| {}
        ),
        Err(CandidateRoutedCompletedTraceError::Retention { .. })
    ));
    let (mut closed, _) = begin(&reducer, vec![key([6])], 1).unwrap();
    closed.input_occurrences = usize::MAX;
    let before = closed.census().clone();
    assert!(matches!(
        closed.trace_batch(
            [key([6])],
            CandidateEntryAdmission::SavedGenerationScope,
            1,
            &AtomicBool::new(false),
            |_| {}
        ),
        Err(CandidateRoutedCompletedTraceError::Retention { .. })
    ));
    assert_eq!(closed.census(), &before);
    assert_eq!(closed.completed_work(), 5);
    assert_eq!(closed.completed_batches(), 1);
    assert_eq!(closed.input_occurrences(), usize::MAX);
}

#[test]
fn completed_empty_batch_is_exhausted_but_does_not_claim_distinct_roots() {
    let reducer = super::super::tests::diamond(Default::default(), Default::default());
    let (mut closed, empty) = begin(&reducer, vec![], 4).unwrap();
    assert!(empty.snapshot().finished);
    assert_eq!(closed.completed_work(), 0);
    assert_eq!(closed.input_occurrences(), 0);
    assert!(closed.declared_terminals().is_empty());
    let first = closed
        .trace_batch(
            [key([1])],
            CandidateEntryAdmission::SavedGenerationScope,
            1,
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
    assert_eq!(first.snapshot().completed_baseline_hits, 0);
    assert_eq!(closed.completed_work(), 2);
    assert_eq!(closed.input_occurrences(), 1);
    assert_eq!(closed.completed_batches(), 2);
    assert_eq!(closed.declared_terminals(), &BTreeSet::from([key([1])]));
}

#[test]
fn completed_final_observer_panic_does_not_commit_or_poison_previous_closure() {
    let reducer = super::super::tests::diamond(Default::default(), Default::default());
    let (mut closed, _) = begin(&reducer, vec![key([6])], 1).unwrap();
    let census = closed.census().clone();
    let mut finished = 0;
    let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        closed.trace_batch(
            [key([5])],
            CandidateEntryAdmission::SavedGenerationScope,
            1,
            &AtomicBool::new(false),
            |snapshot| {
                if snapshot.finished {
                    finished += 1;
                    if finished == 2 {
                        panic!("pre-commit observer fixture");
                    }
                }
            },
        )
    }));
    assert!(error.is_err());
    assert_eq!(finished, 2);
    assert_eq!(closed.census(), &census);
    assert_eq!(closed.completed_work(), 5);
    assert_eq!(closed.completed_batches(), 1);
    assert!(!closed.contains_work(&CandidateRoutedWork::Route(key([5]))));
    closed
        .trace_batch(
            [key([5])],
            CandidateEntryAdmission::SavedGenerationScope,
            4,
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
    assert_eq!(closed.completed_work(), 8);
}
