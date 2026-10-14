use super::*;

fn filtered_fixture(path: &Path) -> SavedApplicationCacheWriter {
    let mut writer =
        SavedApplicationCacheWriter::create(path, binding(1), Default::default()).unwrap();
    writer
        .append_completed(&[record(0), record(5), record(10)], &context())
        .unwrap();
    writer.checkpoint().unwrap();
    writer
}

fn write_unchecked(path: &Path, manifest: &index::Manifest) {
    let mut bytes = b"RR-CACHE-PACK-03".to_vec();
    bincode::encode_into_std_write(manifest, &mut bytes, bincode::config::standard()).unwrap();
    let digest = blake3::hash(&bytes);
    bytes.extend_from_slice(digest.as_bytes());
    fs::write(path.join(index::MANIFEST), bytes).unwrap();
}

#[test]
fn fingerprint_covers_the_full_structural_key_and_keeps_collisions_counted() {
    let integral = IntegralKey::try_new([7]).unwrap();
    let keys = [
        ApplicationCacheKey::guarded_root(&integral),
        ApplicationCacheKey::route(&integral),
        ApplicationCacheKey::apply(vec![true], &integral),
        key(-7),
    ];
    let fingerprints = index::key_filter::collect(keys.iter()).unwrap();
    assert_eq!(fingerprints.len(), keys.len());
    assert!(fingerprints.windows(2).all(|pair| pair[0] < pair[1]));
    for key in &keys {
        let mut framing = b"RustRed cache key fingerprint v1\0".to_vec();
        framing.extend(bincode::encode_to_vec(key, bincode::config::standard()).unwrap());
        let digest = blake3::hash(&framing);
        let expected = u64::from_le_bytes(digest.as_bytes()[..8].try_into().unwrap());
        assert_eq!(index::key_filter::fingerprint(key).unwrap(), expected);
    }
    let repeated = index::key_filter::collect([&keys[0], &keys[0]].into_iter()).unwrap();
    assert_eq!(repeated.len(), 2);
    assert_eq!(repeated[0], repeated[1]);
    assert!(index::key_filter::may_contain(&repeated, repeated[0]));
}

#[test]
fn persisted_filters_skip_more_than_eight_overlaps_without_spending_a_read_grant() {
    let scratch = scratch();
    let path = scratch.0.join("durable-filter-overlap");
    let mut writer = filtered_fixture(&path);
    for end in 20..40 {
        writer
            .append_completed(&[record(0), record(end)], &context())
            .unwrap();
    }
    writer.checkpoint().unwrap();
    drop(writer);
    // This is the failed three-loop preparation pattern: a fresh reader must
    // skip broad range-only candidates, not first warm an exact-key side index.
    let limits = SavedApplicationCachePackLimits {
        max_lookup_chunks: 1,
        ..Default::default()
    };
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(1), limits).unwrap();
    assert_eq!(reader.lookup(&key(5), &context()).unwrap(), Some(record(5)));
    let stats = reader.statistics();
    assert_eq!(stats.chunks_read, 1);
    assert_eq!(stats.key_index_skips, 20);
    assert_eq!(stats.lookup_limit_refusals, 0);
    let no_reads = SavedApplicationCachePackLimits {
        max_lookup_chunks: 0,
        max_lookup_encoded_bytes: 0,
        max_lookup_decoded_bytes: 0,
        ..limits
    };
    let mut absent =
        SavedApplicationCachePack::open_generated(&path, &binding(1), no_reads).unwrap();
    assert_eq!(absent.lookup(&key(6), &context()).unwrap(), None);
    assert_eq!(absent.statistics().key_index_skips, 21);
    assert_eq!(absent.statistics().chunks_read, 0);
    assert_eq!(absent.statistics().lookup_encoded_bytes_charged, 0);
    assert_eq!(absent.statistics().lookup_decoded_bytes_charged, 0);
    assert_eq!(absent.statistics().lookup_limit_refusals, 0);
    assert_eq!(
        absent.lookup(&key(5), &context()).unwrap_err().kind(),
        crate::AppErrorKind::Limit
    );
}

#[test]
fn forced_fingerprint_collision_is_only_a_candidate_not_a_wrong_hit() {
    let scratch = scratch();
    let path = scratch.0.join("filter-collision");
    drop(filtered_fixture(&path));
    let mut reader =
        SavedApplicationCachePack::open_generated(&path, &binding(1), Default::default()).unwrap();
    // Simulate hash(key(6)) == hash(key(5)) while leaving the authenticated
    // manifest and actual decoded key census intact. Native equality wins.
    let collision = index::key_filter::fingerprint(&key(5)).unwrap();
    assert_eq!(
        reader
            .lookup_outcome_with_test_fingerprint(&key(6), &context(), collision)
            .unwrap(),
        SavedApplicationCacheLookupOutcome::Absent
    );
    assert_eq!(reader.statistics().chunks_read, 1);
    assert_eq!(reader.statistics().hits, 0);
    assert_eq!(reader.statistics().misses, 1);
    assert_eq!(reader.lookup(&key(5), &context()).unwrap(), Some(record(5)));
    assert_eq!(reader.statistics().chunks_read, 2);
}

