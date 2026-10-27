use super::*;
use rustred::family::IntegralKey;
use std::collections::BTreeMap;

fn scratch() -> super::super::super::tests::Scratch {
    super::super::super::tests::Scratch::new()
}
fn binding() -> ApplicationCacheBinding {
    ApplicationCacheBinding {
        artifact_digest: [93; 32],
        family_fingerprint: "all-completed-versions".into(),
        physical_arity: 1,
        layer: "raw-declared-unit-mass-v1".into(),
    }
}
fn context() -> CoefficientContext {
    CoefficientContext::try_new(["export_visitor_d"]).unwrap()
}
fn record(power: i64) -> ApplicationCacheRecord {
    let context = context();
    let d = context.parameter("export_visitor_d").unwrap();
    ApplicationCacheRecord {
        key: ApplicationCacheKey::route(&IntegralKey::try_new([power]).unwrap()),
        terms: BTreeMap::from([(IntegralKey::try_new([1]).unwrap(), context.one())]),
        nonzero_conditions: vec![d.clone(), context.integer(-2), d],
    }
}
fn pack(path: &Path, versions: &[Vec<ApplicationCacheRecord>]) -> SavedApplicationCachePack {
    let mut writer =
        SavedApplicationCacheWriter::create(path, binding(), Default::default()).unwrap();
    for rows in versions {
        writer.append_completed(rows, &context()).unwrap();
    }
    writer.checkpoint().unwrap();
    drop(writer);
    SavedApplicationCachePack::open_generated(path, &binding(), Default::default()).unwrap()
}
fn options() -> SavedApplicationRunningCacheOptions {
    SavedApplicationRunningCacheOptions {
        checkpoint_interval: Duration::from_secs(60),
        ..Default::default()
    }
}
fn capture_only(session: &mut SavedApplicationRunningCache, record: &ApplicationCacheRecord) {
    session
        .with_capture(|capture| {
            capture
                .store_completed(record, &context())
                .map_err(|error| AppError::from_native(error, AppErrorKind::Execution))
        })
        .unwrap();
}

#[test]
fn all_versions_pack_scan_preserves_shadowed_guard_variants_and_frozen_index() {
    let scratch = scratch();
    let path = scratch.0.join("all-versions");
    let mut variant = record(2);
    variant.nonzero_conditions.swap(0, 1);
    let mut captured = pack(
        &path,
        &[vec![record(1), record(2)], vec![variant.clone(), record(3)]],
    );
    let captured_generation = captured.generation();
    let mut writer =
        SavedApplicationCacheWriter::resume(&path, &binding(), Default::default()).unwrap();
    writer.append_completed(&[record(4)], &context()).unwrap();
    writer.checkpoint().unwrap();
    drop(writer);
    let mut rows = Vec::new();
    assert_eq!(
        captured
            .visit_all_completed_records(&context(), |record| {
                rows.push(record);
                Ok(())
            })
            .unwrap(),
        4
    );
    assert_eq!(rows, vec![variant.clone(), record(3), record(1), record(2)]);
    assert_eq!(captured.generation(), captured_generation);
    assert_eq!(captured.statistics().chunks_read, 2);
    assert_eq!(captured.statistics().hits, 0);
    assert_eq!(captured.statistics().misses, 0);
    let mut latest = Vec::new();
    assert_eq!(
        captured
            .visit_completed_records(&context(), |record| {
                latest.push(record);
                Ok(())
            })
            .unwrap(),
        3
    );
    assert_eq!(latest, vec![variant, record(3), record(1)]);
}

#[test]
fn all_versions_scan_preserves_callback_and_aggregate_coefficient_limits() {
    let scratch = scratch();
    let path = scratch.0.join("all-limits");
    let captured = pack(&path, &[vec![record(1)], vec![record(1)]]);
    let limits = super::super::super::SavedApplicationCachePackLimits::default();
    let mut tight = limits;
    tight.max_directory_owned_bytes = captured.statistics().directory_owned_bytes;
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(), tight).unwrap();
    // Unlike latest-key scans, an all-version stream allocates no seen-key set.
    assert_eq!(
        reader
            .visit_all_completed_records(&context(), |_| Ok(()))
            .unwrap(),
        2
    );
    tight = limits;
    tight.chunk.diagnostics_enabled = true;
    tight.chunk.max_decoded_coefficient_terms = record(1).coefficient_census().unwrap().0;
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(), tight).unwrap();
    let mut callbacks = 0;
    assert!(
        reader
            .visit_all_completed_records(&context(), |_| {
                callbacks += 1;
                Ok(())
            })
            .is_err()
    );
    assert_eq!(callbacks, 1);
    let sentinel = AppError::limit("explicit export callback failed");
    let mut reader = captured.fork_reader();
    let mut callbacks = 0;
    assert_eq!(
        reader
            .visit_all_completed_records(&context(), |_| {
                callbacks += 1;
                Err(sentinel.clone())
            })
            .unwrap_err(),
        sentinel
    );
    assert_eq!(callbacks, 1);
}

