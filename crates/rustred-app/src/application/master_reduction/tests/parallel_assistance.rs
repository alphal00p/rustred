use super::*;
use rustred::reduction::terminal_relations::{
    TerminalRelationExecutionOptions, TerminalRelationExecutionProgress, TerminalRelationExecutor,
};

#[test]
fn finite_execution_metadata_matches_the_complete_verified_witness_provider() {
    for (circuit, numerator) in [(true, false), (false, true), (true, true)] {
        let scratch = Scratch::new();
        let mut native = circuit_session();
        let mut options = MasterReductionOptions::new("unused", &scratch.0);
        options.circuit_symmetry_assistance = circuit;
        options.numerator_symmetry_assistance = numerator;
        let mut report = json!({"schema":SCHEMA,"checkpoint":{"generation":0}});
        assistance::configure(&options, &mut native, &mut report).unwrap();
        let binding = native.assistance_binding().unwrap().to_owned();
        let mut prepared = assistance::prepare_for_execution(
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
        let finite = prepared.finite.as_ref().unwrap();
        for key in native
            .raw_terminals()
            .union(native.normalization().canonical_terminals())
        {
            assert_eq!(
                (prepared.provider)(key).unwrap(),
                finite.rows().get(key).cloned().unwrap_or_default()
            );
        }
        // Only the full immutable witness services authorize the off-domain
        // empty guarantee, never an observation of past empty lookups.
        let unknown = IntegralKey::try_new([1, 1, 1, 1, -9, 0]).unwrap();
        assert!(!finite.rows().contains_key(&unknown));
        assert!((prepared.provider)(&unknown).unwrap().is_empty());
        assert_eq!(native.assistance_binding(), Some(binding.as_str()));
        if numerator {
            assert!(report["numerator_symmetry_certificate"].is_object());
        }
        assert_eq!(report["saved_rule_assistance"], false);
    }
}

#[test]
fn immutable_empty_provider_unlocks_parallel_ordinary_rows_without_changing_maps() {
    let scratch = Scratch::new();
    let mut assisted = session();
    assisted.extend(&BTreeSet::new(), 2).unwrap();
    let mut reference = TerminalRelationSession::from_native_bytes(
        &assisted
            .to_native_bytes(profile::native_io_limits(None))
            .unwrap(),
        Default::default(),
        profile::native_io_limits(None),
    )
    .unwrap();
    let mut options = MasterReductionOptions::new("unused", &scratch.0);
    options.numerator_symmetry_assistance = true;
    let mut report = json!({"schema":SCHEMA,"checkpoint":{"generation":0}});
    assistance::configure(&options, &mut assisted, &mut report).unwrap();
    let prepared = assistance::prepare_for_execution(
        &options,
        &assisted,
        &mut report,
        &AtomicBool::new(false),
        &|_| {},
        Instant::now(),
    )
    .unwrap()
    .unwrap();
    let finite = prepared.finite.unwrap();
    assert!(finite.rows().is_empty());
    let executor = TerminalRelationExecutor::new(TerminalRelationExecutionOptions::new(2)).unwrap();
    let cancel = AtomicBool::new(false);
    let mut observed = Vec::new();
    while !assisted.is_complete() {
        executor
            .step_with_finite_provider_observed(&mut assisted, &cancel, &finite, |progress| {
                observed.push(progress.stage)
            })
            .unwrap();
    }
    while !reference.is_complete() {
        reference.step(&cancel).unwrap();
    }
    assert_eq!(
        assisted.apply_all_terminals().unwrap(),
        reference.apply_all_terminals().unwrap()
    );
    assert_eq!(
        assisted.statistics().completed_source_rows,
        reference.statistics().completed_source_rows
    );
    // This tiny fixture may finish within the observer interval and expose
    // only its final ordered-merge snapshot; do not invent active-worker data.
    assert!(observed.iter().any(|stage| matches!(
        *stage,
        "preparing rows" | "mapping columns" | "parallel elimination" | "ordered merge"
    )));
}

#[test]
fn progress_distinguishes_reserved_workers_from_parallel_serial_and_provider_barriers() {
    for (stage, mode) in [
        ("preparing rows", "parallel"),
        ("parallel elimination", "parallel"),
        ("ordered merge", "merge"),
        ("mapping columns", "serial"),
        ("provider request", "barrier"),
        ("assistance equation", "serial"),
        ("filtering provider requests", "serial"),
    ] {
        let mut report = json!({"execution":{"workers":128,"reserved_workers":128}});
        observed_execution(
            &mut report,
            TerminalRelationExecutionProgress {
                stage,
                rows_total: 7,
                rows_prepared: 6,
                rows_eliminated: 5,
                rows_committed: 4,
                blocks_completed: 3,
                provider_keys_filtered: 2,
            },
            &|_| {},
            Instant::now(),
        );
        assert_eq!(report["execution"]["mode"], mode);
        assert_eq!(report["execution"]["stage"], stage);
        assert_eq!(report["execution"]["provider_keys_filtered"], 2);
        assert_eq!(report["execution"]["workers"], 128);
        assert!(report["execution"]["active_workers"].is_null());
    }
}

#[test]
fn arbitrary_mutable_provider_has_no_finite_empty_domain_promise() {
    let counter = std::rc::Rc::new(std::cell::Cell::new(0));
    let used = counter.clone();
    let mut prepared = assistance::PreparedAssistance {
        provider: Box::new(move |_| {
            used.set(used.get() + 1);
            Ok(vec![])
        }),
        finite: None,
    };
    assert_eq!(prepared.kind(), "dynamic");
    let key = IntegralKey::try_new([1]).unwrap();
    (prepared.provider)(&key).unwrap();
    (prepared.provider)(&key).unwrap();
    assert_eq!(counter.get(), 2);
    assert!(prepared.finite.is_none());
}

#[test]
fn publication_and_pause_phases_do_not_retain_live_batch_counters() {
    for (stage, mode) in [
        ("terminal collection", "serial"),
        ("paused", "idle"),
        ("completed", "idle"),
    ] {
        let mut report = json!({"execution":{"stage":"parallel elimination","mode":"parallel",
            "workers":128,"reserved_workers":128,"rows_total":7,"rows_committed":4,
            "blocks_completed":3,"provider_keys_filtered":2}});
        reset_execution_phase(&mut report, stage, mode);
        assert_eq!(report["execution"]["stage"], stage);
        assert_eq!(report["execution"]["mode"], mode);
        assert!(report["execution"]["rows_total"].is_null());
        assert!(report["execution"]["provider_keys_filtered"].is_null());
        assert_eq!(report["execution"]["reserved_workers"], 128);
    }
}
