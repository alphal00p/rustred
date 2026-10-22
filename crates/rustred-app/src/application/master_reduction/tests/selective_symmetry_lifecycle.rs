//! Optional feedback retains exact accepted maps across cold lifecycle boundaries.
use super::super::selective_symmetry as selective;
use super::*;

#[test]
fn selected_worklist_completion_requires_every_recorded_stage() {
    let mut report = json!({});
    assert!(selective::complete(&report));
    let mut options = MasterReductionOptions::new("unused", "unused");
    options.selective_symmetry_feedback = true;
    selective::record(&options, &mut report);
    assert!(selective::enabled(&report));
    assert!(!selective::complete(&report));
    for field in [
        "selected_queue_exhausted",
        "source_jobs_complete",
        "ordinary_work_complete",
    ] {
        report["selective_symmetry_feedback"][field] = json!(true);
    }
    assert!(selective::complete(&report));
    for field in [
        "selected_queue_exhausted",
        "source_jobs_complete",
        "ordinary_work_complete",
    ] {
        report["selective_symmetry_feedback"][field] = json!(false);
        assert!(!selective::complete(&report), "ignored unfinished {field}");
        report["selective_symmetry_feedback"][field] = json!(true);
    }
    options.selective_symmetry_feedback = false;
    selective::record(&options, &mut report);
    assert!(!selective::enabled(&report));
    assert!(report.get("selective_symmetry_feedback").is_none());
    assert!(report.get("selective_symmetry_feedback_enabled").is_none());
}

fn descriptor(directory: &Path, name: &str, bytes: &[u8]) -> Value {
    write_file_atomically(&directory.join(name), bytes, true).unwrap();
    json!({"file":name,"bytes":bytes.len(),
        "blake3":blake3::hash(bytes).to_hex().to_string()})
}

#[test]
fn selective_round_inheritance_copies_every_native_payload_and_rejects_mutations() {
    let source = Scratch::new();
    let output = Scratch::new();
    let fields = [
        "parent_native_state",
        "parent_maps",
        "selection",
        "sources",
        "normalization",
        "tasks",
    ];
    let mut round = json!({});
    for (index, field) in fields.iter().enumerate() {
        let name = format!("round-{index}.rrbin");
        round[field] = descriptor(&source.0, &name, field.as_bytes());
    }
    let report = json!({"selective_symmetry_feedback":{"rounds":[round]}});
    selective::inherit(&source.0, &output.0, &report).unwrap();
    for field in fields {
        let file = report["selective_symmetry_feedback"]["rounds"][0][field]["file"]
            .as_str()
            .unwrap();
        assert_eq!(
            std::fs::read(output.0.join(file)).unwrap(),
            field.as_bytes()
        );
        assert_eq!(
            std::fs::read(source.0.join(file)).unwrap(),
            field.as_bytes()
        );
    }
    let fresh = Scratch::new();
    let mut changed = report.clone();
    changed["selective_symmetry_feedback"]["rounds"][0]["tasks"]["blake3"] = json!("0".repeat(64));
    assert!(selective::inherit(&source.0, &fresh.0, &changed).is_err());
    let mut changed = report;
    changed["selective_symmetry_feedback"]["rounds"][0]["tasks"]["file"] = json!("../escape.rrbin");
    assert!(selective::inherit(&source.0, &fresh.0, &changed).is_err());
}

#[test]
fn fresh_relations_only_round_discards_feedback_search_but_not_exact_maps() {
    let source = Scratch::new();
    let source_report = publish_fixture(&source.0, &mut session());
    let artifact = load_master_reduction(&source.0).unwrap();
    let original = std::fs::read(source.0.join("latest.json")).unwrap();
    let output = Scratch::new();
    let mut options = MasterReductionOptions::new("unused", &output.0);
    options.fresh_search = true;
    let mut report = source_report.clone();
    report["selective_symmetry_feedback_enabled"] = json!(true);
    report["selective_symmetry_feedback"] = json!({"rounds":[{"old_cursor":17}]});
    let fresh = fresh::begin(
        &report.clone(),
        &artifact,
        &options,
        MasterNormalizationProfile::ConservativeV1,
        &mut report,
    )
    .unwrap();
    assert!(report.get("selective_symmetry_feedback_enabled").is_none());
    assert!(report.get("selective_symmetry_feedback").is_none());
    assert!(fresh.retained_relations_binding().is_some());
    assert_eq!(fresh.statistics().completed_source_rows, 0);
    assert_eq!(fresh.statistics().completed_assistance_rows, 0);
    for key in fresh.raw_terminals() {
        let expected = artifact.apply_terminal(fresh.family_owner(), key).unwrap();
        let expected: std::collections::BTreeMap<_, _> = expected
            .terms()
            .iter()
            .map(|(key, value)| (key.integral().clone(), value.clone()))
            .collect();
        assert_eq!(fresh.apply_terminal(key).unwrap(), expected);
    }
    assert_eq!(
        std::fs::read(source.0.join("latest.json")).unwrap(),
        original
    );
}

#[test]
fn partial_publication_accepts_ordinary_complete_search_with_optional_feedback_pending() {
    let source = Scratch::new();
    let output = Scratch::new();
    let mut options = MasterReductionOptions::new("unused", &source.0);
    options.sector_symmetry_assistance = true;
    options.sector_symmetry_saved_projection = Some(false);
    options.selective_symmetry_feedback = true;
    let mut native = session();
    let mut report = json!({"schema":SCHEMA,"scope_binding":"feedback-pending-scope",
        "operation":"refine","checkpoint":{"generation":0},
        "inventory":{"complete":true},"inputs":portable_inputs(&source.0)});
    assistance::configure(&options, &mut native, &mut report).unwrap();
    selective::record(&options, &mut report);
    let prepared = assistance::prepare_for_execution(
        &options,
        &native,
        &mut report,
        &AtomicBool::new(false),
        &|_| {},
        Instant::now(),
    )
    .unwrap()
    .unwrap();
    let mut provider = prepared.provider;
    while !native.is_complete() {
        native
            .step_with_provider(&AtomicBool::new(false), &mut provider)
            .unwrap();
    }
    let expected = native.apply_all_terminals().unwrap();
    let expected_guards = native.nonzero_conditions().to_vec();
    save(
        &options,
        &native,
        &mut report,
        "paused",
        Instant::now(),
        &|_| {},
    )
    .unwrap();
    let original = std::fs::read(source.0.join("latest.json")).unwrap();
    let mut publishing = MasterReductionOptions::new("unused", &output.0);
    publishing.operation = MasterReductionOperation::Publish;
    let publication =
        master_publish_partial_refinement(&source.0, &publishing, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(publication["status"], "published_unrefined");
    assert_eq!(publication["finite_search_complete"], false);
    assert_eq!(publication["refinement_complete"], false);
    assert!(selective::enabled(&publication));
    assert!(!selective::complete(&publication));
    let restored = load_master_reduction(&output.0).unwrap();
    assert_eq!(
        restored.primary_session().apply_all_terminals().unwrap(),
        expected
    );
    assert_eq!(
        restored.primary_session().nonzero_conditions(),
        expected_guards
    );
    assert_eq!(
        std::fs::read(source.0.join("latest.json")).unwrap(),
        original
    );
}
