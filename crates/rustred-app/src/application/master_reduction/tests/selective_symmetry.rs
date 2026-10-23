use super::super::selective_symmetry as selective;
use super::*;
use std::cell::Cell;

fn options(directory: &Path) -> MasterReductionOptions {
    let mut options = MasterReductionOptions::new("unused", directory);
    options.sector_symmetry_assistance = true;
    options.sector_symmetry_saved_projection = Some(false);
    options.selective_symmetry_feedback = true;
    options.threads = 2;
    options
}

fn fixture(directory: &Path) -> (MasterReductionOptions, TerminalRelationSession, Value) {
    let options = options(directory);
    let native = TerminalRelationSession::new(
        session().family_owner().clone(),
        BTreeSet::from([
            IntegralKey::try_new([1]).unwrap(),
            IntegralKey::try_new([3]).unwrap(),
        ]),
        0,
        MasterNormalizationProfile::RamControlledV1.relation_limits(),
    )
    .unwrap();
    let mut report = json!({"schema":SCHEMA,"scope_binding":"selective-real-ibp-fixture",
        "operation":"refine","checkpoint":{"generation":0},"inventory":{"complete":true},
        "inputs":portable_inputs(directory)});
    selective::record(&options, &mut report);
    (options, native, report)
}

fn start(
    directory: &Path,
    cancel: &AtomicBool,
    observer: impl Fn(Value),
) -> (MasterReductionOptions, TerminalRelationSession, Value) {
    let (options, mut native, mut report) = fixture(directory);
    assistance::configure(&options, &mut native, &mut report).unwrap();
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
        cancel,
        &observer,
        Instant::now(),
    )
    .unwrap();
    (options, native, report)
}

#[test]
fn selected_native_corner_finds_a_new_exact_relation_without_promoting_masters() {
    let scratch = Scratch::new();
    let (_, native, report) = start(&scratch.0, &AtomicBool::new(false), |_| {});
    assert_eq!(report["status"], "completed_nonminimal");
    assert!(selective::complete(&report));
    assert_eq!(native.raw_terminals().len(), 2);
    assert_eq!(native.remaining_terminals().len(), 1);
    assert!(
        report["selective_symmetry_feedback"]["completed_rounds"]
            .as_u64()
            .unwrap()
            >= 1
    );
    assert_eq!(
        report["selective_symmetry_feedback"]["rounds"][0]["terminal_rank_epoch"],
        2
    );
    assert_eq!(
        report["selective_symmetry_feedback"]["rounds"][0]["remaining_terminals"],
        1
    );
    let restored = load_master_relation_session(&scratch.0).unwrap();
    assert_eq!(
        restored.apply_all_terminals().unwrap(),
        native.apply_all_terminals().unwrap()
    );
    assert_eq!(restored.nonzero_conditions(), native.nonzero_conditions());
    selective::validate(&scratch.0, &restored, &report).unwrap();
}

#[test]
fn prepared_round_cold_resume_with_different_workers_keeps_its_frozen_parent() {
    let scratch = Scratch::new();
    let cancel = AtomicBool::new(false);
    let paused = Cell::new(false);
    let (mut options, _, report) = start(&scratch.0, &cancel, |event| {
        if event["selective_symmetry_feedback"]["stage"] == "prepared" {
            paused.set(true);
            cancel.store(true, Ordering::Relaxed);
        }
    });
    assert!(paused.get());
    assert_eq!(report["status"], "paused");
    assert_eq!(
        report["selective_symmetry_feedback"]["rounds"][0]["stage"],
        "prepared"
    );
    let before = report["selective_symmetry_feedback"]["rounds"][0].clone();
    let mut restored = load_master_relation_session(&scratch.0).unwrap();
    let mut report = report;
    options.resume = true;
    options.threads = 3;
    execute_session(
        &options,
        &mut restored,
        &mut report,
        &AtomicBool::new(false),
        &|_| {},
        Instant::now(),
    )
    .unwrap();
    assert!(selective::complete(&report));
    for field in [
        "binding",
        "selection",
        "sources",
        "normalization",
        "tasks",
        "parent_native_state",
        "parent_maps",
    ] {
        assert_eq!(
            report["selective_symmetry_feedback"]["rounds"][0][field],
            before[field]
        );
    }
    assert_eq!(restored.remaining_terminals().len(), 1);
    selective::validate(&scratch.0, &restored, &report).unwrap();
}

