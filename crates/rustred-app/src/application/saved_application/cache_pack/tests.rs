use super::*;
use rustred::algebra::CoefficientContext;
use rustred::family::IntegralKey;
use rustred::persistence::application_cache::{
    ApplicationCacheBinding, ApplicationCacheKey, ApplicationCacheRecord,
    encode_application_cache_chunk,
};
use std::{collections::BTreeMap, fs, path::Path};

mod lookup_outcome;
mod snapshot;

fn binding(id: u8) -> ApplicationCacheBinding {
    ApplicationCacheBinding {
        artifact_digest: [id; 32],
        family_fingerprint: format!("cache-pack-test-{id}"),
        physical_arity: 1,
        layer: "raw-declared-unit-mass-v1".into(),
    }
}
fn context() -> CoefficientContext {
    CoefficientContext::try_new(["cache_pack_test_d"]).unwrap()
}
fn key(n: i64) -> ApplicationCacheKey {
    ApplicationCacheKey::route(&IntegralKey::try_new([n]).unwrap())
}
fn record(n: i64) -> ApplicationCacheRecord {
    let context = context();
    let guard = context
        .try_sub(
            &context.parameter("cache_pack_test_d").unwrap(),
            &context.integer(4),
            Default::default(),
        )
        .unwrap();
    ApplicationCacheRecord {
        key: key(n),
        terms: if n == 0 {
            BTreeMap::new()
        } else {
            BTreeMap::from([(
                IntegralKey::try_new([1]).unwrap(),
                context
                    .try_div(&context.one(), &guard, Default::default())
                    .unwrap(),
            )])
        },
        nonzero_conditions: vec![guard.clone(), context.integer(-2), guard],
    }
}
fn scratch() -> super::super::tests::Scratch {
    super::super::tests::Scratch::new()
}
fn manifest(path: &Path) -> index::Manifest {
    let limits = SavedApplicationCachePackLimits::default();
    let bytes =
        index::read_bounded(&path.join(index::MANIFEST), limits.max_manifest_bytes).unwrap();
    index::decode(&bytes, &binding(1), limits).unwrap().0
}
fn child_path(bundle: &Path, expected: &ApplicationCacheBinding) -> std::path::PathBuf {
    fs::read_dir(bundle.join("bindings"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            SavedApplicationCachePack::open_generated(path, expected, Default::default()).is_ok()
        })
        .unwrap()
}
fn physical_chunks(path: &Path) -> (usize, u64) {
    fs::read_dir(path.join("chunks"))
        .unwrap()
        .map(|entry| entry.unwrap().metadata().unwrap().len())
        .fold((0, 0), |(count, bytes), size| (count + 1, bytes + size))
}

#[test]
fn lazy_pack_reads_one_exact_chunk_and_no_payload_at_open() {
    let scratch = scratch();
    let path = scratch.0.join("pack");
    let limits = SavedApplicationCachePackLimits::default();
    let mut writer = SavedApplicationCacheWriter::create(&path, binding(1), limits).unwrap();
    for n in 0..8 {
        writer.append_completed(&[record(n)], &context()).unwrap();
    }
    writer.checkpoint().unwrap();
    let maximum = manifest(&path)
        .runs
        .iter()
        .flatten()
        .map(|chunk| chunk.bytes as usize)
        .max()
        .unwrap();
    let total = writer.physical_chunk_bytes();
    assert!(total > maximum as u64);
    drop(writer);
    let mut tight = limits;
    tight.max_encoded_chunk_bytes = maximum;
    tight.chunk.binary.max_program_bytes = manifest(&path)
        .runs
        .iter()
        .flatten()
        .map(|chunk| chunk.decoded_bytes as usize)
        .max()
        .unwrap();
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(1), tight).unwrap();
    assert_eq!(reader.statistics().chunks_read, 0);
    assert_eq!(reader.statistics().runs, 1);
    assert_eq!(reader.lookup(&key(6), &context()).unwrap(), Some(record(6)));
    assert_eq!(reader.statistics().chunks_read, 1);
    assert!(reader.statistics().chunk_bytes_read <= maximum as u64);
    assert_eq!(reader.lookup(&key(100), &context()).unwrap(), None);
    assert_eq!(reader.statistics().chunks_read, 1);
    assert_eq!(reader.fork_reader().statistics().chunks_read, 0);
    assert!(reader.statistics().directory_owned_bytes <= limits.max_directory_owned_bytes);
}

