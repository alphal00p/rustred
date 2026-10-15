//! Real native closure plus exact prefix-window/generated-cursor association.
use super::*;
use crate::application::routed_campaign::finite_demand::{
    backend::{Policy, Session},
    prepared::PreparedExecution,
    storage::{
        GenerationState, Lookup, RawCensus, ScopeBinding, SelectionRecord, StorageLimits,
        StoreWriter, ValidatedGeneration,
    },
    windows::WindowLimits,
};
use rustred::{
    family::IntegralKey,
    solver::{
        CandidateEntryAdmission, CandidateRoutedDeltaExportLimits,
        CandidateRoutedTraceCensusLimits, CandidateRoutedWorkBudget, DomainPowerBounds,
        FiniteRootAdmission, RootRegionInput,
    },
};
use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

fn policy(workers: usize) -> Policy {
    Policy {
        workers,
        windows: WindowLimits {
            max_seed_points: 1,
            max_seed_bytes: 65536,
            max_positive_layers: 64,
            max_partition_checks: 1000,
        },
        per_batch_work: CandidateRoutedWorkBudget {
            max_nodes: 1000,
            max_rule_applications: 1000,
            max_transport_calls: 1000,
            max_transport_operations: 100000,
            max_transport_endpoints: 100000,
            max_coalescing_additions: 100000,
        },
        prepared_reduction_aggregates: None,
        census: CandidateRoutedTraceCensusLimits::default(),
        export: CandidateRoutedDeltaExportLimits {
            max_records: 1000,
            max_live_payload_bytes: 1000000,
        },
        storage: StorageLimits {
            raw: Default::default(),
            max_arity: 16,
            max_delta_records: 1000,
            max_delta_payload_bytes: 1000000,
            max_page_records: 2,
            max_page_bytes: 65536,
            max_cached_pages: 0,
            max_cached_page_bytes: 0,
            max_directory_bytes: 1000000,
            max_manifest_bytes: 1000000,
            max_state_bytes: 1000000,
            max_scope_rows: 16,
            max_census_records: 1000,
            max_runs: 100,
            max_pages: 1000,
            max_physical_files: 10000,
            max_disk_bytes: 10000000,
            max_lookup_pages: 100,
            max_lookup_bytes: 1000000,
            max_transient_bytes: 128 * 1024 * 1024,
        },
        max_total_rows: 16,
        max_source_bytes: 1048576,
    }
}
fn query(id: &str, a: u64) -> Value {
    json!({"id":id,"owner":"1","lower":[0],"upper":[a.saturating_sub(1)],
    "max_numerator_rank":0,"power_bounds":{"max_positive_power":a,"min_power_difference":0,"max_power_difference":10}})
}
fn source(f: &Fixture) -> OwnerDomainWalkRequest {
    let queries = json!({"schema":"rustred.owner-domain-queries.json.v2","queries":[query("original",3),query("empty",0)],
        "query_roles":{"required":["original","empty"],"auxiliary":[]}});
    let mut matching =
        OwnerDomainMatchRequest::new(f.request.selection_json.clone(), queries.to_string());
    matching.owner_base = f.directory.clone();
    OwnerDomainWalkRequest::new(matching)
}

