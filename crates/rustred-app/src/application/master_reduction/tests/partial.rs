use super::*;
use rustred::reduction::terminal_relations::TerminalEquation;
use std::collections::BTreeMap;

fn publishing(directory: &Path) -> MasterReductionOptions {
    let mut options = MasterReductionOptions::new("unused", directory);
    options.operation = MasterReductionOperation::Publish;
    options
}

fn failed_refinement(directory: &Path) -> (Value, TerminalRelationSession) {
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
        "failed",
        Instant::now(),
        &|_| {},
    )
    .unwrap();
    report["last_error"] = json!("injected source search budget failure");
    write_json(&directory.join("latest.json"), &report).unwrap();
    (report, native)
}

#[test]
fn partial_publication_preserves_durable_rows_cursor_and_portable_inherited_maps() {
    let source = Scratch::new();
    let output = Scratch::new();
    let (source_report, native) = failed_refinement(&source.0);
    let original_manifest = std::fs::read(source.0.join("latest.json")).unwrap();
    let original_native = native.to_native_bytes(Default::default()).unwrap();
    let expected = native.apply_all_terminals().unwrap();
    let before = native.statistics();
    assert!(source_report["inherited_collection"].is_object());
    assert!(source_report["collection_state"].is_null());
    let report = master_publish_partial_refinement(
        &source.0,
        &publishing(&output.0),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert_eq!(report["status"], "published_unrefined");
    assert_eq!(report["operation"], "publish");
    assert_eq!(report["finite_search_complete"], false);
    assert_eq!(report["refinement_complete"], false);
    assert_eq!(report["finite_feedback"], false);
    assert_eq!(report["inventory"], source_report["inventory"]);
    assert_eq!(report["scope_binding"], source_report["scope_binding"]);
    assert_eq!(
        report["publication_assurance"],
        source_report["publication_assurance"]
    );
    assert_eq!(
        report["independently_verified"],
        source_report["independently_verified"]
    );
    assert_eq!(
        report["partial_refinement_source"]["native_state"],
        source_report["native_state"]
    );
    assert_eq!(
        report["partial_refinement_source"]["last_error"],
        source_report["last_error"]
    );
    assert!(report.get("last_error").is_none());
    assert_eq!(report["collection"]["status"], "completed");
    assert_eq!(
        std::fs::read(source.0.join("latest.json")).unwrap(),
        original_manifest
    );
    assert_eq!(
        load_master_relation_session(&source.0)
            .unwrap()
            .to_native_bytes(Default::default())
            .unwrap(),
        original_native
    );
    let restored = load_master_reduction(&output.0).unwrap();
    assert_eq!(restored.primary_session().statistics(), before);
    assert_eq!(
        restored.primary_session().apply_all_terminals().unwrap(),
        expected
    );
    let collected = restored.collection().remaining_terminals().clone();

    // A cold application owns every inherited member and proof, including the
    // frozen pre-refinement session. The old source may disappear afterwards.
    let hidden = source.0.with_extension("hidden");
    std::fs::rename(&source.0, &hidden).unwrap();
    let portable = load_master_reduction(&output.0);
    std::fs::rename(&hidden, &source.0).unwrap();
    assert_eq!(
        portable.unwrap().collection().remaining_terminals(),
        &collected
    );

    // Reusing the artifact for a later scope extension reindexes proved rows
    // without advancing the recorded finite source cursor.
    let extended = Scratch::new();
    let mut extended_native = load_master_relation_session(&output.0).unwrap();
    let mut extended_report = report.clone();
    extended_report["checkpoint"] = json!({"generation":0});
    extended_report["native_state"] = Value::Null;
    collection::inherit(&output.0, &extended.0, &mut extended_report).unwrap();
    extended_native
        .extend(&BTreeSet::from([IntegralKey::try_new([3]).unwrap()]), 2)
        .unwrap();
    assert!(extended_native.statistics().pending_rebuild_rows > 0);
    execute_session(
        &publishing(&extended.0),
        &mut extended_native,
        &mut extended_report,
        &AtomicBool::new(false),
        &|_| {},
        Instant::now(),
    )
    .unwrap();
    assert_eq!(
        extended_native.statistics().completed_source_rows,
        before.completed_source_rows
    );
    for key in native.raw_terminals() {
        assert_eq!(
            extended_native.apply_terminal(key).unwrap(),
            native.apply_terminal(key).unwrap()
        );
    }

    // An explicit refinement can still drain the preserved search cursor.
    let refined = Scratch::new();
    let completed = master_refine_published_artifact(
        &output.0,
        &MasterReductionOptions::new("unused", &refined.0),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert_eq!(completed["status"], "completed_nonminimal");
    assert!(completed["relation_rows"].as_u64().unwrap() > before.completed_source_rows);
}

#[test]
fn partial_publication_finishes_only_pending_rebuild_and_resumes_after_cancellation() {
    let source = Scratch::new();
    let output = Scratch::new();
    let (mut source_report, mut native) = failed_refinement(&source.0);
    let expected = native
        .apply_terminal(&IntegralKey::try_new([2]).unwrap())
        .unwrap();
    native
        .extend(&BTreeSet::from([IntegralKey::try_new([3]).unwrap()]), 2)
        .unwrap();
    assert!(native.statistics().pending_rebuild_rows > 0);
    let before = native.statistics();
    save(
        &MasterReductionOptions::new("unused", &source.0),
        &native,
        &mut source_report,
        "paused",
        Instant::now(),
        &|_| {},
    )
    .unwrap();
    let mut options = publishing(&output.0);
    let paused =
        master_publish_partial_refinement(&source.0, &options, &AtomicBool::new(true), |_| {})
            .unwrap();
    assert_eq!(paused["status"], "paused");
    assert!(!output.0.join("artifact.json").exists());
    assert_eq!(
        load_master_relation_session(&output.0)
            .unwrap()
            .statistics(),
        before
    );
    options.resume = true;
    let completed =
        master_publish_partial_refinement(&source.0, &options, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(completed["status"], "published_unrefined");
    let restored = load_master_relation_session(&output.0).unwrap();
    let after = restored.statistics();
    assert_eq!(after.pending_rebuild_rows, 0);
    assert_eq!(after.completed_source_rows, before.completed_source_rows);
    assert_eq!(after.completed_seeds, before.completed_seeds);
    assert_eq!(
        restored
            .apply_terminal(&IntegralKey::try_new([2]).unwrap())
            .unwrap(),
        expected
    );
    let manifest = std::fs::read(output.0.join("artifact.json")).unwrap();
    master_publish_partial_refinement(&source.0, &options, &AtomicBool::new(false), |_| {})
        .unwrap();
    assert_eq!(
        std::fs::read(output.0.join("artifact.json")).unwrap(),
        manifest
    );
    source_report["last_error"] = json!("different durable snapshot");
    write_json(&source.0.join("latest.json"), &source_report).unwrap();
    assert!(
        master_publish_partial_refinement(&source.0, &options, &AtomicBool::new(false), |_| {})
            .is_err()
    );
}

#[test]
fn partial_publication_freezes_assisted_authority_guards_and_pending_provider_work() {
    let source = Scratch::new();
    let output = Scratch::new();
    let sample = session();
    let raw = BTreeSet::from([1, 2, 3].map(|power| IntegralKey::try_new([power]).unwrap()));
    let mut native =
        TerminalRelationSession::new(sample.family_owner().clone(), raw, 2, Default::default())
            .unwrap();
    let mut options = MasterReductionOptions::new("unused", &source.0);
    options.saved_rule_assistance = true;
    let mut report = json!({"schema":SCHEMA,"operation":"refine","scope_binding":"assisted-scope",
        "checkpoint":{"generation":0},"inventory":{"complete":true,"independently_verified":false},
        "inputs":portable_inputs(&source.0),"deep_verification":false});
    assistance::configure(&options, &mut native, &mut report).unwrap();
    let c = native.family_owner().coefficient_context();
    let d = c.parameter("d").unwrap();
    let equation = TerminalEquation {
        terms: BTreeMap::from([
            (
                IntegralKey::try_new([2]).unwrap(),
                c.try_sub(&d, &c.integer(4), Default::default()).unwrap(),
            ),
            (IntegralKey::try_new([1]).unwrap(), c.integer(-4)),
        ]),
        nonzero_conditions: vec![d],
    };
    while native.statistics().completed_assistance_rows == 0 {
        native
            .step_with_provider(&AtomicBool::new(false), |_| Ok(vec![equation.clone()]))
            .unwrap();
    }
    assert!(native.statistics().pending_assistance_keys > 0);
    let expected = native
        .apply_terminal(&IntegralKey::try_new([2]).unwrap())
        .unwrap();
    let guards = native.nonzero_conditions().to_vec();
    let before = native.statistics();
    save(
        &options,
        &native,
        &mut report,
        "failed",
        Instant::now(),
        &|_| {},
    )
    .unwrap();
    let published = master_publish_partial_refinement(
        &source.0,
        &publishing(&output.0),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert_eq!(published["saved_rule_assistance"], false);
    assert_eq!(
        published["inherited_equation_authority"]["saved_rule_assistance"],
        true
    );
    assert_eq!(published["publication_assurance"], "trusted_saved_scope");
    let restored = load_master_reduction(&output.0).unwrap();
    assert_eq!(restored.primary_session().statistics(), before);
    assert_eq!(restored.primary_session().nonzero_conditions(), guards);
    assert_eq!(
        restored
            .primary_session()
            .apply_terminal(&IntegralKey::try_new([2]).unwrap())
            .unwrap(),
        expected
    );
    assert!(
        restored
            .apply_terminal(
                restored.primary_session().family_owner(),
                &IntegralKey::try_new([2]).unwrap()
            )
            .is_ok()
    );
}

#[test]
fn partial_publication_rejects_active_unverified_or_overlapping_sources() {
    let source = Scratch::new();
    let output = Scratch::new();
    let (mut report, _) = failed_refinement(&source.0);
    let options = publishing(&output.0.join("new-output"));
    let lock = storage::WriterLock::acquire(&source.0).unwrap();
    assert!(
        master_publish_partial_refinement(&source.0, &options, &AtomicBool::new(false), |_| {})
            .is_err()
    );
    assert!(!options.directory.exists());
    drop(lock);
    assert!(
        master_publish_partial_refinement(
            &source.0,
            &publishing(&source.0.join("nested")),
            &AtomicBool::new(false),
            |_| {}
        )
        .is_err()
    );
    assert!(!source.0.join("nested").exists());
    report["status"] = json!("running");
    write_json(&source.0.join("latest.json"), &report).unwrap();
    assert!(
        master_publish_partial_refinement(&source.0, &options, &AtomicBool::new(false), |_| {})
            .is_err()
    );
    assert!(!options.directory.exists());
    report["status"] = json!("failed");
    report["inventory"]["complete"] = json!(false);
    write_json(&source.0.join("latest.json"), &report).unwrap();
    assert!(
        master_publish_partial_refinement(&source.0, &options, &AtomicBool::new(false), |_| {})
            .is_err()
    );
    assert!(!options.directory.exists());
}

#[test]
fn partial_publication_rejects_corrupt_durable_state_or_inherited_proof() {
    for field in ["native_state", "inherited_collection"] {
        let source = Scratch::new();
        let output = Scratch::new();
        let (report, _) = failed_refinement(&source.0);
        let state = if field == "native_state" {
            &report["native_state"]
        } else {
            &report["inherited_collection"]["native_state"]
        };
        let path = source.0.join(state["file"].as_str().unwrap());
        let mut bytes = std::fs::read(&path).unwrap();
        *bytes.last_mut().unwrap() ^= 1;
        // Atomic replacement keeps any content-addressed hardlink peers intact.
        write_file_atomically(&path, &bytes, true).unwrap();
        assert!(
            master_publish_partial_refinement(
                &source.0,
                &publishing(&output.0),
                &AtomicBool::new(false),
                |_| {},
            )
            .is_err()
        );
        assert!(!output.0.join("artifact.json").exists());
        assert_eq!(
            master_reduction_inspect(&source.0).unwrap()["status"],
            "failed"
        );
    }
}