#[test]
fn interrupted_tail_resume_lock_and_disk_limits_preserve_committed_records() {
    let scratch = scratch();
    let path = scratch.0.join("pack");
    let limits = SavedApplicationCachePackLimits::default();
    let mut writer = SavedApplicationCacheWriter::create(&path, binding(1), limits).unwrap();
    writer.append_completed(&[record(1)], &context()).unwrap();
    writer.checkpoint().unwrap();
    assert!(SavedApplicationCacheWriter::resume(&path, &binding(1), limits).is_err());
    writer.append_completed(&[record(2)], &context()).unwrap();
    let disk_with_tail = writer.physical_chunk_bytes();
    drop(writer); // Simulated interruption before publishing the last run.
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(1), limits).unwrap();
    assert_eq!(reader.lookup(&key(1), &context()).unwrap(), Some(record(1)));
    assert_eq!(reader.lookup(&key(2), &context()).unwrap(), None);
    let mut resumed = SavedApplicationCacheWriter::resume(&path, &binding(1), limits).unwrap();
    assert_eq!(resumed.physical_chunk_bytes(), disk_with_tail);
    resumed.append_completed(&[record(2)], &context()).unwrap();
    resumed.checkpoint().unwrap();
    drop(resumed);
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(1), limits).unwrap();
    assert_eq!(reader.lookup(&key(2), &context()).unwrap(), Some(record(2)));
    let strict = SavedApplicationCachePackLimits {
        max_disk_bytes: disk_with_tail - 1,
        ..limits
    };
    assert!(SavedApplicationCacheWriter::resume(&path, &binding(1), strict).is_err());
    assert_eq!(reader.lookup(&key(1), &context()).unwrap(), Some(record(1)));
}

#[test]
fn binding_corruption_and_tighter_decode_budget_fail_closed() {
    let scratch = scratch();
    let path = scratch.0.join("pack");
    let limits = SavedApplicationCachePackLimits::default();
    let mut writer = SavedApplicationCacheWriter::create(&path, binding(1), limits).unwrap();
    writer
        .append_completed(&[record(0), record(1)], &context())
        .unwrap();
    writer.checkpoint().unwrap();
    let chunk = manifest(&path).runs[0][0].clone();
    drop(writer);
    assert!(SavedApplicationCachePack::open_generated(&path, &binding(2), limits).is_err());
    let mut strict = limits;
    strict.chunk.max_decoded_coefficient_bytes = 0;
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(1), strict).unwrap();
    assert!(
        reader.lookup(&key(0), &context()).is_err(),
        "even zero outputs retain their guard charge"
    );
    let chunk_path = path.join("chunks").join(chunk.file_name());
    let mut bytes = fs::read(&chunk_path).unwrap();
    let original = bytes.clone();
    bytes[20] ^= 1;
    fs::write(&chunk_path, bytes).unwrap();
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(1), limits).unwrap();
    assert!(reader.lookup(&key(1), &context()).is_err());
    fs::write(&chunk_path, original).unwrap();
    assert_eq!(reader.lookup(&key(1), &context()).unwrap(), Some(record(1)));
    let mut manifest = fs::read(path.join(index::MANIFEST)).unwrap();
    manifest.truncate(manifest.len() - 1);
    fs::write(path.join(index::MANIFEST), manifest).unwrap();
    assert!(SavedApplicationCachePack::open_generated(&path, &binding(1), limits).is_err());
}

