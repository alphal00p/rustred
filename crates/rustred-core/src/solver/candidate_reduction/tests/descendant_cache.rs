//! Independent checks of the native traversal's completed-descendant boundary.
use super::*;
use crate::algebra::CoefficientContext;
use crate::persistence::application_cache::{
    ApplicationCacheBinding, ApplicationCacheChunk, ApplicationCacheKey, ApplicationCacheRecord,
    encode_application_cache_chunk,
};
use crate::reduction::guarded_cache::RootUse;
use crate::reduction::{
    CompletedCachePolicy, GuardedCacheBacking, GuardedCacheBackingError,
    GuardedCacheBackingErrorKind, GuardedCacheError, GuardedRootCache, GuardedRootSource,
};
use crate::solver::candidate_reduction::owner_test_support::{input, rule};
use crate::solver::{
    CandidateCacheRepresentation, CandidateGuardedCacheError, CandidateUnitMassProgram,
};

fn policies() -> [CompletedCachePolicy; 2] {
    [
        CompletedCachePolicy::EvictLeastRecentlyUsed,
        CompletedCachePolicy::EvictAgedLeastFrequentlyUsed,
    ]
}

fn baseline<const N: usize>(
    source: &CandidateReducer<N>,
    target: &IntegralKey,
) -> ApplicationCacheRecord {
    let (unit, nonzero_conditions, _) = source
        .fork_empty()
        .reduce_unit_mass_guarded(target, Default::default())
        .unwrap();
    ApplicationCacheRecord {
        key: ApplicationCacheKey::guarded_root(target),
        terms: unit.into_terms(),
        nonzero_conditions,
    }
}

fn assert_only_outer_census(cache: &GuardedRootCache) {
    let census = cache.shared_budget().snapshot();
    let retained = cache.statistics();
    assert_eq!(census.integrals, retained.cached_integrals);
    assert_eq!(census.coefficient_terms, retained.cached_coefficient_terms);
    assert_eq!(census.coefficient_bytes, retained.cached_coefficient_bytes);
}

