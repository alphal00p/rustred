use super::*;
use crate::reduction::ReductionLimits;
use crate::solver::candidate_reduction::owner_test_support::*;
use crate::solver::{
    CandidateCacheRepresentation, CandidateReductionError, RoutedCandidateReducer,
};
use std::sync::Arc;

fn application(mut limits: ReductionLimits) -> RoutedCandidateApplication<1> {
    // These fixtures exercise explicitly bounded cache/eviction diagnostics.
    limits.diagnostics_enabled = true;
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
            &[[1]],
        )],
        limits,
    );
    RoutedCandidateReducer::try_new(owners, [], Default::default())
        .unwrap()
        .application()
}

fn assert_census<const N: usize>(app: &RoutedCandidateApplication<N>) {
    let mut weight = CacheWeight::default();
    for entry in app.cache.values() {
        weight = weight.checked_add(entry.weight).unwrap();
    }
    assert_eq!(weight, app.cache_weight);
    let census = app.cache_budget.snapshot();
    assert_eq!(census.integrals, app.cache.len());
    assert_eq!(census.coefficient_terms, weight.coefficient_terms);
    assert_eq!(census.coefficient_bytes, weight.coefficient_bytes);
    assert!(app.eviction_index.eligible.len() <= app.cache.len());
    for (priority, stamp, node) in &app.eviction_index.eligible {
        assert_eq!(*stamp, app.cache[node].eviction.last_access);
        assert_eq!(*priority, app.cache[node].eviction.priority);
    }
}

#[test]
fn aged_lfu_counts_only_genuine_reuse_and_uses_recency_for_ties() {
    let mut app = application(ReductionLimits {
        diagnostics_enabled: true,
        max_cached_integrals: 2,
        ..Default::default()
    });
    app.set_cache_policy(CandidateRoutedCachePolicy::EvictAgedLeastFrequentlyUsed)
        .unwrap();
    let mut pins = DependencyPins::default();
    let mut stats = CandidateRoutedCacheStatistics::default();
    let a = Node::Route(key([1]));
    let b = Node::Apply([true], key([1]));
    let c = Node::Route(key([2]));
    let d = Node::Route(key([3]));
    insert(&mut app, a.clone(), vec![], &mut pins, &mut stats).unwrap();
    insert(&mut app, b.clone(), vec![], &mut pins, &mut stats).unwrap();
    for _ in 0..10 {
        app.touch_cache(&a).unwrap();
    }
    assert_eq!(app.cache[&a].eviction.reuse_count, 0);
    assert_eq!(app.cache[&a].eviction.priority, 1);
    insert(&mut app, c.clone(), vec![], &mut pins, &mut stats).unwrap();
    assert!(app.cache.contains_key(&a));
    assert!(!app.cache.contains_key(&b));
    assert_eq!(app.eviction_index.age, 1);
    app.reuse_cache(&a).unwrap();
    assert_eq!(app.cache[&a].eviction.reuse_count, 1);
    assert_eq!(app.cache[&a].eviction.priority, 3);
    insert(&mut app, d.clone(), vec![], &mut pins, &mut stats).unwrap();
    assert!(!app.cache.contains_key(&c));
    assert_eq!(app.eviction_index.age, 2);
    assert_eq!(app.cache[&d].eviction.priority, 3);
    assert_eq!(app.cache[&d].eviction.reuse_count, 0);
    assert_census(&app);
}

#[test]
fn lru_reuse_and_eviction_do_not_update_lfu_counts_or_age() {
    let mut app = application(ReductionLimits {
        diagnostics_enabled: true,
        max_cached_integrals: 2,
        ..Default::default()
    });
    app.set_cache_policy(CandidateRoutedCachePolicy::EvictLeastRecentlyUsed)
        .unwrap();
    let one = app.family().coefficient_context().one();
    for power in [3, 3, 4, 4, 3] {
        app.reduce_linear_combination([(key([power]), one.clone())])
            .unwrap();
    }
    assert!(app.cache_work_statistics().evicted_entries > 0);
    assert_eq!(app.eviction_index.age, 0);
    assert!(
        app.cache
            .values()
            .all(|entry| entry.eviction.reuse_count == 0 && entry.eviction.priority == 0)
    );
    assert_census(&app);
}

