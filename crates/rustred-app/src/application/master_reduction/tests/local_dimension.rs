use super::super::local_dimension as local;
use super::*;
use std::cell::Cell;

fn assisted(directory: &Path) -> (MasterReductionOptions, TerminalRelationSession, Value) {
    let mut options = MasterReductionOptions::new("unused", directory);
    options.local_dimensional_recurrence_assistance = true;
    options.threads = 2;
    let mut native = session();
    let mut report = json!({"schema":SCHEMA,"scope_binding":"local-dimensional-fixture",
        "operation":"refine","checkpoint":{"generation":0},"inventory":{"complete":true},
        "inputs":portable_inputs(directory)});
    assistance::configure(&options, &mut native, &mut report).unwrap();
    (options, native, report)
}

#[test]
fn standalone_local_provider_is_native_replayed_and_keeps_original_master_roster() {
    let scratch = Scratch::new();
    let (options, mut native, mut report) = assisted(&scratch.0);
    let raw = native.raw_terminals().clone();
    save(
        &options,
        &native,
        &mut report,
        "running",
        Instant::now(),
        &|_| {},
    )
    .unwrap();
    execute_session(
        &options,
        &mut native,
        &mut report,
        &AtomicBool::new(false),
        &|_| {},
        Instant::now(),
    )
    .unwrap();
    assert_eq!(report["status"], "completed_nonminimal");
    assert_eq!(
        report["local_dimensional_recurrence_preparation"]["complete"],
        true
    );
    assert!(
        report["local_dimensional_recurrence_preparation"]["weighted_source_rows"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(
        report["local_dimensional_recurrence_preparation"]["equations"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(native.raw_terminals(), &raw);
    assert_eq!(
        report["local_dimensional_recurrence_preparation"]["shifted_dimension_runtime_keys"],
        false
    );
    let restored = load_master_relation_session(&scratch.0).unwrap();
    assert_eq!(
        restored.apply_all_terminals().unwrap(),
        native.apply_all_terminals().unwrap()
    );
    assert_eq!(restored.nonzero_conditions(), native.nonzero_conditions());
    let mut changed = report.clone();
    changed["local_dimensional_recurrence_certificate"]["inventory_binding"] = json!("bad");
    assert!(local::validate(&scratch.0, &restored, &changed).is_err());
    let mut changed = report.clone();
    changed["local_dimensional_recurrence_certificate"]["input_keys"] = json!(0);
    assert!(local::validate(&scratch.0, &restored, &changed).is_err());
    let mut changed = report;
    changed["local_dimensional_recurrence_certificate"] = Value::Null;
    assert!(local::validate(&scratch.0, &restored, &changed).is_err());
}

#[test]
fn local_preparation_checkpoint_resumes_with_changed_workers_and_one_frozen_capsule() {
    let scratch = Scratch::new();
    let (mut options, mut native, mut report) = assisted(&scratch.0);
    options.threads = 1;
    let cancel = AtomicBool::new(false);
    let stopped = Cell::new(false);
    save(
        &options,
        &native,
        &mut report,
        "running",
        Instant::now(),
        &|_| {},
    )
    .unwrap();
    execute_session(
        &options,
        &mut native,
        &mut report,
        &cancel,
        &|event| {
            if event["local_dimensional_recurrence_preparation"]["complete"] == true {
                stopped.set(true);
                cancel.store(true, Ordering::Relaxed);
            }
        },
        Instant::now(),
    )
    .unwrap();
    assert!(stopped.get());
    assert_eq!(report["status"], "paused");
    let certificate = report["local_dimensional_recurrence_certificate"].clone();
    let file = certificate["file"].as_str().unwrap();
    let source_bytes = std::fs::read(scratch.0.join(file)).unwrap();
    // Independently preparing with four workers must generate the same native
    // mathematical source capsule, not merely leave an old file untouched.
    let parallel_scratch = Scratch::new();
    let (mut parallel_options, parallel_native, mut parallel_report) =
        assisted(&parallel_scratch.0);
    parallel_options.threads = 4;
    let _ = local::prepare(
        &parallel_options,
        &parallel_native,
        &mut parallel_report,
        &AtomicBool::new(false),
        &|_| {},
        Instant::now(),
    )
    .unwrap();
    let parallel_certificate = &parallel_report["local_dimensional_recurrence_certificate"];
    assert_eq!(parallel_certificate, &certificate);
    assert_eq!(
        std::fs::read(
            parallel_scratch
                .0
                .join(parallel_certificate["file"].as_str().unwrap())
        )
        .unwrap(),
        source_bytes
    );
    let mut restored = load_master_relation_session(&scratch.0).unwrap();
    options.resume = true;
    options.threads = 4;
    execute_session(
        &options,
        &mut restored,
        &mut report,
        &AtomicBool::new(false),
        &|_| {},
        Instant::now(),
    )
    .unwrap();
    assert_eq!(report["status"], "completed_nonminimal");
    assert_eq!(
        report["local_dimensional_recurrence_certificate"],
        certificate
    );
    assert_eq!(restored.raw_terminals(), native.raw_terminals());
    assert_eq!(
        report["local_dimensional_recurrence_preparation"]["reserved_workers"],
        4
    );
    assert_eq!(std::fs::read(scratch.0.join(file)).unwrap(), source_bytes);
}

#[test]
fn local_prepared_partial_publication_cold_loads_after_source_directory_moves() {
    let source = Scratch::new();
    let output = Scratch::new();
    let moved = Scratch::new();
    let (options, mut native, mut report) = assisted(&source.0);
    let cancel = AtomicBool::new(false);
    save(
        &options,
        &native,
        &mut report,
        "running",
        Instant::now(),
        &|_| {},
    )
    .unwrap();
    execute_session(
        &options,
        &mut native,
        &mut report,
        &cancel,
        &|event| {
            if event["local_dimensional_recurrence_preparation"]["complete"] == true {
                cancel.store(true, Ordering::Relaxed);
            }
        },
        Instant::now(),
    )
    .unwrap();
    let expected = native.apply_all_terminals().unwrap();
    let mut options = MasterReductionOptions::new("unused", &output.0);
    options.operation = MasterReductionOperation::Publish;
    let publication =
        master_publish_partial_refinement(&source.0, &options, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(publication["status"], "published_unrefined");
    std::fs::rename(&source.0, moved.0.join("source-moved")).unwrap();
    let restored = load_master_relation_session(&output.0).unwrap();
    assert_eq!(restored.apply_all_terminals().unwrap(), expected);
    local::validate(&output.0, &restored, &publication).unwrap();
}

#[test]
fn local_mode_is_exact_independent_and_immutable_across_resume() {
    let mut options = MasterReductionOptions::new("unused", "unused");
    local::validate_options(&options, None, MasterRefinementStrategy::Exact).unwrap();
    options.local_dimensional_recurrence_assistance = true;
    local::validate_options(&options, None, MasterRefinementStrategy::Exact).unwrap();
    assert!(!options.sector_symmetry_assistance);
    assert!(
        local::validate_options(&options, None, MasterRefinementStrategy::ModularCircuits).is_err()
    );
    let report = json!({"local_dimensional_recurrence_assistance":true});
    local::validate_options(&options, Some(&report), MasterRefinementStrategy::Exact).unwrap();
    options.local_dimensional_recurrence_assistance = false;
    assert!(
        local::validate_options(&options, Some(&report), MasterRefinementStrategy::Exact).is_err()
    );
}

#[test]
fn cancelled_existing_local_certificate_replay_has_no_native_or_manifest_mutation() {
    let scratch = Scratch::new();
    let (options, native, mut report) = assisted(&scratch.0);
    let _ = local::prepare(
        &options,
        &native,
        &mut report,
        &AtomicBool::new(false),
        &|_| {},
        Instant::now(),
    )
    .unwrap();
    let before_report = report.clone();
    let before_native = native
        .to_native_bytes(profile::native_io_limits(None))
        .unwrap();
    let before_latest = std::fs::read(scratch.0.join("latest.json")).unwrap();
    let file = report["local_dimensional_recurrence_certificate"]["file"]
        .as_str()
        .unwrap()
        .to_owned();
    let before_certificate = std::fs::read(scratch.0.join(&file)).unwrap();
    let result = local::prepare(
        &options,
        &native,
        &mut report,
        &AtomicBool::new(true),
        &|_| {},
        Instant::now(),
    );
    assert!(result.is_err());
    assert_eq!(report, before_report);
    assert_eq!(
        native
            .to_native_bytes(profile::native_io_limits(None))
            .unwrap(),
        before_native
    );
    assert_eq!(
        std::fs::read(scratch.0.join("latest.json")).unwrap(),
        before_latest
    );
    assert_eq!(
        std::fs::read(scratch.0.join(file)).unwrap(),
        before_certificate
    );
}

#[test]
fn initial_local_and_sector_cohorts_have_separate_authority_in_one_finite_provider() {
    let scratch = Scratch::new();
    let (mut options, mut native, mut report) = assisted(&scratch.0);
    // Reconfigure a fresh session, rather than replacing an already bound map.
    native = session();
    options.sector_symmetry_assistance = true;
    options.sector_symmetry_saved_projection = Some(false);
    assistance::configure(&options, &mut native, &mut report).unwrap();
    let binding = native.assistance_binding().unwrap().to_owned();
    assert!(binding.contains("sector-symmetry-assistance-v1:"));
    assert!(binding.contains("local-dimensional-recurrence-assistance-v1:"));
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
    assert_eq!(prepared.kind(), "finite-immutable");
    assert!(report["sector_symmetry_certificate"].is_object());
    assert!(report["local_dimensional_recurrence_certificate"].is_object());
    assert_eq!(native.assistance_binding(), Some(binding.as_str()));
}
