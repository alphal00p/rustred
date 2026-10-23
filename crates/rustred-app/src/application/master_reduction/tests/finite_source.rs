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
        max_windows_per_batch: 1,
        reuse_worker_pool: true,
        parallel_window_preparation: false,
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
        operation_limits: None,
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

#[cfg(feature = "reconstruction")]
#[test]
fn direct_modular_refinement_rechecks_the_same_strategy_bound_source_identity() {
    let scratch = Scratch::new();
    let request = source(&scratch);
    let native = capture(&request, &scratch.0.join("finite"));
    let output = scratch.0.join("modular");
    let mut options = MasterReductionOptions::new("unused-cp6", &output);
    options.refinement_strategy = Some(MasterRefinementStrategy::ModularCircuits);
    let report =
        master_reduce_finite_demand(&native, &options, &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(report["status"], "completed_nonminimal");
    assert_eq!(report["refinement_strategy"], "modular-circuits");
    assert_eq!(report["modular_search_complete"], true);
    assert!(load_master_reduction(&output).is_ok());
}

fn publication_policy(runs: usize) -> crate::FiniteDemandPolicy {
    let mut value: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/input/finite_campaign/high_memory.json"
    )))
    .unwrap();
    value["storage"]["raw"]["max_runs"] = json!(runs);
    value["storage"]["raw"]["max_merge_runs"] = json!(runs * 2);
    if value["storage"]["max_transient_bytes"].is_null() {
        value["storage"]["max_transient_bytes"] = json!(570_000_000_000u64);
    }
    crate::FiniteDemandPolicy::from_json(&serde_json::to_vec(&value).unwrap()).unwrap()
}

fn public_resident_request(scratch: &Scratch, name: &str) -> crate::FiniteDemandRunRequest {
    let mut policy = publication_policy(100);
    policy.workers = 1;
    policy.windows.max_seed_points = 1;
    policy.storage.max_page_records = 2;
    crate::FiniteDemandRunRequest {
        source: source(scratch),
        policy,
        directory: scratch.0.join(name),
        mode: crate::FiniteDemandMode::Create,
    }
}

#[test]
fn operational_planning_allowance_survives_publication_and_cold_inspection() {
    let scratch = Scratch::new();
    let mut request = public_resident_request(&scratch, "planning-publication");
    // The source still binds its old one-layer work quota, not a smaller
    // mathematical domain. The separate execution policy permits exact counts.
    request.policy.windows.max_positive_layers = 1;
    request.policy.windows.max_partition_checks = 0;
    let cancel = AtomicBool::new(false);
    let solved = crate::run_finite_demand_with_progress(request.clone(), &cancel, |_| {}).unwrap();
    assert!(solved.complete);
    request.mode = crate::FiniteDemandMode::Resume;
    let directory = scratch.0.join("planning-artifact");
    let mut options = MasterReductionOptions::new(request.directory.join("store"), &directory);
    options.operation = MasterReductionOperation::Publish;
    let report =
        crate::publish_finite_demand_with_progress(request.clone(), &options, &cancel, |_| {})
            .unwrap();
    assert_eq!(report["status"], "published_unrefined");
    assert_eq!(
        master_reduction_inspect_with_policy(&directory, &request.policy).unwrap()["status"],
        "published_unrefined"
    );
    let mut capped = request.policy;
    capped.operation_limits = None;
    assert!(master_reduction_inspect_with_policy(&directory, &capped).is_err());
}