#[test]
fn aged_lfu_cold_stream_eventually_retires_a_formerly_hot_entry() {
    let mut app = application(ReductionLimits {
        diagnostics_enabled: true,
        max_cached_integrals: 2,
        ..Default::default()
    });
    app.set_cache_policy(CandidateRoutedCachePolicy::EvictAgedLeastFrequentlyUsed)
        .unwrap();
    let mut pins = DependencyPins::default();
    let mut stats = CandidateRoutedCacheStatistics::default();
    let hot = Node::Route(key([1]));
    insert(&mut app, hot.clone(), vec![], &mut pins, &mut stats).unwrap();
    insert(
        &mut app,
        Node::Route(key([2])),
        vec![],
        &mut pins,
        &mut stats,
    )
    .unwrap();
    for _ in 0..3 {
        app.reuse_cache(&hot).unwrap();
    }
    assert_eq!(app.cache[&hot].eviction.priority, 4);
    for power in 3..=5 {
        insert(
            &mut app,
            Node::Route(key([power])),
            vec![],
            &mut pins,
            &mut stats,
        )
        .unwrap();
        assert!(app.cache.contains_key(&hot));
    }
    insert(
        &mut app,
        Node::Route(key([6])),
        vec![],
        &mut pins,
        &mut stats,
    )
    .unwrap();
    assert!(!app.cache.contains_key(&hot));
    assert_eq!(app.eviction_index.age, 4);
    assert_eq!(stats.evicted_entries, 4);
    assert_census(&app);
}

#[test]
fn aged_lfu_shared_pins_survive_aging_and_release_cannot_rewind_age() {
    let mut app = application(ReductionLimits {
        diagnostics_enabled: true,
        max_cached_integrals: 2,
        ..Default::default()
    });
    app.set_cache_policy(CandidateRoutedCachePolicy::EvictAgedLeastFrequentlyUsed)
        .unwrap();
    let mut pins = DependencyPins {
        census: Some(Default::default()),
        ..Default::default()
    };
    let mut stats = CandidateRoutedCacheStatistics::default();
    let protected = Node::Route(key([1]));
    let guard = app.family().coefficient_context().integer(7);
    insert(
        &mut app,
        protected.clone(),
        vec![guard.clone()],
        &mut pins,
        &mut stats,
    )
    .unwrap();
    app.pin_dependency(&protected, &mut pins).unwrap();
    app.pin_dependency(&protected, &mut pins).unwrap();
    for power in 2..=6 {
        insert(
            &mut app,
            Node::Route(key([power])),
            vec![],
            &mut pins,
            &mut stats,
        )
        .unwrap();
        assert!(app.cache.contains_key(&protected));
        assert_eq!(pins.census.unwrap().entries, 1);
    }
    let age = app.eviction_index.age;
    assert!(age > app.cache[&protected].eviction.priority);
    app.release_dependency(&protected, &mut pins).unwrap();
    assert_eq!(pins.census.unwrap().entries, 1);
    app.release_dependency(&protected, &mut pins).unwrap();
    assert_eq!(pins.census.unwrap().entries, 0);
    insert(
        &mut app,
        Node::Route(key([7])),
        vec![],
        &mut pins,
        &mut stats,
    )
    .unwrap();
    assert!(!app.cache.contains_key(&protected));
    assert_eq!(app.eviction_index.age, age);
    assert_census(&app);
}

