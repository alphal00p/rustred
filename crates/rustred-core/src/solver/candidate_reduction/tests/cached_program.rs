use super::*;
use crate::algebra::CoefficientContext;
use crate::persistence::application_cache::{
    ApplicationCacheBinding, ApplicationCacheKey, ApplicationCacheRecord,
};
use crate::reduction::{
    CompletedCachePolicy, GuardedCacheBacking, GuardedCacheBackingError,
    GuardedCacheBackingErrorKind, GuardedCacheError, GuardedRootSource,
};
use crate::solver::{
    CandidateCacheRepresentation, CandidateGuardedCacheError, CandidateUnitMassProgram,
};

struct Backing {
    binding: ApplicationCacheBinding,
    records: BTreeMap<ApplicationCacheKey, ApplicationCacheRecord>,
    lookups: usize,
    stores: usize,
    fail_after: Option<usize>,
}
impl Backing {
    fn new<const N: usize>(program: &CandidateUnitMassProgram<N>) -> Self {
        Self {
            binding: program.binding().clone(),
            records: BTreeMap::new(),
            lookups: 0,
            stores: 0,
            fail_after: None,
        }
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
        Ok(self.records.get(key).cloned())
    }
    fn store_completed(
        &mut self,
        record: &ApplicationCacheRecord,
        _: &CoefficientContext,
    ) -> Result<(), GuardedCacheBackingError> {
        if self.fail_after == Some(self.stores) {
            return Err(GuardedCacheBackingError {
                kind: GuardedCacheBackingErrorKind::Limit,
                message: "test disk allowance".into(),
            });
        }
        self.stores += 1;
        self.records.insert(record.key.clone(), record.clone());
        Ok(())
    }
}

fn policies() -> [CompletedCachePolicy; 2] {
    [
        CompletedCachePolicy::EvictLeastRecentlyUsed,
        CompletedCachePolicy::EvictAgedLeastFrequentlyUsed,
    ]
}

#[test]
fn cold_warm_root_results_and_mass_authority_match_native_without_hot_revalidation() {
    let family = crate::solver::tests::tadpole();
    for representation in [
        CandidateCacheRepresentation::Sparse,
        CandidateCacheRepresentation::Factorized,
    ] {
        let mut source = candidate::<1>(&family, Default::default());
        source.set_cache_representation(representation).unwrap();
        let expected = source
            .reduce_unit_mass_guarded(&key([4]), Default::default())
            .unwrap();
        let before = source.statistics();
        let program = CandidateUnitMassProgram::new(&source, [1; 32]);
        assert!(Arc::strong_count(&source.program) >= 2);
        for policy in policies() {
            let mut cache = program.new_cache(Default::default(), policy);
            let (cold, unit) = program
                .reduce_guarded_root_with_decomposition(
                    &key([4]),
                    &mut cache,
                    Default::default(),
                    None,
                )
                .unwrap();
            assert_eq!(unit, expected.0);
            assert_eq!(cold.record.nonzero_conditions, expected.1);
            assert_eq!(cold.work, expected.2);
            assert_eq!(cold.source, GuardedRootSource::Reduced);
            assert_eq!(unit.common_mass_squared_power(&key([1])).unwrap(), -3);
            let validations = cache.statistics().payload_validations;
            let (warm, repeated) = program
                .clone()
                .reduce_guarded_root_with_decomposition(
                    &key([4]),
                    &mut cache,
                    ReductionLimits {
                        max_rule_applications: 0,
                        ..Default::default()
                    },
                    None,
                )
                .unwrap();
            assert_eq!(repeated, unit);
            assert_eq!(warm.record, cold.record);
            assert_eq!(warm.source, GuardedRootSource::Ram);
            assert_eq!(warm.work.rule_applications(), 0);
            assert_eq!(cache.statistics().payload_validations, validations);
            assert_eq!(
                cache.shared_budget().snapshot().integrals,
                cache.statistics().cached_integrals
            );
        }
        assert_eq!(source.statistics(), before);
    }
}

