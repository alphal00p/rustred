use super::*;
use crate::algebra::{IndexedCoefficientContext, IndexedPolynomial};
use crate::reduction::{ReductionError, ReductionLimits};
use crate::sector::symmetry::{self, CoefficientMatrix, MomentumMap, integral_transport};
use crate::sector::{Mask, OrderingPolicy};
use crate::solver::candidate_reduction::owner_test_support::*;
use crate::solver::{
    CandidateEntryAdmission, CandidateOwnerPrograms, CandidateOwnerRoute, CandidateReducer,
    CandidateReductionError, DomainPowerBounds, FiniteRootAdmission, RootRegionInput,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[test]
fn ordered_frontier_cancels_children_before_expansion_without_crossfeeding_cache() {
    let routed = tadpole(Default::default());
    let c = routed.programs.context.family().coefficient_context();
    let mut application = routed.application();
    let expected = BTreeMap::from([(key([1]), c.integer(2))]);
    let warm = application
        .reduce_linear_combination([(key([3]), c.one())])
        .unwrap();
    assert_eq!(warm.statistics().work.rule_applications(), 2);
    let retained = application.cache_statistics();
    application.set_reduction_strategy(crate::reduction::ReductionStrategy::Ordered);
    let result = application
        .reduce_linear_combination([(key([3]), c.one()), (key([2]), c.integer(3))])
        .unwrap();
    assert_eq!(result.terms(), &expected);
    assert_eq!(result.statistics().work.rule_applications(), 1);
    assert_eq!(result.statistics().work.cache_hits(), 0);
    assert_eq!(
        application.cache_statistics().cached_integrals(),
        retained.cached_integrals()
    );
    application.set_reduction_strategy(crate::reduction::ReductionStrategy::DepthFirst);
    let old = application
        .reduce_linear_combination([(key([3]), c.one())])
        .unwrap();
    assert_eq!(old.terms(), warm.terms());
    assert_eq!(old.statistics().work.rule_applications(), 0);
    assert!(old.statistics().work.cache_hits() > 0);
}

#[test]
fn ordered_phases_have_frozen_total_and_exact_grouping_counters() {
    let routed = tadpole(Default::default());
    let c = routed.programs.context.family().coefficient_context();
    let mut application = routed.application();
    application
        .reduce_linear_combination([(key([3]), c.one())])
        .unwrap();
    assert!(application.cache_statistics().cached_integrals() > 0);
    application.set_reduction_strategy(crate::reduction::ReductionStrategy::Ordered);
    let mut observations = Vec::new();
    let mut callback = |event: &CandidateRoutedApplicationEnvelope<'_>| {
        if let Some(progress) = event.ordered {
            assert!(event.frontier.is_none());
            assert!(event.complexity.coefficient_terms.is_none());
            assert!(event.complexity.coefficient_bytes.is_none());
            assert_eq!(event.snapshot.unwrap().cached.entries, 0);
            observations.push(progress);
        }
        Ok(CandidateRoutedObserverControl::Continue)
    };
    let mut observer = CandidateRoutedApplicationObserver::new(
        CandidateRoutedObservationOptions {
            interval: Duration::ZERO,
            ..Default::default()
        },
        &mut callback,
    );
    let result = application
        .reduce_linear_combination_with_observation(
            [(key([3]), c.one()), (key([2]), c.integer(3))],
            CandidateEntryAdmission::SavedGenerationScope,
            &mut observer,
            None,
        )
        .unwrap();
    assert_eq!(result.terms(), &BTreeMap::from([(key([1]), c.integer(2))]));
    assert!(!observations.is_empty());
    assert!(observations.iter().all(|value| value.total_phases == 3));
    assert!(
        observations
            .windows(2)
            .all(|pair| pair[0].completed_phases <= pair[1].completed_phases)
    );
    let last = observations.last().unwrap();
    assert_eq!(last.completed_phases, 3);
    assert_eq!(last.cancelled_terms, 1);
    assert_eq!(last.frontier_keys, 0);
    assert_eq!(last.terminal_keys, 1);
}

#[test]
fn ordered_inputs_are_validated_before_cancellation_and_residuals_after_combination() {
    let routed = partial_routed_fixture(Default::default());
    let c = routed.programs.context.family().coefficient_context();
    let mut application = routed.application();
    application
        .set_unresolved_policy(UnresolvedPolicy::Preserve)
        .unwrap();
    application.set_reduction_strategy(crate::reduction::ReductionStrategy::Ordered);
    let result = application
        .reduce_linear_combination([(key([4]), c.one())])
        .unwrap();
    assert!(result.is_zero());
    assert!(result.unresolved().is_empty());
    assert_eq!(result.nonzero_conditions(), &[c.coefficient_fixture("d-7")]);
    assert_eq!(result.statistics().work.cache_hits(), 0);
    assert_eq!(application.cache_statistics().cached_integrals(), 0);
    assert!(
        application
            .reduce_linear_combination([(key([1, 0]), c.zero())])
            .is_err()
    );
    let surviving = application
        .reduce_linear_combination([(key([3]), c.one())])
        .unwrap();
    assert_eq!(surviving.unresolved(), &BTreeSet::from([key([2])]));
}

#[test]
fn ordered_failed_requests_preserve_attempted_work_and_monotone_observed_counters() {
    let routed = tadpole(Default::default());
    let c = routed.programs.context.family().coefficient_context();
    let mut application = routed.application();
    application.set_reduction_strategy(crate::reduction::ReductionStrategy::Ordered);
    let cancel = AtomicBool::new(false);
    let mut work = Vec::new();
    let mut last_status = None;
    let mut callback = |event: &CandidateRoutedApplicationEnvelope<'_>| {
        if let Some(snapshot) = event.snapshot {
            work.push(snapshot.statistics.work.rule_applications());
            last_status = Some(snapshot.status);
            if snapshot.statistics.work.rule_applications() > 0 {
                cancel.store(true, Ordering::Relaxed);
            }
        }
        Ok(CandidateRoutedObserverControl::Continue)
    };
    let mut observer = CandidateRoutedApplicationObserver::new(
        CandidateRoutedObservationOptions {
            interval: Duration::ZERO,
            ..Default::default()
        },
        &mut callback,
    );
    assert!(matches!(
        application.reduce_linear_combination_with_observation(
            [(key([3]), c.one())],
            CandidateEntryAdmission::SavedGenerationScope,
            &mut observer,
            Some(&cancel)
        ),
        Err(CandidateRoutedError::Cancelled)
    ));
    assert_eq!(last_status, Some(CandidateRoutedApplicationStatus::Failed));
    assert_eq!(work.last(), Some(&1));
    assert!(work.windows(2).all(|pair| pair[0] <= pair[1]));
    assert_eq!(application.cache_statistics().rule_applications(), 1);
    assert_eq!(application.cache_statistics().cached_integrals(), 0);
}

#[test]
fn ordered_failed_frontier_combination_is_included_in_attempted_work() {
    let routed = tadpole(ReductionLimits {
        diagnostics_enabled: true,
        max_coalescing_additions: 0,
        ..Default::default()
    });
    let c = routed.programs.context.family().coefficient_context();
    let mut application = routed.application();
    application.set_reduction_strategy(crate::reduction::ReductionStrategy::Ordered);
    assert!(
        application
            .reduce_linear_combination([(key([3]), c.one()), (key([2]), c.integer(3))])
            .is_err()
    );
    assert_eq!(application.cache_statistics().rule_applications(), 1);
    assert_eq!(application.cache_statistics().coalescing_additions(), 1);
    assert_eq!(application.cache_statistics().cached_integrals(), 0);
}