#[test]
fn aged_lfu_replacement_is_fresh_admission_and_clear_resets_metadata_only() {
    let mut app = application(Default::default());
    app.set_cache_policy(CandidateRoutedCachePolicy::EvictAgedLeastFrequentlyUsed)
        .unwrap();
    let mut pins = DependencyPins {
        census: Some(Default::default()),
        ..Default::default()
    };
    let mut stats = CandidateRoutedCacheStatistics::default();
    let node = Node::Route(key([1]));
    let guard = app.family().coefficient_context().integer(7);
    insert(
        &mut app,
        node.clone(),
        vec![guard.clone()],
        &mut pins,
        &mut stats,
    )
    .unwrap();
    app.pin_dependency(&node, &mut pins).unwrap();
    app.reuse_cache(&node).unwrap();
    assert_eq!(app.cache[&node].eviction.reuse_count, 1);
    insert(
        &mut app,
        node.clone(),
        vec![guard.clone(), guard],
        &mut pins,
        &mut stats,
    )
    .unwrap();
    assert_eq!(app.cache[&node].eviction.reuse_count, 0);
    assert_eq!(
        pins.census.unwrap(),
        CandidateRoutedCacheCensus::new(1, app.cache[&node].weight)
    );
    app.release_request_pins(&mut pins);
    let one = app.family().coefficient_context().one();
    app.reduce_linear_combination([(key([3]), one.clone())])
        .unwrap();
    assert_eq!(app.cache[&Node::Route(key([3]))].eviction.reuse_count, 0);
    app.reduce_linear_combination([(key([3]), one.clone())])
        .unwrap();
    assert_eq!(app.cache[&Node::Route(key([3]))].eviction.reuse_count, 1);
    let work = app.cache_work_statistics();
    app.clear_cache().unwrap();
    assert_eq!(app.eviction_index.age, 0);
    assert_eq!(app.eviction_index.clock, 0);
    assert!(app.eviction_index.eligible.is_empty());
    assert_eq!(app.cache_work_statistics(), work);
    app.reduce_linear_combination([(key([3]), one)]).unwrap();
    assert_eq!(app.cache[&Node::Route(key([3]))].eviction.reuse_count, 0);
    assert_census(&app);
}

#[test]
fn aged_lfu_counter_and_admission_overflow_preserve_payload_and_index() {
    let mut app = application(Default::default());
    app.set_cache_policy(CandidateRoutedCachePolicy::EvictAgedLeastFrequentlyUsed)
        .unwrap();
    let mut pins = DependencyPins::default();
    let mut stats = CandidateRoutedCacheStatistics::default();
    let node = Node::Route(key([1]));
    insert(&mut app, node.clone(), vec![], &mut pins, &mut stats).unwrap();
    app.cache.get_mut(&node).unwrap().eviction.reuse_count = u64::MAX;
    let metadata = app.cache[&node].eviction;
    let index = app.eviction_index.eligible.clone();
    assert!(app.reuse_cache(&node).is_err());
    assert_eq!(app.cache[&node].eviction, metadata);
    assert_eq!(app.eviction_index.eligible, index);
    app.eviction_index.age = u64::MAX;
    let census = app.cache_budget.snapshot();
    assert!(
        insert(
            &mut app,
            Node::Route(key([2])),
            vec![],
            &mut pins,
            &mut stats
        )
        .is_err()
    );
    assert_eq!(app.cache_budget.snapshot(), census);
    assert_eq!(app.eviction_index.eligible, index);
    assert_eq!(stats.evicted_entries, 0);
    app.clear_cache().unwrap();
    assert_eq!(app.eviction_index.age, 0);
    assert_census(&app);
}

#[test]
fn observed_pin_census_counts_shared_guards_and_references_once() {
    let mut app = application(Default::default());
    app.set_cache_policy(CandidateRoutedCachePolicy::EvictLeastRecentlyUsed)
        .unwrap();
    let mut pins = DependencyPins {
        census: Some(Default::default()),
        ..Default::default()
    };
    let mut stats = CandidateRoutedCacheStatistics::default();
    let first = Node::Route(key([1]));
    let second = Node::Route(key([2]));
    let guard = app.family().coefficient_context().integer(7);
    insert(
        &mut app,
        first.clone(),
        vec![guard.clone()],
        &mut pins,
        &mut stats,
    )
    .unwrap();
    app.pin_dependency(&first, &mut pins).unwrap();
    app.pin_dependency(&first, &mut pins).unwrap();
    app.pin_dependency(&second, &mut pins).unwrap();
    assert_eq!(pins.census.unwrap().entries, 1);
    insert(
        &mut app,
        second.clone(),
        vec![guard.clone()],
        &mut pins,
        &mut stats,
    )
    .unwrap();
    insert(
        &mut app,
        second.clone(),
        vec![guard.clone(), guard],
        &mut pins,
        &mut stats,
    )
    .unwrap();
    let mut expected = CandidateRoutedCacheCensus::default();
    for (node, entry) in &app.cache {
        if pins.nodes.contains_key(node) {
            expected.retain(entry.weight);
        }
    }
    assert_eq!(pins.census, Some(expected));
    app.release_dependency(&first, &mut pins).unwrap();
    assert_eq!(pins.census, Some(expected));
    app.release_dependency(&first, &mut pins).unwrap();
    assert_eq!(
        pins.census.unwrap(),
        CandidateRoutedCacheCensus::new(1, app.cache[&second].weight)
    );
    app.release_request_pins(&mut pins);
    assert_eq!(pins.census, Some(Default::default()));
    assert_eq!(app.eviction_index.eligible.len(), app.cache.len());
}

