use super::*;
use crate::algebra::CoefficientContext;
use crate::persistence::application_cache::{
    ApplicationCacheBinding, ApplicationCacheChunk, ApplicationCacheKey, ApplicationCacheRecord,
    encode_application_cache_chunk,
};
use crate::reduction::{
    CompletedCachePolicy, GuardedCacheBacking, GuardedCacheBackingError,
    GuardedCacheBackingErrorKind, GuardedCacheError, GuardedCaptureStatistics,
    GuardedIntermediateCapture,
};
use crate::solver::candidate_reduction::owner_test_support::{input, rule};
use crate::solver::{
    CandidateCacheRepresentation, CandidateGuardedCacheError, CandidateUnitMassProgram,
};

#[derive(Default)]
struct Capture {
    records: Vec<ApplicationCacheRecord>,
    statistics: GuardedCaptureStatistics,
    metadata: Option<usize>,
    fail_after: Option<usize>,
    interested: bool,
}
impl Capture {
    fn all() -> Self {
        Self {
            interested: true,
            ..Default::default()
        }
    }
}
impl GuardedIntermediateCapture for Capture {
    fn wants(
        &mut self,
        _: &ApplicationCacheKey,
        _: &CoefficientContext,
    ) -> Result<bool, GuardedCacheBackingError> {
        Ok(self.interested)
    }
    fn store_completed(
        &mut self,
        record: &ApplicationCacheRecord,
        _: &CoefficientContext,
    ) -> Result<(), GuardedCacheBackingError> {
        if self.fail_after == Some(self.records.len()) {
            return Err(GuardedCacheBackingError {
                kind: GuardedCacheBackingErrorKind::Io,
                message: "injected capture write failure".into(),
            });
        }
        self.records.push(record.clone());
        Ok(())
    }
    fn metadata_limit_bytes(&self) -> Option<usize> {
        self.metadata
    }
    fn report_statistics(&mut self, statistics: GuardedCaptureStatistics) {
        self.statistics = statistics;
    }
}

struct Reload {
    binding: ApplicationCacheBinding,
    bytes: Vec<u8>,
}
impl GuardedCacheBacking for Reload {
    fn binding(&self) -> &ApplicationCacheBinding {
        &self.binding
    }
    fn lookup(
        &mut self,
        key: &ApplicationCacheKey,
        context: &CoefficientContext,
    ) -> Result<Option<ApplicationCacheRecord>, GuardedCacheBackingError> {
        let bad = |e: String| GuardedCacheBackingError {
            kind: GuardedCacheBackingErrorKind::Invalid,
            message: e,
        };
        ApplicationCacheChunk::inspect(&self.bytes, &self.binding, Default::default())
            .map_err(|e| bad(e.to_string()))?
            .decode_record(key, context)
            .map_err(|e| bad(e.to_string()))
    }
    fn store_completed(
        &mut self,
        _: &ApplicationCacheRecord,
        _: &CoefficientContext,
    ) -> Result<(), GuardedCacheBackingError> {
        Ok(())
    }
}

fn expected<const N: usize>(
    source: &CandidateReducer<N>,
    target: &IntegralKey,
) -> ApplicationCacheRecord {
    let (value, nonzero_conditions, _) = source
        .fork_empty()
        .reduce_unit_mass_guarded(target, Default::default())
        .unwrap();
    ApplicationCacheRecord {
        key: ApplicationCacheKey::guarded_root(target),
        terms: value.into_terms(),
        nonzero_conditions,
    }
}