#[test]
fn executing_round_resumes_ordinary_prefix_and_rejects_forged_completion() {
    let scratch = Scratch::new();
    let cancel = AtomicBool::new(false);
    let (mut options, _, report) = start(&scratch.0, &cancel, |event| {
        if event["selective_symmetry_feedback"]["rounds"][0]["stage"] == "executing" {
            cancel.store(true, Ordering::Relaxed);
        }
    });
    assert_eq!(report["status"], "paused");
    let mut native = load_master_relation_session(&scratch.0).unwrap();
    let mut forged = report.clone();
    forged["selective_symmetry_feedback"]["rounds"][0]["stage"] = json!("complete");
    for key in [
        "selected_queue_exhausted",
        "source_jobs_complete",
        "ordinary_work_complete",
    ] {
        forged["selective_symmetry_feedback"][key] = json!(true);
    }
    assert!(selective::validate(&scratch.0, &native, &forged).is_err());
    let mut report = report;
    options.resume = true;
    options.threads = 1;
    execute_session(
        &options,
        &mut native,
        &mut report,
        &AtomicBool::new(false),
        &|_| {},
        Instant::now(),
    )
    .unwrap();
    assert!(selective::complete(&report));
    assert_eq!(native.remaining_terminals().len(), 1);
}

#[test]
fn mode_validation_is_opt_in_and_immutable_while_worker_count_is_not() {
    let mut options = MasterReductionOptions::new("unused", "unused");
    selective::validate_options(&options, None, MasterRefinementStrategy::Exact).unwrap();
    options.selective_symmetry_feedback = true;
    assert!(selective::validate_options(&options, None, MasterRefinementStrategy::Exact).is_err());
    options.sector_symmetry_assistance = true;
    selective::validate_options(&options, None, MasterRefinementStrategy::Exact).unwrap();
    assert!(
        selective::validate_options(&options, None, MasterRefinementStrategy::ModularCircuits)
            .is_err()
    );
    let report = json!({"selective_symmetry_feedback_enabled":true});
    options.threads = 17;
    selective::validate_options(&options, Some(&report), MasterRefinementStrategy::Exact).unwrap();
    options.selective_symmetry_feedback = false;
    assert!(
        selective::validate_options(&options, Some(&report), MasterRefinementStrategy::Exact)
            .is_err()
    );
}

