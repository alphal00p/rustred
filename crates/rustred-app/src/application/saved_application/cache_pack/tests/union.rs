use super::*;

fn pack(path: &Path, id: u8, runs: &[Vec<ApplicationCacheRecord>]) -> SavedApplicationCachePack {
    let limits = SavedApplicationCachePackLimits::default();
    let mut writer = SavedApplicationCacheWriter::create(path, binding(id), limits).unwrap();
    for run in runs {
        writer.append_completed(run, &context()).unwrap();
        writer.checkpoint().unwrap();
    }
    drop(writer);
    SavedApplicationCachePack::open_generated(path, &binding(id), limits).unwrap()
}

fn union(
    output: &Path,
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
    SavedApplicationCacheBundleWriter::copy_base_union_to_new(output, bindings, bases, limits)
}

#[test]
fn opaque_union_preserves_priority_all_runs_and_physical_dedup_without_decoding() {
    let scratch = scratch();
    let limits = SavedApplicationCachePackLimits::default();
    let first_path = scratch.0.join("first");
    let second_path = scratch.0.join("second");
    let common = vec![record(1), record(3)];
    let mut empty = record(1);
    empty.terms.clear();
    let mut replacement = record(1);
    replacement
        .terms
        .values_mut()
        .for_each(|coefficient| *coefficient = context().integer(7));
    let first = pack(&first_path, 1, &[common, vec![empty.clone()]]);
    let first_manifest = first.captured_snapshot().1.clone();
    // Seed an independent directory from exact native bytes. Independently
    // encoding equal coefficients need not capture the same global-state
    // prefix when other test threads register contexts concurrently.
    drop(SavedApplicationCacheWriter::create(&second_path, binding(1), limits).unwrap());
    let mut common_manifest = first_manifest.clone();
    common_manifest.runs.truncate(1);
    for chunk in common_manifest.runs.iter().flatten() {
        fs::copy(
            first_path.join("chunks").join(chunk.file_name()),
            second_path.join("chunks").join(chunk.file_name()),
        )
        .unwrap();
    }
    fs::write(
        second_path.join(index::MANIFEST),
        index::encode(&common_manifest, limits).unwrap(),
    )
    .unwrap();
    let mut second_writer =
        SavedApplicationCacheWriter::resume(&second_path, &binding(1), limits).unwrap();
    second_writer
        .append_completed(&[replacement.clone(), record(5)], &context())
        .unwrap();
    second_writer.checkpoint().unwrap();
    drop(second_writer);
    let second =
        SavedApplicationCachePack::open_generated(&second_path, &binding(1), limits).unwrap();
    let second_manifest = second.captured_snapshot().1.clone();
    assert_eq!(
        first_manifest.runs[0], second_manifest.runs[0],
        "same native payload has the same descriptor"
    );
    let mut late = SavedApplicationCacheWriter::resume(&first_path, &binding(1), limits).unwrap();
    late.append_completed(&[record(9)], &context()).unwrap();
    late.checkpoint().unwrap();
    drop(late);
    let first_index = fs::read(first_path.join(index::MANIFEST)).unwrap();
    let second_index = fs::read(second_path.join(index::MANIFEST)).unwrap();
    let first_stats = first.statistics();
    let second_stats = second.statistics();
    for reverse in [false, true] {
        let output = scratch
            .0
            .join(if reverse { "second-wins" } else { "first-wins" });
        let bases = if reverse {
            [&second, &first]
        } else {
            [&first, &second]
        };
        let (mut writer, statistics) =
            union(&output, &[binding(1), binding(2)], &bases, limits).unwrap();
        assert_eq!(statistics.namespaces, 2);
        assert_eq!(statistics.copied_bases, 2);
        assert_eq!(statistics.copied_chunks, 3);
        assert_eq!(statistics.preserved_record_references, 7);
        assert_eq!(statistics.transfer_buffer_bytes, 64 * 1024);
        assert_eq!(
            writer.charged_disk_bytes().unwrap(),
            statistics.compressed_bytes + statistics.index_bytes
        );
        let child = child_path(&output, &binding(1));
        let copied = manifest(&child);
        let expected_runs = if reverse {
            [first_manifest.runs.clone(), second_manifest.runs.clone()].concat()
        } else {
            [second_manifest.runs.clone(), first_manifest.runs.clone()].concat()
        };
        assert_eq!(copied.runs, expected_runs);
        assert_eq!(copied.generation, 1);
        assert!(copied.bundle_managed);
        assert_eq!(physical_chunks(&child).0, 3);
        for base in [&first, &second] {
            let (source, captured) = base.captured_snapshot();
            for chunk in captured.runs.iter().flatten() {
                let from = source.join("chunks").join(chunk.file_name());
                let to = child.join("chunks").join(chunk.file_name());
                assert_eq!(fs::read(&from).unwrap(), fs::read(&to).unwrap());
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    let source = fs::metadata(from).unwrap();
                    let copied = fs::metadata(to).unwrap();
                    assert_ne!((source.dev(), source.ino()), (copied.dev(), copied.ino()));
                }
            }
        }
        let bundle =
            SavedApplicationCacheBundle::open_generated(&output, &[binding(1), binding(2)], limits)
                .unwrap();
        assert!(bundle.metadata().unwrap().is_none());
        let mut reader = bundle.open_reader(&binding(1)).unwrap();
        assert_eq!(
            reader.lookup(&key(1), &context()).unwrap(),
            Some(if reverse {
                replacement.clone()
            } else {
                empty.clone()
            })
        );
        for n in [3, 5] {
            assert_eq!(reader.lookup(&key(n), &context()).unwrap(), Some(record(n)));
        }
        assert_eq!(
            reader.lookup(&key(9), &context()).unwrap(),
            None,
            "later source checkpoint is excluded"
        );
        assert_eq!(
            bundle
                .open_reader(&binding(2))
                .unwrap()
                .statistics()
                .records,
            0
        );
        writer
            .append_completed(&binding(1), &[record(7)], &context())
            .unwrap();
        drop(writer);
        let mut resumed =
            SavedApplicationCacheBundleWriter::resume(&output, &[binding(1), binding(2)], limits)
                .unwrap();
        resumed
            .append_completed(&binding(1), &[record(8)], &context())
            .unwrap();
        drop(resumed);
        reader.refresh().unwrap();
        for n in [7, 8] {
            assert_eq!(reader.lookup(&key(n), &context()).unwrap(), Some(record(n)));
        }
    }
    assert_eq!(first.statistics(), first_stats);
    assert_eq!(second.statistics(), second_stats);
    assert_eq!(
        fs::read(first_path.join(index::MANIFEST)).unwrap(),
        first_index
    );
    assert_eq!(
        fs::read(second_path.join(index::MANIFEST)).unwrap(),
        second_index
    );
    fs::remove_dir_all(&first_path).unwrap();
    fs::remove_dir_all(&second_path).unwrap();
    for name in ["first-wins", "second-wins"] {
        let bundle = SavedApplicationCacheBundle::open_generated(
            &scratch.0.join(name),
            &[binding(1), binding(2)],
            limits,
        )
        .unwrap();
        let mut reader = bundle.open_reader(&binding(1)).unwrap();
        assert!(reader.lookup(&key(1), &context()).unwrap().is_some());
        assert_eq!(reader.lookup(&key(5), &context()).unwrap(), Some(record(5)));
    }
}