fn tadpole(limits: ReductionLimits) -> RoutedCandidateReducer<1> {
    let family = Arc::new(crate::solver::tests::tadpole());
    let owners = programs(
        family.clone(),
        None,
        vec![input(
            [true],
            None,
            vec![
                rule(&family, [3], &[([2], -3), ([1], 2)]),
                rule(&family, [2], &[([1], 2)]),
            ],
            &[[1]],
        )],
        limits,
    );
    let mut trace = RoutedCandidateLimits::default();
    trace.expansion.exact_algebra = limits.exact_algebra;
    RoutedCandidateReducer::try_new(owners, [], trace).unwrap()
}

fn partial_routed_fixture(limits: ReductionLimits) -> RoutedCandidateReducer<1> {
    let family = Arc::new(crate::solver::tests::tadpole());
    let mut owners = CandidateOwnerPrograms::try_new(
        context(family.clone(), None, limits),
        [input(
            [true],
            None,
            vec![
                rule(&family, [4], &[([3], 1), ([2], 3), ([1], -2)]),
                rule(&family, [3], &[([2], -3), ([1], 2)]),
            ],
            &[[1]],
        )],
    )
    .unwrap();
    let indexed = owners.context.coefficient_context().clone();
    Arc::get_mut(&mut owners.context)
        .unwrap()
        .shared
        .source_conditions = vec![polynomial(&indexed, "d-7")];
    RoutedCandidateReducer::try_new(Arc::new(owners), [], Default::default()).unwrap()
}

#[test]
fn partial_routed_results_preserve_unknown_coefficients_guards_and_scope_holes() {
    let routed = partial_routed_fixture(Default::default());
    let c = routed.programs.context.family().coefficient_context();
    let admission = FiniteRootAdmission::try_new(
        [RootRegionInput {
            support: [true],
            lower: vec![0],
            upper: vec![Some(0)],
            rank: None,
            powers: DomainPowerBounds::default(),
        }],
        1,
    )
    .unwrap();
    let mut application = routed.application();
    application
        .set_unresolved_policy(UnresolvedPolicy::Preserve)
        .unwrap();
    for _ in 0..2 {
        let result = application
            .reduce_routed_linear_combination_with_entry_admission(
                [(key([3]), c.one())],
                CandidateEntryAdmission::ExplicitFinite(&admission),
            )
            .unwrap();
        assert_eq!(
            result.terms(),
            &BTreeMap::from([(key([2]), c.integer(-3)), (key([1]), c.integer(2))])
        );
        assert_eq!(result.unresolved(), &BTreeSet::from([key([2])]));
        assert_eq!(result.nonzero_conditions(), &[c.coefficient_fixture("d-7")]);
    }
    let cancelled = application
        .reduce_linear_combination([(key([4]), c.one())])
        .unwrap();
    assert!(cancelled.is_zero());
    assert!(cancelled.unresolved().is_empty());
    assert_eq!(
        cancelled.nonzero_conditions(),
        &[c.coefficient_fixture("d-7")]
    );
    assert!(
        application
            .set_unresolved_policy(UnresolvedPolicy::Reject)
            .is_err()
    );
    application.clear_cache().unwrap();
    application
        .set_unresolved_policy(UnresolvedPolicy::Reject)
        .unwrap();
    assert!(matches!(
        application.reduce_linear_combination([(key([3]), c.one())]),
        Err(CandidateRoutedError::Candidate(
            CandidateReductionError::Uncovered { .. }
        ))
    ));
}

#[test]
fn partial_routed_missing_owner_is_retained_and_cancelled_inputs_keep_guards() {
    let routed = partial_routed_fixture(Default::default());
    let c = routed.programs.context.family().coefficient_context();
    let mut application = routed.application();
    application
        .set_unresolved_policy(UnresolvedPolicy::Preserve)
        .unwrap();
    let result = application
        .reduce_routed_linear_combination_with_entry_admission(
            [(key([-1]), c.integer(3))],
            CandidateEntryAdmission::SavedGenerationScope,
        )
        .unwrap();
    assert_eq!(result.terms(), &BTreeMap::from([(key([-1]), c.integer(3))]));
    assert_eq!(result.unresolved(), &BTreeSet::from([key([-1])]));
    assert_eq!(result.nonzero_conditions(), &[c.coefficient_fixture("d-7")]);
    let result = application
        .reduce_routed_linear_combination_with_entry_admission(
            [(key([-2]), c.one()), (key([-2]), c.integer(-1))],
            CandidateEntryAdmission::SavedGenerationScope,
        )
        .unwrap();
    assert!(result.is_zero());
    assert!(result.unresolved().is_empty());
    assert_eq!(result.nonzero_conditions(), &[c.coefficient_fixture("d-7")]);
    assert!(matches!(
        application.route_entry_to_owner(&key([-1])),
        Err(CandidateRoutedError::MissingOwner { .. })
    ));
}

#[test]
fn partial_routed_policy_does_not_hide_malformed_keys_or_operational_limits() {
    let routed = partial_routed_fixture(ReductionLimits {
        diagnostics_enabled: true,
        max_rule_applications: 0,
        ..Default::default()
    });
    let c = routed.programs.context.family().coefficient_context();
    let mut application = routed.application();
    application
        .set_unresolved_policy(UnresolvedPolicy::Preserve)
        .unwrap();
    assert!(
        application
            .reduce_linear_combination([(key([3]), c.one())])
            .is_err()
    );
    assert!(matches!(
        application.reduce_linear_combination([(key([1, 0]), c.zero())]),
        Err(CandidateRoutedError::Candidate(
            CandidateReductionError::Application(ReductionError::WrongArity { .. })
        ))
    ));
}

#[test]
fn single_owner_matches_existing_exact_reducer_and_batch_cancels() {
    let routed = tadpole(Default::default());
    let family = routed.programs.context.family();
    let c = family.coefficient_context();
    let mut old = CandidateReducer::try_new(
        family,
        [true],
        OrderingPolicy::SpiredUncutV1,
        [(
            [true],
            input(
                [true],
                None,
                vec![
                    rule(family, [3], &[([2], -3), ([1], 2)]),
                    rule(family, [2], &[([1], 2)]),
                ],
                &[[1]],
            )
            .solution,
        )],
        vec![],
        Default::default(),
    )
    .unwrap();
    let mut application = routed.application();
    let actual = application
        .reduce_linear_combination([(key([3]), c.one())])
        .unwrap();
    assert_eq!(
        actual.terms(),
        old.reduce_unit_mass(&key([3])).unwrap().terms()
    );
    assert_eq!(actual.terms()[&key([1])], c.integer(-4));
    assert_eq!(actual.statistics().work.rule_applications(), 2);
    assert!(
        application
            .reduce_linear_combination([(key([3]), c.one()), (key([2]), c.integer(2))])
            .unwrap()
            .is_zero()
    );
}

#[test]
fn warm_cache_skips_rules_and_clear_cache_releases_all_payload() {
    let routed = tadpole(Default::default());
    let c = routed.programs.context.family().coefficient_context();
    let mut application = routed.application();
    let cold = application
        .reduce_linear_combination([(key([3]), c.one())])
        .unwrap();
    let warm = application
        .reduce_linear_combination([(key([3]), c.one())])
        .unwrap();
    assert_eq!(cold.terms(), warm.terms());
    assert_eq!(warm.statistics().work.rule_applications(), 0);
    assert_eq!(warm.statistics().work.cache_hits(), 1);
    assert_eq!(application.cache_statistics().cached_integrals(), 4);
    assert!(application.cache_statistics().cached_coefficient_bytes() > 0);
    application.clear_cache().unwrap();
    assert_eq!(application.cache_statistics().cached_integrals(), 0);
    assert_eq!(application.cache_statistics().cached_coefficient_terms(), 0);
    assert_eq!(application.cache_statistics().cached_coefficient_bytes(), 0);
    assert_eq!(
        application
            .reduce_linear_combination([(key([3]), c.one())])
            .unwrap()
            .statistics()
            .work
            .rule_applications(),
        2
    );
}

