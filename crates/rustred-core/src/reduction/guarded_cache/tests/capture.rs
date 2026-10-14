//! Actual computed descendants, not roots separately selected by warmup.
use super::*;
use crate::algebra::Coefficient;
use crate::reduction::{GuardedCaptureStatistics, GuardedIntermediateCapture};
use std::collections::HashSet;

#[derive(Default)]
struct Capture {
    records: BTreeMap<ApplicationCacheKey, ApplicationCacheRecord>,
    wanted: Vec<ApplicationCacheKey>,
    statistics: GuardedCaptureStatistics,
    metadata_limit: Option<usize>,
    fail_after: Option<usize>,
    panic_after: Option<usize>,
}
impl GuardedIntermediateCapture for Capture {
    fn wants(
        &mut self,
        key: &ApplicationCacheKey,
        _: &CoefficientContext,
    ) -> Result<bool, GuardedCacheBackingError> {
        self.wanted.push(key.clone());
        Ok(!self.records.contains_key(key))
    }
    fn store_completed(
        &mut self,
        record: &ApplicationCacheRecord,
        _: &CoefficientContext,
    ) -> Result<(), GuardedCacheBackingError> {
        assert_ne!(
            self.panic_after,
            Some(self.records.len()),
            "injected capture panic"
        );
        if self.fail_after == Some(self.records.len()) {
            return Err(GuardedCacheBackingError {
                kind: GuardedCacheBackingErrorKind::Io,
                message: "injected capture write failure".into(),
            });
        }
        self.records.insert(record.key.clone(), record.clone());
        Ok(())
    }
    fn metadata_limit_bytes(&self) -> Option<usize> {
        self.metadata_limit
    }
    fn report_statistics(&mut self, statistics: GuardedCaptureStatistics) {
        self.statistics = statistics;
    }
}

fn guards(record: &ApplicationCacheRecord) -> HashSet<Coefficient> {
    record.nonzero_conditions.iter().cloned().collect()
}

fn native(program: &ClosedUnitMassProgram<'_>, target: &IntegralKey) -> ApplicationCacheRecord {
    let (unit, nonzero_conditions, _) =
        Reducer::reduce_artifact_unit_mass_guarded(program.artifact(), target, Default::default())
            .unwrap();
    ApplicationCacheRecord {
        key: ApplicationCacheKey::guarded_root(target),
        terms: unit.into_terms(),
        nonzero_conditions,
    }
}

fn indexed_guard_artifact() -> crate::foundry::artifact::ClosedArtifact {
    let mut artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    // Test-only strengthening: distinguish each node's guard from its parent's.
    // Coefficients and descent remain the original native tadpole recurrence.
    let guard = {
        let context = artifact.indexed_context();
        let d = context.lift(artifact.family().dimension()).unwrap();
        let guard = context.sub(&d, &context.index(0).unwrap()).unwrap();
        context
            .numerator_condition_with_limits(&guard, Default::default())
            .unwrap()
    };
    artifact.replace_first_raw_rule_guard_for_test(guard);
    artifact
}

