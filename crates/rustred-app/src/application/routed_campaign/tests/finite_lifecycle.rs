//! Public native lifecycle over the same saved-owner fixture, not a new solver.
use super::*;
use crate::{
    FiniteDemandMode, FiniteDemandPolicy, FiniteDemandRunRequest, inspect_finite_demand,
    inspect_finite_demand_with_policy, run_finite_demand_with_progress,
};
use std::{fs, path::Path, sync::atomic::AtomicBool};

pub(super) fn policy() -> FiniteDemandPolicy {
    FiniteDemandPolicy::from_json(&serde_json::to_vec(&json!({
        "workers":1,"windows":{"max_seed_points":1,"max_seed_bytes":65536,"max_positive_layers":64,"max_partition_checks":1000},
        "census":{"max_unique_selections":1000,"max_identity_payload_bytes":1000000},
        "export":{"max_records":1000,"max_live_payload_bytes":1000000},
        "storage":{"raw":{"max_page_records":2,"max_page_bytes":65536,"max_runs":100,"max_pages":1000,
            "max_directory_bytes":1000000,"max_stored_references":100000,"max_merge_runs":100,
            "max_merge_cached_pages":8,"max_merge_payload_bytes":1000000,"max_merge_read_bytes":10000000,
            "max_decode_transient_bytes":134217728},"max_arity":16,"max_delta_records":1000,"max_delta_payload_bytes":1000000,
            "max_page_records":2,"max_page_bytes":65536,"max_cached_pages":8,"max_cached_page_bytes":1000000,
            "max_directory_bytes":1000000,"max_manifest_bytes":1000000,"max_state_bytes":1000000,
            "max_scope_rows":16,"max_census_records":1000,"max_runs":100,"max_pages":1000,
            "max_physical_files":10000,"max_disk_bytes":10000000,"max_lookup_pages":100,"max_lookup_bytes":1000000,
            "max_transient_bytes":134217728},"max_total_rows":16,"max_source_bytes":1048576,
        "control":{"max_bytes":16777216,"max_files":1000}
    })).unwrap()).unwrap()
}
fn query(id: &str, a: u64) -> Value {
    json!({"id":id,"owner":"1","lower":[0],"upper":[a.saturating_sub(1)],"max_numerator_rank":0,
        "power_bounds":{"max_positive_power":a,"min_power_difference":0,"max_power_difference":10}})
}
fn request(f: &Fixture) -> FiniteDemandRunRequest {
    let queries = json!({"schema":"rustred.owner-domain-queries.json.v2","queries":[query("original",3),query("empty",0)],
        "query_roles":{"required":["original","empty"],"auxiliary":[]}});
    let mut matching =
        OwnerDomainMatchRequest::new(f.request.selection_json.clone(), queries.to_string());
    matching.owner_base = f.directory.clone();
    FiniteDemandRunRequest {
        source: OwnerDomainWalkRequest::new(matching),
        policy: policy(),
        directory: f.directory.join("finite"),
        mode: FiniteDemandMode::Create,
    }
}

#[test]
fn finite_work_progress_preserves_checkpoint_identity_and_cold_resume() {
    let f = Fixture::new();
    let plain = request(&f);
    let mut monitored = plain.clone();
    monitored.directory = f.directory.join("finite-monitored");
    let cancel = AtomicBool::new(false);
    let before = run_finite_demand_with_progress(plain.clone(), &cancel, |_| {}).unwrap();
    let progress = rustred::persistence::finite_work::GeneratedWorkProgress::default();
    let after = crate::run_finite_demand_with_work_progress(
        monitored.clone(),
        &cancel,
        |_| {},
        Some(progress.clone()),
    )
    .unwrap();
    assert!(before.complete && after.complete);
    assert_eq!(
        fs::read(plain.directory.join("store/CURRENT")).unwrap(),
        fs::read(monitored.directory.join("store/CURRENT")).unwrap()
    );
    assert_eq!(progress.snapshot().unwrap().phase, "cleanup_group");
    monitored.mode = FiniteDemandMode::Resume;
    let resumed =
        crate::run_finite_demand_with_work_progress(monitored, &cancel, |_| {}, Some(progress))
            .unwrap();
    assert!(resumed.complete);
}