#[test]
fn newly_computed_unselected_intermediate_survives_native_codec_and_shortcuts_a_different_root() {
    for representation in [
        CandidateCacheRepresentation::Sparse,
        CandidateCacheRepresentation::Factorized,
    ] {
        let mut source = candidate::<1>(&crate::solver::tests::tadpole(), Default::default());
        source.set_cache_representation(representation).unwrap();
        let program = CandidateUnitMassProgram::new(&source, [87; 32]);
        let mut cache = program.new_cache(
            Default::default(),
            CompletedCachePolicy::EvictLeastRecentlyUsed,
        );
        let mut sink = Capture::all();
        let first = program
            .reduce_guarded_root_with_capture(
                &key([5]),
                &mut cache,
                Default::default(),
                None,
                &mut sink,
            )
            .unwrap();
        assert_eq!(first.record, expected(&source, &key([5])));
        assert_eq!(sink.records.len(), 4);
        assert_eq!(sink.statistics.captured_records, 4);
        assert!(sink.records.iter().all(|r| r.key.powers() != [5]));
        for record in &sink.records {
            let key = IntegralKey::try_new(record.key.powers().iter().copied()).unwrap();
            assert_eq!(*record, expected(&source, &key));
        }
        sink.records.sort_by(|a, b| a.key.cmp(&b.key));
        let bytes = encode_application_cache_chunk(
            program.binding(),
            &sink.records,
            program.context(),
            Default::default(),
        )
        .unwrap();
        let mut reloaded = Reload {
            binding: program.binding().clone(),
            bytes,
        };
        let mut cold_cache = program.new_cache(
            Default::default(),
            CompletedCachePolicy::EvictAgedLeastFrequentlyUsed,
        );
        // Neither input 6 nor selected preparation root 5 is in the backing.
        // The newly captured intermediate 4 cuts the five-step path to two.
        let result = program
            .reduce_guarded_root(
                &key([6]),
                &mut cold_cache,
                ReductionLimits {
                    diagnostics_enabled: true,
                    max_rule_applications: 2,
                    ..Default::default()
                },
                Some(&mut reloaded),
            )
            .unwrap();
        assert_eq!(result.record, expected(&source, &key([6])));
        assert_eq!(result.work.rule_applications(), 2);
        assert_eq!(cold_cache.statistics().descendant_backing_hits, 1);
    }
}

#[test]
fn enriched_pack_retains_prepared_root_and_shortcuts_an_unseen_sibling_that_never_visits_it() {
    let family = crate::solver::tests::tadpole();
    let prepared = input(
        [true],
        None,
        vec![
            rule(&family, [6], &[([4], 1)]),
            rule(&family, [5], &[([4], 1)]),
            rule(&family, [4], &[([3], 1)]),
            rule(&family, [3], &[([2], 1)]),
            rule(&family, [2], &[([1], 1)]),
        ],
        &[[1]],
    );
    let source = CandidateReducer::try_new(
        &family,
        [true],
        prepared.ordering,
        [([true], prepared.solution)],
        vec![],
        Default::default(),
    )
    .unwrap();
    let program = CandidateUnitMassProgram::new(&source, [92; 32]);
    let mut cache = program.new_cache(
        Default::default(),
        CompletedCachePolicy::EvictLeastRecentlyUsed,
    );
    let mut sink = Capture::all();
    let root = program
        .reduce_guarded_root_with_capture(
            &key([6]),
            &mut cache,
            Default::default(),
            None,
            &mut sink,
        )
        .unwrap();
    let root_only = encode_application_cache_chunk(
        program.binding(),
        std::slice::from_ref(&root.record),
        program.context(),
        Default::default(),
    )
    .unwrap();
    sink.records.push(root.record);
    sink.records.sort_by(|a, b| a.key.cmp(&b.key));
    let enriched = encode_application_cache_chunk(
        program.binding(),
        &sink.records,
        program.context(),
        Default::default(),
    )
    .unwrap();
    let one = ReductionLimits {
        diagnostics_enabled: true,
        max_rule_applications: 1,
        ..Default::default()
    };
    for (bytes, success) in [(root_only, false), (enriched, true)] {
        let mut backing = Reload {
            binding: program.binding().clone(),
            bytes,
        };
        let mut cache = program.new_cache(
            Default::default(),
            CompletedCachePolicy::EvictLeastRecentlyUsed,
        );
        let result = program.reduce_guarded_root(&key([5]), &mut cache, one, Some(&mut backing));
        if success {
            let result = result.unwrap();
            assert_eq!(result.record, expected(&source, &key([5])));
            assert_eq!(result.work.rule_applications(), 1);
            assert_eq!(cache.statistics().descendant_backing_hits, 1);
        } else {
            assert!(matches!(
                result,
                Err(CandidateGuardedCacheError::Candidate(
                    CandidateReductionError::Application(
                        ReductionError::RuleApplicationLimit { .. }
                    )
                ))
            ));
        }
    }
}

