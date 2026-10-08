use super::*;
use rustred::algebra::CoefficientContext;
use rustred::family::{AffineDenominator, IntegralFamily, IntegralKey};
use std::collections::BTreeSet;
use std::sync::Arc;

#[path = "tests/inherited.rs"]
mod inherited_tests;
#[path = "tests/profile.rs"]
mod profile_tests;
#[path = "tests/collection.rs"]
mod collection_tests;
#[path = "tests/collection_audit.rs"]
mod collection_audit;

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rustred-master-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn session() -> TerminalRelationSession {
    let context = CoefficientContext::try_new(["d"]).unwrap();
    let family = Arc::new(
        IntegralFamily::new(
            "master-checkpoint",
            vec!["k".into()],
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                context.integer(-1),
                vec![context.integer(1)],
            )],
            vec![],
            vec![context.zero()],
        )
        .unwrap(),
    );
    TerminalRelationSession::new(
        family,
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
fn master_checkpoint_resume_preserves_rows_and_exact_application() {
    let scratch = Scratch::new();
    let options = MasterReductionOptions::new("unused", &scratch.0);
    let mut original = session();
    let cancel = AtomicBool::new(false);
    original.step(&cancel).unwrap();
    let mut report = json!({"schema":SCHEMA,"checkpoint":{"generation":0}});
    save(
        &options,
        &original,
        &mut report,
        "paused",
        Instant::now(),
        &|_| {},
    )
    .unwrap();
    let mut restored = load_master_relation_session(&scratch.0).unwrap();
    assert_eq!(restored.statistics(), original.statistics());
    for candidate in [&mut original, &mut restored] {
        while !candidate.is_complete() {
            candidate.step(&cancel).unwrap();
        }
    }
    assert_eq!(restored.statistics(), original.statistics());
    for key in original.raw_terminals() {
        assert_eq!(
            restored.apply_terminal(key).unwrap(),
            original.apply_terminal(key).unwrap()
        );
    }
}

#[test]
fn master_final_publication_is_idempotent_and_portable() {
    let scratch = Scratch::new();
    let mut native = session();
    let cancel = AtomicBool::new(false);
    while !native.is_complete() {
        native.step(&cancel).unwrap();
    }
    let options = MasterReductionOptions::new("unused", &scratch.0);
    let mut report = json!({"schema":SCHEMA,"checkpoint":{"generation":0}});
    save(
        &options,
        &native,
        &mut report,
        "completed_nonminimal",
        Instant::now(),
        &|_| {},
    )
    .unwrap();
    publish_final(&options, &mut report).unwrap();
    publish_final(&options, &mut report).unwrap();
    let moved = Scratch::new();
    for entry in std::fs::read_dir(&scratch.0).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), moved.0.join(entry.file_name())).unwrap();
    }
    assert_eq!(
        master_reduction_inspect(&moved.0).unwrap()["status"],
        "completed_nonminimal"
    );
    assert_eq!(
        load_master_relation_session(&moved.0).unwrap().statistics(),
        native.statistics()
    );
    let state = moved
        .0
        .join(report["native_state"]["file"].as_str().unwrap());
    let mut bytes = std::fs::read(&state).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    std::fs::write(state, bytes).unwrap();
    assert!(load_master_relation_session(&moved.0).is_err());
}

#[test]
fn master_writer_lock_releases_on_drop() {
    let scratch = Scratch::new();
    let first = storage::WriterLock::acquire(&scratch.0).unwrap();
    assert!(storage::WriterLock::acquire(&scratch.0).is_err());
    drop(first);
    assert!(storage::WriterLock::acquire(&scratch.0).is_ok());
}

#[test]
fn master_scope_table_distinguishes_caps_from_full_family() {
    let matching = crate::OwnerDomainMatchRequest::new(
        "{}".into(),
        json!({"queries":[{"max_numerator_rank":0,"power_bounds":{"max_power_difference":9}}]})
            .to_string(),
    );
    let scope = storage::scope_summary(&OwnerDomainWalkRequest::new(matching)).unwrap();
    assert_eq!(scope["max_starting_rank"], 0);
    assert_eq!(scope["max_starting_d"], 9);
    assert!(
        scope["meaning"]
            .as_str()
            .unwrap()
            .contains("not every integral")
    );
}