#[test]
fn bundle_program_identity_and_aggregate_quota_precede_child_writes() {
    let scratch = scratch();
    let path = scratch.0.join("bundle");
    let bindings = [binding(1), binding(2)];
    let limits = SavedApplicationCachePackLimits::default();
    let mut writer = SavedApplicationCacheBundleWriter::create(&path, &bindings, limits).unwrap();
    writer
        .append_completed(&bindings[0], &[record(1)], &context())
        .unwrap();
    let used = writer.charged_disk_bytes().unwrap();
    drop(writer);
    let encoded =
        encode_application_cache_chunk(&bindings[1], &[record(2)], &context(), limits.chunk)
            .unwrap();
    let encoded = compression::encode(&encoded, limits).unwrap();
    let tighter = SavedApplicationCachePackLimits {
        max_disk_bytes: used + encoded.len() as u64 / 2,
        ..limits
    };
    let mut writer = SavedApplicationCacheBundleWriter::resume(&path, &bindings, tighter).unwrap();
    let second_path = child_path(&path, &bindings[1]);
    let before = physical_chunks(&second_path);
    let error = writer
        .append_completed(&bindings[1], &[record(2)], &context())
        .unwrap_err();
    assert_eq!(error.kind(), crate::AppErrorKind::Limit);
    assert_eq!(
        physical_chunks(&second_path),
        before,
        "quota refusal must precede even an unpublished physical chunk"
    );
    assert!(writer.charged_disk_bytes().unwrap() <= tighter.max_disk_bytes);
    drop(writer);
    let bundle = SavedApplicationCacheBundle::open_generated(&path, &bindings, limits).unwrap();
    let mut first = bundle.open_reader(&bindings[0]).unwrap();
    let mut second = bundle.open_reader(&bindings[1]).unwrap();
    assert_eq!(first.lookup(&key(1), &context()).unwrap(), Some(record(1)));
    assert_eq!(second.lookup(&key(2), &context()).unwrap(), None);
    assert_eq!(
        second.statistics().chunks,
        0,
        "quota failure must precede the second child's write"
    );
    assert!(
        SavedApplicationCacheBundle::open_generated(&path, &[bindings[0].clone()], limits).is_err()
    );
    assert!(bundle.open_reader(&binding(3)).is_err());
    let child = fs::read_dir(path.join("bindings"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let descriptor =
        index::read_bounded(&child.join(index::MANIFEST), limits.max_manifest_bytes).unwrap();
    let expected = bindings
        .iter()
        .find(|expected| index::decode(&descriptor, expected, limits).is_ok())
        .unwrap();
    assert!(
        SavedApplicationCacheWriter::resume(&child, expected, limits).is_err(),
        "a child cannot bypass the bundle quota through a standalone writer"
    );
}

#[test]
fn resumed_quota_counts_owned_store_and_bundle_index_staging_without_deletion() {
    let scratch = scratch();
    let limits = SavedApplicationCachePackLimits::default();
    let path = scratch.0.join("pack");
    let mut writer = SavedApplicationCacheWriter::create(&path, binding(1), limits).unwrap();
    writer.append_completed(&[record(1)], &context()).unwrap();
    writer.checkpoint().unwrap();
    let used = writer.charged_disk_bytes().unwrap();
    drop(writer);
    let staging = path.join(".cache.rrindex.rustred-tmp-interrupted-0");
    fs::write(&staging, [0u8; 4096]).unwrap();
    let foreign = path.join("caller-owned-note");
    fs::write(&foreign, b"do not delete").unwrap();
    let tight = SavedApplicationCachePackLimits {
        max_disk_bytes: used + 2048,
        ..limits
    };
    assert!(SavedApplicationCacheWriter::resume(&path, &binding(1), tight).is_err());
    let resumed = SavedApplicationCacheWriter::resume(&path, &binding(1), limits).unwrap();
    assert_eq!(resumed.charged_disk_bytes().unwrap(), used + 4096);
    drop(resumed);
    assert!(staging.exists());
    assert_eq!(fs::read(&foreign).unwrap(), b"do not delete");
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(1), limits).unwrap();
    assert_eq!(reader.lookup(&key(1), &context()).unwrap(), Some(record(1)));

    let path = scratch.0.join("bundle");
    let bindings = [binding(1), binding(2)];
    let writer = SavedApplicationCacheBundleWriter::create(&path, &bindings, limits).unwrap();
    let used = writer.charged_disk_bytes().unwrap();
    drop(writer);
    let child = child_path(&path, &bindings[0]);
    fs::write(
        child.join(".cache.rrindex.rustred-tmp-interrupted-0"),
        [0u8; 1024],
    )
    .unwrap();
    fs::write(
        path.join(".bundle.rrindex.rustred-tmp-interrupted-0"),
        [0u8; 2048],
    )
    .unwrap();
    fs::write(
        path.join(".preparation.rrcheckpoint.rustred-tmp-interrupted-0"),
        [0u8; 512],
    )
    .unwrap();
    let tight = SavedApplicationCachePackLimits {
        max_disk_bytes: used + 3072,
        ..limits
    };
    assert!(SavedApplicationCacheBundleWriter::resume(&path, &bindings, tight).is_err());
    let resumed = SavedApplicationCacheBundleWriter::resume(&path, &bindings, limits).unwrap();
    assert_eq!(resumed.charged_disk_bytes().unwrap(), used + 3584);
}

#[test]
fn uncertain_post_install_write_requires_rescan_before_any_further_append() {
    let scratch = scratch();
    let path = scratch.0.join("pack");
    let limits = SavedApplicationCachePackLimits {
        max_chunks: 2,
        ..Default::default()
    };
    let mut writer = SavedApplicationCacheWriter::create(&path, binding(1), limits).unwrap();
    writer.append_completed(&[record(1)], &context()).unwrap();
    writer.checkpoint().unwrap();
    let committed = fs::read(path.join(index::MANIFEST)).unwrap();
    writer.inject_post_install_failure();
    assert!(writer.append_completed(&[record(2)], &context()).is_err());
    assert!(writer.is_poisoned());
    assert_eq!(physical_chunks(&path).0, 2);
    assert!(writer.append_completed(&[record(3)], &context()).is_err());
    assert!(writer.checkpoint().is_err());
    assert_eq!(fs::read(path.join(index::MANIFEST)).unwrap(), committed);
    drop(writer);
    let mut resumed = SavedApplicationCacheWriter::resume(&path, &binding(1), limits).unwrap();
    assert!(!resumed.is_poisoned());
    assert!(resumed.append_completed(&[record(3)], &context()).is_err());
    assert_eq!(
        physical_chunks(&path).0,
        2,
        "installed orphan must count after rescan"
    );
    drop(resumed);
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(1), limits).unwrap();
    assert_eq!(reader.lookup(&key(1), &context()).unwrap(), Some(record(1)));
    assert_eq!(reader.lookup(&key(2), &context()).unwrap(), None);

    let path = scratch.0.join("bundle");
    let bindings = [binding(1), binding(2)];
    let mut writer =
        SavedApplicationCacheBundleWriter::create(&path, &bindings, Default::default()).unwrap();
    writer.inject_child_post_install_failure();
    assert!(
        writer
            .append_completed(&bindings[0], &[record(1)], &context())
            .is_err()
    );
    assert!(writer.is_poisoned());
    assert!(
        writer
            .append_completed(&bindings[1], &[record(2)], &context())
            .is_err()
    );
    assert_eq!(physical_chunks(&child_path(&path, &bindings[1])).0, 0);
    drop(writer);
    let resumed =
        SavedApplicationCacheBundleWriter::resume(&path, &bindings, Default::default()).unwrap();
    assert!(!resumed.is_poisoned());
}

