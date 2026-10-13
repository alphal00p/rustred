use super::*;
use crate::algebra::CoefficientContext;
use crate::family::IntegralKey;
use crate::foundry::artifact::derive_one_loop_unit_mass_tadpole;
use crate::persistence::application_cache::{
    ApplicationCacheBinding, ApplicationCacheKey, ApplicationCacheRecord,
};
use crate::reduction::{
    CompletedCachePolicy, Reducer, ReductionError, ReductionLimits, coefficient_cache_weight,
};
use std::collections::BTreeMap;

mod descendants;
mod export_tests;
mod statistics;

fn key(n: i64) -> IntegralKey {
    IntegralKey::try_new([n]).unwrap()
}
fn baseline(program: &ClosedUnitMassProgram<'_>, n: i64) -> ApplicationCacheRecord {
    let (unit, nonzero_conditions, _) =
        Reducer::reduce_artifact_unit_mass_guarded(program.artifact(), &key(n), Default::default())
            .unwrap();
    ApplicationCacheRecord {
        key: ApplicationCacheKey::guarded_root(&key(n)),
        terms: unit.terms().clone(),
        nonzero_conditions,
    }
}

struct Backing {
    binding: ApplicationCacheBinding,
    records: BTreeMap<ApplicationCacheKey, ApplicationCacheRecord>,
    lookups: usize,
    stores: usize,
    lookup_error: Option<GuardedCacheBackingError>,
    store_error: Option<GuardedCacheBackingError>,
    override_record: Option<ApplicationCacheRecord>,
}
impl Backing {
    fn new(program: &ClosedUnitMassProgram<'_>) -> Self {
        Self {
            binding: program.binding().clone(),
            records: BTreeMap::new(),
            lookups: 0,
            stores: 0,
            lookup_error: None,
            store_error: None,
            override_record: None,
        }
    }
    fn put(&mut self, record: ApplicationCacheRecord) {
        self.records.insert(record.key.clone(), record);
    }
}
impl GuardedCacheBacking for Backing {
    fn binding(&self) -> &ApplicationCacheBinding {
        &self.binding
    }
    fn lookup(
        &mut self,
        key: &ApplicationCacheKey,
        _: &CoefficientContext,
    ) -> Result<Option<ApplicationCacheRecord>, GuardedCacheBackingError> {
        self.lookups += 1;
        if let Some(error) = &self.lookup_error {
            return Err(error.clone());
        }
        Ok(self
            .override_record
            .clone()
            .or_else(|| self.records.get(key).cloned()))
    }
    fn store_completed(
        &mut self,
        record: &ApplicationCacheRecord,
        _: &CoefficientContext,
    ) -> Result<(), GuardedCacheBackingError> {
        self.stores += 1;
        if let Some(error) = &self.store_error {
            return Err(error.clone());
        }
        self.put(record.clone());
        Ok(())
    }
}

#[test]
fn cold_warm_one_through_four_and_held_out_five_match_native_guarded_results() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let program = ClosedUnitMassProgram::new(&artifact, [7; 32]);
    for policy in [
        CompletedCachePolicy::FailOnLimit,
        CompletedCachePolicy::EvictLeastRecentlyUsed,
        CompletedCachePolicy::EvictAgedLeastFrequentlyUsed,
    ] {
        let mut cache = GuardedRootCache::with_policy(&program, Default::default(), policy);
        for n in 1..=5 {
            let reference = baseline(&program, n);
            let first = program
                .reduce_guarded_root(&key(n), &mut cache, Default::default(), None)
                .unwrap();
            assert_eq!(first.source, GuardedRootSource::Reduced);
            assert_eq!(first.record, reference);
            let warm = program
                .reduce_guarded_root(
                    &key(n),
                    &mut cache,
                    ReductionLimits {
                        max_rule_applications: 0,
                        ..Default::default()
                    },
                    None,
                )
                .unwrap();
            assert_eq!(warm.source, GuardedRootSource::Ram);
            assert_eq!(warm.record, reference);
            assert_eq!(warm.work.rule_applications(), 0);
            assert_eq!(warm.work.coalescing_additions(), 0);
            assert_eq!(warm.work.cache_hits(), 1);
        }
        let stats = cache.statistics();
        assert_eq!(stats.completed_reductions, 5);
        assert_eq!(stats.ram_hits, 5);
        assert_eq!(stats.cached_integrals, 5);
    }
}