#[test]
fn durable_driver_pending_pause_cold_reopen_and_exact_stream_match_uninterrupted() {
    use crate::application::routed_campaign::finite_demand::backend::diagnostic::{
        Mode, OutputBudget, drive,
    };
    let f = Fixture::new();
    let source = source(&f);
    let p = policy(1);
    let cancel = AtomicBool::new(false);
    let path = f.directory.join("driver-cold");
    let run = |mode, stop, checkpoint: &Path, label: &str| {
        let output = f.directory.join(label);
        fs::create_dir(&output).unwrap();
        let result = drive::<1>(
            &source,
            p,
            mode,
            checkpoint,
            stop,
            &output,
            &OutputBudget::new(1048576),
            &cancel,
            &|_| {},
        )
        .unwrap();
        (result, output)
    };
    let (initial, _) = run(Mode::Create, Some(0), &path, "driver-initial");
    assert_eq!(initial["status"], "paused");
    assert_eq!(initial["progress"]["input_occurrences"], 0);
    assert!(path.join("CURRENT").exists());
    let (paused, _) = run(Mode::Resume, Some(1), &path, "driver-paused");
    assert_eq!(paused["status"], "paused");
    assert_eq!(paused["progress"]["input_occurrences"], 1);
    let (completed, output) = run(Mode::Resume, None, &path, "driver-finished");
    assert_eq!(completed["status"], "complete");
    assert_eq!(completed["progress"]["input_occurrences"], 3);
    let (control, control_output) = run(
        Mode::Create,
        None,
        &f.directory.join("driver-control"),
        "driver-control-output",
    );
    assert_eq!(control["status"], "complete");
    // Compare all native key and full identity/multiplicity bytes, not hashes/counts.
    for name in ["terminals.jsonl", "zeros.jsonl", "selections.jsonl"] {
        assert_eq!(
            fs::read(output.join(name)).unwrap(),
            fs::read(control_output.join(name)).unwrap()
        );
    }
    // Already-exhausted reopen is only metadata/census replay, not a native all-hit test.
    let (reopened, _) = run(Mode::Resume, None, &path, "driver-exhausted");
    assert_eq!(reopened["committed_windows_this_process"], 0);
    assert_eq!(reopened["status"], "complete");
    let (last_boundary, _) = run(
        Mode::Create,
        Some(3),
        &f.directory.join("driver-last-boundary"),
        "driver-last-boundary-output",
    );
    assert_eq!(last_boundary["status"], "paused");
    assert_eq!(last_boundary["progress"]["all_rows_exhausted"], true);
    assert!(last_boundary["census"].is_null());
    let mut empty_source = source.clone();
    empty_source.matching.queries_json = json!({
        "schema":"rustred.owner-domain-queries.json.v2", "queries":[query("empty",0)],
        "query_roles":{"required":["empty"],"auxiliary":[]}
    })
    .to_string();
    let empty_output = f.directory.join("driver-empty-output");
    fs::create_dir(&empty_output).unwrap();
    let empty = drive::<1>(
        &empty_source,
        p,
        Mode::Create,
        &f.directory.join("driver-empty"),
        Some(0),
        &empty_output,
        &OutputBudget::new(1048576),
        &cancel,
        &|_| {},
    )
    .unwrap();
    assert_eq!(empty["status"], "paused");
    assert_eq!(empty["progress"]["all_rows_exhausted"], true);
    assert!(empty["census"].is_null());
}

#[test]
fn durable_driver_output_refusal_and_cancel_never_report_complete() {
    use crate::application::routed_campaign::finite_demand::backend::diagnostic::{
        Mode, OutputBudget, drive,
    };
    let f = Fixture::new();
    let source = source(&f);
    let p = policy(1);
    let cancel = AtomicBool::new(false);
    let checkpoint = f.directory.join("driver-output-limit");
    let output = f.directory.join("driver-output-limit-result");
    fs::create_dir(&output).unwrap();
    let result = drive::<1>(
        &source,
        p,
        Mode::Create,
        &checkpoint,
        None,
        &output,
        &OutputBudget::new(0),
        &cancel,
        &|_| {},
    )
    .unwrap();
    assert_eq!(result["status"], "incomplete");
    assert_eq!(result["error_kind"], "Limit");
    assert_eq!(result["progress"]["all_rows_exhausted"], true);
    assert!(result["census"].is_null());
    let before = tree(&checkpoint);
    let output = f.directory.join("driver-output-retry");
    fs::create_dir(&output).unwrap();
    let result = drive::<1>(
        &source,
        p,
        Mode::Resume,
        &checkpoint,
        None,
        &output,
        &OutputBudget::new(1048576),
        &cancel,
        &|_| {},
    )
    .unwrap();
    assert_eq!(result["status"], "complete");
    assert_eq!(tree(&checkpoint), before);
    let checkpoint = f.directory.join("driver-cancel");
    let output = f.directory.join("driver-cancel-output");
    fs::create_dir(&output).unwrap();
    let result = drive::<1>(
        &source,
        p,
        Mode::Create,
        &checkpoint,
        None,
        &output,
        &OutputBudget::new(1048576),
        &cancel,
        &|event| {
            if event["event"] == "finite_backend_before_commit" {
                cancel.store(true, Ordering::Release);
            }
        },
    )
    .unwrap();
    assert_eq!(result["status"], "incomplete");
    assert_eq!(result["error_kind"], "Cancelled");
    assert_eq!(result["progress"]["input_occurrences"], 0);
    assert_eq!(result["progress"]["no_active_native_work"], true);
}
fn append(source: &mut OwnerDomainWalkRequest, path: PathBuf) {
    source.amendments.push(OwnerDomainWalkAmendment {path,text:json!({"schema":OWNER_DOMAIN_WALK_SCOPE_EXTENSION_SCHEMA,
        "sequence":1,"parent":source.checkpoint_binding(),"queries":[query("rank-or-power-extension",5)],
        "query_roles":{"required":["rank-or-power-extension"],"auxiliary":[]},"provenance":{"test":"cold exact append"}}).to_string()});
}
fn finish<const N: usize>(session: &mut Session<N>) {
    for _ in 0..100 {
        let step = session.step(&AtomicBool::new(false), &|_| {}).unwrap();
        assert!(step.progress.no_active_native_work);
        if step.progress.all_rows_exhausted {
            assert!(step.progress.required_complete);
            return;
        }
    }
    panic!("finite fixture did not finish within test iteration bound");
}

