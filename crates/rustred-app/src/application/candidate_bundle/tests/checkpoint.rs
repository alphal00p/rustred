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

pub(super) struct Directory(pub(super) PathBuf);
impl Directory {
    pub(super) fn new() -> Self {
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
    pub(super) fn options(&self) -> CandidateCheckpointOptions {
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
fn materialized_callback_plan_is_persisted_and_resumed_without_callback() {
    use rustred::solver::{SectorConfig, SectorSolver, SourceVisitOrder};
    let directory = Directory::new();
    let mut request = FamilyCandidatesRequest::new(K1);
    let family = preparation::family(K1, request.input_format).unwrap();
    let prepared = preparation::prepare::<1>(family, &[true], None).unwrap();
    let mut callback_calls = 0;
    let plans = prepared
        .sectors
        .iter()
        .map(|&sector| {
            let solver = SectorSolver::new(
                &prepared.sources,
                sector,
                SectorConfig {
                    zero_sectors: prepared.zeros.clone(),
                    ..Default::default()
                },
            )
            .unwrap();
            let plan = SourceVisitOrder::by_key(solver.basis(), |ordinal, _| {
                callback_calls += 1;
                std::cmp::Reverse(ordinal)
            })
            .unwrap();
            CandidateSourceVisitPlan {
                sector: sector.to_vec(),
                ordinals: plan.ordinals().to_vec(),
            }
        })
        .collect();
    assert!(callback_calls > 0);
    request.discovery_strategy = Some(CandidateDiscoveryStrategy {
        rows: CandidateSourcePriority::Materialized { sectors: plans },
        ..Default::default()
    });
    request.checkpoint = Some(directory.options());
    let (original, _) = observed(request.clone());
    let manifest_path = request
        .checkpoint
        .as_ref()
        .unwrap()
        .directory
        .join("checkpoint.toml");
    let manifest = fs::read(&manifest_path).unwrap();
    assert!(
        std::str::from_utf8(&manifest)
            .unwrap()
            .contains("materialized")
    );
    request.checkpoint.as_mut().unwrap().resume = true;
    // Simulate reconstructing only the descriptor, with no closure present.
    let json = serde_json::to_string(request.discovery_strategy.as_ref().unwrap()).unwrap();
    request.discovery_strategy = Some(CandidateDiscoveryStrategy::from_json(&json).unwrap());
    let before = callback_calls;
    let (resumed, events) = observed(request.clone());
    assert_no_search(&events);
    assert_same_program(original.bundle(), resumed.bundle());
    assert_eq!(callback_calls, before);
    assert_eq!(manifest, fs::read(&manifest_path).unwrap());
    request.discovery_strategy = None;
    assert!(family_candidates(request).is_err());
    assert_eq!(manifest, fs::read(&manifest_path).unwrap());
}

#[test]
fn bounded_portfolio_report_checkpoint_and_complete_resume_keep_exact_recipe() {
    let directory = Directory::new();
    let mut request = FamilyCandidatesRequest::new(K1);
    request.discovery_strategy = Some(CandidateDiscoveryStrategy::from_json(r#"{
        "version":2,"sectors":{"kind":"active-first"},"rows":{"kind":"input-order"},
        "rule_selection":{"kind":"bounded-portfolio","version":1,
            "alternatives":[{"kind":"features","priorities":[{"feature":{"kind":"terms"},"descending":true}]}],
            "limits":{"max_depth":0,"max_rows":32,"max_exact_trace_rows":32,"max_exact_trace_terms":1024},
            "quality":[{"feature":"rhs-terms","descending":false}],
            "trigger":{"kind":"always"}}
    }"#).unwrap());
    request.checkpoint = Some(directory.options());
    let (original, _) = observed(request.clone());
    let report: toml::Value = toml::from_str(original.to_toml()).unwrap();
    let saved_strategy: CandidateDiscoveryStrategy =
        report["discovery_strategy"].clone().try_into().unwrap();
    assert_eq!(Some(saved_strategy), request.discovery_strategy);
    assert_eq!(
        report["rule_selection"]["newly_solved_sectors"].as_integer(),
        Some(1)
    );
    assert!(report["rule_selection"]["attempted"].as_integer().unwrap() > 0);
    let path = request
        .checkpoint
        .as_ref()
        .unwrap()
        .directory
        .join("checkpoint.toml");
    let before = fs::read(&path).unwrap();
    request.checkpoint.as_mut().unwrap().resume = true;
    let (resumed, events) = observed(request.clone());
    assert_no_search(&events);
    assert_same_program(original.bundle(), resumed.bundle());
    let resumed_report: toml::Value = toml::from_str(resumed.to_toml()).unwrap();
    assert_eq!(
        resumed_report["rule_selection"]["newly_solved_sectors"].as_integer(),
        Some(0)
    );
    assert_eq!(
        resumed_report["rule_selection"]["attempted"].as_integer(),
        Some(0)
    );
    assert_eq!(before, fs::read(&path).unwrap());
    let crate::CandidateRulePortfolio::BoundedPortfolio { trigger, .. } = request
        .discovery_strategy
        .as_mut()
        .unwrap()
        .rule_selection
        .as_mut()
        .unwrap();
    *trigger = crate::CandidateRulePortfolioTrigger::AnyAtLeast {
        thresholds: vec![crate::CandidateRuleQualityThreshold {
            feature: crate::CandidateRuleQualityFeature::RhsTerms,
            minimum: 2,
        }],
    };
    assert!(
        family_candidates(request)
            .unwrap_err()
            .message()
            .contains("manifest differs")
    );
    assert_eq!(before, fs::read(&path).unwrap());
}

#[test]
fn explicit_default_discovery_preserves_default_candidates() {
    let ordinary = family_candidates(FamilyCandidatesRequest::new(K1)).unwrap();
    let mut explicit = FamilyCandidatesRequest::new(K1);
    explicit.discovery_strategy = Some(CandidateDiscoveryStrategy::default());
    let explicit = family_candidates(explicit).unwrap();
    assert_same_program(ordinary.bundle(), explicit.bundle());
}

#[test]
fn case_intersection_resources_do_not_change_saved_identity_or_regenerate_shards() {
    let directory = Directory::new();
    let mut request = FamilyCandidatesRequest::new(K3);
    request.max_numerator_rank = Some(2);
    request.finite_case_policy = FiniteCasePolicy::RetainRankFinite;
    request.checkpoint = Some(directory.options());
    let (original, _) = observed(request.clone());
    let path = request.checkpoint.as_ref().unwrap().directory.clone();
    let files = || {
        fs::read_dir(&path)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                (entry.file_name(), fs::read(entry.path()).unwrap())
            })
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    let before = files();
    request.checkpoint.as_mut().unwrap().resume = true;
    request.case_intersection_limits = CaseIntersectionLimits {
        max_work_items: 16_384,
        max_terms_per_conjunction: 400_000,
        max_normalizations: 4096,
        max_factorizations: 16_384,
    };
    let (resumed, events) = observed(request.clone());
    assert_no_search(&events);
    assert_same_program(original.bundle(), resumed.bundle());
    let report: toml::Value = toml::from_str(resumed.to_toml()).unwrap();
    assert_eq!(report["checkpoint"]["reused_sectors"].as_integer(), Some(4));
    assert_eq!(
        report["checkpoint"]["newly_solved_sectors"].as_integer(),
        Some(0)
    );
    assert_eq!(report["case_max_work_items"].as_integer(), Some(16_384));
    // A zero-work Rust diagnostic would fail any new intersection but must
    // not invalidate or regenerate complete saved sectors.
    request.case_intersection_limits.max_work_items = 0;
    let (zero, events) = observed(request);
    assert_no_search(&events);
    assert_same_program(original.bundle(), zero.bundle());
    assert_eq!(before, files());
}

#[test]
fn finite_retention_checkpoint_binds_policy_and_enumeration_limits() {
    let directory = Directory::new();
    let mut request = FamilyCandidatesRequest::new(K1);
    request.max_numerator_rank = Some(10);
    request.finite_case_policy = FiniteCasePolicy::RetainRankFinite;
    request.checkpoint = Some(directory.options());
    let (original, _) = observed(request.clone());
    request.checkpoint.as_mut().unwrap().resume = true;
    let (resumed, events) = observed(request.clone());
    assert_no_search(&events);
    assert_same_program(original.bundle(), resumed.bundle());
    let mut changed = request.clone();
    changed.finite_case_policy = FiniteCasePolicy::SearchFinite;
    assert!(
        family_candidates(changed)
            .unwrap_err()
            .message()
            .contains("manifest differs")
    );
    for changed_limit in [true, false] {
        let mut changed = request.clone();
        if changed_limit {
            changed.finite_case_limits.max_visited_points += 1;
        } else {
            changed.finite_case_limits.max_retained_terminals += 1;
        }
        assert!(
            family_candidates(changed)
                .unwrap_err()
                .message()
                .contains("manifest differs")
        );
    }
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
fn partial_materialized_resume_reuses_per_sector_plans_and_rejects_same_shape_changes() {
    use rustred::solver::{SectorConfig, SectorSolver, SourceVisitOrder};
    let directory = Directory::new();
    let mut request = FamilyCandidatesRequest::new(K3);
    request.numerical_depth = 0;
    request.permutation = Some(vec![1, 2, 0]);
    let family = preparation::family(K3, request.input_format).unwrap();
    let prepared =
        preparation::prepare::<3>(family, &[true; 3], request.permutation.as_deref()).unwrap();
    let plans: Vec<_> = prepared
        .sectors
        .iter()
        .enumerate()
        .map(|(ordinal, &sector)| {
            let solver = SectorSolver::new(
                &prepared.sources,
                sector,
                SectorConfig {
                    zero_sectors: prepared.zeros.clone(),
                    permutation: prepared.permutation,
                    ..Default::default()
                },
            )
            .unwrap();
            let count = solver.basis().len();
            let plan =
                SourceVisitOrder::by_key(solver.basis(), |row, _| (row + ordinal) % count).unwrap();
            CandidateSourceVisitPlan {
                sector: sector.to_vec(),
                ordinals: plan.ordinals().to_vec(),
            }
        })
        .collect();
    assert!(
        plans
            .iter()
            .map(|p| &p.ordinals)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            > 1
    );
    request.discovery_strategy = Some(CandidateDiscoveryStrategy {
        sectors: CandidateSectorPriority::Materialized {
            ordinals: (0..plans.len()).rev().collect(),
        },
        rows: CandidateSourcePriority::Materialized { sectors: plans },
        ..Default::default()
    });
    let baseline = family_candidates(request.clone()).unwrap();
    let bundle = codec::read(baseline.bundle(), request.bundle_limits).unwrap();
    let last = bundle.sectors.len() - 1;
    assert!(last > 1);
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
        if ordinal == 1 || ordinal == last {
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
    let files = || {
        fs::read_dir(&request.checkpoint.as_ref().unwrap().directory)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                (entry.file_name(), fs::read(entry.path()).unwrap())
            })
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    let before = files();
    let mut wrong = request.clone();
    let CandidateSourcePriority::Materialized { sectors: plans } =
        &mut wrong.discovery_strategy.as_mut().unwrap().rows
    else {
        unreachable!()
    };
    let plan = plans
        .iter_mut()
        .find(|p| p.ordinals.len() >= 2)
        .expect("nontrivial prepared K3 basis");
    plan.ordinals.swap(0, 1); // Same length, still bijective, different saved recipe.
    let rejected_events = Mutex::new(Vec::new());
    assert!(
        family_candidates_with_progress(wrong, |event| rejected_events.lock().unwrap().push(event))
            .is_err()
    );
    assert_no_search(&rejected_events.into_inner().unwrap());
    assert_eq!(before, files());
    let (resumed, events) = observed(request.clone());
    assert_same_program(baseline.bundle(), resumed.bundle());
    let completed: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            FamilyCloseProgress::GeneratedSector { ordinal, .. } => Some(*ordinal),
            _ => None,
        })
        .collect();
    assert_eq!(completed, [last, 1]); // W1, reversed full plan restricted to pending slots.
    let after = files();
    for (name, bytes) in before {
        assert_eq!(after.get(&name), Some(&bytes));
    }
    let report: toml::Value = toml::from_str(resumed.to_toml()).unwrap();
    assert_eq!(
        report["checkpoint"]["newly_solved_sectors"].as_integer(),
        Some(2)
    );
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
fn aggregate_entry_refusal_resumes_assembly_without_any_search() {
    let directory = Directory::new();
    let mut request = FamilyCandidatesRequest::new(K3);
    request.checkpoint = Some(directory.options());
    let baseline = family_candidates(request.clone()).unwrap();
    let bundle = codec::read(baseline.bundle(), request.bundle_limits).unwrap();
    let full = codec::collection_entries(&bundle.records).unwrap();
    let local_max = std::fs::read_dir(&request.checkpoint.as_ref().unwrap().directory)
        .unwrap()
        .map(|e| e.unwrap())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".rrbin"))
        .map(|e| {
            let bytes = std::fs::read(e.path()).unwrap();
            let (_, record, _) = codec::read_structure(&bytes, request.bundle_limits).unwrap();
            codec::collection_entries(&record).unwrap()
        })
        .max()
        .unwrap();
    assert!(local_max < full);
    request.checkpoint.as_mut().unwrap().resume = true;
    request.bundle_limits.max_collection_entries = full - 1;
    let events = Mutex::new(Vec::new());
    let error =
        family_candidates_with_progress(request.clone(), |e| events.lock().unwrap().push(e))
            .unwrap_err();
    assert!(
        error
            .message()
            .contains("aggregate collection-entry budget exceeded:")
    );
    assert_no_search(&events.into_inner().unwrap());
    request.bundle_limits.max_collection_entries = full;
    let (retry, events) = observed(request.clone());
    assert_no_search(&events);
    assert_same_program(baseline.bundle(), retry.bundle());
    assert_eq!(retry.bundle_limits(), request.bundle_limits);
    assert_eq!(
        retry.artifact().unwrap().metadata().unwrap()["collection_entries"],
        full
    );
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