#[test]
fn default_fails_but_opt_in_preserves_shared_siblings_and_warm_root() {
    let limits = ReductionLimits {
        diagnostics_enabled: true,
        max_cached_integrals: 2,
        ..Default::default()
    };
    let mut default = application(limits);
    assert_eq!(
        default.cache_policy(),
        CandidateRoutedCachePolicy::FailOnLimit
    );
    let one = default.family().coefficient_context().one();
    assert!(matches!(
        default.reduce_linear_combination([(key([3]), one.clone())]),
        Err(CandidateRoutedError::Candidate(
            CandidateReductionError::Application(ReductionError::CacheLimit { .. })
        ))
    ));
    assert_eq!(
        default.cache_work_statistics(),
        CandidateRoutedCacheStatistics::default()
    );

    for representation in [
        CandidateCacheRepresentation::Sparse,
        CandidateCacheRepresentation::Factorized,
    ] {
        let mut app = application(limits);
        app.set_cache_policy(CandidateRoutedCachePolicy::EvictLeastRecentlyUsed)
            .unwrap();
        app.set_cache_representation(representation).unwrap();
        let expected = application(Default::default())
            .reduce_linear_combination([(key([3]), one.clone())])
            .unwrap();
        let actual = app
            .reduce_linear_combination([(key([3]), one.clone())])
            .unwrap();
        assert_eq!(actual.terms(), expected.terms());
        assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
        assert_eq!(actual.statistics().work.rule_applications(), 2);
        assert_eq!(actual.statistics().cache.evicted_entries, 2);
        assert_eq!(actual.statistics().cache.recomputed_nodes, 0);
        assert_census(&app);
        assert_eq!(app.eviction_index.eligible.len(), app.cache.len());
        let warm = app
            .reduce_linear_combination([(key([3]), one.clone())])
            .unwrap();
        assert_eq!(warm.terms(), actual.terms());
        assert_eq!(warm.nonzero_conditions(), actual.nonzero_conditions());
        assert_eq!(warm.statistics().work.rule_applications(), 0);
        assert_eq!(warm.statistics().work.cache_hits(), 1);
        assert_eq!(
            warm.statistics().cache,
            CandidateRoutedCacheStatistics::default()
        );
        assert!(
            app.set_cache_policy(CandidateRoutedCachePolicy::FailOnLimit)
                .is_err()
        );
        let cumulative = app.cache_work_statistics();
        let owners = app.owner_preparation_statistics();
        app.clear_cache().unwrap();
        assert_census(&app);
        assert_eq!(app.cache_work_statistics(), cumulative);
        assert_eq!(app.owner_preparation_statistics(), owners);
        app.set_cache_policy(CandidateRoutedCachePolicy::FailOnLimit)
            .unwrap();
    }
}

#[test]
fn pinned_working_set_failure_releases_all_pins_for_the_next_request() {
    let mut app = application(ReductionLimits {
        diagnostics_enabled: true,
        max_cached_integrals: 1,
        ..Default::default()
    });
    app.set_cache_policy(CandidateRoutedCachePolicy::EvictLeastRecentlyUsed)
        .unwrap();
    let one = app.family().coefficient_context().one();
    assert!(matches!(
        app.reduce_linear_combination([(key([3]), one.clone())]),
        Err(CandidateRoutedError::Candidate(
            CandidateReductionError::Application(ReductionError::CacheLimit {
                requested: 2,
                limit: 1
            })
        ))
    ));
    assert_eq!(app.cache.len(), 1);
    assert_eq!(app.eviction_index.eligible.len(), 1);
    assert_census(&app);
    let next = app
        .reduce_linear_combination([(key([1]), one.clone())])
        .unwrap();
    assert_eq!(next.terms(), &BTreeMap::from([(key([1]), one)]));
    assert_eq!(next.statistics().cache.evicted_entries, 1);
    assert_census(&app);
}

