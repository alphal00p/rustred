use super::*;
use crate::application::routed_campaign::finite_demand::{
    backend::{
        Policy, Session,
        publication::{CaptureLimits, CompletedFiniteSource},
    },
    storage::StorageLimits,
    windows::WindowLimits,
};
use rustred::solver::{CandidateRoutedDeltaExportLimits, CandidateRoutedWorkBudget};
use std::fs;

const SOURCE: &str = r#"
schema="rustred.project.toml.v1"
[family]
name="finite_source_publication_tadpole"
loop_momenta=["q"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P"
expression="q^2-1"
[target]
powers=[1]
"#;
fn source(s: &Scratch) -> OwnerDomainWalkRequest {
    let mut generation = crate::FamilyCandidatesRequest::new(SOURCE);
    generation.max_numerator_rank = Some(0);
    let bundle = crate::family_candidates(generation).unwrap();
    let inspected =
        crate::inspect_generated_candidate_bundle(bundle.bundle(), Default::default()).unwrap();
    fs::write(s.0.join("owner.rrbin"), bundle.bundle()).unwrap();
    let selection = json!({"family_fingerprint":inspected.family_fingerprint,
        "owners":[{"path":"owner.rrbin","bytes":bundle.bundle().len(),"mask":"1"}],"initial_frontier_routes":[]});
    let queries = json!({"schema":"rustred.owner-domain-queries.json.v2","queries":[{"id":"original",
        "owner":"1","lower":[0],"upper":[2],"max_numerator_rank":0,
        "power_bounds":{"max_positive_power":3,"min_power_difference":1,"max_power_difference":3}}],
        "query_roles":{"required":["original"],"auxiliary":[]}});
    let mut request = OwnerDomainWalkRequest::new(crate::OwnerDomainMatchRequest::new(
        selection.to_string(),
        queries.to_string(),
    ));
    request.matching.owner_base = s.0.clone();
    request.publication_policy = crate::OwnerDomainWalkPublicationPolicy::Epoch;
    request.scheduling_policy = crate::OwnerDomainWalkSchedulingPolicy::TransferUnreserved {
        lookahead: std::num::NonZeroUsize::new(1).unwrap(),
    };
    request
}
fn policy() -> Policy {
    Policy {
        workers: 1,
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
        max_resident_work_bytes: 0,
        census: Default::default(),
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
            max_cached_pages: 8,
            max_cached_page_bytes: 1000000,
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
            max_transient_bytes: 128 << 20,
        },
        max_total_rows: 16,
        max_source_bytes: 1048576,
    }
}
fn finish(s: &mut Session<4>) -> CompletedFiniteSource {
    let cancel = AtomicBool::new(false);
    for _ in 0..20 {
        if s.step(&cancel, &|_| {})
            .unwrap()
            .progress
            .all_rows_exhausted
        {
            return s
                .capture_completed_source(
                    CaptureLimits {
                        max_terminal_keys: 100,
                        max_terminal_payload_bytes: 65536,
                    },
                    &cancel,
                )
                .unwrap();
        }
    }
    panic!("native finite publication fixture did not close");
}
fn capture(request: &OwnerDomainWalkRequest, path: &Path) -> CompletedFiniteSource {
    let mut s =
        Session::<4>::create(request, policy(), path, &AtomicBool::new(false), &|_| {}).unwrap();
    finish(&mut s)
}
fn publish(source: &CompletedFiniteSource, path: &Path, previous: Option<PathBuf>) -> Value {
    let mut options = MasterReductionOptions::new("unused-cp6", path);
    options.operation = MasterReductionOperation::Publish;
    options.previous_artifact = previous;
    master_reduce_finite_demand(source, &options, &AtomicBool::new(false), |_| {}).unwrap()
}