#[test]
fn admission_precedes_even_present_ram_and_backing_entries() {
    use crate::sector::Mask;
    let artifact = derive_one_loop_unit_mass_tadpole()
        .unwrap()
        .with_total_excess_scope_for_test(1, BTreeMap::from([(Mask::try_new([true]).unwrap(), 5)]))
        .unwrap();
    let program = ClosedUnitMassProgram::new(&artifact, [8; 32]);
    let mut cache = GuardedRootCache::new(&program, Default::default());
    let forbidden = ApplicationCacheRecord {
        key: ApplicationCacheKey::guarded_root(&key(3)),
        terms: BTreeMap::from([(key(1), program.context().one())]),
        nonzero_conditions: Vec::new(),
    };
    // Test-only insertion deliberately bypasses public program validation.
    cache
        .insert(key(3), forbidden.clone(), Default::default())
        .unwrap();
    let mut backing = Backing::new(&program);
    backing.put(forbidden);
    let before = cache.statistics();
    assert_eq!(
        program.reduce_guarded_root(&key(3), &mut cache, Default::default(), Some(&mut backing)),
        Err(GuardedCacheError::Reduction(
            ReductionError::OutsideCertifiedTotalExcessDomain { maximum: 1 }
        ))
    );
    assert_eq!(backing.lookups, 0);
    assert_eq!(cache.statistics(), before);
    assert!(matches!(
        program.reduce_guarded_root(
            &IntegralKey::try_new([1, 1]).unwrap(),
            &mut cache,
            Default::default(),
            Some(&mut backing)
        ),
        Err(GuardedCacheError::Reduction(
            ReductionError::WrongArity { .. }
        ))
    ));
    assert_eq!(backing.lookups, 0);
}

#[test]
fn artifact_digest_local_identity_and_backing_layer_must_match() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let other_artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let program = ClosedUnitMassProgram::new(&artifact, [9; 32]);
    let mut cache = GuardedRootCache::new(&program, Default::default());
    program
        .reduce_guarded_root(&key(1), &mut cache, Default::default(), None)
        .unwrap();
    for other in [
        ClosedUnitMassProgram::new(&artifact, [10; 32]),
        ClosedUnitMassProgram::new(&other_artifact, [9; 32]),
    ] {
        assert_eq!(
            other.reduce_guarded_root(&key(1), &mut cache, Default::default(), None),
            Err(GuardedCacheError::BindingMismatch)
        );
    }
    let mut backing = Backing::new(&program);
    backing.binding.layer = "different-normalization".into();
    assert_eq!(
        program.reduce_guarded_root(&key(1), &mut cache, Default::default(), Some(&mut backing)),
        Err(GuardedCacheError::BindingMismatch)
    );
    assert_eq!(backing.lookups, 0);
}

#[test]
fn backing_roundtrip_clear_and_zero_keep_ordered_guards() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let program = ClosedUnitMassProgram::new(&artifact, [11; 32]);
    let mut cache = GuardedRootCache::new(&program, Default::default());
    let mut backing = Backing::new(&program);
    let first = program
        .reduce_guarded_root(&key(4), &mut cache, Default::default(), Some(&mut backing))
        .unwrap();
    assert_eq!(backing.stores, 1);
    cache.clear().unwrap();
    assert_eq!(cache.statistics().cached_coefficient_bytes, 0);
    let restored = program
        .reduce_guarded_root(
            &key(4),
            &mut cache,
            ReductionLimits {
                max_rule_applications: 0,
                ..Default::default()
            },
            Some(&mut backing),
        )
        .unwrap();
    assert_eq!(restored.source, GuardedRootSource::Backing);
    assert_eq!(restored.record, first.record);
    assert_eq!(backing.stores, 1);
    let mut zero = baseline(&program, 0);
    assert!(zero.terms.is_empty());
    let guard = program
        .context()
        .parameter(&program.context().parameter_names()[0])
        .unwrap();
    zero.nonzero_conditions = vec![guard.clone(), program.context().integer(-2), guard];
    let expected = coefficient_cache_weight(&zero.nonzero_conditions).unwrap();
    backing.put(zero.clone());
    cache.clear().unwrap();
    let restored = program
        .reduce_guarded_root(&key(0), &mut cache, Default::default(), Some(&mut backing))
        .unwrap();
    assert_eq!(restored.record, zero);
    assert_eq!(
        cache.statistics().cached_coefficient_bytes,
        expected.coefficient_bytes
    );
    assert_eq!(
        cache.statistics().cached_coefficient_terms,
        expected.coefficient_terms
    );
    let mut too_small = GuardedRootCache::new(
        &program,
        ReductionLimits {
            max_cached_coefficient_bytes: 0,
            ..Default::default()
        },
    );
    assert!(matches!(
        program.reduce_guarded_root(
            &key(0),
            &mut too_small,
            Default::default(),
            Some(&mut backing)
        ),
        Err(GuardedCacheError::Reduction(
            ReductionError::CacheCoefficientByteLimit { .. }
        ))
    ));
    assert_eq!(too_small.statistics().cached_integrals, 0);
}

