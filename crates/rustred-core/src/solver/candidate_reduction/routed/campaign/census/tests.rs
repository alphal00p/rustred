//! Dispatch/storage tests using synthetic rules, not new IBP authority.
use super::super::*;
use super::*;
use crate::solver::candidate_reduction::owner_test_support::*;
use crate::solver::{CandidateOwnerPrograms, RuleDispatchPolicy};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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
fn run<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    keys: Vec<IntegralKey>,
    limits: CandidateRoutedTraceCensusLimits,
) -> Result<CandidateRoutedCampaignReport<N>, CandidateRoutedCampaignError<N>> {
    reducer.trace_targets_inline_with_entry_admission_and_census_observer(
        keys,
        CandidateEntryAdmission::SavedGenerationScope,
        budget(),
        limits,
        &AtomicBool::new(false),
        |_| {},
    )
}

#[test]
fn census_enabled_matches_disabled_trace_and_records_actual_occurrences() {
    let reducer = super::super::tests::diamond(Default::default(), Default::default());
    let targets = vec![key([6]), key([5]), key([6])];
    let plain = reducer
        .trace_targets_inline_with_entry_admission_and_observer(
            targets.clone(),
            CandidateEntryAdmission::SavedGenerationScope,
            budget(),
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
    let recorded = run(&reducer, targets, Default::default()).unwrap();
    assert!(plain.complete_census().is_none());
    assert_eq!(plain.trace(), recorded.trace());
    let mut a = plain.snapshot().clone();
    let mut b = recorded.snapshot().clone();
    a.elapsed = Duration::ZERO;
    b.elapsed = Duration::ZERO;
    assert_eq!(a, b);
    let census = recorded.complete_census().unwrap();
    assert_eq!(census.selections().len(), 5);
    assert_eq!(census.selections().values().sum::<u64>(), 5);
    for (identity, count) in census.selections() {
        assert_eq!(identity.owner, vec![true]);
        assert_eq!(identity.batch_ordinal, 0);
        assert_eq!(
            identity.baseline_rule_ordinal,
            identity.selected_rule_ordinal
        );
        assert_eq!(*count, 1);
    }
    assert_eq!(
        recorded.trace().declared_terminals(),
        &std::collections::BTreeSet::from([key([1])])
    );
}

#[test]
fn census_uses_sparse_saved_preference_ordinal() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let preferred = (0..111).map(|_| rule(&family, [3], &[([1], 1)])).collect();
    let ordinals = [110usize];
    let programs = CandidateOwnerPrograms::try_new_with_preference_rule_subsets(
        context(family, None, Default::default()),
        [input([true], None, vec![], &[[1]])],
        [input([true], None, preferred, &[])],
        &BTreeMap::from([([true], ordinals.as_slice())]),
    )
    .unwrap();
    let reducer =
        RoutedCandidateReducer::try_new(Arc::new(programs), [], Default::default()).unwrap();
    let report = run(&reducer, vec![key([3])], Default::default()).unwrap();
    assert_eq!(
        report.complete_census().unwrap().selections(),
        &BTreeMap::from([(
            CandidateRoutedSelectionIdentity {
                owner: vec![true],
                batch_ordinal: 0,
                baseline_rule_ordinal: 110,
                selected_rule_ordinal: 110
            },
            1
        )])
    );
    assert_eq!(
        report.trace().declared_terminals(),
        &std::collections::BTreeSet::from([key([1])])
    );
}

#[test]
fn later_batch_terminal_does_not_hide_an_earlier_applied_rule_from_census() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let terminal_program = programs(
        family,
        Some(0),
        vec![input([true], Some(0), vec![], &[[4]])],
        Default::default(),
    );
    let later = terminal_program.owners[&[true]].batches[0].clone();
    let mut reducer = super::super::tests::diamond(Default::default(), Default::default());
    let programs = Arc::get_mut(&mut reducer.programs).unwrap();
    Arc::get_mut(programs.owners.get_mut(&[true]).unwrap())
        .unwrap()
        .batches
        .push(later);
    let report = run(&reducer, vec![key([4])], Default::default()).unwrap();
    assert_eq!(
        report.complete_census().unwrap().selections(),
        &BTreeMap::from([(
            CandidateRoutedSelectionIdentity {
                owner: vec![true],
                batch_ordinal: 0,
                baseline_rule_ordinal: 2,
                selected_rule_ordinal: 2
            },
            1
        )])
    );
    assert_eq!(
        report.trace().declared_terminals(),
        &std::collections::BTreeSet::from([key([1])])
    );
}