#[test]
fn finite_source_publishes_refines_resumes_extends_and_exports_exactly_like_walk() {
    let s = Scratch::new();
    let mut request = source(&s);
    let backend = s.0.join("backend");
    let captured = capture(&request, &backend);
    let published = s.0.join("published");
    let first = publish(&captured, &published, None);
    assert_eq!(first["schema"], FINITE_SCHEMA);
    assert_eq!(first["publication_assurance"], "trusted_generated");
    assert_eq!(first["independently_verified"], false);
    assert!(first["inventory_census_cache"].is_null());
    assert_eq!(
        first["inventory"]["encountered"]["terminals"],
        captured.terminal_keys().len()
    );

    let mut control = request.clone();
    let checkpoint = s.0.join("walk");
    control.checkpoint = Some(crate::OwnerDomainWalkCheckpointOptions::new(&checkpoint));
    let walk =
        crate::owner_domain_walk_with_progress(control.clone(), &AtomicBool::new(false), |_| {})
            .unwrap();
    assert!(walk.all_scheduled_domains_resolved);
    let control_publication = s.0.join("control-publication");
    let mut opts = MasterReductionOptions::new(&checkpoint, &control_publication);
    opts.operation = MasterReductionOperation::Publish;
    crate::master_reduce_saved_campaign(&control, &opts, &AtomicBool::new(false), |_| {}).unwrap();
    let mut reductions = Vec::new();
    for (name, publication) in [
        ("finite-runtime", &published),
        ("walk-runtime", &control_publication),
    ] {
        let runtime = s.0.join(name);
        crate::export_saved_application::<4>(publication, &runtime, Default::default()).unwrap();
        let mut app = crate::load_saved_application::<4>(&runtime, Default::default()).unwrap();
        let key = IntegralKey::try_new([3]).unwrap();
        let one = app.family().coefficient_context().one();
        let first = app
            .reduce_linear_combination([(key.clone(), one.clone())])
            .unwrap();
        let warm = app.reduce_linear_combination([(key, one)]).unwrap();
        assert_eq!(first.terms(), warm.terms());
        assert_eq!(first.nonzero_conditions(), warm.nonzero_conditions());
        assert_eq!(warm.statistics().work.rule_applications(), 0);
        reductions.push(first);
    }
    assert_eq!(reductions[0].terms(), reductions[1].terms());
    assert_eq!(
        reductions[0].nonzero_conditions(),
        reductions[1].nonzero_conditions()
    );

    let refined = s.0.join("refined");
    let cancel = AtomicBool::new(false);
    let mut refine = MasterReductionOptions::new("unused", &refined);
    refine.seed_depth = 1;
    let paused = crate::master_refine_published_artifact(&published, &refine, &cancel, |event| {
        if event["event"] == "master_reduction_checkpoint" {
            cancel.store(true, Ordering::Release);
        }
    })
    .unwrap();
    assert_eq!(paused["status"], "paused");
    assert_eq!(paused["schema"], FINITE_SCHEMA);
    cancel.store(false, Ordering::Release);
    refine.resume = true;
    let completed =
        crate::master_refine_published_artifact(&published, &refine, &cancel, |_| {}).unwrap();
    assert_eq!(completed["status"], "completed_nonminimal");
    assert_eq!(
        completed["finite_source"]["generation_blake3"],
        first["finite_source"]["generation_blake3"]
    );
    let refined_session = load_master_relation_session(&refined).unwrap();
    let prior_rows = refined_session.statistics().completed_source_rows;
    assert!(prior_rows > 0);
    let before = refined_session
        .apply_terminal(&IntegralKey::try_new([1]).unwrap())
        .unwrap();

    request.amendments.push(crate::OwnerDomainWalkAmendment {path:s.0.join("append.json"),text:json!({
        "schema":crate::OWNER_DOMAIN_WALK_SCOPE_EXTENSION_SCHEMA,"sequence":1,"parent":request.checkpoint_binding(),
        "queries":[{"id":"appended-singleton","owner":"1","lower":[4],"upper":[4],"max_numerator_rank":0,
            "power_bounds":{"max_positive_power":5,"min_power_difference":5,"max_power_difference":5}}],
        "query_roles":{"required":["appended-singleton"],"auxiliary":[]}}).to_string()});
    let mut extended =
        Session::<4>::resume(&request, policy(), &backend, &cancel, &|_| {}).unwrap();
    let next = finish(&mut extended);
    drop(extended);
    let republished = s.0.join("republished");
    let report = publish(&next, &republished, Some(refined.clone()));
    assert_eq!(report["schema"], FINITE_SCHEMA);
    assert_eq!(
        report["inherited_finite_sources"].as_array().unwrap().len(),
        1
    );
    let session = load_master_relation_session(&republished).unwrap();
    assert!(session.statistics().completed_source_rows >= prior_rows);
    assert_eq!(
        session
            .apply_terminal(&IntegralKey::try_new([1]).unwrap())
            .unwrap(),
        before
    );
    let runtime = s.0.join("portable-runtime");
    crate::export_saved_application::<4>(&republished, &runtime, Default::default()).unwrap();
    fs::rename(&backend, s.0.join("backend-held")).unwrap();
    fs::rename(&republished, s.0.join("republished-held")).unwrap();
    let mut app = crate::load_saved_application::<4>(&runtime, Default::default()).unwrap();
    let one = app.family().coefficient_context().one();
    assert!(
        app.reduce_linear_combination([(IntegralKey::try_new([5]).unwrap(), one.clone())])
            .is_ok()
    );
    // No rectangular hull between original1..3 and appended5.
    assert!(
        app.reduce_linear_combination([(IntegralKey::try_new([4]).unwrap(), one)])
            .is_err()
    );
}