#[test]
fn finite_publication_capture_requires_complete_no_aux_and_bounded_exact_raw_stream() {
    use crate::application::routed_campaign::finite_demand::backend::publication::CaptureLimits;
    let f = Fixture::new();
    let mut source = source(&f);
    let p = policy(1);
    let cancel = AtomicBool::new(false);
    let limits = CaptureLimits {
        max_terminal_keys: 100,
        max_terminal_payload_bytes: 65536,
    };
    let path = f.directory.join("capture");
    let mut session = Session::<1>::create(&source, p, &path, &cancel, &|_| {}).unwrap();
    assert!(session.capture_completed_source(limits, &cancel).is_err());
    finish(&mut session);
    let original_generation = session.generation().unwrap().digest();
    assert!(
        session
            .capture_completed_source(
                CaptureLimits {
                    max_terminal_keys: 0,
                    ..limits
                },
                &cancel
            )
            .is_err()
    );
    assert_eq!(session.generation().unwrap().digest(), original_generation);
    let captured = session.capture_completed_source(limits, &cancel).unwrap();
    assert_eq!(captured.summary()["complete"], true);
    assert_eq!(captured.summary()["independently_verified"], false);
    assert_eq!(
        captured.summary()["encountered"]["terminals"],
        captured.terminal_keys().len()
    );
    assert_eq!(captured.owner_payload_digests().len(), 1);
    drop(session);
    append(&mut source, f.directory.join("capture-append.json"));
    let mut extended = Session::<1>::resume(&source, p, &path, &cancel, &|_| {}).unwrap();
    finish(&mut extended);
    assert_ne!(
        captured.source_identity(),
        extended.generation().unwrap().digest()
    );
    assert!(captured.request().amendments.is_empty());
    assert_eq!(captured.source_identity(), original_generation);

    let mut auxiliary = source.clone();
    auxiliary.amendments.clear();
    let mut queries: Value = serde_json::from_str(&auxiliary.matching.queries_json).unwrap();
    queries["query_roles"] = json!({"required":["empty"],"auxiliary":["original"]});
    auxiliary.matching.queries_json = queries.to_string();
    let mut session = Session::<1>::create(
        &auxiliary,
        p,
        &f.directory.join("capture-aux"),
        &cancel,
        &|_| {},
    )
    .unwrap();
    finish(&mut session);
    assert!(session.capture_completed_source(limits, &cancel).is_err());
}
fn tree(path: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut result = BTreeMap::new();
    for entry in fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            result.extend(tree(&path));
        } else {
            result.insert(path.clone(), fs::read(path).unwrap());
        }
    }
    result
}
fn compare_union(f: &Fixture, generation: &ValidatedGeneration, p: Policy) {
    let cancel = AtomicBool::new(false);
    let prepared = PreparedExecution::<1>::prepare(&f.request, &cancel, &|_| {})
        .unwrap()
        .unwrap();
    let control_path = f.directory.join(format!("control-{}", p.workers));
    let mut writer =
        StoreWriter::create(&control_path, prepared.execution().clone(), p.storage).unwrap();
    let empty = GenerationState {
        scope: ScopeBinding {
            original_source: [9; 32],
            rows: Vec::new(),
        },
        cursors: Vec::new(),
        completed_work: 0,
        input_occurrences: 0,
        census: RawCensus::default(),
    };
    writer.commit_generated(Vec::new(), empty, &cancel).unwrap();
    let reader = writer
        .current()
        .unwrap()
        .unwrap()
        .reader(p.storage)
        .unwrap();
    let authenticated = prepared
        .authenticate_generation(
            reader.snapshot().clone(),
            &reader.snapshot().state().scope,
            &cancel,
        )
        .unwrap();
    let baseline = authenticated
        .bind(&reader, p.per_batch_work, p.census, p.export, &cancel)
        .unwrap();
    let admission = FiniteRootAdmission::try_new(
        [RootRegionInput {
            support: [true],
            lower: vec![0],
            upper: vec![Some(4)],
            rank: Some(0),
            powers: DomainPowerBounds {
                max_positive_power: Some(5),
                min_power_difference: Some(0),
                max_power_difference: Some(10),
            },
        }],
        1,
    )
    .unwrap();
    let delta = baseline
        .trace_batch(
            (1..=5).map(|n| IntegralKey::try_new([n]).unwrap()),
            CandidateEntryAdmission::ExplicitFinite(&admission),
            p.workers,
            &cancel,
            |_| {},
        )
        .unwrap();
    let actual = generation.reader(p.storage).unwrap();
    assert_eq!(generation.state().completed_work, delta.work().len() as u64);
    for key in delta.work() {
        assert_eq!(actual.lookup(key).unwrap(), Lookup::Present);
    }
    let report = delta.report();
    let census = prepared
        .authenticate_generation(generation.clone(), &generation.state().scope, &cancel)
        .unwrap();
    let mut terminals = Vec::new();
    let mut zeros = Vec::new();
    census.visit_raw_census(p.storage.raw,&cancel,|kind,key|{match kind {
        crate::application::routed_campaign::finite_demand::storage::RawKeyKind::Terminal=>terminals.push(key.to_vec()),
        crate::application::routed_campaign::finite_demand::storage::RawKeyKind::Zero=>zeros.push(key.to_vec())};Ok(())}).unwrap();
    assert_eq!(
        terminals,
        report
            .trace()
            .declared_terminals()
            .iter()
            .map(|k| k.powers().to_vec())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        zeros,
        report
            .trace()
            .visited_zeros()
            .iter()
            .map(|k| k.powers().to_vec())
            .collect::<Vec<_>>()
    );
    let selected = report
        .complete_census()
        .unwrap()
        .selections()
        .iter()
        .map(|(id, &n)| SelectionRecord {
            owner: id.owner.clone(),
            batch: id.batch_ordinal as u64,
            baseline: id.baseline_rule_ordinal as u64,
            selected: id.selected_rule_ordinal as u64,
            multiplicity: n,
        })
        .collect::<Vec<_>>();
    assert_eq!(generation.state().census.selections, selected);
}

