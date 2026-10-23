use super::persistent::CandidateRoutedPersistentCacheStatistics;
use super::*;
use crate::algebra::CoefficientContext;
use crate::persistence::application_cache::{
    ApplicationCacheBinding, ApplicationCacheKey, ApplicationCacheRecord,
};
use crate::reduction::{GuardedCacheBacking, GuardedCacheBackingError, GuardedIntermediateCapture};
use crate::solver::candidate_reduction::owner_test_support::*;
use crate::solver::{CandidateEntryAdmission, UnresolvedPolicy};

fn polynomial(
    context: &crate::algebra::IndexedCoefficientContext,
    text: &str,
) -> crate::algebra::IndexedPolynomial {
    let value = context
        .lift(&context.base().coefficient_fixture(text))
        .unwrap();
    context
        .admit_native_polynomial_result_with_limits(
            value.raw().numerator.clone(),
            Default::default(),
        )
        .unwrap()
}

struct Store {
    binding: ApplicationCacheBinding,
    records: BTreeMap<ApplicationCacheKey, ApplicationCacheRecord>,
}
impl GuardedCacheBacking for Store {
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
impl GuardedIntermediateCapture for Store {
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
        context: &CoefficientContext,
    ) -> Result<(), GuardedCacheBackingError> {
        GuardedCacheBacking::store_completed(self, record, context)
    }
}
fn fixture() -> RoutedCandidateReducer<1> {
    let family = Arc::new(crate::solver::tests::tadpole());
    let mut owners = CandidateOwnerPrograms::try_new(
        context(family.clone(), None, Default::default()),
        [input(
            [true],
            None,
            vec![
                rule(&family, [5], &[([4], 2)]),
                rule(&family, [4], &[([3], 2)]),
                rule(&family, [3], &[([2], -3), ([1], 2)]),
                rule(&family, [2], &[([1], 2)]),
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
fn persistent_completed_nodes_reuse_held_out_descendants_with_exact_guards() {
    let source = fixture();
    let c = source.programs.context.family().coefficient_context();
    let mut prepared = source.application();
    let binding = prepared.persistent_cache_binding([7; 32]);
    let mut store = Store {
        binding: binding.clone(),
        records: Default::default(),
    };
    prepared
        .reduce_linear_combination_with_cache(
            [(key([3]), c.one())],
            CandidateEntryAdmission::SavedGenerationScope,
            &binding,
            None,
            Some(&mut store),
        )
        .unwrap();
    assert!(
        store
            .records
            .contains_key(&ApplicationCacheKey::apply(vec![true], &key([2])))
    );
    assert!(
        !store
            .records
            .contains_key(&ApplicationCacheKey::route(&key([4])))
    );
    let cold = source
        .application()
        .reduce_linear_combination([(key([4]), c.one())])
        .unwrap();
    let warm = source
        .application()
        .reduce_linear_combination_with_cache(
            [(key([4]), c.one())],
            CandidateEntryAdmission::SavedGenerationScope,
            &binding,
            Some(&mut store),
            None,
        )
        .unwrap();
    assert_eq!(cold.terms(), warm.terms());
    assert_eq!(cold.nonzero_conditions(), warm.nonzero_conditions());
    assert_eq!(warm.statistics().work.rule_applications(), 1);
    assert_eq!(warm.statistics().persistent_cache.intermediate_hits, 1);
    assert_eq!(warm.statistics().persistent_cache.requested_root_hits, 0);
    let repeated = source
        .application()
        .reduce_linear_combination_with_cache(
            [(key([3]), c.one())],
            CandidateEntryAdmission::SavedGenerationScope,
            &binding,
            Some(&mut store),
            None,
        )
        .unwrap();
    assert_eq!(repeated.statistics().work.rule_applications(), 0);
    assert_eq!(
        repeated.statistics().persistent_cache.requested_root_hits,
        1
    );
    assert_eq!(repeated.statistics().persistent_cache.intermediate_hits, 0);
    assert_eq!(
        repeated.nonzero_conditions(),
        &[c.coefficient_fixture("d-7")]
    );
}
#[test]
fn persistent_capture_rejects_permissive_sessions_and_foreign_or_incomplete_records() {
    let source = fixture();
    let c = source.programs.context.family().coefficient_context();
    let mut app = source.application();
    let binding = app.persistent_cache_binding([5; 32]);
    let mut store = Store {
        binding: binding.clone(),
        records: Default::default(),
    };
    app.set_unresolved_policy(UnresolvedPolicy::Preserve)
        .unwrap();
    assert!(
        app.reduce_linear_combination_with_cache(
            [(key([4]), c.one())],
            CandidateEntryAdmission::SavedGenerationScope,
            &binding,
            None,
            Some(&mut store)
        )
        .is_err()
    );
    assert!(store.records.is_empty());
    assert_eq!(app.export_completed(&binding, &mut store).unwrap(), 0);
    store.binding.artifact_digest = [6; 32];
    assert!(
        app.reduce_linear_combination_with_cache(
            [(key([3]), c.one())],
            CandidateEntryAdmission::SavedGenerationScope,
            &binding,
            Some(&mut store),
            None
        )
        .is_err()
    );
    store.binding = binding.clone();
    let target = ApplicationCacheKey::route(&key([3]));
    store.records.insert(
        target.clone(),
        ApplicationCacheRecord {
            key: target.clone(),
            terms: BTreeMap::from([(key([6]), c.one())]),
            nonzero_conditions: vec![],
        },
    );
    assert!(
        source
            .application()
            .reduce_linear_combination_with_cache(
                [(key([3]), c.one())],
                CandidateEntryAdmission::SavedGenerationScope,
                &binding,
                Some(&mut store),
                None
            )
            .unwrap_err()
            .to_string()
            .contains("undeclared")
    );
    store.records.get_mut(&target).unwrap().terms = BTreeMap::from([(key([1]), c.one())]);
    store.records.get_mut(&target).unwrap().nonzero_conditions = vec![c.zero()];
    assert!(
        source
            .application()
            .reduce_linear_combination_with_cache(
                [(key([3]), c.one())],
                CandidateEntryAdmission::SavedGenerationScope,
                &binding,
                Some(&mut store),
                None
            )
            .unwrap_err()
            .to_string()
            .contains("vanished")
    );
}
#[test]
fn persistent_export_and_native_record_validation_share_authority() {
    let source = fixture();
    let c = source.programs.context.family().coefficient_context();
    let mut app = source.application();
    let binding = app.persistent_cache_binding([4; 32]);
    app.reduce_linear_combination([(key([3]), c.one())])
        .unwrap();
    let mut store = Store {
        binding: binding.clone(),
        records: Default::default(),
    };
    let count = app.export_completed(&binding, &mut store).unwrap();
    assert_eq!(count, store.records.len());
    assert!(count > 3);
    for record in store.records.values() {
        app.validate_completed_cache_record(&binding, record)
            .unwrap();
    }
    let mut wrong_stage = store.records.values().next().unwrap().clone();
    wrong_stage.key = ApplicationCacheKey::guarded_root(&key([3]));
    assert!(
        app.validate_completed_cache_record(&binding, &wrong_stage)
            .is_err()
    );
    assert_eq!(
        CandidateRoutedPersistentCacheStatistics::default().captured_records,
        0
    );
}

#[test]
fn persistent_export_omits_unknown_nodes_but_preserves_exact_cancelled_parents() {
    let family = Arc::new(crate::solver::tests::tadpole());
    let c = family.coefficient_context();
    let owners = programs(
        family.clone(),
        None,
        vec![input(
            [true],
            None,
            vec![
                rule(&family, [4], &[([3], 1), ([2], 3), ([1], -2)]),
                rule(&family, [3], &[([2], -3), ([1], 2)]),
            ],
            &[[1]],
        )],
        Default::default(),
    );
    let source = RoutedCandidateReducer::try_new(owners, [], Default::default()).unwrap();
    let mut app = source.application();
    app.set_unresolved_policy(UnresolvedPolicy::Preserve)
        .unwrap();
    let binding = app.persistent_cache_binding([4; 32]);
    let partial = app
        .reduce_linear_combination([(key([3]), c.one())])
        .unwrap();
    assert_eq!(
        partial.unresolved(),
        &std::collections::BTreeSet::from([key([2])])
    );
    let cancelled = app
        .reduce_linear_combination([(key([4]), c.one())])
        .unwrap();
    assert!(cancelled.is_zero());
    let mut store = Store {
        binding: binding.clone(),
        records: Default::default(),
    };
    let statistics = app
        .export_completed_with_statistics(&binding, &mut store)
        .unwrap();
    assert!(statistics.skipped_partial_records > 0);
    assert!(statistics.exported_records > 0);
    for record in store.records.values() {
        assert!(!record.terms.contains_key(&key([2])));
        app.validate_completed_cache_record(&binding, record)
            .unwrap();
    }
    assert!(
        source
            .application()
            .reduce_linear_combination_with_cache(
                [(key([3]), c.one())],
                CandidateEntryAdmission::SavedGenerationScope,
                &binding,
                Some(&mut store),
                None
            )
            .is_err()
    );
    let exact_zero = source
        .application()
        .reduce_linear_combination_with_cache(
            [(key([4]), c.one())],
            CandidateEntryAdmission::SavedGenerationScope,
            &binding,
            Some(&mut store),
            None,
        )
        .unwrap();
    assert!(exact_zero.is_zero());
    assert_eq!(
        exact_zero.statistics().persistent_cache.requested_root_hits,
        1
    );
}

#[test]
fn persistent_captured_descendant_keeps_only_its_own_subtree_guards() {
    let mut source = fixture();
    let programs = Arc::get_mut(&mut source.programs).unwrap();
    let indexed = programs.context.coefficient_context().clone();
    let d = indexed
        .lift(&indexed.base().parameter("d").unwrap())
        .unwrap();
    let condition = indexed.add(&d, &indexed.index(0).unwrap()).unwrap();
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
    let c = source.programs.context.family().coefficient_context();
    let mut app = source.application();
    let binding = app.persistent_cache_binding([9; 32]);
    let mut store = Store {
        binding: binding.clone(),
        records: Default::default(),
    };
    app.reduce_linear_combination_with_cache(
        [(key([4]), c.one())],
        CandidateEntryAdmission::SavedGenerationScope,
        &binding,
        None,
        Some(&mut store),
    )
    .unwrap();
    let intermediate = store
        .records
        .get(&ApplicationCacheKey::apply(vec![true], &key([3])))
        .unwrap()
        .clone();
    for text in ["d+1", "d+2", "d+3"] {
        assert!(
            intermediate
                .nonzero_conditions
                .contains(&c.coefficient_fixture(text))
        );
    }
    assert!(
        !intermediate
            .nonzero_conditions
            .contains(&c.coefficient_fixture("d+4"))
    );
    let cold = source
        .application()
        .reduce_linear_combination([(key([5]), c.one())])
        .unwrap();
    let warm = source
        .application()
        .reduce_linear_combination_with_cache(
            [(key([5]), c.one())],
            CandidateEntryAdmission::SavedGenerationScope,
            &binding,
            Some(&mut store),
            None,
        )
        .unwrap();
    assert_eq!(cold.terms(), warm.terms());
    assert_eq!(cold.nonzero_conditions(), warm.nonzero_conditions());
    let mut malformed = intermediate.clone();
    malformed
        .nonzero_conditions
        .retain(|g| g != &c.coefficient_fixture("d+3"));
    assert!(
        app.validate_completed_cache_record(&binding, &malformed)
            .unwrap_err()
            .to_string()
            .contains("source condition")
    );
}

#[test]
fn persistent_panicking_callbacks_release_dependency_pins() {
    struct PanicCapture;
    impl GuardedIntermediateCapture for PanicCapture {
        fn wants(
            &mut self,
            _: &ApplicationCacheKey,
            _: &CoefficientContext,
        ) -> Result<bool, GuardedCacheBackingError> {
            panic!("test callback");
        }
        fn store_completed(
            &mut self,
            _: &ApplicationCacheRecord,
            _: &CoefficientContext,
        ) -> Result<(), GuardedCacheBackingError> {
            unreachable!()
        }
    }
    let source = fixture();
    let c = source.programs.context.family().coefficient_context();
    for routed in [false, true] {
        let mut app = source.application();
        app.set_cache_policy(CandidateRoutedCachePolicy::EvictLeastRecentlyUsed)
            .unwrap();
        let binding = app.persistent_cache_binding([8; 32]);
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if routed {
                app.reduce_routed_linear_combination_with_cache(
                    [(key([4]), c.one())],
                    CandidateEntryAdmission::SavedGenerationScope,
                    &binding,
                    None,
                    Some(&mut PanicCapture),
                )
            } else {
                app.reduce_linear_combination_with_cache(
                    [(key([4]), c.one())],
                    CandidateEntryAdmission::SavedGenerationScope,
                    &binding,
                    None,
                    Some(&mut PanicCapture),
                )
            }
        }));
        assert!(panic.is_err());
        assert_eq!(app.cache.len(), app.eviction_index.eligible.len());
        assert!(
            app.completed_cache_record(&binding, &ApplicationCacheKey::route(&key([4])))
                .unwrap()
                .is_none()
        );
        let result = app
            .reduce_linear_combination([(key([5]), c.one())])
            .unwrap();
        assert!(result.unresolved().is_empty());
        assert_eq!(app.cache.len(), app.eviction_index.eligible.len());
    }
}

#[test]
fn persistent_merge_preserve_certified_zero_keeps_source_validation_and_limits() {
    for vanished in [false, true] {
        let mut source = fixture();
        Arc::get_mut(&mut Arc::get_mut(&mut source.programs).unwrap().context)
            .unwrap()
            .shared
            .zero_sectors
            .insert([false]);
        if vanished {
            let programs = Arc::get_mut(&mut source.programs).unwrap();
            let indexed = programs.context.coefficient_context().clone();
            Arc::get_mut(&mut programs.context)
                .unwrap()
                .shared
                .source_conditions = vec![
                indexed
                    .admit_native_polynomial_result_with_limits(
                        indexed.index(0).unwrap().raw().numerator.clone(),
                        Default::default(),
                    )
                    .unwrap(),
            ];
        }
        let c = source.programs.context.family().coefficient_context();
        let mut app = source.application();
        app.set_unresolved_policy(UnresolvedPolicy::Preserve)
            .unwrap();
        let result = app.reduce_routed_linear_combination_with_entry_admission(
            [(key([0]), c.one())],
            CandidateEntryAdmission::SavedGenerationScope,
        );
        if vanished {
            assert!(matches!(
                result,
                Err(CandidateRoutedError::Candidate(
                    CandidateReductionError::SourceConditionVanished { .. }
                ))
            ));
        } else {
            let result = result.unwrap();
            assert!(result.is_zero());
            assert!(result.unresolved().is_empty());
            assert_eq!(result.nonzero_conditions(), &[c.coefficient_fixture("d-7")]);
            assert_eq!(result.statistics().transport_calls, 0);
        }
        assert!(
            app.reduce_routed_linear_combination_with_entry_admission(
                [(key([0, 0]), c.one())],
                CandidateEntryAdmission::SavedGenerationScope
            )
            .is_err()
        );
    }
    let mut source = fixture();
    source.limits.max_input_targets = 0;
    let c = source.programs.context.family().coefficient_context();
    let mut app = source.application();
    app.set_unresolved_policy(UnresolvedPolicy::Preserve)
        .unwrap();
    assert!(
        app.reduce_routed_linear_combination_with_entry_admission(
            [(key([0]), c.one())],
            CandidateEntryAdmission::SavedGenerationScope
        )
        .is_err()
    );
}