struct Backing {
    binding: ApplicationCacheBinding,
    records: BTreeMap<ApplicationCacheKey, ApplicationCacheRecord>,
    lookups: Vec<ApplicationCacheKey>,
    stores: usize,
    fail_at: Option<IntegralKey>,
    panic_at: Option<IntegralKey>,
}
impl Backing {
    fn new<const N: usize>(program: &CandidateUnitMassProgram<N>) -> Self {
        Self {
            binding: program.binding().clone(),
            records: BTreeMap::new(),
            lookups: Vec::new(),
            stores: 0,
            fail_at: None,
            panic_at: None,
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
        self.lookups.push(key.clone());
        assert!(
            !self
                .panic_at
                .as_ref()
                .is_some_and(|target| target.powers() == key.powers()),
            "injected descendant lookup panic"
        );
        if self
            .fail_at
            .as_ref()
            .is_some_and(|target| target.powers() == key.powers())
        {
            return Err(GuardedCacheBackingError {
                kind: GuardedCacheBackingErrorKind::Limit,
                message: "injected bounded descendant read refusal".into(),
            });
        }
        Ok(self.records.get(key).cloned())
    }
    fn store_completed(
        &mut self,
        record: &ApplicationCacheRecord,
        _: &CoefficientContext,
    ) -> Result<(), GuardedCacheBackingError> {
        self.stores += 1;
        self.put(record.clone());
        Ok(())
    }
}

#[test]
fn prepared_child_reduces_unseen_root_with_one_rule_for_both_representations_and_policies() {
    for representation in [
        CandidateCacheRepresentation::Sparse,
        CandidateCacheRepresentation::Factorized,
    ] {
        let mut source = candidate::<1>(&crate::solver::tests::tadpole(), Default::default());
        source.set_cache_representation(representation).unwrap();
        // Existing source work and its ordinary memo remain untouched by the
        // program snapshot, including across successful descendant imports.
        source.reduce_unit_mass(&key([3])).unwrap();
        let source_before = source.statistics();
        let expected = baseline(&source, &key([5]));
        let program = CandidateUnitMassProgram::new(&source, [61; 32]);
        let one_rule = ReductionLimits {
            max_rule_applications: 1,
            ..Default::default()
        };
        assert!(
            source
                .fork_empty()
                .reduce_unit_mass_guarded(&key([5]), one_rule)
                .is_err()
        );
        for policy in policies() {
            let mut cache = program.new_cache(Default::default(), policy);
            program
                .reduce_guarded_root(&key([4]), &mut cache, Default::default(), None)
                .unwrap();
            let validations = cache.statistics().payload_validations;
            let result = program
                .reduce_guarded_root(&key([5]), &mut cache, one_rule, None)
                .unwrap();
            assert_eq!(result.record, expected);
            assert_eq!(result.source, GuardedRootSource::Reduced);
            assert_eq!(result.work.rule_applications(), 1);
            assert_eq!(result.work.cache_hits(), 1);
            assert_eq!(cache.statistics().descendant_ram_hits, 1);
            assert_eq!(cache.statistics().ram_hits, 0);
            assert_eq!(cache.statistics().payload_validations, validations + 1);
            assert_eq!(cache.statistics().cached_integrals, 2);
            assert!(cache.peek(&key([2])).is_none());
            assert!(cache.peek(&key([3])).is_none());
            assert_only_outer_census(&cache);
        }
        assert_eq!(source.statistics(), source_before);
    }
}

/// A fresh reader decodes only the requested native record from bytes on disk.
/// This deliberately uses the existing cache framing and artifact binding.
struct DiskBacking {
    binding: ApplicationCacheBinding,
    path: std::path::PathBuf,
    lookups: Vec<ApplicationCacheKey>,
    stores: usize,
}
impl Drop for DiskBacking {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
impl GuardedCacheBacking for DiskBacking {
    fn binding(&self) -> &ApplicationCacheBinding {
        &self.binding
    }
    fn lookup(
        &mut self,
        key: &ApplicationCacheKey,
        context: &CoefficientContext,
    ) -> Result<Option<ApplicationCacheRecord>, GuardedCacheBackingError> {
        self.lookups.push(key.clone());
        let invalid = |error: String| GuardedCacheBackingError {
            kind: GuardedCacheBackingErrorKind::Invalid,
            message: error,
        };
        let bytes = std::fs::read(&self.path).map_err(|error| invalid(error.to_string()))?;
        let chunk = ApplicationCacheChunk::inspect(&bytes, &self.binding, Default::default())
            .map_err(|error| invalid(error.to_string()))?;
        chunk
            .decode_record(key, context)
            .map_err(|error| invalid(error.to_string()))
    }
    fn store_completed(
        &mut self,
        _: &ApplicationCacheRecord,
        _: &CoefficientContext,
    ) -> Result<(), GuardedCacheBackingError> {
        self.stores += 1;
        Ok(())
    }
}

fn disk_backing(
    program: &CandidateUnitMassProgram<1>,
    record: &ApplicationCacheRecord,
) -> DiskBacking {
    use std::io::Write;
    static NEXT_FILE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let nonce = NEXT_FILE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "rustred-candidate-descendant-{}-{nonce}.bin",
        std::process::id()
    ));
    let bytes = encode_application_cache_chunk(
        program.binding(),
        std::slice::from_ref(record),
        program.context(),
        Default::default(),
    )
    .unwrap();
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    file.write_all(&bytes).unwrap();
    drop(file);
    DiskBacking {
        binding: program.binding().clone(),
        path,
        lookups: Vec::new(),
        stores: 0,
    }
}

#[test]
fn fresh_disk_provider_reuses_child_in_existing_native_schema_and_rejects_corruption() {
    let source = candidate::<1>(&crate::solver::tests::tadpole(), Default::default());
    let program = CandidateUnitMassProgram::new(&source, [62; 32]);
    let child = baseline(&source, &key([4]));
    for corrupt in [false, true] {
        let mut backing = disk_backing(&program, &child);
        if corrupt {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .open(&backing.path)
                .unwrap();
            file.set_len(17).unwrap();
        }
        let mut cache = program.new_cache(Default::default(), policies()[0]);
        let result = program.reduce_guarded_root(
            &key([5]),
            &mut cache,
            ReductionLimits {
                max_rule_applications: 1,
                ..Default::default()
            },
            Some(&mut backing),
        );
        if corrupt {
            assert!(matches!(
                result,
                Err(CandidateGuardedCacheError::Cache(
                    GuardedCacheError::Backing(GuardedCacheBackingError {
                        kind: GuardedCacheBackingErrorKind::Invalid,
                        ..
                    })
                ))
            ));
            assert_eq!(cache.statistics().completed_reductions, 0);
            assert_eq!(backing.stores, 0);
            assert_eq!(cache.shared_budget().snapshot(), Default::default());
        } else {
            let result = result.unwrap();
            assert_eq!(result.record, baseline(&source, &key([5])));
            assert_eq!(result.work.rule_applications(), 1);
            assert_eq!(
                backing.lookups,
                vec![
                    ApplicationCacheKey::guarded_root(&key([5])),
                    ApplicationCacheKey::guarded_root(&key([4]))
                ]
            );
            assert_eq!(cache.statistics().backing_hits, 0);
            assert_eq!(cache.statistics().descendant_backing_hits, 1);
            assert_eq!(backing.stores, 1);
            assert_only_outer_census(&cache);
        }
    }
}