#[test]
fn computed_intermediates_have_their_own_guards_and_reuse_from_fresh_backing() {
    let artifact = indexed_guard_artifact();
    let program = ClosedUnitMassProgram::new(&artifact, [71; 32]);
    for policy in [
        CompletedCachePolicy::EvictLeastRecentlyUsed,
        CompletedCachePolicy::EvictAgedLeastFrequentlyUsed,
    ] {
        let mut cache = GuardedRootCache::with_policy(&program, Default::default(), policy);
        let mut capture = Capture::default();
        let result = program
            .reduce_guarded_root_with_capture(
                &key(7),
                &mut cache,
                Default::default(),
                None,
                &mut capture,
            )
            .unwrap();
        assert_eq!(result.record.terms, baseline(&program, 7).terms);
        assert_eq!(guards(&result.record), guards(&baseline(&program, 7)));
        assert_eq!(capture.records.len(), 5);
        assert!(
            !capture
                .records
                .contains_key(&ApplicationCacheKey::guarded_root(&key(7)))
        );
        for n in 2..=6 {
            let record = &capture.records[&ApplicationCacheKey::guarded_root(&key(n))];
            let expected = baseline(&program, n);
            assert_eq!(record.terms, expected.terms);
            assert_eq!(guards(record), guards(&expected));
            assert!(guards(record).len() < guards(&result.record).len());
            program
                .validate_completed_record(record, Default::default())
                .unwrap();
        }
        // Root 7 was the ONLY prepared input; discard it and its RAM cache.
        // Root 8 now benefits from captured I6, which was never a warmup root.
        let mut backing = Backing::new(&program);
        backing.records = capture.records;
        let mut fresh = GuardedRootCache::with_policy(&program, Default::default(), policy);
        let hit = program
            .reduce_guarded_root(
                &key(8),
                &mut fresh,
                ReductionLimits {
                    max_rule_applications: 2,
                    ..Default::default()
                },
                Some(&mut backing),
            )
            .unwrap();
        assert_eq!(hit.work.rule_applications(), 2);
        assert_eq!(fresh.statistics().descendant_backing_hits, 1);
        assert_eq!(hit.record.terms, baseline(&program, 8).terms);
        assert_eq!(guards(&hit.record), guards(&baseline(&program, 8)));
    }
}

#[test]
fn metadata_exhaustion_stops_capture_without_losing_requested_root_conditions() {
    let artifact = indexed_guard_artifact();
    let program = ClosedUnitMassProgram::new(&artifact, [72; 32]);
    let mut cache = GuardedRootCache::new(&program, Default::default());
    let mut capture = Capture {
        metadata_limit: Some(0),
        ..Default::default()
    };
    let result = program
        .reduce_guarded_root_with_capture(
            &key(8),
            &mut cache,
            Default::default(),
            None,
            &mut capture,
        )
        .unwrap();
    assert_eq!(result.record.terms, baseline(&program, 8).terms);
    assert_eq!(guards(&result.record), guards(&baseline(&program, 8)));
    assert!(capture.records.is_empty());
    assert!(capture.statistics.metadata_exhausted);
    assert_eq!(cache.shared_budget().snapshot().integrals, 1);
}

#[test]
fn metadata_exhaustion_inside_factorization_preserves_original_angular_guards() {
    let artifact = crate::foundry::artifact::derive_two_loop_unit_mass_sunset().unwrap();
    let program = ClosedUnitMassProgram::new(&artifact, [75; 32]);
    let target = IntegralKey::try_new([2, 1, -4]).unwrap();
    let expected = native(&program, &target);
    let mut cache = GuardedRootCache::new(&program, Default::default());
    let mut capture = Capture {
        metadata_limit: Some(1024),
        ..Default::default()
    };
    let result = program
        .reduce_guarded_root_with_capture(
            &target,
            &mut cache,
            Default::default(),
            None,
            &mut capture,
        )
        .unwrap();
    assert_eq!(result.record.terms, expected.terms);
    assert_eq!(guards(&result.record), guards(&expected));
    assert!(
        result
            .record
            .nonzero_conditions
            .contains(artifact.family().dimension())
    );
    assert!(capture.statistics.metadata_exhausted);
    assert!(capture.statistics.peak_metadata_bytes > 0);
    assert!(
        capture.statistics.nodes >= 2,
        "exhaustion occurs after entering the lower family"
    );
    assert_eq!(cache.shared_budget().snapshot().integrals, 1);
}