#[test]
fn bounded_binary_preparation_metadata_shares_aggregate_quota_and_survives_resume() {
    let scratch = scratch();
    let path = scratch.0.join("bundle");
    let bindings = [binding(1)];
    let limits = SavedApplicationCachePackLimits::default();
    let mut writer = SavedApplicationCacheBundleWriter::create(&path, &bindings, limits).unwrap();
    let before = writer.charged_disk_bytes().unwrap();
    let metadata =
        bincode::encode_to_vec(([7u8; 32], 11u64, "partial"), bincode::config::standard()).unwrap();
    writer.checkpoint_metadata(&metadata).unwrap();
    let after = writer.charged_disk_bytes().unwrap();
    assert!(after > before + metadata.len() as u64);
    writer.checkpoint_metadata(&metadata).unwrap();
    assert_eq!(writer.charged_disk_bytes().unwrap(), after);
    drop(writer);
    let bundle = SavedApplicationCacheBundle::open_generated(&path, &bindings, limits).unwrap();
    assert_eq!(bundle.metadata().unwrap(), Some(metadata.clone()));
    let resumed = SavedApplicationCacheBundleWriter::resume(&path, &bindings, limits).unwrap();
    assert_eq!(resumed.charged_disk_bytes().unwrap(), after);
    drop(resumed);
    let tight = SavedApplicationCachePackLimits {
        max_disk_bytes: after,
        ..limits
    };
    let mut writer = SavedApplicationCacheBundleWriter::resume(&path, &bindings, tight).unwrap();
    assert!(
        writer.checkpoint_metadata(&metadata).is_err(),
        "replacement scratch must also fit aggregate disk quota"
    );
    assert_eq!(bundle.metadata().unwrap(), Some(metadata));
}

