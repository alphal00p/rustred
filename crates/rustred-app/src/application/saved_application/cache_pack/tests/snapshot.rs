use super::*;

fn copy(
    path: &Path,
    bindings: &[ApplicationCacheBinding],
    bases: &[&SavedApplicationCachePack],
    limits: SavedApplicationCachePackLimits,
) -> Result<
    (
        SavedApplicationCacheBundleWriter,
        SavedApplicationCacheCopyStatistics,
    ),
    crate::AppError,
> {
    SavedApplicationCacheBundleWriter::copy_bases_to_new(path, bindings, bases, limits)
}

#[test]
fn base_copy_preserves_unloaded_shadowed_records_captured_generation_and_resident_overlay() {
    let scratch = scratch();
    let source = scratch.0.join("source");
    let output = scratch.0.join("snapshot");
    let limits = SavedApplicationCachePackLimits::default();
    let mut writer = SavedApplicationCacheWriter::create(&source, binding(1), limits).unwrap();
    writer
        .append_completed(&[record(1), record(3)], &context())
        .unwrap();
    let mut replacement = record(1);
    replacement.terms.clear();
    writer
        .append_completed(&[replacement.clone()], &context())
        .unwrap();
    writer.checkpoint().unwrap();
    let captured = SavedApplicationCachePack::open_generated(&source, &binding(1), limits).unwrap();
    let old_manifest = manifest(&source);
    writer.append_completed(&[record(4)], &context()).unwrap();
    writer.checkpoint().unwrap(); // A later checkpoint must NOT enter this copy.
    let source_index = fs::read(source.join(index::MANIFEST)).unwrap();
    let before = captured.statistics();
    let (mut copied, stats) = copy(&output, &[binding(1)], &[&captured], limits).unwrap();
    assert_eq!(stats.copied_bases, 1);
    assert_eq!(stats.copied_chunks, 2);
    assert_eq!(stats.preserved_record_references, 3);
    assert_eq!(stats.transfer_buffer_bytes, 64 * 1024);
    assert_eq!(
        copied.charged_disk_bytes().unwrap(),
        stats.compressed_bytes + stats.index_bytes
    );
    assert_eq!(
        captured.statistics(),
        before,
        "copy performs no lookup/native decode"
    );
    assert_eq!(
        fs::read(source.join(index::MANIFEST)).unwrap(),
        source_index
    );
    let child = child_path(&output, &binding(1));
    let copied_index = manifest(&child);
    assert_eq!(copied_index.runs, old_manifest.runs);
    assert_eq!(copied_index.generation, captured.generation());
    assert!(copied_index.bundle_managed);
    for chunk in old_manifest.runs.iter().flatten() {
        let from = source.join("chunks").join(chunk.file_name());
        let to = child.join("chunks").join(chunk.file_name());
        assert_eq!(fs::read(&from).unwrap(), fs::read(&to).unwrap());
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let from = fs::metadata(from).unwrap();
            let to = fs::metadata(to).unwrap();
            assert_ne!((from.dev(), from.ino()), (to.dev(), to.ino()));
        }
    }
    let bundle =
        SavedApplicationCacheBundle::open_generated(&output, &[binding(1)], limits).unwrap();
    assert!(
        bundle.metadata().unwrap().is_none(),
        "a snapshot is not a preparation-job checkpoint"
    );
    let mut reader = bundle.open_reader(&binding(1)).unwrap();
    assert_eq!(
        reader.lookup(&key(1), &context()).unwrap(),
        Some(replacement)
    );
    assert_eq!(reader.lookup(&key(3), &context()).unwrap(), Some(record(3)));
    assert_eq!(reader.lookup(&key(4), &context()).unwrap(), None);
    copied
        .append_completed(&binding(1), &[record(1), record(2)], &context())
        .unwrap();
    reader.refresh().unwrap();
    for n in [1, 2, 3] {
        assert_eq!(reader.lookup(&key(n), &context()).unwrap(), Some(record(n)));
    }
    assert_eq!(
        fs::read(source.join(index::MANIFEST)).unwrap(),
        source_index
    );
    drop(writer);
    fs::rename(&source, scratch.0.join("relocated-source")).unwrap();
    assert!(!source.exists());
    for n in [1, 2, 3] {
        assert_eq!(reader.lookup(&key(n), &context()).unwrap(), Some(record(n)));
    }
}