#[test]
fn both_eviction_policies_admit_bounded_backing_results_and_reuse_after_clear() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let program = ClosedUnitMassProgram::new(&artifact, [12; 32]);
    for policy in [
        CompletedCachePolicy::EvictLeastRecentlyUsed,
        CompletedCachePolicy::EvictAgedLeastFrequentlyUsed,
    ] {
        let limits = ReductionLimits {
            max_cached_integrals: 2,
            ..Default::default()
        };
        let mut cache = GuardedRootCache::with_policy(&program, limits, policy);
        let mut backing = Backing::new(&program);
        for n in 1..=4 {
            backing.put(baseline(&program, n));
        }
        for n in [1, 2, 1, 3, 4] {
            let result = program
                .reduce_guarded_root(&key(n), &mut cache, limits, Some(&mut backing))
                .unwrap();
            assert_eq!(result.record, baseline(&program, n));
            assert!(cache.statistics().cached_integrals <= 2);
        }
        assert_eq!(cache.statistics().evicted_entries, 2);
        cache.clear().unwrap();
        assert_eq!(cache.statistics().cached_integrals, 0);
        assert_eq!(
            program
                .reduce_guarded_root(&key(1), &mut cache, limits, Some(&mut backing))
                .unwrap()
                .source,
            GuardedRootSource::Backing
        );
    }
}

#[test]
fn invalid_backing_records_and_typed_errors_never_become_misses() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let program = ClosedUnitMassProgram::new(&artifact, [13; 32]);
    let valid = baseline(&program, 2);
    let mut invalid = Vec::new();
    let mut record = valid.clone();
    record.key = ApplicationCacheKey::guarded_root(&key(3));
    invalid.push(record);
    let mut record = valid.clone();
    record.terms.insert(key(0), program.context().one());
    invalid.push(record);
    let mut record = valid.clone();
    record.nonzero_conditions.push(program.context().zero());
    invalid.push(record);
    let foreign = CoefficientContext::try_new(["guarded_cache_foreign_symbol"]).unwrap();
    let mut record = valid;
    record
        .nonzero_conditions
        .push(foreign.parameter("guarded_cache_foreign_symbol").unwrap());
    invalid.push(record);
    for record in invalid {
        let mut cache = GuardedRootCache::new(&program, Default::default());
        let mut backing = Backing::new(&program);
        backing.override_record = Some(record);
        assert!(
            program
                .reduce_guarded_root(&key(2), &mut cache, Default::default(), Some(&mut backing))
                .is_err()
        );
        assert_eq!(backing.lookups, 1);
        assert_eq!(backing.stores, 0);
        assert_eq!(cache.statistics().cached_integrals, 0);
        assert_eq!(cache.statistics().completed_reductions, 0);
    }
    for kind in [
        GuardedCacheBackingErrorKind::Invalid,
        GuardedCacheBackingErrorKind::Limit,
        GuardedCacheBackingErrorKind::Io,
    ] {
        let error = GuardedCacheBackingError {
            kind,
            message: "injected backing error".into(),
        };
        let mut cache = GuardedRootCache::new(&program, Default::default());
        let mut backing = Backing::new(&program);
        backing.lookup_error = Some(error.clone());
        assert_eq!(
            program.reduce_guarded_root(
                &key(2),
                &mut cache,
                Default::default(),
                Some(&mut backing)
            ),
            Err(GuardedCacheError::Backing(error))
        );
        assert_eq!(cache.statistics().completed_reductions, 0);
    }
}

#[test]
fn failed_reduction_or_store_does_not_publish_a_root_and_cache_stays_reusable() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let program = ClosedUnitMassProgram::new(&artifact, [14; 32]);
    let mut cache = GuardedRootCache::new(&program, Default::default());
    let mut backing = Backing::new(&program);
    let no_work = ReductionLimits {
        max_rule_applications: 0,
        ..Default::default()
    };
    assert!(
        program
            .reduce_guarded_root(&key(2), &mut cache, no_work, Some(&mut backing))
            .is_err()
    );
    assert_eq!(backing.stores, 0);
    assert_eq!(cache.statistics().cached_integrals, 0);
    backing.store_error = Some(GuardedCacheBackingError {
        kind: GuardedCacheBackingErrorKind::Limit,
        message: "disk full".into(),
    });
    assert!(matches!(
        program.reduce_guarded_root(&key(2), &mut cache, Default::default(), Some(&mut backing)),
        Err(GuardedCacheError::Backing(_))
    ));
    assert_eq!(cache.statistics().cached_integrals, 0);
    backing.store_error = None;
    assert_eq!(
        program
            .reduce_guarded_root(&key(2), &mut cache, Default::default(), Some(&mut backing))
            .unwrap()
            .record,
        baseline(&program, 2)
    );
}