#[test]
fn admission_and_arity_precede_cache_lookup_and_input_cancellation() {
    let routed = tadpole(Default::default());
    let c = routed.programs.context.family().coefficient_context();
    let mut application = routed.application();
    application
        .reduce_linear_combination([(key([3]), c.one())])
        .unwrap();
    let admission = FiniteRootAdmission::try_new(
        [RootRegionInput {
            support: [true],
            lower: vec![0],
            upper: vec![Some(0)],
            rank: None,
            powers: DomainPowerBounds::default(),
        }],
        1,
    )
    .unwrap();
    for coefficient in [c.zero(), c.one()] {
        assert!(matches!(
            application.reduce_linear_combination_with_entry_admission(
                [(key([3]), coefficient)],
                CandidateEntryAdmission::ExplicitFinite(&admission)
            ),
            Err(CandidateRoutedError::EntryAdmission(_))
        ));
    }
    assert!(matches!(
        application.reduce_linear_combination_with_entry_admission(
            [(key([3]), c.one()), (key([3]), c.integer(-1))],
            CandidateEntryAdmission::ExplicitFinite(&admission)
        ),
        Err(CandidateRoutedError::EntryAdmission(_))
    ));
    assert!(matches!(
        application.reduce_linear_combination([(key([1, 0]), c.zero())]),
        Err(CandidateRoutedError::Candidate(
            CandidateReductionError::Application(ReductionError::WrongArity { .. })
        ))
    ));
}

#[test]
fn cooperative_cancellation_preserves_complete_root_admission_before_cache_lookup() {
    let routed = tadpole(Default::default());
    let c = routed.programs.context.family().coefficient_context();
    let mut application = routed.application();
    application
        .reduce_linear_combination([(key([3]), c.one())])
        .unwrap();
    let cancel = AtomicBool::new(true);
    let admission = FiniteRootAdmission::try_new(
        [RootRegionInput {
            support: [true],
            lower: vec![0],
            upper: vec![Some(0)],
            rank: None,
            powers: DomainPowerBounds::default(),
        }],
        1,
    )
    .unwrap();
    // Cancellation and warm cache cannot conceal a later inadmissible input,
    // including one whose caller coefficient is zero.
    for coefficient in [c.zero(), c.one()] {
        assert!(matches!(
            application.reduce_linear_combination_with_entry_admission_cancellable(
                [(key([1]), c.one()), (key([3]), coefficient)],
                CandidateEntryAdmission::ExplicitFinite(&admission),
                &cancel,
            ),
            Err(CandidateRoutedError::EntryAdmission(_))
        ));
    }
    assert!(matches!(
        application.reduce_linear_combination_with_entry_admission_cancellable(
            [(key([1, 0]), c.zero())],
            CandidateEntryAdmission::SavedGenerationScope,
            &cancel,
        ),
        Err(CandidateRoutedError::Candidate(
            CandidateReductionError::Application(ReductionError::WrongArity { .. })
        ))
    ));
    let mut snapshots = Vec::new();
    assert!(matches!(
        application.reduce_linear_combination_with_entry_admission_and_control(
            [(key([3]), c.one())],
            CandidateEntryAdmission::SavedGenerationScope,
            &cancel,
            Duration::ZERO,
            |snapshot| snapshots.push(*snapshot),
        ),
        Err(CandidateRoutedError::Cancelled)
    ));
    assert_eq!(snapshots.len(), 2);
    let last = snapshots.last().unwrap();
    assert_eq!(last.status, CandidateRoutedApplicationStatus::Failed);
    assert_eq!(last.unique_nodes, 0);
    assert_eq!(last.statistics.work.cache_hits(), 0);
    // The original API remains independent of an unrelated cancellation flag.
    assert_eq!(
        application
            .reduce_linear_combination([(key([3]), c.one())])
            .unwrap()
            .terms()[&key([1])],
        c.integer(-4)
    );
}

#[test]
fn cooperative_cancellation_retains_only_completed_guarded_cache_and_releases_pins() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let c = family.coefficient_context();
    let mut owners = CandidateOwnerPrograms::try_new(
        context(
            family.clone(),
            None,
            ReductionLimits {
                max_cached_integrals: 2,
                ..Default::default()
            },
        ),
        [input(
            [true],
            None,
            vec![
                rule(&family, [3], &[([2], 3)]),
                rule(&family, [2], &[([1], 2)]),
            ],
            &[[1], [5]],
        )],
    )
    .unwrap();
    let indexed = owners.context.coefficient_context().clone();
    Arc::get_mut(&mut owners.context)
        .unwrap()
        .shared
        .source_conditions = vec![polynomial(&indexed, "d-7")];
    let routed = RoutedCandidateReducer::try_new(Arc::new(owners), [], Default::default()).unwrap();
    for policy in [
        CandidateRoutedCachePolicy::EvictLeastRecentlyUsed,
        CandidateRoutedCachePolicy::EvictAgedLeastFrequentlyUsed,
    ] {
        for representation in [
            CandidateCacheRepresentation::Sparse,
            CandidateCacheRepresentation::Factorized,
        ] {
            let mut application = routed.application();
            application.set_cache_policy(policy).unwrap();
            application
                .set_cache_representation(representation)
                .unwrap();
            let cancel = AtomicBool::new(false);
            let mut snapshots = Vec::new();
            assert!(matches!(
                application.reduce_linear_combination_with_entry_admission_and_control(
                    [(key([3]), c.one())],
                    CandidateEntryAdmission::SavedGenerationScope,
                    &cancel,
                    Duration::ZERO,
                    |snapshot| {
                        snapshots.push(*snapshot);
                        if snapshot.cached.entries > 0 && snapshot.active_nodes > 0 {
                            cancel.store(true, Ordering::Relaxed);
                        }
                    },
                ),
                Err(CandidateRoutedError::Cancelled)
            ));
            let last = snapshots.last().unwrap();
            assert_eq!(last.status, CandidateRoutedApplicationStatus::Failed);
            assert!(last.pinned.unwrap().entries > 0);
            assert!(last.active_nodes > 0);
            assert!(
                !application
                    .cache
                    .contains_key(&super::super::trace::Node::Route(key([3])))
            );
            let completed_leaf = application
                .reduce_linear_combination([(key([1]), c.one())])
                .unwrap();
            assert_eq!(completed_leaf.statistics().work.cache_hits(), 1);
            assert_eq!(
                completed_leaf.nonzero_conditions(),
                &[c.coefficient_fixture("d-7")]
            );
            // A separate root can evict the completed leaf: aborted parent
            // references no longer pin any payload after the typed return.
            application
                .reduce_linear_combination([(key([5]), c.one())])
                .unwrap();
            cancel.store(false, Ordering::Relaxed);
            let resumed = application
                .reduce_linear_combination_with_entry_admission_cancellable(
                    [(key([3]), c.one())],
                    CandidateEntryAdmission::SavedGenerationScope,
                    &cancel,
                )
                .unwrap();
            assert_eq!(resumed.terms(), &BTreeMap::from([(key([1]), c.integer(6))]));
            assert_eq!(
                resumed.nonzero_conditions(),
                &[c.coefficient_fixture("d-7")]
            );
            application.clear_cache().unwrap();
            assert_eq!(application.cache_statistics().cached_integrals(), 0);
            assert_eq!(application.cache_statistics().cached_coefficient_terms(), 0);
            assert_eq!(application.cache_statistics().cached_coefficient_bytes(), 0);
        }
    }
}

