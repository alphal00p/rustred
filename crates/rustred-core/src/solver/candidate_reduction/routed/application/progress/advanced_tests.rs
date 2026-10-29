use super::super::{CandidateRoutedCachePolicy, RoutedCandidateApplication};
use super::*;
use crate::algebra::CoefficientContext;
use crate::persistence::application_cache::{
    ApplicationCacheBinding, ApplicationCacheKey, ApplicationCacheRecord,
};
use crate::reduction::{
    GuardedCacheBacking, GuardedCacheBackingError, GuardedIntermediateCapture, ReductionLimits,
};
use crate::solver::candidate_reduction::owner_test_support::*;
use crate::solver::{
    CandidateCacheRepresentation, CandidateEntryAdmission, CandidateOwnerPrograms,
    CandidateReductionError, RoutedCandidateReducer, UnresolvedPolicy,
};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[derive(Clone, Debug)]
struct Recorded {
    sequence: u64,
    snapshot: Option<CandidateRoutedApplicationSnapshot>,
    event: CandidateRoutedApplicationEvent,
    metrics: CandidateRoutedApplicationMetrics,
    stored_state: Option<CandidateRoutedStoredState>,
    activity: CandidateRoutedActivity,
    node: Option<(u8, Vec<bool>, Vec<i64>)>,
    frontier: Option<CandidateRoutedFrontierSample>,
    complexity: CandidateRoutedComplexity,
    path_len: usize,
    path_total: usize,
}

fn record(envelope: &CandidateRoutedApplicationEnvelope<'_>) -> Recorded {
    if let Some(current) = envelope.current {
        assert_eq!(
            envelope.degrees,
            Some(CandidateRoutedKeyDegrees::from_key(current.target()))
        );
        if let CandidateRoutedWorkRef::Apply {
            owner_sector,
            target,
        } = current
        {
            assert_eq!(owner_sector.len(), target.powers().len());
        }
    }
    assert!(!envelope.family_fingerprint.is_empty());
    Recorded {
        sequence: envelope.sequence,
        snapshot: envelope.snapshot,
        event: envelope.event,
        metrics: envelope.metrics,
        stored_state: envelope.stored_state,
        activity: envelope.activity,
        node: envelope.current.map(|node| match node {
            CandidateRoutedWorkRef::Route(key) => (0, vec![], key.powers().to_vec()),
            CandidateRoutedWorkRef::Apply {
                owner_sector,
                target,
            } => (1, owner_sector.to_vec(), target.powers().to_vec()),
        }),
        frontier: envelope.frontier,
        complexity: envelope.complexity,
        path_len: envelope.active_path.len(),
        path_total: envelope.path_total_nodes,
    }
}

fn immediate_options() -> CandidateRoutedObservationOptions {
    CandidateRoutedObservationOptions {
        interval: Duration::ZERO,
        max_frontier_frames: 2,
        max_path_nodes: 1,
    }
}

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
    let mut app = RoutedCandidateReducer::try_new(owners, [], Default::default())
        .unwrap()
        .application();
    app.set_cache_policy(CandidateRoutedCachePolicy::EvictLeastRecentlyUsed)
        .unwrap();
    app
}

fn partial_application() -> RoutedCandidateApplication<1> {
    let family = Arc::new(crate::solver::tests::tadpole());
    let owners = programs(
        family.clone(),
        None,
        vec![input(
            [true],
            None,
            vec![rule(&family, [3], &[([2], 1)])],
            &[[1]],
        )],
        Default::default(),
    );
    let mut app = RoutedCandidateReducer::try_new(owners, [], Default::default())
        .unwrap()
        .application();
    app.set_unresolved_policy(UnresolvedPolicy::Preserve)
        .unwrap();
    app
}

struct MemoryStore {
    binding: ApplicationCacheBinding,
    records: BTreeMap<ApplicationCacheKey, ApplicationCacheRecord>,
}

impl GuardedCacheBacking for MemoryStore {
    fn binding(&self) -> &ApplicationCacheBinding {
        &self.binding
    }

    fn lookup(
        &mut self,
        key: &ApplicationCacheKey,
        _: &CoefficientContext,
    ) -> Result<Option<ApplicationCacheRecord>, GuardedCacheBackingError> {
        Ok(self.records.get(key).cloned())
    }

    fn store_completed(
        &mut self,
        record: &ApplicationCacheRecord,
        _: &CoefficientContext,
    ) -> Result<(), GuardedCacheBackingError> {
        self.records.insert(record.key.clone(), record.clone());
        Ok(())
    }
}

impl GuardedIntermediateCapture for MemoryStore {
    fn wants(
        &mut self,
        _: &ApplicationCacheKey,
        _: &CoefficientContext,
    ) -> Result<bool, GuardedCacheBackingError> {
        Ok(true)
    }

    fn store_completed(
        &mut self,
        record: &ApplicationCacheRecord,
        _: &CoefficientContext,
    ) -> Result<(), GuardedCacheBackingError> {
        self.records.insert(record.key.clone(), record.clone());
        Ok(())
    }
}

