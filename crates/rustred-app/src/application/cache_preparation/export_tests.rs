//! Live-export storage controls using the existing native one-loop test fixture.
use super::*;
use crate::{AppErrorKind, SavedApplicationCacheBundle, SavedApplicationCachePackLimits};
use rustred::{
    family::IntegralKey,
    foundry::artifact::{ClosedArtifact, derive_one_loop_unit_mass_tadpole},
    persistence::application_cache::ApplicationCacheKey,
    reduction::{ClosedUnitMassProgram, CompletedCachePolicy, GuardedRootSource},
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rustred-live-cache-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
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
fn key(power: i64) -> IntegralKey {
    IntegralKey::try_new([power]).unwrap()
}
fn fixture() -> (ClosedArtifact, [u8; 32]) {
    let bytes = derive_one_loop_unit_mass_tadpole()
        .unwrap()
        .encode_durable()
        .unwrap();
    // Pair the decoded fixture with the exact bytes it consumed, not a hash
    // of some other in-memory artifact encoding or a mutable path reread.
    (
        ClosedArtifact::decode_durable(&bytes).unwrap(),
        *blake3::hash(&bytes).as_bytes(),
    )
}

#[test]
fn live_export_is_bounded_guard_preserving_and_does_not_touch_ram_policy() {
    let (artifact, digest) = fixture();
    let mut session = ClosedApplicationCache::new(
        ClosedUnitMassProgram::new(&artifact, digest),
        Default::default(),
        CompletedCachePolicy::EvictAgedLeastFrequentlyUsed,
    );
    let expected = session.reduce_guarded_root(&key(4)).unwrap().record;
    session.reduce_guarded_root(&key(5)).unwrap();
    let before = session.cache_statistics();
    let scratch = Scratch::new();
    let output = scratch.0.join("saved");
    let mut limits = SavedApplicationCachePackLimits::default();
    limits.chunk.max_records = 1;
    let report = session.save_completed(&output, limits, false).unwrap();
    assert!(report.complete, "{}", report.to_document());
    assert!(!report.backing_attached);
    assert_eq!(report.accepted.exported_records, before.cached_integrals);
    assert_eq!(report.durable_records, before.cached_integrals);
    assert_eq!(session.cache_statistics(), before);
    assert_eq!(
        session.cache_policy(),
        CompletedCachePolicy::EvictAgedLeastFrequentlyUsed
    );
    let mut loaded = ClosedApplicationCache::new(
        ClosedUnitMassProgram::new(&artifact, digest),
        Default::default(),
        CompletedCachePolicy::EvictLeastRecentlyUsed,
    );
    loaded.attach_bundle(&output, limits).unwrap();
    assert_eq!(loaded.io_statistics().unwrap().chunks_read, 0);
    let actual = loaded.reduce_guarded_root(&key(4)).unwrap();
    assert_eq!(actual.source, GuardedRootSource::Backing);
    assert_eq!(actual.record, expected);
    assert_eq!(actual.work.rule_applications(), 0);
    assert_eq!(loaded.cache_statistics().completed_reductions, 0);
}

#[test]
fn resident_snapshot_does_not_copy_unloaded_or_evicted_base_records() {
    let (artifact, digest) = fixture();
    let program = || ClosedUnitMassProgram::new(&artifact, digest);
    let mut source = ClosedApplicationCache::new(
        program(),
        Default::default(),
        CompletedCachePolicy::EvictLeastRecentlyUsed,
    );
    source.reduce_guarded_root(&key(4)).unwrap();
    source.reduce_guarded_root(&key(7)).unwrap();
    source.reduce_guarded_root(&key(8)).unwrap();
    let scratch = Scratch::new();
    let base = scratch.0.join("base");
    let output = scratch.0.join("resident");
    let limits = SavedApplicationCachePackLimits::default();
    assert!(
        source
            .save_completed(&base, limits, false)
            .unwrap()
            .complete
    );
    let mut loaded = ClosedApplicationCache::new(
        program(),
        Default::default(),
        CompletedCachePolicy::EvictLeastRecentlyUsed,
    );
    loaded.attach_bundle(&base, limits).unwrap();
    // A lazy attachment is initially empty. Loading then clearing a root
    // must not turn the backing itself into a resident-cache snapshot.
    assert_eq!(loaded.cache_statistics().cached_integrals, 0);
    assert_eq!(
        loaded.reduce_guarded_root(&key(4)).unwrap().source,
        GuardedRootSource::Backing
    );
    loaded.clear_ram().unwrap();
    assert_eq!(
        loaded.reduce_guarded_root(&key(7)).unwrap().source,
        GuardedRootSource::Backing
    );
    let before = loaded.cache_statistics();
    let disk_before = loaded.io_statistics();
    let saved = loaded.save_completed(&output, limits, false).unwrap();
    assert!(saved.complete, "{}", saved.to_document());
    assert!(saved.backing_attached);
    assert_eq!(saved.durable_records, 1);
    assert_eq!(loaded.cache_statistics(), before);
    assert_eq!(loaded.io_statistics(), disk_before);
    let native = loaded.program();
    let bundle = SavedApplicationCacheBundle::open_generated(
        &output,
        std::slice::from_ref(native.binding()),
        limits,
    )
    .unwrap();
    let mut reader = bundle.open_reader(native.binding()).unwrap();
    assert!(
        reader
            .lookup(
                &ApplicationCacheKey::guarded_root(&key(4)),
                native.context()
            )
            .unwrap()
            .is_none()
    );
    assert!(
        reader
            .lookup(
                &ApplicationCacheKey::guarded_root(&key(7)),
                native.context()
            )
            .unwrap()
            .is_some()
    );
    assert!(
        reader
            .lookup(
                &ApplicationCacheKey::guarded_root(&key(8)),
                native.context()
            )
            .unwrap()
            .is_none()
    );
    // The original base remains independently usable and unchanged.
    let base = SavedApplicationCacheBundle::open_generated(
        &base,
        std::slice::from_ref(native.binding()),
        limits,
    )
    .unwrap();
    assert!(
        base.open_reader(native.binding())
            .unwrap()
            .lookup(
                &ApplicationCacheKey::guarded_root(&key(4)),
                native.context()
            )
            .unwrap()
            .is_some()
    );
}

#[test]
fn save_storage_defaults_share_preparation_policy_without_resolving_ram() {
    for loops in [1, 2, 3, 4, 5] {
        let mut preparation = CachePreparationOptions::default();
        preparation.ram_bytes = Some(17);
        let prepared = preparation.resolve(loops, Default::default()).unwrap();
        let saved = CacheSaveOptions::default()
            .resolve(loops, Default::default())
            .unwrap();
        assert_eq!(saved.max_disk_bytes, prepared.disk_bytes);
    }
    let mut options = CacheSaveOptions::default();
    options.disk_bytes = Some(u64::MAX);
    // Disk quotas remain portable u64: saving does not reserve a matching RAM
    // allowance or silently change the live native reduction policy.
    assert_eq!(
        options
            .resolve(8, Default::default())
            .unwrap()
            .max_disk_bytes,
        u64::MAX
    );
    options.disk_bytes = Some(0);
    assert!(options.resolve(1, Default::default()).is_err());
    assert!(
        CacheSaveOptions::default()
            .resolve(0, Default::default())
            .is_err()
    );
    assert!(
        CacheSaveOptions::default()
            .resolve(8, Default::default())
            .is_err()
    );
}

#[test]
fn fresh_preparation_resume_reuses_durable_root_inside_one_rule_budget() {
    let (artifact, digest) = fixture();
    let program = || ClosedUnitMassProgram::new(&artifact, digest);
    let workload = CachePreparationWorkload::explicit(
        vec![program().binding().clone()],
        [(0, key(4)), (0, key(5))],
        2,
    )
    .unwrap();
    let scratch = Scratch::new();
    let output = scratch.0.join("prepared");
    let options = CachePreparationOptions::default()
        .resolve(1, Default::default())
        .unwrap();
    let mut first =
        ClosedCachePreparationBackend::new(vec![program()], Default::default()).unwrap();
    let mut calls = 0;
    let partial = prepare_application_cache(
        &workload,
        &options,
        &output,
        &AtomicBool::new(false),
        &mut first,
        |_| {
            calls += 1;
            if calls == 2 {
                Err(crate::AppError::new(
                    AppErrorKind::Cancelled,
                    "stop before I5",
                ))
            } else {
                Ok(())
            }
        },
    )
    .unwrap();
    assert_eq!(partial.status, CachePreparationStatus::Cancelled);
    assert_eq!(partial.completed_roots, 1);
    let mut strict = rustred::reduction::ReductionLimits::default();
    strict.max_rule_applications = 1;
    let mut resumed = ClosedCachePreparationBackend::new(vec![program()], strict).unwrap();
    let report = prepare_application_cache(
        &workload,
        &options,
        &output,
        &AtomicBool::new(false),
        &mut resumed,
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(
        report.status,
        CachePreparationStatus::Complete,
        "{}",
        report.to_document()
    );
    assert_eq!(report.reused_roots, 1);
    assert_eq!(report.newly_stored_roots, 1);
    assert_eq!(report.completed_roots, 2);
    assert_eq!(report.native_statistics[0]["descendant_backing_hits"], 1);
    assert_eq!(report.native_statistics[0]["rule_applications"], 1);
    assert_eq!(
        report.native_statistics[0]["backing_io"]["hits"], 1,
        "checkpoint refresh must not reset cumulative backing counters"
    );
    assert!(report.directory_owned_bytes > 0);
    let bundle = SavedApplicationCacheBundle::open_generated(
        &output,
        workload.bindings(),
        options.storage_limits,
    )
    .unwrap();
    let expected = program();
    let actual = bundle
        .open_reader(expected.binding())
        .unwrap()
        .lookup(
            &ApplicationCacheKey::guarded_root(&key(5)),
            expected.context(),
        )
        .unwrap()
        .unwrap();
    expected
        .validate_completed_record(&actual, Default::default())
        .unwrap();
    let (reference, guards, _) = rustred::reduction::Reducer::reduce_artifact_unit_mass_guarded(
        &artifact,
        &key(5),
        Default::default(),
    )
    .unwrap();
    assert_eq!(actual.terms, *reference.terms());
    assert_eq!(
        actual
            .nonzero_conditions
            .iter()
            .collect::<std::collections::HashSet<_>>(),
        guards.iter().collect::<std::collections::HashSet<_>>()
    );
}

#[test]
fn failed_live_checkpoint_reports_only_published_prefix_and_can_resume() {
    let (artifact, digest) = fixture();
    let mut session = ClosedApplicationCache::new(
        ClosedUnitMassProgram::new(&artifact, digest),
        Default::default(),
        CompletedCachePolicy::EvictLeastRecentlyUsed,
    );
    session.reduce_guarded_root(&key(4)).unwrap();
    session.reduce_guarded_root(&key(5)).unwrap();
    let before = session.cache_statistics();
    let scratch = Scratch::new();
    let output = scratch.0.join("saved");
    let mut limits = SavedApplicationCachePackLimits::default();
    limits.chunk.max_records = 1;
    limits.max_chunks = 1;
    let partial = session.save_completed(&output, limits, false).unwrap();
    assert!(!partial.complete);
    assert_eq!(partial.accepted.exported_records, 2);
    assert_eq!(partial.durable_records, 1);
    assert_eq!(
        partial.checkpoint_failure.unwrap().kind(),
        AppErrorKind::Limit
    );
    assert_eq!(session.cache_statistics(), before);
    let program = session.program();
    let bundle = SavedApplicationCacheBundle::open_generated(
        &output,
        std::slice::from_ref(program.binding()),
        limits,
    )
    .unwrap();
    let mut reader = bundle.open_reader(program.binding()).unwrap();
    assert!(
        reader
            .lookup(
                &ApplicationCacheKey::guarded_root(&key(4)),
                program.context()
            )
            .unwrap()
            .is_some()
    );
    assert!(
        reader
            .lookup(
                &ApplicationCacheKey::guarded_root(&key(5)),
                program.context()
            )
            .unwrap()
            .is_none()
    );
    limits.max_chunks = 4;
    let resumed = session.save_completed(&output, limits, true).unwrap();
    assert!(resumed.complete, "{}", resumed.to_document());
    assert_eq!(resumed.durable_records, 2);
    assert_eq!(session.cache_statistics(), before);
    let bundle = SavedApplicationCacheBundle::open_generated(
        &output,
        std::slice::from_ref(program.binding()),
        limits,
    )
    .unwrap();
    assert!(
        bundle
            .open_reader(program.binding())
            .unwrap()
            .lookup(
                &ApplicationCacheKey::guarded_root(&key(5)),
                program.context()
            )
            .unwrap()
            .is_some()
    );
}