#[test]
fn fresh_process_native_state_and_guarded_zero_roundtrip() {
    const CHILD: &str = "RUSTRED_APPLICATION_CACHE_TEST_CHILD";
    if let Some(path) = std::env::var_os(CHILD) {
        // Register unrelated names first, so native numeric symbol IDs need
        // not match the exporting process's IDs.
        let _shift =
            CoefficientContext::try_new(["cache_child_foreign_a", "cache_child_foreign_b"])
                .unwrap();
        let mut reader = SavedApplicationCachePack::open_generated(
            Path::new(&path),
            &binding(1),
            Default::default(),
        )
        .unwrap();
        assert_eq!(reader.statistics().chunks_read, 0);
        for n in [0, 1] {
            assert_eq!(reader.lookup(&key(n), &context()).unwrap(), Some(record(n)));
        }
        return;
    }
    let scratch = scratch();
    let path = scratch.0.join("pack");
    let mut writer =
        SavedApplicationCacheWriter::create(&path, binding(1), Default::default()).unwrap();
    writer
        .append_completed(&[record(0), record(1)], &context())
        .unwrap();
    writer.checkpoint().unwrap();
    drop(writer);
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "application::saved_application::cache_pack::tests::fresh_process_native_state_and_guarded_zero_roundtrip", "--nocapture", "--test-threads=1"])
        .env(CHILD, &path).output().unwrap();
    assert!(
        result.status.success(),
        "fresh child failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        String::from_utf8_lossy(&result.stdout).contains("1 passed"),
        "child filter ran no test"
    );
}

#[test]
fn gzip_requires_one_complete_member_exact_length_digest_and_bounded_output() {
    let limits = SavedApplicationCachePackLimits::default();
    let raw = encode_application_cache_chunk(
        &binding(1),
        &[record(0), record(1)],
        &context(),
        limits.chunk,
    )
    .unwrap();
    let digest = *blake3::hash(&raw).as_bytes();
    let encoded = compression::encode(&raw, limits).unwrap();
    assert_eq!(
        compression::decode(&encoded, raw.len() as u64, &digest, limits).unwrap(),
        raw
    );
    assert!(compression::decode(&encoded, raw.len() as u64 - 1, &digest, limits).is_err());
    assert!(compression::decode(&encoded, raw.len() as u64 + 1, &digest, limits).is_err());
    assert!(compression::decode(&encoded, raw.len() as u64, &[0; 32], limits).is_err());
    let mut truncated = encoded.clone();
    truncated.pop();
    assert!(compression::decode(&truncated, raw.len() as u64, &digest, limits).is_err());
    let mut corrupt = encoded.clone();
    let trailer = corrupt.len() - 8;
    corrupt[trailer] ^= 1;
    assert!(compression::decode(&corrupt, raw.len() as u64, &digest, limits).is_err());
    for tail in [&encoded[..], &[0u8][..]] {
        let mut trailing = encoded.clone();
        trailing.extend_from_slice(tail);
        assert!(compression::decode(&trailing, raw.len() as u64, &digest, limits).is_err());
    }
    let mut tight = limits;
    tight.chunk.binary.max_program_bytes = raw.len() - 1;
    assert_eq!(
        compression::decode(&encoded, raw.len() as u64, &digest, tight)
            .unwrap_err()
            .kind(),
        crate::AppErrorKind::Limit
    );
    assert_eq!(
        compression::decode(&encoded, u64::MAX, &digest, tight)
            .unwrap_err()
            .kind(),
        crate::AppErrorKind::Limit
    );
    tight = limits;
    tight.max_encoded_chunk_bytes = encoded.len() - 1;
    assert_eq!(
        compression::encode(&raw, tight).unwrap_err().kind(),
        crate::AppErrorKind::Limit
    );
    assert_eq!(
        compression::decode(&encoded, raw.len() as u64, &digest, tight)
            .unwrap_err()
            .kind(),
        crate::AppErrorKind::Limit
    );
}

