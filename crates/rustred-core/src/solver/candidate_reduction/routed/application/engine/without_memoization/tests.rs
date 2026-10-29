use super::*;
use crate::algebra::IndexedCoefficientContext;
use crate::persistence::application_cache::{
    ApplicationCacheBinding, ApplicationCacheKey, ApplicationCacheRecord,
};
use crate::reduction::{GuardedCacheBacking, GuardedCacheBackingError, GuardedIntermediateCapture};
use crate::solver::candidate_reduction::owner_test_support::*;
use crate::solver::{
    CandidateOwnerPrograms, CandidateRoutedObservationOptions, CandidateRoutedObserverControl,
    RoutedCandidateLimits, RoutedCandidateReducer,
};

fn fixture() -> RoutedCandidateReducer<1> {
    fixture_with_routed_limits(Default::default())
}

fn fixture_with_routed_limits(limits: RoutedCandidateLimits) -> RoutedCandidateReducer<1> {
    let family = Arc::new(crate::solver::tests::tadpole());
    let mut programs = CandidateOwnerPrograms::try_new(
        context(family.clone(), None, Default::default()),
        [input(
            [true],
            None,
            vec![
                rule(&family, [4], &[([3], 1), ([2], 1)]),
                rule(&family, [3], &[([2], 2)]),
                rule(&family, [2], &[([1], 3)]),
            ],
            &[[1]],
        )],
    )
    .unwrap();
    let indexed: IndexedCoefficientContext = programs.context.coefficient_context().clone();
    let source = indexed
        .lift(&indexed.base().coefficient_fixture("d-7"))
        .unwrap();
    let source = indexed
        .admit_native_polynomial_result_with_limits(
            source.raw().numerator.clone(),
            Default::default(),
        )
        .unwrap();
    Arc::get_mut(&mut programs.context)
        .unwrap()
        .shared
        .source_conditions = vec![source];
    RoutedCandidateReducer::try_new(Arc::new(programs), [], limits).unwrap()
}

#[test]
fn unique_node_diagnostic_is_not_reinterpreted_as_an_occurrence_limit() {
    let source = fixture_with_routed_limits(RoutedCandidateLimits {
        diagnostics_enabled: true,
        max_unique_nodes: 1,
        ..Default::default()
    });
    let c = source.programs.context.family().coefficient_context();
    let mut application = source.application();
    application.set_reduction_strategy(ReductionStrategy::DepthFirstWithoutMemoization);
    let result = application
        .reduce_linear_combination([(key([4]), c.one())])
        .unwrap();
    assert_eq!(result.statistics().work.rule_applications(), 4);
    assert!(result.statistics().operational_nodes > 1);
    assert!(application.cache.is_empty());
}

#[test]
fn repeated_diamond_recomputes_each_occurrence_and_preserves_the_dfs_cache() {
    let source = fixture();
    let c = source.programs.context.family().coefficient_context();
    let mut application = source.application();
    let expected = application
        .reduce_linear_combination([(key([4]), c.one())])
        .unwrap();
    assert_eq!(expected.statistics().work.rule_applications(), 3);
    let retained_keys: Vec<_> = application.cache.keys().cloned().collect();
    let retained_weight = application.cache_weight;
    let cache_work = application.cache_work_statistics();
    application.set_reduction_strategy(ReductionStrategy::DepthFirstWithoutMemoization);
    for _ in 0..2 {
        let result = application
            .reduce_linear_combination([(key([4]), c.one())])
            .unwrap();
        assert_eq!(result.terms(), expected.terms());
        assert_eq!(result.nonzero_conditions(), expected.nonzero_conditions());
        assert_eq!(result.statistics().work.rule_applications(), 4);
        assert_eq!(result.statistics().work.cache_hits(), 0);
        assert_eq!(result.statistics().cache, Default::default());
        assert_eq!(result.statistics().persistent_cache, Default::default());
        assert_eq!(
            application.cache.keys().cloned().collect::<Vec<_>>(),
            retained_keys
        );
        assert_eq!(application.cache_weight, retained_weight);
        assert_eq!(application.cache_work_statistics(), cache_work);
    }
    application.set_reduction_strategy(ReductionStrategy::DepthFirst);
    let warm = application
        .reduce_linear_combination([(key([4]), c.one())])
        .unwrap();
    assert_eq!(warm.statistics().work.rule_applications(), 0);
    assert!(warm.statistics().work.cache_hits() > 0);
}

#[test]
fn original_input_validation_and_partial_result_semantics_are_preserved() {
    let source = fixture();
    let c = source.programs.context.family().coefficient_context();
    let mut application = source.application();
    application.set_reduction_strategy(ReductionStrategy::DepthFirstWithoutMemoization);
    assert!(
        application
            .reduce_linear_combination([(key([1, 0]), c.zero())])
            .is_err()
    );
    assert!(
        application
            .reduce_linear_combination([(key([5]), c.one())])
            .is_err()
    );
    application
        .set_unresolved_policy(UnresolvedPolicy::Preserve)
        .unwrap();
    let preserved = application
        .reduce_linear_combination([(key([5]), c.one())])
        .unwrap();
    assert_eq!(preserved.terms(), &BTreeMap::from([(key([5]), c.one())]));
    assert_eq!(preserved.unresolved(), &BTreeSet::from([key([5])]));
    assert_eq!(
        preserved.nonzero_conditions(),
        &[c.coefficient_fixture("d-7")]
    );
    assert!(application.cache.is_empty());
}