#[test]
fn snapshot_exact_namespaces_existing_output_and_aggregate_quota_fail_before_creation() {
    let scratch = scratch();
    let limits = SavedApplicationCachePackLimits::default();
    let mut bases = Vec::new();
    for id in [1, 2] {
        let path = scratch.0.join(format!("base-{id}"));
        let mut writer = SavedApplicationCacheWriter::create(&path, binding(id), limits).unwrap();
        writer
            .append_completed(&[record(i64::from(id))], &context())
            .unwrap();
        writer.checkpoint().unwrap();
        bases.push(SavedApplicationCachePack::open_generated(&path, &binding(id), limits).unwrap());
    }
    for (name, bindings, selected) in [
        ("unknown", vec![binding(1)], vec![&bases[1]]),
        ("duplicate", vec![binding(1)], vec![&bases[0], &bases[0]]),
        (
            "changed-layer",
            vec![ApplicationCacheBinding {
                layer: "different".into(),
                ..binding(1)
            }],
            vec![&bases[0]],
        ),
    ] {
        let path = scratch.0.join(name);
        assert!(copy(&path, &bindings, &selected, limits).is_err());
        assert!(!path.exists());
    }
    let path = scratch.0.join("full");
    let bindings = [binding(1), binding(2), binding(3)];
    let (writer, stats) = copy(&path, &bindings, &[&bases[1], &bases[0]], limits).unwrap();
    let used = writer.charged_disk_bytes().unwrap();
    assert_eq!(stats.namespaces, 3);
    assert_eq!(stats.copied_bases, 2);
    assert_eq!(stats.copied_chunks, 2);
    drop(writer);
    assert!(copy(&path, &bindings, &[&bases[0], &bases[1]], limits).is_err());
    let tight_path = scratch.0.join("too-tight");
    assert_eq!(
        copy(
            &tight_path,
            &bindings,
            &[&bases[0], &bases[1]],
            SavedApplicationCachePackLimits {
                max_disk_bytes: used - 1,
                ..limits
            }
        )
        .err()
        .unwrap()
        .kind(),
        crate::AppErrorKind::Limit
    );
    assert!(!tight_path.exists());
    let bundle = SavedApplicationCacheBundle::open_generated(&path, &bindings, limits).unwrap();
    for id in [1, 2] {
        assert_eq!(
            bundle
                .open_reader(&binding(id))
                .unwrap()
                .lookup(&key(i64::from(id)), &context())
                .unwrap(),
            Some(record(i64::from(id)))
        );
    }
    assert_eq!(
        bundle
            .open_reader(&binding(3))
            .unwrap()
            .statistics()
            .records,
        0
    );
}

#[test]
fn corrupted_base_leaves_an_unpublished_partial_destination_and_never_mutates_source() {
    let scratch = scratch();
    let source = scratch.0.join("source");
    let output = scratch.0.join("partial");
    let limits = SavedApplicationCachePackLimits::default();
    let mut writer = SavedApplicationCacheWriter::create(&source, binding(1), limits).unwrap();
    for n in [1, 2] {
        writer.append_completed(&[record(n)], &context()).unwrap();
    }
    writer.checkpoint().unwrap();
    drop(writer);
    let base = SavedApplicationCachePack::open_generated(&source, &binding(1), limits).unwrap();
    let mut chunks = manifest(&source)
        .runs
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    chunks.sort_by_key(|chunk| chunk.digest);
    let bad_path = source.join("chunks").join(chunks[1].file_name());
    let mut corrupted = fs::read(&bad_path).unwrap();
    corrupted[20] ^= 1;
    fs::write(&bad_path, &corrupted).unwrap();
    let source_index = fs::read(source.join(index::MANIFEST)).unwrap();
    assert!(copy(&output, &[binding(1)], &[&base], limits).is_err());
    assert!(output.is_dir());
    assert!(!output.join("bundle.rrindex").exists());
    assert!(SavedApplicationCacheBundle::open_generated(&output, &[binding(1)], limits).is_err());
    let children = fs::read_dir(output.join("bindings"))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(
        physical_chunks(&children[0].path()).0,
        1,
        "complete copied prefix retained, corrupt chunk not installed"
    );
    assert_eq!(
        fs::read(source.join(index::MANIFEST)).unwrap(),
        source_index
    );
    assert_eq!(fs::read(bad_path).unwrap(), corrupted);
    assert!(
        copy(&output, &[binding(1)], &[&base], limits).is_err(),
        "no implicit partial-copy resume"
    );
}