#[test]
fn newest_record_wins_and_refreshed_forked_indexes_keep_the_durable_filter() {
    let scratch = scratch();
    let path = scratch.0.join("filter-refresh");
    let mut writer = filtered_fixture(&path);
    let mut reader =
        SavedApplicationCachePack::open_generated(&path, &binding(1), Default::default()).unwrap();
    let mut newer = record(5);
    newer.terms.clear();
    writer
        .append_completed(&[record(0), newer.clone(), record(20)], &context())
        .unwrap();
    writer.checkpoint().unwrap();
    assert_eq!(reader.lookup(&key(5), &context()).unwrap(), Some(record(5)));
    reader.refresh().unwrap();
    assert_eq!(
        reader.lookup(&key(5), &context()).unwrap(),
        Some(newer.clone())
    );
    let mut fork = reader.fork_reader();
    assert_eq!(fork.lookup(&key(6), &context()).unwrap(), None);
    assert_eq!(fork.statistics().chunks_read, 0);
    assert_eq!(fork.statistics().key_index_skips, 2);
    assert_eq!(fork.lookup(&key(5), &context()).unwrap(), Some(newer));
    assert_eq!(fork.statistics().chunks_read, 1);
}

#[test]
fn fingerprint_memory_is_charged_and_no_full_key_duplicate_is_retained() {
    let scratch = scratch();
    let path = scratch.0.join("filter-memory");
    drop(filtered_fixture(&path));
    let filtered = manifest(&path);
    let mut plain = filtered.clone();
    plain.runs[0][0].key_fingerprints = None;
    let limits = SavedApplicationCachePackLimits::default();
    let total = index::validate(&filtered, &binding(1), limits)
        .unwrap()
        .directory_owned_bytes;
    let base = index::validate(&plain, &binding(1), limits)
        .unwrap()
        .directory_owned_bytes;
    assert_eq!(
        total - base,
        filtered.runs[0][0]
            .key_fingerprints
            .as_ref()
            .unwrap()
            .capacity()
            * std::mem::size_of::<u64>()
    );
    let tight = SavedApplicationCachePackLimits {
        max_directory_owned_bytes: total - 1,
        ..limits
    };
    assert!(SavedApplicationCachePack::open_generated(&path, &binding(1), tight).is_err());
    let exact = SavedApplicationCachePackLimits {
        max_directory_owned_bytes: total,
        ..limits
    };
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(1), exact).unwrap();
    assert_eq!(reader.lookup(&key(5), &context()).unwrap(), Some(record(5)));
    assert_eq!(reader.statistics().directory_owned_bytes, total);
    assert_eq!(reader.lookup(&key(6), &context()).unwrap(), None);
    assert_eq!(reader.statistics().directory_owned_bytes, total);
}

#[test]
fn malformed_count_sort_endpoint_checksum_and_truncation_are_rejected() {
    let scratch = scratch();
    let path = scratch.0.join("filter-malformed");
    drop(filtered_fixture(&path));
    let original = manifest(&path);
    for mutation in 0..4 {
        let mut malformed = original.clone();
        let chunk = &mut malformed.runs[0][0];
        let keys = chunk.key_fingerprints.as_mut().unwrap();
        match mutation {
            0 => {
                keys.pop();
            }
            1 => keys.reverse(),
            2 => {
                keys.fill(0);
            }
            3 => chunk.records += 1,
            _ => unreachable!(),
        }
        write_unchecked(&path, &malformed);
        assert!(
            SavedApplicationCachePack::open_generated(&path, &binding(1), Default::default())
                .is_err()
        );
    }
    let bytes = index::encode(&original, Default::default()).unwrap();
    for mutation in 0..2 {
        let mut damaged = bytes.clone();
        if mutation == 0 {
            damaged[20] ^= 1;
        } else {
            damaged.pop();
        }
        fs::write(path.join(index::MANIFEST), damaged).unwrap();
        assert!(
            SavedApplicationCachePack::open_generated(&path, &binding(1), Default::default())
                .is_err()
        );
    }
}

#[test]
fn coherent_wrong_interior_filter_is_rejected_when_native_chunk_is_read() {
    let scratch = scratch();
    let path = scratch.0.join("filter-native-census");
    drop(filtered_fixture(&path));
    let mut altered = manifest(&path);
    altered.runs[0][0].key_fingerprints =
        Some(index::key_filter::collect([key(0), key(6), key(10)].iter()).unwrap());
    // Count, ordering, endpoints and checksum agree. A real read still checks
    // every fingerprint against the authenticated native keys, before any hit.
    fs::write(
        path.join(index::MANIFEST),
        index::encode(&altered, Default::default()).unwrap(),
    )
    .unwrap();
    let mut reader =
        SavedApplicationCachePack::open_generated(&path, &binding(1), Default::default()).unwrap();
    assert!(reader.lookup(&key(0), &context()).is_err());
    assert_eq!(reader.statistics().hits, 0);
    assert_eq!(reader.statistics().misses, 0);
}

