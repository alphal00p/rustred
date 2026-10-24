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
fn preparation_captures_unselected_closed_intermediate_for_fresh_disk_application() {
    let (artifact, digest) = fixture();
    let program = || ClosedUnitMassProgram::new(&artifact, digest);
    let mut cold = ClosedApplicationCache::new(
        program(),
        Default::default(),
        CompletedCachePolicy::EvictLeastRecentlyUsed,
    );
    let expected = cold.reduce_guarded_root(&key(4)).unwrap().record;
    let scratch = Scratch::new();
    let output = scratch.0.join("captured-closed");
    let options = CachePreparationOptions::default()
        .resolve(1, Default::default())
        .unwrap();
    let mut backend =
        ClosedCachePreparationBackend::new(vec![program()], Default::default()).unwrap();
    let workload =
        CachePreparationWorkload::explicit(backend.bindings().to_vec(), [(0, key(5))], 1).unwrap();
    let report = prepare_application_cache(
        &workload,
        &options,
        &output,
        &AtomicBool::new(false),
        &mut backend,
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(
        report.status,
        CachePreparationStatus::Complete,
        "{}",
        report.to_document()
    );
    assert_eq!(report.completed_roots, 1);
    assert_eq!(report.newly_stored_roots, 1);
    assert!(report.newly_stored_intermediates >= 1);
    drop(backend);
    let mut fresh = ClosedApplicationCache::new(
        program(),
        Default::default(),
        CompletedCachePolicy::EvictLeastRecentlyUsed,
    );
    fresh
        .attach_bundle(&output, options.storage_limits)
        .unwrap();
    let actual = fresh.reduce_guarded_root(&key(4)).unwrap();
    assert_eq!(actual.source, GuardedRootSource::Backing);
    assert_eq!(actual.work.rule_applications(), 0);
    assert_eq!(actual.record.terms, expected.terms);
    let mut actual_guards = actual.record.nonzero_conditions;
    let mut expected_guards = expected.nonzero_conditions;
    assert_eq!(actual_guards.len(), expected_guards.len());
    for guard in actual_guards.drain(..) {
        let index = expected_guards
            .iter()
            .position(|candidate| *candidate == guard)
            .expect("same exact local guard obligation");
        expected_guards.swap_remove(index);
    }
    assert!(expected_guards.is_empty());
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
    limits.chunk.diagnostics_enabled = true;
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
            .is_ok()
    );
}