#[test]
fn backing_root_hit_counts_completion_without_replaying_dependencies() {
    let mut source = application(20);
    let one = source.family().coefficient_context().one();
    let binding = source.persistent_cache_binding([23; 32]);
    let mut store = MemoryStore {
        binding: binding.clone(),
        records: BTreeMap::new(),
    };
    let expected = source
        .reduce_linear_combination_with_cache(
            [(key([3]), one.clone())],
            CandidateEntryAdmission::SavedGenerationScope,
            &binding,
            None,
            Some(&mut store),
        )
        .unwrap();
    let mut restored = application(20);
    let records = RefCell::new(Vec::new());
    let mut sink = |envelope: &CandidateRoutedApplicationEnvelope<'_>| {
        records.borrow_mut().push(record(envelope));
        Ok(CandidateRoutedObserverControl::Continue)
    };
    let mut observer = CandidateRoutedApplicationObserver::new(immediate_options(), &mut sink);
    let actual = restored
        .reduce_linear_combination_with_cache_and_observation(
            [(key([3]), one)],
            CandidateEntryAdmission::SavedGenerationScope,
            &binding,
            Some(&mut store),
            None,
            &mut observer,
            None,
        )
        .unwrap();
    assert_eq!(actual.terms(), expected.terms());
    assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
    assert_eq!(actual.statistics().work.rule_applications(), 0);
    assert_eq!(actual.statistics().persistent_cache.requested_root_hits, 1);
    let rows = records.borrow();
    assert!(rows.iter().any(|row| row.event
        == CandidateRoutedApplicationEvent::NodeCompleted(
            CandidateRoutedCompletionSource::BackingHit
        )));
    let metrics = rows.last().unwrap().metrics;
    assert_eq!(metrics.completed_unique_operational_nodes, 1);
    assert_eq!(metrics.completion_attempts, 1);
    assert_eq!(metrics.observed_descent_checks, 0);
    assert_eq!(metrics.completed_roots, 1);
    assert_eq!(
        rows.last().unwrap().stored_state,
        Some(CandidateRoutedStoredState {
            unstarted_roots: Some(0),
            deferred_rule_maps: Some(0),
            deferred_child_integral_occurrences: Some(0),
            collected_output_integrals: Some(actual.terms().len()),
        })
    );
}

#[test]
fn cold_and_warm_requests_count_operational_identity_and_reset_request_metrics() {
    let mut app = application(20);
    let mut reference = application(20);
    let one = app.family().coefficient_context().one();
    let terms = [(key([3]), one.clone()), (key([4]), one)];
    let expected = reference.reduce_linear_combination(terms.clone()).unwrap();
    let records = RefCell::new(Vec::new());
    let mut sink = |envelope: &CandidateRoutedApplicationEnvelope<'_>| {
        records.borrow_mut().push(record(envelope));
        Ok(CandidateRoutedObserverControl::Continue)
    };
    let mut observer = CandidateRoutedApplicationObserver::new(immediate_options(), &mut sink);
    let actual = app
        .reduce_linear_combination_with_observation(
            terms.clone(),
            CandidateEntryAdmission::SavedGenerationScope,
            &mut observer,
            None,
        )
        .unwrap();
    assert_eq!(actual, expected);
    let split = records.borrow().len();
    {
        let cold = records.borrow();
        let finished = cold.last().unwrap();
        assert_eq!(
            finished.event,
            CandidateRoutedApplicationEvent::CoreFinished
        );
        assert_eq!(
            finished.snapshot.unwrap().status,
            CandidateRoutedApplicationStatus::Succeeded
        );
        assert_eq!(finished.metrics.completed_roots, 2);
        assert_eq!(
            finished.stored_state,
            Some(CandidateRoutedStoredState {
                unstarted_roots: Some(0),
                deferred_rule_maps: Some(0),
                deferred_child_integral_occurrences: Some(0),
                collected_output_integrals: Some(actual.terms().len()),
            })
        );
        assert_eq!(finished.metrics.requested_roots, Some(2));
        assert_eq!(finished.metrics.output_terms, Some(actual.terms().len()));
        assert_eq!(finished.metrics.completed_unique_operational_nodes, 6);
        assert!(finished.metrics.observed_descent_checks > 0);
        let completed = cold
            .iter()
            .filter(|row| matches!(row.event, CandidateRoutedApplicationEvent::NodeCompleted(_)))
            .map(|row| {
                row.node
                    .clone()
                    .expect("completion identifies its operational node")
            })
            .collect::<Vec<_>>();
        let unique = completed.iter().cloned().collect::<BTreeSet<_>>();
        assert_eq!(
            unique.len(),
            finished.metrics.completed_unique_operational_nodes
        );
        assert_eq!(completed.len(), finished.metrics.completion_attempts);
        assert!(
            completed.len() > unique.len(),
            "diamond joins are reuse, not cycles"
        );
        assert!(unique.contains(&(0, vec![], vec![3])));
        assert!(unique.contains(&(1, vec![true], vec![3])));
        assert!(
            !cold
                .iter()
                .any(|row| row.event == CandidateRoutedApplicationEvent::CycleDetected)
        );
        assert!(
            cold.iter()
                .all(|row| row.path_len <= 1 && row.path_len <= row.path_total)
        );
        assert!(cold.iter().any(|row| row.path_total > row.path_len));
        for sample in cold.iter().filter_map(|row| row.frontier) {
            assert!(sample.frames_examined <= 2);
            assert!(sample.frames_examined <= sample.total_pending_frames);
            assert!(sample.unique_integrals <= sample.unique_nodes);
            if sample.exact {
                assert_eq!(sample.frames_examined, sample.total_pending_frames);
            }
        }
        assert!(cold.windows(2).all(|rows| {
            rows[0].metrics.completed_unique_operational_nodes
                <= rows[1].metrics.completed_unique_operational_nodes
                && rows[0].metrics.completion_attempts <= rows[1].metrics.completion_attempts
        }));
    }
    let warm = app
        .reduce_linear_combination_with_observation(
            terms,
            CandidateEntryAdmission::SavedGenerationScope,
            &mut observer,
            None,
        )
        .unwrap();
    assert_eq!(warm.terms(), actual.terms());
    assert_eq!(warm.statistics().work.rule_applications(), 0);
    let all = records.borrow();
    let warm_records = &all[split..];
    assert_eq!(
        warm_records
            .first()
            .unwrap()
            .metrics
            .completed_unique_operational_nodes,
        0
    );
    assert_eq!(warm_records.first().unwrap().metrics.completion_attempts, 0);
    assert_eq!(
        warm_records.first().unwrap().stored_state,
        Some(CandidateRoutedStoredState {
            unstarted_roots: None,
            deferred_rule_maps: Some(0),
            deferred_child_integral_occurrences: Some(0),
            collected_output_integrals: None,
        })
    );
    let finished = warm_records.last().unwrap();
    assert_eq!(finished.metrics.completed_unique_operational_nodes, 2);
    assert_eq!(finished.metrics.completion_attempts, 2);
    assert_eq!(finished.metrics.observed_descent_checks, 0);
    assert_eq!(finished.metrics.completed_roots, 2);
    assert!(
        all.windows(2)
            .all(|rows| rows[0].sequence < rows[1].sequence)
    );
}