#[test]
fn finite_backend_native_windows_cold_resume_append_and_union_work_census_w1_w4() {
    let f = Fixture::new();
    for workers in [1, 4] {
        assert!(crate::test_gates::workers_or_skip(
            "exact finite window backend",
            workers
        ));
        let p = policy(workers);
        let cancel = AtomicBool::new(false);
        let mut source = source(&f);
        let path = f.directory.join(format!("backend-{workers}"));
        let mut session = Session::<1>::create(&source, p, &path, &cancel, &|_| {}).unwrap();
        assert!(path.join("CURRENT").exists());
        assert_eq!(session.progress().unwrap().input_occurrences, 0);
        assert_eq!(session.progress().unwrap().exhausted_required_rows, 1);
        session.step(&cancel, &|_| {}).unwrap();
        session.step(&cancel, &|_| {}).unwrap();
        let before = session.generation().unwrap();
        assert!(before.state().completed_work > 0);
        assert!(!session.progress().unwrap().required_complete);
        drop(session);
        append(&mut source, f.directory.join("appended.json"));
        let mut resumed = Session::<1>::resume(&source, p, &path, &cancel, &|_| {}).unwrap();
        let extended = resumed.generation().unwrap();
        assert_eq!(
            extended.state().completed_work,
            before.state().completed_work
        );
        assert_eq!(extended.state().census, before.state().census);
        assert_eq!(
            &extended.state().cursors[..2],
            before.state().cursors.as_slice()
        );
        assert_eq!(extended.state().cursors[2].input_occurrences, 0);
        assert_eq!(extended.generation(), before.generation() + 1);
        finish(&mut resumed);
        assert_eq!(resumed.progress().unwrap().input_occurrences, 8);
        compare_union(&f, &resumed.generation().unwrap(), p);
        let uninterrupted = f.directory.join(format!("uninterrupted-{workers}"));
        let mut full = Session::<1>::create(&source, p, &uninterrupted, &cancel, &|_| {}).unwrap();
        finish(&mut full);
        assert_eq!(
            full.generation().unwrap().state(),
            resumed.generation().unwrap().state()
        );
        drop(resumed);
        // Only operational aggregate grants and workers may vary on resume.
        let mut changed = p;
        changed.workers = 1;
        changed.per_batch_work.max_rule_applications = 2000;
        source.amendments[0].path = f.directory.join("relocated-amendment-same-bytes.json");
        let reopened = Session::<1>::resume(&source, changed, &path, &cancel, &|_| {}).unwrap();
        assert!(reopened.progress().unwrap().all_rows_exhausted);
    }
}