#[test]
fn opaque_union_rejects_conflicting_descriptors_across_legacy_filter_omissions() {
    let scratch = scratch();
    let source = scratch.0.join("source");
    let limits = SavedApplicationCachePackLimits::default();
    let original = pack(&source, 1, &[vec![record(0), record(5), record(10)]]);
    let mut altered = original.captured_snapshot().1.clone();
    altered.runs[0][0].key_fingerprints = None;
    fs::write(
        source.join(index::MANIFEST),
        index::encode(&altered, limits).unwrap(),
    )
    .unwrap();
    let omitted = SavedApplicationCachePack::open_generated(&source, &binding(1), limits).unwrap();
    altered.runs[0][0].key_fingerprints =
        Some(index::key_filter::collect([key(0), key(6), key(10)].iter()).unwrap());
    fs::write(
        source.join(index::MANIFEST),
        index::encode(&altered, limits).unwrap(),
    )
    .unwrap();
    let conflicting =
        SavedApplicationCachePack::open_generated(&source, &binding(1), limits).unwrap();
    for (position, bases) in [
        [&original, &omitted, &conflicting],
        [&conflicting, &omitted, &original],
    ]
    .iter()
    .enumerate()
    {
        let output = scratch.0.join(format!("conflict-{position}"));
        assert!(union(&output, &[binding(1)], bases, limits).is_err());
        assert!(!output.exists());
    }
    let output = scratch.0.join("compatible");
    let (writer, statistics) =
        union(&output, &[binding(1)], &[&original, &omitted], limits).unwrap();
    assert_eq!(statistics.copied_chunks, 1);
    assert_eq!(statistics.preserved_record_references, 6);
    drop(writer);
    let bundle =
        SavedApplicationCacheBundle::open_generated(&output, &[binding(1)], limits).unwrap();
    assert_eq!(
        bundle
            .open_reader(&binding(1))
            .unwrap()
            .lookup(&key(5), &context())
            .unwrap(),
        Some(record(5))
    );
    altered.runs[0][0].key_fingerprints = original.captured_snapshot().1.runs[0][0]
        .key_fingerprints
        .clone();
    altered.runs[0][0].decoded_digest[0] ^= 1;
    fs::write(
        source.join(index::MANIFEST),
        index::encode(&altered, limits).unwrap(),
    )
    .unwrap();
    let before = fs::read(source.join(index::MANIFEST)).unwrap();
    let wrong_payload =
        SavedApplicationCachePack::open_generated(&source, &binding(1), limits).unwrap();
    let output = scratch.0.join("payload-conflict");
    assert!(union(&output, &[binding(1)], &[&wrong_payload, &original], limits).is_err());
    assert!(!output.exists());
    assert_eq!(fs::read(source.join(index::MANIFEST)).unwrap(), before);
    assert_eq!(original.statistics().chunks_read, 0);
    assert_eq!(omitted.statistics().chunks_read, 0);
    assert_eq!(conflicting.statistics().chunks_read, 0);
}