#[test]
fn eviction_recompletion_does_not_inflate_unique_completion_or_change_the_answer() {
    let mut app = application(2);
    let mut reference = application(2);
    let one = app.family().coefficient_context().one();
    let terms = [(key([3]), one.clone()), (key([4]), one)];
    let expected = reference.reduce_linear_combination(terms.clone()).unwrap();
    let records = RefCell::new(Vec::new());
    let mut sink = |envelope: &CandidateRoutedApplicationEnvelope<'_>| {
        records.borrow_mut().push(record(envelope));
        Ok(CandidateRoutedObserverControl::Continue)
    };
    let mut observer = CandidateRoutedApplicationObserver::new(immediate_options(), &mut sink);
    let actual = app
        .reduce_linear_combination_with_observation(
            terms,
            CandidateEntryAdmission::SavedGenerationScope,
            &mut observer,
            None,
        )
        .unwrap();
    assert_eq!(actual, expected);
    assert!(actual.statistics().cache.recomputed_nodes > 0);
    let records = records.borrow();
    let finished = records.last().unwrap();
    let completed = records
        .iter()
        .filter(|row| matches!(row.event, CandidateRoutedApplicationEvent::NodeCompleted(_)))
        .map(|row| row.node.clone().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(completed.len(), finished.metrics.completion_attempts);
    assert_eq!(
        completed.iter().cloned().collect::<BTreeSet<_>>().len(),
        finished.metrics.completed_unique_operational_nodes
    );
    assert_eq!(finished.metrics.completed_unique_operational_nodes, 6);
    assert!(
        finished.metrics.completion_attempts > finished.metrics.completed_unique_operational_nodes
    );
    assert_eq!(finished.snapshot.unwrap().statistics, *actual.statistics());
    assert_eq!(finished.snapshot.unwrap().active_nodes, 0);
    assert_eq!(finished.snapshot.unwrap().pending_frames, 0);
    assert_eq!(finished.snapshot.unwrap().pinned.unwrap().entries, 0);
    assert_eq!(finished.stored_state.unwrap().deferred_rule_maps, Some(0));
    assert_eq!(
        finished
            .stored_state
            .unwrap()
            .deferred_child_integral_occurrences,
        Some(0)
    );
}

#[test]
fn partial_preserve_reports_ready_nodes_without_claiming_resolved_output() {
    let mut app = partial_application();
    let one = app.family().coefficient_context().one();
    let records = RefCell::new(Vec::new());
    let mut sink = |envelope: &CandidateRoutedApplicationEnvelope<'_>| {
        records.borrow_mut().push(record(envelope));
        Ok(CandidateRoutedObserverControl::Continue)
    };
    let mut observer = CandidateRoutedApplicationObserver::new(immediate_options(), &mut sink);
    let result = app
        .reduce_routed_linear_combination_with_observation(
            [(key([3]), one)],
            CandidateEntryAdmission::SavedGenerationScope,
            &mut observer,
            None,
        )
        .unwrap();
    assert_eq!(result.unresolved(), &BTreeSet::from([key([2])]));
    let rows = records.borrow();
    assert!(rows.iter().any(|row| row.event
        == CandidateRoutedApplicationEvent::NodeCompleted(
            CandidateRoutedCompletionSource::UnresolvedPreserved
        )));
    let finished = rows.last().unwrap();
    assert_eq!(finished.metrics.completed_roots, 1);
    assert_eq!(finished.metrics.requested_roots, Some(1));
    assert_eq!(finished.metrics.output_terms, Some(1));
    assert_eq!(
        finished.snapshot.unwrap().status,
        CandidateRoutedApplicationStatus::Succeeded
    );
}