#[test]
fn public_source_join_preserves_native_encoded_request_identity() {
    use crate::application::routed_campaign::finite_demand::backend::original_source_identity;
    let raw = "12".repeat(32);
    let encoded = bincode::encode_to_vec(&raw, bincode::config::standard()).unwrap();
    let expected = *blake3::hash(&encoded).as_bytes();
    assert_eq!(original_source_identity(&raw).unwrap(), expected);
    assert_ne!(expected, *blake3::Hash::from_hex(&raw).unwrap().as_bytes());
    assert_ne!(
        original_source_identity(&"34".repeat(32)).unwrap(),
        expected
    );
}

#[test]
fn public_inspection_uses_native_raw_frame_bounds_not_stale_operational_grants() {
    let f = Fixture::new();
    let request = request(&f);
    run_finite_demand_with_progress(request.clone(), &AtomicBool::new(false), |_| {}).unwrap();
    let raw = request.directory.join("store/raw");
    assert!(fs::read_dir(&raw).unwrap().next().is_some());
    let before = inspect_finite_demand(&request.directory).unwrap();
    let marker = request.directory.join("active-source.json");
    let mut active: Value = serde_json::from_slice(&fs::read(&marker).unwrap()).unwrap();
    active["policy"]["storage"]["raw"]["max_page_records"] = json!(usize::MAX);
    active["policy"]["storage"]["raw"]["max_page_bytes"] = json!(1);
    active["policy"]["storage"]["raw"]["max_decode_transient_bytes"] = json!(1);
    active["policy"]["storage"]["max_manifest_bytes"] = json!(1);
    fs::write(&marker, serde_json::to_vec(&active).unwrap()).unwrap();
    assert_eq!(inspect_finite_demand(&request.directory).unwrap(), before);
    // Metadata inspection validates framing/directories, not raw payloads.
    // A same-length corrupt page is detected by full source validation later;
    // merely showing committed counters must not read all raw census pages.
    let page = fs::read_dir(&raw).unwrap().next().unwrap().unwrap().path();
    let original = fs::read(&page).unwrap();
    let mut corrupted = original.clone();
    corrupted[0] ^= 1;
    fs::write(&page, corrupted).unwrap();
    assert_eq!(inspect_finite_demand(&request.directory).unwrap(), before);
    fs::write(&page, original).unwrap();
    let path = request.directory.join("execution.json");
    let original = fs::read(&path).unwrap();
    for arity in [0, 257] {
        let mut execution: Value = serde_json::from_slice(&original).unwrap();
        execution["physical_arity"] = json!(arity);
        fs::write(&path, serde_json::to_vec(&execution).unwrap()).unwrap();
        assert!(inspect_finite_demand(&request.directory).is_err());
    }
    fs::write(&path, original).unwrap();
    assert_eq!(inspect_finite_demand(&request.directory).unwrap(), before);
}

#[test]
fn public_finite_numeric_grants_can_be_raised_for_inspection_and_resume() {
    let f = Fixture::new();
    let mut request = request(&f);
    let cancel = AtomicBool::new(false);
    run_finite_demand_with_progress(request.clone(), &cancel, |_| {}).unwrap();
    let current = fs::read(request.directory.join("store/CURRENT")).unwrap();
    request.mode = FiniteDemandMode::Resume;
    let mut limited: Value = serde_json::from_slice(&request.policy.to_json().unwrap()).unwrap();
    limited["storage"]["max_runs"] = json!(0);
    request.policy = FiniteDemandPolicy::from_json(&serde_json::to_vec(&limited).unwrap()).unwrap();
    assert_eq!(
        inspect_finite_demand_with_policy(&request.directory, Some(&request.policy))
            .unwrap_err()
            .kind(),
        crate::AppErrorKind::Limit
    );
    assert_eq!(
        run_finite_demand_with_progress(request.clone(), &cancel, |_| {})
            .unwrap_err()
            .kind(),
        crate::AppErrorKind::Limit
    );
    assert_eq!(
        fs::read(request.directory.join("store/CURRENT")).unwrap(),
        current
    );
    limited["storage"]["max_runs"] = Value::Null;
    request.policy = FiniteDemandPolicy::from_json(&serde_json::to_vec(&limited).unwrap()).unwrap();
    assert_eq!(
        inspect_finite_demand_with_policy(&request.directory, Some(&request.policy)).unwrap()["committed"]
            ["input_occurrences"],
        3
    );
    assert!(
        run_finite_demand_with_progress(request.clone(), &cancel, |_| {})
            .unwrap()
            .complete
    );
    assert_eq!(
        fs::read(request.directory.join("store/CURRENT")).unwrap(),
        current
    );
}

