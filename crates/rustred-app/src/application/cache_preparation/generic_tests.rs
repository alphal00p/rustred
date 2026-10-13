//! Independent generic Candidate façade tests. These fixtures declare leaves
//! only: no rule search or nonterminal-coverage claim is made.
use super::*;
use crate::{
    AppErrorKind, SavedApplicationCacheBundle, SavedApplicationCacheBundleWriter,
    SavedApplicationCachePackLimits,
};
use rustred::{
    algebra::CoefficientContext,
    family::{AffineDenominator, IntegralFamily, IntegralKey},
    persistence::application_cache::{ApplicationCacheKey, ApplicationCacheRecord},
    reduction::{CompletedCachePolicy, GuardedRootSource, ReductionLimits},
    sector::OrderingPolicy,
    solver::{CandidateReducer, CandidateUnitMassProgram, Integral, IntegralOrder, SectorSolution},
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rustred-generic-cache-facade-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn key(n: i64) -> IntegralKey {
    IntegralKey::try_new([n]).unwrap()
}

fn candidate_program(kinetic_scale: i64) -> CandidateUnitMassProgram<1> {
    // Native unit-mass candidates deliberately require the canonical d
    // context. Distinguish namespaces by valid denominator geometry instead
    // of constructing an inadmissible alternative dimension-parameter map.
    let context = CoefficientContext::try_new(["d"]).unwrap();
    let family = IntegralFamily::new(
        format!("generic-cache-test-{kinetic_scale}"),
        vec!["q".into()],
        vec![],
        context.clone(),
        context.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            context.integer(-1),
            vec![context.integer(kinetic_scale)],
        )],
        vec![],
        vec![context.zero()],
    )
    .unwrap();
    let solution = SectorSolution {
        order: IntegralOrder::new([true], [false]),
        finite_case_policy: Default::default(),
        max_numerator_rank: Some(0),
        rules: vec![],
        finite_residuals: vec![Integral::numeric([2]).unwrap()],
        stats: Default::default(),
    };
    let reducer = CandidateReducer::try_new(
        &family,
        [true],
        OrderingPolicy::SpiredUncutV1,
        [([true], solution)],
        vec![],
        Default::default(),
    )
    .unwrap();
    // Domain-separated identity for this exact in-memory test declaration,
    // not a claimed shipped artifact or certified reduction program.
    let mut digest = blake3::Hasher::new();
    digest.update(b"generic-candidate-cache-test-leaf2-rank0-v1\0");
    digest.update(&kinetic_scale.to_le_bytes());
    CandidateUnitMassProgram::new(&reducer, *digest.finalize().as_bytes())
}

fn guarded_leaf(program: &CandidateUnitMassProgram<1>, pole: i64) -> ApplicationCacheRecord {
    let mut cache = program.new_cache(
        Default::default(),
        CompletedCachePolicy::EvictLeastRecentlyUsed,
    );
    let mut record = program
        .reduce_guarded_root(&key(2), &mut cache, Default::default(), None)
        .unwrap()
        .record;
    // Preserve an explicitly stronger trusted input obligation which cancels
    // from the leaf identity itself. This tests transport, not inference.
    let d = program
        .context()
        .parameter(&program.context().parameter_names()[0])
        .unwrap();
    let guard = program
        .context()
        .try_sub(&d, &program.context().integer(pole), Default::default())
        .unwrap();
    record
        .nonzero_conditions
        .extend([guard.clone(), program.context().integer(-2), guard]);
    program
        .validate_completed_record(&record, Default::default())
        .unwrap();
    record
}