#[test]
fn completed_zero_descendant_retains_ordered_source_exception_and_cancelled_denominator_guards() {
    for representation in [
        CandidateCacheRepresentation::Sparse,
        CandidateCacheRepresentation::Factorized,
    ] {
        let mut source = super::root_guards::guarded_fixture(true);
        source.set_cache_representation(representation).unwrap();
        let program = CandidateUnitMassProgram::new(&source, [63; 32]);
        let expected = baseline(&source, &key([5]));
        assert!(expected.terms.is_empty());
        assert!(expected.nonzero_conditions.len() >= 5);
        for disk in [false, true] {
            let mut cache = program.new_cache(Default::default(), policies()[1]);
            let mut backing = Backing::new(&program);
            if disk {
                backing.put(baseline(&source, &key([3])));
            } else {
                program
                    .reduce_guarded_root(&key([3]), &mut cache, Default::default(), None)
                    .unwrap();
            }
            let result = program
                .reduce_guarded_root(
                    &key([5]),
                    &mut cache,
                    Default::default(),
                    disk.then_some(&mut backing as &mut dyn GuardedCacheBacking),
                )
                .unwrap();
            assert_eq!(
                result.record, expected,
                "ordered guard union must survive an empty imported expansion"
            );
            assert_eq!(result.work.rule_applications(), 3);
            assert_eq!(cache.statistics().descendant_ram_hits, usize::from(!disk));
            assert_eq!(
                cache.statistics().descendant_backing_hits,
                usize::from(disk)
            );
            assert_only_outer_census(&cache);
        }
    }
}

#[test]
fn imported_extra_obligations_keep_constants_order_and_first_occurrence() {
    let source = candidate::<1>(&crate::solver::tests::tadpole(), Default::default());
    let program = CandidateUnitMassProgram::new(&source, [64; 32]);
    let mut child = baseline(&source, &key([4]));
    let d = program.context().parameter("d").unwrap();
    let shifted = program
        .context()
        .try_sub(&d, &program.context().integer(19), Default::default())
        .unwrap();
    child.nonzero_conditions = vec![
        shifted.clone(),
        program.context().integer(-2),
        d.clone(),
        shifted.clone(),
    ];
    let mut backing = Backing::new(&program);
    backing.put(child);
    let mut cache = program.new_cache(Default::default(), policies()[0]);
    let result = program
        .reduce_guarded_root(
            &key([5]),
            &mut cache,
            Default::default(),
            Some(&mut backing),
        )
        .unwrap();
    assert_eq!(result.record.terms, baseline(&source, &key([5])).terms);
    assert_eq!(
        result.record.nonzero_conditions,
        vec![shifted, program.context().integer(-2), d]
    );
}

#[test]
fn descendant_backing_error_and_unwind_release_only_request_ownership() {
    let source = candidate::<1>(&crate::solver::tests::tadpole(), Default::default());
    let program = CandidateUnitMassProgram::new(&source, [65; 32]);
    for panic in [false, true] {
        let mut cache = program.new_cache(Default::default(), policies()[0]);
        program
            .reduce_guarded_root(&key([0]), &mut cache, Default::default(), None)
            .unwrap();
        let before = cache.shared_budget().snapshot();
        let mut backing = Backing::new(&program);
        if panic {
            backing.panic_at = Some(key([3]));
        } else {
            backing.fail_at = Some(key([3]));
        }
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            program.reduce_guarded_root(
                &key([5]),
                &mut cache,
                Default::default(),
                Some(&mut backing),
            )
        }));
        if panic {
            assert!(outcome.is_err());
        } else {
            assert!(matches!(
                outcome.unwrap(),
                Err(CandidateGuardedCacheError::Cache(
                    GuardedCacheError::Backing(GuardedCacheBackingError {
                        kind: GuardedCacheBackingErrorKind::Limit,
                        ..
                    })
                ))
            ));
        }
        assert_eq!(backing.stores, 0);
        assert_eq!(cache.statistics().completed_reductions, 1);
        assert_eq!(cache.shared_budget().snapshot(), before);
        assert!(cache.peek(&key([5])).is_none());
        assert_eq!(
            program
                .reduce_guarded_root(&key([5]), &mut cache, Default::default(), None)
                .unwrap()
                .record,
            baseline(&source, &key([5]))
        );
    }
}