#[test]
fn disjoint_batches_coalesce_in_both_directions_and_holes_without_changing_precedence() {
    let scratch = scratch();
    let path = scratch.0.join("coalesced");
    let limits = SavedApplicationCachePackLimits {
        max_runs: 1,
        ..Default::default()
    };
    let mut writer = SavedApplicationCacheWriter::create(&path, binding(1), limits).unwrap();
    for n in [4, 8, 0, 6, 2] {
        writer.append_completed(&[record(n)], &context()).unwrap();
    }
    assert_eq!(writer.statistics().unwrap().runs, 1);
    writer.checkpoint().unwrap();
    let before = physical_chunks(&path);
    assert_eq!(
        writer
            .append_completed(&[record(4)], &context())
            .unwrap_err()
            .kind(),
        crate::AppErrorKind::Limit
    );
    assert_eq!(
        physical_chunks(&path),
        before,
        "directory rejection precedes writes"
    );
    drop(writer);
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(1), limits).unwrap();
    for n in [0, 2, 4, 6, 8] {
        assert_eq!(reader.lookup(&key(n), &context()).unwrap(), Some(record(n)));
    }
    assert_eq!(reader.statistics().chunks_read, 5);
    assert_eq!(reader.lookup(&key(5), &context()).unwrap(), None);
    assert_eq!(reader.statistics().chunks_read, 5);

    let limits = SavedApplicationCachePackLimits {
        max_runs: 2,
        ..limits
    };
    let mut writer = SavedApplicationCacheWriter::resume(&path, &binding(1), limits).unwrap();
    let mut replacement = record(4);
    replacement.terms.clear(); // Trusted completed zero; guard order still exact.
    writer
        .append_completed(&[replacement.clone()], &context())
        .unwrap();
    writer.append_completed(&[record(7)], &context()).unwrap();
    writer.checkpoint().unwrap();
    assert_eq!(writer.statistics().unwrap().runs, 2);
    let mut newer = SavedApplicationCachePack::open_generated(&path, &binding(1), limits).unwrap();
    assert_eq!(
        newer.lookup(&key(4), &context()).unwrap(),
        Some(replacement)
    );
    assert_eq!(newer.lookup(&key(6), &context()).unwrap(), Some(record(6)));
    assert_eq!(
        reader.lookup(&key(4), &context()).unwrap(),
        Some(record(4)),
        "old immutable snapshot unchanged"
    );
}