#[test]
fn public_finite_publication_skips_work_hydration_preserves_policy_and_exact_package() {
    use std::cell::RefCell;
    let scratch = Scratch::new();
    let mut request = public_resident_request(&scratch, "public-high-resident");
    let grant = request.policy.max_resident_work_bytes;
    assert_eq!(grant, 300_000_000_000);
    let cancel = AtomicBool::new(false);
    let solve_events = RefCell::new(Vec::new());
    let solved = crate::run_finite_demand_with_progress(request.clone(), &cancel, |event| {
        solve_events.borrow_mut().push(event)
    })
    .unwrap();
    assert!(solved.complete);
    assert!(
        solve_events
            .borrow()
            .iter()
            .any(|event| event["event"] == "finite_backend_resident_hydrated"
                && event["status"] == "active")
    );
    request.mode = crate::FiniteDemandMode::Resume;
    let current_path = request.directory.join("store/CURRENT");
    let current = fs::read(&current_path).unwrap();
    let active_path = request.directory.join("active-source.json");
    let active = fs::read(&active_path).unwrap();
    let before: Value = serde_json::from_slice(&active).unwrap();
    assert_eq!(before["policy"]["max_resident_work_bytes"], grant);

    let mut publications = Vec::new();
    let mut work_pages = Vec::new();
    for unavailable_work in [false, true] {
        if unavailable_work {
            // Corrupt every Work payload while retaining its framing length
            // and metadata. Any accidental page read/hydration must fail.
            // Raw census files remain untouched and still undergo validation.
            for entry in fs::read_dir(request.directory.join("store/pages")).unwrap() {
                let path = entry.unwrap().path();
                let bytes = fs::read(&path).unwrap();
                let mut corrupt = bytes.clone();
                corrupt[0] ^= 0xff;
                fs::write(&path, corrupt).unwrap();
                work_pages.push((path, bytes));
            }
            assert!(!work_pages.is_empty());
        }
        let directory = scratch.0.join(format!("published-{unavailable_work}"));
        let mut options = MasterReductionOptions::new(request.directory.join("store"), &directory);
        options.operation = MasterReductionOperation::Publish;
        let events = RefCell::new(Vec::new());
        let report = crate::publish_finite_demand_with_progress(
            request.clone(),
            &options,
            &cancel,
            |event| events.borrow_mut().push(event),
        )
        .unwrap();
        assert_eq!(report["status"], "published_unrefined");
        assert_eq!(fs::read(&current_path).unwrap(), current);
        assert_eq!(fs::read(&active_path).unwrap(), active);
        let events = events.borrow();
        let skipped = events
            .iter()
            .find(|event| event["event"] == "finite_publication_work_cache_skipped")
            .unwrap();
        assert_eq!(skipped["requested_resident_work_bytes"], grant);
        assert_eq!(skipped["effective_resident_work_bytes"], 0);
        assert_eq!(skipped["saved_solving_policy_unchanged"], true);
        let hydration = events
            .iter()
            .find(|event| event["event"] == "finite_backend_resident_hydrated")
            .unwrap();
        assert_eq!(hydration["status"], "disabled");
        assert_eq!(hydration["requested_bytes"], 0);
        assert!(hydration["resident"].is_null());
        assert!(
            events
                .iter()
                .any(|event| event["event"] == "finite_cleanup_finished"
                    && event["reason"] == "publication_capture")
        );
        publications.push((directory, report));
    }
    // Ordinary solving still validates/hydrates its requested resident index.
    assert!(crate::run_finite_demand_with_progress(request.clone(), &cancel, |_| {}).is_err());
    for (path, bytes) in work_pages {
        fs::write(path, bytes).unwrap();
    }

    for field in [
        "raw_terminals",
        "normalized_terminals",
        "remaining_terminals",
        "published_rule_records",
        "observed_cover_rules",
        "inventory",
    ] {
        assert_eq!(
            publications[0].1[field], publications[1].1[field],
            "{field}"
        );
    }
    let mut expected = None;
    for (index, (published, _)) in publications.iter().enumerate() {
        let package = scratch.0.join(format!("runtime-{index}"));
        crate::export_saved_application::<4>(published, &package, Default::default()).unwrap();
        let mut loaded = crate::load_saved_application::<4>(&package, Default::default()).unwrap();
        let one = loaded.family().coefficient_context().one();
        let result = loaded
            .reduce_linear_combination([(rustred::family::IntegralKey::try_new([3]).unwrap(), one)])
            .unwrap();
        let exact = (result.terms().clone(), result.nonzero_conditions().to_vec());
        if let Some(expected) = &expected {
            assert_eq!(&exact, expected);
        } else {
            expected = Some(exact);
        }
    }
}