#[test]
fn prepared_partial_export_is_portable_and_new_search_requires_relations_only_fresh() {
    let source = Scratch::new();
    let output = Scratch::new();
    let moved = Scratch::new();
    let cancel = AtomicBool::new(false);
    let (_, native, report) = start(&source.0, &cancel, |event| {
        if event["selective_symmetry_feedback"]["stage"] == "prepared" {
            cancel.store(true, Ordering::Relaxed);
        }
    });
    assert_eq!(report["status"], "paused");
    let expected = native.apply_all_terminals().unwrap();
    let mut options = MasterReductionOptions::new("unused", &output.0);
    options.operation = MasterReductionOperation::Publish;
    let publication =
        master_publish_partial_refinement(&source.0, &options, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(publication["status"], "published_unrefined");
    assert!(!selective::complete(&publication));
    std::fs::rename(&source.0, moved.0.join("old-source")).unwrap();
    let restored = load_master_reduction(&output.0).unwrap();
    assert_eq!(
        restored.primary_session().apply_all_terminals().unwrap(),
        expected
    );
    let newround = Scratch::new();
    let mut options = MasterReductionOptions::new("unused", &newround.0);
    let error =
        master_refine_published_artifact(&output.0, &options, &AtomicBool::new(false), |_| {})
            .unwrap_err();
    assert!(error.to_string().contains("fresh-search"));
    options.fresh_search = true;
    let fresh =
        master_refine_published_artifact(&output.0, &options, &AtomicBool::new(true), |_| {})
            .unwrap();
    assert_eq!(fresh["status"], "paused");
    assert!(!selective::enabled(&fresh));
    let native = load_master_relation_session(&newround.0).unwrap();
    assert_eq!(native.apply_all_terminals().unwrap(), expected);
}

#[test]
fn real_native_new_support_chain_continues_across_no_rank_gain() {
    let scratch = Scratch::new();
    let options = options(&scratch.0);
    // Concrete caller fixture (same coordinates as three_loop_k6.toml), not
    // topology dispatch in the refinement engine. These are the 38 full-rank
    // unit-line sectors of this six-denominator family.
    let c = CoefficientContext::try_new(["d"]).unwrap();
    let momenta = [
        [1, 0, 0],
        [0, 1, 0],
        [0, 0, 1],
        [1, 0, -1],
        [1, -1, 0],
        [0, 1, -1],
    ];
    let family = Arc::new(
        IntegralFamily::new(
            "selective-native-chain-fixture",
            vec!["k1".into(), "k2".into(), "k3".into()],
            vec![],
            c.clone(),
            c.parameter("d").unwrap(),
            momenta
                .iter()
                .map(|q| {
                    AffineDenominator::new(
                        c.integer(-1),
                        (0..3)
                            .flat_map(|i| {
                                (i..3).map(move |j| q[i] * q[j] * if i == j { 1 } else { 2 })
                            })
                            .map(|n| c.integer(n))
                            .collect(),
                    )
                })
                .collect(),
            vec![],
            vec![c.zero(); 6],
        )
        .unwrap(),
    );
    let raw = [
        [0, 0, 1, 0, 1, 1],
        [0, 0, 1, 1, 0, 1],
        [0, 0, 1, 1, 1, 0],
        [0, 0, 1, 1, 1, 1],
        [0, 1, 0, 0, 1, 1],
        [0, 1, 0, 1, 0, 1],
        [0, 1, 0, 1, 1, 0],
        [0, 1, 0, 1, 1, 1],
        [0, 1, 1, 0, 1, 0],
        [0, 1, 1, 0, 1, 1],
        [0, 1, 1, 1, 0, 0],
        [0, 1, 1, 1, 0, 1],
        [0, 1, 1, 1, 1, 0],
        [0, 1, 1, 1, 1, 1],
        [1, 0, 0, 0, 1, 1],
        [1, 0, 0, 1, 0, 1],
        [1, 0, 0, 1, 1, 0],
        [1, 0, 0, 1, 1, 1],
        [1, 0, 1, 0, 0, 1],
        [1, 0, 1, 0, 1, 0],
        [1, 0, 1, 0, 1, 1],
        [1, 0, 1, 1, 0, 1],
        [1, 0, 1, 1, 1, 0],
        [1, 0, 1, 1, 1, 1],
        [1, 1, 0, 0, 0, 1],
        [1, 1, 0, 0, 1, 1],
        [1, 1, 0, 1, 0, 0],
        [1, 1, 0, 1, 0, 1],
        [1, 1, 0, 1, 1, 0],
        [1, 1, 0, 1, 1, 1],
        [1, 1, 1, 0, 0, 0],
        [1, 1, 1, 0, 0, 1],
        [1, 1, 1, 0, 1, 0],
        [1, 1, 1, 0, 1, 1],
        [1, 1, 1, 1, 0, 0],
        [1, 1, 1, 1, 0, 1],
        [1, 1, 1, 1, 1, 0],
        [1, 1, 1, 1, 1, 1],
    ]
    .into_iter()
    .map(|powers| IntegralKey::try_new(powers).unwrap())
    .collect();
    let mut native = TerminalRelationSession::new(
        family,
        raw,
        0,
        MasterNormalizationProfile::RamControlledV1.relation_limits(),
    )
    .unwrap();
    let raw = native.raw_terminals().clone();
    let mut report = json!({"schema":SCHEMA,"scope_binding":"selective-native-no-gain-chain",
        "operation":"refine","checkpoint":{"generation":0},"inventory":{"complete":true},
        "inputs":portable_inputs(&scratch.0)});
    selective::record(&options, &mut report);
    assistance::configure(&options, &mut native, &mut report).unwrap();
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
    let rounds = report["selective_symmetry_feedback"]["rounds"]
        .as_array()
        .unwrap();
    assert!(
        rounds.len() >= 2,
        "a new native support must be inspected even though the preceding round made no terminal-rank gain"
    );
    assert!(
        rounds.windows(2).any(|pair| pair[0]["terminal_rank_epoch"]
            == pair[0]["remaining_terminals"]
            && pair[0]["remaining_terminals"] == pair[1]["terminal_rank_epoch"]),
        "a no-rank-gain round must still admit a newly exposed support in the same epoch"
    );
    assert_eq!(native.raw_terminals(), &raw);
    assert!(selective::complete(&report));
    selective::validate(&scratch.0, &native, &report).unwrap();
}
