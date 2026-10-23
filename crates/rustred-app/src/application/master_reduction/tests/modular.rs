use super::*;

/// Explicit scratch-only native source for exercising the public CLI.
#[test]
#[ignore = "set RUSTRED_MODULAR_CLI_FIXTURE_DIRECTORY to a new directory under repository TMP"]
fn write_modular_cli_fixture() {
    let directory = PathBuf::from(
        std::env::var_os("RUSTRED_MODULAR_CLI_FIXTURE_DIRECTORY")
            .expect("RUSTRED_MODULAR_CLI_FIXTURE_DIRECTORY is required"),
    );
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let directory = std::path::absolute(directory).unwrap();
    let tmp = repository.join("TMP");
    let parent = directory
        .parent()
        .expect("fixture directory needs a parent");
    assert!(
        parent
            .canonicalize()
            .unwrap()
            .starts_with(tmp.canonicalize().unwrap()),
        "CLI fixture output must remain under repository TMP"
    );
    std::fs::create_dir(&directory).expect("fixture output must be a new directory");
    let report = publish_fixture(&directory, &mut session());
    assert_eq!(report["status"], "published_unrefined");
    println!(
        "Authentic unrefined one-loop fixture: {}",
        directory.display()
    );
}

#[test]
fn partial_export_after_a_finished_modular_scan_still_reports_unfinished_refinement() {
    let mut report = json!({"operation":"publish","refinement_strategy":"modular-circuits",
        "modular_search_complete":true,"partial_refinement_source":{"status":"paused"},
        "collection":{"status":"completed"}});
    update_stats(&mut report, &session());
    assert_eq!(report["modular_search_complete"], true);
    assert_eq!(report["refinement_complete"], false);
    assert_eq!(report["finite_search_complete"], false);
}

#[test]
fn modular_refinement_publishes_exact_maps_without_advancing_exact_cursor() {
    let source = Scratch::new();
    publish_fixture(&source.0, &mut session());
    let before = std::fs::read(source.0.join("latest.json")).unwrap();
    let output = Scratch::new();
    let mut options = MasterReductionOptions::new("unused", &output.0);
    options.refinement_strategy = Some(MasterRefinementStrategy::ModularCircuits);
    options.finite_feedback = Some(false);
    options.modular_policy = Some(json!({"batch_rows":1,"reconstruction":{"max_primes":24}}));
    let initializing = AtomicBool::new(false);
    let report =
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |event| {
            if event["stage"] == "initializing_modular_image" {
                initializing.store(true, Ordering::Relaxed);
            }
        })
        .unwrap();
    assert!(initializing.load(Ordering::Relaxed));
    assert_eq!(report["status"], "completed_nonminimal");
    assert_eq!(report["refinement_strategy"], "modular-circuits");
    assert_eq!(report["relation_rows"], 0);
    assert_eq!(report["finite_search_complete"], false);
    assert_eq!(report["modular_search_complete"], true);
    assert!(
        report["completion_policy"]
            .as_str()
            .unwrap()
            .contains("heuristic modular")
    );
    assert_eq!(report["refinement_complete"], true);
    assert!(
        report["modular_refinement"]["accepted_relations"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(report["remaining_terminals"], 1);
    let loaded = load_master_reduction(&output.0).unwrap();
    assert_eq!(
        loaded.primary_session().statistics().completed_source_rows,
        0
    );
    assert!(!loaded.primary_session().is_complete());
    assert_eq!(std::fs::read(source.0.join("latest.json")).unwrap(), before);
    options.resume = true;
    options.modular_policy = None;
    options.refinement_strategy = None;
    master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {}).unwrap();
    options.modular_policy = Some(json!({"reconstruction":{"max_primes":32}}));
    let retried =
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(
        retried["modular_policy"]["reconstruction"]["max_primes"],
        32
    );
    assert!(load_master_reduction(&output.0).is_ok());
    options.modular_policy = None;
    options.refinement_strategy = Some(MasterRefinementStrategy::Exact);
    assert!(
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {})
            .is_err()
    );
}