#[test]
fn census_retains_composite_baseline_and_whole_piece_alternative_winner() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let baseline = rule(&family, [3], &[([1], 1)]);
    let mut alternative = rule(&family, [3], &[([2], 1)]);
    alternative.dispatch_policy = RuleDispatchPolicy::AfterBaselinePartitionWholePiece;
    let reducer = RoutedCandidateReducer::try_new(
        programs(
            family,
            Some(0),
            vec![input(
                [true],
                Some(0),
                vec![baseline, alternative],
                &[[1], [2]],
            )],
            Default::default(),
        ),
        [],
        Default::default(),
    )
    .unwrap();
    let report = run(&reducer, vec![key([3])], Default::default()).unwrap();
    assert_eq!(
        report.complete_census().unwrap().selections(),
        &BTreeMap::from([(
            CandidateRoutedSelectionIdentity {
                owner: vec![true],
                batch_ordinal: 0,
                baseline_rule_ordinal: 0,
                selected_rule_ordinal: 1
            },
            1
        )])
    );
    assert_eq!(
        report.trace().declared_terminals(),
        &std::collections::BTreeSet::from([key([2])])
    );
}

#[test]
fn census_records_the_actual_later_batch_after_uncovered_preference() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let base = input([true], None, vec![rule(&family, [2], &[([1], 1)])], &[[1]]);
    let preferred = input([true], None, vec![rule(&family, [3], &[([1], 1)])], &[]);
    let programs = CandidateOwnerPrograms::try_new_with_preferences(
        context(family, None, Default::default()),
        [base],
        [preferred],
    )
    .unwrap();
    let reducer =
        RoutedCandidateReducer::try_new(Arc::new(programs), [], Default::default()).unwrap();
    let report = run(&reducer, vec![key([2])], Default::default()).unwrap();
    assert_eq!(
        report.complete_census().unwrap().selections(),
        &BTreeMap::from([(
            CandidateRoutedSelectionIdentity {
                owner: vec![true],
                batch_ordinal: 1,
                baseline_rule_ordinal: 0,
                selected_rule_ordinal: 0
            },
            1
        )])
    );
}

#[test]
fn terminal_only_trace_has_complete_empty_census_under_zero_selection_budget() {
    let reducer = super::super::tests::diamond(Default::default(), Default::default());
    let report = run(
        &reducer,
        vec![key([1])],
        CandidateRoutedTraceCensusLimits {
            max_unique_selections: 0,
            max_identity_payload_bytes: 0,
        },
    )
    .unwrap();
    let census = report.complete_census().unwrap();
    assert!(census.selections().is_empty());
    assert_eq!(census.identity_payload_bytes(), 0);
    assert_eq!(
        report.trace().declared_terminals(),
        &std::collections::BTreeSet::from([key([1])])
    );
}

#[test]
fn zero_rhs_still_records_its_applied_rule_but_no_terminal() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let source = input([true], Some(0), vec![rule(&family, [2], &[])], &[]);
    let reducer = RoutedCandidateReducer::try_new(
        programs(family, Some(0), vec![source], Default::default()),
        [],
        Default::default(),
    )
    .unwrap();
    let report = run(&reducer, vec![key([2])], Default::default()).unwrap();
    assert_eq!(
        report
            .complete_census()
            .unwrap()
            .selections()
            .values()
            .sum::<u64>(),
        1
    );
    assert!(report.trace().declared_terminals().is_empty());
    assert!(report.trace().visited_zeros().is_empty());
}

#[test]
fn census_quota_refuses_complete_inventory_without_turning_work_into_a_master() {
    let reducer = super::super::tests::diamond(Default::default(), Default::default());
    for limits in [
        CandidateRoutedTraceCensusLimits {
            max_unique_selections: 0,
            ..Default::default()
        },
        CandidateRoutedTraceCensusLimits {
            max_identity_payload_bytes: 0,
            ..Default::default()
        },
    ] {
        let error = run(&reducer, vec![key([6])], limits).unwrap_err();
        assert!(matches!(
            error.reason(),
            CandidateRoutedCampaignFailure::Trace(CandidateRoutedError::ResourceLimit { .. })
        ));
        assert!(error.partial_report().complete_census().is_none());
        assert!(!error.snapshot().finished);
        assert!(
            error
                .partial_report()
                .trace()
                .declared_terminals()
                .is_empty()
        );
    }
}