#[test]
fn detach_is_observational_and_the_observer_stays_detached_on_reuse() {
    let mut app = application(20);
    let mut reference = application(20);
    let one = app.family().coefficient_context().one();
    let terms = [(key([3]), one.clone())];
    let expected = reference.reduce_linear_combination(terms.clone()).unwrap();
    let calls = std::cell::Cell::new(0);
    let mut sink = |_: &CandidateRoutedApplicationEnvelope<'_>| {
        calls.set(calls.get() + 1);
        Ok(CandidateRoutedObserverControl::Detach)
    };
    let mut observer = CandidateRoutedApplicationObserver::new(immediate_options(), &mut sink);
    let actual = app
        .reduce_linear_combination_with_observation(
            terms.clone(),
            CandidateEntryAdmission::SavedGenerationScope,
            &mut observer,
            None,
        )
        .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(observer.status(), CandidateRoutedObserverStatus::Detached);
    assert_eq!(calls.get(), 1);
    app.reduce_linear_combination_with_observation(
        terms,
        CandidateEntryAdmission::SavedGenerationScope,
        &mut observer,
        None,
    )
    .unwrap();
    assert_eq!(calls.get(), 1);
}

#[test]
fn observation_cadence_preserves_forced_final_metrics_and_exact_work() {
    let mut immediate = application(20);
    let mut throttled = application(20);
    let one = immediate.family().coefficient_context().one();
    let terms = [(key([3]), one.clone()), (key([4]), one)];
    let all = RefCell::new(Vec::new());
    let sparse = RefCell::new(Vec::new());
    let mut immediate_sink = |envelope: &CandidateRoutedApplicationEnvelope<'_>| {
        all.borrow_mut().push(record(envelope));
        Ok(CandidateRoutedObserverControl::Continue)
    };
    let mut throttled_sink = |envelope: &CandidateRoutedApplicationEnvelope<'_>| {
        sparse.borrow_mut().push(record(envelope));
        Ok(CandidateRoutedObserverControl::Continue)
    };
    let mut immediate_observer =
        CandidateRoutedApplicationObserver::new(immediate_options(), &mut immediate_sink);
    let mut throttled_observer = CandidateRoutedApplicationObserver::new(
        CandidateRoutedObservationOptions {
            interval: Duration::from_secs(3600),
            ..immediate_options()
        },
        &mut throttled_sink,
    );
    let full = immediate
        .reduce_linear_combination_with_observation(
            terms.clone(),
            CandidateEntryAdmission::SavedGenerationScope,
            &mut immediate_observer,
            None,
        )
        .unwrap();
    let sparse_result = throttled
        .reduce_linear_combination_with_observation(
            terms,
            CandidateEntryAdmission::SavedGenerationScope,
            &mut throttled_observer,
            None,
        )
        .unwrap();
    assert_eq!(full, sparse_result);
    let all = all.borrow();
    let sparse = sparse.borrow();
    assert!(sparse.len() < all.len());
    assert_eq!(sparse.first().unwrap().metrics.completed_roots, 0);
    assert_eq!(
        sparse.last().unwrap().event,
        CandidateRoutedApplicationEvent::CoreFinished
    );
    assert_eq!(sparse.last().unwrap().metrics, all.last().unwrap().metrics);
    assert_eq!(
        sparse.last().unwrap().stored_state,
        all.last().unwrap().stored_state
    );
    assert_eq!(
        sparse.last().unwrap().snapshot.unwrap().statistics,
        *full.statistics()
    );
}