#[test]
fn opaque_union_exact_bindings_and_combined_metadata_and_disk_grants_precede_output() {
    use std::time::Duration;
    use symbolica::utils::resources::{ResourceBudget, ResourceLimitError, ResourceLimits};

    let scratch = scratch();
    let limits = SavedApplicationCachePackLimits::default();
    let first = pack(&scratch.0.join("first"), 1, &[vec![record(1)]]);
    let second = pack(&scratch.0.join("second"), 1, &[vec![record(2)]]);
    let foreign = pack(&scratch.0.join("foreign"), 2, &[vec![record(3)]]);
    let expired_output = scratch.0.join("expired-budget");
    let expired = ResourceBudget::new(ResourceLimits {
        max_process_rss_bytes: None,
        max_elapsed: Some(Duration::ZERO),
    });
    let error = expired
        .run(|| union(&expired_output, &[binding(1)], &[&first, &second], limits))
        .err()
        .unwrap();
    assert!(matches!(
        error.resource_limit_error(),
        Some(ResourceLimitError::Elapsed { .. })
    ));
    assert!(!expired_output.exists());
    for (name, bindings, bases) in [
        ("unknown", vec![binding(1)], vec![&foreign]),
        (
            "changed-layer",
            vec![ApplicationCacheBinding {
                layer: "different".into(),
                ..binding(1)
            }],
            vec![&first],
        ),
        (
            "changed-artifact",
            vec![ApplicationCacheBinding {
                artifact_digest: [99; 32],
                ..binding(1)
            }],
            vec![&first],
        ),
        (
            "duplicate-binding",
            vec![binding(1), binding(1)],
            vec![&first],
        ),
    ] {
        let output = scratch.0.join(name);
        assert!(union(&output, &bindings, &bases, limits).is_err());
        assert!(!output.exists());
    }
    let metadata = [&first, &second]
        .iter()
        .map(|base| {
            index::validate(base.captured_snapshot().1, &binding(1), limits)
                .unwrap()
                .directory_owned_bytes
        })
        .max()
        .unwrap();
    let tight = SavedApplicationCachePackLimits {
        max_directory_owned_bytes: metadata,
        ..limits
    };
    for base in [&first, &second] {
        index::validate(base.captured_snapshot().1, &binding(1), tight).unwrap();
    }
    let output = scratch.0.join("combined-metadata");
    assert!(union(&output, &[binding(1)], &[&first, &second], tight).is_err());
    assert!(!output.exists());
    let output = scratch.0.join("complete");
    let (writer, statistics) = union(&output, &[binding(1)], &[&first, &second], limits).unwrap();
    let used = writer.charged_disk_bytes().unwrap();
    assert_eq!(used, statistics.compressed_bytes + statistics.index_bytes);
    drop(writer);
    let output = scratch.0.join("combined-disk");
    assert!(
        union(
            &output,
            &[binding(1)],
            &[&first, &second],
            SavedApplicationCachePackLimits {
                max_disk_bytes: used - 1,
                ..limits
            }
        )
        .is_err()
    );
    assert!(!output.exists());
    let output = scratch.0.join("legacy-repeated");
    assert!(
        SavedApplicationCacheBundleWriter::copy_bases_to_new(
            &output,
            &[binding(1)],
            &[&first, &second],
            limits
        )
        .is_err()
    );
    assert!(
        !output.exists(),
        "legacy copy still rejects repeated namespace bases"
    );
}