#[test]
fn frontier_or_local_application_failure_cannot_supply_complete_census() {
    let mut reducer = super::super::tests::diamond(Default::default(), Default::default());
    let frontier = run(&reducer, vec![key([7])], Default::default()).unwrap();
    assert!(frontier.snapshot().finished);
    assert_eq!(frontier.trace().frontier().len(), 1);
    assert!(frontier.complete_census().is_none());
    let programs = Arc::get_mut(&mut reducer.programs).unwrap();
    let owner = Arc::get_mut(programs.owners.get_mut(&[true]).unwrap()).unwrap();
    Arc::get_mut(&mut owner.batches[0]).unwrap().rules[2].rhs[0].shift = [1];
    let error = run(&reducer, vec![key([6])], Default::default()).unwrap_err();
    assert!(!error.snapshot().finished);
    assert!(error.partial_report().complete_census().is_none());
    let partial = error.partial_report().census.as_ref().unwrap();
    assert!(!partial.selections().is_empty());
    assert!(
        partial
            .selections()
            .keys()
            .all(|id| id.selected_rule_ordinal != 2)
    );
}

#[test]
fn cancellation_after_completed_prefix_cannot_supply_complete_census() {
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
    let reducer = RoutedCandidateReducer::try_new(
        programs(
            family,
            Some(0),
            vec![input([true; 3], Some(0), rules, &[[1, 1, 1]])],
            Default::default(),
        ),
        [],
        Default::default(),
    )
    .unwrap();
    let cancel = AtomicBool::new(false);
    let error = reducer
        .trace_targets_inline_with_entry_admission_and_census_observer(
            [key([22, 22, 1])],
            CandidateEntryAdmission::SavedGenerationScope,
            budget(),
            Default::default(),
            &cancel,
            |snapshot| {
                if snapshot.completed_nodes >= 256 {
                    cancel.store(true, Ordering::Release);
                }
            },
        )
        .unwrap_err();
    assert_eq!(error.reason(), &CandidateRoutedCampaignFailure::Cancelled);
    assert_eq!(error.snapshot().completed_nodes, 256);
    assert!(error.partial_report().complete_census().is_none());
    assert!(
        !error
            .partial_report()
            .census
            .as_ref()
            .unwrap()
            .selections()
            .is_empty()
    );
}

#[test]
fn census_payload_budget_counts_distinct_identity_and_not_duplicate_visits() {
    let mut census = CandidateRoutedTraceCensus::default();
    let bytes = std::mem::size_of::<(CandidateRoutedSelectionIdentity, u64)>() + 3;
    let limits = CandidateRoutedTraceCensusLimits {
        max_unique_selections: 1,
        max_identity_payload_bytes: bytes,
    };
    census
        .record(&[true, false, true], 2, 10, 110, limits)
        .unwrap();
    census
        .record(&[true, false, true], 2, 10, 110, limits)
        .unwrap();
    assert_eq!(census.identity_payload_bytes(), bytes);
    assert_eq!(
        census.selections().values().copied().collect::<Vec<_>>(),
        vec![2]
    );
    let before = census.clone();
    assert!(
        census
            .record(&[true, false, true], 2, 10, 111, limits)
            .is_err()
    );
    assert_eq!(census, before);
}

#[test]
fn census_multiplicity_overflow_preserves_the_existing_prefix() {
    let mut census = CandidateRoutedTraceCensus::default();
    let limits = CandidateRoutedTraceCensusLimits::default();
    census.record(&[true], 0, 0, 110, limits).unwrap();
    *census.selections.values_mut().next().unwrap() = u64::MAX;
    let before = census.clone();
    let error = census.record(&[true], 0, 0, 110, limits).unwrap_err();
    assert!(matches!(
        error,
        CandidateRoutedError::ResourceLimit {
            resource: "trace census selection multiplicity",
            ..
        }
    ));
    assert_eq!(census, before);
}

#[test]
fn census_does_not_turn_original_pole_on_zero_coefficient_into_success() {
    // Reuse the ordinary inline trace's exact native denominator-pole fixture.
    let mut reducer = super::super::tests::diamond(Default::default(), Default::default());
    let programs = Arc::get_mut(&mut reducer.programs).unwrap();
    let context = programs.context.coefficient_context().clone();
    let pole = context
        .numerator_condition_with_limits(
            &context
                .sub(&context.index(0).unwrap(), &context.integer(4))
                .unwrap(),
            Default::default(),
        )
        .unwrap();
    let owner = Arc::get_mut(programs.owners.get_mut(&[true]).unwrap()).unwrap();
    let batch = Arc::get_mut(&mut owner.batches[0]).unwrap();
    batch.rules[2].rhs[0].coefficient = context.zero();
    batch.rules[2].rhs[0].denominator = pole;
    let report = run(&reducer, vec![key([6])], Default::default()).unwrap();
    assert!(report.snapshot().finished);
    assert_eq!(report.snapshot().missing_rules, 1);
    assert!(report.complete_census().is_none());
    assert!(
        report
            .census
            .as_ref()
            .unwrap()
            .selections()
            .keys()
            .all(|identity| identity.selected_rule_ordinal != 2)
    );
    assert!(!report.trace().declared_terminals().contains(&key([4])));
}
