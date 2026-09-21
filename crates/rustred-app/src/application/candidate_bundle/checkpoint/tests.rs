//! Store/metadata tests, deliberately without sector discovery. The root
//! integration suite separately checks solver-free complete resume and final
//! native coefficient/source/guard equivalence across worker counts.

use std::sync::atomic::{AtomicUsize, Ordering};

use rustred::persistence::{CoefficientTableBuilder, EncodedCoefficientTable, NativeFamilyRecord};

use crate::application::candidate_bundle::{codec, model::*, policy, preparation};
use crate::application::{AppErrorKind, InputFormat};

use super::*;

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        // Always workspace-local, including when the caller forgot TMPDIR.
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../TMP");
        fs::create_dir_all(&base).unwrap();
        for _ in 0..1024 {
            let path = base.join(format!(
                "candidate-checkpoint-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("{error}"),
            }
        }
        panic!("cannot reserve checkpoint test directory")
    }
    fn options(&self) -> CandidateCheckpointOptions {
        CandidateCheckpointOptions::new(self.0.join("checkpoint"))
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        // Exact uniquely created test path, never the parent/workspace/TMP root.
        let _ = fs::remove_dir_all(&self.0);
    }
}

const SOURCE: &str = r#"
schema = "rustred.project.toml.v1"
[family]
name = "checkpoint_store_fixture"
loop_momenta = ["q1", "q2"]
external_momenta = []
dimension = "d"
[[family.denominators]]
id = "P1"
expression = "q1^2-1"
[[family.denominators]]
id = "P2"
expression = "q2^2-1"
[[family.denominators]]
id = "P3"
expression = "(q1-q2)^2-1"
[target]
powers = [1, 1, 1]
"#;

fn masks() -> Vec<Vec<bool>> {
    vec![
        vec![false, true, true],
        vec![true, false, true],
        vec![true, true, false],
        vec![true, true, true],
    ]
}

fn manifest(request: &FamilyCandidatesRequest) -> CheckpointManifest {
    CheckpointManifest::for_request(request, "fixture-fingerprint", &[true; 3], masks()).unwrap()
}

/// Native family and dictionary, but no solve or rule/certificate invention.
/// The ordinary empty-rule sector has one concrete finite residual.
fn fixture() -> (FamilyCandidatesRequest, CheckpointManifest, Vec<Vec<u8>>) {
    let mut request = FamilyCandidatesRequest::new(SOURCE);
    request.input_format = InputFormat::Toml;
    let family = preparation::family(SOURCE, request.input_format).unwrap();
    let manifest =
        CheckpointManifest::for_request(&request, family.fingerprint(), &[true; 3], masks())
            .unwrap();
    let mut table = CoefficientTableBuilder::new(request.bundle_limits.binary_limits());
    let family_record = NativeFamilyRecord::from_family(&family, &mut table).unwrap();
    let table = table.finish().unwrap();
    let bytes = masks()
        .into_iter()
        .map(|sector| {
            let record = ProgramRecord {
                schema: CANDIDATE_BUNDLE_SCHEMA.into(),
                status: STATUS.into(),
                solver_policy: policy::encode(request.numerical_depth),
                family_source: request.source.clone(),
                input_format: request.input_format.as_str().into(),
                family_fingerprint: family.fingerprint().into(),
                root_sector: vec![true; 3],
                permutation: request.permutation.clone(),
                sectors: vec![SectorRecord {
                    finite_residuals: vec![IntegralRecord {
                        symbolic: vec![false; 3],
                        values: sector.iter().map(|&active| i16::from(active)).collect(),
                    }],
                    sector,
                    rules: Vec::new(),
                }],
            };
            codec::write_records(&record, &family_record, &table, request.bundle_limits).unwrap()
        })
        .collect();
    (request, manifest, bytes)
}

