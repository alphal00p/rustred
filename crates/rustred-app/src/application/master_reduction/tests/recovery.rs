//! Explicit recovery authenticates a durable prefix; it never searches sources.
use super::*;
use std::collections::BTreeMap;

fn publish_options(directory: &Path) -> MasterReductionOptions {
    let mut options = MasterReductionOptions::new("unused", directory);
    options.operation = MasterReductionOperation::Publish;
    options
}

/// This is the on-disk state left when a writer dies after a completed native
/// checkpoint but before changing the manifest to `paused`. No marker is
/// repaired by the recovery operation itself.
fn abandoned_running(directory: &Path) -> (Value, TerminalRelationSession) {
    let initial = Scratch::new();
    publish_fixture(&initial.0, &mut session());
    let mut options = MasterReductionOptions::new("unused", directory);
    options.seed_depth = 2;
    let mut report =
        master_refine_published_artifact(&initial.0, &options, &AtomicBool::new(true), |_| {})
            .unwrap();
    let mut native = load_master_relation_session(directory).unwrap();
    while native.statistics().terminal_relations == 0 {
        assert!(!native.is_complete());
        native.step(&AtomicBool::new(false)).unwrap();
    }
    assert!(!native.is_complete());
    save(
        &options,
        &native,
        &mut report,
        "running",
        Instant::now(),
        &|_| {},
    )
    .unwrap();
    (report, native)
}

fn tree(directory: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, directory: &Path, result: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            assert!(!entry.file_type().unwrap().is_symlink());
            if entry.file_type().unwrap().is_dir() {
                visit(root, &entry.path(), result);
            } else {
                result.insert(
                    entry.path().strip_prefix(root).unwrap().to_path_buf(),
                    std::fs::read(entry.path()).unwrap(),
                );
            }
        }
    }
    let mut result = BTreeMap::new();
    visit(directory, directory, &mut result);
    result
}