fn portable_inputs(directory: &Path) -> Value {
    std::fs::create_dir_all(directory.join("inputs")).unwrap();
    for (name, value) in [
        ("selection", json!({})),
        ("queries", json!({"queries":[]})),
        ("amendments", json!([])),
    ] {
        write_json(&directory.join(format!("inputs/{name}.json")), &value).unwrap();
    }
    json!({"selection":"inputs/selection.json","queries":"inputs/queries.json",
        "amendments":"inputs/amendments.json","payloads":[],
        "queries_blake3":storage::digest_file(&directory.join("inputs/queries.json")).unwrap(),
        "amendments_blake3":storage::digest_file(&directory.join("inputs/amendments.json")).unwrap(),
        "program_binding":storage::digest_file(&directory.join("inputs/selection.json")).unwrap()})
}

fn publish_fixture(directory: &Path, native: &mut TerminalRelationSession) -> Value {
    let mut options = MasterReductionOptions::new("unused", directory);
    options.operation = MasterReductionOperation::Publish;
    let mut report = json!({"schema":SCHEMA,"scope_binding":"test-scope",
        "operation":"publish","checkpoint":{"generation":0},
        "inventory":{"complete":true},"inputs":portable_inputs(directory)});
    execute_session(
        &options,
        native,
        &mut report,
        &AtomicBool::new(false),
        &|_| {},
        Instant::now(),
    )
    .unwrap();
    report
}

#[test]
fn publication_processes_no_source_rows_and_cold_applies_normalization() {
    let scratch = Scratch::new();
    let mut native = session();
    let report = publish_fixture(&scratch.0, &mut native);
    assert_eq!(report["status"], "published_unrefined");
    assert_eq!(report["relation_rows"], 0);
    assert_eq!(
        report["capabilities"]["exact_current_terminal_substitutions"],
        true
    );
    assert_eq!(
        report["capabilities"]["routed_coefficient_application"],
        false
    );
    assert!(!native.is_complete());
    let restored = load_master_relation_session(&scratch.0).unwrap();
    let key = IntegralKey::try_new([2]).unwrap();
    assert_eq!(
        restored.apply_terminal(&key).unwrap(),
        std::collections::BTreeMap::from([(
            key,
            restored.family_owner().coefficient_context().one()
        )])
    );
}

#[test]
fn explicit_refinement_preserves_published_source_and_resumes_in_new_directory() {
    let source = Scratch::new();
    let report = publish_fixture(&source.0, &mut session());
    let source_bytes = std::fs::read(source.0.join("latest.json")).unwrap();
    let output = Scratch::new();
    let mut options = MasterReductionOptions::new("absent-checkpoint", &output.0);
    let paused =
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(true), |_| {})
            .unwrap();
    assert_eq!(paused["status"], "paused");
    assert_eq!(paused["relation_rows"], 0);
    options.resume = true;
    let completed =
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(completed["status"], "completed_nonminimal");
    assert_eq!(completed["remaining_terminals"], 1);
    assert_eq!(
        std::fs::read(source.0.join("latest.json")).unwrap(),
        source_bytes
    );
    assert_eq!(master_reduction_inspect(&source.0).unwrap(), report);
    assert_eq!(
        load_master_relation_session(&source.0)
            .unwrap()
            .statistics()
            .completed_source_rows,
        0
    );
    // A completed checkpoint repairs an interrupted final-publication marker.
    std::fs::remove_file(output.0.join("artifact.json")).unwrap();
    master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(output.0.join("artifact.json").exists());
    options.seed_depth = 1;
    assert!(
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {})
            .is_err()
    );
    options.directory = source.0.join("nested");
    assert!(
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {})
            .is_err()
    );
    assert!(!options.directory.exists());
}

#[test]
fn publication_of_extended_scope_keeps_previous_exact_rows_without_new_sources() {
    let scratch = Scratch::new();
    let mut native = session();
    while !native.is_complete() {
        native.step(&AtomicBool::new(false)).unwrap();
    }
    let key = IntegralKey::try_new([2]).unwrap();
    let old_row = native.apply_terminal(&key).unwrap();
    let old_sources = native.statistics().completed_source_rows;
    native
        .extend(&BTreeSet::from([IntegralKey::try_new([3]).unwrap()]), 0)
        .unwrap();
    assert!(native.statistics().pending_rebuild_rows > 0);
    let report = publish_fixture(&scratch.0, &mut native);
    assert_eq!(report["status"], "published_unrefined");
    assert_eq!(native.statistics().completed_source_rows, old_sources);
    assert_eq!(native.statistics().pending_rebuild_rows, 0);
    assert_eq!(
        load_master_relation_session(&scratch.0)
            .unwrap()
            .apply_terminal(&key)
            .unwrap(),
        old_row
    );
}