#[test]
fn held_out_candidate_root_remains_cold_and_clear_releases_only_resident_roots() {
    let family = crate::solver::tests::tadpole();
    let source = candidate::<1>(&family, Default::default());
    let program = CandidateUnitMassProgram::new(&source, [2; 32]);
    let mut reference = source.fork_empty();
    let expected = reference
        .reduce_unit_mass_guarded(&key([5]), Default::default())
        .unwrap();
    let mut cache = program.new_cache(Default::default(), policies()[0]);
    program
        .reduce_guarded_root(&key([4]), &mut cache, Default::default(), None)
        .unwrap();
    let held_out = program
        .reduce_guarded_root(&key([5]), &mut cache, Default::default(), None)
        .unwrap();
    assert_eq!(held_out.record.terms, *expected.0.terms());
    assert_eq!(held_out.work, expected.2);
    assert_eq!(cache.statistics().descendant_ram_hits, 0);
    cache.clear().unwrap();
    assert_eq!(cache.shared_budget().snapshot(), Default::default());
    assert_eq!(cache.statistics().completed_reductions, 2);
    assert_eq!(
        program
            .reduce_guarded_root(&key([4]), &mut cache, Default::default(), None)
            .unwrap()
            .source,
        GuardedRootSource::Reduced
    );
}

#[test]
fn readthrough_export_and_typed_prefix_failure_preserve_exact_records_and_usage() {
    let source = candidate::<1>(&crate::solver::tests::tadpole(), Default::default());
    let program = CandidateUnitMassProgram::new(&source, [3; 32]);
    let mut cache = program.new_cache(Default::default(), policies()[1]);
    for n in [0, 1, 4] {
        program
            .reduce_guarded_root(&key([n]), &mut cache, Default::default(), None)
            .unwrap();
    }
    let before = cache.statistics();
    let mut backing = Backing::new(&program);
    backing.fail_after = Some(2);
    let failed = program.export_completed(&cache, &mut backing).unwrap_err();
    assert_eq!(failed.accepted_prefix.exported_records, 2);
    assert!(matches!(
        failed.source,
        GuardedCacheError::Backing(GuardedCacheBackingError {
            kind: GuardedCacheBackingErrorKind::Limit,
            ..
        })
    ));
    assert_eq!(cache.statistics(), before);
    backing.fail_after = None;
    assert_eq!(
        program
            .export_completed(&cache, &mut backing)
            .unwrap()
            .exported_records,
        3
    );
    cache.clear().unwrap();
    let loaded = program
        .reduce_guarded_root(
            &key([4]),
            &mut cache,
            ReductionLimits {
                max_rule_applications: 0,
                ..Default::default()
            },
            Some(&mut backing),
        )
        .unwrap();
    assert_eq!(loaded.source, GuardedRootSource::Backing);
    assert_eq!(loaded.work.rule_applications(), 0);
    assert_eq!(
        loaded.record,
        backing.records[&ApplicationCacheKey::guarded_root(&key([4]))]
    );
    assert_eq!(
        cache.statistics().completed_reductions,
        before.completed_reductions
    );
}

#[test]
fn instance_digest_and_context_mismatches_fail_before_use() {
    let source = candidate::<1>(&crate::solver::tests::tadpole(), Default::default());
    let program = CandidateUnitMassProgram::new(&source, [4; 32]);
    let same_bytes_other_snapshot = CandidateUnitMassProgram::new(&source, [4; 32]);
    let mut cache = program.new_cache(Default::default(), policies()[0]);
    assert!(matches!(
        same_bytes_other_snapshot.reduce_guarded_root(
            &key([1]),
            &mut cache,
            Default::default(),
            None
        ),
        Err(CandidateGuardedCacheError::Cache(
            GuardedCacheError::BindingMismatch
        ))
    ));
    let mut backing = Backing::new(&program);
    backing.binding.artifact_digest = [5; 32];
    assert!(matches!(
        program.reduce_guarded_root(
            &key([1]),
            &mut cache,
            Default::default(),
            Some(&mut backing)
        ),
        Err(CandidateGuardedCacheError::Cache(
            GuardedCacheError::BindingMismatch
        ))
    ));
    assert_eq!(backing.lookups, 0);
    let mut record = program
        .reduce_guarded_root(&key([4]), &mut cache, Default::default(), None)
        .unwrap()
        .record;
    record.terms.insert(
        key([1]),
        CoefficientContext::new(["foreign"])
            .parameter("foreign")
            .unwrap(),
    );
    assert!(
        program
            .validate_completed_record(&record, Default::default())
            .is_err()
    );
}