#[test]
fn interrupted_running_recovery_preserves_exact_maps_guards_cursors_and_all_source_bytes() {
    let source = Scratch::new();
    let output = Scratch::new();
    let (original, native) = abandoned_running(&source.0);
    let before = tree(&source.0);
    let expected_maps = native.apply_all_terminals().unwrap();
    let expected_conditions = native.nonzero_conditions().to_vec();
    let expected_stats = native.statistics();
    assert!(
        master_publish_partial_refinement(
            &source.0,
            &publish_options(&output.0),
            &AtomicBool::new(false),
            |_| {},
        )
        .is_err()
    );
    assert!(!output.0.join("latest.json").exists());
    let report = master_recover_interrupted_refinement(
        &source.0,
        &publish_options(&output.0),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert_eq!(report["status"], "published_unrefined");
    assert_eq!(report["partial_refinement_source"]["status"], "running");
    assert_eq!(
        report["partial_refinement_source"]["native_state"],
        original["native_state"]
    );
    assert_eq!(
        report["interrupted_refinement_recovery"]["source_files_unchanged"],
        true
    );
    assert_eq!(
        report["interrupted_refinement_recovery"]["new_ordinary_source_rows"],
        0
    );
    assert_eq!(
        report["interrupted_refinement_recovery"]["new_modular_discovery_or_reconstruction"],
        false
    );
    assert_eq!(tree(&source.0), before);
    let recovered = load_master_reduction(&output.0).unwrap();
    assert_eq!(recovered.primary_session().statistics(), expected_stats);
    assert_eq!(
        recovered.primary_session().apply_all_terminals().unwrap(),
        expected_maps
    );
    assert_eq!(
        recovered.primary_session().nonzero_conditions(),
        expected_conditions
    );
    // The output is portable and cold-resumable without rewriting its source.
    let mut resume = publish_options(&output.0);
    resume.resume = true;
    let resumed =
        master_recover_interrupted_refinement(&source.0, &resume, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(
        resumed["partial_refinement_binding"],
        report["partial_refinement_binding"]
    );
    assert_eq!(tree(&source.0), before);
    let continued = Scratch::new();
    let mut refining = MasterReductionOptions::new("unused", &continued.0);
    refining.fresh_search = true;
    refining.seed_depth = 0;
    let next =
        master_refine_published_artifact(&output.0, &refining, &AtomicBool::new(true), |_| {})
            .unwrap();
    assert_eq!(next["status"], "paused");
    assert_eq!(next["relation_rows"], 0);
    assert_eq!(
        load_master_relation_session(&continued.0)
            .unwrap()
            .apply_all_terminals()
            .unwrap(),
        recovered.primary_session().apply_all_terminals().unwrap()
    );
}

#[test]
fn interrupted_recovery_refuses_an_active_writer_and_a_missing_existing_lock() {
    let source = Scratch::new();
    abandoned_running(&source.0);
    let before = tree(&source.0);
    let active = storage::WriterLock::acquire(&source.0).unwrap();
    let output = Scratch::new();
    assert!(
        master_recover_interrupted_refinement(
            &source.0,
            &publish_options(&output.0),
            &AtomicBool::new(false),
            |_| {},
        )
        .is_err()
    );
    assert!(!output.0.join("latest.json").exists());
    assert_eq!(tree(&source.0), before);
    drop(active);
    // The next valid recovery proves that the refusal was lock ownership,
    // rather than an invented permanent stale marker or PID-time heuristic.
    master_recover_interrupted_refinement(
        &source.0,
        &publish_options(&output.0),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    std::fs::remove_file(source.0.join("writer.lock")).unwrap();
    let unlocked_before = tree(&source.0);
    let refused = Scratch::new();
    assert!(
        master_recover_interrupted_refinement(
            &source.0,
            &publish_options(&refused.0),
            &AtomicBool::new(false),
            |_| {},
        )
        .is_err()
    );
    assert!(!source.0.join("writer.lock").exists());
    assert_eq!(tree(&source.0), unlocked_before);
}

#[test]
fn interrupted_recovery_rejects_unknown_status_incomplete_inventory_and_corrupt_native_state() {
    for mutation in ["status", "inventory", "generation", "native", "digest"] {
        let source = Scratch::new();
        let (mut report, _) = abandoned_running(&source.0);
        match mutation {
            "status" => report["status"] = json!("unknown-writer-status"),
            "inventory" => report["inventory"]["complete"] = json!(false),
            "generation" => report["checkpoint"]["generation"] = json!(0),
            "native" => report["native_state"] = Value::Null,
            "digest" => report["native_state"]["blake3"] = json!("wrong-digest"),
            _ => unreachable!(),
        }
        write_json(&source.0.join("latest.json"), &report).unwrap();
        let before = tree(&source.0);
        let output = Scratch::new();
        assert!(
            master_recover_interrupted_refinement(
                &source.0,
                &publish_options(&output.0),
                &AtomicBool::new(false),
                |_| {},
            )
            .is_err(),
            "mutation {mutation}"
        );
        assert!(!output.0.join("latest.json").exists());
        assert!(!output.0.join("artifact.json").exists());
        assert_eq!(tree(&source.0), before);
    }
}

#[test]
fn interrupted_recovery_cancellation_and_resume_keep_the_same_authenticated_source() {
    let source = Scratch::new();
    abandoned_running(&source.0);
    let before = tree(&source.0);
    let output = Scratch::new();
    let mut options = publish_options(&output.0);
    let paused =
        master_recover_interrupted_refinement(&source.0, &options, &AtomicBool::new(true), |_| {})
            .unwrap();
    assert_eq!(paused["status"], "paused");
    assert!(!output.0.join("artifact.json").exists());
    options.resume = true;
    let completed =
        master_recover_interrupted_refinement(&source.0, &options, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(completed["status"], "published_unrefined");
    assert_eq!(tree(&source.0), before);
    // Ordinary finalize cannot accidentally resume a recovery publication.
    assert!(
        master_publish_partial_refinement(&source.0, &options, &AtomicBool::new(false), |_| {},)
            .is_err()
    );
}

#[cfg(feature = "reconstruction")]
#[test]
fn interrupted_modular_recovery_uses_a_durable_checkpoint_without_restarting_discovery() {
    let initial = Scratch::new();
    publish_fixture(&initial.0, &mut session());
    let source = Scratch::new();
    let mut options = MasterReductionOptions::new("unused", &source.0);
    options.refinement_strategy = Some(MasterRefinementStrategy::ModularCircuits);
    options.modular_policy = Some(json!({"batch_rows":1}));
    options.checkpoint_interval = Duration::from_nanos(1);
    let killed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        master_refine_published_artifact(&initial.0, &options, &AtomicBool::new(false), |event| {
            if event["event"] == "master_reduction_checkpoint"
                && event["status"] == "running"
                && event["modular_refinement"]["accepted_relations"]
                    .as_u64()
                    .unwrap_or(0)
                    > 0
            {
                panic!("simulate writer death after a durable accepted modular proof");
            }
        })
    }));
    assert!(killed.is_err());
    let before = tree(&source.0);
    let report = master_reduction_inspect(&source.0).unwrap();
    assert_eq!(report["status"], "running");
    assert!(report["modular_state"].is_object());
    assert!(
        report["modular_refinement"]["accepted_relations"]
            .as_u64()
            .unwrap()
            > 0
    );
    let exact = load_master_relation_session(&source.0).unwrap();
    assert_eq!(exact.statistics().remaining_terminals, 1);
    assert!(!exact.terminal_rules().is_empty());
    let output = Scratch::new();
    let artifact = master_recover_interrupted_refinement(
        &source.0,
        &publish_options(&output.0),
        &AtomicBool::new(false),
        |event| {
            assert_ne!(event["stage"], "initializing_modular_image");
            assert_ne!(event["stage"], "modular_discovery");
            assert_ne!(event["stage"], "modular_reconstruction");
        },
    )
    .unwrap();
    assert_eq!(artifact["status"], "published_unrefined");
    assert_eq!(artifact["modular_state"], report["modular_state"]);
    assert_eq!(artifact["modular_refinement"], report["modular_refinement"]);
    let loaded = load_master_reduction(&output.0).unwrap();
    assert_eq!(loaded.primary_session().statistics(), exact.statistics());
    assert_eq!(
        loaded.primary_session().apply_all_terminals().unwrap(),
        exact.apply_all_terminals().unwrap()
    );
    assert_eq!(tree(&source.0), before);
}