#[test]
fn public_finite_publication_without_work_cache_still_rejects_corrupt_census_and_pending_scope() {
    let scratch = Scratch::new();
    let mut request = public_resident_request(&scratch, "public-census-guard");
    let cancel = AtomicBool::new(false);
    crate::run_finite_demand_with_progress(request.clone(), &cancel, |_| {}).unwrap();
    request.mode = crate::FiniteDemandMode::Resume;
    let current_path = request.directory.join("store/CURRENT");
    let current = fs::read(&current_path).unwrap();
    let mut raw_pages = Vec::new();
    for entry in fs::read_dir(request.directory.join("store/raw")).unwrap() {
        let path = entry.unwrap().path();
        let bytes = fs::read(&path).unwrap();
        let mut corrupted = bytes.clone();
        corrupted[0] ^= 0xff;
        fs::write(&path, corrupted).unwrap();
        raw_pages.push((path, bytes));
    }
    assert!(!raw_pages.is_empty());
    let bad_output = scratch.0.join("must-reject-corrupt-census");
    let mut options = MasterReductionOptions::new(request.directory.join("store"), &bad_output);
    options.operation = MasterReductionOperation::Publish;
    assert!(
        crate::publish_finite_demand_with_progress(request.clone(), &options, &cancel, |_| {})
            .is_err()
    );
    assert!(!bad_output.join("artifact.json").exists());
    assert_eq!(fs::read(&current_path).unwrap(), current);
    for (path, bytes) in raw_pages {
        fs::write(path, bytes).unwrap();
    }

    // A complete old prefix is not permission to publish an appended request.
    request
        .source
        .amendments
        .push(crate::OwnerDomainWalkAmendment {
            path: "extra-power.json".into(),
            text: json!({
            "schema":crate::OWNER_DOMAIN_WALK_SCOPE_EXTENSION_SCHEMA,"sequence":1,
            "parent":request.source.checkpoint_binding(),
            "queries":[{"id":"new-four","owner":"1","lower":[3],"upper":[3],
                "max_numerator_rank":0,"power_bounds":{"max_positive_power":4,
                "min_power_difference":4,"max_power_difference":4}}],
            "query_roles":{"required":["new-four"],"auxiliary":[]}})
            .to_string(),
        });
    options.directory = scratch.0.join("must-reject-pending-scope");
    assert!(
        crate::publish_finite_demand_with_progress(request, &options, &cancel, |_| {}).is_err()
    );
    assert!(!options.directory.join("artifact.json").exists());
    assert_eq!(fs::read(&current_path).unwrap(), current);
}