#[test]
fn descendant_payload_wrong_key_foreign_terminal_missing_guard_and_context_fail_closed() {
    let mut source = candidate::<1>(&crate::solver::tests::tadpole(), Default::default());
    let d = source.coefficient_context().base().parameter("d").unwrap();
    let guard = source
        .coefficient_context()
        .lift_base_polynomial(&d.numerator)
        .unwrap();
    Arc::get_mut(&mut source.program)
        .unwrap()
        .source_conditions
        .push(guard);
    let program = CandidateUnitMassProgram::new(&source, [66; 32]);
    let good = baseline(&source, &key([4]));
    for damage in 0..5 {
        let mut bad = good.clone();
        match damage {
            0 => bad.key = ApplicationCacheKey::guarded_root(&key([3])),
            1 => {
                bad.terms.insert(key([99]), program.context().one());
            }
            2 => bad.nonzero_conditions.clear(),
            3 => bad.nonzero_conditions.push(program.context().zero()),
            _ => bad.nonzero_conditions.push(
                CoefficientContext::new(["foreign_descendant"])
                    .parameter("foreign_descendant")
                    .unwrap(),
            ),
        }
        let mut backing = Backing::new(&program);
        backing.records.insert(good.key.clone(), bad);
        let mut cache = program.new_cache(Default::default(), policies()[0]);
        assert!(
            program
                .reduce_guarded_root(
                    &key([5]),
                    &mut cache,
                    Default::default(),
                    Some(&mut backing)
                )
                .is_err()
        );
        assert_eq!(backing.lookups.len(), 2);
        assert_eq!(backing.stores, 0);
        assert_eq!(cache.statistics().completed_reductions, 0);
        assert_eq!(cache.shared_budget().snapshot(), Default::default());
    }
}

#[test]
fn missing_rule_and_non_descending_cycle_are_not_hidden_by_backing_misses() {
    let family = crate::solver::tests::tadpole();
    for cyclic in [false, true] {
        let mut rules = vec![rule(&family, [5], &[([4], 1)])];
        if cyclic {
            rules.push(rule(&family, [4], &[([5], 1)]));
        }
        let record = input([true], None, rules, &[[1]]);
        let source = CandidateReducer::try_new(
            &family,
            [true],
            record.ordering,
            [([true], record.solution)],
            vec![],
            Default::default(),
        )
        .unwrap();
        let program = CandidateUnitMassProgram::new(&source, [67; 32]);
        let mut backing = Backing::new(&program);
        if !cyclic {
            backing.put(ApplicationCacheRecord {
                key: ApplicationCacheKey::guarded_root(&key([4])),
                terms: BTreeMap::from([(key([1]), program.context().one())]),
                nonzero_conditions: Vec::new(),
            });
        }
        let mut cache = program.new_cache(Default::default(), policies()[0]);
        let error = program
            .reduce_guarded_root(
                &key([5]),
                &mut cache,
                Default::default(),
                Some(&mut backing),
            )
            .unwrap_err();
        if cyclic {
            assert!(matches!(
                error,
                CandidateGuardedCacheError::Candidate(
                    CandidateReductionError::NonDescending { .. }
                )
            ));
        } else {
            assert!(
                matches!(error, CandidateGuardedCacheError::Candidate(CandidateReductionError::Uncovered { target }) if target == key([4]))
            );
            assert_eq!(
                backing.lookups,
                vec![ApplicationCacheKey::guarded_root(&key([5]))]
            );
        }
        assert_eq!(cache.shared_budget().snapshot(), Default::default());
        assert_eq!(backing.stores, 0);
    }
}