#[test]
fn cooperative_cancellation_at_root_collection_keeps_completed_root_reusable() {
    let routed = tadpole(Default::default());
    let c = routed.programs.context.family().coefficient_context();
    let mut application = routed.application();
    let cancel = AtomicBool::new(false);
    assert!(matches!(
        application.reduce_linear_combination_with_entry_admission_and_control(
            [(key([3]), c.one())],
            CandidateEntryAdmission::SavedGenerationScope,
            &cancel,
            Duration::ZERO,
            |snapshot| {
                if snapshot.phase == CandidateRoutedApplicationPhase::RootCollection {
                    cancel.store(true, Ordering::Relaxed);
                }
            },
        ),
        Err(CandidateRoutedError::Cancelled)
    ));
    let completed = application
        .reduce_linear_combination([(key([3]), c.one())])
        .unwrap();
    assert_eq!(completed.statistics().work.cache_hits(), 1);
    assert_eq!(completed.statistics().work.rule_applications(), 0);
    assert_eq!(completed.terms()[&key([1])], c.integer(-4));
}

#[test]
fn descendant_cache_does_not_bypass_saved_rank_for_a_new_entry() {
    let family = Arc::new(crate::solver::tests::sunset());
    let owners = programs(
        family.clone(),
        Some(0),
        vec![
            input(
                [true; 3],
                Some(0),
                vec![rule(&family, [2, 1, 1], &[([0, 3, 1], 2)])],
                &[],
            ),
            input(
                [false, true, true],
                Some(0),
                vec![rule(&family, [0, 3, 1], &[([-1, 1, 1], -3)])],
                &[[-1, 1, 1]],
            ),
        ],
        Default::default(),
    );
    let routed = RoutedCandidateReducer::try_new(owners, [], Default::default()).unwrap();
    let c = family.coefficient_context();
    let mut application = routed.application();
    let reduced = application
        .reduce_linear_combination([(key([2, 1, 1]), c.one())])
        .unwrap();
    assert_eq!(reduced.terms()[&key([-1, 1, 1])], c.integer(-6));
    assert!(matches!(
        application.reduce_linear_combination([(key([-1, 1, 1]), c.one())]),
        Err(CandidateRoutedError::Candidate(
            CandidateReductionError::OutsideNumeratorRank { .. }
        ))
    ));
}

fn weighted<const N: usize>() -> RoutedCandidateReducer<N> {
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
    let owner_sector = Mask::try_new([false, true, false]).unwrap();
    let transport = integral_transport::compile(
        &family,
        family.clone(),
        Arc::new(map),
        Mask::try_new([true, false, false]).unwrap(),
        owner_sector.clone(),
        Default::default(),
    )
    .unwrap();
    let pad = |p: [i16; 3]| crate::storage_array::<_, N>(&p, 0).unwrap();
    // The affine route has a pinched empty-support endpoint. Install its
    // actual zero certificate; an absent owner must never imply zero.
    let crate::sector::zero::Decision::ProvedZero(zero) =
        crate::sector::zero::Analyzer::try_unrestricted(&family)
            .unwrap()
            .analyze(&Mask::try_new([false; 3]).unwrap())
            .unwrap()
    else {
        panic!("empty vacuum support must have a zero certificate")
    };
    let context = Arc::new(
        crate::solver::CandidateOwnerContext::try_new(
            family.clone(),
            crate::solver::CandidateOwnerScope {
                max_numerator_rank: None,
                finite_case_policy: Default::default(),
            },
            vec![zero],
            Default::default(),
        )
        .unwrap(),
    );
    let owners = Arc::new(
        CandidateOwnerPrograms::try_new(
            context,
            [input(
                crate::storage_array(&[false, true, false], false).unwrap(),
                None,
                vec![
                    rule(&family, pad([-1, 1, 0]), &[(pad([0, 1, 0]), 2)]),
                    rule(&family, pad([0, 1, -1]), &[(pad([0, 1, 0]), -3)]),
                ],
                &[pad([0, 1, 0])],
            )],
        )
        .unwrap(),
    );
    RoutedCandidateReducer::try_new(
        owners,
        [CandidateOwnerRoute {
            owner_sector,
            transport: Arc::new(transport),
        }],
        Default::default(),
    )
    .unwrap()
}

#[test]
fn weighted_route_coefficients_and_signs_are_back_substituted_exactly() {
    let routed = weighted::<3>();
    let c = routed.programs.context.family().coefficient_context();
    let target = key([1, 0, -1]);
    let mapped = routed
        .routes
        .values()
        .next()
        .unwrap()
        .transport
        .transport(&target, Default::default())
        .unwrap();
    assert!(
        mapped
            .terms()
            .iter()
            .any(|term| term.coefficient() != &c.one())
    );
    let mut expected = c.zero();
    for term in mapped.terms() {
        let factor = if term.key() == &key([-1, 1, 0]) {
            2
        } else if term.key() == &key([0, 1, -1]) {
            -3
        } else if term.key() == &key([0, 0, 0]) {
            0
        } else {
            assert_eq!(term.key(), &key([0, 1, 0]));
            1
        };
        expected = c
            .try_add(
                &expected,
                &c.try_mul(term.coefficient(), &c.integer(factor), Default::default())
                    .unwrap(),
                Default::default(),
            )
            .unwrap();
    }
    let mut application = routed.application();
    let result = application
        .reduce_linear_combination([(target.clone(), c.one())])
        .unwrap();
    assert_eq!(
        result.terms(),
        &BTreeMap::from([(key([0, 1, 0]), expected.clone())])
    );
    assert_eq!(result.statistics().transport_calls, 1);
    assert_eq!(result.statistics().work.rule_applications(), 2);
    let negative = c.try_neg(&expected, Default::default()).unwrap();
    assert!(
        application
            .reduce_linear_combination([(target, c.one()), (key([0, 1, 0]), negative)])
            .unwrap()
            .is_zero()
    );
}

#[test]
fn ordered_routes_group_in_the_verified_common_owner_basis_before_ibps() {
    let mut routed = weighted::<3>();
    let programs = Arc::get_mut(&mut routed.programs).unwrap();
    let indexed = programs.context.coefficient_context().clone();
    let d = indexed
        .lift(&indexed.base().parameter("d").unwrap())
        .unwrap();
    let condition = indexed
        .add(
            &d,
            &indexed
                .sub(&indexed.index(1).unwrap(), &indexed.index(0).unwrap())
                .unwrap(),
        )
        .unwrap();
    Arc::get_mut(&mut programs.context)
        .unwrap()
        .shared
        .source_conditions = vec![
        indexed
            .admit_native_polynomial_result_with_limits(
                condition.raw().numerator.clone(),
                Default::default(),
            )
            .unwrap(),
    ];
    let c = routed.programs.context.family().coefficient_context();
    let target = key([1, 0, -1]);
    let mapped = routed
        .routes
        .values()
        .next()
        .unwrap()
        .transport
        .transport(&target, Default::default())
        .unwrap();
    let mut inputs = vec![(target, c.one())];
    for endpoint in mapped.terms() {
        inputs.push((
            endpoint.key().clone(),
            c.try_neg(endpoint.coefficient(), Default::default())
                .unwrap(),
        ));
    }
    let mut application = routed.application();
    application.set_reduction_strategy(crate::reduction::ReductionStrategy::Ordered);
    let result = application.reduce_linear_combination(inputs).unwrap();
    assert!(result.is_zero());
    assert!(result.unresolved().is_empty());
    assert_eq!(result.statistics().transport_calls, 1);
    assert_eq!(result.statistics().work.rule_applications(), 0);
    assert_eq!(result.statistics().work.cache_hits(), 0);
    // The empty-support endpoint canceled before expansion, but its source
    // validation was already incurred by the checked transport.
    assert!(
        result
            .nonzero_conditions()
            .contains(&c.parameter("d").unwrap())
    );
}