#[test]
fn current_source_rank_arity_and_root_admission_precede_backing_lookup() {
    let family = crate::solver::tests::tadpole();
    let mut source = candidate::<1>(&family, Default::default());
    let condition = source
        .coefficient_context()
        .admit_native_polynomial_result_with_limits(
            index_offset(source.coefficient_context(), 0, 2),
            Default::default(),
        )
        .unwrap();
    let owned = Arc::get_mut(&mut source.program).unwrap();
    owned.source_conditions.push(condition);
    owned.max_numerator_rank = Some(0);
    let program = CandidateUnitMassProgram::new(&source, [6; 32]);
    let mut cache = program.new_cache(Default::default(), policies()[0]);
    let mut backing = Backing::new(&program);
    for target in [key([2]), key([-1]), key([1, 1])] {
        assert!(matches!(
            program.reduce_guarded_root(
                &target,
                &mut cache,
                Default::default(),
                Some(&mut backing)
            ),
            Err(CandidateGuardedCacheError::Candidate(_))
        ));
    }
    assert_eq!(backing.lookups, 0);
    drop(program);
    Arc::get_mut(&mut source.program).unwrap().root_sector = [false];
    let restricted = CandidateUnitMassProgram::new(&source, [7; 32]);
    assert!(matches!(
        restricted.validate_entry(&key([1]), Default::default()),
        Err(CandidateReductionError::OutsideRoot { .. })
    ));
}

#[test]
fn zero_guards_leaf_rows_and_unknown_keys_are_not_promoted() {
    let family = crate::solver::tests::tadpole();
    let mut source = candidate::<1>(&family, Default::default());
    let d = source.coefficient_context().base().parameter("d").unwrap();
    let condition = source
        .coefficient_context()
        .lift_base_polynomial(&d.numerator)
        .unwrap();
    Arc::get_mut(&mut source.program)
        .unwrap()
        .source_conditions
        .push(condition);
    let program = CandidateUnitMassProgram::new(&source, [8; 32]);
    let mut cache = program.new_cache(Default::default(), policies()[0]);
    let zero = program
        .reduce_guarded_root(&key([0]), &mut cache, Default::default(), None)
        .unwrap();
    assert!(zero.record.terms.is_empty());
    assert!(zero.record.nonzero_conditions.contains(&d));
    assert!(cache.statistics().cached_coefficient_bytes > 0);
    let mut omitted = zero.record.clone();
    omitted.nonzero_conditions.clear();
    assert!(
        program
            .validate_completed_record(&omitted, Default::default())
            .is_err()
    );
    let mut contradictory = zero.record;
    contradictory
        .terms
        .insert(key([1]), program.context().one());
    assert!(
        program
            .validate_completed_record(&contradictory, Default::default())
            .is_err()
    );
    let mut leaf = program
        .reduce_guarded_root(&key([1]), &mut cache, Default::default(), None)
        .unwrap()
        .record;
    leaf.terms.insert(key([1]), program.context().integer(2));
    assert!(
        program
            .validate_completed_record(&leaf, Default::default())
            .is_err()
    );
    let mut solution = one_rule(&family);
    solution.rules.clear();
    let missing = CandidateUnitMassProgram::new(&from_one_rule(&family, solution), [9; 32]);
    let mut empty = missing.new_cache(Default::default(), policies()[0]);
    assert!(matches!(
        missing.reduce_guarded_root(&key([2]), &mut empty, Default::default(), None),
        Err(CandidateGuardedCacheError::Candidate(
            CandidateReductionError::Uncovered { .. }
        ))
    ));
    assert_eq!(empty.shared_budget().snapshot(), Default::default());
}

#[test]
fn explicit_cache_limits_are_not_clipped_to_loader_defaults_and_tightening_revalidates() {
    let source = candidate::<1>(
        &crate::solver::tests::tadpole(),
        ReductionLimits {
            max_rule_applications: 0,
            ..Default::default()
        },
    );
    let program = CandidateUnitMassProgram::new(&source, [10; 32]);
    let mut cache = program.new_cache(Default::default(), policies()[0]);
    program
        .reduce_guarded_root(&key([4]), &mut cache, Default::default(), None)
        .unwrap();
    let before = cache.statistics().payload_validations;
    let mut tighter = ReductionLimits::default();
    tighter.exact_algebra.max_polynomial_terms = 4;
    program
        .reduce_guarded_root(&key([4]), &mut cache, tighter, None)
        .unwrap();
    assert_eq!(cache.statistics().payload_validations, before + 1);
    program
        .reduce_guarded_root(&key([4]), &mut cache, tighter, None)
        .unwrap();
    assert_eq!(cache.statistics().payload_validations, before + 1);
    tighter.exact_algebra.max_polynomial_terms = 1;
    assert!(
        program
            .reduce_guarded_root(&key([4]), &mut cache, tighter, None)
            .is_err()
    );
    assert_eq!(cache.statistics().cached_integrals, 1);
}