#[test]
fn capture_respects_entry_scope_without_clipping_out_of_scope_native_descendants() {
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
    let program = CandidateUnitMassProgram::new(&source, [93; 32]);
    let mut cache = program.new_cache(
        Default::default(),
        CompletedCachePolicy::EvictLeastRecentlyUsed,
    );
    let mut sink = Capture::all();
    let result = program
        .reduce_guarded_root_with_capture(
            &key([2]),
            &mut cache,
            Default::default(),
            None,
            &mut sink,
        )
        .unwrap();
    assert_eq!(result.record, expected(&source, &key([2])));
    assert_eq!(result.work.rule_applications(), 2);
    assert!(sink.records.iter().all(|r| r.key.powers() != [-1]));
    assert!(sink.statistics.skipped_records > 0);
}

#[test]
fn imported_guarded_child_is_an_opaque_provenance_leaf_in_capture_and_soft_disabled_modes() {
    let source = super::root_guards::guarded_fixture(false);
    let program = CandidateUnitMassProgram::new(&source, [95; 32]);
    let extra = program.context().coefficient_fixture("d-999");
    let mut child = expected(&source, &key([4]));
    child.nonzero_conditions.push(extra.clone());
    let bytes = encode_application_cache_chunk(
        program.binding(),
        &[child],
        program.context(),
        Default::default(),
    )
    .unwrap();
    for metadata in [None, Some(0)] {
        let mut backing = Reload {
            binding: program.binding().clone(),
            bytes: bytes.clone(),
        };
        let mut cache = program.new_cache(
            Default::default(),
            CompletedCachePolicy::EvictLeastRecentlyUsed,
        );
        let mut sink = Capture {
            metadata,
            ..Capture::all()
        };
        let result = program
            .reduce_guarded_root_with_capture(
                &key([6]),
                &mut cache,
                Default::default(),
                Some(&mut backing),
                &mut sink,
            )
            .unwrap();
        assert_eq!(result.record.terms, expected(&source, &key([6])).terms);
        let mut guards = expected(&source, &key([6])).nonzero_conditions;
        guards.push(extra.clone());
        assert_eq!(result.record.nonzero_conditions, guards);
        assert_eq!(result.work.rule_applications(), 2);
        assert!(sink.records.iter().all(|r| r.key.powers() != [4]));
        if metadata.is_none() {
            let captured = sink.records.iter().find(|r| r.key.powers() == [5]).unwrap();
            let mut expected = expected(&source, &key([5]));
            expected.nonzero_conditions.push(extra.clone());
            assert_eq!(*captured, expected);
        } else {
            assert!(sink.records.is_empty());
        }
    }
}