#[test]
fn modular_sidecar_is_required_for_completed_artifact_and_exact_binding_is_unchanged() {
    let source = Scratch::new();
    publish_fixture(&source.0, &mut session());
    let exact = Scratch::new();
    let options = MasterReductionOptions::new("unused", &exact.0);
    let exact_report =
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {})
            .unwrap();
    let modular = Scratch::new();
    let mut options = MasterReductionOptions::new("unused", &modular.0);
    options.refinement_strategy = Some(MasterRefinementStrategy::ModularCircuits);
    let modular_report =
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_ne!(
        exact_report["refinement_binding"],
        modular_report["refinement_binding"]
    );
    let name = modular_report["modular_state"]["file"].as_str().unwrap();
    let path = modular.0.join(name);
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[0] ^= 1;
    std::fs::write(&path, bytes).unwrap();
    // Metadata-only inspection does not decode coefficients or circuit proofs.
    assert!(master_reduction_inspect(&modular.0).is_ok());
    assert!(load_master_reduction(&modular.0).is_err());
}

#[test]
fn paused_modular_scan_resumes_with_larger_operational_allowances() {
    let source = Scratch::new();
    publish_fixture(&source.0, &mut session());
    let output = Scratch::new();
    let mut options = MasterReductionOptions::new("unused", &output.0);
    options.refinement_strategy = Some(MasterRefinementStrategy::ModularCircuits);
    options.modular_policy = Some(json!({"batch_rows":1,"reconstruction":{"max_primes":2}}));
    let cancel = AtomicBool::new(true);
    let paused = master_refine_published_artifact(&source.0, &options, &cancel, |_| {}).unwrap();
    assert_eq!(paused["status"], "paused");
    assert_eq!(paused["modular_policy"]["reconstruction"]["max_primes"], 2);
    options.resume = true;
    options.modular_policy = Some(json!({"batch_rows":3,"reconstruction":{"max_primes":32}}));
    let resumed =
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(resumed["status"], "completed_nonminimal");
    assert_eq!(
        resumed["modular_policy"]["reconstruction"]["max_primes"],
        32
    );
    assert_eq!(resumed["relation_rows"], 0);
}

#[test]
fn partial_modular_publication_preserves_the_saved_search_sidecar_and_maps() {
    let source = Scratch::new();
    publish_fixture(&source.0, &mut session());
    let paused = Scratch::new();
    let mut options = MasterReductionOptions::new("unused", &paused.0);
    options.refinement_strategy = Some(MasterRefinementStrategy::ModularCircuits);
    options.modular_policy = Some(json!({"batch_rows":1}));
    options.checkpoint_interval = Duration::from_nanos(1);
    let cancel = AtomicBool::new(false);
    let report = master_refine_published_artifact(&source.0, &options, &cancel, |event| {
        if event["modular_refinement"]["source_rows_scanned"]
            .as_u64()
            .unwrap_or(0)
            > 0
        {
            cancel.store(true, Ordering::Relaxed);
        }
    })
    .unwrap();
    assert_eq!(report["status"], "paused");
    assert!(report["modular_state"].is_object());
    let saved = std::fs::read(paused.0.join("latest.json")).unwrap();
    let output = Scratch::new();
    let mut export = MasterReductionOptions::new("unused", &output.0);
    export.operation = MasterReductionOperation::Publish;
    let artifact =
        master_publish_partial_refinement(&paused.0, &export, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(artifact["status"], "published_unrefined");
    assert_eq!(artifact["finite_search_complete"], false);
    assert_eq!(
        artifact["remaining_terminals"],
        report["remaining_terminals"]
    );
    assert_eq!(artifact["terminal_relations"], report["terminal_relations"]);
    assert_eq!(artifact["modular_state"], report["modular_state"]);
    assert!(load_master_reduction(&output.0).is_ok());
    assert_eq!(std::fs::read(paused.0.join("latest.json")).unwrap(), saved);
}