#[test]
fn homogeneity_uses_the_existing_native_conversion_and_actual_limits() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let program = ClosedUnitMassProgram::new(&artifact, [15; 32]);
    let mut cache = GuardedRootCache::new(&program, Default::default());
    let mut plain = Reducer::new(&artifact).unwrap();
    for n in 0..=5 {
        let result = program
            .reduce_guarded_root(&key(n), &mut cache, Default::default(), None)
            .unwrap();
        assert_eq!(
            program
                .homogeneous_decomposition(&result.record, Default::default())
                .unwrap(),
            plain.reduce_with_common_mass_homogeneity(&key(n)).unwrap()
        );
        assert_eq!(
            program
                .decomposition(&result.record, Default::default())
                .unwrap()
                .terms(),
            &result.record.terms
        );
    }
}

#[test]
fn full_outer_roots_yield_to_the_same_cold_insertion_without_restarting_work() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let program = ClosedUnitMassProgram::new(&artifact, [16; 32]);
    let limits = ReductionLimits {
        max_cached_integrals: 2,
        ..Default::default()
    };
    let (_, _, reference_work) =
        Reducer::reduce_artifact_unit_mass_guarded(&artifact, &key(2), limits).unwrap();
    for policy in [
        CompletedCachePolicy::EvictLeastRecentlyUsed,
        CompletedCachePolicy::EvictAgedLeastFrequentlyUsed,
    ] {
        let mut cache = GuardedRootCache::with_policy(&program, limits, policy);
        let mut backing = Backing::new(&program);
        for n in [1, 3] {
            backing.put(baseline(&program, n));
            program
                .reduce_guarded_root(&key(n), &mut cache, limits, Some(&mut backing))
                .unwrap();
        }
        assert_eq!(cache.statistics().cached_integrals, 2);
        let cold = program
            .reduce_guarded_root(&key(2), &mut cache, limits, None)
            .unwrap();
        assert_eq!(cold.record, baseline(&program, 2));
        assert_eq!(cold.source, GuardedRootSource::Reduced);
        assert_eq!(
            cold.work.rule_applications(),
            reference_work.rule_applications()
        );
        assert_eq!(
            cold.work.coalescing_additions(),
            reference_work.coalescing_additions()
        );
        assert_eq!(cache.statistics().evicted_entries, 2);
        assert_eq!(cache.statistics().cached_integrals, 1);
        let total = cache.shared_budget().snapshot();
        assert_eq!(total.integrals, 1);
        assert_eq!(
            total.coefficient_bytes,
            cache.statistics().cached_coefficient_bytes
        );
    }
}

#[test]
fn impossible_single_owned_payload_does_not_evict_useful_roots() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let program = ClosedUnitMassProgram::new(&artifact, [17; 32]);
    let mut cache = GuardedRootCache::new(&program, Default::default());
    program
        .reduce_guarded_root(&key(1), &mut cache, Default::default(), None)
        .unwrap();
    let before = cache.statistics();
    let limits = ReductionLimits::default();
    assert!(
        cache
            .shared_budget()
            .replace(
                0,
                crate::reduction::CacheWeight::default(),
                1,
                crate::reduction::CacheWeight {
                    coefficient_terms: 1,
                    coefficient_bytes: limits.max_cached_coefficient_bytes + 1
                },
                limits
            )
            .is_err()
    );
    assert_eq!(cache.statistics(), before);
    assert!(cache.peek(&key(1)).is_some());
}

#[test]
fn native_error_releases_transient_budget_and_next_cold_request_can_succeed() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let program = ClosedUnitMassProgram::new(&artifact, [18; 32]);
    let limits = ReductionLimits {
        max_cached_integrals: 2,
        ..Default::default()
    };
    let mut cache = GuardedRootCache::new(&program, limits);
    program
        .reduce_guarded_root(&key(1), &mut cache, limits, None)
        .unwrap();
    assert!(
        program
            .reduce_guarded_root(
                &key(2),
                &mut cache,
                ReductionLimits {
                    max_rule_applications: 0,
                    ..limits
                },
                None
            )
            .is_err()
    );
    assert_eq!(
        cache.shared_budget().snapshot().integrals,
        cache.statistics().cached_integrals
    );
    assert_eq!(
        program
            .reduce_guarded_root(&key(2), &mut cache, limits, None)
            .unwrap()
            .record,
        baseline(&program, 2)
    );
    assert_eq!(
        cache.shared_budget().snapshot().integrals,
        cache.statistics().cached_integrals
    );
}