#[test]
fn manifest_identity_excludes_workers_budgets_but_includes_every_recipe_binding() {
    let request = FamilyCandidatesRequest::new(SOURCE);
    let original = manifest(&request);
    let mut changed = request.clone();
    changed.n_cores = 6;
    changed.bundle_limits.max_bundle_bytes /= 2;
    assert_eq!(original, manifest(&changed));
    let bytes = original.encode(usize::MAX).unwrap();
    for change in 0..6 {
        let mut changed = request.clone();
        match change {
            0 => changed.source.push_str("\n# different exact source"),
            1 => changed.input_format = InputFormat::Toml,
            2 => changed.numerical_depth = 0,
            3 => changed.exact_backend = CandidateExactBackend::SparseFactorized,
            4 => changed.permutation = Some(vec![2, 0, 1]),
            _ => changed.max_numerator_rank = Some(20),
        }
        assert!(
            manifest(&changed)
                .admit_manifest(&bytes, request.bundle_limits)
                .is_err()
        );
    }
    let other_family =
        CheckpointManifest::for_request(&request, "different", &[true; 3], masks()).unwrap();
    assert!(
        other_family
            .admit_manifest(&bytes, request.bundle_limits)
            .is_err()
    );
    let mut restricted = request.clone();
    restricted.nonpositive_indices = vec![0];
    let other_root = CheckpointManifest::for_request(
        &restricted,
        "fixture-fingerprint",
        &[false, true, true],
        vec![vec![false, true, true]],
    )
    .unwrap();
    assert!(
        other_root
            .admit_manifest(&bytes, request.bundle_limits)
            .is_err()
    );
    let subset = CheckpointManifest::for_request(
        &request,
        "fixture-fingerprint",
        &[true; 3],
        masks()[1..].to_vec(),
    )
    .unwrap();
    assert!(
        subset
            .admit_manifest(&bytes, request.bundle_limits)
            .is_err()
    );
    let unknown = [bytes.as_slice(), b"unknown_key = 1\n"].concat();
    assert!(
        original
            .admit_manifest(&unknown, request.bundle_limits)
            .is_err()
    );
}

#[test]
fn manifest_rejects_invalid_masks_order_roots_and_bounded_collections() {
    let mut request = FamilyCandidatesRequest::new(SOURCE);
    for sectors in [
        vec![vec![true; 3], vec![true; 3]],
        masks().into_iter().rev().collect(),
        vec![vec![true; 2]],
    ] {
        assert!(CheckpointManifest::for_request(&request, "fp", &[true; 3], sectors).is_err());
    }
    request.nonpositive_indices = vec![1];
    assert!(
        CheckpointManifest::for_request(&request, "fp", &[true, false, true], masks()).is_err()
    );
    assert!(CheckpointManifest::for_request(&request, "fp", &[true; 3], masks()).is_err());
    request.nonpositive_indices.clear();
    request.bundle_limits.max_collection_entries = 18; // 4*3 + 4 + 3 = 19.
    assert_eq!(
        CheckpointManifest::for_request(&request, "fp", &[true; 3], masks())
            .unwrap_err()
            .kind(),
        AppErrorKind::Limit
    );
    request.bundle_limits.max_collection_entries = 19;
    let manifest = manifest(&request);
    let bytes = manifest.encode(usize::MAX).unwrap();
    assert!(manifest.encode(bytes.len() - 1).is_err());
    assert_eq!(manifest.encode(bytes.len()).unwrap(), bytes);
}

#[test]
fn exclusive_lock_is_released_on_drop_and_stable_lockfile_is_not_unlinked() {
    let directory = TestDirectory::new();
    let mut options = directory.options();
    let request = FamilyCandidatesRequest::new(SOURCE);
    let store = CheckpointStore::open(&options, manifest(&request), request.bundle_limits).unwrap();
    options.resume = true;
    let error =
        CheckpointStore::open(&options, manifest(&request), request.bundle_limits).unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Execution);
    assert!(error.message().contains("active owner"));
    drop(store);
    assert!(options.directory.join(LOCK).is_file());
    let resumed =
        CheckpointStore::open(&options, manifest(&request), request.bundle_limits).unwrap();
    assert_eq!(resumed.pending().unwrap().len(), 4);
    drop(resumed);
    assert!(options.directory.join(LOCK).is_file());
}

