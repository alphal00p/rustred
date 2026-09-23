//! Entry admission changes no rule, terminal or source-condition authority.
use super::*;
use crate::solver::{CandidateRoutedCampaignFailure, DomainPowerBounds};
use std::sync::atomic::AtomicBool;

fn fixed_region<const N: usize>(powers: [i64; N]) -> RootRegionInput<N> {
    let lower = powers.map(|n| {
        if n > 0 {
            n as u64 - 1
        } else {
            n.unsigned_abs()
        }
    });
    RootRegionInput {
        support: powers.map(|n| n > 0),
        lower: lower.to_vec(),
        upper: lower.map(Some).to_vec(),
        rank: None,
        powers: DomainPowerBounds::default(),
    }
}

fn same_parallel<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    policy: &FiniteRootAdmission<N>,
    targets: Vec<crate::family::IntegralKey>,
) -> CandidateRoutedTraceReport<N> {
    let admission = CandidateEntryAdmission::ExplicitFinite(policy);
    let serial = reducer
        .trace_targets_with_entry_admission(targets.clone(), admission)
        .unwrap();
    for workers in [1, 2, 6] {
        let parallel = reducer
            .trace_targets_parallel_with_entry_admission_and_observer(
                targets.clone(),
                admission,
                workers,
                &AtomicBool::new(false),
                |_| {},
            )
            .unwrap();
        assert_eq!(parallel.trace(), &serial);
        assert!(parallel.snapshot().finished);
    }
    serial
}

#[test]
fn explicit_roots_replace_saved_rank_without_clipping_descendants() {
    let family = Arc::new(crate::solver::tests::sunset());
    let mut source = input(
        [true, true, false],
        Some(10),
        vec![rule(&family, [1, 3, -15], &[([1, 1, -16], 1)])],
        &[[1, 1, -16]],
    );
    source.ordering = OrderingPolicy::SpiredUncutV1;
    let reducer = routed(family, Some(10), vec![source]);
    let entry = key([1, 3, -15]);
    assert!(matches!(
        reducer.trace_targets([entry.clone()]),
        Err(CandidateRoutedError::Candidate(
            crate::solver::CandidateReductionError::OutsideNumeratorRank { .. }
        ))
    ));
    // Exact A=4, R<=15, D=-11; the descendant has A=2, R=16, D=-14.
    let mut region = fixed_region([1, 3, -15]);
    region.rank = Some(15);
    region.powers = DomainPowerBounds {
        max_positive_power: Some(4),
        min_power_difference: Some(-11),
        max_power_difference: Some(-11),
    };
    let policy = FiniteRootAdmission::try_new([region], 1).unwrap();
    assert!(policy.validate_entry(&key([1, 1, -16])).is_err());
    let report = same_parallel(&reducer, &policy, vec![entry]);
    assert_eq!(report.rule_applications(), 1);
    assert_eq!(report.max_negative_index_degree(), 16);
    assert_eq!(
        report.declared_terminals(),
        &BTreeSet::from([key([1, 1, -16])])
    );
    assert!(report.frontier().is_empty());
    assert_eq!(
        reducer.programs().context().scope().max_numerator_rank,
        Some(10)
    );
}

#[test]
fn selected_routing_only_support_and_missing_owner_keep_native_meanings() {
    let family = Arc::new(crate::solver::tests::sunset());
    let programs = programs(
        family.clone(),
        Some(10),
        vec![input([true, false, true], Some(10), vec![], &[[1, -11, 1]])],
        Default::default(),
    );
    let reducer = RoutedCandidateReducer::try_new(
        programs,
        [swap_route(family, [false, true, true], [true, false, true])],
        Default::default(),
    )
    .unwrap();
    let policy =
        FiniteRootAdmission::try_new([fixed_region([-11, 1, 1]), fixed_region([1, 1, 0])], 2)
            .unwrap();
    // Neither the routed image nor its literal owner support is admitted as a root.
    assert!(matches!(
        policy.validate_entry(&key([1, -11, 1])),
        Err(RootAdmissionError::OutsideSelectedSupport { .. })
    ));
    let report = same_parallel(&reducer, &policy, vec![key([-11, 1, 1]), key([1, 1, 0])]);
    assert_eq!(report.transport_calls(), 1);
    assert_eq!(
        report.declared_terminals(),
        &BTreeSet::from([key([1, -11, 1])])
    );
    assert_eq!(report.frontier().len(), 1);
    let missing = report.frontier().first().unwrap();
    assert_eq!(missing.target, key([1, 1, 0]));
    assert!(matches!(
        missing.reason,
        CandidateRoutedFrontierReason::MissingOwner
    ));
}