#[test]
fn factorized_observation_preserves_guarded_work_and_reports_retained_census() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let mut owners = CandidateOwnerPrograms::try_new(
        context(
            family.clone(),
            None,
            ReductionLimits {
                diagnostics_enabled: true,
                ..Default::default()
            },
        ),
        [input(
            [true],
            None,
            vec![
                rule(&family, [4], &[([2], 1)]),
                rule(&family, [3], &[([2], -3), ([1], 2)]),
                rule(&family, [2], &[([1], 2)]),
            ],
            &[[1]],
        )],
    )
    .unwrap();
    let indexed = owners.context.coefficient_context().clone();
    let lifted = indexed
        .lift(&family.coefficient_context().coefficient_fixture("d-7"))
        .unwrap();
    let guard = indexed
        .admit_native_polynomial_result_with_limits(
            lifted.raw().numerator.clone(),
            Default::default(),
        )
        .unwrap();
    Arc::get_mut(&mut owners.context)
        .unwrap()
        .shared
        .source_conditions = vec![guard];
    let graph = RoutedCandidateReducer::try_new(Arc::new(owners), [], Default::default()).unwrap();
    let mut observed = graph.application();
    let mut reference = graph.application();
    observed
        .set_cache_representation(CandidateCacheRepresentation::Factorized)
        .unwrap();
    reference
        .set_cache_representation(CandidateCacheRepresentation::Factorized)
        .unwrap();
    let one = family.coefficient_context().one();
    let terms = [(key([3]), one.clone()), (key([4]), one)];
    let expected = reference.reduce_linear_combination(terms.clone()).unwrap();
    let records = RefCell::new(Vec::new());
    let mut sink = |envelope: &CandidateRoutedApplicationEnvelope<'_>| {
        records.borrow_mut().push(record(envelope));
        Ok(CandidateRoutedObserverControl::Continue)
    };
    let mut observer = CandidateRoutedApplicationObserver::new(immediate_options(), &mut sink);
    let actual = observed
        .reduce_linear_combination_with_observation(
            terms,
            CandidateEntryAdmission::SavedGenerationScope,
            &mut observer,
            None,
        )
        .unwrap();
    assert_eq!(actual, expected);
    assert!(!actual.nonzero_conditions().is_empty());
    assert_eq!(observed.cache_statistics(), reference.cache_statistics());
    assert!(observed.cache.values().all(|entry| matches!(
        &entry.terms,
        crate::solver::candidate_reduction::cache::CachedTerms::Factorized(_)
    )));
    let rows = records.borrow();
    let sampled = rows
        .iter()
        .filter(|row| row.complexity.factorized)
        .collect::<Vec<_>>();
    assert!(!sampled.is_empty());
    for row in sampled {
        let (stage, owner, powers) = row.node.as_ref().unwrap();
        let target = crate::family::IntegralKey::try_new(powers.clone()).unwrap();
        let node = if *stage == 0 {
            super::super::Node::Route(target)
        } else {
            assert_eq!(owner.as_slice(), &[true]);
            super::super::Node::Apply([true], target)
        };
        let entry = observed.cache.get(&node).unwrap();
        let crate::solver::candidate_reduction::cache::CachedTerms::Factorized(retained) =
            &entry.terms
        else {
            unreachable!("the retained representation must remain factorized");
        };
        assert_eq!(row.complexity.terms, Some(retained.len()));
        assert_eq!(
            row.complexity.coefficient_terms,
            Some(entry.weight.coefficient_terms)
        );
        assert_eq!(
            row.complexity.coefficient_bytes,
            Some(entry.weight.coefficient_bytes)
        );
    }
    let final_row = rows.last().unwrap();
    assert_eq!(final_row.metrics.output_terms, Some(actual.terms().len()));
    assert_eq!(
        final_row.stored_state.unwrap().collected_output_integrals,
        Some(actual.terms().len())
    );
    assert_eq!(final_row.stored_state.unwrap().deferred_rule_maps, Some(0));
    assert!(
        rows.iter()
            .all(|row| row.complexity.encoded_expression_bytes.is_none())
    );
    let retained = observed.cache_statistics();
    assert_eq!(
        final_row.snapshot.unwrap().cached.coefficient_terms,
        retained.cached_coefficient_terms()
    );
    assert_eq!(
        final_row.snapshot.unwrap().cached.coefficient_bytes,
        retained.cached_coefficient_bytes()
    );
}

#[test]
fn activity_notices_do_not_starve_the_quantitative_clock_or_work_gate() {
    let mut sink =
        |_: &CandidateRoutedApplicationEnvelope<'_>| Ok(CandidateRoutedObserverControl::Continue);
    let mut observer = CandidateRoutedApplicationObserver::new(
        CandidateRoutedObservationOptions {
            interval: Duration::from_secs(3600),
            ..immediate_options()
        },
        &mut sink,
    );
    let mut progress = Progress::from_observer(&mut observer).unwrap();
    assert!(progress.due(true).is_some());
    for _ in 0..17 {
        assert!(progress.due(false).is_none());
    }
    let quantitative_last = progress.last;
    let quantitative_remaining = progress.remaining;
    let sequence_before_notices = progress.sequence;
    for _ in 0..1024 {
        assert!(progress.notice().is_some());
    }
    assert_eq!(progress.last, quantitative_last);
    assert_eq!(progress.remaining, quantitative_remaining);
    assert_eq!(progress.sequence, sequence_before_notices + 1024);
    assert!(progress.due(false).is_none());
    assert_eq!(progress.remaining, quantitative_remaining - 1);
    assert_eq!(progress.last, quantitative_last);
}

#[test]
fn ordered_progress_is_optional_and_cleared_at_the_next_native_request() {
    let mut sink =
        |_: &CandidateRoutedApplicationEnvelope<'_>| Ok(CandidateRoutedObserverControl::Continue);
    let mut observer = CandidateRoutedApplicationObserver::new(immediate_options(), &mut sink);
    assert!(observer.last_ordered_progress().is_none());
    let counts = CandidateOrderedReductionProgress {
        completed_phases: 3,
        total_phases: 8,
        active_support: Some(4),
        frontier_keys: 17,
        cancelled_terms: 5,
        ..Default::default()
    };
    let mut envelope = CandidateRoutedApplicationEnvelope::new(
        CandidateRoutedApplicationEvent::Snapshot,
        "ordered-family",
    );
    envelope.ordered = Some(counts);
    observer.emit(envelope);
    assert_eq!(observer.last_ordered_progress(), Some(counts));
    observer.emit(CandidateRoutedApplicationEnvelope::new(
        CandidateRoutedApplicationEvent::CallerProgress,
        "ordered-family",
    ));
    assert_eq!(observer.last_ordered_progress(), Some(counts));
    observer.clear_native_summary();
    assert!(observer.last_ordered_progress().is_none());
}