#[test]
fn public_control_metadata_uses_the_caller_byte_grant() {
    let f = Fixture::new();
    let request = request(&f);
    run_finite_demand_with_progress(request.clone(), &AtomicBool::new(false), |_| {}).unwrap();
    let expected = inspect_finite_demand(&request.directory).unwrap();
    let marker = request.directory.join("active-source.json");
    let mut encoded = fs::read(&marker).unwrap();
    encoded.resize(encoded.len() + (1 << 20), b' ');
    fs::write(marker, encoded).unwrap();
    let mut granted: Value = serde_json::from_slice(&request.policy.to_json().unwrap()).unwrap();
    granted["control"]["max_bytes"] = json!(1 << 20);
    let small = FiniteDemandPolicy::from_json(&serde_json::to_vec(&granted).unwrap()).unwrap();
    assert_eq!(
        inspect_finite_demand_with_policy(&request.directory, Some(&small))
            .unwrap_err()
            .kind(),
        crate::AppErrorKind::Limit
    );
    granted["control"]["max_bytes"] = json!(4 << 20);
    let large = FiniteDemandPolicy::from_json(&serde_json::to_vec(&granted).unwrap()).unwrap();
    assert_eq!(
        inspect_finite_demand_with_policy(&request.directory, Some(&large)).unwrap(),
        expected
    );
}
fn append(request: &mut FiniteDemandRunRequest) {
    let text = json!({"schema":crate::OWNER_DOMAIN_WALK_SCOPE_EXTENSION_SCHEMA,"sequence":1,
        "parent":request.source.checkpoint_binding(),"queries":[query("next",4)],
        "query_roles":{"required":["next"],"auxiliary":[]}})
    .to_string();
    request
        .source
        .amendments
        .push(crate::OwnerDomainWalkAmendment {
            path: "amendment-0001.json".into(),
            text,
        });
}

#[test]
fn public_finite_rejects_zero_workers_without_preparing_source() {
    let mut value: Value = serde_json::from_slice(&policy().to_json().unwrap()).unwrap();
    value["workers"] = json!(0);
    let error = FiniteDemandPolicy::from_json(&serde_json::to_vec(&value).unwrap()).unwrap_err();
    assert_eq!(error.kind(), crate::AppErrorKind::Input);
    assert!(error.to_string().contains("positive"));
}