#[test]
fn finite_source_rejects_contradictory_metadata_without_changing_native_generation() {
    let s = Scratch::new();
    let request = source(&s);
    let captured = capture(&request, &s.0.join("backend"));
    let published = s.0.join("published");
    let report = publish(&captured, &published, None);
    let manifest = published.join("artifact.json");
    let original_manifest = fs::read(&manifest).unwrap();
    let capsule = published.join(report["finite_source"]["capsule"].as_str().unwrap());
    let source_path = capsule.join("source.json");
    let original_source = fs::read(&source_path).unwrap();
    let selection = published.join("inputs/selection.json");
    let original_selection = fs::read(&selection).unwrap();
    let queries = published.join("inputs/queries.json");
    let original_queries = fs::read(&queries).unwrap();
    for mutation in 0..7 {
        let mut changed: Value = serde_json::from_slice(&original_manifest).unwrap();
        let mut metadata: Value = serde_json::from_slice(&original_source).unwrap();
        match mutation {
            0 => {
                changed.as_object_mut().unwrap().remove("finite_source");
            }
            1 => changed["schema"] = json!(SCHEMA),
            2 => metadata["window_policy"]["max_seed_points"] = json!(2),
            3 => metadata["request_binding"] = json!("changed-original-binding"),
            4 => {
                let mut q: Value =
                    serde_json::from_str(metadata["original_queries"].as_str().unwrap()).unwrap();
                q["queries"][0]["upper"] = json!([3]);
                metadata["original_queries"] = json!(q.to_string());
                fs::write(&queries, q.to_string()).unwrap();
                changed["inputs"]["queries_blake3"] =
                    json!(storage::digest_file(&queries).unwrap());
            }
            5 => {
                let mut q: Value = serde_json::from_slice(&original_selection).unwrap();
                q["owners"][0]["mask"] = json!("0");
                fs::write(&selection, q.to_string()).unwrap();
                changed["inputs"]["program_binding"] =
                    json!(storage::digest_file(&selection).unwrap());
            }
            _ => metadata["execution_identity"]["exact_algebra"] = json!("changed-policy"),
        }
        let bytes = serde_json::to_vec(&metadata).unwrap();
        fs::write(&source_path, &bytes).unwrap();
        if mutation >= 2 {
            changed["finite_source"]["source_bytes"] = json!(bytes.len());
            changed["finite_source"]["source_blake3"] =
                json!(blake3::hash(&bytes).to_hex().to_string());
        }
        fs::write(&manifest, serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(
            master_reduction_inspect(&manifest).is_err(),
            "mutation{mutation}"
        );
        let refused = s.0.join(format!("refused-{mutation}"));
        assert!(
            crate::export_saved_application::<4>(&manifest, &refused, Default::default()).is_err()
        );
        assert!(!refused.exists());
        fs::write(&manifest, &original_manifest).unwrap();
        fs::write(&source_path, &original_source).unwrap();
        fs::write(&selection, &original_selection).unwrap();
        fs::write(&queries, &original_queries).unwrap();
    }
    let marker = capsule.join("RAW_CURRENT");
    fs::rename(&marker, capsule.join("held-marker")).unwrap();
    assert!(load_master_relation_session(&manifest).is_err());
    fs::rename(capsule.join("held-marker"), &marker).unwrap();
    assert!(load_master_relation_session(&manifest).is_ok());
}

#[test]
fn finite_source_cancelled_staging_never_exposes_latest_and_raw_union_may_overcover() {
    let s = Scratch::new();
    let request = source(&s);
    let captured = capture(&request, &s.0.join("backend"));
    let output = s.0.join("cancelled-publication");
    let cancel = AtomicBool::new(false);
    let mut options = MasterReductionOptions::new("unused", &output);
    options.operation = MasterReductionOperation::Publish;
    assert!(
        master_reduce_finite_demand(&captured, &options, &cancel, |event| {
            if event["event"] == "finite_source_staged" {
                cancel.store(true, Ordering::Release);
            }
        })
        .is_err()
    );
    assert!(!output.join("latest.json").exists());
    assert!(!output.join("artifact.json").exists());
    cancel.store(false, Ordering::Release);
    let report = master_reduce_finite_demand(&captured, &options, &cancel, |_| {}).unwrap();
    let mut raw = captured.terminal_keys().clone();
    raw.insert(IntegralKey::try_new([2]).unwrap());
    let union =
        TerminalRelationSession::new(captured.family_owner().clone(), raw, 0, Default::default())
            .unwrap();
    assert!(union.raw_terminals().len() > captured.terminal_keys().len());
    finite_provenance::validate_session(&output, &report, &union).unwrap();
    assert_eq!(
        report["inventory"]["encountered"]["terminals"],
        captured.terminal_keys().len()
    );
}