#[test]
fn overlapping_run_misses_charge_cumulative_read_and_decode_before_next_operation() {
    let scratch = scratch();
    let path = scratch.0.join("overlapping");
    let limits = SavedApplicationCachePackLimits::default();
    let mut writer = SavedApplicationCacheWriter::create(&path, binding(1), limits).unwrap();
    for endpoint in [10, 11, 12] {
        writer
            .append_completed(&[record(0), record(endpoint)], &context())
            .unwrap();
    }
    writer.checkpoint().unwrap();
    drop(writer);
    let directory = manifest(&path);
    assert_eq!(directory.runs.len(), 3);
    let newest = &directory.runs[2][0];
    let second = &directory.runs[1][0];
    let encoded_two = newest.bytes + second.bytes + 2;
    let decoded_two = newest.decoded_bytes + second.decoded_bytes + 2;
    for tight in [
        SavedApplicationCachePackLimits {
            max_lookup_chunks: 1,
            ..limits
        },
        SavedApplicationCachePackLimits {
            max_lookup_encoded_bytes: encoded_two - 1,
            ..limits
        },
        SavedApplicationCachePackLimits {
            max_lookup_decoded_bytes: decoded_two - 1,
            ..limits
        },
    ] {
        let mut reader =
            SavedApplicationCachePack::open_generated(&path, &binding(1), tight).unwrap();
        assert_eq!(
            reader.lookup(&key(5), &context()).unwrap_err().kind(),
            crate::AppErrorKind::Limit
        );
        let stats = reader.statistics();
        assert_eq!(stats.chunks_read, 1);
        assert_eq!(stats.chunk_bytes_read, newest.bytes);
        assert_eq!(stats.decoded_chunk_bytes, newest.decoded_bytes);
        assert_eq!(stats.lookup_encoded_bytes_charged, newest.bytes + 1);
        assert_eq!(stats.lookup_decoded_bytes_charged, newest.decoded_bytes + 1);
        assert_eq!(stats.lookup_limit_refusals, 1);
        assert_eq!(stats.misses, 0, "a budget refusal is not a cache miss");
        assert_eq!(
            reader.lookup(&key(12), &context()).unwrap(),
            Some(record(12)),
            "limits reset for each request"
        );
    }
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(1), limits).unwrap();
    assert_eq!(reader.lookup(&key(5), &context()).unwrap(), None);
    assert_eq!(reader.statistics().chunks_read, 3);
    assert_eq!(reader.statistics().misses, 1);
    for tight in [
        SavedApplicationCachePackLimits {
            max_lookup_chunks: 0,
            ..limits
        },
        SavedApplicationCachePackLimits {
            max_lookup_encoded_bytes: 0,
            ..limits
        },
        SavedApplicationCachePackLimits {
            max_lookup_decoded_bytes: 0,
            ..limits
        },
    ] {
        let mut reader =
            SavedApplicationCachePack::open_generated(&path, &binding(1), tight).unwrap();
        assert_eq!(
            reader.lookup(&key(100), &context()).unwrap(),
            None,
            "range-only misses do not need IO grants"
        );
        assert_eq!(
            reader.lookup(&key(5), &context()).unwrap_err().kind(),
            crate::AppErrorKind::Limit
        );
        assert_eq!(reader.statistics().chunks_read, 0);
    }
}

#[test]
fn schema_is_explicit_and_compressed_disk_census_matches_physical_chunks() {
    let scratch = scratch();
    let path = scratch.0.join("pack");
    let limits = SavedApplicationCachePackLimits::default();
    let mut writer = SavedApplicationCacheWriter::create(&path, binding(1), limits).unwrap();
    writer
        .append_completed(&[record(0), record(1)], &context())
        .unwrap();
    writer.checkpoint().unwrap();
    assert_eq!(writer.physical_chunk_bytes(), physical_chunks(&path).1);
    assert_eq!(
        writer.statistics().unwrap().referenced_chunk_bytes,
        physical_chunks(&path).1
    );
    drop(writer);
    let mut bytes = fs::read(path.join(index::MANIFEST)).unwrap();
    assert!(bytes.starts_with(b"RR-CACHE-PACK-02"));
    bytes[..16].copy_from_slice(b"RR-CACHE-PACK-01");
    let end = bytes.len() - 32;
    let digest = blake3::hash(&bytes[..end]);
    bytes[end..].copy_from_slice(digest.as_bytes());
    fs::write(path.join(index::MANIFEST), bytes).unwrap();
    assert!(
        SavedApplicationCachePack::open_generated(&path, &binding(1), limits).is_err(),
        "no silent legacy-format fallback"
    );
}

