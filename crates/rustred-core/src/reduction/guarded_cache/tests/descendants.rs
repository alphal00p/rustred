use super::*;
use std::collections::HashSet;

fn guard_set(record: &ApplicationCacheRecord) -> HashSet<crate::algebra::Coefficient> {
    record.nonzero_conditions.iter().cloned().collect()
}

#[test]
fn prepared_four_reduces_different_five_with_one_native_rule_for_both_policies() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let program = ClosedUnitMassProgram::new(&artifact, [31; 32]);
    let reference = baseline(&program, 5);
    let one_rule = ReductionLimits {
        diagnostics_enabled: true,
        max_rule_applications: 1,
        ..Default::default()
    };
    assert!(Reducer::reduce_artifact_unit_mass_guarded(&artifact, &key(5), one_rule).is_err());
    for policy in [
        CompletedCachePolicy::EvictLeastRecentlyUsed,
        CompletedCachePolicy::EvictAgedLeastFrequentlyUsed,
    ] {
        let mut cache = GuardedRootCache::with_policy(&program, Default::default(), policy);
        program
            .reduce_guarded_root(&key(4), &mut cache, Default::default(), None)
            .unwrap();
        let validations = cache.statistics().payload_validations;
        let result = program
            .reduce_guarded_root(&key(5), &mut cache, one_rule, None)
            .unwrap();
        assert_eq!(result.source, GuardedRootSource::Reduced);
        assert_eq!(result.work.rule_applications(), 1);
        assert_eq!(result.record.terms, reference.terms);
        assert_eq!(guard_set(&result.record), guard_set(&reference));
        assert_eq!(cache.statistics().descendant_ram_hits, 1);
        assert_eq!(cache.statistics().ram_hits, 0);
        assert_eq!(
            cache.statistics().payload_validations,
            validations + 1,
            "only the newly completed root is validated; the descendant RAM hit reuses its stamp"
        );
        assert_eq!(cache.statistics().cached_integrals, 2);
        assert!(cache.peek(&key(2)).is_none());
        assert!(
            cache.peek(&key(3)).is_none(),
            "guardless inner memo is never published"
        );
        assert_eq!(cache.shared_budget().snapshot().integrals, 2);
    }
}

#[test]
fn disk_prepared_four_is_a_descendant_hit_without_repeating_starting_root_lookup() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let program = ClosedUnitMassProgram::new(&artifact, [32; 32]);
    let mut backing = Backing::new(&program);
    backing.put(baseline(&program, 4));
    let mut cache = GuardedRootCache::new(&program, Default::default());
    let result = program
        .reduce_guarded_root(
            &key(5),
            &mut cache,
            ReductionLimits {
                diagnostics_enabled: true,
                max_rule_applications: 1,
                ..Default::default()
            },
            Some(&mut backing),
        )
        .unwrap();
    assert_eq!(result.record.terms, baseline(&program, 5).terms);
    assert_eq!(result.work.rule_applications(), 1);
    assert_eq!(
        backing.lookups, 2,
        "one miss for I5 and one hit for descendant I4"
    );
    assert_eq!(cache.statistics().backing_hits, 0);
    assert_eq!(cache.statistics().descendant_backing_hits, 1);
    assert_eq!(
        backing.stores, 1,
        "only completed caller root I5 is written"
    );
}

#[test]
fn imported_descendant_guards_keep_order_constants_and_cancelled_obligations() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let program = ClosedUnitMassProgram::new(&artifact, [33; 32]);
    let mut prepared = baseline(&program, 4);
    // A trusted guarded identity may retain stronger original obligations
    // even when they no longer occur in its final rational coefficients.
    let d = program
        .context()
        .parameter(&program.context().parameter_names()[0])
        .unwrap();
    let shifted = program
        .context()
        .try_sub(&d, &program.context().integer(7), Default::default())
        .unwrap();
    prepared.nonzero_conditions = vec![
        shifted.clone(),
        program.context().integer(-2),
        d.clone(),
        shifted.clone(),
    ];
    let mut backing = Backing::new(&program);
    backing.put(prepared);
    let mut cache = GuardedRootCache::new(&program, Default::default());
    let result = program
        .reduce_guarded_root(&key(5), &mut cache, Default::default(), Some(&mut backing))
        .unwrap();
    assert_eq!(result.record.terms, baseline(&program, 5).terms);
    assert_eq!(
        result.record.nonzero_conditions,
        vec![shifted, program.context().integer(-2), d]
    );
}

#[test]
fn external_lookup_cannot_make_an_uncovered_descendant_applicable() {
    let reference_artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let reference = ClosedUnitMassProgram::new(&reference_artifact, [34; 32]);
    let record = std::sync::Arc::new(baseline(&reference, 4));
    let mut artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let guard = {
        let context = artifact.indexed_context();
        let difference = context
            .sub(&context.index(0).unwrap(), &context.integer(3))
            .unwrap();
        context
            .numerator_condition_with_limits(&difference, Default::default())
            .unwrap()
    };
    // Deliberately broken test-only authority: I5 still selects its rule,
    // but I4's specialized guard is zero and has no fallback.
    artifact.replace_first_raw_rule_guard_for_test(guard);
    let mut lookups = 0;
    let mut lookup = |_: &IntegralKey| {
        lookups += 1;
        Ok(Some(std::sync::Arc::clone(&record)))
    };
    let result = Reducer::reduce_artifact_unit_mass_guarded_with_lookup(
        &artifact,
        &key(5),
        Default::default(),
        std::sync::Arc::new(Default::default()),
        &mut lookup,
    );
    assert_eq!(
        result,
        Err(GuardedCacheError::Reduction(
            ReductionError::UncoveredIntegral { target: key(4) }
        ))
    );
    assert_eq!(lookups, 0);
}