#[test]
fn public_finite_cleanup_is_visible_cancellable_and_keeps_completed_checkpoint_reusable() {
    use std::cell::RefCell;
    let scratch = Scratch::new();
    let mut request = public_resident_request(&scratch, "public-cleanup");
    let cancel = AtomicBool::new(false);
    let events = RefCell::new(Vec::new());
    let error = crate::run_finite_demand_with_progress(request.clone(), &cancel, |event| {
        if event["event"] == "finite_cleanup_started" {
            assert_eq!(event["reason"], "solve_complete");
            assert_eq!(event["committed"]["required_complete"], true);
            assert_eq!(event["publication_claim"], false);
            cancel.store(true, Ordering::Release);
        }
        events.borrow_mut().push(event);
    })
    .unwrap_err();
    assert_eq!(error.kind(), crate::AppErrorKind::Cancelled);
    let finished = events
        .borrow()
        .iter()
        .find(|event| event["event"] == "finite_cleanup_finished")
        .unwrap()
        .clone();
    assert!(finished["duration_seconds"].as_f64().unwrap() >= 0.0);
    let status = crate::inspect_finite_demand(&request.directory).unwrap();
    assert_eq!(status["committed"]["all_rows_exhausted"], true);
    let current_path = request.directory.join("store/CURRENT");
    let current = fs::read(&current_path).unwrap();
    cancel.store(false, Ordering::Release);
    request.mode = crate::FiniteDemandMode::Resume;
    let resumed = crate::run_finite_demand_with_progress(request.clone(), &cancel, |_| {}).unwrap();
    assert!(resumed.complete);
    assert_eq!(fs::read(&current_path).unwrap(), current);

    let published = scratch.0.join("after-cleanup");
    let mut options = MasterReductionOptions::new(request.directory.join("store"), &published);
    options.operation = MasterReductionOperation::Publish;
    let error =
        crate::publish_finite_demand_with_progress(request.clone(), &options, &cancel, |event| {
            if event["event"] == "finite_cleanup_started"
                && event["reason"] == "publication_capture"
            {
                cancel.store(true, Ordering::Release);
            }
        })
        .unwrap_err();
    assert_eq!(error.kind(), crate::AppErrorKind::Cancelled);
    assert!(!published.join("artifact.json").exists());
    assert_eq!(fs::read(&current_path).unwrap(), current);
    cancel.store(false, Ordering::Release);
    let report =
        crate::publish_finite_demand_with_progress(request, &options, &cancel, |_| {}).unwrap();
    assert_eq!(report["status"], "published_unrefined");
}