#[test]
fn complete_and_partial_resume_keep_exact_original_ordinals_and_receipts() {
    let directory = TestDirectory::new();
    let mut options = directory.options();
    let (request, manifest, chunks) = fixture();
    let store = CheckpointStore::open(&options, manifest.clone(), request.bundle_limits).unwrap();
    for ordinal in [3, 1] {
        store.publish(ordinal, &chunks[ordinal]).unwrap();
    }
    assert_eq!(
        store
            .receipts()
            .unwrap()
            .iter()
            .map(|r| r.ordinal)
            .collect::<Vec<_>>(),
        vec![1, 3]
    );
    assert!(store.publish(1, &chunks[1]).is_err());
    assert!(store.read(0).is_err());
    drop(store);
    options.resume = true;
    let store = CheckpointStore::open(&options, manifest.clone(), request.bundle_limits).unwrap();
    assert_eq!(
        store.pending().unwrap(),
        vec![(0, masks()[0].clone()), (2, masks()[2].clone())]
    );
    for ordinal in [2, 0] {
        store.publish(ordinal, &chunks[ordinal]).unwrap();
    }
    drop(store);
    let store = CheckpointStore::open(&options, manifest, request.bundle_limits).unwrap();
    assert!(store.pending().unwrap().is_empty());
    for (ordinal, receipt) in store.receipts().unwrap().iter().enumerate() {
        assert_eq!(receipt.ordinal, ordinal);
        assert_eq!((receipt.rules, receipt.finite_residuals), (0, 1));
        assert_eq!(store.read(ordinal).unwrap(), chunks[ordinal]);
        // Demonstrate the later, sole algebra import still succeeds.
        assert_eq!(
            codec::read(&store.read(ordinal).unwrap(), request.bundle_limits)
                .unwrap()
                .sectors
                .len(),
            1
        );
    }
}

#[test]
fn shard_binding_precedes_native_import_and_never_overwrites_mismatched_work() {
    let directory = TestDirectory::new();
    let mut options = directory.options();
    let (request, manifest, chunks) = fixture();
    let store = CheckpointStore::open(&options, manifest.clone(), request.bundle_limits).unwrap();
    assert!(store.publish(0, &chunks[1]).is_err());
    let (_, mut record, family) = codec::read_structure(&chunks[0], request.bundle_limits).unwrap();
    record.family_fingerprint.push_str("-wrong");
    // Native poison must never be entered: metadata mismatch is sufficient.
    let poison = EncodedCoefficientTable {
        state: vec![0xde],
        atoms: vec![0xad],
    };
    let wrong = codec::write_records(&record, &family, &poison, request.bundle_limits).unwrap();
    assert!(
        store
            .publish(0, &wrong)
            .unwrap_err()
            .message()
            .contains("binding")
    );
    fs::write(options.directory.join(sector_name(0)), &wrong).unwrap();
    drop(store);
    options.resume = true;
    assert!(
        CheckpointStore::open(&options, manifest, request.bundle_limits)
            .unwrap_err()
            .message()
            .contains("binding")
    );
    assert_eq!(
        fs::read(options.directory.join(sector_name(0))).unwrap(),
        wrong
    );
}

#[test]
fn invalid_native_payload_is_structurally_retained_for_single_assembly_import() {
    let directory = TestDirectory::new();
    let mut options = directory.options();
    let (request, manifest, chunks) = fixture();
    let store = CheckpointStore::open(&options, manifest.clone(), request.bundle_limits).unwrap();
    let (_, record, family) = codec::read_structure(&chunks[0], request.bundle_limits).unwrap();
    // Invalid coefficient-table frame is rejected by native preflight before
    // State import. Structural resume does not import algebra a second time.
    let poison = EncodedCoefficientTable {
        state: vec![0xde],
        atoms: Vec::new(),
    };
    let malformed = codec::write_records(&record, &family, &poison, request.bundle_limits).unwrap();
    store.publish(0, &malformed).unwrap();
    drop(store);
    options.resume = true;
    let store = CheckpointStore::open(&options, manifest, request.bundle_limits).unwrap();
    assert_eq!(store.pending().unwrap().len(), 3);
    let bytes = store.read(0).unwrap();
    assert!(codec::read(&bytes, request.bundle_limits).is_err());
    assert!(store.publish(0, &chunks[0]).is_err());
    assert_eq!(
        fs::read(options.directory.join(sector_name(0))).unwrap(),
        malformed
    );
}

#[test]
fn exact_disk_cap_reserves_concurrent_outputs_and_keeps_failed_write_charge() {
    let directory = TestDirectory::new();
    let mut options = directory.options();
    let (request, manifest, chunks) = fixture();
    let manifest_bytes = manifest.encode(usize::MAX).unwrap().len();
    options.max_total_bytes = manifest_bytes + chunks[0].len();
    let store = CheckpointStore::open(&options, manifest, request.bundle_limits).unwrap();
    let error = store
        .publish_with(0, &chunks[0], |_, _| Err("simulated staging error".into()))
        .unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Execution);
    assert_eq!(store.charged_bytes().unwrap(), options.max_total_bytes);
    assert_eq!(
        store.publish(1, &chunks[1]).unwrap_err().kind(),
        AppErrorKind::Limit
    );
    assert!(store.pending().is_err());
    assert!(store.receipts().unwrap().is_empty());
    assert!(!options.directory.join(sector_name(0)).exists());
}