#[test]
fn optional_factorized_intermediate_record_refusal_does_not_abort_a_cancelling_parent() {
    let family = crate::solver::tests::tadpole();
    let prepared = input(
        [true],
        None,
        vec![
            rule(&family, [5], &[([4], 1), ([3], -1)]),
            rule(&family, [4], &[([1], 1)]),
            rule(&family, [3], &[([1], 1)]),
        ],
        &[[1]],
    );
    let mut source = CandidateReducer::try_new(
        &family,
        [true],
        prepared.ordering,
        [([true], prepared.solution)],
        vec![],
        Default::default(),
    )
    .unwrap();
    source
        .set_cache_representation(CandidateCacheRepresentation::Factorized)
        .unwrap();
    let context = source.program.context.clone();
    let coefficient = context.base().coefficient_fixture("1/(d-1)^256");
    let lifted = context.lift(&coefficient).unwrap();
    let denominator = context
        .lift_base_polynomial(&coefficient.denominator)
        .unwrap();
    for rule in Arc::get_mut(&mut source.program)
        .unwrap()
        .rules
        .get_mut(&[true])
        .unwrap()
        .iter_mut()
        .skip(1)
    {
        rule.rhs[0].coefficient = lifted.clone();
        rule.rhs[0].denominator = denominator.clone();
    }
    let mut ordinary = source.fork_empty();
    let (reference, guards, _) = ordinary
        .reduce_fresh_unit_mass_guarded(&key([5]), Default::default())
        .unwrap();
    assert!(reference.is_zero());
    let allowed = ordinary.statistics().cached_coefficient_bytes()
        + crate::reduction::coefficient_cache_weight(guards.iter())
            .unwrap()
            .coefficient_bytes;
    let intermediate = expected(&source, &key([4]));
    let intermediate_bytes = crate::reduction::coefficient_cache_weight(
        intermediate
            .terms
            .values()
            .chain(&intermediate.nonzero_conditions),
    )
    .unwrap()
    .coefficient_bytes;
    assert!(
        intermediate_bytes > allowed,
        "fixture must expand larger than complete native factorized DAG: {intermediate_bytes} vs {allowed}"
    );
    let limits = ReductionLimits {
        diagnostics_enabled: true,
        max_cached_coefficient_bytes: allowed,
        ..Default::default()
    };
    let program = CandidateUnitMassProgram::new(&source, [94; 32]);
    let mut cache = program.new_cache(limits, CompletedCachePolicy::EvictLeastRecentlyUsed);
    let mut sink = Capture::all();
    let result = program
        .reduce_guarded_root_with_capture(&key([5]), &mut cache, limits, None, &mut sink)
        .unwrap();
    assert_eq!(result.record.terms, *reference.terms());
    assert_eq!(result.record.nonzero_conditions, guards);
    assert!(sink.statistics.skipped_records >= 2);
    assert!(
        sink.records
            .iter()
            .all(|r| !matches!(r.key.powers(), [3] | [4]))
    );
}

#[test]
fn per_node_guards_match_native_with_diamond_siblings_cancellation_and_source_conditions() {
    for representation in [
        CandidateCacheRepresentation::Sparse,
        CandidateCacheRepresentation::Factorized,
    ] {
        let mut source = super::root_guards::guarded_fixture(true);
        source.set_cache_representation(representation).unwrap();
        let program = CandidateUnitMassProgram::new(&source, [88; 32]);
        let mut cache = program.new_cache(
            Default::default(),
            CompletedCachePolicy::EvictLeastRecentlyUsed,
        );
        let mut sink = Capture::all();
        let result = program
            .reduce_guarded_root_with_capture(
                &key([5]),
                &mut cache,
                Default::default(),
                None,
                &mut sink,
            )
            .unwrap();
        assert_eq!(result.record, expected(&source, &key([5])));
        assert!(!sink.records.is_empty());
        for record in &sink.records {
            let target = IntegralKey::try_new(record.key.powers().iter().copied()).unwrap();
            assert_eq!(*record, expected(&source, &target));
            assert!(record.nonzero_conditions.len() < result.record.nonzero_conditions.len());
        }
    }
}