#[test]
fn read_only_all_bases_preserve_file_and_pack_duplicate_references() {
    let scratch = scratch();
    let mut variant = record(1);
    variant.nonzero_conditions.swap(0, 1);
    let packed = pack(
        &scratch.0.join("readonly-base"),
        &[vec![record(1)], vec![variant.clone()]],
    );
    let (bytes, _) = SavedApplicationCacheFile::encode_generated(
        &binding(),
        &[record(1), record(2)],
        &context(),
        Default::default(),
    )
    .unwrap();
    let file = SavedApplicationCacheFile::decode(&bytes, &binding(), Default::default()).unwrap();
    let read_only = SavedApplicationReadOnlyCache::new(
        binding(),
        context(),
        vec![
            SavedApplicationRunningCacheBase::Pack(packed),
            SavedApplicationRunningCacheBase::File(file),
        ],
    )
    .unwrap();
    let mut rows = Vec::new();
    assert_eq!(
        read_only
            .visit_all_completed_records(&context(), |record| {
                rows.push(record);
                Ok(())
            })
            .unwrap(),
        4
    );
    assert_eq!(rows, vec![variant, record(1), record(1), record(2)]);
    if let SavedApplicationRunningCacheBase::File(file) = &read_only.bases[1] {
        assert_eq!(file.statistics().unwrap().hits, 0);
        assert_eq!(file.statistics().unwrap().misses, 0);
    } else {
        unreachable!();
    }
    if let SavedApplicationRunningCacheBase::Pack(pack) = &read_only.bases[0] {
        assert_eq!(pack.statistics().chunks_read, 0);
    } else {
        unreachable!();
    }
    let foreign = CoefficientContext::try_new(["foreign_export_d"]).unwrap();
    assert!(
        read_only
            .visit_all_completed_records(&foreign, |_| panic!("context failure precedes callbacks"))
            .is_err()
    );
}

#[test]
fn closed_capture_only_export_includes_all_delta_versions_and_explicit_bases() {
    let scratch = scratch();
    let base = pack(
        &scratch.0.join("closed-base"),
        &[vec![record(1)], vec![record(1)]],
    );
    let delta = scratch.0.join("closed-delta");
    let mut session = SavedApplicationRunningCache::open(
        binding(),
        context(),
        &delta,
        vec![SavedApplicationRunningCacheBase::Pack(base)],
        options(),
    )
    .unwrap();
    let initial_index = std::fs::read(delta.join("cache.rrindex")).unwrap();
    assert!(
        session
            .visit_closed_completed_records(&context(), |_| panic!("live writer must not emit"))
            .is_err()
    );
    assert!(!session.control().report().unwrap().closed);
    assert_eq!(
        std::fs::read(delta.join("cache.rrindex")).unwrap(),
        initial_index
    );
    capture_only(&mut session, &record(10));
    let mut variant = record(10);
    variant.nonzero_conditions.swap(0, 1);
    capture_only(&mut session, &variant);
    let closed = session.control().close().unwrap();
    assert!(closed.closed && closed.pending_records == 0 && closed.writer_error.is_none());
    let captured_generation = session.delta.generation();
    assert!(closed.generation > captured_generation);
    let mut rows = Vec::new();
    assert_eq!(
        session
            .visit_closed_completed_records(&context(), |record| {
                rows.push(record);
                Ok(())
            })
            .unwrap(),
        4
    );
    assert_eq!(rows, vec![variant, record(10), record(1), record(1)]);
    assert_eq!(session.delta.generation(), captured_generation);
    assert_eq!(
        session.control().report().unwrap().generation,
        closed.generation
    );
}

#[test]
fn closed_export_rejects_changed_generation_and_failed_or_pending_writer() {
    let scratch = scratch();
    let delta = scratch.0.join("stale-delta");
    let mut session =
        SavedApplicationRunningCache::open(binding(), context(), &delta, Vec::new(), options())
            .unwrap();
    capture_only(&mut session, &record(1));
    session.control().close().unwrap();
    let mut writer =
        SavedApplicationCacheWriter::resume(&delta, &binding(), Default::default()).unwrap();
    writer.append_completed(&[record(2)], &context()).unwrap();
    writer.checkpoint().unwrap();
    drop(writer);
    assert!(
        session
            .visit_closed_completed_records(&context(), |_| panic!(
                "generation failure precedes callbacks"
            ))
            .unwrap_err()
            .message()
            .contains("generation")
    );

    let failed_delta = scratch.0.join("failed-delta");
    let mut failed = SavedApplicationRunningCache::open(
        binding(),
        context(),
        &failed_delta,
        Vec::new(),
        options(),
    )
    .unwrap();
    std::fs::remove_dir(failed_delta.join("chunks")).unwrap();
    assert!(
        failed
            .with_capture(|capture| capture
                .store_completed(&record(1), &context())
                .map_err(|error| AppError::from_native(error, AppErrorKind::Execution)))
            .is_err()
    );
    assert!(failed.control().close().is_err());
    let report = failed.control().report().unwrap();
    assert!(report.closed && report.writer_error.is_some());
    assert_eq!(
        failed
            .visit_closed_completed_records(&context(), |_| panic!(
                "writer failure precedes callbacks"
            ))
            .unwrap_err(),
        report.writer_error.unwrap()
    );

    let pending_delta = scratch.0.join("pending-delta");
    let pending = SavedApplicationRunningCache::open(
        binding(),
        context(),
        &pending_delta,
        Vec::new(),
        options(),
    )
    .unwrap();
    pending.control().close().unwrap();
    pending.control.shared.lock().unwrap().pending = 1;
    assert!(
        pending
            .visit_closed_completed_records(&context(), |_| panic!(
                "pending failure precedes callbacks"
            ))
            .unwrap_err()
            .message()
            .contains("uncommitted")
    );
}
