//! End-to-end native checkpoint integration. Named families are test inputs,
//! never campaign or solver dispatch conditions.
use std::fs;
use std::path::PathBuf;
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

use super::super::checkpoint::{CheckpointManifest, CheckpointStore};
use super::*;
use crate::FamilyCloseProgress;

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../TMP");
        fs::create_dir_all(&base).unwrap();
        for _ in 0..1024 {
            let path = base.join(format!(
                "checkpoint-integration-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(e) => panic!("{e}"),
            }
        }
        panic!("failed to reserve test directory")
    }
    fn options(&self) -> CandidateCheckpointOptions {
        CandidateCheckpointOptions::new(self.0.join("checkpoint"))
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn observed(request: FamilyCandidatesRequest) -> (CandidateBundleResult, Vec<FamilyCloseProgress>) {
    let events = Mutex::new(Vec::new());
    let result =
        family_candidates_with_progress(request, |e| events.lock().unwrap().push(e)).unwrap();
    (result, events.into_inner().unwrap())
}
fn assert_no_search(events: &[FamilyCloseProgress]) {
    assert!(!events.iter().any(|e| matches!(
        e,
        FamilyCloseProgress::Generating { .. }
            | FamilyCloseProgress::GeneratedSector { .. }
            | FamilyCloseProgress::CheckpointedSector { .. }
    )));
}

#[test]
fn checkpoints_preserve_native_programs_and_complete_resume_never_solves() {
    for (source, permutation, depth) in [(K1, None, 2), (K3, Some(vec![2, 0, 1]), 0)] {
        let directory = Directory::new();
        let mut request = FamilyCandidatesRequest::new(source);
        request.permutation = permutation;
        request.numerical_depth = depth;
        request.exact_backend = CandidateExactBackend::SparseFactorized;
        let plain = family_candidates(request.clone()).unwrap();
        let report: toml::Value = toml::from_str(plain.to_toml()).unwrap();
        assert!(report.get("checkpoint").is_none());
        request.checkpoint = Some(directory.options());
        let (saved, events) = observed(request.clone());
        assert_same_program(plain.bundle(), saved.bundle());
        let count = report["solved_sectors"].as_integer().unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, FamilyCloseProgress::CheckpointedSector { .. }))
                .count(),
            count as usize
        );
        // Introduce unrelated native state before cold-style re-import. Both
        // ordered coefficient maps remain part of assert_same_program.
        let _ = preparation::family(
            &K1.replace(
                "dimension = \"d\"",
                "dimension = \"checkpoint_dirty_dimension\"",
            ),
            InputFormat::Toml,
        )
        .unwrap();
        request.checkpoint.as_mut().unwrap().resume = true;
        request.n_cores = 3;
        let (resumed, events) = observed(request);
        assert_no_search(&events);
        assert_same_program(plain.bundle(), resumed.bundle());
        let report: toml::Value = toml::from_str(resumed.to_toml()).unwrap();
        assert_eq!(
            report["checkpoint"]["reused_sectors"].as_integer(),
            Some(count)
        );
        assert_eq!(
            report["checkpoint"]["newly_solved_sectors"].as_integer(),
            Some(0)
        );
        assert!(report["checkpoint"]["disk_bytes"].as_integer().unwrap() > 0);
    }
}

#[test]
fn partial_resume_solves_only_missing_original_ordinal_with_changed_workers() {
    let directory = Directory::new();
    let mut request = FamilyCandidatesRequest::new(K3);
    request.numerical_depth = 0;
    request.permutation = Some(vec![1, 2, 0]);
    let baseline = family_candidates(request.clone()).unwrap();
    let bundle = codec::read(baseline.bundle(), request.bundle_limits).unwrap();
    assert!(bundle.sectors.len() > 2);
    let manifest = CheckpointManifest::for_request(
        &request,
        &bundle.family_fingerprint,
        &bundle.root_sector,
        bundle.sectors.iter().map(|s| s.sector.clone()).collect(),
    )
    .unwrap();
    let mut options = directory.options();
    let store = CheckpointStore::open(&options, manifest, request.bundle_limits).unwrap();
    for (ordinal, sector) in bundle.sectors.iter().enumerate() {
        if ordinal == 1 {
            continue;
        }
        let mut shard = bundle.clone();
        shard.sectors = vec![sector.clone()];
        store
            .publish(
                ordinal,
                &codec::write(&shard, request.bundle_limits).unwrap(),
            )
            .unwrap();
    }
    drop(store);
    options.resume = true;
    request.checkpoint = Some(options);
    request.n_cores = 2;
    let (resumed, events) = observed(request);
    assert_same_program(baseline.bundle(), resumed.bundle());
    let completed: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            FamilyCloseProgress::GeneratedSector { ordinal, .. } => Some(*ordinal),
            _ => None,
        })
        .collect();
    assert_eq!(completed, [1]);
}

#[test]
fn failed_final_encoding_keeps_all_sectors_for_assembly_only_retry() {
    let directory = Directory::new();
    let mut request = FamilyCandidatesRequest::new(K3);
    request.checkpoint = Some(directory.options());
    let baseline = family_candidates(request.clone()).unwrap();
    let directory = &request.checkpoint.as_ref().unwrap().directory;
    let largest = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".rrbin"))
        .map(|e| e.metadata().unwrap().len() as usize)
        .max()
        .unwrap();
    assert!(largest < baseline.bundle().len());
    request.checkpoint.as_mut().unwrap().resume = true;
    request.bundle_limits.max_bundle_bytes = largest;
    let events = Mutex::new(Vec::new());
    let error =
        family_candidates_with_progress(request.clone(), |e| events.lock().unwrap().push(e))
            .unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Limit);
    assert_no_search(&events.into_inner().unwrap());
    request.bundle_limits = CandidateBundleLimits::default();
    let (retry, events) = observed(request);
    assert_no_search(&events);
    assert_same_program(baseline.bundle(), retry.bundle());
}