#[test]
fn generic_ordered_progress_forces_phase_boundaries_without_inventing_routed_work() {
    let mut records = Vec::new();
    let current = key([3]);
    {
        let mut sink = |envelope: &CandidateRoutedApplicationEnvelope<'_>| {
            assert!(envelope.current.is_none());
            assert!(envelope.snapshot.is_none());
            records.push((
                envelope.caller_phase.map(str::to_owned),
                envelope.ordered,
                envelope.ordered_current.map(|key| key.powers().to_vec()),
                envelope.native_summary_age,
            ));
            Ok(CandidateRoutedObserverControl::Continue)
        };
        let mut observer = CandidateRoutedApplicationObserver::new(
            CandidateRoutedObservationOptions {
                interval: Duration::from_secs(3600),
                ..immediate_options()
            },
            &mut sink,
        );
        let mut progress = CandidateOrderedReductionProgress {
            total_phases: 2,
            input_keys: 1,
            ..Default::default()
        };
        observer.emit_ordered_progress("generic-family", progress, Some(&current));
        progress.frontier_keys = 7;
        // A generic artifact may traverse different supports inside one saved
        // ordering phase; this is current-key detail, not a new phase boundary.
        progress.active_support = Some(1);
        observer.emit_ordered_progress("generic-family", progress, Some(&current));
        progress.completed_phases = 1;
        observer.emit_ordered_progress("generic-family", progress, Some(&current));
        progress.completed_phases = 2;
        observer.emit_ordered_progress("generic-family", progress, None);
        observer.clear_native_summary();
        observer.emit_ordered_progress("next-family", Default::default(), None);
    }
    assert_eq!(records.len(), 6);
    assert_eq!(records[0].0.as_deref(), Some("entry_admission"));
    assert_eq!(records[1].1.unwrap().completed_phases, 0);
    assert_eq!(records[1].2.as_deref(), Some([3].as_slice()));
    assert_eq!(records[1].3, Some(Duration::ZERO));
    assert_eq!(records[2].1.unwrap().completed_phases, 1);
    assert_eq!(records[3].1.unwrap().completed_phases, 2);
    assert_eq!(records[4].0.as_deref(), Some("entry_admission"));
}

#[test]
fn typed_resource_failure_survives_a_panicking_failure_callback() {
    use symbolica::utils::resources::{ResourceBudget, ResourceLimitError, ResourceLimits};
    let mut app = application(20);
    let one = app.family().coefficient_context().one();
    let mut sink = |envelope: &CandidateRoutedApplicationEnvelope<'_>| {
        if envelope.status == CandidateRoutedApplicationStatus::Failed {
            panic!("failure callback must not replace the resource cause");
        }
        Ok(CandidateRoutedObserverControl::Continue)
    };
    let mut observer = CandidateRoutedApplicationObserver::new(immediate_options(), &mut sink);
    let budget = ResourceBudget::new(ResourceLimits {
        max_elapsed: Some(Duration::ZERO),
        ..Default::default()
    });
    let error = budget
        .run(|| {
            app.reduce_linear_combination_with_observation(
                [(key([3]), one.clone())],
                CandidateEntryAdmission::SavedGenerationScope,
                &mut observer,
                None,
            )
        })
        .unwrap_err();
    let mut cause: &(dyn std::error::Error + 'static) = &error;
    loop {
        if let Some(resource) = cause.downcast_ref::<ResourceLimitError>() {
            assert!(matches!(resource, ResourceLimitError::Elapsed { .. }));
            break;
        }
        cause = cause
            .source()
            .expect("the original typed resource cause must survive");
    }
    assert_eq!(
        observer.status(),
        CandidateRoutedObserverStatus::Faulted(CandidateRoutedObserverFault::Panicked)
    );
    assert_eq!(app.cache_statistics().cached_integrals(), 0);
    app.reduce_linear_combination([(key([3]), one)]).unwrap();
}

#[test]
fn panicking_payload_destructor_cannot_turn_telemetry_fault_into_reduction_unwind() {
    struct PanicOnDrop;
    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            panic!("observer payload destructor");
        }
    }
    let mut app = application(20);
    let mut reference = application(20);
    let one = app.family().coefficient_context().one();
    let terms = [(key([3]), one)];
    let expected = reference.reduce_linear_combination(terms.clone()).unwrap();
    let mut sink =
        |_: &CandidateRoutedApplicationEnvelope<'_>| -> Result<CandidateRoutedObserverControl, ()> {
            std::panic::panic_any(PanicOnDrop);
        };
    let mut observer = CandidateRoutedApplicationObserver::new(immediate_options(), &mut sink);
    let actual = app
        .reduce_linear_combination_with_observation(
            terms,
            CandidateEntryAdmission::SavedGenerationScope,
            &mut observer,
            None,
        )
        .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(
        observer.status(),
        CandidateRoutedObserverStatus::Faulted(CandidateRoutedObserverFault::Panicked)
    );
}