#[test]
fn finite_backend_cancellation_before_commit_preserves_current_and_requires_cold_reopen() {
    let f = Fixture::new();
    let source = source(&f);
    let p = policy(1);
    let path = f.directory.join("cancel");
    let cancel = AtomicBool::new(false);
    let mut session = Session::<1>::create(&source, p, &path, &cancel, &|_| {}).unwrap();
    let initial = session.generation().unwrap();
    let result = session.step(&cancel, &|event| {
        if event["event"] == "finite_backend_before_commit" {
            cancel.store(true, Ordering::Release);
        }
    });
    assert_eq!(result.unwrap_err().kind(), crate::AppErrorKind::Cancelled);
    assert_eq!(session.generation().unwrap().digest(), initial.digest());
    assert_eq!(session.progress().unwrap().input_occurrences, 0);
    assert!(session.progress().unwrap().stopped);
    cancel.store(false, Ordering::Release);
    assert!(session.step(&cancel, &|_| {}).is_err());
    drop(session);
    let mut resumed = Session::<1>::resume(&source, p, &path, &cancel, &|_| {}).unwrap();
    assert_eq!(
        resumed
            .step(&cancel, &|_| {})
            .unwrap()
            .window
            .unwrap()
            .inputs,
        1
    );
    let previous = resumed.generation().unwrap();
    assert_eq!(
        resumed
            .step(&cancel, &|event| {
                if event["event"] == "finite_backend_native" {
                    cancel.store(true, Ordering::Release);
                }
            })
            .unwrap_err()
            .kind(),
        crate::AppErrorKind::Cancelled
    );
    assert_eq!(resumed.generation().unwrap().digest(), previous.digest());
    drop(resumed);
    cancel.store(false, Ordering::Release);
    let mut resumed = Session::<1>::resume(&source, p, &path, &cancel, &|_| {}).unwrap();
    finish(&mut resumed);
    assert_eq!(resumed.progress().unwrap().input_occurrences, 3);
}

#[test]
fn finite_backend_rejects_scope_geometry_roles_window_policy_and_execution_before_writes() {
    let f = Fixture::new();
    let source = source(&f);
    let p = policy(1);
    let path = f.directory.join("reject");
    let session =
        Session::<1>::create(&source, p, &path, &AtomicBool::new(false), &|_| {}).unwrap();
    drop(session);
    let before = tree(&path);
    for change in ["role", "id", "geometry", "order", "execution", "window"] {
        let mut changed = source.clone();
        let mut policy = p;
        let mut doc: Value = serde_json::from_str(&changed.matching.queries_json).unwrap();
        match change {
            "role" => doc["query_roles"] = json!({"required":["empty"],"auxiliary":["original"]}),
            "id" => {
                doc["queries"][0]["id"] = json!("changed");
                doc["query_roles"]["required"] = json!(["changed", "empty"]);
            }
            "geometry" => doc["queries"][0] = query("original", 4),
            "order" => doc["queries"].as_array_mut().unwrap().reverse(),
            "execution" => {
                changed
                    .matching
                    .reduction_limits
                    .exact_algebra
                    .max_polynomial_terms += 1
            }
            "window" => policy.windows.max_seed_points = 2,
            _ => unreachable!(),
        }
        changed.matching.queries_json = doc.to_string();
        assert!(
            Session::<1>::resume(&changed, policy, &path, &AtomicBool::new(false), &|_| {})
                .is_err(),
            "{change}"
        );
        assert_eq!(tree(&path), before, "{change}");
    }
    let mut invalid = source.clone();
    append(&mut invalid, f.directory.join("bad-chain.json"));
    invalid.matching.reduction_limits.max_rule_applications += 1;
    assert!(Session::<1>::resume(&invalid, p, &path, &AtomicBool::new(false), &|_| {}).is_err());
    assert_eq!(tree(&path), before);
}