#[test]
fn preparation_batch_layout_preserves_native_work_records_and_guards() {
    let (artifact, digest) = fixture();
    let program = || ClosedUnitMassProgram::new(&artifact, digest);
    let workload = CachePreparationWorkload::explicit(
        vec![program().binding().clone()],
        (1..=5).map(|power| (0, key(power))),
        5,
    )
    .unwrap();
    let scratch = Scratch::new();
    let mut expected = None;
    for (batch_records, chunks) in [(4096, 1), (1, 5), (2, 3)] {
        let output = scratch.0.join(format!("batch-{batch_records}"));
        let options = CachePreparationOptions {
            batch_records,
            ..Default::default()
        }
        .resolve(1, Default::default())
        .unwrap();
        let mut backend =
            ClosedCachePreparationBackend::new(vec![program()], Default::default()).unwrap();
        let report = prepare_application_cache(
            &workload,
            &options,
            &output,
            &AtomicBool::new(false),
            &mut backend,
            |_| Ok(()),
        )
        .unwrap();
        assert_eq!(report.status, CachePreparationStatus::Complete);
        assert_eq!(report.newly_stored_roots, 5);
        assert_eq!(report.options["batch_records"], batch_records);
        assert_eq!(options.storage_limits.chunk.max_records, 4096);
        let bundle = SavedApplicationCacheBundle::open_generated(
            &output,
            workload.bindings(),
            options.storage_limits,
        )
        .unwrap();
        let active = program();
        let mut reader = bundle.open_reader(active.binding()).unwrap();
        assert_eq!(reader.statistics().chunks, chunks);
        let records = (1..=5)
            .map(|power| {
                let record = reader
                    .lookup(
                        &ApplicationCacheKey::guarded_root(&key(power)),
                        active.context(),
                    )
                    .unwrap()
                    .unwrap();
                active
                    .validate_completed_record(&record, Default::default())
                    .unwrap();
                record
            })
            .collect::<Vec<_>>();
        let work = [
            "rule_applications",
            "coalescing_additions",
            "completed_reductions",
            "descendant_ram_hits",
        ]
        .map(|name| report.native_statistics[0][name].clone());
        assert!(work[0].as_u64().unwrap() > 0);
        if let Some((expected_records, expected_work)) = &expected {
            assert_eq!(&records, expected_records);
            assert_eq!(&work, expected_work);
        } else {
            expected = Some((records, work));
        }
    }
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
    strict.diagnostics_enabled = true;
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
    limits.chunk.diagnostics_enabled = true;
    limits.chunk.max_records = 1;
    limits.diagnostics_enabled = true;
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

#[test]
fn full_snapshot_facade_preserves_unloaded_base_and_reports_failed_overlay() {
    let (artifact, digest) = fixture();
    let program = || ClosedUnitMassProgram::new(&artifact, digest);
    let mut source = ClosedApplicationCache::new(program(), Default::default(), Default::default());
    source.reduce_guarded_root(&key(4)).unwrap();
    let scratch = Scratch::new();
    let base = scratch.0.join("base");
    let limits = SavedApplicationCachePackLimits::default();
    source.save_completed(&base, limits, false).unwrap();
    let mut loaded = ClosedApplicationCache::new(program(), Default::default(), Default::default());
    loaded.reduce_guarded_root(&key(7)).unwrap();
    loaded.attach_bundle(&base, limits).unwrap();
    assert_eq!(loaded.io_statistics().unwrap().chunks_read, 0);
    let before = loaded.cache_statistics();
    let output = scratch.0.join("full");
    let saved = loaded
        .save_cache(&output, CacheSaveMode::FullSnapshot, limits, false)
        .unwrap();
    assert!(saved.complete, "{}", saved.to_document());
    assert_eq!(saved.base_copy.unwrap().copied_bases, 1);
    assert_eq!(saved.durable_records, 1);
    assert_eq!(loaded.cache_statistics(), before);
    assert_eq!(loaded.io_statistics().unwrap().chunks_read, 0);
    std::fs::rename(&base, scratch.0.join("source-moved")).unwrap();
    let mut independent =
        ClosedApplicationCache::new(program(), Default::default(), Default::default());
    independent.attach_bundle(&output, limits).unwrap();
    assert_eq!(
        independent.reduce_guarded_root(&key(4)).unwrap().source,
        GuardedRootSource::Backing
    );
    assert_eq!(
        independent.reduce_guarded_root(&key(7)).unwrap().source,
        GuardedRootSource::Backing
    );

    loaded.attach_bundle(&output, limits).unwrap();
    loaded.reduce_guarded_root(&key(8)).unwrap();
    let mut tight = limits;
    tight.diagnostics_enabled = true;
    tight.max_chunks = saved.base_copy.unwrap().copied_chunks + 1;
    // The copied full base already uses two chunks; no room for a new overlay.
    assert_eq!(tight.max_chunks, 2);
    let partial_output = scratch.0.join("partial-overlay");
    let partial = loaded
        .save_cache(&partial_output, CacheSaveMode::FullSnapshot, tight, false)
        .unwrap();
    assert!(!partial.complete);
    assert!(partial.base_copy.is_some());
    assert_eq!(partial.durable_records, 0);
    assert_eq!(
        partial.checkpoint_failure.unwrap().kind(),
        AppErrorKind::Limit
    );
    let mut recovered =
        ClosedApplicationCache::new(program(), Default::default(), Default::default());
    recovered.attach_bundle(&partial_output, limits).unwrap();
    assert_eq!(
        recovered.reduce_guarded_root(&key(4)).unwrap().source,
        GuardedRootSource::Backing
    );
}

#[test]
fn optional_lookup_budget_falls_back_but_strict_and_corruption_do_not() {
    let (artifact, digest) = fixture();
    let program = || ClosedUnitMassProgram::new(&artifact, digest);
    let mut source = ClosedApplicationCache::new(program(), Default::default(), Default::default());
    let expected = source.reduce_guarded_root(&key(4)).unwrap().record;
    let scratch = Scratch::new();
    let base = scratch.0.join("base");
    let limits = SavedApplicationCachePackLimits::default();
    source.save_completed(&base, limits, false).unwrap();
    let mut no_reads = limits;
    no_reads.diagnostics_enabled = true;
    no_reads.max_lookup_chunks = 0;
    let mut optional =
        ClosedApplicationCache::new(program(), Default::default(), Default::default());
    optional.attach_bundle(&base, no_reads).unwrap();
    let result = optional.reduce_guarded_root(&key(4)).unwrap();
    assert_eq!(result.source, GuardedRootSource::Reduced);
    assert_eq!(result.record, expected);
    assert!(result.work.rule_applications() > 0);
    assert_eq!(optional.backing_budget_fallbacks(), 1);
    assert_eq!(optional.io_statistics().unwrap().chunks_read, 0);
    let mut strict = ClosedApplicationCache::new(program(), Default::default(), Default::default());
    strict.set_lookup_budget_policy(CacheLookupBudgetPolicy::Strict);
    strict.attach_bundle(&base, no_reads).unwrap();
    assert_eq!(
        strict.reduce_guarded_root(&key(4)).unwrap_err().kind(),
        AppErrorKind::Limit
    );
    assert_eq!(strict.cache_statistics().completed_reductions, 0);
    assert_eq!(strict.backing_budget_fallbacks(), 0);
    let child = std::fs::read_dir(base.join("bindings"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let chunk = std::fs::read_dir(child.join("chunks"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let mut bytes = std::fs::read(&chunk).unwrap();
    bytes[0] ^= 1;
    std::fs::write(chunk, bytes).unwrap();
    let mut corrupt =
        ClosedApplicationCache::new(program(), Default::default(), Default::default());
    corrupt.attach_bundle(&base, limits).unwrap();
    assert_eq!(
        corrupt.reduce_guarded_root(&key(4)).unwrap_err().kind(),
        AppErrorKind::Input
    );
    assert_eq!(corrupt.backing_budget_fallbacks(), 0);
    assert_eq!(corrupt.cache_statistics().completed_reductions, 0);
}

#[test]
fn preparation_root_resume_budget_refusal_remains_strict() {
    let (artifact, digest) = fixture();
    let program = || ClosedUnitMassProgram::new(&artifact, digest);
    let workload =
        CachePreparationWorkload::explicit(vec![program().binding().clone()], [(0, key(4))], 1)
            .unwrap();
    let scratch = Scratch::new();
    let output = scratch.0.join("prepared");
    let mut options = CachePreparationOptions::default()
        .resolve(1, Default::default())
        .unwrap();
    let mut source =
        ClosedCachePreparationBackend::new(vec![program()], Default::default()).unwrap();
    assert_eq!(
        prepare_application_cache(
            &workload,
            &options,
            &output,
            &AtomicBool::new(false),
            &mut source,
            |_| Ok(())
        )
        .unwrap()
        .status,
        CachePreparationStatus::Complete
    );
    options.storage_limits.diagnostics_enabled = true;
    options.storage_limits.max_lookup_chunks = 0;
    let mut fresh =
        ClosedCachePreparationBackend::new(vec![program()], Default::default()).unwrap();
    let report = prepare_application_cache(
        &workload,
        &options,
        &output,
        &AtomicBool::new(false),
        &mut fresh,
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(report.status, CachePreparationStatus::ResourceRefusal);
    assert_eq!(report.reused_roots, 0);
    assert_eq!(report.completed_roots, 0);
    assert_eq!(report.native_statistics[0]["completed_reductions"], 0);
    assert_eq!(report.native_statistics[0]["backing_budget_fallbacks"], 0);
}

#[test]
fn generic_bundle_export_preserves_distinct_native_namespaces_and_aggregate_budget() {
    let (one, digest) = fixture();
    let bytes = rustred::foundry::artifact::derive_two_loop_unit_mass_sunset()
        .unwrap()
        .encode_durable()
        .unwrap();
    let two = ClosedArtifact::decode_durable(&bytes).unwrap();
    let mut limits = rustred::reduction::ReductionLimits::default();
    limits.max_cached_coefficient_bytes = 20_000;
    let mut backend = ClosedCachePreparationBackend::new(
        vec![
            ClosedUnitMassProgram::new(&one, digest),
            ClosedUnitMassProgram::new(&two, *blake3::hash(&bytes).as_bytes()),
        ],
        limits,
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
    let keys = [key(4), IntegralKey::try_new([2, 1, 1]).unwrap()];
    let expected = keys
        .iter()
        .enumerate()
        .map(|(n, key)| {
            backend
                .session_mut(n)
                .unwrap()
                .reduce_guarded_root(key)
                .unwrap()
                .record
        })
        .collect::<Vec<_>>();
    let before = backend
        .sessions()
        .iter()
        .map(|s| s.cache_statistics())
        .collect::<Vec<_>>();
    let scratch = Scratch::new();
    let output = scratch.0.join("bundle");
    let storage = SavedApplicationCachePackLimits::default();
    let report = backend
        .save_cache(&output, CacheSaveMode::FullSnapshot, storage, false)
        .unwrap();
    assert!(report.complete, "{}", report.to_document());
    assert!(!report.backing_attached);
    assert_eq!(report.base_copy.unwrap().copied_bases, 0);
    assert_eq!(report.durable_records, 2);
    assert_eq!(
        backend
            .sessions()
            .iter()
            .map(|s| s.cache_statistics())
            .collect::<Vec<_>>(),
        before
    );
    let bundle =
        SavedApplicationCacheBundle::open_generated(&output, backend.bindings(), storage).unwrap();
    for (namespace, target) in keys.iter().enumerate() {
        let program = backend.session(namespace).unwrap().program();
        let actual = bundle
            .open_reader(program.binding())
            .unwrap()
            .lookup(
                &ApplicationCacheKey::guarded_root(target),
                program.context(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(actual, expected[namespace]);
    }
    let mut attached = ClosedCachePreparationBackend::new(
        vec![
            ClosedUnitMassProgram::new(&one, digest),
            ClosedUnitMassProgram::new(&two, *blake3::hash(&bytes).as_bytes()),
        ],
        limits,
    )
    .unwrap();
    attached.attach_bundle(&output, storage).unwrap();
    let overlay = attached
        .session_mut(0)
        .unwrap()
        .reduce_guarded_root(&key(7))
        .unwrap()
        .record;
    assert_eq!(
        attached
            .session(1)
            .unwrap()
            .io_statistics()
            .unwrap()
            .chunks_read,
        0
    );
    let cloned = scratch.0.join("full-clone");
    let report = attached
        .save_cache(&cloned, CacheSaveMode::FullSnapshot, storage, false)
        .unwrap();
    assert!(report.complete, "{}", report.to_document());
    assert_eq!(report.base_copy.unwrap().copied_bases, 2);
    assert_eq!(
        attached
            .session(1)
            .unwrap()
            .io_statistics()
            .unwrap()
            .chunks_read,
        0
    );
    let copy =
        SavedApplicationCacheBundle::open_generated(&cloned, backend.bindings(), storage).unwrap();
    for (namespace, target) in keys.iter().enumerate() {
        let program = backend.session(namespace).unwrap().program();
        assert_eq!(
            copy.open_reader(program.binding())
                .unwrap()
                .lookup(
                    &ApplicationCacheKey::guarded_root(target),
                    program.context()
                )
                .unwrap()
                .unwrap(),
            expected[namespace]
        );
    }
    let program = backend.session(0).unwrap().program();
    assert_eq!(
        copy.open_reader(program.binding())
            .unwrap()
            .lookup(
                &ApplicationCacheKey::guarded_root(&key(7)),
                program.context()
            )
            .unwrap()
            .unwrap(),
        overlay
    );
}