#[test]
fn cold_pressure_evicts_completed_roots_but_never_grants_a_second_native_budget() {
    let source = candidate::<1>(&crate::solver::tests::tadpole(), Default::default());
    let program = CandidateUnitMassProgram::new(&source, [11; 32]);
    for policy in policies() {
        let limits = ReductionLimits {
            max_cached_integrals: 4,
            ..Default::default()
        };
        let mut cache = program.new_cache(limits, policy);
        for n in [-3, -2, -1, 0] {
            program
                .reduce_guarded_root(&key([n]), &mut cache, limits, None)
                .unwrap();
        }
        assert_eq!(cache.statistics().cached_integrals, 4);
        let result = program
            .reduce_guarded_root(&key([4]), &mut cache, limits, None)
            .unwrap();
        assert_eq!(result.work.rule_applications(), 3);
        assert!(cache.statistics().evicted_entries >= 4);
        assert_eq!(
            cache.shared_budget().snapshot().integrals,
            cache.statistics().cached_integrals
        );
        assert!(
            program
                .reduce_guarded_root(&key([5]), &mut cache, limits, None)
                .is_err()
        );
        assert_eq!(
            cache.shared_budget().snapshot().integrals,
            cache.statistics().cached_integrals
        );
    }
}

#[test]
fn native_shared_census_drop_releases_own_share_on_success_error_and_pressure_panic() {
    let source = candidate::<1>(&crate::solver::tests::tadpole(), Default::default());
    for mode in 0..3 {
        let shared = Arc::new(crate::reduction::SharedCacheBudget::default());
        let retained = crate::reduction::coefficient_cache_weight(
            [source.coefficient_context().base().one()].iter(),
        )
        .unwrap();
        shared
            .replace(0, Default::default(), 1, retained, Default::default())
            .unwrap();
        let before = shared.snapshot();
        let limits = ReductionLimits {
            max_cached_integrals: if mode == 0 { 100 } else { 3 },
            ..Default::default()
        };
        if mode == 2 {
            shared.set_pressure_handler(Arc::new(|| panic!("injected candidate pressure panic")));
        }
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut session = source.fork_empty_with_shared_budget(limits, Arc::clone(&shared));
            session.reduce_unit_mass_guarded(&key([4]), limits)
        }));
        match mode {
            0 => assert!(outcome.unwrap().is_ok()),
            1 => assert!(outcome.unwrap().is_err()),
            _ => assert!(outcome.is_err()),
        }
        assert_eq!(shared.snapshot(), before);
    }
}

#[cfg(feature = "capacity-dispatch")]
#[test]
fn padded_candidate_capacity_uses_physical_arity_for_binding_and_record_keys() {
    let family = crate::solver::tests::tadpole();
    let sector = [true, false, false, false];
    let source = CandidateReducer::<4>::try_new(
        &family,
        sector,
        OrderingPolicy::SpiredUncutV1,
        [(
            sector,
            SectorSolution {
                order: crate::solver::IntegralOrder::new(sector, [false; 4]),
                finite_case_policy: Default::default(),
                max_numerator_rank: None,
                rules: vec![],
                finite_residuals: vec![Integral::numeric([1, 0, 0, 0]).unwrap()],
                stats: Default::default(),
            },
        )],
        vec![],
        Default::default(),
    )
    .unwrap();
    assert_eq!(source.index_count(), 4);
    assert_eq!(source.physical_arity(), 1);
    let program = CandidateUnitMassProgram::new(&source, [12; 32]);
    assert_eq!(program.binding().physical_arity, 1);
    let mut cache = program.new_cache(Default::default(), policies()[0]);
    let cold = program
        .reduce_guarded_root(&key([1]), &mut cache, Default::default(), None)
        .unwrap();
    assert_eq!(cold.record.key.powers(), &[1]);
    assert_eq!(cold.record.terms.keys().next().unwrap().powers(), &[1]);
    assert_eq!(
        program
            .reduce_guarded_root(&key([1]), &mut cache, Default::default(), None)
            .unwrap()
            .source,
        GuardedRootSource::Ram
    );
    assert!(
        program
            .reduce_guarded_root(&key([1, 0]), &mut cache, Default::default(), None)
            .is_err()
    );
}