#[test]
fn capture_metadata_exhaustion_keeps_exact_root_and_successfully_releases_native_ownership() {
    let source = super::root_guards::guarded_fixture(true);
    let program = CandidateUnitMassProgram::new(&source, [89; 32]);
    let mut baseline_cache = program.new_cache(
        Default::default(),
        CompletedCachePolicy::EvictLeastRecentlyUsed,
    );
    let mut baseline_capture = Capture::all();
    let baseline = program
        .reduce_guarded_root_with_capture(
            &key([5]),
            &mut baseline_cache,
            Default::default(),
            None,
            &mut baseline_capture,
        )
        .unwrap();
    assert!(!baseline_capture.statistics.metadata_exhausted);
    assert!(!baseline_capture.records.is_empty());
    let peak = baseline_capture.statistics.peak_metadata_bytes;
    assert!(peak > 1);
    // Exercise immediate, partial and last-growth exhaustion without assuming
    // allocator/vector capacities or a particular representation's byte size.
    for metadata in [0, peak / 2, peak - 1] {
        let mut cache = program.new_cache(
            Default::default(),
            CompletedCachePolicy::EvictLeastRecentlyUsed,
        );
        let mut sink = Capture {
            metadata: Some(metadata),
            ..Capture::all()
        };
        let result = program
            .reduce_guarded_root_with_capture(
                &key([5]),
                &mut cache,
                Default::default(),
                None,
                &mut sink,
            )
            .unwrap();
        assert_eq!(result.record, expected(&source, &key([5])));
        assert_eq!(result.record, baseline.record);
        assert!(sink.statistics.metadata_exhausted);
        assert!(sink.statistics.skipped_records > 0);
        assert_eq!(
            cache.shared_budget().snapshot().coefficient_bytes,
            cache.statistics().cached_coefficient_bytes
        );
    }
}

#[test]
fn interrupted_capture_has_only_completed_exact_prefix_and_no_native_memory_leak() {
    let source = super::root_guards::guarded_fixture(false);
    let program = CandidateUnitMassProgram::new(&source, [90; 32]);
    let mut cache = program.new_cache(
        Default::default(),
        CompletedCachePolicy::EvictLeastRecentlyUsed,
    );
    let mut sink = Capture {
        fail_after: Some(2),
        ..Capture::all()
    };
    assert!(matches!(
        program.reduce_guarded_root_with_capture(
            &key([24]),
            &mut cache,
            Default::default(),
            None,
            &mut sink
        ),
        Err(CandidateGuardedCacheError::Cache(
            GuardedCacheError::Backing(_)
        ))
    ));
    assert_eq!(sink.records.len(), 2);
    assert_eq!(sink.statistics.captured_records, 2);
    assert_eq!(cache.shared_budget().snapshot(), Default::default());
    for record in &sink.records {
        let target = IntegralKey::try_new(record.key.powers().iter().copied()).unwrap();
        assert_eq!(*record, expected(&source, &target));
    }
}

#[test]
fn uninterested_capture_preserves_factorized_native_work_and_tight_guard_term_allowance() {
    let source = super::root_guards::guarded_fixture(false);
    let expected = source
        .fork_empty()
        .reduce_unit_mass_guarded(&key([24]), Default::default())
        .unwrap();
    let mut ordinary = source.fork_empty();
    ordinary.reduce_unit_mass(&key([24])).unwrap();
    let guard_weight = crate::reduction::coefficient_cache_weight(expected.1.iter()).unwrap();
    let limits = ReductionLimits {
        diagnostics_enabled: true,
        max_cached_coefficient_terms: ordinary.statistics().cached_coefficient_terms()
            + guard_weight.coefficient_terms,
        ..Default::default()
    };
    let program = CandidateUnitMassProgram::new(&source, [91; 32]);
    let mut cache = program.new_cache(limits, CompletedCachePolicy::EvictLeastRecentlyUsed);
    let mut sink = Capture::default();
    let result = program
        .reduce_guarded_root_with_capture(&key([24]), &mut cache, limits, None, &mut sink)
        .unwrap();
    assert_eq!(result.record.terms, *expected.0.terms());
    assert_eq!(result.record.nonzero_conditions, expected.1);
    assert_eq!(
        result.work.rule_applications(),
        expected.2.rule_applications()
    );
    assert!(sink.records.is_empty());
    assert!(sink.statistics.skipped_records > 0);
}