#[test]
fn copied_base_native_state_and_guards_roundtrip_in_a_fresh_process() {
    const CHILD: &str = "RUSTRED_CACHE_COPIED_BASE_CHILD";
    if let Some(path) = std::env::var_os(CHILD) {
        let _shift =
            CoefficientContext::try_new(["copied_base_foreign_a", "copied_base_foreign_b"])
                .unwrap();
        let bundle = SavedApplicationCacheBundle::open_generated(
            Path::new(&path),
            &[binding(1)],
            Default::default(),
        )
        .unwrap();
        let mut reader = bundle.open_reader(&binding(1)).unwrap();
        for n in [0, 1, 3] {
            assert_eq!(reader.lookup(&key(n), &context()).unwrap(), Some(record(n)));
        }
        return;
    }
    let scratch = scratch();
    let source = scratch.0.join("source");
    let output = scratch.0.join("copy");
    let limits = SavedApplicationCachePackLimits::default();
    let mut writer = SavedApplicationCacheWriter::create(&source, binding(1), limits).unwrap();
    writer
        .append_completed(&[record(0), record(1), record(3)], &context())
        .unwrap();
    writer.checkpoint().unwrap();
    drop(writer);
    let base = SavedApplicationCachePack::open_generated(&source, &binding(1), limits).unwrap();
    drop(copy(&output, &[binding(1)], &[&base], limits).unwrap());
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "application::saved_application::cache_pack::tests::snapshot::copied_base_native_state_and_guards_roundtrip_in_a_fresh_process", "--nocapture", "--test-threads=1"])
        .env(CHILD, &output).output().unwrap();
    assert!(
        child.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&child.stdout),
        String::from_utf8_lossy(&child.stderr)
    );
    assert!(String::from_utf8_lossy(&child.stdout).contains("1 passed"));
}

#[test]
fn repeated_references_copy_one_physical_chunk_but_conflicting_descriptors_refuse() {
    let scratch = scratch();
    let source = scratch.0.join("duplicates");
    let output = scratch.0.join("copy");
    let limits = SavedApplicationCachePackLimits::default();
    let mut writer = SavedApplicationCacheWriter::create(&source, binding(1), limits).unwrap();
    for _ in 0..2 {
        writer.append_completed(&[record(1)], &context()).unwrap();
    }
    writer.checkpoint().unwrap();
    drop(writer);
    let base = SavedApplicationCachePack::open_generated(&source, &binding(1), limits).unwrap();
    let (writer, stats) = copy(&output, &[binding(1)], &[&base], limits).unwrap();
    assert_eq!(stats.copied_chunks, 1);
    assert_eq!(stats.preserved_record_references, 2);
    assert_eq!(physical_chunks(&child_path(&output, &binding(1))).0, 1);
    assert_eq!(
        writer.charged_disk_bytes().unwrap(),
        stats.compressed_bytes + stats.index_bytes
    );
    let mut conflicting = manifest(&source);
    conflicting.runs[1][0].decoded_digest[0] ^= 1;
    fs::write(
        source.join(index::MANIFEST),
        index::encode(&conflicting, limits).unwrap(),
    )
    .unwrap();
    let base = SavedApplicationCachePack::open_generated(&source, &binding(1), limits).unwrap();
    let rejected = scratch.0.join("conflict");
    assert!(copy(&rejected, &[binding(1)], &[&base], limits).is_err());
    assert!(!rejected.exists());
}
