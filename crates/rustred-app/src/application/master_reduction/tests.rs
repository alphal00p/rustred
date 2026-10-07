use super::*;
use rustred::algebra::CoefficientContext;
use rustred::family::{AffineDenominator, IntegralFamily, IntegralKey};
use std::collections::BTreeSet;
use std::sync::Arc;

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
    let mut restored = load_master_reduction(&scratch.0).unwrap();
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
        load_master_reduction(&moved.0).unwrap().statistics(),
        native.statistics()
    );
    let state = moved
        .0
        .join(report["native_state"]["file"].as_str().unwrap());
    let mut bytes = std::fs::read(&state).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    std::fs::write(state, bytes).unwrap();
    assert!(load_master_reduction(&moved.0).is_err());
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
    let restored = load_master_reduction(&scratch.0).unwrap();
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
        load_master_reduction(&source.0)
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
        load_master_reduction(&scratch.0)
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
            load_master_reduction(&source.0)
                .unwrap()
                .statistics()
                .completed_source_rows,
            0
        );
    }
}
