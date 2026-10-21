use super::*;

fn collected_only_source(directory: &Path) -> Value {
    let native = session();
    let mut report = publish_fixture(directory, &mut session());
    let mut options = MasterReductionOptions::new("unused", directory);
    options.finite_feedback = Some(false);
    // Deliberately collect without advancing the primary ordinary source
    // cursor. This catches importing only primary_session's weaker maps.
    report["collection_state"] = Value::Null;
    report["collection"] = json!({"strategy":collection::STRATEGY,"status":"pending"});
    collection::record_feedback(&mut report, false);
    collection::complete(
        &options,
        &native,
        &mut report,
        &AtomicBool::new(false),
        &|_| {},
        Instant::now(),
    )
    .unwrap();
    save(
        &options,
        &native,
        &mut report,
        "published_unrefined",
        Instant::now(),
        &|_| {},
    )
    .unwrap();
    publish_final(&options, &mut report).unwrap();
    report
}

#[test]
fn relations_only_round_preserves_full_collection_and_resets_requested_shape() {
    let source = Scratch::new();
    let source_report = collected_only_source(&source.0);
    let source_manifest = std::fs::read(source.0.join("latest.json")).unwrap();
    let original = load_master_reduction(&source.0).unwrap();
    assert_eq!(
        original
            .primary_session()
            .statistics()
            .completed_source_rows,
        0
    );
    assert_eq!(
        original.primary_session().statistics().remaining_terminals,
        2
    );
    assert_eq!(original.collection().remaining_terminals().len(), 1);
    let output = Scratch::new();
    let mut options = MasterReductionOptions::new("unused", &output.0);
    options.fresh_search = true;
    options.seed_depth = 3;
    options.containing_sector_depth = 0;
    options.finite_feedback = Some(false);
    let paused =
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(true), |_| {})
            .unwrap();
    assert_eq!(paused["status"], "paused");
    assert_eq!(paused["fresh_search"], true);
    assert_eq!(paused["fresh_search_recipe"], fresh::RECIPE);
    assert_eq!(paused["seed_depth"], 3);
    assert_eq!(paused["containing_sector_depth"], 0);
    assert_eq!(paused["relation_rows"], 0);
    assert!(paused["inherited_collection"].is_null());
    assert!(paused["modular_state"].is_null());
    let fresh = load_master_relation_session(&output.0).unwrap();
    assert_eq!(fresh.statistics().pending_rebuild_rows, 0);
    assert_eq!(fresh.statistics().completed_source_rows, 0);
    assert_eq!(fresh.statistics().auxiliary_columns, 0);
    for key in original.primary_session().raw_terminals() {
        let exact = original
            .apply_terminal(original.primary_session().family_owner(), key)
            .unwrap();
        let local = exact
            .terms()
            .iter()
            .map(|(key, value)| (key.integral().clone(), value.clone()))
            .collect();
        assert_eq!(fresh.apply_terminal(key).unwrap(), local);
        assert!(
            exact
                .nonzero_conditions()
                .iter()
                .all(|guard| fresh.nonzero_conditions().contains(guard))
        );
    }
    assert!(
        paused["relation_authority"]
            .as_str()
            .unwrap()
            .contains("not newly regenerated")
    );
    options.resume = true;
    let completed =
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(completed["status"], "completed_nonminimal");
    assert_eq!(completed["inventory"], source_report["inventory"]);
    assert_eq!(completed["inputs"], source_report["inputs"]);
    assert_eq!(
        std::fs::read(source.0.join("latest.json")).unwrap(),
        source_manifest
    );
    let portable = load_master_reduction(&output.0).unwrap();
    assert_eq!(portable.collection().remaining_terminals().len(), 1);
    let continued = Scratch::new();
    let continuation = MasterReductionOptions::new("unused", &continued.0);
    let ordinary =
        master_refine_published_artifact(&output.0, &continuation, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(ordinary["status"], "completed_nonminimal");
    assert_eq!(ordinary["fresh_search"], false);
    assert!(ordinary["retained_relations"].is_object());
    assert_eq!(
        load_master_reduction(&continued.0)
            .unwrap()
            .collection()
            .remaining_terminals()
            .len(),
        1
    );
    options.fresh_search = false;
    assert!(
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {})
            .is_err()
    );
}

#[test]
fn retained_relations_refuse_external_peer_transport_without_losing_internal_continuation() {
    let source = Scratch::new();
    collected_only_source(&source.0);
    let fresh = Scratch::new();
    let mut options = MasterReductionOptions::new("unused", &fresh.0);
    options.fresh_search = true;
    master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {}).unwrap();
    let output = Scratch::new();
    let mut peer_options = MasterReductionOptions::new("unused", &output.0);
    peer_options.collection_artifacts.push(fresh.0.clone());
    assert!(
        master_refine_published_artifact(&source.0, &peer_options, &AtomicBool::new(true), |_| {})
            .is_err()
    );
    assert!(!output.0.join("artifact.json").exists());
    let output = Scratch::new();
    peer_options.directory = output.0.clone();
    peer_options.collection_artifacts = vec![source.0.clone()];
    assert!(
        master_refine_published_artifact(&fresh.0, &peer_options, &AtomicBool::new(true), |_| {})
            .is_err()
    );
    assert!(!output.0.join("artifact.json").exists());
}