#[test]
fn ordered_canceled_transport_endpoint_keeps_its_incurred_source_condition() {
    let mut routed = weighted::<3>();
    let programs = Arc::get_mut(&mut routed.programs).unwrap();
    let indexed = programs.context.coefficient_context().clone();
    let d = indexed
        .lift(&indexed.base().parameter("d").unwrap())
        .unwrap();
    let condition = indexed
        .add(
            &d,
            &indexed
                .sub(&indexed.index(1).unwrap(), &indexed.index(0).unwrap())
                .unwrap(),
        )
        .unwrap();
    Arc::get_mut(&mut programs.context)
        .unwrap()
        .shared
        .source_conditions = vec![
        indexed
            .admit_native_polynomial_result_with_limits(
                condition.raw().numerator.clone(),
                Default::default(),
            )
            .unwrap(),
    ];
    let c = routed.programs.context.family().coefficient_context();
    let first = key([1, 0, -1]);
    let second = key([1, 0, -2]);
    let zero = key([0, 0, 0]);
    let transport = &routed.routes.values().next().unwrap().transport;
    let a = transport.transport(&first, Default::default()).unwrap();
    let b = transport.transport(&second, Default::default()).unwrap();
    let weight_a = a
        .terms()
        .iter()
        .find(|term| term.key() == &zero)
        .unwrap()
        .coefficient();
    let weight_b = b
        .terms()
        .iter()
        .find(|term| term.key() == &zero)
        .unwrap()
        .coefficient();
    let inputs = [
        (first, weight_b.clone()),
        (second, c.try_neg(weight_a, Default::default()).unwrap()),
    ];
    let mut application = routed.application();
    application
        .set_unresolved_policy(UnresolvedPolicy::Preserve)
        .unwrap();
    application.set_reduction_strategy(crate::reduction::ReductionStrategy::Ordered);
    let mut observed_zero = false;
    let mut callback = |event: &CandidateRoutedApplicationEnvelope<'_>| {
        if let Some(CandidateRoutedWorkRef::Route(target)) = event.current {
            observed_zero |= target == &zero;
        }
        Ok(CandidateRoutedObserverControl::Continue)
    };
    let mut observer = CandidateRoutedApplicationObserver::new(
        CandidateRoutedObservationOptions {
            interval: Duration::ZERO,
            ..Default::default()
        },
        &mut callback,
    );
    let result = application
        .reduce_linear_combination_with_observation(
            inputs,
            CandidateEntryAdmission::SavedGenerationScope,
            &mut observer,
            None,
        )
        .unwrap();
    assert_eq!(result.statistics().transport_calls, 2);
    // Both original inputs have guard d-1. No original input or surviving
    // nonzero-support endpoint carries d; it belongs to the canceled child.
    assert!(
        result
            .nonzero_conditions()
            .contains(&c.parameter("d").unwrap())
    );
    assert!(!observed_zero);
}

#[cfg(feature = "capacity-dispatch")]
#[test]
fn padded_storage_preserves_physical_arity_and_exact_outputs() {
    let exact = weighted::<3>();
    let padded = weighted::<4>();
    let c = exact.programs.context.family().coefficient_context();
    let expected = exact
        .application()
        .reduce_linear_combination([(key([1, 0, -1]), c.one())])
        .unwrap();
    let actual = padded
        .application()
        .reduce_linear_combination([(key([1, 0, -1]), c.one())])
        .unwrap();
    assert_eq!(actual.terms(), expected.terms());
    assert!(actual.terms().keys().all(|key| key.powers().len() == 3));
}

fn polynomial(context: &IndexedCoefficientContext, text: &str) -> IndexedPolynomial {
    let value = match text {
        "n0-2" => context
            .sub(&context.index(0).unwrap(), &context.integer(2))
            .unwrap(),
        "n0-1" => context
            .sub(&context.index(0).unwrap(), &context.integer(1))
            .unwrap(),
        _ => context
            .lift(&context.base().coefficient_fixture(text))
            .unwrap(),
    };
    context
        .admit_native_polynomial_result_with_limits(
            value.raw().numerator.clone(),
            Default::default(),
        )
        .unwrap()
}

#[test]
fn original_poles_and_exception_witnesses_survive_zero_results_and_cache_hits() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let mut owners = CandidateOwnerPrograms::try_new(
        context(family.clone(), None, Default::default()),
        [input(
            [true],
            None,
            vec![rule(&family, [2], &[([1], 1), ([1], -1)])],
            &[[1]],
        )],
    )
    .unwrap();
    let indexed = owners.context.coefficient_context().clone();
    let owner = Arc::get_mut(owners.owners.get_mut(&[true]).unwrap()).unwrap();
    let prepared = &mut Arc::get_mut(&mut owner.batches[0]).unwrap().rules[0];
    prepared.rhs[0].denominator = polynomial(&indexed, "d-4");
    prepared.exceptions = vec![vec![
        polynomial(&indexed, "d-5"),
        polynomial(&indexed, "d-6"),
    ]];
    let routed = RoutedCandidateReducer::try_new(Arc::new(owners), [], Default::default()).unwrap();
    let mut application = routed.application();
    let c = family.coefficient_context();
    for _ in 0..2 {
        let result = application
            .reduce_linear_combination([(key([2]), c.one())])
            .unwrap();
        assert!(result.is_zero());
        assert!(
            result
                .nonzero_conditions()
                .contains(&c.coefficient_fixture("d-4"))
        );
        assert!(
            result
                .nonzero_conditions()
                .contains(&c.coefficient_fixture("d-5"))
        );
        assert!(
            !result
                .nonzero_conditions()
                .contains(&c.coefficient_fixture("d-6"))
        );
    }
    assert!(application.cache_statistics().cached_coefficient_terms() > 0);
}

#[test]
fn vanished_original_denominator_precedes_local_rhs_cancellation() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let mut owners = CandidateOwnerPrograms::try_new(
        context(family.clone(), None, Default::default()),
        [input(
            [true],
            None,
            vec![rule(&family, [2], &[([1], 1), ([1], -1)])],
            &[[1]],
        )],
    )
    .unwrap();
    let indexed = owners.context.coefficient_context().clone();
    let owner = Arc::get_mut(owners.owners.get_mut(&[true]).unwrap()).unwrap();
    Arc::get_mut(&mut owner.batches[0]).unwrap().rules[0].rhs[0].denominator =
        polynomial(&indexed, "n0-2");
    let mut application = RoutedCandidateReducer::try_new(Arc::new(owners), [], Default::default())
        .unwrap()
        .application();
    assert!(matches!(
        application.reduce_linear_combination([(key([2]), family.coefficient_context().one())]),
        Err(CandidateRoutedError::Candidate(
            CandidateReductionError::Uncovered { .. }
        ))
    ));
    assert_eq!(application.cache_statistics().cached_integrals(), 0);
}

#[test]
fn missing_owner_missing_rule_and_non_descending_edge_fail_closed() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let c = family.coefficient_context();
    let sunset = Arc::new(crate::solver::tests::sunset());
    let owners = programs(
        sunset.clone(),
        None,
        vec![input([true; 3], None, vec![], &[[1, 1, 1]])],
        Default::default(),
    );
    let mut application = RoutedCandidateReducer::try_new(owners, [], Default::default())
        .unwrap()
        .application();
    assert!(matches!(
        application
            .reduce_linear_combination([(key([1, 1, 0]), sunset.coefficient_context().one())]),
        Err(CandidateRoutedError::MissingOwner { .. })
    ));
    let mut application = tadpole(Default::default()).application();
    assert!(matches!(
        application.reduce_linear_combination([(key([4]), c.one())]),
        Err(CandidateRoutedError::Candidate(
            CandidateReductionError::Uncovered { .. }
        ))
    ));
    let owners = programs(
        family.clone(),
        None,
        vec![input(
            [true],
            None,
            vec![rule(&family, [2], &[([2], 1)])],
            &[],
        )],
        Default::default(),
    );
    let mut application = RoutedCandidateReducer::try_new(owners, [], Default::default())
        .unwrap()
        .application();
    assert!(matches!(
        application.reduce_linear_combination([(key([2]), c.one())]),
        Err(CandidateRoutedError::Candidate(
            CandidateReductionError::NonDescending { .. }
        ))
    ));
}