#[test]
fn inspection_prefers_paused_checkpoint_over_stale_completed_marker() {
    let scratch = Scratch::new();
    let mut native = session();
    while !native.is_complete() {
        native.step(&AtomicBool::new(false)).unwrap();
    }
    publish_fixture(&scratch.0, &mut native);
    assert_eq!(
        master_reduction_inspect(&scratch.0).unwrap()["status"],
        "completed_nonminimal"
    );
    let mut latest = read_json(&scratch.0.join("latest.json")).unwrap();
    latest["status"] = json!("paused");
    write_json(&scratch.0.join("latest.json"), &latest).unwrap();
    assert_eq!(
        master_reduction_inspect(&scratch.0).unwrap()["status"],
        "paused"
    );
}

#[test]
fn refinement_rejects_mutated_portable_scope_without_publishing() {
    for name in ["queries", "amendments"] {
        let source = Scratch::new();
        publish_fixture(&source.0, &mut session());
        write_json(&source.0.join(format!("inputs/{name}.json")), &json!([])).unwrap();
        // For the empty original amendment index use a nonempty changed index.
        if name == "amendments" {
            write_json(&source.0.join("inputs/amendments.json"), &json!([{}])).unwrap();
        }
        let output = Scratch::new();
        let options = MasterReductionOptions::new("unused", &output.0);
        assert!(
            master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {})
                .is_err()
        );
        assert!(!output.0.join("artifact.json").exists());
        assert_eq!(
            load_master_relation_session(&source.0)
                .unwrap()
                .statistics()
                .completed_source_rows,
            0
        );
    }
}

fn completed_policy_fixture(directory: &Path, assisted: bool) -> Value {
    completed_equation_policy_fixture(directory, assisted, false)
}

fn completed_equation_policy_fixture(directory: &Path, saved: bool, circuit: bool) -> Value {
    let mut options = MasterReductionOptions::new("unused", directory);
    options.saved_rule_assistance = saved;
    options.circuit_symmetry_assistance = circuit;
    let mut native = session();
    let mut report = json!({"schema":SCHEMA,"scope_binding":"test-scope",
        "operation":"refine","checkpoint":{"generation":0},
        "inventory":{"complete":true},"inputs":portable_inputs(directory)});
    assistance::configure(&options, &mut native, &mut report).unwrap();
    while !native.is_complete() {
        if saved || circuit {
            native
                .step_with_provider(&AtomicBool::new(false), |_| Ok(vec![]))
                .unwrap();
        } else {
            native.step(&AtomicBool::new(false)).unwrap();
        }
    }
    save(
        &options,
        &native,
        &mut report,
        "completed_nonminimal",
        Instant::now(),
        &|_| {},
    )
    .unwrap();
    publish_final(&options, &mut report).unwrap();
    report
}

fn circuit_session() -> TerminalRelationSession {
    let context = CoefficientContext::try_new(["d"]).unwrap();
    let momenta = [
        [1, 0, 0],
        [0, 1, 0],
        [0, 0, 1],
        [1, 1, 1],
        [1, 0, 1],
        [0, 1, 1],
    ];
    let family = Arc::new(
        IntegralFamily::new(
            "master-circuit-equations",
            vec!["k1".into(), "k2".into(), "k3".into()],
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            momenta
                .iter()
                .map(|q| {
                    AffineDenominator::new(
                        context.integer(-1),
                        (0..3)
                            .flat_map(|i| {
                                (i..3).map(move |j| q[i] * q[j] * if i == j { 1 } else { 2 })
                            })
                            .map(|n| context.integer(n))
                            .collect(),
                    )
                })
                .collect(),
            vec![],
            vec![context.zero(); 6],
        )
        .unwrap(),
    );
    TerminalRelationSession::new(
        family,
        BTreeSet::from([IntegralKey::try_new([1, 1, 1, 1, -2, 0]).unwrap()]),
        0,
        Default::default(),
    )
    .unwrap()
}

