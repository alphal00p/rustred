use super::*;

fn assisted(directory: &Path) -> (MasterReductionOptions, TerminalRelationSession, Value) {
    let mut options = MasterReductionOptions::new("unused", directory);
    options.numerator_symmetry_assistance = true;
    let mut native = circuit_session();
    let mut report = json!({"schema":SCHEMA,"checkpoint":{"generation":0}});
    assistance::configure(&options, &mut native, &mut report).unwrap();
    (options, native, report)
}

#[test]
fn numerator_provider_is_opt_in_and_has_a_cold_replayed_native_certificate() {
    let scratch = Scratch::new();
    let mut native = circuit_session();
    let target = native.raw_terminals().first().unwrap().clone();
    let mut report = json!({"schema":SCHEMA,"checkpoint":{"generation":0}});
    let mut options = MasterReductionOptions::new("unused", &scratch.0);
    assistance::configure(&options, &mut native, &mut report).unwrap();
    assert!(
        assistance::prepare(
            &options,
            &native,
            &mut report,
            &AtomicBool::new(false),
            &|_| {},
            Instant::now(),
        )
        .unwrap()
        .is_none()
    );
    assert!(report["numerator_symmetry_certificate"].is_null());
    options.numerator_symmetry_assistance = true;
    assistance::configure(&options, &mut native, &mut report).unwrap();
    let provider = assistance::prepare(
        &options,
        &native,
        &mut report,
        &AtomicBool::new(false),
        &|_| {},
        Instant::now(),
    )
    .unwrap()
    .unwrap();
    let mut provider = provider;
    let rows = provider(&target).unwrap();
    assert!(!rows.is_empty());
    assert_eq!(report["numerator_symmetry_assistance"], true);
    assert_eq!(report["saved_rule_assistance"], false);
    assert_eq!(report["circuit_symmetry_assistance"], false);
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
    // Isolate the new provider: do not consume any ordinary IBP source row.
    while native.statistics().pending_assistance_keys != 0
        || native.statistics().pending_assistance_rows != 0
    {
        native
            .step_with_provider(&AtomicBool::new(false), &mut provider)
            .unwrap();
    }
    assert_eq!(native.statistics().completed_source_rows, 0);
    assert!(native.statistics().completed_assistance_rows > 0);
    let exact_before = native.apply_all_terminals().unwrap();
    save(
        &options,
        &native,
        &mut report,
        "paused",
        Instant::now(),
        &|_| {},
    )
    .unwrap();
    let restored = load_master_relation_session(&scratch.0).unwrap();
    assert_eq!(restored.apply_all_terminals().unwrap(), exact_before);
    options.resume = true;
    assistance::validate(&options, &restored, &report).unwrap();
    options.numerator_symmetry_assistance = false;
    assert!(assistance::validate(&options, &restored, &report).is_err());
    let certificate = storage::safe_child(
        &scratch.0,
        report["numerator_symmetry_certificate"]["file"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let original = std::fs::read(&certificate).unwrap();
    let mut changed = original.clone();
    changed[0] ^= 1;
    std::fs::write(&certificate, changed).unwrap();
    assert!(load_master_relation_session(&scratch.0).is_err());
    std::fs::write(&certificate, original).unwrap();
    assert!(load_master_relation_session(&scratch.0).is_ok());
    let mut tampered = report.clone();
    tampered["numerator_symmetry_certificate"]["inventory_binding"] = json!("forged");
    assert!(super::super::numerator_symmetry::validate(&scratch.0, &restored, &tampered).is_err());
}

#[test]
fn numerator_certificate_inherits_with_frozen_authority_without_new_search() {
    let source = Scratch::new();
    let output = Scratch::new();
    let (options, mut native, mut report) = assisted(&source.0);
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
    while native.statistics().completed_assistance_rows == 0 {
        assert!(
            native.statistics().pending_assistance_keys != 0
                || native.statistics().pending_assistance_rows != 0
        );
        native
            .step_with_provider(&AtomicBool::new(false), &mut provider)
            .unwrap();
    }
    let mut published = report.clone();
    inherited::capture(&native, &report, &mut published).unwrap();
    super::super::numerator_symmetry::inherit(&source.0, &output.0, &published).unwrap();
    let mut publishing = MasterReductionOptions::new("unused", &output.0);
    publishing.operation = MasterReductionOperation::Publish;
    assistance::configure(&publishing, &mut native, &mut published).unwrap();
    assert_eq!(published["numerator_symmetry_assistance"], false);
    assert_eq!(
        published["inherited_equation_authority"]["numerator_symmetry_assistance"],
        true
    );
    inherited::validate(&native, &published).unwrap();
    super::super::numerator_symmetry::validate(&output.0, &native, &published).unwrap();
    assert_eq!(native.statistics().completed_source_rows, 0);
    let mut missing = published.clone();
    missing
        .as_object_mut()
        .unwrap()
        .remove("numerator_symmetry_certificate");
    assert!(inherited::validate(&native, &missing).is_err());
    assert!(super::super::numerator_symmetry::validate(&output.0, &native, &missing).is_err());
}

#[test]
fn legacy_provider_binding_is_identical_when_numerator_assistance_is_disabled() {
    for (saved, circuit) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut options = MasterReductionOptions::new("unused", "unused");
        options.saved_rule_assistance = saved;
        options.circuit_symmetry_assistance = circuit;
        let native = circuit_session();
        let report = json!({"inputs":{"program_binding":"unchanged"}});
        let old = assistance::expected_binding(&options, &native, &report).unwrap();
        options.numerator_symmetry_assistance = true;
        let changed = assistance::expected_binding(&options, &native, &report).unwrap();
        assert_ne!(old, changed);
        options.numerator_symmetry_assistance = false;
        assert_eq!(
            old,
            assistance::expected_binding(&options, &native, &report).unwrap()
        );
    }
}

#[test]
fn saved_symmetry_resources_follow_the_ram_guard_without_changing_flag_off_defaults() {
    use crate::application::routed_campaign::RoutedCampaignRequest;
    use rustred::algebra::{ExactAlgebraLimits, IndexedAlgebraLimits};
    use rustred::reduction::ReductionLimits;
    use rustred::sector::symmetry::integral_transport::ExpansionLimits;
    use rustred::solver::RoutedCandidateLimits;

    let mut request = RoutedCampaignRequest::new("selection".into(), "targets".into());
    request.workers = 7;
    request.owner_base = PathBuf::from("unchanged-owner-base");
    let original_reduction = request.reduction_limits;
    let original_trace = request.trace_limits;
    let mut options = MasterReductionOptions::new("unused", "unused");
    options.saved_rule_assistance = true;
    assistance::configure_saved_resources(&options, &mut request);
    assert_eq!(request.reduction_limits, original_reduction);
    assert_eq!(request.trace_limits, original_trace);

    options.numerator_symmetry_assistance = true;
    assistance::configure_saved_resources(&options, &mut request);
    assert_eq!(
        request.reduction_limits,
        ReductionLimits {
            exact_algebra: ExactAlgebraLimits::unbounded_resources(),
            indexed_algebra: IndexedAlgebraLimits::unbounded_resources(),
            max_rule_applications: usize::MAX,
            max_cached_integrals: usize::MAX,
            max_cached_coefficient_terms: usize::MAX,
            max_cached_coefficient_bytes: usize::MAX,
            max_pending_frames: usize::MAX,
            max_coalescing_additions: usize::MAX,
            max_factorization_terms: usize::MAX,
        }
    );
    assert_eq!(
        request.trace_limits,
        RoutedCandidateLimits {
            max_input_targets: usize::MAX,
            max_unique_nodes: usize::MAX,
            max_transport_calls: usize::MAX,
            max_transport_operations: usize::MAX,
            max_transport_endpoints: usize::MAX,
            expansion: ExpansionLimits::unbounded_resources(),
        }
    );
    assert_eq!(request.workers, 7);
    assert_eq!(request.owner_base, PathBuf::from("unchanged-owner-base"));
    assert_eq!(request.selection_json, "selection");
    assert_eq!(request.targets_csv, "targets");
}

#[test]
fn fresh_round_can_enable_then_disable_numerator_provider_without_losing_proved_maps() {
    let source = Scratch::new();
    publish_fixture(&source.0, &mut session());
    let source_manifest = std::fs::read(source.0.join("latest.json")).unwrap();
    let original = load_master_reduction(&source.0).unwrap();
    let output = Scratch::new();
    let mut options = MasterReductionOptions::new("unused", &output.0);
    options.fresh_search = true;
    options.numerator_symmetry_assistance = true;
    options.finite_feedback = Some(false);
    let paused =
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(true), |_| {})
            .unwrap();
    assert_eq!(paused["status"], "paused");
    assert!(paused["numerator_symmetry_certificate"].is_null());
    let native = load_master_relation_session(&output.0).unwrap();
    assert!(native.retained_relations_binding().is_some());
    assert!(native.assistance_binding().is_some());
    assert_eq!(native.statistics().completed_source_rows, 0);
    for key in native.raw_terminals() {
        let expected = original.apply_terminal(native.family_owner(), key).unwrap();
        let local: std::collections::BTreeMap<_, _> = expected
            .terms()
            .iter()
            .map(|(key, value)| (key.integral().clone(), value.clone()))
            .collect();
        assert_eq!(native.apply_terminal(key).unwrap(), local);
    }
    options.resume = true;
    let completed =
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(completed["status"], "completed_nonminimal");
    assert!(completed["numerator_symmetry_certificate"].is_object());
    let exact = load_master_reduction(&output.0).unwrap();
    let next = Scratch::new();
    let mut next_options = MasterReductionOptions::new("unused", &next.0);
    next_options.fresh_search = true;
    next_options.finite_feedback = Some(false);
    let paused =
        master_refine_published_artifact(&output.0, &next_options, &AtomicBool::new(true), |_| {})
            .unwrap();
    assert_eq!(paused["numerator_symmetry_assistance"], false);
    assert!(paused["numerator_symmetry_certificate"].is_null());
    assert!(paused["numerator_symmetry_preparation"].is_null());
    assert!(paused["numerator_symmetry_reduction"].is_null());
    let next_native = load_master_relation_session(&next.0).unwrap();
    assert!(next_native.assistance_binding().is_none());
    assert!(next_native.retained_relations_binding().is_some());
    for key in next_native.raw_terminals() {
        let expected = exact
            .apply_terminal(next_native.family_owner(), key)
            .unwrap();
        let local: std::collections::BTreeMap<_, _> = expected
            .terms()
            .iter()
            .map(|(key, value)| (key.integral().clone(), value.clone()))
            .collect();
        assert_eq!(next_native.apply_terminal(key).unwrap(), local);
    }
    assert_eq!(
        std::fs::read(source.0.join("latest.json")).unwrap(),
        source_manifest
    );
}

#[cfg(feature = "reconstruction")]
#[test]
fn modular_refinement_never_silently_discards_numerator_assistance() {
    let mut options = MasterReductionOptions::new("unused", "unused");
    options.refinement_strategy = Some(MasterRefinementStrategy::ModularCircuits);
    options.numerator_symmetry_assistance = true;
    assert!(modular::for_phase(&options, None).is_err());
}
