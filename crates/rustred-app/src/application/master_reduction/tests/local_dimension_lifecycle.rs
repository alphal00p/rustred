//! Localized dimensional witnesses remain exact portable inherited authority.
use super::*;

#[test]
fn disabled_local_dimension_does_not_change_inherited_authority_shape() {
    let scratch = Scratch::new();
    let mut native = session();
    let mut options = MasterReductionOptions::new("unused", &scratch.0);
    options.saved_rule_assistance = true;
    let mut report = json!({"inputs":portable_inputs(&scratch.0)});
    assistance::configure(&options, &mut native, &mut report).unwrap();
    let mut inherited_report = report.clone();
    inherited::capture(&native, &report, &mut inherited_report).unwrap();
    let authority = &inherited_report["inherited_equation_authority"];
    assert!(
        authority
            .get("local_dimensional_recurrence_assistance")
            .is_none()
    );
    inherited::validate(&native, &inherited_report).unwrap();
    assert_eq!(
        authority["provider_binding"],
        format!(
            "saved-rule-assistance-v1:{}",
            report["inputs"]["program_binding"].as_str().unwrap()
        )
    );
}

#[test]
fn inherited_local_dimension_binding_remains_frozen_and_requires_publication_proof() {
    let scratch = Scratch::new();
    let mut native = session();
    let mut options = MasterReductionOptions::new("unused", &scratch.0);
    options.local_dimensional_recurrence_assistance = true;
    let mut report = json!({"inputs":portable_inputs(&scratch.0)});
    assistance::configure(&options, &mut native, &mut report).unwrap();
    let mut inherited_report = report.clone();
    inherited::capture(&native, &report, &mut inherited_report).unwrap();
    assert_eq!(
        inherited_report["inherited_equation_authority"]["local_dimensional_recurrence_assistance"],
        true
    );
    assert!(
        inherited_report["inherited_equation_authority"]["provider_binding"]
            .as_str()
            .unwrap()
            .starts_with("local-dimensional-recurrence-assistance-v1:")
    );
    // Before preparation no equation has entered the U: a cancelled initial
    // snapshot can retain the frozen provider identity without inventing proof.
    inherited::validate(&native, &inherited_report).unwrap();
    let mut changed = inherited_report.clone();
    changed["inherited_equation_authority"]["local_dimensional_recurrence_assistance"] =
        json!(false);
    assert!(inherited::validate(&native, &changed).is_err());
    let mut changed = inherited_report;
    changed["operation"] = json!("publish");
    // A cancelled-before-preparation round may be finalized safely; merely
    // changing its operation cannot manufacture an accepted source cohort.
    inherited::validate(&native, &changed).unwrap();
    changed["local_dimensional_recurrence_preparation"]["complete"] = json!(true);
    assert!(inherited::validate(&native, &changed).is_err());
}

#[test]
fn local_dimension_source_capsule_is_copied_with_portable_inputs() {
    let source = Scratch::new();
    let output = Scratch::new();
    let bytes = b"copy-only integrity probe, not an accepted mathematical witness";
    let name = "local-dimension-copy-probe.rrbin";
    write_file_atomically(&source.0.join(name), bytes, true).unwrap();
    let report = json!({"inputs":portable_inputs(&source.0),
    "local_dimensional_recurrence_assistance":true,
    "local_dimensional_recurrence_certificate":{
        "recipe":super::super::local_dimension::RECIPE,
        "file":name,"bytes":bytes.len(),
        "blake3":blake3::hash(bytes).to_hex().to_string()
    }});
    storage::clone_inputs(&source.0, &output.0, &report).unwrap();
    assert_eq!(std::fs::read(output.0.join(name)).unwrap(), bytes);
    let bad_recipe_output = Scratch::new();
    let mut changed = report.clone();
    changed["local_dimensional_recurrence_certificate"]["recipe"] =
        json!("unknown-local-source-recipe");
    assert!(storage::clone_inputs(&source.0, &bad_recipe_output.0, &changed).is_err());
    let bad_output = Scratch::new();
    let mut changed = report;
    changed["local_dimensional_recurrence_certificate"]["blake3"] = json!("0".repeat(64));
    assert!(storage::clone_inputs(&source.0, &bad_output.0, &changed).is_err());
}

#[test]
fn fresh_search_keeps_proved_maps_without_old_local_dimensional_search_state() {
    let source = Scratch::new();
    let source_report = publish_fixture(&source.0, &mut session());
    let artifact = load_master_reduction(&source.0).unwrap();
    let output = Scratch::new();
    let mut options = MasterReductionOptions::new("unused", &output.0);
    options.fresh_search = true;
    let mut report = source_report.clone();
    report["local_dimensional_recurrence_assistance"] = json!(true);
    report["local_dimensional_recurrence_preparation"] = json!({"old_cursor":12});
    report["local_dimensional_recurrence_certificate"] = json!({"old_file":"unused.rrbin"});
    let fresh = fresh::begin(
        &report.clone(),
        &artifact,
        &options,
        MasterNormalizationProfile::ConservativeV1,
        &mut report,
    )
    .unwrap();
    for field in [
        "local_dimensional_recurrence_assistance",
        "local_dimensional_recurrence_preparation",
        "local_dimensional_recurrence_certificate",
    ] {
        assert!(
            report.get(field).is_none(),
            "retained old search field {field}"
        );
    }
    assert!(fresh.retained_relations_binding().is_some());
    for key in fresh.raw_terminals() {
        let expected = artifact.apply_terminal(fresh.family_owner(), key).unwrap();
        let expected: std::collections::BTreeMap<_, _> = expected
            .terms()
            .iter()
            .map(|(key, value)| (key.integral().clone(), value.clone()))
            .collect();
        assert_eq!(fresh.apply_terminal(key).unwrap(), expected);
    }
}

#[test]
fn partial_publication_rejects_new_local_dimension_search() {
    let source = Scratch::new();
    let output = Scratch::new();
    let mut options = MasterReductionOptions::new("unused", &output.0);
    options.operation = MasterReductionOperation::Publish;
    options.local_dimensional_recurrence_assistance = true;
    let error =
        master_publish_partial_refinement(&source.0, &options, &AtomicBool::new(false), |_| {})
            .unwrap_err();
    assert!(error.to_string().contains("no new search"));
    assert!(!output.0.join("latest.json").exists());
}