#[test]
fn rejected_roots_fail_before_scheduling_but_emit_error_snapshot() {
    let family = Arc::new(crate::solver::tests::sunset());
    let reducer = routed(
        family,
        Some(10),
        vec![input([true; 3], Some(10), vec![], &[])],
    );
    let policy =
        FiniteRootAdmission::try_new([fixed_region([1, 1, -11]), fixed_region([3, 1, -11])], 2)
            .unwrap();
    for (target, support_error) in [(key([2, 1, -11]), false), (key([1, 1, 1]), true)] {
        let admission = CandidateEntryAdmission::ExplicitFinite(&policy);
        let expected = policy.validate_entry(&target).unwrap_err();
        assert_eq!(
            matches!(expected, RootAdmissionError::OutsideSelectedSupport { .. }),
            support_error
        );
        assert_eq!(
            reducer
                .trace_targets_with_entry_admission([key([1, 1, -11]), target.clone()], admission),
            Err(CandidateRoutedError::EntryAdmission(expected.clone()))
        );
        for workers in [1, 2, 6] {
            let mut observed = 0;
            let error = reducer
                .trace_targets_parallel_with_entry_admission_and_observer(
                    [key([1, 1, -11]), target.clone()],
                    admission,
                    workers,
                    &AtomicBool::new(false),
                    |snapshot| {
                        observed += 1;
                        assert_eq!(snapshot.scheduled_nodes, 0);
                        assert_eq!(snapshot.completed_nodes, 0);
                        assert_eq!(snapshot.rule_attempts, 0);
                    },
                )
                .unwrap_err();
            assert_eq!(observed, 1);
            assert_eq!(
                error.reason(),
                &CandidateRoutedCampaignFailure::Trace(CandidateRoutedError::EntryAdmission(
                    expected.clone()
                ))
            );
        }
    }
}

#[test]
fn explicit_admission_preserves_initial_and_descendant_source_guards() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let mut shared = context(family.clone(), Some(0), Default::default());
    let mutable = Arc::get_mut(&mut shared).unwrap();
    let c = &mutable.shared.context;
    let condition = c
        .sub(&c.index(0).unwrap(), &c.integer(2))
        .unwrap()
        .raw()
        .numerator
        .clone();
    let ordinal = mutable.shared.source_conditions.len();
    mutable.shared.source_conditions.push(
        c.admit_native_polynomial_result_with_limits(condition, Default::default())
            .unwrap(),
    );
    let programs = Arc::new(
        crate::solver::CandidateOwnerPrograms::try_new(
            shared,
            [input(
                [true],
                Some(0),
                vec![rule(&family, [3], &[([2], 1)])],
                &[[2]],
            )],
        )
        .unwrap(),
    );
    let reducer = RoutedCandidateReducer::try_new(programs, [], Default::default()).unwrap();
    let policy = FiniteRootAdmission::try_new([fixed_region([3])], 1).unwrap();
    let admission = CandidateEntryAdmission::ExplicitFinite(&policy);
    for root in [2, 3] {
        // n=2 is outside the entry region AND invalid as a source. Source
        // validation comes first; n=3 reaches the same invalid terminal child.
        let expected = CandidateRoutedError::Candidate(
            crate::solver::CandidateReductionError::SourceConditionVanished {
                target: key([2]),
                ordinal,
            },
        );
        assert_eq!(
            reducer.trace_targets_with_entry_admission([key([root])], admission),
            Err(expected.clone())
        );
        for workers in [1, 2, 6] {
            let error = reducer
                .trace_targets_parallel_with_entry_admission_and_observer(
                    [key([root])],
                    admission,
                    workers,
                    &AtomicBool::new(false),
                    |_| {},
                )
                .unwrap_err();
            assert_eq!(
                error.reason(),
                &CandidateRoutedCampaignFailure::Trace(expected.clone())
            );
        }
    }
}
