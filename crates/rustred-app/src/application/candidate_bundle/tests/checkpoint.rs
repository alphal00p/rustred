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
fn numerator_rank_checkpoint_reuses_exact_scope_and_rejects_changed_scope() {
    let directory = Directory::new();
    let mut request = FamilyCandidatesRequest::new(K3);
    request.max_numerator_rank = Some(1);
    request.numerical_depth = 0;
    request.checkpoint = Some(directory.options());
    let (original, _) = observed(request.clone());
    request.checkpoint.as_mut().unwrap().resume = true;
    request.n_cores = 2;
    let (resumed, events) = observed(request.clone());
    assert_no_search(&events);
    assert_same_program(original.bundle(), resumed.bundle());
    assert_eq!(
        inspect_generated_candidate_bundle(resumed.bundle(), Default::default())
            .unwrap()
            .max_numerator_rank,
        Some(1)
    );
    for rank in [None, Some(0), Some(2)] {
        request.max_numerator_rank = rank;
        let events = Mutex::new(Vec::new());
        let rejected = family_candidates_with_progress(request.clone(), |event| {
            events.lock().unwrap().push(event)
        })
        .unwrap_err();
        assert!(rejected.message().contains("manifest differs"));
        assert_no_search(&events.into_inner().unwrap());
    }
}

#[test]
fn checkpoints_preserve_native_programs_and_complete_resume_never_solves() {
    for (source, permutation, depth, backend) in [
        (K1, None, 2, CandidateExactBackend::SparseFactorized),
        (
            K3,
            Some(vec![2, 0, 1]),
            0,
            CandidateExactBackend::SparseFactorized,
        ),
        (
            K1,
            None,
            2,
            CandidateExactBackend::SparseTargetOnlyFactorized,
        ),
        (
            K3,
            Some(vec![2, 0, 1]),
            0,
            CandidateExactBackend::SparseTargetOnlyFactorized,
        ),
    ] {
        let directory = Directory::new();
        let mut request = FamilyCandidatesRequest::new(source);
        request.permutation = permutation;
        request.numerical_depth = depth;
        request.exact_backend = backend;
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

#[test]
fn checkpoint_output_failures_are_live_and_keep_resumed_manifest_ordinals() {
    let directory = Directory::new();
    let mut request = FamilyCandidatesRequest::new(K3);
    request.numerical_depth = 0;
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
    let mut first = bundle.clone();
    first.sectors.truncate(1);
    store
        .publish(0, &codec::write(&first, request.bundle_limits).unwrap())
        .unwrap();
    // Admission fits exactly; every new sector output must fail its byte
    // reservation. An existing shard must not be reported as newly failed.
    options.max_total_bytes = store.charged_bytes().unwrap();
    drop(store);
    options.resume = true;
    request.checkpoint = Some(options);
    let events = Mutex::new(Vec::new());
    let error =
        family_candidates_with_progress(request, |event| events.lock().unwrap().push(event))
            .unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Limit);
    assert!(error.to_string().contains("sector 1 output:"));
    let events = events.into_inner().unwrap();
    let mut failed = Vec::new();
    for (position, event) in events.iter().enumerate() {
        if let FamilyCloseProgress::FailedSector {
            ordinal,
            sector,
            message,
            ..
        } = event
        {
            failed.push(*ordinal);
            assert_eq!(
                *sector,
                bundle.sectors[*ordinal]
                    .sector
                    .iter()
                    .enumerate()
                    .fold(0_u64, |mask, (axis, &active)| mask
                        | (u64::from(active) << axis))
            );
            assert!(message.starts_with("output:") && message.contains("exceed total limit"));
            // Serial execution emits the error immediately after the solved
            // event, before proceeding to another sector's work.
            assert!(matches!(events[position - 1],
                FamilyCloseProgress::GeneratedSector { ordinal: done, .. } if done == *ordinal));
        }
    }
    failed.sort_unstable();
    assert_eq!(failed, (1..bundle.sectors.len()).collect::<Vec<_>>());
    assert!(!events.iter().any(|event| matches!(
        event,
        FamilyCloseProgress::Encoded { .. } | FamilyCloseProgress::CheckpointedSector { .. }
    )));
}