#[test]
fn normal_reduction_ignores_work_count_diagnostics_but_records_work() {
    let routed = tadpole(ReductionLimits {
        max_rule_applications: 0,
        max_cached_integrals: 0,
        max_cached_coefficient_terms: 0,
        max_pending_frames: 0,
        max_coalescing_additions: 0,
        max_factorization_terms: 0,
        ..Default::default()
    });
    let c = routed.programs.context.family().coefficient_context();
    let mut application = routed.application();
    let actual = application
        .reduce_linear_combination([(key([3]), c.one())])
        .unwrap();
    assert_eq!(actual.terms(), &BTreeMap::from([(key([1]), c.integer(-4))]));
    assert_eq!(actual.statistics().work.rule_applications(), 2);
    assert!(actual.statistics().work.coalescing_additions() > 0);
    assert!(application.cache_statistics().cached_integrals() > 0);
}

#[test]
fn ordinary_routing_can_cross_the_previous_transport_count_ceiling() {
    let routed = weighted::<3>();
    let mut application = routed.application();
    let mut statistics = CandidateRoutedApplicationStatistics {
        transport_calls: 100_000,
        ..Default::default()
    };
    let actual = application
        .route_entry_in_request(&key([1, 0, -1]), &mut statistics, UnresolvedPolicy::Reject)
        .unwrap();
    assert_eq!(statistics.transport_calls, 100_001);
    assert_eq!(actual.terms().len(), 3);
    application.limits.diagnostics_enabled = true;
    statistics.transport_calls = 100_000;
    assert!(matches!(
        application.route_entry_in_request(
            &key([1, 0, -1]),
            &mut statistics,
            UnresolvedPolicy::Reject,
        ),
        Err(CandidateRoutedError::ResourceLimit {
            resource: "transport calls",
            requested: 100_001,
            limit: 100_000,
        })
    ));
}

#[test]
fn scoped_elapsed_budget_stops_a_cached_request_and_releases_the_scope() {
    use symbolica::utils::resources::{ResourceBudget, ResourceLimits};
    let routed = tadpole(Default::default());
    let c = routed.programs.context.family().coefficient_context();
    let mut application = routed.application();
    let expected = application
        .reduce_linear_combination([(key([3]), c.one())])
        .unwrap();
    let budget = ResourceBudget::new(ResourceLimits {
        max_elapsed: Some(Duration::ZERO),
        ..Default::default()
    });
    let error = budget
        .run(|| application.reduce_linear_combination([(key([3]), c.one())]))
        .unwrap_err();
    let mut cause: &(dyn std::error::Error + 'static) = &error;
    loop {
        if let Some(resource) =
            cause.downcast_ref::<symbolica::utils::resources::ResourceLimitError>()
        {
            assert!(matches!(
                resource,
                symbolica::utils::resources::ResourceLimitError::Elapsed { .. }
            ));
            break;
        }
        cause = cause
            .source()
            .expect("typed resource cause must survive native wrappers");
    }
    let actual = application
        .reduce_linear_combination([(key([3]), c.one())])
        .unwrap();
    assert_eq!(actual.terms(), expected.terms());
    assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
}

#[test]
fn scoped_ram_headroom_failure_keeps_the_original_typed_cause() {
    use symbolica::utils::resources::{ResourceBudget, ResourceLimitError, ResourceLimits};
    let routed = tadpole(Default::default());
    let c = routed.programs.context.family().coefficient_context();
    let mut application = routed.application();
    // RSS is below the limit; an imminent native allocation consumes headroom.
    // Rechecking RSS after unwinding would lose this original failure.
    let budget = ResourceBudget::with_memory_monitor(
        ResourceLimits {
            max_process_rss_bytes: Some(1),
            ..Default::default()
        },
        || Ok(0),
    );
    let error = budget
        .run(|| application.reduce_linear_combination([(key([3]), c.one())]))
        .unwrap_err();
    let mut cause: &(dyn std::error::Error + 'static) = &error;
    loop {
        if let Some(resource) = cause.downcast_ref::<ResourceLimitError>() {
            assert!(
                matches!(resource, ResourceLimitError::Memory { rss_bytes: 0, additional_bytes, limit_bytes: 1 } if *additional_bytes > 1)
            );
            break;
        }
        cause = cause
            .source()
            .expect("typed allocation headroom failure must survive wrappers");
    }
    assert_eq!(application.cache_statistics().cached_integrals(), 0);
    assert!(
        application
            .reduce_linear_combination([(key([3]), c.one())])
            .is_ok()
    );
}

#[test]
fn shared_request_and_retained_cache_limits_are_enforced() {
    for limits in [
        ReductionLimits {
            max_rule_applications: 1,
            ..Default::default()
        },
        ReductionLimits {
            max_cached_integrals: 1,
            ..Default::default()
        },
        ReductionLimits {
            max_cached_coefficient_terms: 1,
            ..Default::default()
        },
        ReductionLimits {
            max_cached_coefficient_bytes: 1,
            ..Default::default()
        },
        ReductionLimits {
            max_pending_frames: 1,
            ..Default::default()
        },
        ReductionLimits {
            max_coalescing_additions: 0,
            ..Default::default()
        },
    ] {
        let routed = tadpole(limits.with_diagnostics());
        let c = routed.programs.context.family().coefficient_context();
        assert!(
            routed
                .application()
                .reduce_linear_combination([(key([3]), c.one())])
                .is_err()
        );
    }
}

#[test]
fn source_conditions_are_checked_on_children_and_retained_at_terminals() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let c = family.coefficient_context();
    for condition in ["n0-1", "d-7"] {
        let mut owners = CandidateOwnerPrograms::try_new(
            context(family.clone(), None, Default::default()),
            [input(
                [true],
                None,
                vec![rule(&family, [2], &[([1], 2)])],
                &[[1]],
            )],
        )
        .unwrap();
        let indexed = owners.context.coefficient_context().clone();
        Arc::get_mut(&mut owners.context)
            .unwrap()
            .shared
            .source_conditions = vec![polynomial(&indexed, condition)];
        let mut application =
            RoutedCandidateReducer::try_new(Arc::new(owners), [], Default::default())
                .unwrap()
                .application();
        if condition == "n0-1" {
            assert!(
                matches!(application.reduce_linear_combination([(key([2]), c.one())]),
                Err(CandidateRoutedError::Candidate(CandidateReductionError::SourceConditionVanished { target, .. })) if target == key([1]))
            );
            assert!(
                application
                    .reduce_linear_combination([(key([1]), c.zero())])
                    .is_err()
            );
        } else {
            for target in [key([2]), key([2]), key([1])] {
                let result = application
                    .reduce_linear_combination([(target, c.one())])
                    .unwrap();
                assert_eq!(result.nonzero_conditions(), &[c.coefficient_fixture("d-7")]);
            }
        }
    }
}

#[test]
fn input_parameter_denominators_survive_exact_cancellation() {
    let routed = tadpole(Default::default());
    let c = routed.programs.context.family().coefficient_context();
    let mut application = routed.application();
    let result = application
        .reduce_linear_combination([
            (key([3]), c.coefficient_fixture("1/(d-8)")),
            (key([3]), c.coefficient_fixture("-1/(d-8)")),
        ])
        .unwrap();
    assert!(result.is_zero());
    assert_eq!(result.nonzero_conditions(), &[c.coefficient_fixture("d-8")]);
    assert_eq!(result.statistics().work.rule_applications(), 0);
    assert_eq!(application.cache_statistics().cached_integrals(), 0);
}

#[test]
fn selected_alternative_retains_baseline_and_selected_guards_only() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let mut alternative = rule(&family, [3], &[([1], -5)]);
    alternative.dispatch_policy =
        crate::solver::RuleDispatchPolicy::AfterBaselinePartitionWholePiece;
    let mut owners = CandidateOwnerPrograms::try_new(
        context(family.clone(), None, Default::default()),
        [input(
            [true],
            None,
            vec![rule(&family, [3], &[([1], 2)]), alternative],
            &[[1]],
        )],
    )
    .unwrap();
    let indexed = owners.context.coefficient_context().clone();
    let owner = Arc::get_mut(owners.owners.get_mut(&[true]).unwrap()).unwrap();
    let batch = Arc::get_mut(&mut owner.batches[0]).unwrap();
    batch.rules[0].rhs[0].denominator = polynomial(&indexed, "d-4");
    batch.rules[1].rhs[0].denominator = polynomial(&indexed, "d-5");
    let mut application = RoutedCandidateReducer::try_new(Arc::new(owners), [], Default::default())
        .unwrap()
        .application();
    let c = family.coefficient_context();
    let result = application
        .reduce_linear_combination([(key([3]), c.one())])
        .unwrap();
    assert_eq!(result.terms()[&key([1])], c.integer(-5));
    assert!(
        result
            .nonzero_conditions()
            .contains(&c.coefficient_fixture("d-4"))
    );
    assert!(
        result
            .nonzero_conditions()
            .contains(&c.coefficient_fixture("d-5"))
    );
}