#[test]
fn evicted_dependencies_recompute_exactly_and_spend_the_same_work_grant() {
    for policy in [
        CandidateRoutedCachePolicy::EvictLeastRecentlyUsed,
        CandidateRoutedCachePolicy::EvictAgedLeastFrequentlyUsed,
    ] {
        let limits = ReductionLimits {
            diagnostics_enabled: true,
            max_cached_integrals: 2,
            ..Default::default()
        };
        let mut app = application(limits);
        app.set_cache_policy(policy).unwrap();
        let one = app.family().coefficient_context().one();
        let inputs = [(key([3]), one.clone()), (key([4]), one.clone())];
        let result = app.reduce_linear_combination(inputs.clone()).unwrap();
        let expected = application(Default::default())
            .reduce_linear_combination(inputs.clone())
            .unwrap();
        assert_eq!(result.terms(), expected.terms());
        assert_eq!(result.nonzero_conditions(), expected.nonzero_conditions());
        assert_eq!(expected.statistics().work.rule_applications(), 3);
        assert_eq!(result.statistics().work.rule_applications(), 4);
        assert_eq!(result.statistics().cache.recomputed_nodes, 2);
        assert_census(&app);
        let mut bounded = application(ReductionLimits {
            diagnostics_enabled: true,
            max_rule_applications: 3,
            ..limits
        });
        bounded.set_cache_policy(policy).unwrap();
        assert!(matches!(
            bounded.reduce_linear_combination(inputs),
            Err(CandidateRoutedError::Candidate(
                CandidateReductionError::Application(ReductionError::RuleApplicationLimit {
                    requested: 4,
                    limit: 3
                })
            ))
        ));
        assert!(bounded.cache_work_statistics().evicted_entries > 0);
        assert_eq!(bounded.eviction_index.eligible.len(), bounded.cache.len());
        assert_census(&bounded);
    }
}

fn insert(
    app: &mut RoutedCandidateApplication<1>,
    node: Node<1>,
    conditions: Vec<Coefficient>,
    pins: &mut DependencyPins<1>,
    stats: &mut CandidateRoutedCacheStatistics,
) -> Result<(), CandidateRoutedError> {
    app.cache_insert(
        node,
        CachedTerms::Sparse(BTreeMap::new()),
        conditions,
        pins,
        stats,
    )
}

#[test]
fn exact_lru_touches_and_reference_counts_preserve_route_apply_identity() {
    let mut app = application(ReductionLimits {
        diagnostics_enabled: true,
        max_cached_integrals: 2,
        ..Default::default()
    });
    app.set_cache_policy(CandidateRoutedCachePolicy::EvictLeastRecentlyUsed)
        .unwrap();
    let mut pins = DependencyPins::default();
    let mut stats = CandidateRoutedCacheStatistics::default();
    let route = Node::Route(key([1]));
    let apply = Node::Apply([true], key([1]));
    let other = Node::Route(key([2]));
    insert(&mut app, route.clone(), vec![], &mut pins, &mut stats).unwrap();
    insert(&mut app, apply.clone(), vec![], &mut pins, &mut stats).unwrap();
    app.touch_cache(&route).unwrap();
    insert(&mut app, other.clone(), vec![], &mut pins, &mut stats).unwrap();
    assert!(app.cache.contains_key(&route));
    assert!(!app.cache.contains_key(&apply));
    assert_eq!(stats.evicted_entries, 1);
    app.pin_dependency(&route, &mut pins).unwrap();
    app.pin_dependency(&route, &mut pins).unwrap();
    app.release_dependency(&route, &mut pins).unwrap();
    assert_eq!(pins.nodes[&route], 1);
    insert(&mut app, apply.clone(), vec![], &mut pins, &mut stats).unwrap();
    assert!(app.cache.contains_key(&route));
    assert!(!app.cache.contains_key(&other));
    app.release_dependency(&route, &mut pins).unwrap();
    assert!(pins.nodes.is_empty());
    assert_eq!(app.eviction_index.eligible.len(), 2);
    assert_census(&app);
}