#[test]
fn imported_opaque_child_guards_flow_to_only_the_completed_ancestors() {
    let artifact = indexed_guard_artifact();
    let program = ClosedUnitMassProgram::new(&artifact, [76; 32]);
    let mut imported = baseline(&program, 4);
    let extra = program
        .context()
        .try_sub(
            artifact.family().dimension(),
            &program.context().integer(99),
            Default::default(),
        )
        .unwrap();
    let constant = program.context().integer(-2);
    imported
        .nonzero_conditions
        .extend([extra.clone(), constant.clone()]);
    let mut backing = Backing::new(&program);
    backing.put(imported);
    let mut cache = GuardedRootCache::new(&program, Default::default());
    let mut capture = Capture::default();
    let result = program
        .reduce_guarded_root_with_capture(
            &key(7),
            &mut cache,
            Default::default(),
            Some(&mut backing),
            &mut capture,
        )
        .unwrap();
    assert_eq!(result.work.rule_applications(), 3);
    assert_eq!(capture.records.len(), 2);
    for n in [5, 6] {
        let record = &capture.records[&ApplicationCacheKey::guarded_root(&key(n))];
        let expected = baseline(&program, n);
        let mut expected_guards = guards(&expected);
        expected_guards.extend([extra.clone(), constant.clone()]);
        assert_eq!(record.terms, expected.terms);
        assert_eq!(guards(record), expected_guards);
        assert!(guards(record).len() < guards(&result.record).len());
    }
}

#[test]
fn failed_or_panicking_sink_leaves_only_valid_completed_prefix_and_no_root() {
    let artifact = indexed_guard_artifact();
    let program = ClosedUnitMassProgram::new(&artifact, [73; 32]);
    for panic in [false, true] {
        let mut cache = GuardedRootCache::new(&program, Default::default());
        let shared = cache.shared_budget();
        let mut capture = Capture {
            fail_after: (!panic).then_some(2),
            panic_after: panic.then_some(2),
            ..Default::default()
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            program.reduce_guarded_root_with_capture(
                &key(8),
                &mut cache,
                Default::default(),
                None,
                &mut capture,
            )
        }));
        if panic {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(capture.records.len(), 2);
        assert!(cache.peek(&key(8)).is_none());
        assert_eq!(shared.snapshot(), Default::default());
        for record in capture.records.values() {
            let target = IntegralKey::try_new(record.key.powers().iter().copied()).unwrap();
            let expected = native(&program, &target);
            assert_eq!(record.terms, expected.terms);
            assert_eq!(guards(record), guards(&expected));
        }
    }
}

#[test]
fn closed_factorization_capture_retains_angular_poles_and_only_own_subtree_guards() {
    let artifact = crate::foundry::artifact::derive_two_loop_unit_mass_sunset().unwrap();
    let program = ClosedUnitMassProgram::new(&artifact, [74; 32]);
    let mut all_records = 0;
    for powers in [[2, 2, 2], [3, 2, 1], [1, 1, -4], [2, 1, -2]] {
        let target = IntegralKey::try_new(powers).unwrap();
        let expected = native(&program, &target);
        let mut cache = GuardedRootCache::new(&program, Default::default());
        let mut capture = Capture::default();
        let result = program
            .reduce_guarded_root_with_capture(
                &target,
                &mut cache,
                Default::default(),
                None,
                &mut capture,
            )
            .unwrap();
        assert_eq!(result.record.terms, expected.terms);
        assert_eq!(guards(&result.record), guards(&expected));
        if powers[2] < 0 {
            assert!(
                result
                    .record
                    .nonzero_conditions
                    .contains(artifact.family().dimension())
            );
        }
        for record in capture.records.values() {
            let target = IntegralKey::try_new(record.key.powers().iter().copied()).unwrap();
            let expected = native(&program, &target);
            assert_eq!(record.terms, expected.terms);
            assert_eq!(
                guards(record),
                guards(&expected),
                "guard cross-talk at {target:?}"
            );
            program
                .validate_completed_record(record, Default::default())
                .unwrap();
            // Existing mass restoration is valid for the captured row too.
            let decomposition = program
                .homogeneous_decomposition(record, Default::default())
                .unwrap();
            let native = program
                .homogeneous_decomposition(&expected, Default::default())
                .unwrap();
            assert_eq!(decomposition, native);
        }
        all_records += capture.records.len();
        assert_eq!(cache.shared_budget().snapshot().integrals, 1);
    }
    assert!(
        all_records > 0,
        "exercise captured nontrivial closed intermediates"
    );
}