/// Uses the actual public lifecycle and native routed scheduler, not a mocked
/// worker count. On smaller hosts the visible skip must not count as evidence
/// that a 150-worker run was tested.
#[test]
fn public_finite_150_workers_resume_from_32_preserves_work_and_execution_binding() {
    if !crate::test_gates::workers_or_skip("finite 32-to-150 worker resume", 150) {
        return;
    }
    let f = Fixture::new();
    let mut request = request(&f);
    let set_workers = |request: &mut FiniteDemandRunRequest, workers: usize| {
        let mut value: Value = serde_json::from_slice(&request.policy.to_json().unwrap()).unwrap();
        value["workers"] = json!(workers);
        request.policy =
            FiniteDemandPolicy::from_json(&serde_json::to_vec(&value).unwrap()).unwrap();
    };
    set_workers(&mut request, 32);
    let cancel = AtomicBool::new(false);
    let error = run_finite_demand_with_progress(request.clone(), &cancel, |event| {
        if event["event"] == "finite_checkpoint_saved" && event["generation_number"] == 2 {
            cancel.store(true, Ordering::Release);
        }
    })
    .unwrap_err();
    assert_eq!(error.kind(), crate::AppErrorKind::Cancelled);
    let before = inspect_finite_demand(&request.directory).unwrap();
    assert_eq!(before["committed"]["input_occurrences"], 1);
    let execution = fs::read(request.directory.join("execution.json")).unwrap();

    cancel.store(false, Ordering::Release);
    request.mode = FiniteDemandMode::Resume;
    set_workers(&mut request, 150);
    let saw_native = AtomicBool::new(false);
    let finished = run_finite_demand_with_progress(request.clone(), &cancel, |event| {
        if event["event"] == "finite_backend_native" {
            assert_eq!(event["native"]["workers"], 150);
            saw_native.store(true, Ordering::Release);
        }
    })
    .unwrap();
    assert!(saw_native.load(Ordering::Acquire));
    assert!(finished.complete);
    assert_eq!(finished.document["committed"]["input_occurrences"], 3);
    assert_eq!(
        fs::read(request.directory.join("execution.json")).unwrap(),
        execution
    );

    append(&mut request);
    let extended = run_finite_demand_with_progress(request.clone(), &cancel, |_| {}).unwrap();
    assert!(extended.complete);
    assert_eq!(extended.document["committed"]["input_occurrences"], 7);
    assert_eq!(
        fs::read(request.directory.join("execution.json")).unwrap(),
        execution
    );

    // Independent serial replay of exactly the same source/amendment must have
    // identical mathematical results and the same execution identity.
    let mut serial = request;
    serial.directory = f.directory.join("finite-serial-control");
    serial.mode = FiniteDemandMode::Create;
    set_workers(&mut serial, 1);
    let storage_limits = serial.policy.native().storage;
    let reference = run_finite_demand_with_progress(serial, &cancel, |_| {}).unwrap();
    assert!(reference.complete);
    assert_eq!(
        extended.document["committed"],
        reference.document["committed"]
    );
    // An append adds a metadata-only generation; compare execution and the
    // completed Work/census through inspection rather than generation numbers.
    let resumed = inspect_finite_demand(&f.directory.join("finite")).unwrap();
    let serial = inspect_finite_demand(&f.directory.join("finite-serial-control")).unwrap();
    assert_eq!(
        fs::read(f.directory.join("finite-serial-control/execution.json")).unwrap(),
        execution
    );
    assert_eq!(resumed["committed"], serial["committed"]);
    use crate::application::routed_campaign::finite_demand::storage::{
        RawExecutionBinding, ValidatedGeneration,
    };
    let execution_value: Value = serde_json::from_slice(&execution).unwrap();
    let binding = RawExecutionBinding {
        physical_arity: serde_json::from_value(execution_value["physical_arity"].clone()).unwrap(),
        program_identity: serde_json::from_value(execution_value["program_identity"].clone())
            .unwrap(),
    };
    let resumed = ValidatedGeneration::open_generated(
        &f.directory.join("finite/store"),
        &binding,
        storage_limits,
    )
    .unwrap();
    let serial = ValidatedGeneration::open_generated(
        &f.directory.join("finite-serial-control/store"),
        &binding,
        storage_limits,
    )
    .unwrap();
    assert_eq!(resumed.state(), serial.state());
}