#[test]
fn finite_backend_policy_refusal_keeps_earlier_closed_rows_and_empty_initial_generation() {
    let f = Fixture::new();
    let mut source = source(&f);
    let mut doc: Value = serde_json::from_str(&source.matching.queries_json).unwrap();
    doc["queries"][0] = query("original", 1);
    source.matching.queries_json = doc.to_string();
    append(&mut source, f.directory.join("extension.json"));
    let mut p = policy(1);
    p.per_batch_work.max_rule_applications = 0;
    let path = f.directory.join("quota");
    let mut session =
        Session::<1>::create(&source, p, &path, &AtomicBool::new(false), &|_| {}).unwrap();
    session.step(&AtomicBool::new(false), &|_| {}).unwrap();
    session.step(&AtomicBool::new(false), &|_| {}).unwrap();
    let before = session.generation().unwrap();
    assert_eq!(
        session
            .step(&AtomicBool::new(false), &|_| {})
            .unwrap_err()
            .kind(),
        crate::AppErrorKind::Limit
    );
    assert_eq!(session.generation().unwrap().digest(), before.digest());
    let status = session.progress().unwrap();
    assert!(!status.required_complete);
    assert!(status.no_active_native_work);
    assert_eq!(status.exhausted_required_rows, 2);
    assert_eq!(status.current_row, Some(2));
    assert_eq!(status.current_window, Some(1));
}

#[test]
fn finite_backend_path_relocation_and_required_vs_auxiliary_completion_are_explicit() {
    let f = Fixture::new();
    let mut source = source(&f);
    let mut doc: Value = serde_json::from_str(&source.matching.queries_json).unwrap();
    doc["queries"][0] = query("original", 1);
    doc["queries"].as_array_mut().unwrap().push(query("aux", 3));
    doc["query_roles"] = json!({"required":["original","empty"],"auxiliary":["aux"]});
    source.matching.queries_json = doc.to_string();
    let p = policy(1);
    let path = f.directory.join("aux");
    let mut session =
        Session::<1>::create(&source, p, &path, &AtomicBool::new(false), &|_| {}).unwrap();
    let status = session
        .step(&AtomicBool::new(false), &|_| {})
        .unwrap()
        .progress;
    assert!(status.required_complete);
    assert!(!status.all_rows_exhausted);
    assert_eq!(status.exhausted_auxiliary_rows, 0);
    drop(session);
    let relocated = f.directory.join("relocated");
    fs::create_dir(&relocated).unwrap();
    fs::copy(
        f.directory.join("owner.rrbin"),
        relocated.join("owner.rrbin"),
    )
    .unwrap();
    source.matching.owner_base = relocated;
    let mut resumed =
        Session::<1>::resume(&source, p, &path, &AtomicBool::new(false), &|_| {}).unwrap();
    finish(&mut resumed);
    assert_eq!(resumed.progress().unwrap().auxiliary_rows, 1);
    assert_eq!(resumed.progress().unwrap().exhausted_auxiliary_rows, 1);
}

#[test]
fn finite_backend_persistence_refusal_does_not_commit_a_completed_native_window() {
    let f = Fixture::new();
    let source = source(&f);
    let mut p = policy(1);
    // Initial empty metadata fits; the first actual native Work delta cannot.
    p.storage.max_delta_records = 0;
    let path = f.directory.join("storage-refusal");
    let cancel = AtomicBool::new(false);
    let mut session = Session::<1>::create(&source, p, &path, &cancel, &|_| {}).unwrap();
    let before = session.generation().unwrap();
    let sealed = AtomicBool::new(false);
    let result = session.step(&cancel, &|event| {
        if event["event"] == "finite_backend_before_commit" {
            sealed.store(true, Ordering::Release);
        }
    });
    assert_eq!(result.unwrap_err().kind(), crate::AppErrorKind::Limit);
    assert!(sealed.load(Ordering::Acquire));
    assert_eq!(session.generation().unwrap().digest(), before.digest());
    assert_eq!(session.progress().unwrap().input_occurrences, 0);
    assert!(session.progress().unwrap().stopped);
    assert!(session.step(&cancel, &|_| {}).is_err());
    drop(session);
    let mut resumed = Session::<1>::resume(&source, policy(1), &path, &cancel, &|_| {}).unwrap();
    assert_eq!(
        resumed
            .step(&cancel, &|_| {})
            .unwrap()
            .window
            .unwrap()
            .inputs,
        1
    );
}