#[test]
fn precancelled_observed_request_remains_cancelled_and_leaves_cache_usable() {
    let mut app = application(20);
    let one = app.family().coefficient_context().one();
    let cancel = AtomicBool::new(true);
    let records = RefCell::new(Vec::new());
    let mut sink = |envelope: &CandidateRoutedApplicationEnvelope<'_>| {
        records.borrow_mut().push(record(envelope));
        Ok(CandidateRoutedObserverControl::Continue)
    };
    let mut observer = CandidateRoutedApplicationObserver::new(immediate_options(), &mut sink);
    let error = app
        .reduce_linear_combination_with_observation(
            [(key([3]), one.clone())],
            CandidateEntryAdmission::SavedGenerationScope,
            &mut observer,
            Some(&cancel),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        super::super::CandidateRoutedError::Cancelled
    ));
    let rows = records.borrow();
    assert_eq!(
        rows.last().unwrap().snapshot.unwrap().status,
        CandidateRoutedApplicationStatus::Failed
    );
    assert_eq!(
        rows.last()
            .unwrap()
            .metrics
            .completed_unique_operational_nodes,
        0
    );
    assert_eq!(rows.last().unwrap().metrics.completed_roots, 0);
    let state = rows.last().unwrap().stored_state.unwrap();
    assert_eq!(state.unstarted_roots, None);
    assert_eq!(state.collected_output_integrals, None);
    assert_eq!(state.deferred_rule_maps, Some(0));
    assert_eq!(state.deferred_child_integral_occurrences, Some(0));
    assert_eq!(app.cache_statistics().cached_integrals(), 0);
    app.reduce_linear_combination([(key([3]), one)]).unwrap();
}

#[test]
fn non_descending_self_dependency_is_not_hidden_by_observation() {
    let family = Arc::new(crate::solver::tests::tadpole());
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
    let mut app = RoutedCandidateReducer::try_new(owners, [], Default::default())
        .unwrap()
        .application();
    let records = RefCell::new(Vec::new());
    let mut sink = |envelope: &CandidateRoutedApplicationEnvelope<'_>| {
        records.borrow_mut().push(record(envelope));
        Ok(CandidateRoutedObserverControl::Continue)
    };
    let mut observer = CandidateRoutedApplicationObserver::new(immediate_options(), &mut sink);
    let error = app
        .reduce_linear_combination_with_observation(
            [(key([2]), family.coefficient_context().one())],
            CandidateEntryAdmission::SavedGenerationScope,
            &mut observer,
            None,
        )
        .unwrap_err();
    assert!(matches!(
        error,
        super::super::CandidateRoutedError::Candidate(
            CandidateReductionError::NonDescending { .. }
        )
    ));
    let rows = records.borrow();
    assert!(
        !rows
            .iter()
            .any(|row| matches!(row.event, CandidateRoutedApplicationEvent::NodeCompleted(_)))
    );
    assert_eq!(
        rows.last()
            .unwrap()
            .metrics
            .completed_unique_operational_nodes,
        0
    );
    assert_eq!(
        rows.last().unwrap().snapshot.unwrap().status,
        CandidateRoutedApplicationStatus::Failed
    );
}

#[test]
fn key_degrees_cover_dynamic_arity_and_extreme_signed_powers() {
    let mut powers = vec![0; 21];
    powers[0] = 2;
    powers[4] = -3;
    powers[20] = 4;
    let dynamic_key = crate::family::IntegralKey::try_new(powers).unwrap();
    assert_eq!(
        CandidateRoutedKeyDegrees::from_key(&dynamic_key),
        CandidateRoutedKeyDegrees {
            numerator_rank: 3,
            positive_power_sum: 6,
            power_difference: 3,
            support: 2,
            dot_excess: 4,
        }
    );
    let extreme = key([i64::MIN, i64::MAX, 0, 1]);
    assert_eq!(
        CandidateRoutedKeyDegrees::from_key(&extreme),
        CandidateRoutedKeyDegrees {
            numerator_rank: 1_u128 << 63,
            positive_power_sum: 1_u128 << 63,
            power_difference: 0,
            support: 2,
            dot_excess: (1_u128 << 63) - 2,
        }
    );
}

#[test]
fn callback_error_detaches_observation_without_changing_exact_reduction() {
    let mut app = application(20);
    let mut reference = application(20);
    let one = app.family().coefficient_context().one();
    let terms = [(key([3]), one.clone()), (key([4]), one)];
    let expected = reference.reduce_linear_combination(terms.clone()).unwrap();
    let mut calls = 0;
    let mut sink =
        |_: &CandidateRoutedApplicationEnvelope<'_>| -> Result<CandidateRoutedObserverControl, ()> {
            calls += 1;
            Err(())
        };
    let mut observer = CandidateRoutedApplicationObserver::new(Default::default(), &mut sink);
    let actual = app
        .reduce_linear_combination_with_observation(
            terms,
            CandidateEntryAdmission::SavedGenerationScope,
            &mut observer,
            None,
        )
        .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(
        observer.status(),
        CandidateRoutedObserverStatus::Faulted(CandidateRoutedObserverFault::CallbackError)
    );
    drop(observer);
    assert_eq!(calls, 1);
    assert_eq!(app.cache_statistics(), reference.cache_statistics());
}