#[test]
fn descendant_backing_error_is_not_a_miss_and_releases_native_budget() {
    struct FailDescendant {
        inner: Backing,
    }
    impl GuardedCacheBacking for FailDescendant {
        fn binding(&self) -> &ApplicationCacheBinding {
            self.inner.binding()
        }
        fn lookup(
            &mut self,
            key: &ApplicationCacheKey,
            context: &CoefficientContext,
        ) -> Result<Option<ApplicationCacheRecord>, GuardedCacheBackingError> {
            if key.powers() == [4] {
                Err(GuardedCacheBackingError {
                    kind: GuardedCacheBackingErrorKind::Limit,
                    message: "bounded descendant read refused".into(),
                })
            } else {
                self.inner.lookup(key, context)
            }
        }
        fn store_completed(
            &mut self,
            record: &ApplicationCacheRecord,
            context: &CoefficientContext,
        ) -> Result<(), GuardedCacheBackingError> {
            self.inner.store_completed(record, context)
        }
    }
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let program = ClosedUnitMassProgram::new(&artifact, [35; 32]);
    let mut cache = GuardedRootCache::new(&program, Default::default());
    let mut backing = FailDescendant {
        inner: Backing::new(&program),
    };
    assert!(matches!(
        program.reduce_guarded_root(&key(5), &mut cache, Default::default(), Some(&mut backing)),
        Err(GuardedCacheError::Backing(GuardedCacheBackingError {
            kind: GuardedCacheBackingErrorKind::Limit,
            ..
        }))
    ));
    assert_eq!(backing.inner.stores, 0);
    assert_eq!(cache.statistics().completed_reductions, 0);
    assert_eq!(
        cache.shared_budget().snapshot(),
        crate::reduction::CacheCensus::default()
    );
    assert_eq!(
        program
            .reduce_guarded_root(&key(5), &mut cache, Default::default(), None)
            .unwrap()
            .record,
        baseline(&program, 5)
    );
}

#[test]
fn inadmissible_entry_descendant_never_queries_external_cache_or_clips_internal_work() {
    use crate::sector::Mask;
    let artifact = derive_one_loop_unit_mass_tadpole()
        .unwrap()
        .with_total_excess_scope_for_test(1, BTreeMap::from([(Mask::try_new([true]).unwrap(), 5)]))
        .unwrap();
    let program = ClosedUnitMassProgram::new(&artifact, [36; 32]);
    let mut cache = GuardedRootCache::new(&program, Default::default());
    let record = ApplicationCacheRecord {
        key: ApplicationCacheKey::guarded_root(&key(3)),
        terms: BTreeMap::from([(key(1), program.context().one())]),
        nonzero_conditions: Vec::new(),
    };
    cache
        .insert(key(3), record.clone(), Default::default())
        .unwrap();
    let mut backing = Backing::new(&program);
    backing.put(record);
    let before = cache.statistics();
    let mut store = Some(&mut backing as &mut dyn GuardedCacheBacking);
    assert!(
        program
            .lookup_completed(
                &key(3),
                &mut cache,
                Default::default(),
                &mut store,
                RootUse::Descendant
            )
            .unwrap()
            .is_none()
    );
    assert_eq!(cache.statistics(), before);
    assert_eq!(backing.lookups, 0);
    assert!(matches!(
        program.reduce_guarded_root(&key(3), &mut cache, Default::default(), None),
        Err(GuardedCacheError::Reduction(
            ReductionError::OutsideCertifiedTotalExcessDomain { maximum: 1 }
        ))
    ));
    // Native internal descent outside an entry scope is separately covered by
    // reduction::reducer::scoped_tests; returning None preserves that path.
}

#[test]
fn failed_parent_preserves_only_valid_completed_roots_and_releases_import_census() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let program = ClosedUnitMassProgram::new(&artifact, [37; 32]);
    let mut cache = GuardedRootCache::new(&program, Default::default());
    let prepared = program
        .reduce_guarded_root(&key(4), &mut cache, Default::default(), None)
        .unwrap();
    // I4 has four numerator terms and fits this exact-algebra allowance;
    // multiplying by I5's recurrence produces a larger polynomial. A zero
    // coalescing allowance is not a failure trigger for one vacant master.
    assert_eq!(prepared.record.terms[&key(1)].numerator.nterms(), 4);
    let mut constrained = ReductionLimits::default();
    constrained.diagnostics_enabled = true;
    constrained.exact_algebra.max_polynomial_terms = 4;
    assert!(matches!(
        program.reduce_guarded_root(&key(5), &mut cache, constrained, None),
        Err(GuardedCacheError::Reduction(ReductionError::ExactAlgebra(
            crate::algebra::ExactAlgebraError::ResourceLimit { limit: 4, .. }
        )))
    ));
    assert!(cache.peek(&key(5)).is_none());
    assert_eq!(cache.peek(&key(4)).unwrap(), baseline(&program, 4));
    assert_eq!(cache.statistics().descendant_ram_hits, 1);
    let total = cache.shared_budget().snapshot();
    assert_eq!(total.integrals, cache.statistics().cached_integrals);
    assert_eq!(
        total.coefficient_bytes,
        cache.statistics().cached_coefficient_bytes
    );
    assert_eq!(
        program
            .reduce_guarded_root(&key(5), &mut cache, Default::default(), None)
            .unwrap()
            .record,
        baseline(&program, 5)
    );
}