#[test]
fn candidate_full_snapshot_keeps_distinct_families_unloaded_namespace_and_native_decomposition() {
    let programs = vec![candidate_program(1), candidate_program(2)];
    assert!(
        programs[0]
            .context()
            .has_same_variable_map(programs[1].context())
    );
    assert_ne!(
        programs[0].binding().family_fingerprint,
        programs[1].binding().family_fingerprint
    );
    let records = programs
        .iter()
        .enumerate()
        .map(|(n, p)| guarded_leaf(p, 4 + n as i64))
        .collect::<Vec<_>>();
    assert_ne!(records[0].nonzero_conditions, records[1].nonzero_conditions);
    let foreign = CoefficientContext::try_new(["cache_facade_foreign_parameter"]).unwrap();
    let mut invalid = records[0].clone();
    invalid
        .nonzero_conditions
        .push(foreign.parameter("cache_facade_foreign_parameter").unwrap());
    assert!(
        programs[0]
            .validate_completed_record(&invalid, Default::default())
            .is_err()
    );
    let bindings = programs
        .iter()
        .map(|p| p.binding().clone())
        .collect::<Vec<_>>();
    let scratch = Scratch::new();
    let base = scratch.0.join("different-families");
    let storage = SavedApplicationCachePackLimits::default();
    let mut writer = SavedApplicationCacheBundleWriter::create(&base, &bindings, storage).unwrap();
    for namespace in 0..2 {
        writer
            .append_completed(
                &bindings[namespace],
                &[records[namespace].clone()],
                programs[namespace].context(),
            )
            .unwrap();
    }
    drop(writer);
    let limits = ReductionLimits {
        max_cached_coefficient_bytes: 20_001,
        ..Default::default()
    };
    let mut backend = CandidateCachePreparationBackend::with_policy(
        programs.clone(),
        limits,
        CompletedCachePolicy::EvictAgedLeastFrequentlyUsed,
    )
    .unwrap();
    assert_eq!(
        backend
            .sessions()
            .iter()
            .map(|s| s.limits().max_cached_coefficient_bytes)
            .sum::<usize>(),
        20_000
    );
    backend.attach_bundle(&base, storage).unwrap();
    let (loaded, decomposition) = backend
        .session_mut(0)
        .unwrap()
        .reduce_guarded_root_with_decomposition(&key(2))
        .unwrap();
    assert_eq!(loaded.source, GuardedRootSource::Backing);
    assert_eq!(loaded.record, records[0]);
    assert_eq!(
        decomposition.family_fingerprint(),
        programs[0].binding().family_fingerprint
    );
    assert_eq!(decomposition.terms(), &records[0].terms);
    assert_eq!(decomposition.common_mass_squared_power(&key(2)).unwrap(), 0);
    assert_eq!(
        backend
            .session(1)
            .unwrap()
            .io_statistics()
            .unwrap()
            .chunks_read,
        0
    );
    let before = backend
        .sessions()
        .iter()
        .map(|s| {
            (
                s.cache_statistics(),
                s.io_statistics(),
                s.limits(),
                s.cache_policy(),
            )
        })
        .collect::<Vec<_>>();
    let copied = scratch.0.join("full-snapshot");
    let report = backend
        .save_cache(&copied, CacheSaveMode::FullSnapshot, storage, false)
        .unwrap();
    assert!(report.complete, "{}", report.to_document());
    assert_eq!(report.base_copy.unwrap().copied_bases, 2);
    assert_eq!(report.durable_records, 1, "only namespace0 is resident");
    assert_eq!(
        backend
            .sessions()
            .iter()
            .map(|s| (
                s.cache_statistics(),
                s.io_statistics(),
                s.limits(),
                s.cache_policy()
            ))
            .collect::<Vec<_>>(),
        before
    );
    std::fs::rename(&base, scratch.0.join("old-base-relocated")).unwrap();
    let mut fresh = CandidateCachePreparationBackend::new(programs, limits).unwrap();
    fresh.attach_bundle(&copied, storage).unwrap();
    for namespace in 0..2 {
        let result = fresh
            .session_mut(namespace)
            .unwrap()
            .reduce_guarded_root(&key(2))
            .unwrap();
        assert_eq!(result.source, GuardedRootSource::Backing);
        assert_eq!(result.record, records[namespace]);
        assert_eq!(result.work.rule_applications(), 0);
        assert_eq!(
            fresh
                .session(namespace)
                .unwrap()
                .cache_statistics()
                .completed_reductions,
            0
        );
    }
}

#[test]
fn candidate_session_admission_precedes_backing_and_failed_attachment_keeps_old_session() {
    let program = candidate_program(1);
    let record = guarded_leaf(&program, 4);
    let scratch = Scratch::new();
    let output = scratch.0.join("authority");
    let storage = SavedApplicationCachePackLimits::default();
    let mut writer =
        SavedApplicationCacheBundleWriter::create(&output, &[program.binding().clone()], storage)
            .unwrap();
    let mut forbidden = record.clone();
    forbidden.key = ApplicationCacheKey::guarded_root(&key(-1));
    writer
        .append_completed(
            program.binding(),
            &[forbidden, record.clone()],
            program.context(),
        )
        .unwrap();
    drop(writer);
    let mut session = CandidateApplicationCache::new(
        program.clone(),
        Default::default(),
        CompletedCachePolicy::EvictLeastRecentlyUsed,
    );
    session.attach_bundle(&output, storage).unwrap();
    assert_eq!(
        session.reduce_guarded_root(&key(-1)).unwrap_err().kind(),
        AppErrorKind::Input
    );
    assert_eq!(
        session.io_statistics().unwrap().chunks_read,
        0,
        "rank refusal happens before even present disk record"
    );
    assert_eq!(session.cache_statistics().cached_integrals, 0);
    let hit = session.reduce_guarded_root(&key(2)).unwrap();
    assert_eq!(hit.source, GuardedRootSource::Backing);
    assert_eq!(hit.record, record);
    let before = (session.cache_statistics(), session.io_statistics());
    let other = candidate_program(3);
    let foreign_path = scratch.0.join("foreign");
    drop(
        SavedApplicationCacheBundleWriter::create(
            &foreign_path,
            &[other.binding().clone()],
            storage,
        )
        .unwrap(),
    );
    let bundle = SavedApplicationCacheBundle::open_generated(
        &foreign_path,
        &[other.binding().clone()],
        storage,
    )
    .unwrap();
    assert!(
        session
            .attach_pack(bundle.open_reader(other.binding()).unwrap())
            .is_err()
    );
    assert_eq!(
        (session.cache_statistics(), session.io_statistics()),
        before
    );
    assert_eq!(
        session.reduce_guarded_root(&key(2)).unwrap().source,
        GuardedRootSource::Ram
    );
    assert_eq!(
        session.reduce_guarded_root(&key(3)).unwrap_err().kind(),
        AppErrorKind::Execution
    );
    assert_eq!(
        session.cache_statistics().completed_reductions,
        0,
        "unknown key was not promoted to a terminal"
    );
    session.clear_ram().unwrap();
    assert_eq!(session.cache_statistics().cached_integrals, 0);
    assert_eq!(
        session.reduce_guarded_root(&key(2)).unwrap().source,
        GuardedRootSource::Backing
    );
}