#[test]
fn warm_hits_reuse_validation_stamp_and_tighter_bounds_validate_once() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let program = ClosedUnitMassProgram::new(&artifact, [19; 32]);
    let mut cache = GuardedRootCache::new(&program, Default::default());
    let first = program
        .reduce_guarded_root(&key(2), &mut cache, Default::default(), None)
        .unwrap();
    assert_eq!(cache.statistics().payload_validations, 1);
    for _ in 0..3 {
        assert_eq!(
            program
                .reduce_guarded_root(&key(2), &mut cache, Default::default(), None)
                .unwrap()
                .record,
            first.record
        );
    }
    assert_eq!(cache.statistics().payload_validations, 1);
    let mut tighter = ReductionLimits::default();
    tighter.exact_algebra.max_polynomial_terms -= 1;
    program
        .reduce_guarded_root(&key(2), &mut cache, tighter, None)
        .unwrap();
    assert_eq!(cache.statistics().payload_validations, 2);
    program
        .reduce_guarded_root(&key(2), &mut cache, tighter, None)
        .unwrap();
    program
        .reduce_guarded_root(&key(2), &mut cache, Default::default(), None)
        .unwrap();
    assert_eq!(cache.statistics().payload_validations, 2);
    let mut impossible = tighter;
    impossible.exact_algebra.max_polynomial_terms = 0;
    let hits = cache.statistics().ram_hits;
    assert!(
        program
            .reduce_guarded_root(&key(2), &mut cache, impossible, None)
            .is_err()
    );
    assert_eq!(cache.statistics().payload_validations, 3);
    assert_eq!(cache.statistics().ram_hits, hits);
    assert_eq!(
        program
            .reduce_guarded_root(&key(2), &mut cache, tighter, None)
            .unwrap()
            .record,
        first.record
    );
    assert_eq!(cache.statistics().payload_validations, 3);
}

#[test]
fn backing_cannot_contradict_sealed_zero_or_master_leaf_authority() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let program = ClosedUnitMassProgram::new(&artifact, [20; 32]);
    let mut zero = baseline(&program, 0);
    zero.terms.insert(key(1), program.context().one());
    let mut master_zero = baseline(&program, 1);
    master_zero.terms.clear();
    let mut master_scaled = baseline(&program, 1);
    master_scaled
        .terms
        .insert(key(1), program.context().integer(2));
    for (n, record) in [(0, zero), (1, master_zero), (1, master_scaled)] {
        let mut cache = GuardedRootCache::new(&program, Default::default());
        let mut backing = Backing::new(&program);
        backing.put(record);
        assert!(matches!(
            program.reduce_guarded_root(
                &key(n),
                &mut cache,
                Default::default(),
                Some(&mut backing)
            ),
            Err(GuardedCacheError::InvalidRecord(_))
        ));
        assert_eq!(cache.statistics().cached_integrals, 0);
        assert_eq!(cache.statistics().completed_reductions, 0);
        assert_eq!(backing.stores, 0);
    }
}

#[test]
fn combined_homogeneity_matches_plain_reducer_without_warm_payload_revalidation() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let program = ClosedUnitMassProgram::new(&artifact, [21; 32]);
    let mut cache = GuardedRootCache::new(&program, Default::default());
    let mut plain = Reducer::new(&artifact).unwrap();
    for n in 0..=5 {
        let (first, homogeneous) = program
            .reduce_guarded_root_with_homogeneity(&key(n), &mut cache, Default::default(), None)
            .unwrap();
        assert_eq!(first.record, baseline(&program, n));
        assert_eq!(
            homogeneous,
            plain.reduce_with_common_mass_homogeneity(&key(n)).unwrap()
        );
        let validations = cache.statistics().payload_validations;
        let (warm, warm_homogeneous) = program
            .reduce_guarded_root_with_homogeneity(&key(n), &mut cache, Default::default(), None)
            .unwrap();
        assert_eq!(warm.record, first.record);
        assert_eq!(warm.source, GuardedRootSource::Ram);
        assert_eq!(warm_homogeneous, homogeneous);
        assert_eq!(cache.statistics().payload_validations, validations);
    }
}