#[test]
fn relations_only_search_can_lower_old_depth_and_cold_resume_without_source_path() {
    let source = Scratch::new();
    let mut native = session();
    native.extend(&BTreeSet::new(), 2).unwrap();
    while !native.is_complete() {
        native.step(&AtomicBool::new(false)).unwrap();
    }
    let mut source_report = publish_fixture(&source.0, &mut native);
    source_report["containing_sector_depth"] = json!(2);
    write_json(&source.0.join("latest.json"), &source_report).unwrap();
    let output = Scratch::new();
    let mut options = MasterReductionOptions::new("unused", &output.0);
    options.fresh_search = true;
    options.seed_depth = 1;
    options.containing_sector_depth = 0;
    let paused =
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(true), |_| {})
            .unwrap();
    assert_eq!(paused["seed_depth"], 1);
    assert_eq!(paused["containing_sector_depth"], 0);
    assert_eq!(paused["relation_rows"], 0);
    let hidden = source.0.with_extension("hidden-fresh-source");
    std::fs::rename(&source.0, &hidden).unwrap();
    let loaded = load_master_relation_session(&output.0);
    std::fs::rename(&hidden, &source.0).unwrap();
    assert!(loaded.is_ok());
    options.resume = true;
    let resumed =
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(resumed["status"], "completed_nonminimal");
}

#[test]
fn retained_capsule_and_authority_mutations_fail_cold_load() {
    let source = Scratch::new();
    collected_only_source(&source.0);
    let output = Scratch::new();
    let mut options = MasterReductionOptions::new("unused", &output.0);
    options.fresh_search = true;
    let report =
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(true), |_| {})
            .unwrap();
    for changed in [
        "binding",
        "source",
        "native_state",
        "raw_terminals",
        "normalization_profile",
    ] {
        let mut mutated = report.clone();
        mutated["retained_relations"][changed] = json!("tampered");
        write_json(&output.0.join("latest.json"), &mutated).unwrap();
        assert!(
            load_master_relation_session(&output.0).is_err(),
            "{changed}"
        );
    }
    let mut erased = report.clone();
    erased.as_object_mut().unwrap().remove("retained_relations");
    write_json(&output.0.join("latest.json"), &erased).unwrap();
    assert!(load_master_relation_session(&output.0).is_err());
    write_json(&output.0.join("latest.json"), &report).unwrap();
    let path = output.0.join(
        report["retained_relations"]["native_state"]["file"]
            .as_str()
            .unwrap(),
    );
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[0] ^= 1;
    std::fs::write(&path, bytes).unwrap();
    assert!(load_master_relation_session(&output.0).is_err());
}

#[test]
fn retained_capsule_cold_loads_during_inventory_rebuild_and_survives_publication() {
    let source = Scratch::new();
    collected_only_source(&source.0);
    let fresh = Scratch::new();
    let mut options = MasterReductionOptions::new("unused", &fresh.0);
    options.fresh_search = true;
    let report =
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {})
            .unwrap();
    let mut native = load_master_relation_session(&fresh.0).unwrap();
    let output = Scratch::new();
    let mut extension = report.clone();
    extension["operation"] = json!("publish");
    storage::clone_inputs(&fresh.0, &output.0, &report).unwrap();
    collection::inherit(&fresh.0, &output.0, &mut extension).unwrap();
    let mut inventory = native.raw_terminals().clone();
    let added = IntegralKey::try_new([3]).unwrap();
    inventory.insert(added.clone());
    let depth = native.statistics().seed_depth;
    extend_previous_inventory(&mut native, &inventory, depth).unwrap();
    assert!(native.statistics().pending_rebuild_rows > 0);
    let mut publication = MasterReductionOptions::new("unused", &output.0);
    publication.operation = MasterReductionOperation::Publish;
    save(
        &publication,
        &native,
        &mut extension,
        "paused",
        Instant::now(),
        &|_| {},
    )
    .unwrap();
    let mut restored = load_master_relation_session(&output.0).unwrap();
    assert!(restored.apply_terminal(&added).is_err());
    execute_session(
        &publication,
        &mut restored,
        &mut extension,
        &AtomicBool::new(false),
        &|_| {},
        Instant::now(),
    )
    .unwrap();
    let loaded = load_master_reduction(&output.0).unwrap();
    assert_eq!(
        loaded.primary_session().statistics().pending_rebuild_rows,
        0
    );
    assert!(
        loaded
            .apply_terminal(loaded.primary_session().family_owner(), &added)
            .is_ok()
    );
    assert!(extension["retained_relations"].is_object());
}

#[cfg(feature = "reconstruction")]
#[test]
fn fresh_modular_round_keeps_only_compact_maps_and_zero_exact_source_cursor() {
    let source = Scratch::new();
    collected_only_source(&source.0);
    let output = Scratch::new();
    let mut options = MasterReductionOptions::new("unused", &output.0);
    options.fresh_search = true;
    options.refinement_strategy = Some(MasterRefinementStrategy::ModularCircuits);
    options.finite_feedback = Some(false);
    let report =
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(report["status"], "completed_nonminimal");
    assert_eq!(report["relation_rows"], 0);
    assert!(report["modular_search_complete"].as_bool().unwrap());
    assert_eq!(
        load_master_reduction(&output.0)
            .unwrap()
            .collection()
            .remaining_terminals()
            .len(),
        1
    );
}