#[test]
fn post_install_error_keeps_installed_chunk_and_only_resume_resolves_receipt() {
    let directory = TestDirectory::new();
    let mut options = directory.options();
    let (request, manifest, chunks) = fixture();
    let store = CheckpointStore::open(&options, manifest.clone(), request.bundle_limits).unwrap();
    let before = store.charged_bytes().unwrap();
    store
        .publish_with(0, &chunks[0], |path, bytes| {
            write_file_atomically(path, bytes, false)?;
            Err("simulated post-install directory-sync failure".into())
        })
        .unwrap_err();
    assert_eq!(store.charged_bytes().unwrap(), before + chunks[0].len());
    assert!(store.publish(0, &chunks[0]).is_err());
    drop(store);
    options.resume = true;
    let resumed = CheckpointStore::open(&options, manifest, request.bundle_limits).unwrap();
    assert_eq!(resumed.receipts().unwrap().len(), 1);
    assert_eq!(resumed.read(0).unwrap(), chunks[0]);
}

#[test]
fn destination_appearing_after_open_is_not_clobbered() {
    let directory = TestDirectory::new();
    let options = directory.options();
    let (request, manifest, chunks) = fixture();
    let store = CheckpointStore::open(&options, manifest, request.bundle_limits).unwrap();
    let path = options.directory.join(sector_name(0));
    fs::write(&path, b"preserve unexpected destination").unwrap();
    assert!(store.publish(0, &chunks[0]).is_err());
    assert!(store.pending().is_err());
    assert_eq!(fs::read(path).unwrap(), b"preserve unexpected destination");
}

#[test]
fn concurrent_distinct_publications_have_exact_shared_accounting() {
    let directory = TestDirectory::new();
    let options = directory.options();
    let (request, manifest, chunks) = fixture();
    let store = CheckpointStore::open(&options, manifest, request.bundle_limits).unwrap();
    let before = store.charged_bytes().unwrap();
    std::thread::scope(|scope| {
        for (ordinal, bytes) in chunks.iter().enumerate() {
            let store = &store;
            scope.spawn(move || {
                store.publish(ordinal, bytes).unwrap();
            });
        }
    });
    assert_eq!(
        store.charged_bytes().unwrap(),
        before + chunks.iter().map(Vec::len).sum::<usize>()
    );
    assert!(store.pending().unwrap().is_empty());
    assert_eq!(store.receipts().unwrap().len(), 4);
}

#[test]
fn pending_write_reserves_disk_cap_before_a_second_worker_can_publish() {
    let directory = TestDirectory::new();
    let mut options = directory.options();
    let (request, manifest, chunks) = fixture();
    options.max_total_bytes = manifest.encode(usize::MAX).unwrap().len() + chunks[0].len();
    let store = CheckpointStore::open(&options, manifest, request.bundle_limits).unwrap();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        let worker_store = &store;
        let bytes = &chunks[0];
        let worker = scope.spawn(move || {
            worker_store.publish_with(0, bytes, |path, bytes| {
                ready_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                write_file_atomically(path, bytes, false)
            })
        });
        ready_rx.recv().unwrap();
        assert_eq!(store.charged_bytes().unwrap(), options.max_total_bytes);
        assert_eq!(
            store.publish(1, &chunks[1]).unwrap_err().kind(),
            AppErrorKind::Limit
        );
        assert!(store.pending().is_err());
        release_tx.send(()).unwrap();
        worker.join().unwrap().unwrap();
    });
    assert_eq!(store.receipts().unwrap().len(), 1);
}