#[test]
fn opaque_union_corruption_leaves_owned_partial_output_and_unchanged_sources() {
    let scratch = scratch();
    let limits = SavedApplicationCachePackLimits::default();
    let first_path = scratch.0.join("first");
    let second_path = scratch.0.join("second");
    let first = pack(&first_path, 1, &[vec![record(1)]]);
    let second = pack(&second_path, 1, &[vec![record(2)]]);
    let first_index = fs::read(first_path.join(index::MANIFEST)).unwrap();
    let second_index = fs::read(second_path.join(index::MANIFEST)).unwrap();
    let first_chunk = first.captured_snapshot().1.runs[0][0].clone();
    let second_chunk = second.captured_snapshot().1.runs[0][0].clone();
    let (bad_path, good_path) = if first_chunk.digest > second_chunk.digest {
        (
            first_path.join("chunks").join(first_chunk.file_name()),
            second_path.join("chunks").join(second_chunk.file_name()),
        )
    } else {
        (
            second_path.join("chunks").join(second_chunk.file_name()),
            first_path.join("chunks").join(first_chunk.file_name()),
        )
    };
    let good_bytes = fs::read(&good_path).unwrap();
    let mut bad_bytes = fs::read(&bad_path).unwrap();
    bad_bytes[20] ^= 1;
    fs::write(&bad_path, &bad_bytes).unwrap();
    let output = scratch.0.join("partial");
    assert!(union(&output, &[binding(1)], &[&first, &second], limits).is_err());
    assert!(output.is_dir());
    assert!(!output.join("bundle.rrindex").exists());
    assert!(SavedApplicationCacheBundle::open_generated(&output, &[binding(1)], limits).is_err());
    let children = fs::read_dir(output.join("bindings"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(children.len(), 1);
    assert_eq!(
        physical_chunks(&children[0]).0,
        1,
        "valid physical prefix is retained"
    );
    assert_eq!(fs::read(&good_path).unwrap(), good_bytes);
    assert_eq!(fs::read(&bad_path).unwrap(), bad_bytes);
    assert_eq!(
        fs::read(first_path.join(index::MANIFEST)).unwrap(),
        first_index
    );
    assert_eq!(
        fs::read(second_path.join(index::MANIFEST)).unwrap(),
        second_index
    );
    assert!(
        union(&output, &[binding(1)], &[&first, &second], limits).is_err(),
        "partial output is not silently replaced"
    );
}
