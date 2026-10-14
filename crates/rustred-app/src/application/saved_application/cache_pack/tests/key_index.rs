use super::*;

fn fixture(path: &Path) -> SavedApplicationCacheWriter {
    let mut writer =
        SavedApplicationCacheWriter::create(path, binding(1), Default::default()).unwrap();
    writer
        .append_completed(&[record(0), record(10)], &context())
        .unwrap();
    writer.checkpoint().unwrap();
    writer
}

#[test]
fn repeated_range_misses_skip_io_but_genuine_hits_keep_exact_records_and_guards() {
    let scratch = scratch();
    let path = scratch.0.join("negative-prefilter");
    drop(fixture(&path));
    let mut reader =
        SavedApplicationCachePack::open_generated(&path, &binding(1), Default::default()).unwrap();
    let baseline = reader.statistics().directory_owned_bytes;
    assert_eq!(reader.lookup(&key(5), &context()).unwrap(), None);
    let first = reader.statistics();
    assert!(first.directory_owned_bytes > baseline);
    assert_eq!(reader.lookup(&key(6), &context()).unwrap(), None);
    let second = reader.statistics();
    assert_eq!(second.chunks_read, first.chunks_read);
    assert_eq!(second.decoded_chunk_bytes, first.decoded_chunk_bytes);
    assert_eq!(
        second.lookup_encoded_bytes_charged,
        first.lookup_encoded_bytes_charged
    );
    assert_eq!(
        second.lookup_decoded_bytes_charged,
        first.lookup_decoded_bytes_charged
    );
    assert_eq!(second.key_index_skips, 1);
    assert_eq!(
        reader.lookup(&key(10), &context()).unwrap(),
        Some(record(10))
    );
    assert_eq!(reader.statistics().chunks_read, first.chunks_read + 1);
    assert_eq!(
        reader.statistics().directory_owned_bytes,
        first.directory_owned_bytes
    );
    // Presence metadata never replaces native context or coefficient validation.
    let foreign = CoefficientContext::try_new(["key_index_foreign_context"]).unwrap();
    assert!(reader.lookup(&key(10), &foreign).is_err());
    assert_eq!(reader.statistics().hits, 1);
}

#[test]
fn exact_directory_allowance_disables_optional_retention_without_changing_results() {
    let scratch = scratch();
    let path = scratch.0.join("bounded-prefilter");
    drop(fixture(&path));
    let base = SavedApplicationCachePack::open_generated(&path, &binding(1), Default::default())
        .unwrap()
        .statistics()
        .directory_owned_bytes;
    for allowance in [base, base + 1] {
        let limits = SavedApplicationCachePackLimits {
            max_directory_owned_bytes: allowance,
            ..Default::default()
        };
        let mut reader =
            SavedApplicationCachePack::open_generated(&path, &binding(1), limits).unwrap();
        for n in [5, 6] {
            assert_eq!(reader.lookup(&key(n), &context()).unwrap(), None);
        }
        assert_eq!(reader.statistics().chunks_read, 2);
        assert_eq!(reader.statistics().key_index_skips, 0);
        assert_eq!(reader.statistics().directory_owned_bytes, base);
        assert_eq!(
            reader.lookup(&key(10), &context()).unwrap(),
            Some(record(10))
        );
    }
}

#[test]
fn same_range_chunks_in_different_runs_do_not_alias_exact_membership() {
    let scratch = scratch();
    let path = scratch.0.join("distinct-descriptors");
    let mut writer =
        SavedApplicationCacheWriter::create(&path, binding(1), Default::default()).unwrap();
    writer
        .append_completed(&[record(0), record(5), record(10)], &context())
        .unwrap();
    writer
        .append_completed(&[record(0), record(4), record(10)], &context())
        .unwrap();
    writer.checkpoint().unwrap();
    drop(writer);
    let mut reader =
        SavedApplicationCachePack::open_generated(&path, &binding(1), Default::default()).unwrap();
    assert_eq!(reader.lookup(&key(6), &context()).unwrap(), None);
    assert_eq!(reader.statistics().chunks_read, 2);
    assert_eq!(reader.lookup(&key(5), &context()).unwrap(), Some(record(5)));
    assert_eq!(reader.statistics().chunks_read, 3);
    assert_eq!(reader.statistics().key_index_skips, 1);
}

