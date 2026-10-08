use super::*;

fn named_session(name: &str) -> TerminalRelationSession {
    let c = CoefficientContext::try_new(["d"]).unwrap();
    let f = Arc::new(
        IntegralFamily::new(
            name,
            vec!["q".into()],
            vec![],
            c.clone(),
            c.parameter("d").unwrap(),
            vec![AffineDenominator::new(c.integer(-1), vec![c.one()])],
            vec![],
            vec![c.zero()],
        )
        .unwrap(),
    );
    TerminalRelationSession::new(
        f,
        BTreeSet::from([
            IntegralKey::try_new([1]).unwrap(),
            IntegralKey::try_new([2]).unwrap(),
        ]),
        0,
        Default::default(),
    )
    .unwrap()
}

#[test]
fn published_application_includes_collection_and_is_portable() {
    let primary = Scratch::new();
    let peer = Scratch::new();
    let output = Scratch::new();
    publish_fixture(&primary.0, &mut named_session("primary"));
    publish_fixture(&peer.0, &mut named_session("peer"));
    let mut options = MasterReductionOptions::new("unused", &output.0);
    options.collection_artifacts = vec![peer.0.clone()];
    let report =
        master_refine_published_artifact(&primary.0, &options, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(report["collection"]["families"], 2);
    assert_eq!(report["collection"]["remaining_terminals"], 1);
    assert_eq!(report["remaining_terminals"], 1);
    let loaded = load_master_reduction(&output.0).unwrap();
    let expected = loaded.collection().remaining_terminals().clone();
    assert_eq!(expected.len(), 1);
    for s in std::iter::once(loaded.primary_session()).chain(loaded.member_sessions().iter()) {
        for key in s.raw_terminals() {
            let reduction = loaded.apply_terminal(s.family_owner(), key).unwrap();
            assert!(reduction.terms().keys().all(|key| expected.contains(key)));
        }
    }
    // Moving the contributing inputs out of the way cannot affect application
    // or a new plain refinement: all member native sessions are owned locally.
    let hidden_peer = peer.0.with_extension("hidden");
    std::fs::rename(&peer.0, &hidden_peer).unwrap();
    let again = Scratch::new();
    let repeated = master_refine_published_artifact(
        &output.0,
        &MasterReductionOptions::new("unused", &again.0),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    std::fs::rename(&hidden_peer, &peer.0).unwrap();
    assert_eq!(repeated["collection"]["remaining_terminals"], 1);
    assert_eq!(
        load_master_reduction(&again.0)
            .unwrap()
            .collection()
            .remaining_terminals(),
        &expected
    );
}

#[test]
fn collection_boundary_interrupt_resumes_without_losing_finite_work() {
    let source = Scratch::new();
    let output = Scratch::new();
    publish_fixture(&source.0, &mut session());
    let mut options = MasterReductionOptions::new("unused", &output.0);
    let cancel = AtomicBool::new(false);
    let report = master_refine_published_artifact(&source.0, &options, &cancel, |event| {
        if event["stage"] == "terminal_collection" {
            cancel.store(true, Ordering::Relaxed);
        }
    })
    .unwrap();
    assert_eq!(report["status"], "paused");
    assert_eq!(report["finite_search_complete"], true);
    assert_eq!(report["finite_feedback"], true);
    assert_eq!(
        report["finite_feedback_recipe"],
        collection::FEEDBACK_RECIPE
    );
    assert_eq!(report["refinement_complete"], false);
    let finite = load_master_relation_session(&output.0).unwrap();
    assert!(finite.is_complete());
    let completed_rows = finite.statistics().completed_source_rows;
    options.resume = true;
    options.finite_feedback = Some(false);
    assert!(
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {},)
            .is_err()
    );
    options.finite_feedback = None;
    let resumed =
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(resumed["status"], "completed_nonminimal");
    assert_eq!(resumed["collection"]["status"], "completed");
    assert_eq!(resumed["collection"]["finite_feedback_stage"], "completed");
    assert!(resumed["collection"]["finite_feedback_rows"].is_u64());
    assert_eq!(
        load_master_relation_session(&output.0)
            .unwrap()
            .statistics()
            .completed_source_rows,
        completed_rows
    );
    assert_eq!(
        load_master_reduction(&output.0)
            .unwrap()
            .collection()
            .remaining_terminals()
            .len(),
        1
    );
}

#[test]
fn paused_collection_can_resume_after_original_peer_disappears() {
    let source = Scratch::new();
    let peer = Scratch::new();
    let output = Scratch::new();
    publish_fixture(&source.0, &mut named_session("resume-primary"));
    publish_fixture(&peer.0, &mut named_session("resume-peer"));
    let mut options = MasterReductionOptions::new("unused", &output.0);
    options.collection_artifacts = vec![peer.0.clone()];
    let cancel = AtomicBool::new(false);
    let paused = master_refine_published_artifact(&source.0, &options, &cancel, |event| {
        if event["stage"] == "terminal_collection" {
            cancel.store(true, Ordering::Relaxed);
        }
    })
    .unwrap();
    assert_eq!(paused["status"], "paused");
    let hidden = peer.0.with_extension("hidden");
    std::fs::rename(&peer.0, &hidden).unwrap();
    options.resume = true;
    let completed =
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {});
    std::fs::rename(hidden, &peer.0).unwrap();
    let completed = completed.unwrap();
    assert_eq!(completed["collection"]["families"], 2);
    assert_eq!(
        load_master_reduction(&output.0)
            .unwrap()
            .collection()
            .remaining_terminals()
            .len(),
        1
    );
}