#[test]
fn entry_rank_bounds_skip_external_descendants_but_allow_native_rank_growth() {
    let family = crate::solver::tests::tadpole();
    let positive = input([true], Some(0), vec![rule(&family, [2], &[([-1], 1)])], &[]);
    let pinched = input(
        [false],
        Some(0),
        vec![rule(&family, [-1], &[([0], 1)])],
        &[[0]],
    );
    let source = CandidateReducer::try_new(
        &family,
        [true],
        positive.ordering,
        [([true], positive.solution), ([false], pinched.solution)],
        vec![],
        Default::default(),
    )
    .unwrap();
    let program = CandidateUnitMassProgram::new(&source, [68; 32]);
    let mut cache = program.new_cache(Default::default(), policies()[0]);
    let child = ApplicationCacheRecord {
        key: ApplicationCacheKey::guarded_root(&key([-1])),
        terms: BTreeMap::from([(key([0]), program.context().integer(99))]),
        nonzero_conditions: Vec::new(),
    };
    cache
        .insert_shared(key([-1]), Arc::new(child.clone()), Default::default())
        .unwrap();
    let mut backing = Backing::new(&program);
    backing.put(child);
    let before = cache.statistics();
    assert!(
        program
            .lookup_completed(
                &key([-1]),
                &mut cache,
                Default::default(),
                &mut Some(&mut backing as &mut dyn GuardedCacheBacking),
                RootUse::Descendant
            )
            .unwrap()
            .is_none()
    );
    assert_eq!(cache.statistics(), before);
    assert!(backing.lookups.is_empty());
    assert!(matches!(
        program.reduce_guarded_root(
            &key([-1]),
            &mut cache,
            Default::default(),
            Some(&mut backing)
        ),
        Err(CandidateGuardedCacheError::Candidate(
            CandidateReductionError::OutsideNumeratorRank { .. }
        ))
    ));
    let result = program
        .reduce_guarded_root(
            &key([2]),
            &mut cache,
            Default::default(),
            Some(&mut backing),
        )
        .unwrap();
    assert_eq!(result.record, baseline(&source, &key([2])));
    assert_eq!(result.work.rule_applications(), 2);
    assert!(!backing.lookups.iter().any(|key| key.powers() == [-1]));
    assert_eq!(cache.statistics().descendant_ram_hits, 0);
    assert_only_outer_census(&cache);
}

#[test]
fn imported_child_pressure_evicts_outer_records_and_cleans_up_after_refusal_or_panic() {
    let source = candidate::<1>(&crate::solver::tests::tadpole(), Default::default());
    let program = CandidateUnitMassProgram::new(&source, [69; 32]);
    for policy in policies() {
        let mut cache = program.new_cache(Default::default(), policy);
        program
            .reduce_guarded_root(&key([4]), &mut cache, Default::default(), None)
            .unwrap();
        let result = program
            .reduce_guarded_root(
                &key([5]),
                &mut cache,
                ReductionLimits {
                    max_cached_integrals: 2,
                    ..Default::default()
                },
                None,
            )
            .unwrap();
        assert_eq!(result.record, baseline(&source, &key([5])));
        assert_eq!(result.work.rule_applications(), 1);
        assert_eq!(cache.statistics().descendant_ram_hits, 1);
        assert!(cache.statistics().evicted_entries >= 1);
        assert_only_outer_census(&cache);
    }
    for panic in [false, true] {
        let mut cache = program.new_cache(Default::default(), CompletedCachePolicy::FailOnLimit);
        program
            .reduce_guarded_root(&key([4]), &mut cache, Default::default(), None)
            .unwrap();
        let before = cache.shared_budget().snapshot();
        if panic {
            cache
                .shared_budget()
                .set_pressure_handler(Arc::new(|| panic!("imported candidate pressure panic")));
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            program.reduce_guarded_root(
                &key([5]),
                &mut cache,
                ReductionLimits {
                    max_cached_integrals: 1,
                    ..Default::default()
                },
                None,
            )
        }));
        if panic {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(cache.shared_budget().snapshot(), before);
        assert_eq!(cache.peek(&key([4])).unwrap(), baseline(&source, &key([4])));
        assert!(cache.peek(&key([5])).is_none());
    }
}