#[test]
fn circuit_only_prepares_without_candidate_inputs_and_resumes_exact_rows() {
    let scratch = Scratch::new();
    let mut options = MasterReductionOptions::new("unused", &scratch.0);
    let mut native = circuit_session();
    let target = native.raw_terminals().first().unwrap().clone();
    // No saved-owner selection, payload inventory or program binding exists.
    let mut report = json!({"schema":SCHEMA,"checkpoint":{"generation":0}});
    assistance::configure(&options, &mut native, &mut report).unwrap();
    assert!(
        assistance::prepare(
            &options,
            &native,
            &mut report,
            &AtomicBool::new(false),
            &|_| {},
            Instant::now()
        )
        .unwrap()
        .is_none()
    );
    assert!(report["circuit_equation_preparation"].is_null());
    options.circuit_symmetry_assistance = true;
    options.containing_sector_depth = 1;
    assistance::configure(&options, &mut native, &mut report).unwrap();
    let mut provider = assistance::prepare(
        &options,
        &native,
        &mut report,
        &AtomicBool::new(false),
        &|_| {},
        Instant::now(),
    )
    .unwrap()
    .unwrap();
    let rows = provider(&target).unwrap();
    assert!(!rows.is_empty());
    assert!(rows.iter().any(|row| row.terms.len() == 27));
    assert_eq!(
        report["circuit_equation_preparation"]["verified_generators"],
        3
    );
    assert_eq!(
        report["circuit_equation_preparation"]["input_keys"]
            .as_u64()
            .unwrap() as usize,
        native
            .raw_terminals()
            .union(native.normalization().canonical_terminals())
            .count()
    );
    assert_eq!(report["saved_rule_assistance"], false);
    assert_eq!(report["circuit_symmetry_assistance"], true);
    assert!(
        report["relation_authority"]
            .as_str()
            .unwrap()
            .contains("no candidate-rule authority")
    );
    assert!(
        provider(&IntegralKey::try_new([1, 1, 1, 1, -3, 0]).unwrap())
            .unwrap()
            .is_empty()
    );
    native
        .step_with_provider(&AtomicBool::new(false), &mut provider)
        .unwrap();
    save(
        &options,
        &native,
        &mut report,
        "paused",
        Instant::now(),
        &|_| {},
    )
    .unwrap();
    let original = native.statistics();
    let mut restored = load_master_relation_session(&scratch.0).unwrap();
    options.resume = true;
    assistance::configure(&options, &mut restored, &mut report).unwrap();
    assert_eq!(restored.statistics(), original);
    let mut resumed_provider = assistance::prepare(
        &options,
        &restored,
        &mut report,
        &AtomicBool::new(false),
        &|_| {},
        Instant::now(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(resumed_provider(&target).unwrap(), rows);
    assert_eq!(restored.raw_terminals(), native.raw_terminals());
    options.circuit_symmetry_assistance = false;
    assert!(assistance::configure(&options, &mut restored, &mut report).is_err());
    assert_eq!(restored.statistics(), original);
}

#[test]
fn circuit_mode_switches_are_durable_even_when_saved_mode_is_unchanged() {
    for (old_saved, old_circuit, saved, circuit) in [
        (false, false, false, true),
        (false, true, false, false),
        (true, false, true, true),
        (true, true, true, false),
        (true, false, false, true),
        (false, true, true, false),
    ] {
        let source = Scratch::new();
        completed_equation_policy_fixture(&source.0, old_saved, old_circuit);
        let original = load_master_relation_session(&source.0).unwrap();
        let output = Scratch::new();
        let mut options = MasterReductionOptions::new("unused", &output.0);
        options.saved_rule_assistance = saved;
        options.circuit_symmetry_assistance = circuit;
        let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            master_refine_published_artifact(
                &source.0,
                &options,
                &AtomicBool::new(false),
                |event| {
                    if event["event"] == "master_reduction_checkpoint" {
                        panic!("durable policy checkpoint");
                    }
                },
            )
        }));
        assert!(interrupted.is_err());
        let mut report = master_reduction_inspect(&output.0).unwrap();
        let mut restored = load_master_relation_session(&output.0).unwrap();
        assert_eq!(report["finite_search_restarted_for_policy"], true);
        assert_eq!(report["saved_rule_assistance"], saved);
        assert_eq!(report["circuit_symmetry_assistance"], circuit);
        assert_eq!(restored.assistance_binding().is_some(), saved || circuit);
        assert_eq!(restored.statistics().completed_source_rows, 0);
        assert_eq!(restored.raw_terminals(), original.raw_terminals());
        options.resume = true;
        assistance::configure(&options, &mut restored, &mut report).unwrap();
        options.circuit_symmetry_assistance = !circuit;
        assert!(assistance::configure(&options, &mut restored, &mut report).is_err());
        assert_eq!(
            load_master_relation_session(&source.0).unwrap().statistics(),
            original.statistics()
        );
    }
}