#[test]
fn tiny_frame_writes_preserve_bytes_and_never_request_capacity_above_limit() {
    use std::io::Write;
    const LIMIT: usize = 1_003;
    let mut output = index::BoundedBytes::new(LIMIT);
    let expected: Vec<u8> = (0..LIMIT).map(|n| (n % 251) as u8).collect();
    for byte in &expected {
        output.write_all(&[*byte]).unwrap();
        assert!(output.capacity() <= LIMIT);
    }
    assert!(!output.limit_exceeded());
    assert!(output.write_all(&[1]).is_err());
    assert!(output.limit_exceeded());
    assert!(output.capacity() <= LIMIT);
    assert_eq!(output.finish(), expected);
    let mut empty = index::BoundedBytes::new(0);
    empty.write_all(&[]).unwrap();
    assert!(empty.write_all(&[1]).is_err());
    assert_eq!(empty.capacity(), 0);
    assert!(empty.finish().is_empty());
}

#[test]
fn refresh_uses_only_committed_bounded_indexes_and_preserves_lookup_counters() {
    let scratch = scratch();
    let path = scratch.0.join("refresh");
    let limits = SavedApplicationCachePackLimits::default();
    let mut writer = SavedApplicationCacheWriter::create(&path, binding(1), limits).unwrap();
    writer.append_completed(&[record(1)], &context()).unwrap();
    writer.checkpoint().unwrap();
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(1), limits).unwrap();
    let mut old_snapshot = reader.fork_reader();
    let mut bounded = SavedApplicationCachePack::open_generated(
        &path,
        &binding(1),
        SavedApplicationCachePackLimits {
            max_chunks: 1,
            ..limits
        },
    )
    .unwrap();
    assert_eq!(reader.lookup(&key(1), &context()).unwrap(), Some(record(1)));
    let before = reader.statistics();
    writer.append_completed(&[record(2)], &context()).unwrap();
    reader.refresh().unwrap();
    assert_eq!(
        reader.statistics(),
        before,
        "unpublished append is invisible"
    );
    assert_eq!(reader.lookup(&key(2), &context()).unwrap(), None);
    let before = reader.statistics();
    writer.checkpoint().unwrap();
    reader.refresh().unwrap();
    let after = reader.statistics();
    assert_eq!(after.chunks, 2);
    assert_eq!(after.chunks_read, before.chunks_read);
    assert_eq!(after.chunk_bytes_read, before.chunk_bytes_read);
    assert_eq!(after.decoded_chunk_bytes, before.decoded_chunk_bytes);
    assert_eq!(
        after.lookup_encoded_bytes_charged,
        before.lookup_encoded_bytes_charged
    );
    assert_eq!(
        after.lookup_decoded_bytes_charged,
        before.lookup_decoded_bytes_charged
    );
    assert_eq!(after.hits, before.hits);
    assert_eq!(after.misses, before.misses);
    assert_eq!(reader.lookup(&key(2), &context()).unwrap(), Some(record(2)));
    assert_eq!(old_snapshot.lookup(&key(2), &context()).unwrap(), None);
    assert_eq!(
        bounded.refresh().unwrap_err().kind(),
        crate::AppErrorKind::Limit
    );
    assert_eq!(bounded.statistics().chunks, 1);
    assert_eq!(
        bounded.lookup(&key(1), &context()).unwrap(),
        Some(record(1))
    );
    let before = reader.statistics();
    let generation = reader.generation();
    let original = fs::read(path.join(index::MANIFEST)).unwrap();
    fs::write(path.join(index::MANIFEST), &original[..original.len() - 1]).unwrap();
    assert!(reader.refresh().is_err());
    assert_eq!(reader.statistics(), before);
    assert_eq!(reader.generation(), generation);
    fs::write(path.join(index::MANIFEST), &original).unwrap();
    let mut rewound = manifest(&path);
    rewound.generation -= 1;
    fs::write(
        path.join(index::MANIFEST),
        index::encode(&rewound, limits).unwrap(),
    )
    .unwrap();
    assert!(reader.refresh().is_err());
    assert_eq!(reader.statistics(), before);
    assert_eq!(reader.lookup(&key(2), &context()).unwrap(), Some(record(2)));
}