#[test]
fn public_finite_cancel_resume_append_and_owner_load_free_status() {
    let f = Fixture::new();
    let mut request = request(&f);
    let old = f.directory.join("old-checkpoint");
    fs::write(&old, b"inert source checkpoint, not CP6").unwrap();
    let mut checkpoint = OwnerDomainWalkCheckpointOptions::new(&old);
    checkpoint.resume = true;
    request.source.checkpoint = Some(checkpoint);
    let cancel = AtomicBool::new(false);
    let reported_totals = std::cell::RefCell::new(Vec::new());
    let error = run_finite_demand_with_progress(request.clone(), &cancel, |event| {
        if let Some(total) = event.get("input_occurrences_total") {
            reported_totals.borrow_mut().push(total.clone());
        }
        if event["event"] == "finite_checkpoint_saved" && event["generation_number"] == 2 {
            cancel.store(true, Ordering::Release);
        }
    })
    .unwrap_err();
    assert_eq!(error.kind(), crate::AppErrorKind::Cancelled);
    assert_eq!(*reported_totals.borrow(), vec![json!("3")]);
    let current = fs::read(request.directory.join("store/CURRENT")).unwrap();
    fs::rename(
        f.directory.join("owner.rrbin"),
        f.directory.join("owner-unavailable.rrbin"),
    )
    .unwrap();
    let paused = inspect_finite_demand(&request.directory).unwrap();
    assert_eq!(paused["generation"]["number"], 2);
    assert_eq!(paused["committed"]["input_occurrences"], 1);
    assert_eq!(paused["committed"]["committed_windows"], 1);
    assert_eq!(paused["committed"]["exhausted_required_rows"], 1); // Empty declaration retained.
    assert_eq!(paused["source"]["status"], "bound");
    fs::rename(
        f.directory.join("owner-unavailable.rrbin"),
        f.directory.join("owner.rrbin"),
    )
    .unwrap();
    assert_eq!(
        fs::read(request.directory.join("store/CURRENT")).unwrap(),
        current
    );
    cancel.store(false, Ordering::Release);
    request.mode = FiniteDemandMode::Resume;
    let finished = run_finite_demand_with_progress(request.clone(), &cancel, |event| {
        if let Some(total) = event.get("input_occurrences_total") {
            reported_totals.borrow_mut().push(total.clone());
        }
    })
    .unwrap();
    assert!(finished.complete);
    assert_eq!(finished.document["committed"]["input_occurrences"], 3);
    assert_eq!(finished.document["committed"]["committed_windows"], 3);
    assert_eq!(finished.document["input_occurrences_total"], "3");
    append(&mut request);
    let extended = run_finite_demand_with_progress(request.clone(), &cancel, |event| {
        if let Some(total) = event.get("input_occurrences_total") {
            reported_totals.borrow_mut().push(total.clone());
        }
    })
    .unwrap();
    // The appended four inputs overlap all three original inputs. The total
    // counts seven request occurrences, not four unique integrals.
    assert_eq!(
        *reported_totals.borrow(),
        vec![json!("3"), json!("3"), json!("7")]
    );
    assert_eq!(extended.document["input_occurrences_total"], "7");
    assert_eq!(extended.document["committed"]["input_occurrences"], 7);
    assert_eq!(extended.document["committed"]["committed_windows"], 7);
    let status = inspect_finite_demand(&request.directory).unwrap();
    assert_eq!(status["committed"]["required_rows"], 3);
    assert_eq!(status["source"]["amendments"][0]["first_input"], 2);
    assert_eq!(status["source"]["amendments"][0]["queries"], 1);
    assert_eq!(
        status["source"]["amendments"][0]["digest"],
        request.source.amendments[0].digest()
    );
    assert_eq!(fs::read(old).unwrap(), b"inert source checkpoint, not CP6");
}

#[test]
fn public_finite_inspection_rejects_mutation_and_repairs_missing_initial_control_after_resume() {
    let f = Fixture::new();
    let mut request = request(&f);
    let cancel = AtomicBool::new(false);
    let fault = AtomicBool::new(false);
    let error = run_finite_demand_with_progress(request.clone(), &cancel, |event| {
        if event["event"] == "preparation" && !fault.swap(true, Ordering::AcqRel) {
            fs::create_dir(request.directory.join("execution.json")).unwrap();
        }
    })
    .unwrap_err();
    assert!(fault.load(Ordering::Acquire));
    assert!(error.to_string().contains("is durable"), "{error}");
    assert!(request.directory.join("store/CURRENT").is_file());
    assert!(inspect_finite_demand(&request.directory).is_err());
    fs::remove_dir(request.directory.join("execution.json")).unwrap(); // Only the injected fixture fault.
    request.mode = FiniteDemandMode::Resume;
    let repaired = run_finite_demand_with_progress(request.clone(), &cancel, |_| {}).unwrap();
    assert!(repaired.complete);
    let marker = request.directory.join("active-source.json");
    let original = fs::read(&marker).unwrap();
    let mut altered: Value = serde_json::from_slice(&original).unwrap();
    altered["source"]["total_queries"] = json!(999);
    fs::write(&marker, serde_json::to_vec(&altered).unwrap()).unwrap();
    assert!(inspect_finite_demand(&request.directory).is_err());
    fs::write(&marker, &original).unwrap();
    altered = serde_json::from_slice(&original).unwrap();
    altered["join"]["scope"] = json!("00".repeat(32));
    fs::write(&marker, serde_json::to_vec(&altered).unwrap()).unwrap();
    let stale = inspect_finite_demand(&request.directory).unwrap();
    assert_eq!(stale["source"]["status"], "stale");
    assert_eq!(stale["committed"]["input_occurrences"], 3);
    let before = fs::read(request.directory.join("store/CURRENT")).unwrap();
    run_finite_demand_with_progress(request.clone(), &cancel, |_| {}).unwrap();
    assert_eq!(
        fs::read(request.directory.join("store/CURRENT")).unwrap(),
        before
    );
    assert_eq!(
        inspect_finite_demand(&request.directory).unwrap()["source"]["status"],
        "bound"
    );
    fs::remove_file(request.directory.join("store/CURRENT")).unwrap();
    assert!(run_finite_demand_with_progress(request.clone(), &cancel, |_| {}).is_err());
    request.mode = FiniteDemandMode::Create;
    assert!(run_finite_demand_with_progress(request.clone(), &cancel, |_| {}).is_err());
    assert!(!request.directory.join("store/CURRENT").exists());
}

