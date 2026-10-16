//! Public native lifecycle over the same saved-owner fixture, not a new solver.
use super::*;
use crate::{
    FiniteDemandMode, FiniteDemandPolicy, FiniteDemandRunRequest, inspect_finite_demand,
    run_finite_demand_with_progress,
};
use std::{fs, sync::atomic::AtomicBool};

fn policy() -> FiniteDemandPolicy {
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
fn public_finite_cancel_resume_append_and_owner_load_free_status() {
    let f = Fixture::new();
    let mut request = request(&f);
    let old = f.directory.join("old-checkpoint");
    fs::write(&old, b"inert source checkpoint, not CP6").unwrap();
    let mut checkpoint = OwnerDomainWalkCheckpointOptions::new(&old);
    checkpoint.resume = true;
    request.source.checkpoint = Some(checkpoint);
    let cancel = AtomicBool::new(false);
    let error = run_finite_demand_with_progress(request.clone(), &cancel, |event| {
        if event["event"] == "finite_checkpoint_saved" && event["generation_number"] == 2 {
            cancel.store(true, Ordering::Release);
        }
    })
    .unwrap_err();
    assert_eq!(error.kind(), crate::AppErrorKind::Cancelled);
    let current = fs::read(request.directory.join("store/CURRENT")).unwrap();
    fs::rename(
        f.directory.join("owner.rrbin"),
        f.directory.join("owner-unavailable.rrbin"),
    )
    .unwrap();
    let paused = inspect_finite_demand(&request.directory).unwrap();
    assert_eq!(paused["generation"]["number"], 2);
    assert_eq!(paused["committed"]["input_occurrences"], 1);
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
    let finished = run_finite_demand_with_progress(request.clone(), &cancel, |_| {}).unwrap();
    assert!(finished.complete);
    assert_eq!(finished.document["committed"]["input_occurrences"], 3);
    append(&mut request);
    let extended = run_finite_demand_with_progress(request.clone(), &cancel, |_| {}).unwrap();
    assert_eq!(extended.document["committed"]["input_occurrences"], 7);
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
    assert_eq!(launch["owner_base"], "..");
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