#[test]
fn policy_restart_preserves_containing_sources_and_resets_per_phase_report() {
    let scratch = Scratch::new();
    let mut options = MasterReductionOptions::new("unused", &scratch.0);
    options.containing_sector_depth = 1;
    let mut native = circuit_session();
    let mut report = json!({"schema":SCHEMA,"checkpoint":{"generation":0}});
    assistance::configure(&options, &mut native, &mut report).unwrap();
    let seeds = native.statistics().seeds;
    assert!(seeds > 1);
    native.step(&AtomicBool::new(false)).unwrap();
    options.containing_sector_depth = 0;
    options.circuit_symmetry_assistance = true;
    assistance::configure(&options, &mut native, &mut report).unwrap();
    assert_eq!(native.statistics().completed_source_rows, 0);
    assert_eq!(native.statistics().seeds, seeds);
    assert_eq!(report["containing_sector_depth"], 1);
    assert_eq!(report["requested_containing_sector_depth"], 0);
    assert_eq!(report["finite_search_restarted_for_policy"], true);
    let binding = native.assistance_binding().unwrap().to_owned();
    assistance::configure(&options, &mut native, &mut report).unwrap();
    assert_eq!(report["finite_search_restarted_for_policy"], false);
    assert_eq!(native.assistance_binding(), Some(binding.as_str()));
    native
        .extend(
            &BTreeSet::from([IntegralKey::try_new([1, 1, 1, 1, -3, 0]).unwrap()]),
            0,
        )
        .unwrap();
    assistance::configure(&options, &mut native, &mut report).unwrap();
    assert_ne!(native.assistance_binding(), Some(binding.as_str()));
    assert_eq!(report["finite_search_restarted_for_policy"], true);
}

#[test]
fn policy_switches_are_resumable_at_the_first_durable_checkpoint() {
    for assisted in [false, true] {
        let source = Scratch::new();
        completed_policy_fixture(&source.0, !assisted);
        let original = load_master_relation_session(&source.0).unwrap();
        assert!(original.statistics().completed_source_rows > 0);
        let output = Scratch::new();
        let mut options = MasterReductionOptions::new("unused", &output.0);
        options.saved_rule_assistance = assisted;
        options.seed_depth = 1;
        // Simulate termination immediately after the first durable manifest,
        // before provider preparation or any finite-search work can happen.
        let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            master_refine_published_artifact(
                &source.0,
                &options,
                &AtomicBool::new(false),
                |event| {
                    if event["event"] == "master_reduction_checkpoint" {
                        panic!("interrupt after first durable checkpoint");
                    }
                },
            )
        }));
        assert!(interrupted.is_err());
        let mut report = master_reduction_inspect(&output.0).unwrap();
        let mut resumed = load_master_relation_session(&output.0).unwrap();
        assert_eq!(report["checkpoint"]["generation"], 1);
        assert_eq!(report["saved_rule_assistance"], assisted);
        assert_eq!(report["finite_search_restarted_for_policy"], true);
        assert_eq!(resumed.assistance_binding().is_some(), assisted);
        assert_eq!(resumed.raw_terminals(), original.raw_terminals());
        assert_eq!(resumed.statistics().completed_source_rows, 0);
        assert_eq!(resumed.statistics().seed_depth, 1);
        options.resume = true;
        let before = resumed.statistics();
        assistance::configure(&options, &mut resumed, &mut report).unwrap();
        assert_eq!(resumed.statistics(), before);
        while !resumed.is_complete() {
            if assisted {
                resumed
                    .step_with_provider(&AtomicBool::new(false), |_| Ok(vec![]))
                    .unwrap();
            } else {
                resumed.step(&AtomicBool::new(false)).unwrap();
            }
        }
        assert_eq!(resumed.statistics().remaining_terminals, 1);
        // The unassisted direction also exercises the public resume path.
        if !assisted {
            let completed = master_refine_published_artifact(
                &source.0,
                &options,
                &AtomicBool::new(false),
                |_| {},
            )
            .unwrap();
            assert_eq!(completed["status"], "completed_nonminimal");
            assert_eq!(completed["saved_rule_assistance"], false);
        }
        assert_eq!(
            load_master_relation_session(&source.0).unwrap().statistics(),
            original.statistics()
        );
    }
}