#[test]
fn public_finite_namespace_relocation_and_partition_mismatch_preserve_work() {
    let mut f = Fixture::new();
    let mut request = request(&f);
    let cancel = AtomicBool::new(false);
    run_finite_demand_with_progress(request.clone(), &cancel, |_| {}).unwrap();
    let before = fs::read(request.directory.join("store/CURRENT")).unwrap();
    let mut changed: Value = serde_json::from_slice(&request.policy.to_json().unwrap()).unwrap();
    changed["windows"]["max_seed_points"] = json!(2);
    let mut invalid = request.clone();
    invalid.mode = FiniteDemandMode::Resume;
    invalid.policy = FiniteDemandPolicy::from_json(&serde_json::to_vec(&changed).unwrap()).unwrap();
    assert!(run_finite_demand_with_progress(invalid, &cancel, |_| {}).is_err());
    assert_eq!(
        fs::read(request.directory.join("store/CURRENT")).unwrap(),
        before
    );
    let new = f.directory.with_extension("relocated");
    fs::rename(&f.directory, &new).unwrap();
    f.directory = new;
    request.directory = f.directory.join("finite");
    request.source.matching.owner_base = f.directory.clone();
    request.mode = FiniteDemandMode::Resume;
    assert_eq!(
        inspect_finite_demand(&request.directory).unwrap()["committed"]["input_occurrences"],
        3
    );
    run_finite_demand_with_progress(request.clone(), &cancel, |_| {}).unwrap();
    assert_eq!(
        fs::read(request.directory.join("store/CURRENT")).unwrap(),
        before
    );
    let launch: Value =
        serde_json::from_slice(&fs::read(request.directory.join("launch.json")).unwrap()).unwrap();
    assert_eq!(
        Path::new(launch["owner_base"].as_str().unwrap()),
        Path::new("..")
    );
}

#[test]
fn public_finite_preparation_cancel_requires_explicit_no_store_repair() {
    let f = Fixture::new();
    let mut request = request(&f);
    let cancel = AtomicBool::new(false);
    let error = run_finite_demand_with_progress(request.clone(), &cancel, |event| {
        if event["event"] == "preparation" {
            cancel.store(true, Ordering::Release);
        }
    })
    .unwrap_err();
    assert_eq!(error.kind(), crate::AppErrorKind::Cancelled);
    assert!(!request.directory.join("store").exists());
    let status = inspect_finite_demand(&request.directory).unwrap();
    assert_eq!(status["status"], "preparing");
    assert!(status["generation"].is_null());
    assert!(status["committed"].is_null());
    assert_eq!(status["repair_preparation_possible"], true);
    cancel.store(false, Ordering::Release);
    request.mode = FiniteDemandMode::Resume;
    let events = AtomicU64::new(0);
    let error = run_finite_demand_with_progress(request.clone(), &cancel, |_| {
        events.fetch_add(1, Ordering::Relaxed);
    })
    .unwrap_err();
    assert!(error.to_string().contains("--repair-preparation"));
    assert_eq!(events.load(Ordering::Relaxed), 0);
    request.mode = FiniteDemandMode::RepairPreparation;
    assert!(
        run_finite_demand_with_progress(request.clone(), &cancel, |_| {})
            .unwrap()
            .complete
    );
    let before = fs::read(request.directory.join("store/CURRENT")).unwrap();
    assert!(run_finite_demand_with_progress(request.clone(), &cancel, |_| {}).is_err());
    fs::remove_file(request.directory.join("store/CURRENT")).unwrap();
    assert!(run_finite_demand_with_progress(request, &cancel, |_| {}).is_err());
    assert!(!before.is_empty());
    assert!(!f.directory.join("finite/store/CURRENT").exists());
}