#[test]
fn callback_panic_is_contained_and_completed_cache_stays_reusable() {
    let mut app = application(2);
    let mut reference = application(2);
    let one = app.family().coefficient_context().one();
    let terms = [(key([3]), one.clone()), (key([4]), one.clone())];
    let expected = reference.reduce_linear_combination(terms.clone()).unwrap();
    let mut calls = 0;
    let mut sink =
        |_: &CandidateRoutedApplicationEnvelope<'_>| -> Result<CandidateRoutedObserverControl, ()> {
            calls += 1;
            panic!("test observational callback panic");
        };
    let mut observer = CandidateRoutedApplicationObserver::new(Default::default(), &mut sink);
    let actual = app
        .reduce_linear_combination_with_observation(
            terms,
            CandidateEntryAdmission::SavedGenerationScope,
            &mut observer,
            None,
        )
        .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(
        observer.status(),
        CandidateRoutedObserverStatus::Faulted(CandidateRoutedObserverFault::Panicked)
    );
    drop(observer);
    assert_eq!(calls, 1);
    let warm = app.reduce_linear_combination([(key([4]), one)]).unwrap();
    assert_eq!(warm.statistics().work.rule_applications(), 0);
    assert_eq!(app.cache.len(), app.eviction_index.eligible.len());
}

#[test]
fn stored_state_counts_retained_maps_and_cancellation_without_changing_cache_records() {
    let mut observed = application(20);
    let mut reference = application(20);
    let coefficients = observed.family().coefficient_context();
    let terms = [
        (key([3]), coefficients.one()),
        (key([4]), coefficients.coefficient_fixture("2")),
    ];
    let binding = observed.persistent_cache_binding([71; 32]);
    let mut expected_store = MemoryStore {
        binding: binding.clone(),
        records: BTreeMap::new(),
    };
    let mut actual_store = MemoryStore {
        binding: binding.clone(),
        records: BTreeMap::new(),
    };
    let expected = reference
        .reduce_linear_combination_with_cache(
            terms.clone(),
            CandidateEntryAdmission::SavedGenerationScope,
            &binding,
            None,
            Some(&mut expected_store),
        )
        .unwrap();
    let rows = RefCell::new(Vec::new());
    let mut sink = |envelope: &CandidateRoutedApplicationEnvelope<'_>| {
        rows.borrow_mut().push(record(envelope));
        Ok(CandidateRoutedObserverControl::Continue)
    };
    let mut observer = CandidateRoutedApplicationObserver::new(immediate_options(), &mut sink);
    let actual = observed
        .reduce_linear_combination_with_cache_and_observation(
            terms,
            CandidateEntryAdmission::SavedGenerationScope,
            &binding,
            None,
            Some(&mut actual_store),
            &mut observer,
            None,
        )
        .unwrap();
    assert_eq!(actual, expected);
    assert!(
        actual.terms().is_empty(),
        "the two completed roots cancel exactly"
    );
    assert_eq!(actual_store.records, expected_store.records);
    assert_eq!(observed.cache_statistics(), reference.cache_statistics());
    let rows = rows.borrow();
    let roots = rows
        .iter()
        .filter(|row| row.event == CandidateRoutedApplicationEvent::RootCollected)
        .map(|row| row.stored_state.unwrap())
        .collect::<Vec<_>>();
    assert!(
        rows.iter()
            .filter(|row| row.event
                == CandidateRoutedApplicationEvent::NodeCompleted(
                    CandidateRoutedCompletionSource::Computed
                ))
            .all(|row| row.stored_state.unwrap().deferred_rule_maps.unwrap() > 0),
        "a completed combination still owns its popped child map through completion delivery"
    );
    assert_eq!(roots.len(), 2);
    assert_eq!(roots[0].unstarted_roots, Some(1));
    assert_eq!(roots[0].collected_output_integrals, Some(1));
    assert_eq!(roots[1].unstarted_roots, Some(0));
    assert_eq!(roots[1].collected_output_integrals, Some(0));
    assert!(roots.iter().all(|state| state.deferred_rule_maps == Some(0)
        && state.deferred_child_integral_occurrences == Some(0)));
    assert_eq!(
        rows.iter()
            .filter_map(|row| row.stored_state?.deferred_rule_maps)
            .max(),
        Some(3)
    );
    assert_eq!(
        rows.iter()
            .filter_map(|row| row.stored_state?.deferred_child_integral_occurrences)
            .max(),
        Some(4)
    );
    assert!(
        rows.iter().any(
            |row| row.activity == CandidateRoutedActivity::CoefficientCombination
                && row
                    .stored_state
                    .is_some_and(|state| state.deferred_rule_maps.unwrap() > 0)
        ),
        "the currently popped Combine map remains counted while being processed"
    );
}

#[test]
fn stored_state_unknowns_reset_and_empty_request_is_exact_zero() {
    let mut app = application(20);
    let mut rows = Vec::new();
    let mut sink = |envelope: &CandidateRoutedApplicationEnvelope<'_>| {
        rows.push(record(envelope));
        Ok(CandidateRoutedObserverControl::Continue)
    };
    let mut observer = CandidateRoutedApplicationObserver::new(immediate_options(), &mut sink);
    let result = app
        .reduce_linear_combination_with_observation(
            [],
            CandidateEntryAdmission::SavedGenerationScope,
            &mut observer,
            None,
        )
        .unwrap();
    assert!(result.terms().is_empty());
    assert_eq!(
        observer.last_stored_state(),
        Some(CandidateRoutedStoredState {
            unstarted_roots: Some(0),
            deferred_rule_maps: Some(0),
            deferred_child_integral_occurrences: Some(0),
            collected_output_integrals: Some(0),
        })
    );
    observer.clear_native_summary();
    assert_eq!(observer.last_stored_state(), None);
}