#[test]
fn guards_in_cached_zeros_are_charged_and_released_without_duplicates() {
    let mut sizing = application(Default::default());
    let condition = sizing
        .family()
        .coefficient_context()
        .coefficient_fixture("d-4");
    let weight = coefficient_cache_weight(std::slice::from_ref(&condition)).unwrap();
    let mut pins = DependencyPins::default();
    for limits in [
        ReductionLimits {
            diagnostics_enabled: true,
            max_cached_coefficient_terms: weight.coefficient_terms,
            ..Default::default()
        },
        ReductionLimits {
            diagnostics_enabled: true,
            max_cached_coefficient_bytes: weight.coefficient_bytes,
            ..Default::default()
        },
    ] {
        sizing = application(limits);
        sizing
            .set_cache_policy(CandidateRoutedCachePolicy::EvictLeastRecentlyUsed)
            .unwrap();
        let mut stats = CandidateRoutedCacheStatistics::default();
        let first = Node::Route(key([1]));
        let second = Node::Route(key([2]));
        insert(
            &mut sizing,
            first.clone(),
            vec![condition.clone()],
            &mut pins,
            &mut stats,
        )
        .unwrap();
        // Native polynomial cloning can trim spare allocation capacity. The
        // eviction receipt must count the installed payload, not the fixture's
        // original allocation (whose size only set the operational allowance).
        let installed_weight = coefficient_cache_weight(&sizing.cache[&first].conditions).unwrap();
        assert_eq!(sizing.cache[&first].weight, installed_weight);
        insert(
            &mut sizing,
            second.clone(),
            vec![condition.clone()],
            &mut pins,
            &mut stats,
        )
        .unwrap();
        assert!(!sizing.cache.contains_key(&first));
        assert_eq!(sizing.cache[&second].conditions, [condition.clone()]);
        assert_eq!(stats.evicted_entries, 1);
        assert_eq!(
            stats.evicted_coefficient_terms,
            installed_weight.coefficient_terms
        );
        assert_eq!(
            stats.evicted_coefficient_bytes,
            installed_weight.coefficient_bytes
        );
        assert_census(&sizing);
        // A result too large by itself must not discard the useful old zero.
        let before = sizing.cache_budget.snapshot();
        assert!(
            insert(
                &mut sizing,
                first,
                vec![condition.clone(), condition.clone()],
                &mut pins,
                &mut stats
            )
            .is_err()
        );
        assert_eq!(sizing.cache_budget.snapshot(), before);
        assert_eq!(stats.evicted_entries, 1);
        assert_census(&sizing);
    }
}

#[test]
fn guarded_cancellation_recomputes_after_eviction_and_keeps_original_poles() {
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
    let owners = programs(
        family.clone(),
        None,
        vec![input([true], None, vec![three, two], &[[1]])],
        ReductionLimits {
            diagnostics_enabled: true,
            max_cached_integrals: 2,
            ..Default::default()
        },
    );
    let routed = RoutedCandidateReducer::try_new(owners, [], Default::default()).unwrap();
    for policy in [
        CandidateRoutedCachePolicy::EvictLeastRecentlyUsed,
        CandidateRoutedCachePolicy::EvictAgedLeastFrequentlyUsed,
    ] {
        for representation in [
            CandidateCacheRepresentation::Sparse,
            CandidateCacheRepresentation::Factorized,
        ] {
            let mut app = routed.application();
            app.set_cache_policy(policy).unwrap();
            app.set_cache_representation(representation).unwrap();
            let zero = app
                .reduce_linear_combination([(key([3]), c.one())])
                .unwrap();
            assert!(zero.is_zero());
            assert!(
                zero.nonzero_conditions()
                    .contains(&c.coefficient_fixture("d-4"))
            );
            assert!(
                zero.nonzero_conditions()
                    .contains(&c.coefficient_fixture("d-3"))
            );
            let warm = app
                .reduce_linear_combination([(key([3]), c.one())])
                .unwrap();
            assert_eq!(warm.nonzero_conditions(), zero.nonzero_conditions());
            assert_eq!(warm.statistics().work.rule_applications(), 0);
            app.reduce_linear_combination([(key([2]), c.one())])
                .unwrap();
            assert!(!app.cache.contains_key(&Node::Route(key([3]))));
            let again = app
                .reduce_linear_combination([(key([3]), c.one())])
                .unwrap();
            assert_eq!(again.terms(), zero.terms());
            assert_eq!(again.nonzero_conditions(), zero.nonzero_conditions());
            assert!(again.statistics().work.rule_applications() > 0);
            assert_census(&app);
        }
    }
}