#[test]
fn fork_noop_changed_and_failed_refresh_keep_snapshot_metadata_ownership_exact() {
    let scratch = scratch();
    let path = scratch.0.join("prefilter-refresh");
    let mut writer = fixture(&path);
    let mut reader =
        SavedApplicationCachePack::open_generated(&path, &binding(1), Default::default()).unwrap();
    let base = reader.statistics().directory_owned_bytes;
    reader.lookup(&key(5), &context()).unwrap();
    let primed = reader.statistics();
    reader.refresh().unwrap();
    assert_eq!(reader.statistics(), primed);
    let mut fork = reader.fork_reader();
    assert_eq!(fork.statistics().directory_owned_bytes, base);
    assert_eq!(fork.statistics().key_index_skips, 0);
    fork.lookup(&key(6), &context()).unwrap();
    assert_eq!(fork.statistics().chunks_read, 1);
    assert_eq!(reader.statistics(), primed);
    let index_path = path.join(index::MANIFEST);
    let bytes = fs::read(&index_path).unwrap();
    fs::write(&index_path, &bytes[..bytes.len() - 1]).unwrap();
    assert!(reader.refresh().is_err());
    assert_eq!(reader.statistics(), primed);
    reader.lookup(&key(7), &context()).unwrap();
    assert_eq!(reader.statistics().key_index_skips, 1);
    fs::write(&index_path, bytes).unwrap();
    // A disjoint insertion shifts the old chunk's slot within the same run.
    writer
        .append_completed(&[record(-10), record(-5)], &context())
        .unwrap();
    writer.checkpoint().unwrap();
    reader.refresh().unwrap();
    let fresh =
        SavedApplicationCachePack::open_generated(&path, &binding(1), Default::default()).unwrap();
    assert_eq!(
        reader.statistics().directory_owned_bytes,
        fresh.statistics().directory_owned_bytes
    );
    assert_eq!(
        reader.lookup(&key(-5), &context()).unwrap(),
        Some(record(-5))
    );
    assert_eq!(
        reader.statistics().key_index_skips,
        1,
        "cumulative telemetry survives refresh"
    );
}

#[test]
fn failed_chunk_context_and_descriptor_checks_never_install_absence_metadata() {
    let scratch = scratch();
    let path = scratch.0.join("failed-prefilter-admission");
    drop(fixture(&path));
    let descriptor = manifest(&path).runs[0][0].clone();
    let chunk_path = path.join("chunks").join(descriptor.file_name());
    let original = fs::read(&chunk_path).unwrap();
    let mut reader =
        SavedApplicationCachePack::open_generated(&path, &binding(1), Default::default()).unwrap();
    let base = reader.statistics().directory_owned_bytes;
    let mut damaged = original.clone();
    damaged[0] ^= 1;
    fs::write(&chunk_path, damaged).unwrap();
    assert!(reader.lookup(&key(5), &context()).is_err());
    assert_eq!(reader.statistics().directory_owned_bytes, base);
    fs::write(&chunk_path, &original).unwrap();
    let foreign = CoefficientContext::try_new(["key_index_failed_context"]).unwrap();
    assert!(reader.lookup(&key(10), &foreign).is_err());
    assert_eq!(reader.statistics().directory_owned_bytes, base);
    let before = reader.statistics().chunks_read;
    assert_eq!(reader.lookup(&key(5), &context()).unwrap(), None);
    assert_eq!(reader.statistics().chunks_read, before + 1);
    assert!(reader.statistics().directory_owned_bytes > base);
    assert_eq!(reader.statistics().key_index_skips, 0);
    let mut damaged = original;
    damaged[0] ^= 1;
    fs::write(&chunk_path, damaged).unwrap();
    assert!(
        reader.lookup(&key(10), &context()).is_err(),
        "retained presence metadata never bypasses a fresh integrity check"
    );
}

#[test]
fn native_framing_must_match_the_authenticated_descriptor_before_retention() {
    let scratch = scratch();
    let path = scratch.0.join("descriptor-prefilter-admission");
    drop(fixture(&path));
    let mut declared = manifest(&path);
    declared.runs[0][0].records += 1;
    fs::write(
        path.join(index::MANIFEST),
        index::encode(&declared, Default::default()).unwrap(),
    )
    .unwrap();
    let mut reader =
        SavedApplicationCachePack::open_generated(&path, &binding(1), Default::default()).unwrap();
    let baseline = reader.statistics().directory_owned_bytes;
    for _ in 0..2 {
        assert!(reader.lookup(&key(5), &context()).is_err());
        assert_eq!(reader.statistics().directory_owned_bytes, baseline);
        assert_eq!(reader.statistics().key_index_skips, 0);
    }
    assert_eq!(reader.statistics().chunks_read, 2);
}