#[test]
fn observer_never_reports_an_ordered_phase_or_completed_cache_census() {
    let source = fixture();
    let c = source.programs.context.family().coefficient_context();
    let mut application = source.application();
    application
        .reduce_linear_combination([(key([4]), c.one())])
        .unwrap();
    application.set_reduction_strategy(ReductionStrategy::DepthFirstWithoutMemoization);
    let mut snapshots = Vec::new();
    let mut callback = |event: &CandidateRoutedApplicationEnvelope<'_>| {
        assert!(event.ordered.is_none());
        assert!(event.frontier.is_none());
        assert_eq!(event.metrics.completed_unique_operational_nodes, 0);
        if let Some(snapshot) = event.snapshot {
            assert_eq!(snapshot.cached.entries, 0);
            assert!(snapshot.pinned.is_none());
            assert!(snapshot.eligible.is_none());
            assert_eq!(snapshot.statistics.work.cache_hits(), 0);
            snapshots.push(snapshot);
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
    let actual = application
        .reduce_linear_combination_with_observation(
            [(key([4]), c.one())],
            CandidateEntryAdmission::SavedGenerationScope,
            &mut observer,
            None,
        )
        .unwrap();
    assert_eq!(actual.statistics().work.rule_applications(), 4);
    assert!(!snapshots.is_empty());
    assert!(snapshots.windows(2).all(|pair| {
        pair[0].statistics.operational_nodes <= pair[1].statistics.operational_nodes
            && pair[0].statistics.work.rule_applications()
                <= pair[1].statistics.work.rule_applications()
    }));
    assert_eq!(
        snapshots
            .last()
            .unwrap()
            .statistics
            .work
            .rule_applications(),
        4
    );
}

#[test]
fn failed_and_cancelled_requests_retain_work_without_completed_results() {
    let source = fixture();
    let c = source.programs.context.family().coefficient_context();
    let mut application = source.application();
    application.set_reduction_strategy(ReductionStrategy::DepthFirstWithoutMemoization);
    let cancel = AtomicBool::new(false);
    let mut callback = |event: &CandidateRoutedApplicationEnvelope<'_>| {
        if let Some(snapshot) = event.snapshot
            && snapshot.statistics.work.rule_applications() > 0
        {
            cancel.store(true, Ordering::Relaxed);
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
            [(key([4]), c.one())],
            CandidateEntryAdmission::SavedGenerationScope,
            &mut observer,
            Some(&cancel),
        ),
        Err(CandidateRoutedError::Cancelled)
    ));
    assert!(application.cache.is_empty());
    assert!(application.cache_statistics().rule_applications() > 0);
    assert_eq!(application.cache_work_statistics(), Default::default());
}

struct ForbiddenStore {
    binding: ApplicationCacheBinding,
}

impl GuardedCacheBacking for ForbiddenStore {
    fn binding(&self) -> &ApplicationCacheBinding {
        &self.binding
    }

    fn lookup(
        &mut self,
        _: &ApplicationCacheKey,
        _: &CoefficientContext,
    ) -> Result<Option<ApplicationCacheRecord>, GuardedCacheBackingError> {
        panic!("uncached strategies must reject before querying backing")
    }

    fn store_completed(
        &mut self,
        _: &ApplicationCacheRecord,
        _: &CoefficientContext,
    ) -> Result<(), GuardedCacheBackingError> {
        panic!("uncached strategies must reject before storing a result")
    }
}

impl GuardedIntermediateCapture for ForbiddenStore {
    fn wants(
        &mut self,
        _: &ApplicationCacheKey,
        _: &CoefficientContext,
    ) -> Result<bool, GuardedCacheBackingError> {
        panic!("uncached strategies must reject before querying capture")
    }

    fn store_completed(
        &mut self,
        _: &ApplicationCacheRecord,
        _: &CoefficientContext,
    ) -> Result<(), GuardedCacheBackingError> {
        panic!("uncached strategies must reject before capturing a result")
    }
}

#[test]
fn direct_and_initially_routed_cache_apis_reject_backing_and_capture() {
    let source = fixture();
    let c = source.programs.context.family().coefficient_context();
    let mut application = source.application();
    let binding = application.persistent_cache_binding([9; 32]);
    for strategy in [
        ReductionStrategy::DepthFirstWithoutMemoization,
        ReductionStrategy::Ordered,
    ] {
        application.set_reduction_strategy(strategy);
        for routed in [false, true] {
            for capture in [false, true] {
                let mut store = ForbiddenStore {
                    binding: binding.clone(),
                };
                let (backing, capture): (
                    Option<&mut dyn GuardedCacheBacking>,
                    Option<&mut dyn GuardedIntermediateCapture>,
                ) = if capture {
                    (None, Some(&mut store))
                } else {
                    (Some(&mut store), None)
                };
                let result = if routed {
                    application.reduce_routed_linear_combination_with_cache(
                        [(key([4]), c.one())],
                        CandidateEntryAdmission::SavedGenerationScope,
                        &binding,
                        backing,
                        capture,
                    )
                } else {
                    application.reduce_linear_combination_with_cache(
                        [(key([4]), c.one())],
                        CandidateEntryAdmission::SavedGenerationScope,
                        &binding,
                        backing,
                        capture,
                    )
                };
                assert!(matches!(result, Err(CandidateRoutedError::InvalidInput(_))));
                assert!(application.cache.is_empty());
            }
        }
    }
}