#[test]
fn entry_transport_is_exact_separate_from_admission_rules_and_cache() {
    let routed = weighted::<3>();
    let target = key([1, 0, -1]);
    let expected = routed
        .routes
        .values()
        .next()
        .unwrap()
        .transport
        .transport(&target, Default::default())
        .unwrap();
    let application = routed.application();
    let actual = application.route_entry_to_owner(&target).unwrap();
    assert_eq!(
        actual.terms(),
        &expected
            .terms()
            .iter()
            .filter(|term| term.key() != &key([0, 0, 0]))
            .map(|term| (term.key().clone(), term.coefficient().clone()))
            .collect()
    );
    assert_eq!(actual.statistics().work.rule_applications(), 0);
    assert_eq!(actual.statistics().transport_calls, 1);
    assert_eq!(expected.terms().len(), 4);
    assert_eq!(actual.terms().len(), 3);
    assert_eq!(actual.statistics().operational_nodes, 5);
    assert_eq!(application.cache_statistics().cached_integrals(), 0);
    let literal = application.route_entry_to_owner(&key([-1, 1, 0])).unwrap();
    assert_eq!(
        literal.terms(),
        &BTreeMap::from([(
            key([-1, 1, 0]),
            application.family().coefficient_context().one()
        )])
    );
    assert_eq!(literal.statistics().transport_calls, 0);
    assert!(matches!(
        application.route_entry_to_owner(&key([1, 1, 1])),
        Err(CandidateRoutedError::MissingOwner { .. })
    ));
}

#[test]
fn routed_batch_admits_each_endpoint_before_cancellation_and_shares_caps() {
    let mut routed = weighted::<3>();
    let c = routed
        .programs
        .context
        .family()
        .coefficient_context()
        .clone();
    let images = routed
        .application()
        .route_entry_to_owner(&key([1, 0, -1]))
        .unwrap();
    let regions = images
        .terms()
        .keys()
        .map(|key| {
            let lower = key
                .powers()
                .iter()
                .map(|&n| {
                    if n > 0 {
                        n as u64 - 1
                    } else {
                        n.unsigned_abs()
                    }
                })
                .collect::<Vec<_>>();
            RootRegionInput {
                support: std::array::from_fn(|i| key.powers()[i] > 0),
                upper: lower.iter().copied().map(Some).collect(),
                lower,
                rank: None,
                powers: Default::default(),
            }
        })
        .collect::<Vec<_>>();
    let admission = FiniteRootAdmission::try_new(regions.clone(), 4).unwrap();
    assert!(admission.validate_entry(&key([0, 0, 0])).is_err());
    assert_eq!(regions.len(), 3);
    let mut application = routed.application();
    let result = application
        .reduce_routed_linear_combination_with_entry_admission(
            [(key([1, 0, -1]), c.one())],
            CandidateEntryAdmission::ExplicitFinite(&admission),
        )
        .unwrap();
    assert_eq!(result.terms()[&key([0, 1, 0])], c.integer(-6));
    assert_eq!(
        result.terms(),
        routed
            .application()
            .reduce_linear_combination([(key([1, 0, -1]), c.one())])
            .unwrap()
            .terms()
    );
    assert_eq!(result.statistics().transport_calls, 1);
    assert_eq!(result.statistics().input_terms, 1);
    // Remove exactly one genuine endpoint. A zero caller coefficient must
    // still be rejected before cancellation can hide its missing admission.
    let narrow = FiniteRootAdmission::try_new(regions[..2].iter().cloned(), 2).unwrap();
    assert!(matches!(
        application.reduce_routed_linear_combination_with_entry_admission(
            [(key([1, 0, -1]), c.zero())],
            CandidateEntryAdmission::ExplicitFinite(&narrow)
        ),
        Err(CandidateRoutedError::EntryAdmission(_))
    ));
    routed.limits.diagnostics_enabled = true;
    routed.limits.max_transport_calls = 1;
    assert!(matches!(
        routed
            .application()
            .reduce_routed_linear_combination_with_entry_admission(
                [(key([1, 0, -1]), c.one()), (key([1, 0, -1]), c.integer(-1))],
                CandidateEntryAdmission::ExplicitFinite(&admission)
            ),
        Err(CandidateRoutedError::ResourceLimit {
            resource: "transport calls",
            ..
        })
    ));
}

#[test]
fn omitted_certified_zero_entry_images_keep_their_source_guards() {
    let mut routed = weighted::<3>();
    let programs = Arc::get_mut(&mut routed.programs).unwrap();
    let indexed = programs.context.coefficient_context().clone();
    let d = indexed
        .lift(&indexed.base().parameter("d").unwrap())
        .unwrap();
    let condition = indexed
        .add(
            &d,
            &indexed
                .sub(&indexed.index(1).unwrap(), &indexed.index(0).unwrap())
                .unwrap(),
        )
        .unwrap();
    Arc::get_mut(&mut programs.context)
        .unwrap()
        .shared
        .source_conditions = vec![
        indexed
            .admit_native_polynomial_result_with_limits(
                condition.raw().numerator.clone(),
                Default::default(),
            )
            .unwrap(),
    ];
    let result = routed
        .application()
        .route_entry_to_owner(&key([1, 0, -1]))
        .unwrap();
    assert!(!result.terms().contains_key(&key([0, 0, 0])));
    // d is the guard at the omitted zero; the source and retained endpoints
    // instead specialize to d-1, d+1 or d+2.
    assert!(
        result.nonzero_conditions().contains(
            &routed
                .programs
                .context
                .family()
                .coefficient_context()
                .parameter("d")
                .unwrap()
        )
    );
}

#[test]
fn certified_zero_source_needs_no_owner_route_or_cache() {
    let routed = weighted::<3>();
    let application = routed.application();
    for target in [key([0, 0, 0]), key([-1, 0, -2])] {
        let result = application.route_entry_to_owner(&target).unwrap();
        assert!(result.is_zero());
        assert_eq!(result.statistics().requested_targets, 1);
        assert_eq!(result.statistics().input_terms, 1);
        assert_eq!(result.statistics().operational_nodes, 1);
        assert_eq!(result.statistics().transport_calls, 0);
        assert_eq!(result.statistics().work.rule_applications(), 0);
        assert_eq!(application.cache_statistics().cached_integrals(), 0);
    }
    let mut uncertified = weighted::<3>();
    let programs = Arc::get_mut(&mut uncertified.programs).unwrap();
    Arc::get_mut(&mut programs.context)
        .unwrap()
        .shared
        .zero_sectors
        .clear();
    assert!(matches!(
        uncertified
            .application()
            .route_entry_to_owner(&key([0, 0, 0])),
        Err(CandidateRoutedError::MissingOwner { .. })
    ));
}

