use super::*;
use rustred::reduction::terminal_normalization::TerminalNumeratorSymmetryEquations;

fn proof_recipe(source: &Path, root: &Path, native: &TerminalRelationSession) -> PathBuf {
    let certificate = TerminalNumeratorSymmetryEquations::prepare(
        native.family_owner().clone(),
        native.raw_terminals(),
        Default::default(),
    )
    .unwrap();
    let bytes = certificate
        .encode_native(profile::native_io_limits(None))
        .unwrap();
    let path = root.join("symmetry.rrbin");
    std::fs::write(&path, &bytes).unwrap();
    let manifest = source.join("artifact.json");
    let recipe = root.join("recipe.json");
    write_json(
        &recipe,
        &json!({"schema":"rustred.master-refinement-proof-import.v1",
        "baseline":{"artifact":manifest,"blake3":storage::digest_file(&manifest).unwrap()},
        "numerator_symmetry":{"path":path,"blake3":blake3::hash(&bytes).to_hex().to_string()},
        "include_raw_symmetry":true,"selected_saved_rows":[]}),
    )
    .unwrap();
    recipe
}

#[test]
fn import_preserves_newer_maps_and_publishes_portable_no_search_state() {
    let baseline = Scratch::new();
    let current = Scratch::new();
    let recipe_root = Scratch::new();
    let output = Scratch::new();
    let base_report = publish_fixture(&baseline.0, &mut session());
    let mut stronger = session();
    stronger.step(&AtomicBool::new(false)).unwrap();
    let current_report = publish_fixture(&current.0, &mut stronger);
    let source = load_master_reduction(&current.0).unwrap();
    let before = std::fs::read(current.0.join("artifact.json")).unwrap();
    let recipe = proof_recipe(&baseline.0, &recipe_root.0, &session());
    let mut options = MasterReductionOptions::new("unused", &output.0);
    options.operation = MasterReductionOperation::Publish;
    let result = master_import_proved_refinement(
        &current.0,
        &recipe,
        &options,
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert_eq!(result["status"], "published_unrefined");
    assert_eq!(result["import_complete"], true);
    assert_eq!(result["inventory"], current_report["inventory"]);
    assert_eq!(result["scope_binding"], base_report["scope_binding"]);
    let imported = load_master_reduction(&output.0).unwrap();
    for key in source.primary_session().raw_terminals() {
        let actual = imported
            .apply_terminal(imported.primary_session().family_owner(), key)
            .unwrap();
        let expected = source
            .apply_terminal(source.primary_session().family_owner(), key)
            .unwrap();
        assert_eq!(actual.terms(), expected.terms());
        assert!(
            expected
                .nonzero_conditions()
                .iter()
                .all(|guard| actual.nonzero_conditions().contains(guard))
        );
    }
    assert_eq!(
        imported
            .primary_session()
            .statistics()
            .completed_source_rows,
        0
    );
    assert_eq!(imported.primary_session().statistics().auxiliary_columns, 0);
    assert!(imported.primary_session().assistance_binding().is_none());
    assert_eq!(
        std::fs::read(current.0.join("artifact.json")).unwrap(),
        before
    );
    drop(source);
    drop(imported);
    // Published application contains its own recipes/certificates and never
    // reopens the original immutable publication or research input paths.
    std::fs::remove_file(recipe_root.0.join("symmetry.rrbin")).unwrap();
    assert!(load_master_reduction(&output.0).is_ok());
    let certificate = result["proof_import"]["archive"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["path"] == "proof-import/symmetry.rrbin")
        .unwrap();
    let path = storage::safe_child(&output.0, certificate["path"].as_str().unwrap()).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let mut changed = bytes.clone();
    changed[0] ^= 1;
    std::fs::write(&path, changed).unwrap();
    assert!(load_master_reduction(&output.0).is_err());
    std::fs::write(path, bytes).unwrap();
    assert!(load_master_reduction(&output.0).is_ok());
}

#[test]
fn import_pauses_before_preparation_and_resume_uses_archived_recipe() {
    let source = Scratch::new();
    publish_fixture(&source.0, &mut session());
    let recipe_root = Scratch::new();
    let recipe = proof_recipe(&source.0, &recipe_root.0, &session());
    let output = Scratch::new();
    let mut options = MasterReductionOptions::new("unused", &output.0);
    options.operation = MasterReductionOperation::Publish;
    let paused = master_import_proved_refinement(
        &source.0,
        &recipe,
        &options,
        &AtomicBool::new(true),
        |_| {},
    )
    .unwrap();
    assert_eq!(paused["status"], "paused");
    assert_eq!(paused["import_complete"], false);
    assert!(!output.0.join("artifact.json").exists());
    let archived = output.0.join("proof-import/recipe.json");
    std::fs::remove_file(recipe).unwrap();
    options.resume = true;
    let result = master_import_proved_refinement(
        &source.0,
        &archived,
        &options,
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert_eq!(result["import_complete"], true);
    let again = master_import_proved_refinement(
        &source.0,
        &archived,
        &options,
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert_eq!(again["status"], "published_unrefined");
}

#[test]
fn import_rejects_mutated_recipe_sources_and_selected_witnesses_without_publication() {
    let source = Scratch::new();
    publish_fixture(&source.0, &mut session());
    let recipe_root = Scratch::new();
    let recipe = proof_recipe(&source.0, &recipe_root.0, &session());
    let original = std::fs::read(&recipe).unwrap();
    for mutation in ["certificate", "baseline", "witness"] {
        let mut value: Value = serde_json::from_slice(&original).unwrap();
        match mutation {
            "certificate" => value["numerator_symmetry"]["blake3"] = json!("0".repeat(64)),
            "baseline" => value["baseline"]["blake3"] = json!("0".repeat(64)),
            "witness" => value["selected_saved_rows"] = json!([{"source":[2],"witness":100}]),
            _ => unreachable!(),
        }
        write_json(&recipe, &value).unwrap();
        let output = Scratch::new();
        let mut options = MasterReductionOptions::new("unused", &output.0);
        options.operation = MasterReductionOperation::Publish;
        assert!(
            master_import_proved_refinement(
                &source.0,
                &recipe,
                &options,
                &AtomicBool::new(false),
                |_| {}
            )
            .is_err()
        );
        assert!(!output.0.join("artifact.json").exists());
    }
}

#[test]
fn import_collection_pause_retains_fixed_parent_identity_and_resumes_without_original_source() {
    let source = Scratch::new();
    publish_fixture(&source.0, &mut session());
    let recipe_root = Scratch::new();
    let recipe = proof_recipe(&source.0, &recipe_root.0, &session());
    let output = Scratch::new();
    let mut options = MasterReductionOptions::new("unused", &output.0);
    options.operation = MasterReductionOperation::Publish;
    let cancel = AtomicBool::new(false);
    let paused = master_import_proved_refinement(&source.0, &recipe, &options, &cancel, |event| {
        if event["stage"] == "terminal_collection" {
            cancel.store(true, Ordering::Relaxed);
        }
    })
    .unwrap();
    assert_eq!(paused["status"], "paused");
    assert!(paused["retained_relations"].is_object());
    let binding = paused["retained_relations"]["binding"].clone();
    let parent = paused["retained_relations"]["source"].clone();
    let moved = Scratch::new();
    std::fs::rename(&source.0, moved.0.join("source")).unwrap();
    std::fs::remove_file(recipe).unwrap();
    options.resume = true;
    let archived = output.0.join("proof-import/recipe.json");
    let result = master_import_proved_refinement(
        &source.0,
        &archived,
        &options,
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert_eq!(result["import_complete"], true);
    assert_eq!(result["retained_relations"]["binding"], binding);
    assert_eq!(result["retained_relations"]["source"], parent);
    assert!(load_master_reduction(&output.0).is_ok());
}

#[test]
fn expected_result_is_only_a_consequence_check_not_authority() {
    let source = Scratch::new();
    publish_fixture(&source.0, &mut session());
    let recipe_root = Scratch::new();
    let recipe = proof_recipe(&source.0, &recipe_root.0, &session());
    let mut expected = session();
    expected
        .enable_assistance("negative-test-unproved-zero".into())
        .unwrap();
    let first = expected.raw_terminals().first().unwrap().clone();
    let row = rustred::reduction::terminal_relations::TerminalEquation {
        terms: std::collections::BTreeMap::from([(
            first.clone(),
            expected.family_owner().coefficient_context().one(),
        )]),
        nonzero_conditions: vec![],
    };
    while expected.statistics().pending_assistance_keys != 0
        || expected.statistics().pending_assistance_rows != 0
    {
        expected
            .step_with_provider(&AtomicBool::new(false), |key| {
                Ok(if key == &first {
                    vec![row.clone()]
                } else {
                    vec![]
                })
            })
            .unwrap();
    }
    let bytes = expected
        .to_native_bytes(profile::native_io_limits(None))
        .unwrap();
    let expected_path = recipe_root.0.join("expected.rrbin");
    std::fs::write(&expected_path, &bytes).unwrap();
    let mut value: Value = serde_json::from_slice(&std::fs::read(&recipe).unwrap()).unwrap();
    value["expected_result"] =
        json!({"path":expected_path,"blake3":blake3::hash(&bytes).to_hex().to_string()});
    write_json(&recipe, &value).unwrap();
    let output = Scratch::new();
    let mut options = MasterReductionOptions::new("unused", &output.0);
    options.operation = MasterReductionOperation::Publish;
    assert!(
        master_import_proved_refinement(
            &source.0,
            &recipe,
            &options,
            &AtomicBool::new(false),
            |_| {}
        )
        .is_err()
    );
    assert!(!output.0.join("artifact.json").exists());
}

#[cfg(unix)]
#[test]
fn import_rejects_direct_and_symlinked_baseline_overlap_before_creating_output() {
    let baseline = Scratch::new();
    publish_fixture(&baseline.0, &mut session());
    let current = Scratch::new();
    publish_fixture(&current.0, &mut session());
    let root = Scratch::new();
    let recipe = proof_recipe(&baseline.0, &root.0, &session());
    let original = std::fs::read(&recipe).unwrap();
    let before = std::fs::read(baseline.0.join("artifact.json")).unwrap();
    let original_entries = std::fs::read_dir(&baseline.0).unwrap().count();
    for symlinked in [false, true] {
        let mut value: Value = serde_json::from_slice(&original).unwrap();
        if symlinked {
            let alias = root.0.join("baseline-alias");
            std::os::unix::fs::symlink(&baseline.0, &alias).unwrap();
            value["baseline"]["artifact"] = json!(alias.join("artifact.json"));
        }
        write_json(&recipe, &value).unwrap();
        let output = baseline.0.join(if symlinked {
            "nested-alias"
        } else {
            "nested-direct"
        });
        let mut options = MasterReductionOptions::new("unused", &output);
        options.operation = MasterReductionOperation::Publish;
        assert!(
            master_import_proved_refinement(
                &current.0,
                &recipe,
                &options,
                &AtomicBool::new(false),
                |_| {}
            )
            .is_err()
        );
        assert!(!output.exists());
        assert_eq!(
            std::fs::read_dir(&baseline.0).unwrap().count(),
            original_entries
        );
        assert_eq!(
            std::fs::read(baseline.0.join("artifact.json")).unwrap(),
            before
        );
    }
}
