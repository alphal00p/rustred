//! Independent checks of the public saved-artifact collection lifecycle.
use super::*;

fn peer_session(name: &str) -> TerminalRelationSession {
    let c = CoefficientContext::try_new(["d"]).unwrap();
    let family = Arc::new(
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
        family,
        BTreeSet::from([IntegralKey::try_new([1]).unwrap(), IntegralKey::try_new([2]).unwrap()]),
        0,
        Default::default(),
    )
    .unwrap()
}

#[test]
fn completed_resume_must_authenticate_collection_before_republishing() {
    let source = Scratch::new();
    let output = Scratch::new();
    publish_fixture(&source.0, &mut session());
    let mut options = MasterReductionOptions::new("unused", &output.0);
    let report = master_refine_published_artifact(
        &source.0, &options, &AtomicBool::new(false), |_| {},
    ).unwrap();
    let publication = std::fs::read(output.0.join("artifact.json")).unwrap();
    let native = output.0.join(report["collection_state"]["file"].as_str().unwrap());
    let mut bytes = std::fs::read(&native).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    std::fs::write(&native, bytes).unwrap();
    options.resume = true;
    assert!(master_refine_published_artifact(
        &source.0, &options, &AtomicBool::new(false), |_| {},
    ).is_err());
    assert_eq!(std::fs::read(output.0.join("artifact.json")).unwrap(), publication);
}

#[test]
fn paused_collection_resumes_from_owned_peer_and_reports_real_phase_completion() {
    let source = Scratch::new();
    let peer = Scratch::new();
    let output = Scratch::new();
    publish_fixture(&source.0, &mut session());
    publish_fixture(&peer.0, &mut peer_session("collection-audit-owned-peer"));
    let mut options = MasterReductionOptions::new("unused", &output.0);
    options.collection_artifacts = vec![peer.0.clone()];
    let cancel = AtomicBool::new(false);
    let paused = master_refine_published_artifact(&source.0, &options, &cancel, |event| {
        if event["stage"] == "terminal_collection" {
            cancel.store(true, Ordering::Relaxed);
        }
    }).unwrap();
    assert_eq!(paused["status"], "paused");
    assert_eq!(paused["finite_search_complete"], true);
    assert_eq!(paused["refinement_complete"], false);
    let native_rows = load_master_relation_session(&output.0).unwrap().statistics().completed_source_rows;

    let hidden = peer.0.with_extension("peer-hidden");
    std::fs::rename(&peer.0, &hidden).unwrap();
    options.resume = true;
    let resumed = master_refine_published_artifact(
        &source.0, &options, &AtomicBool::new(false), |_| {},
    );
    std::fs::rename(&hidden, &peer.0).unwrap();
    let resumed = resumed.unwrap();
    assert_eq!(resumed["status"], "completed_nonminimal");
    assert_eq!(resumed["refinement_complete"], true);
    assert_eq!(resumed["collection"]["families"], 2);
    let loaded = load_master_reduction(&output.0).unwrap();
    assert_eq!(loaded.primary_session().statistics().completed_source_rows, native_rows);
    assert_eq!(loaded.collection().remaining_terminals().len(), 1);
    assert_eq!(loaded.member_sessions().len(), 1);
}

#[test]
fn explicit_publication_does_not_follow_newer_unusable_latest_manifest() {
    let source = Scratch::new();
    let first = Scratch::new();
    let second = Scratch::new();
    publish_fixture(&source.0, &mut session());
    let mut report = master_refine_published_artifact(
        &source.0,
        &MasterReductionOptions::new("unused", &first.0),
        &AtomicBool::new(false),
        |_| {},
    ).unwrap();
    // A published snapshot remains addressable even if an unrelated latest
    // checkpoint is unusable. The caller selected artifact.json explicitly.
    report["collection_state"] = Value::Null;
    write_json(&first.0.join("latest.json"), &report).unwrap();
    assert!(load_master_reduction(&first.0).is_err());
    let source_artifact = first.0.join("artifact.json");
    assert!(load_master_reduction(&source_artifact).is_ok());
    let final_report = master_refine_published_artifact(
        &source_artifact,
        &MasterReductionOptions::new("unused", &second.0),
        &AtomicBool::new(false),
        |_| {},
    ).unwrap();
    assert_eq!(final_report["remaining_terminals"], 1);
    assert!(load_master_reduction(&second.0).is_ok());
}