#[test]
fn public_finite_publish_refine_extend_republish_keeps_source_and_exact_entry_union() {
    let f = Fixture::new();
    let mut request = request(&f);
    let cancel = AtomicBool::new(false);
    run_finite_demand_with_progress(request.clone(), &cancel, |_| {}).unwrap();
    request.mode = FiniteDemandMode::Resume;
    let published = f.directory.join("published");
    let mut options =
        crate::MasterReductionOptions::new(request.directory.join("store"), &published);
    options.operation = crate::MasterReductionOperation::Publish;
    let first =
        crate::publish_finite_demand_with_progress(request.clone(), &options, &cancel, |_| {})
            .unwrap();
    assert_eq!(first["schema"], "rustred.master-reduction.v3");
    let first_bytes = fs::read(published.join("artifact.json")).unwrap();
    let refined = f.directory.join("refined");
    let mut refine = crate::MasterReductionOptions::new("unused", &refined);
    refine.seed_depth = 1;
    let paused = crate::master_refine_published_artifact(&published, &refine, &cancel, |event| {
        if event["event"] == "master_reduction_checkpoint" {
            cancel.store(true, Ordering::Release);
        }
    })
    .unwrap();
    assert_eq!(paused["status"], "paused");
    cancel.store(false, Ordering::Release);
    refine.resume = true;
    let done =
        crate::master_refine_published_artifact(&published, &refine, &cancel, |_| {}).unwrap();
    assert_eq!(done["status"], "completed_nonminimal");
    let previous = crate::load_master_relation_session(&refined).unwrap();
    assert!(previous.statistics().completed_source_rows > 0);
    let key = rustred::family::IntegralKey::try_new([1]).unwrap();
    let original_map = previous.apply_terminal(&key).unwrap();
    request.source.amendments.push(crate::OwnerDomainWalkAmendment {path:"amendment-0001.json".into(),text:json!({
        "schema":crate::OWNER_DOMAIN_WALK_SCOPE_EXTENSION_SCHEMA,"sequence":1,"parent":request.source.checkpoint_binding(),
        "queries":[{"id":"only-five","owner":"1","lower":[4],"upper":[4],"max_numerator_rank":0,
            "power_bounds":{"max_positive_power":5,"min_power_difference":5,"max_power_difference":5}}],
        "query_roles":{"required":["only-five"],"auxiliary":[]}}).to_string()});
    let before = fs::read(request.directory.join("store/CURRENT")).unwrap();
    let republished = f.directory.join("republished");
    options.directory = republished.clone();
    options.previous_artifact = Some(refined.clone());
    assert!(
        crate::publish_finite_demand_with_progress(request.clone(), &options, &cancel, |_| {})
            .is_err()
    );
    assert_eq!(
        fs::read(request.directory.join("store/CURRENT")).unwrap(),
        before
    );
    assert!(!republished.exists());
    let pending = inspect_finite_demand(&request.directory).unwrap();
    assert_eq!(pending["committed"]["required_rows"], 2);
    assert_eq!(pending["requested"]["required_rows"], 3);
    run_finite_demand_with_progress(request.clone(), &cancel, |_| {}).unwrap();
    let next =
        crate::publish_finite_demand_with_progress(request.clone(), &options, &cancel, |_| {})
            .unwrap();
    assert_eq!(
        next["inherited_finite_sources"].as_array().unwrap().len(),
        1
    );
    let after = crate::load_master_relation_session(&republished).unwrap();
    assert!(
        after.statistics().completed_source_rows >= previous.statistics().completed_source_rows
    );
    assert_eq!(after.apply_terminal(&key).unwrap(), original_map);
    assert_eq!(
        fs::read(published.join("artifact.json")).unwrap(),
        first_bytes
    );
    let runtime = f.directory.join("runtime");
    crate::export_saved_application::<4>(&republished, &runtime, Default::default()).unwrap();
    let mut app = crate::load_saved_application::<4>(&runtime, Default::default()).unwrap();
    let one = app.family().coefficient_context().one();
    let first = app
        .reduce_linear_combination([(
            rustred::family::IntegralKey::try_new([5]).unwrap(),
            one.clone(),
        )])
        .unwrap();
    assert!(!first.terms().is_empty());
    let warm = app
        .reduce_linear_combination([(
            rustred::family::IntegralKey::try_new([5]).unwrap(),
            one.clone(),
        )])
        .unwrap();
    assert_eq!(first.terms(), warm.terms());
    assert_eq!(first.nonzero_conditions(), warm.nonzero_conditions());
    assert_eq!(warm.statistics().work.rule_applications(), 0);
    assert!(
        app.reduce_linear_combination([(rustred::family::IntegralKey::try_new([4]).unwrap(), one)])
            .is_err()
    );
}