#[test]
fn imported_child_has_one_native_memo_charge_and_failed_parent_keeps_only_completed_roots() {
    let source = candidate::<1>(&crate::solver::tests::tadpole(), Default::default());
    let program = CandidateUnitMassProgram::new(&source, [70; 32]);
    let mut cache = program.new_cache(Default::default(), CompletedCachePolicy::FailOnLimit);
    program
        .reduce_guarded_root(&key([4]), &mut cache, Default::default(), None)
        .unwrap();
    let mut tight = ReductionLimits::default();
    tight.exact_algebra.max_polynomial_terms = 4;
    assert!(
        program
            .reduce_guarded_root(&key([5]), &mut cache, tight, None)
            .is_err()
    );
    assert_eq!(cache.statistics().descendant_ram_hits, 1);
    assert!(cache.peek(&key([5])).is_none());
    assert_only_outer_census(&cache);
    let result = program
        .reduce_guarded_root(
            &key([5]),
            &mut cache,
            ReductionLimits {
                max_cached_integrals: 3,
                max_rule_applications: 1,
                ..Default::default()
            },
            None,
        )
        .unwrap();
    assert_eq!(result.record, baseline(&source, &key([5])));
    assert_eq!(cache.statistics().cached_integrals, 2);
    assert_only_outer_census(&cache);
}

#[test]
fn installed_weighted_child_reuses_exact_canonical_rows_and_inherited_guards() {
    use crate::reduction::terminal_normalization::TerminalNormalizationPlan;
    let (family, raw, terminal) = super::terminal_weighted_audit::fixture();
    let plan = TerminalNormalizationPlan::vacuum_quadratic_numerators(
        &family,
        &raw,
        OrderingPolicy::SpiredUncutV1,
        Default::default(),
    )
    .unwrap();
    assert_eq!(plan.terms()[&terminal].len(), 2);
    let root = [3, 1, 1, 1, -1, 0];
    let child = [2, 1, 1, 1, -1, 0];
    let terminal_powers = [1, 1, 1, 1, -1, 0];
    let mut sectors = BTreeMap::<[bool; 6], Vec<[i16; 6]>>::new();
    for key in &raw {
        sectors
            .entry(std::array::from_fn(|i| key.powers()[i] > 0))
            .or_default()
            .push(std::array::from_fn(|i| {
                i16::try_from(key.powers()[i]).unwrap()
            }));
    }
    for representation in [
        CandidateCacheRepresentation::Sparse,
        CandidateCacheRepresentation::Factorized,
    ] {
        let inputs = sectors.iter().map(|(sector, terminals)| {
            let rules = if *sector == [true, true, true, true, false, false] {
                vec![
                    rule(&family, root, &[(child, 1)]),
                    rule(&family, child, &[(terminal_powers, 1)]),
                ]
            } else {
                Vec::new()
            };
            (*sector, input(*sector, None, rules, terminals).solution)
        });
        let mut source = CandidateReducer::try_new(
            &family,
            [true; 6],
            OrderingPolicy::SpiredUncutV1,
            inputs,
            vec![],
            Default::default(),
        )
        .unwrap();
        source.set_cache_representation(representation).unwrap();
        source.install_terminal_normalization(plan.clone()).unwrap();
        let d = source.coefficient_context().base().parameter("d").unwrap();
        let guard = source
            .coefficient_context()
            .lift_base_polynomial(&d.numerator)
            .unwrap();
        Arc::get_mut(&mut source.program)
            .unwrap()
            .source_conditions
            .push(guard);
        let program = CandidateUnitMassProgram::new(&source, [71; 32]);
        let mut cache = program.new_cache(Default::default(), policies()[0]);
        program
            .reduce_guarded_root(
                &key(child.map(i64::from)),
                &mut cache,
                Default::default(),
                None,
            )
            .unwrap();
        let result = program
            .reduce_guarded_root(
                &key(root.map(i64::from)),
                &mut cache,
                Default::default(),
                None,
            )
            .unwrap();
        assert_eq!(result.record, baseline(&source, &key(root.map(i64::from))));
        assert_eq!(result.record.terms, plan.terms()[&terminal]);
        assert!(result.record.nonzero_conditions.contains(&d));
        // Current verified weighted rows have constant denominators; every
        // possible nonconstant row pole must nevertheless remain an obligation.
        for coefficient in plan.terms()[&terminal].values() {
            if !coefficient.denominator.is_constant() {
                assert!(
                    result
                        .record
                        .nonzero_conditions
                        .contains(&coefficient.denominator.clone().into())
                );
            }
        }
        assert_eq!(cache.statistics().descendant_ram_hits, 1);
        assert_eq!(result.work.rule_applications(), 1);
        assert_only_outer_census(&cache);
    }
}