#[test]
fn certified_zero_source_keeps_guards_through_routed_reduction() {
    for vanished in [false, true] {
        let mut routed = weighted::<3>();
        let programs = Arc::get_mut(&mut routed.programs).unwrap();
        let indexed = programs.context.coefficient_context().clone();
        let d = indexed.base().parameter("d").unwrap();
        let condition = if vanished {
            indexed.index(0).unwrap()
        } else {
            indexed.lift(&d).unwrap()
        };
        Arc::get_mut(&mut programs.context)
            .unwrap()
            .shared
            .source_conditions = vec![
            indexed
                .admit_native_polynomial_result_with_limits(
                    condition.raw().numerator.clone(),
                    Default::default(),
                )
                .unwrap(),
        ];
        let mut application = routed.application();
        let target = key([0, 0, 0]);
        if vanished {
            assert!(matches!(
                application.route_entry_to_owner(&target),
                Err(CandidateRoutedError::Candidate(
                    CandidateReductionError::SourceConditionVanished { .. }
                ))
            ));
        } else {
            let result = application.route_entry_to_owner(&target).unwrap();
            assert!(result.is_zero());
            assert_eq!(result.nonzero_conditions(), &[d.clone()]);
            let result = application
                .reduce_routed_linear_combination_with_entry_admission(
                    [(target, indexed.base().one())],
                    CandidateEntryAdmission::SavedGenerationScope,
                )
                .unwrap();
            assert!(result.is_zero());
            assert_eq!(result.nonzero_conditions(), &[d]);
            assert_eq!(result.statistics().transport_calls, 0);
            assert_eq!(result.statistics().work.rule_applications(), 0);
            assert_eq!(application.cache_statistics().cached_integrals(), 0);
        }
    }
}

#[test]
fn certified_zero_source_does_not_bypass_arity_or_resource_limits() {
    let routed = weighted::<3>();
    let mut application = routed.application();
    assert!(matches!(
        application.route_entry_to_owner(&key([0, 0])),
        Err(CandidateRoutedError::Candidate(
            CandidateReductionError::Application(ReductionError::WrongArity { .. })
        ))
    ));
    application.limits.diagnostics_enabled = true;
    application.limits.max_input_targets = 0;
    assert!(matches!(
        application.route_entry_to_owner(&key([0, 0, 0])),
        Err(CandidateRoutedError::ResourceLimit {
            resource: "input targets",
            requested: 1,
            limit: 0
        })
    ));
    application.limits.max_input_targets = 1;
    application.limits.max_unique_nodes = 0;
    assert!(matches!(
        application.route_entry_to_owner(&key([0, 0, 0])),
        Err(CandidateRoutedError::ResourceLimit {
            resource: "operational nodes",
            requested: 1,
            limit: 0
        })
    ));
}

#[test]
fn factorized_routed_values_match_sparse_cold_warm_and_guarded_cancellation() {
    use crate::solver::CandidateCacheRepresentation;
    let family = Arc::new(crate::solver::tests::tadpole());
    let c = family.coefficient_context();
    let indexed = crate::identity::ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let mut three = rule(&family, [3], &[([2], 1), ([1], 1)]);
    three.candidate.rhs[0].coefficient = indexed
        .lift(&c.coefficient_fixture("(d-3)/(d-4)"))
        .unwrap()
        .raw()
        .clone();
    three.candidate.rhs[1].coefficient = indexed
        .lift(&c.coefficient_fixture("-2/(d-4)"))
        .unwrap()
        .raw()
        .clone();
    let mut two = rule(&family, [2], &[([1], 1)]);
    two.candidate.rhs[0].coefficient = indexed
        .lift(&c.coefficient_fixture("2/(d-3)"))
        .unwrap()
        .raw()
        .clone();
    let mut four = rule(&family, [4], &[([2], 1)]);
    four.candidate.rhs[0].coefficient = indexed
        .lift(&c.coefficient_fixture("(d-3)/(d-4)"))
        .unwrap()
        .raw()
        .clone();
    let owners = programs(
        family.clone(),
        None,
        vec![input([true], None, vec![four, three, two], &[[1]])],
        Default::default(),
    );
    let routed = RoutedCandidateReducer::try_new(owners, [], Default::default()).unwrap();
    let mut sparse = routed.application();
    let mut factorized = routed.application();
    assert_eq!(
        factorized.cache_representation(),
        CandidateCacheRepresentation::Sparse
    );
    factorized
        .set_cache_representation(CandidateCacheRepresentation::Factorized)
        .unwrap();
    for target in [key([4]), key([3]), key([2])] {
        let expected = sparse
            .reduce_linear_combination([(target.clone(), c.one())])
            .unwrap();
        for _ in 0..2 {
            let actual = factorized
                .reduce_linear_combination([(target.clone(), c.one())])
                .unwrap();
            assert_eq!(actual.terms(), expected.terms());
            assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
        }
    }
    let canceled = factorized
        .reduce_linear_combination([(key([3]), c.one())])
        .unwrap();
    assert!(canceled.is_zero());
    assert!(
        canceled
            .nonzero_conditions()
            .contains(&c.coefficient_fixture("d-3"))
    );
    assert!(
        canceled
            .nonzero_conditions()
            .contains(&c.coefficient_fixture("d-4"))
    );
    assert_eq!(canceled.statistics().work.rule_applications(), 0);
    assert_eq!(canceled.statistics().work.cache_hits(), 1);
    let terms = [
        (key([4]), c.coefficient_fixture("(d-4)/(d-6)")),
        (key([2]), c.coefficient_fixture("-(d-3)/(d-6)")),
    ];
    let expected = sparse.reduce_linear_combination(terms.clone()).unwrap();
    let actual = factorized.reduce_linear_combination(terms).unwrap();
    assert!(actual.is_zero());
    assert_eq!(actual.terms(), expected.terms());
    assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
    assert!(
        factorized
            .set_cache_representation(CandidateCacheRepresentation::Sparse)
            .is_err()
    );
    factorized.clear_cache().unwrap();
    assert_eq!(factorized.cache_statistics().cached_integrals(), 0);
    assert_eq!(factorized.cache_statistics().cached_coefficient_terms(), 0);
    assert_eq!(factorized.cache_statistics().cached_coefficient_bytes(), 0);
    factorized
        .set_cache_representation(CandidateCacheRepresentation::Sparse)
        .unwrap();
    assert_eq!(
        factorized
            .reduce_linear_combination([(key([4]), c.one())])
            .unwrap()
            .terms()[&key([1])],
        c.coefficient_fixture("2/(d-4)")
    );
}

#[test]
fn factorized_weighted_routes_and_cache_limits_reuse_existing_primitives() {
    use crate::solver::CandidateCacheRepresentation;
    let routed = weighted::<3>();
    let c = routed.programs.context.family().coefficient_context();
    let mut sparse = routed.application();
    let mut factorized = routed.application();
    factorized
        .set_cache_representation(CandidateCacheRepresentation::Factorized)
        .unwrap();
    for _ in 0..2 {
        let expected = sparse
            .reduce_linear_combination([(key([1, 0, -1]), c.one())])
            .unwrap();
        let actual = factorized
            .reduce_linear_combination([(key([1, 0, -1]), c.one())])
            .unwrap();
        assert_eq!(actual.terms(), expected.terms());
        assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
    }
    let bounded = tadpole(ReductionLimits {
        max_cached_coefficient_bytes: 1,
        ..Default::default()
    });
    let mut bounded = bounded.application();
    bounded
        .set_cache_representation(CandidateCacheRepresentation::Factorized)
        .unwrap();
    assert!(matches!(
        bounded
            .reduce_linear_combination([(key([3]), bounded.family().coefficient_context().one())]),
        Err(CandidateRoutedError::Candidate(
            CandidateReductionError::Application(ReductionError::CacheCoefficientByteLimit { .. })
        ))
    ));
    bounded.clear_cache().unwrap();
    assert_eq!(bounded.cache_statistics().cached_coefficient_bytes(), 0);
}