#[test]
fn abandoned_staging_is_never_completed_but_is_charged_on_resume() {
    let directory = TestDirectory::new();
    let mut options = directory.options();
    let request = FamilyCandidatesRequest::new(SOURCE);
    let store = CheckpointStore::open(&options, manifest(&request), request.bundle_limits).unwrap();
    let before = store.charged_bytes().unwrap();
    drop(store);
    let staging = options.directory.join(".sector-0.rrbin.rustred-tmp-123-0");
    fs::write(&staging, [0_u8; 17]).unwrap();
    options.resume = true;
    options.max_total_bytes = before + 16;
    assert_eq!(
        CheckpointStore::open(&options, manifest(&request), request.bundle_limits)
            .unwrap_err()
            .kind(),
        AppErrorKind::Limit
    );
    options.max_total_bytes += 1;
    let store = CheckpointStore::open(&options, manifest(&request), request.bundle_limits).unwrap();
    assert_eq!(store.charged_bytes().unwrap(), before + 17);
    assert_eq!(store.pending().unwrap().len(), 4);
    assert!(staging.exists());
}

#[test]
fn unknown_files_corrupt_committed_chunks_and_noncanonical_names_fail_closed() {
    for name in [
        "report.toml",
        "sector-00.rrbin",
        "sector-4.rrbin",
        ".unrelated.rustred-tmp-123-0",
    ] {
        let directory = TestDirectory::new();
        let mut options = directory.options();
        let request = FamilyCandidatesRequest::new(SOURCE);
        drop(CheckpointStore::open(&options, manifest(&request), request.bundle_limits).unwrap());
        fs::write(options.directory.join(name), []).unwrap();
        options.resume = true;
        assert!(
            CheckpointStore::open(&options, manifest(&request), request.bundle_limits).is_err(),
            "{name}"
        );
    }
    let directory = TestDirectory::new();
    let mut options = directory.options();
    let (request, manifest, chunks) = fixture();
    let store = CheckpointStore::open(&options, manifest.clone(), request.bundle_limits).unwrap();
    store.publish(0, &chunks[0]).unwrap();
    fs::write(options.directory.join(sector_name(0)), &chunks[0][..8]).unwrap();
    assert!(store.read(0).is_err());
    drop(store);
    options.resume = true;
    assert!(CheckpointStore::open(&options, manifest, request.bundle_limits).is_err());
    assert_eq!(
        fs::metadata(options.directory.join(sector_name(0)))
            .unwrap()
            .len(),
        8
    );
}

#[test]
fn new_resume_and_byte_boundaries_fail_without_replacing_existing_data() {
    let directory = TestDirectory::new();
    let mut options = directory.options();
    let request = FamilyCandidatesRequest::new(SOURCE);
    options.resume = true;
    assert!(CheckpointStore::open(&options, manifest(&request), request.bundle_limits).is_err());
    options.resume = false;
    options.max_total_bytes = 1;
    assert_eq!(
        CheckpointStore::open(&options, manifest(&request), request.bundle_limits)
            .unwrap_err()
            .kind(),
        AppErrorKind::Limit
    );
    options.max_total_bytes = 1 << 20;
    drop(CheckpointStore::open(&options, manifest(&request), request.bundle_limits).unwrap());
    let old = fs::read(options.directory.join(MANIFEST)).unwrap();
    assert!(CheckpointStore::open(&options, manifest(&request), request.bundle_limits).is_err());
    assert_eq!(fs::read(options.directory.join(MANIFEST)).unwrap(), old);
    assert!(read_bounded(&options.directory.join(MANIFEST), old.len() - 1).is_err());
    assert_eq!(
        read_bounded(&options.directory.join(MANIFEST), old.len()).unwrap(),
        old
    );
    assert!(sector_ordinal("sector-00.rrbin", 4).is_none());
    assert!(is_staging(".checkpoint.toml.rustred-tmp-1-0", 4));
    assert!(!is_staging(".sector-0.rrbin.rustred-tmp--0", 4));
}

#[cfg(unix)]
#[test]
fn symlinked_checkpoint_entries_are_rejected_without_touching_their_target() {
    let directory = TestDirectory::new();
    let mut options = directory.options();
    let request = FamilyCandidatesRequest::new(SOURCE);
    drop(CheckpointStore::open(&options, manifest(&request), request.bundle_limits).unwrap());
    let target = directory.0.join("user-file");
    fs::write(&target, b"preserve").unwrap();
    std::os::unix::fs::symlink(&target, options.directory.join(sector_name(0))).unwrap();
    options.resume = true;
    assert!(CheckpointStore::open(&options, manifest(&request), request.bundle_limits).is_err());
    assert_eq!(fs::read(target).unwrap(), b"preserve");
}