#[test]
fn missing_or_modified_collection_payload_cannot_claim_application() {
    let source = Scratch::new();
    let output = Scratch::new();
    publish_fixture(&source.0, &mut session());
    let report = master_refine_published_artifact(
        &source.0,
        &MasterReductionOptions::new("unused", &output.0),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    let path = output
        .0
        .join(report["collection_state"]["file"].as_str().unwrap());
    let mut bytes = std::fs::read(&path).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    std::fs::write(&path, bytes).unwrap();
    assert!(load_master_reduction(&output.0).is_err());
    let mut missing = report;
    missing["collection_state"] = Value::Null;
    write_json(&output.0.join("latest.json"), &missing).unwrap();
    assert!(master_reduction_inspect(&output.0).is_err());
}

#[test]
fn legacy_finite_input_upgrades_but_collected_outputs_cannot_be_downgraded() {
    let source = Scratch::new();
    let output = Scratch::new();
    let native = session();
    let options = MasterReductionOptions::new("unused", &source.0);
    let mut report =
        json!({"schema":SCHEMA,"checkpoint":{"generation":0},"inputs":portable_inputs(&source.0)});
    save(
        &options,
        &native,
        &mut report,
        "published_unrefined",
        Instant::now(),
        &|_| {},
    )
    .unwrap();
    report["schema"] = json!(LEGACY_SOURCE_SCHEMA);
    write_json(&source.0.join("artifact.json"), &report).unwrap();
    write_json(&source.0.join("latest.json"), &report).unwrap();
    assert!(load_master_relation_session(&source.0).is_ok());
    let mut collected = master_refine_published_artifact(
        &source.0,
        &MasterReductionOptions::new("unused", &output.0),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert_eq!(collected["schema"], SCHEMA);
    collected["schema"] = json!(LEGACY_SOURCE_SCHEMA);
    write_json(&output.0.join("latest.json"), &collected).unwrap();
    assert!(master_reduction_inspect(&output.0).is_err());
    assert!(load_master_reduction(&output.0).is_err());
}

#[test]
fn extension_keeps_collected_peer_maps_without_new_diagonal_search() {
    let primary = Scratch::new();
    let peer = Scratch::new();
    let refined = Scratch::new();
    publish_fixture(&primary.0, &mut named_session("extension-primary"));
    publish_fixture(&peer.0, &mut named_session("extension-peer"));
    let mut options = MasterReductionOptions::new("unused", &refined.0);
    options.collection_artifacts = vec![peer.0.clone()];
    master_refine_published_artifact(&primary.0, &options, &AtomicBool::new(false), |_| {})
        .unwrap();
    let prior = load_master_reduction(&refined.0).unwrap();
    let extension = Scratch::new();
    let mut current = load_master_relation_session(&refined.0).unwrap();
    current
        .extend(&BTreeSet::from([IntegralKey::try_new([4]).unwrap()]), 0)
        .unwrap();
    let mut report = json!({"schema":SCHEMA,"operation":"publish","checkpoint":{"generation":0},"inputs":portable_inputs(&extension.0)});
    collection::inherit(&refined.0, &extension.0, &mut report).unwrap();
    let mut options = MasterReductionOptions::new("unused", &extension.0);
    options.operation = MasterReductionOperation::Publish;
    execute_session(
        &options,
        &mut current,
        &mut report,
        &AtomicBool::new(false),
        &|_| {},
        Instant::now(),
    )
    .unwrap();
    let loaded = load_master_reduction(&extension.0).unwrap();
    for source in std::iter::once(prior.primary_session()).chain(prior.member_sessions().iter()) {
        for key in source.raw_terminals() {
            assert_eq!(
                loaded
                    .apply_terminal(source.family_owner(), key)
                    .unwrap()
                    .terms(),
                prior
                    .apply_terminal(source.family_owner(), key)
                    .unwrap()
                    .terms()
            );
        }
    }
    assert!(!current.is_complete());
    assert_eq!(
        loaded.collection().statistics().terminal_equations,
        prior.collection().statistics().terminal_equations
    );
    assert_eq!(report["collection"]["finite_feedback_enabled"], false);
    assert_eq!(
        loaded.collection().statistics().finite_feedback_rows,
        prior.collection().statistics().finite_feedback_rows
    );
    assert_eq!(
        loaded.collection().statistics().finite_feedback_equations,
        prior.collection().statistics().finite_feedback_equations
    );
    assert!(
        loaded
            .apply_terminal(current.family_owner(), &IntegralKey::try_new([4]).unwrap())
            .is_ok()
    );
}

#[test]
fn disabled_feedback_has_a_bound_baseline_and_repeated_resume_is_a_noop() {
    let source = Scratch::new();
    let output = Scratch::new();
    publish_fixture(&source.0, &mut session());
    let mut options = MasterReductionOptions::new("unused", &output.0);
    options.finite_feedback = Some(false);
    let baseline =
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(baseline["finite_feedback"], false);
    assert_eq!(baseline["collection"]["finite_feedback_equations"], 0);
    assert_eq!(baseline["collection"]["finite_feedback_stage"], "disabled");
    options.resume = true;
    options.finite_feedback = None;
    let repeated =
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(repeated["checkpoint"], baseline["checkpoint"]);
    assert_eq!(repeated["collection_state"], baseline["collection_state"]);
    options.finite_feedback = Some(true);
    assert!(
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {},)
            .is_err()
    );
    let upgraded = Scratch::new();
    options.directory = upgraded.0.clone();
    options.resume = false;
    let upgraded =
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_ne!(
        upgraded["refinement_binding"],
        baseline["refinement_binding"]
    );
    assert_eq!(upgraded["finite_feedback"], true);
}

#[test]
fn feedback_resource_snapshot_rejects_changes_and_legacy_baseline_remains_readable() {
    let source = Scratch::new();
    let output = Scratch::new();
    publish_fixture(&source.0, &mut session());
    let mut options = MasterReductionOptions::new("unused", &output.0);
    options.finite_feedback = Some(false);
    let report =
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(
        report["collection_limits"]["recipe"],
        "terminal-collection-limits-v2"
    );
    assert!(report["collection_limits"]["bounds"]["finite_feedback"].is_object());
    let mut changed = report.clone();
    changed["collection_limits"]["bounds"]["finite_feedback"]["max_source_rows"] = json!(1);
    write_json(&output.0.join("latest.json"), &changed).unwrap();
    assert!(load_master_reduction(&output.0).is_err());
    let mut legacy = report;
    legacy["collection_limits"]["recipe"] = json!("terminal-collection-limits-v1");
    legacy["collection_limits"]["bounds"]
        .as_object_mut()
        .unwrap()
        .remove("finite_feedback");
    write_json(&output.0.join("latest.json"), &legacy).unwrap();
    assert!(load_master_reduction(&output.0).is_ok());
}
