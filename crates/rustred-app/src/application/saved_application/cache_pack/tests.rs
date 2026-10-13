use super::*;
use rustred::algebra::CoefficientContext;
use rustred::family::IntegralKey;
use rustred::persistence::application_cache::{
    ApplicationCacheBinding, ApplicationCacheKey, ApplicationCacheRecord,
    encode_application_cache_chunk,
};
use std::{collections::BTreeMap, fs, path::Path};

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
    tight.chunk.binary.max_program_bytes = maximum;
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(1), tight).unwrap();
    assert_eq!(reader.statistics().chunks_read, 0);
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