#[test]
fn caller_policy_crosses_4096_runs_through_publication_refinement_and_cold_load() {
    let scratch = Scratch::new();
    let mut request = source(&scratch);
    let mut queries: Value = serde_json::from_str(&request.matching.queries_json).unwrap();
    queries["queries"][0]["lower"] = json!([9999]);
    queries["queries"][0]["upper"] = json!([9999]);
    queries["queries"][0]["power_bounds"] = json!({
        "max_positive_power":12000,"min_power_difference":1,"max_power_difference":12000
    });
    request.matching.queries_json = queries.to_string();
    let backend = scratch.0.join("backend");
    let mut native_policy = policy();
    native_policy.windows.max_positive_layers = 20_000;
    native_policy.windows.max_partition_checks = 100_000;
    native_policy.per_batch_work.max_nodes = 30_000;
    native_policy.per_batch_work.max_rule_applications = 30_000;
    native_policy.per_batch_work.max_transport_calls = 30_000;
    native_policy.export.max_records = 30_000;
    native_policy.export.max_live_payload_bytes = 8 << 20;
    native_policy.storage.max_delta_records = 30_000;
    native_policy.storage.max_delta_payload_bytes = 8 << 20;
    native_policy.storage.max_page_records = 512;
    let mut session = Session::<4>::create(
        &request,
        native_policy,
        &backend,
        &AtomicBool::new(false),
        &|_| {},
    )
    .unwrap();
    let captured = finish(&mut session);
    drop(session);
    assert!(captured.raw_provenance().state().completed_work >= 8194);
    let captured = captured.with_repeated_raw_runs_for_test(4097);
    // One real native chain supplies the completed Work commitment. Repeating
    // typed raw page references preserves its exact union while exercising a
    // large merge directory without thousands of separate commits or files.
    let original_current = fs::read(backend.join("CURRENT")).unwrap();
    let published = scratch.0.join("published");
    let mut options = MasterReductionOptions::new(&backend, &published);
    options.operation = MasterReductionOperation::Publish;
    options.finite_policy = Some(publication_policy(4096));
    assert_eq!(
        master_reduce_finite_demand(&captured, &options, &AtomicBool::new(false), |_| {})
            .unwrap_err()
            .kind(),
        crate::AppErrorKind::Limit
    );
    assert!(!published.join("latest.json").exists());
    assert_eq!(fs::read(backend.join("CURRENT")).unwrap(), original_current);

    let raised = publication_policy(8194);
    options.finite_policy = Some(raised.clone());
    let report =
        master_reduce_finite_demand(&captured, &options, &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(report["status"], "published_unrefined");
    let latest = fs::read(published.join("latest.json")).unwrap();
    let artifact = fs::read(published.join("artifact.json")).unwrap();
    let lower = publication_policy(4096);
    assert_eq!(
        master_reduction_inspect_with_policy(&published, &lower)
            .unwrap_err()
            .kind(),
        crate::AppErrorKind::Limit
    );
    assert!(load_master_relation_session_with_policy(&published, &lower).is_err());
    let mut lower_merge = raised.clone();
    lower_merge.storage.raw.max_merge_runs = 4096;
    let error = master_reduction_inspect_with_policy(&published, &lower_merge).unwrap_err();
    assert_eq!(error.kind(), crate::AppErrorKind::Limit);
    assert!(error.to_string().contains("merge fan-in"));
    options.resume = true;
    options.finite_policy = Some(lower);
    assert!(
        master_reduce_finite_demand(&captured, &options, &AtomicBool::new(false), |_| {}).is_err()
    );
    assert_eq!(fs::read(published.join("latest.json")).unwrap(), latest);
    assert_eq!(fs::read(published.join("artifact.json")).unwrap(), artifact);
    assert_eq!(fs::read(backend.join("CURRENT")).unwrap(), original_current);

    options.finite_policy = Some(raised.clone());
    assert!(
        master_reduce_finite_demand(&captured, &options, &AtomicBool::new(false), |_| {}).is_ok()
    );
    assert_eq!(
        master_reduction_inspect_with_policy(&published, &raised).unwrap()["status"],
        "published_unrefined"
    );
    assert_eq!(
        load_master_reduction_with_policy(&published, &raised)
            .unwrap()
            .primary_session()
            .raw_terminals(),
        captured.terminal_keys()
    );
    assert!(master_reduction_inspect(&published).is_ok());
    assert!(load_master_relation_session(&published).is_ok());
    let refined = scratch.0.join("refined");
    let mut refinement = MasterReductionOptions::new("unused", &refined);
    refinement.finite_policy = Some(raised.clone());
    master_refine_published_artifact(&published, &refinement, &AtomicBool::new(false), |_| {})
        .unwrap();
    assert!(load_master_reduction_with_policy(&refined, &raised).is_ok());
    assert!(
        crate::prepare_saved_application_with_policy::<4>(&published, Default::default(), &raised)
            .is_ok()
    );
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
    // CP6 emits a checkpoint-only summary with finalization=not_evaluated;
    // its legacy full-result Boolean is not a closure certificate. Require
    // actual independent cold closure before comparing the two publications.
    assert_eq!(
        walk.document["recursive_worklist_exhausted"], true,
        "{}",
        walk.document
    );
    assert_eq!(walk.document["failed_nodes"], 0, "{}", walk.document);
    assert_eq!(walk.document["frontiers"], 0, "{}", walk.document);
    let mut verify = crate::OwnerDomainWalkVerifyOptions::new(&checkpoint);
    verify.reinspect = crate::OwnerDomainWalkVerifyReinspect::All;
    verify.require_closure = true;
    verify.result = None;
    let closure =
        crate::owner_domain_walk_verify_closure(&control, &verify, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(closure["verdict"], "PASS", "{closure}");
    assert_eq!(closure["closure_required"], true, "{closure}");
    assert!(closure["roots_total"].as_u64().unwrap() > 0, "{closure}");
    assert_eq!(
        closure["roots_independently_verified"], closure["roots_total"],
        "{closure}"
    );
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
    finite_provenance::validate_session(&output, &report, &union, None).unwrap();
    assert_eq!(
        report["inventory"]["encountered"]["terminals"],
        captured.terminal_keys().len()
    );
}