#[test]
fn legacy02_read_and_resume_keep_payloads_and_publish_mixed03_metadata() {
    let scratch = scratch();
    let path = scratch.0.join("filter-legacy");
    drop(filtered_fixture(&path));
    let before = physical_chunks(&path);
    make_legacy(&path);
    assert!(
        fs::read(path.join(index::MANIFEST))
            .unwrap()
            .starts_with(b"RR-CACHE-PACK-02")
    );
    let mut old =
        SavedApplicationCachePack::open_generated(&path, &binding(1), Default::default()).unwrap();
    assert_eq!(old.lookup(&key(5), &context()).unwrap(), Some(record(5)));
    let mut writer =
        SavedApplicationCacheWriter::resume(&path, &binding(1), Default::default()).unwrap();
    writer.append_completed(&[record(20)], &context()).unwrap();
    writer.checkpoint().unwrap();
    drop(writer);
    assert!(
        fs::read(path.join(index::MANIFEST))
            .unwrap()
            .starts_with(b"RR-CACHE-PACK-03")
    );
    let mixed = manifest(&path);
    assert!(mixed.runs[0][0].key_fingerprints.is_none());
    assert!(mixed.runs[0][1].key_fingerprints.is_some());
    assert_eq!(physical_chunks(&path).0, before.0 + 1);
    old.refresh().unwrap();
    assert_eq!(old.lookup(&key(5), &context()).unwrap(), Some(record(5)));
    assert_eq!(old.lookup(&key(20), &context()).unwrap(), Some(record(20)));
}

#[test]
fn portable_bundle_snapshot_preserves_filter_and_its_zero_read_misses() {
    let scratch = scratch();
    let source = scratch.0.join("filter-copy-source");
    drop(filtered_fixture(&source));
    let base = SavedApplicationCachePack::open_generated(&source, &binding(1), Default::default())
        .unwrap();
    let destination = scratch.0.join("filter-copy-destination");
    let (writer, _) = SavedApplicationCacheBundleWriter::copy_bases_to_new(
        &destination,
        &[binding(1)],
        &[&base],
        Default::default(),
    )
    .unwrap();
    drop(writer);
    let bundle = SavedApplicationCacheBundle::open_generated(
        &destination,
        &[binding(1)],
        Default::default(),
    )
    .unwrap();
    let mut reader = bundle.open_reader(&binding(1)).unwrap();
    let child = child_path(&destination, &binding(1));
    assert_eq!(manifest(&child).runs, manifest(&source).runs);
    assert_eq!(reader.lookup(&key(6), &context()).unwrap(), None);
    assert_eq!(reader.statistics().chunks_read, 0);
    assert_eq!(reader.lookup(&key(5), &context()).unwrap(), Some(record(5)));
}

#[test]
fn mixed_legacy_and_filtered_descriptors_share_one_identical_payload_on_copy() {
    let scratch = scratch();
    let source = scratch.0.join("mixed-identical-source");
    drop(filtered_fixture(&source));
    make_legacy(&source);
    let before = physical_chunks(&source);
    let mut writer =
        SavedApplicationCacheWriter::resume(&source, &binding(1), Default::default()).unwrap();
    writer
        .append_completed(&[record(0), record(5), record(10)], &context())
        .unwrap();
    writer.checkpoint().unwrap();
    drop(writer);
    assert_eq!(physical_chunks(&source), before);
    let base = SavedApplicationCachePack::open_generated(&source, &binding(1), Default::default())
        .unwrap();
    let destination = scratch.0.join("mixed-identical-destination");
    let (writer, stats) = SavedApplicationCacheBundleWriter::copy_bases_to_new(
        &destination,
        &[binding(1)],
        &[&base],
        Default::default(),
    )
    .unwrap();
    drop(writer);
    assert_eq!(stats.copied_chunks, 1);
    assert_eq!(stats.preserved_record_references, 6);
    let bundle = SavedApplicationCacheBundle::open_generated(
        &destination,
        &[binding(1)],
        Default::default(),
    )
    .unwrap();
    let mut reader = bundle.open_reader(&binding(1)).unwrap();
    assert_eq!(reader.lookup(&key(5), &context()).unwrap(), Some(record(5)));
    assert_eq!(reader.statistics().chunks_read, 1);

    let mut contradictory = manifest(&source);
    let mut other = contradictory.runs[1][0].clone();
    other.key_fingerprints =
        Some(index::key_filter::collect([key(0), key(6), key(10)].iter()).unwrap());
    // A legacy omission must not hide disagreeing Some filters for one digest.
    contradictory.runs.insert(0, vec![other]);
    fs::write(
        source.join(index::MANIFEST),
        index::encode(&contradictory, Default::default()).unwrap(),
    )
    .unwrap();
    let conflicting =
        SavedApplicationCachePack::open_generated(&source, &binding(1), Default::default())
            .unwrap();
    let refused = scratch.0.join("mixed-contradictory-copy");
    assert!(
        SavedApplicationCacheBundleWriter::copy_bases_to_new(
            &refused,
            &[binding(1)],
            &[&conflicting],
            Default::default()
        )
        .is_err()
    );
    assert!(!refused.exists());
}