#[test]
fn resume_rejects_policy_and_binding_mismatches_without_resetting_native_work() {
    for assisted in [false, true] {
        let scratch = Scratch::new();
        let mut report = completed_policy_fixture(&scratch.0, assisted);
        let mut native = load_master_relation_session(&scratch.0).unwrap();
        let before = native.statistics();
        let mut options = MasterReductionOptions::new("unused", &scratch.0);
        options.resume = true;
        options.saved_rule_assistance = !assisted;
        assert!(assistance::configure(&options, &mut native, &mut report).is_err());
        assert_eq!(native.statistics(), before);
        options.saved_rule_assistance = assisted;
        report["saved_rule_assistance"] = json!(!assisted);
        assert!(assistance::configure(&options, &mut native, &mut report).is_err());
        assert_eq!(native.statistics(), before);
        report["saved_rule_assistance"] = json!(assisted);
        if assisted {
            report["inputs"]["program_binding"] = json!("different-program");
            assert!(assistance::configure(&options, &mut native, &mut report).is_err());
            assert_eq!(native.statistics(), before);
        }
    }
}

#[test]
fn publication_rejects_assisted_import_without_discarding_existing_refinements() {
    let scratch = Scratch::new();
    let mut report = completed_policy_fixture(&scratch.0, true);
    let mut native = load_master_relation_session(&scratch.0).unwrap();
    let before = native.statistics();
    let rules = native.terminal_rules();
    let conditions = native.nonzero_conditions().to_vec();
    let mut options = MasterReductionOptions::new("unused", &scratch.0);
    options.operation = MasterReductionOperation::Publish;
    assert!(assistance::configure(&options, &mut native, &mut report).is_err());
    assert_eq!(native.statistics(), before);
    assert_eq!(native.terminal_rules(), rules);
    assert_eq!(native.nonzero_conditions(), conditions);
    assert!(native.assistance_binding().is_some());
    assert_ne!(report["finite_search_restarted_for_policy"], true);
}

#[test]
fn publication_stage_reports_replay_not_frozen_assistance_work() {
    let scratch = Scratch::new();
    let mut report = completed_policy_fixture(&scratch.0, true);
    let mut native = load_master_relation_session(&scratch.0).unwrap();
    native
        .extend(&BTreeSet::from([IntegralKey::try_new([5]).unwrap()]), 0)
        .unwrap();
    assert!(native.statistics().pending_assistance_keys > 0);
    assert!(native.statistics().pending_rebuild_rows > 0);
    update_stats(&mut report, &native);
    assert_eq!(report["stage"], "assisted_equations");
    report["operation"] = json!("publish");
    update_stats(&mut report, &native);
    assert_eq!(report["stage"], "elimination");
    while native.statistics().pending_rebuild_rows > 0 {
        native.step_rebuild_only(&AtomicBool::new(false)).unwrap();
    }
    update_stats(&mut report, &native);
    assert_eq!(report["stage"], "relations");
    assert!(native.statistics().pending_assistance_keys > 0);
}

#[test]
fn containing_sector_policy_is_durable_and_resume_cannot_change_it() {
    let scratch = Scratch::new();
    let mut native = session();
    let mut report = json!({"schema":SCHEMA,"checkpoint":{"generation":0}});
    let mut options = MasterReductionOptions::new("unused", &scratch.0);
    options.containing_sector_depth = 1;
    assistance::configure(&options, &mut native, &mut report).unwrap();
    assert_eq!(report["containing_sector_depth"], 1);
    assert_eq!(report["requested_containing_sector_depth"], 1);
    // This one-loop control has no inactive slots; the strategy is recorded
    // without inventing any new terminal or source.
    assert_eq!(report["added_containing_sector_seeds"], 0);
    save(
        &options,
        &native,
        &mut report,
        "paused",
        Instant::now(),
        &|_| {},
    )
    .unwrap();
    let mut restored = load_master_relation_session(&scratch.0).unwrap();
    options.resume = true;
    assistance::configure(&options, &mut restored, &mut report).unwrap();
    let before = restored.statistics();
    options.containing_sector_depth = 0;
    assert!(assistance::configure(&options, &mut restored, &mut report).is_err());
    assert_eq!(restored.statistics(), before);
}