#[test]
fn scheduling_failure_cleans_pins_and_access_sequence_fails_closed() {
    let mut app = application(ReductionLimits {
        diagnostics_enabled: true,
        max_pending_frames: 2,
        ..Default::default()
    });
    app.set_cache_policy(CandidateRoutedCachePolicy::EvictLeastRecentlyUsed)
        .unwrap();
    let one = app.family().coefficient_context().one();
    app.reduce_linear_combination([(key([1]), one.clone())])
        .unwrap();
    assert!(matches!(
        app.reduce_linear_combination([(key([3]), one)]),
        Err(CandidateRoutedError::Candidate(
            CandidateReductionError::Application(ReductionError::PendingFrameLimit { .. })
        ))
    ));
    assert_eq!(app.eviction_index.eligible.len(), app.cache.len());
    assert_census(&app);
    app.eviction_index.clock = u64::MAX;
    let error = insert(
        &mut app,
        Node::Route(key([1])),
        vec![],
        &mut DependencyPins::default(),
        &mut CandidateRoutedCacheStatistics::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        CandidateRoutedError::Candidate(CandidateReductionError::Application(
            ReductionError::CacheResourceCountOverflow {
                resource: "completed cache access sequence"
            }
        ))
    ));
    assert_census(&app);
}

#[test]
fn entry_admission_still_precedes_warm_lookup_and_input_cancellation() {
    use crate::solver::{
        CandidateEntryAdmission, DomainPowerBounds, FiniteRootAdmission, RootRegionInput,
    };
    let mut app = application(ReductionLimits {
        diagnostics_enabled: true,
        max_cached_integrals: 2,
        ..Default::default()
    });
    app.set_cache_policy(CandidateRoutedCachePolicy::EvictLeastRecentlyUsed)
        .unwrap();
    let one = app.family().coefficient_context().one();
    let negative_one = app.family().coefficient_context().integer(-1);
    app.reduce_linear_combination([(key([3]), one.clone())])
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
    for terms in [
        vec![(key([3]), one.clone())],
        vec![(key([3]), one), (key([3]), negative_one)],
    ] {
        assert!(matches!(
            app.reduce_linear_combination_with_entry_admission(
                terms,
                CandidateEntryAdmission::ExplicitFinite(&admission)
            ),
            Err(CandidateRoutedError::EntryAdmission(_))
        ));
        assert_eq!(app.eviction_index.eligible.len(), app.cache.len());
        assert_census(&app);
    }
}

#[test]
fn evicting_cache_never_turns_an_uncovered_target_into_a_terminal() {
    let mut app = application(ReductionLimits {
        diagnostics_enabled: true,
        max_cached_integrals: 1,
        ..Default::default()
    });
    app.set_cache_policy(CandidateRoutedCachePolicy::EvictLeastRecentlyUsed)
        .unwrap();
    let one = app.family().coefficient_context().one();
    app.reduce_linear_combination([(key([1]), one.clone())])
        .unwrap();
    assert!(matches!(app.reduce_linear_combination([(key([5]), one)]),
        Err(CandidateRoutedError::Candidate(CandidateReductionError::Uncovered { target })) if target == key([5])));
    assert_eq!(app.eviction_index.eligible.len(), app.cache.len());
    assert_census(&app);
}
