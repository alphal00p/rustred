use super::super::sector_symmetry as sector;
use super::*;
use crate::application::routed_campaign::{
    PreparedTerminalEquations, TerminalEquationReductionFailure,
};
use std::sync::atomic::AtomicUsize;

fn sector_session() -> TerminalRelationSession {
    let context = CoefficientContext::try_new(["d"]).unwrap();
    let family = Arc::new(
        IntegralFamily::new(
            "master-sector-equations",
            vec!["k".into(), "l".into()],
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            [[1, 0, 0], [0, 0, 1], [1, 2, 1]]
                .into_iter()
                .map(|row| {
                    AffineDenominator::new(
                        context.integer(-1),
                        row.map(|value| context.integer(value)).into(),
                    )
                })
                .collect(),
            vec![],
            vec![context.zero(); 3],
        )
        .unwrap(),
    );
    TerminalRelationSession::new(
        family,
        // Distinct dot degrees retain independent source equations after
        // eliminating each original integral's complete polynomial orbit.
        BTreeSet::from([
            IntegralKey::try_new([1, 2, -1]).unwrap(),
            IntegralKey::try_new([1, 3, -1]).unwrap(),
        ]),
        0,
        Default::default(),
    )
    .unwrap()
}

fn assisted(directory: &Path) -> (MasterReductionOptions, TerminalRelationSession, Value) {
    assisted_with_policy(directory, None)
}

fn assisted_with_policy(
    directory: &Path,
    policy: Option<MasterSectorSymmetryPolicy>,
) -> (MasterReductionOptions, TerminalRelationSession, Value) {
    assisted_with_controls(directory, policy, None)
}

fn assisted_with_controls(
    directory: &Path,
    policy: Option<MasterSectorSymmetryPolicy>,
    saved_projection: Option<bool>,
) -> (MasterReductionOptions, TerminalRelationSession, Value) {
    let mut options = MasterReductionOptions::new("unused", directory);
    options.sector_symmetry_assistance = true;
    options.sector_symmetry_policy = policy;
    options.sector_symmetry_saved_projection = saved_projection;
    let mut native = sector_session();
    let mut report = json!({"schema":SCHEMA,"checkpoint":{"generation":0},
        "inputs":{"program_binding":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"}});
    assistance::configure(&options, &mut native, &mut report).unwrap();
    (options, native, report)
}

fn identity_saved(calls: Arc<AtomicUsize>) -> PreparedTerminalEquations {
    PreparedTerminalEquations {
        provider: Box::new(|_| Ok(vec![])),
        reduce_equation: Box::new(|equation| Ok(equation.clone())),
        reduce_equation_controlled: Box::new(move |equation, cancel, _| {
            calls.fetch_add(1, Ordering::Relaxed);
            if cancel.load(Ordering::Relaxed) {
                return Err(TerminalEquationReductionFailure::SavedApplication(
                    rustred::solver::CandidateRoutedError::Cancelled,
                ));
            }
            Ok(equation.clone())
        }),
    }
}

#[test]
fn sector_flag_off_preserves_historical_source_scope_bindings() {
    let scratch = Scratch::new();
    write_json(&scratch.0.join("latest.json"), &json!({"saved":"source"})).unwrap();
    let request = OwnerDomainWalkRequest::new(crate::OwnerDomainMatchRequest::new(
        "{}".into(),
        json!({"queries":[]}).to_string(),
    ));
    let mut options = MasterReductionOptions::new(&scratch.0, "unused");
    assert!(!options.sector_symmetry_assistance);
    let original = std::fs::read(scratch.0.join("latest.json")).unwrap();
    for circuit in [false, true] {
        options.circuit_symmetry_assistance = circuit;
        for numerator in [false, true] {
            options.numerator_symmetry_assistance = numerator;
            for normalization in [
                MasterNormalizationProfile::ConservativeV1,
                MasterNormalizationProfile::StandardV1,
            ] {
                let legacy = storage::scope_binding(
                    &request,
                    &options.checkpoint,
                    options.seed_depth,
                    options.containing_sector_depth,
                    circuit,
                    false,
                    true,
                    normalization,
                )
                .unwrap();
                let expected = if numerator {
                    let mut hash = blake3::Hasher::new();
                    hash.update(b"numerator-symmetry-assistance-v1");
                    hash.update(legacy.as_bytes());
                    hash.finalize().to_hex().to_string()
                } else {
                    legacy
                };
                assert_eq!(
                    source::SourceRequest::Walk(&request)
                        .binding(&options, false, true, normalization)
                        .unwrap(),
                    expected
                );
            }
        }
    }
    assert_eq!(
        std::fs::read(scratch.0.join("latest.json")).unwrap(),
        original
    );
}

#[test]
fn strong_binding_subsumes_numerator_and_authenticates_the_selected_programs() {
    let native = circuit_session();
    let mut report = json!({"inputs":{"program_binding":"a".repeat(64)}});
    for saved in [false, true] {
        for circuit in [false, true] {
            let mut options = MasterReductionOptions::new("unused", "unused");
            options.saved_rule_assistance = saved;
            options.circuit_symmetry_assistance = circuit;
            options.numerator_symmetry_assistance = true;
            let legacy = assistance::expected_binding(&options, &native, &report).unwrap();
            options.sector_symmetry_assistance = true;
            let strong = assistance::expected_binding(&options, &native, &report)
                .unwrap()
                .unwrap();
            assert_ne!(Some(strong.clone()), legacy);
            assert_eq!(strong.matches("sector-symmetry-assistance-v1:").count(), 1);
            assert!(!strong.contains("numerator-symmetry-assistance-v1:"));
            options.numerator_symmetry_assistance = false;
            assert_eq!(
                assistance::expected_binding(&options, &native, &report).unwrap(),
                Some(strong.clone())
            );
            report["inputs"]["program_binding"] = json!("b".repeat(64));
            assert_ne!(
                assistance::expected_binding(&options, &native, &report).unwrap(),
                Some(strong)
            );
            report["inputs"]
                .as_object_mut()
                .unwrap()
                .remove("program_binding");
            assert!(assistance::expected_binding(&options, &native, &report).is_err());
            report["inputs"]["program_binding"] = json!("a".repeat(64));
        }
    }
}

#[test]
fn strong_resume_rejects_missing_or_changed_program_selection() {
    let scratch = Scratch::new();
    let (mut options, native, report) = assisted(&scratch.0);
    options.resume = true;
    assistance::validate(&options, &native, &report).unwrap();
    let mut changed = report.clone();
    changed["inputs"]["program_binding"] = json!("b".repeat(64));
    assert!(assistance::validate(&options, &native, &changed).is_err());
    changed["inputs"]
        .as_object_mut()
        .unwrap()
        .remove("program_binding");
    assert!(assistance::validate(&options, &native, &changed).is_err());
    options.sector_symmetry_assistance = false;
    assert!(assistance::validate(&options, &native, &report).is_err());
}

#[test]
fn strong_policy_defaults_to_generators_and_binds_explicit_polynomial_orbits() {
    let native = sector_session();
    let report = json!({"inputs":{"program_binding":"a".repeat(64)}});
    let mut options = MasterReductionOptions::new("unused", "unused");
    options.sector_symmetry_assistance = true;
    assert_eq!(options.sector_symmetry_policy, None);
    let default = assistance::expected_binding(&options, &native, &report).unwrap();
    options.sector_symmetry_policy = Some(MasterSectorSymmetryPolicy::GeneratorRelations);
    assert_eq!(
        assistance::expected_binding(&options, &native, &report).unwrap(),
        default
    );
    options.sector_symmetry_policy = Some(MasterSectorSymmetryPolicy::PolynomialOrbits);
    let orbits = assistance::expected_binding(&options, &native, &report).unwrap();
    assert_ne!(orbits, default);
    options.numerator_symmetry_assistance = true;
    assert_eq!(
        assistance::expected_binding(&options, &native, &report).unwrap(),
        orbits
    );

    let scratch = Scratch::new();
    write_json(&scratch.0.join("latest.json"), &json!({"saved":"source"})).unwrap();
    let request = OwnerDomainWalkRequest::new(crate::OwnerDomainMatchRequest::new(
        "{}".into(),
        json!({"queries":[]}).to_string(),
    ));
    options.checkpoint = scratch.0.clone();
    options.numerator_symmetry_assistance = false;
    options.sector_symmetry_policy = None;
    let default_scope = source::SourceRequest::Walk(&request)
        .binding(
            &options,
            false,
            true,
            MasterNormalizationProfile::ConservativeV1,
        )
        .unwrap();
    options.sector_symmetry_policy = Some(MasterSectorSymmetryPolicy::GeneratorRelations);
    assert_eq!(
        source::SourceRequest::Walk(&request)
            .binding(
                &options,
                false,
                true,
                MasterNormalizationProfile::ConservativeV1
            )
            .unwrap(),
        default_scope
    );
    options.sector_symmetry_policy = Some(MasterSectorSymmetryPolicy::PolynomialOrbits);
    assert_ne!(
        source::SourceRequest::Walk(&request)
            .binding(
                &options,
                false,
                true,
                MasterNormalizationProfile::ConservativeV1
            )
            .unwrap(),
        default_scope
    );
    options.sector_symmetry_assistance = false;
    let mut disabled = sector_session();
    let mut disabled_report = report.clone();
    assert!(assistance::configure(&options, &mut disabled, &mut disabled_report).is_err());
    assert!(disabled.assistance_binding().is_none());
    assert_eq!(disabled_report, report);
}

#[test]
fn strong_policy_is_native_authenticated_and_omission_inherits_on_resume() {
    for (policy, name, changed_policy, changed_name) in [
        (
            MasterSectorSymmetryPolicy::GeneratorRelations,
            "generators",
            MasterSectorSymmetryPolicy::PolynomialOrbits,
            "polynomial-orbits",
        ),
        (
            MasterSectorSymmetryPolicy::PolynomialOrbits,
            "polynomial-orbits",
            MasterSectorSymmetryPolicy::GeneratorRelations,
            "generators",
        ),
    ] {
        let scratch = Scratch::new();
        let (mut options, native, mut report) = assisted_with_policy(&scratch.0, Some(policy));
        assert_eq!(report["sector_symmetry_policy"], name);
        let plan = sector::prepare(
            &options,
            &native,
            &mut report,
            &AtomicBool::new(false),
            &|_| {},
            Instant::now(),
        )
        .unwrap();
        assert_eq!(plan.policy(), policy);
        assert_eq!(report["sector_symmetry_certificate"]["policy"], name);
        assert_eq!(report["sector_symmetry_preparation"]["policy"], name);
        sector::validate(&scratch.0, &native, &report).unwrap();
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
        options.sector_symmetry_policy = None;
        assistance::validate(&options, &restored, &report).unwrap();
        assistance::configure(&options, &mut restored, &mut report).unwrap();
        assert_eq!(report["sector_symmetry_policy"], name);
        let preserved = report.clone();
        let before = restored.statistics();
        options.sector_symmetry_policy = Some(changed_policy);
        assert!(assistance::configure(&options, &mut restored, &mut report).is_err());
        assert_eq!(report, preserved);
        assert_eq!(restored.statistics(), before);
        let mut relabelled = preserved;
        relabelled["sector_symmetry_policy"] = json!(changed_name);
        assert!(sector::validate(&scratch.0, &restored, &relabelled).is_err());
    }
}

#[test]
fn saved_projection_defaults_to_true_and_false_is_a_distinct_resumable_policy() {
    let native = sector_session();
    let report = json!({"inputs":{"program_binding":"a".repeat(64)}});
    let mut options = MasterReductionOptions::new("unused", "unused");
    options.sector_symmetry_assistance = true;
    assert_eq!(options.sector_symmetry_saved_projection, None);
    let default = assistance::expected_binding(&options, &native, &report).unwrap();
    options.sector_symmetry_saved_projection = Some(true);
    assert_eq!(
        assistance::expected_binding(&options, &native, &report).unwrap(),
        default
    );
    options.sector_symmetry_saved_projection = Some(false);
    assert_ne!(
        assistance::expected_binding(&options, &native, &report).unwrap(),
        default
    );

    for requested in [false, true] {
        let scratch = Scratch::new();
        let (mut options, native, mut report) =
            assisted_with_controls(&scratch.0, None, Some(requested));
        assert_eq!(report["sector_symmetry_saved_projection"], requested);
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
        options.sector_symmetry_saved_projection = None;
        assert_eq!(
            sector::projection_for_phase(&options, &report).unwrap(),
            requested
        );
        assistance::configure(&options, &mut restored, &mut report).unwrap();
        let preserved = report.clone();
        let before = restored.statistics();
        options.sector_symmetry_saved_projection = Some(!requested);
        assert!(assistance::configure(&options, &mut restored, &mut report).is_err());
        assert_eq!(report, preserved);
        assert_eq!(restored.statistics(), before);
    }

    options.sector_symmetry_assistance = false;
    let mut disabled = sector_session();
    let mut disabled_report = report.clone();
    assert!(assistance::configure(&options, &mut disabled, &mut disabled_report).is_err());
    assert_eq!(disabled_report, report);
    assert!(disabled.assistance_binding().is_none());
}

#[test]
fn pure_sector_rows_skip_saved_programs_and_survive_cold_publication_and_fresh_search() {
    let source = Scratch::new();
    let published = Scratch::new();
    let fresh_output = Scratch::new();
    let mut options = MasterReductionOptions::new("unused", &source.0);
    options.sector_symmetry_assistance = true;
    options.sector_symmetry_saved_projection = Some(false);
    let mut native = sector_session();
    let raw = native.raw_terminals().clone();
    // This portable selection is deliberately not a candidate-program
    // selection. The pure finite provider must never load it as saved rules.
    let mut report = json!({"schema":SCHEMA,"scope_binding":"sector-pure-scope",
        "operation":"refine","checkpoint":{"generation":0},
        "inventory":{"complete":true},"inputs":portable_inputs(&source.0)});
    assistance::configure(&options, &mut native, &mut report).unwrap();
    let plan = sector::prepare(
        &options,
        &native,
        &mut report,
        &AtomicBool::new(false),
        &|_| {},
        Instant::now(),
    )
    .unwrap();
    let mut unavailable = PreparedTerminalEquations {
        provider: Box::new(|_| panic!("pure provider requested dynamic saved rules")),
        reduce_equation: Box::new(|_| panic!("pure provider invoked saved reduction")),
        reduce_equation_controlled: Box::new(|_, _, _| {
            panic!("pure provider invoked controlled saved reduction")
        }),
    };
    let rows = sector::prepare_rows(
        &options,
        &native,
        &plan,
        Some(&mut unavailable),
        &mut report,
        &AtomicBool::new(false),
        &|_| {},
        Instant::now(),
    )
    .unwrap();
    assert!(!rows.is_empty());
    assert_eq!(report["sector_symmetry_saved_projection"], false);
    assert_eq!(
        report["sector_symmetry_reduction"]["saved_projection_requested"],
        false
    );
    assert!(report["sector_symmetry_reduction"]["saved_projection_complete"].is_null());
    assert_eq!(
        report["sector_symmetry_reduction"]["exact_source_rows_ready"],
        true
    );
    assert_eq!(report["sector_symmetry_reduction"]["saved_reduced_rows"], 0);
    assert_eq!(
        report["sector_symmetry_reduction"]["saved_application_failures"],
        0
    );
    assert_eq!(
        report["sector_symmetry_reduction"]["combined_closure_complete"],
        false
    );
    assert!(
        report["sector_symmetry_reduction"]["unresolved_output_keys"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(
        report["sector_symmetry_reduced_certificate"]["saved_projection_requested"],
        false
    );
    sector::validate(&source.0, &native, &report).unwrap();
    assert_eq!(
        sector::prepare_rows(
            &options,
            &native,
            &plan,
            None,
            &mut report,
            &AtomicBool::new(false),
            &|_| {},
            Instant::now(),
        )
        .unwrap(),
        rows
    );
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
    let mut provider = prepared.provider;
    assert!(
        provider(&IntegralKey::try_new([1, 4, -1]).unwrap())
            .unwrap()
            .is_empty()
    );
    while native.statistics().pending_assistance_keys != 0
        || native.statistics().pending_assistance_rows != 0
    {
        native
            .step_with_provider(&AtomicBool::new(false), &mut provider)
            .unwrap();
    }
    assert_eq!(native.raw_terminals(), &raw);
    assert_eq!(native.statistics().completed_source_rows, 0);
    assert!(native.statistics().completed_assistance_rows > 0);
    let expected = native.apply_all_terminals().unwrap();
    save(
        &options,
        &native,
        &mut report,
        "paused",
        Instant::now(),
        &|_| {},
    )
    .unwrap();
    let restored_native = load_master_relation_session(&source.0).unwrap();
    assert_eq!(restored_native.apply_all_terminals().unwrap(), expected);
    let mut tampered = report.clone();
    tampered["sector_symmetry_saved_projection"] = json!(true);
    assert!(sector::validate(&source.0, &restored_native, &tampered).is_err());

    let mut publishing = MasterReductionOptions::new("unused", &published.0);
    publishing.operation = MasterReductionOperation::Publish;
    let publication =
        master_publish_partial_refinement(&source.0, &publishing, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(publication["sector_symmetry_saved_projection"], false);
    let artifact = load_master_reduction(&published.0).unwrap();
    assert_eq!(
        artifact.primary_session().apply_all_terminals().unwrap(),
        expected
    );
    assert_eq!(
        artifact
            .primary_session()
            .statistics()
            .completed_source_rows,
        0
    );
    let mut fresh_options = MasterReductionOptions::new("unused", &fresh_output.0);
    fresh_options.fresh_search = true;
    fresh_options.finite_feedback = Some(false);
    let paused = master_refine_published_artifact(
        &published.0,
        &fresh_options,
        &AtomicBool::new(true),
        |_| {},
    )
    .unwrap();
    let fresh_native = load_master_relation_session(&fresh_output.0).unwrap();
    fresh::validate(&fresh_output.0, &paused, &fresh_native).unwrap();
    assert!(fresh_native.retained_relations_binding().is_some());
    assert!(fresh_native.assistance_binding().is_none());
    assert_eq!(fresh_native.statistics().completed_source_rows, 0);
    assert_eq!(fresh_native.statistics().completed_assistance_rows, 0);
    assert_eq!(fresh_native.raw_terminals(), &raw);
    assert!(paused["sector_symmetry_reduced_certificate"].is_null());
    for key in &raw {
        let published_map = artifact.apply_terminal(native.family_owner(), key).unwrap();
        let local: std::collections::BTreeMap<_, _> = published_map
            .terms()
            .iter()
            .map(|(key, value)| (key.integral().clone(), value.clone()))
            .collect();
        assert_eq!(fresh_native.apply_terminal(key).unwrap(), local);
    }
}

#[test]
fn sector_source_certificate_replays_without_changing_the_raw_inventory() {
    let scratch = Scratch::new();
    let (options, native, mut report) = assisted(&scratch.0);
    let raw = native.raw_terminals().clone();
    let before = native.statistics();
    let plan = sector::prepare(
        &options,
        &native,
        &mut report,
        &AtomicBool::new(false),
        &|_| {},
        Instant::now(),
    )
    .unwrap();
    assert_eq!(
        plan.policy(),
        MasterSectorSymmetryPolicy::GeneratorRelations
    );
    assert_eq!(report["sector_symmetry_policy"], "generators");
    assert!(plan.statistics().equations > 0);
    assert!(plan.inventory().len() > raw.len());
    for auxiliary in plan.inventory().difference(plan.input_keys()) {
        assert!(plan.equations(auxiliary).is_empty());
    }
    assert_eq!(native.raw_terminals(), &raw);
    assert_eq!(native.statistics(), before);
    assert_eq!(native.statistics().completed_source_rows, 0);
    assert_eq!(native.statistics().completed_assistance_rows, 0);
    assert!(report["sector_symmetry_normalization_certificate"].is_null());
    // No equation has been accepted yet, so source preparation can be resumed
    // before the separate normalization/projection receipts exist.
    sector::validate(&scratch.0, &native, &report).unwrap();
    let path = storage::safe_child(
        &scratch.0,
        report["sector_symmetry_certificate"]["file"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let original = std::fs::read(&path).unwrap();
    let mut changed = original.clone();
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, changed).unwrap();
    assert!(sector::validate(&scratch.0, &native, &report).is_err());
    std::fs::write(&path, original).unwrap();
    sector::validate(&scratch.0, &native, &report).unwrap();
    let mut changed = report.clone();
    changed["sector_symmetry_certificate"]["inventory_binding"] = json!("forged");
    assert!(sector::validate(&scratch.0, &native, &changed).is_err());
    changed = report.clone();
    changed["inputs"]["program_binding"] = json!("b".repeat(64));
    assert!(sector::validate(&scratch.0, &native, &changed).is_err());
    changed = report.clone();
    changed["status"] = json!("completed_nonminimal");
    changed["sector_symmetry_reduction"]["complete"] = json!(true);
    assert!(sector::validate(&scratch.0, &native, &changed).is_err());
}

#[test]
fn cancellation_before_preparation_publishes_no_sector_witness_or_rows() {
    let scratch = Scratch::new();
    let (options, native, mut report) = assisted(&scratch.0);
    let raw = native.raw_terminals().clone();
    let before = native.statistics();
    assert!(
        assistance::prepare_for_execution(
            &options,
            &native,
            &mut report,
            &AtomicBool::new(true),
            &|_| {},
            Instant::now(),
        )
        .is_err()
    );
    assert!(report["sector_symmetry_certificate"].is_null());
    assert!(report["sector_symmetry_normalization_certificate"].is_null());
    assert!(report["sector_symmetry_task_certificate"].is_null());
    assert!(report["sector_symmetry_reduced_certificate"].is_null());
    assert_eq!(std::fs::read_dir(&scratch.0).unwrap().count(), 0);
    assert_eq!(native.raw_terminals(), &raw);
    assert_eq!(native.statistics(), before);
}

#[test]
fn sector_projection_resumes_its_frozen_prefix_and_reuses_the_completed_provider() {
    let scratch = Scratch::new();
    let (options, native, mut report) = assisted(&scratch.0);
    let raw = native.raw_terminals().clone();
    let plan = sector::prepare(
        &options,
        &native,
        &mut report,
        &AtomicBool::new(false),
        &|_| {},
        Instant::now(),
    )
    .unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut saved = identity_saved(Arc::clone(&calls));
    let cancel = AtomicBool::new(false);
    assert!(
        sector::prepare_rows(
            &options,
            &native,
            &plan,
            Some(&mut saved),
            &mut report,
            &cancel,
            &|event| {
                if event["sector_symmetry_reduction"]["processed_rows"]
                    .as_u64()
                    .is_some_and(|rows| rows >= 1)
                {
                    cancel.store(true, Ordering::Relaxed);
                }
            },
            Instant::now(),
        )
        .is_err()
    );
    let prefix = report["sector_symmetry_projection_checkpoint"].clone();
    assert_eq!(prefix["complete"], false);
    assert_eq!(prefix["processed_rows"], 1);
    assert!(prefix["source_rows"].as_u64().unwrap() > 1);
    assert!(report["sector_symmetry_reduced_certificate"].is_null());
    assert_eq!(
        report["sector_symmetry_reduction"]["normalization_outputs_outside_source_inventory"],
        0
    );
    assert_eq!(
        report["sector_symmetry_reduction"]["combined_closure_complete"],
        false
    );
    assert_eq!(native.raw_terminals(), &raw);
    assert_eq!(native.statistics().completed_source_rows, 0);
    assert_eq!(native.statistics().completed_assistance_rows, 0);

    // Resume through the cold native state, retaining the exact frozen task
    // vector and already completed projected rows.
    let restored = load_master_relation_session(&scratch.0).unwrap();
    let mut resumed = read_json(&scratch.0.join("latest.json")).unwrap();
    let tasks = resumed["sector_symmetry_task_certificate"].clone();
    let rows = sector::prepare_rows(
        &options,
        &restored,
        &plan,
        Some(&mut saved),
        &mut resumed,
        &AtomicBool::new(false),
        &|_| {},
        Instant::now(),
    )
    .unwrap();
    assert!(!rows.is_empty());
    assert_eq!(resumed["sector_symmetry_task_certificate"], tasks);
    assert_eq!(resumed["sector_symmetry_reduction"]["complete"], true);
    assert_eq!(
        resumed["sector_symmetry_reduction"]["normalization_outputs_outside_source_inventory"],
        0
    );
    assert_eq!(
        resumed["sector_symmetry_reduction"]["saved_projection_complete"],
        true
    );
    assert_eq!(
        resumed["sector_symmetry_reduction"]["combined_closure_complete"],
        false
    );
    assert!(
        resumed["sector_symmetry_reduction"]["unresolved_output_keys"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(
        resumed["sector_symmetry_reduced_certificate"]["processed_rows"],
        prefix["source_rows"]
    );
    assert!(calls.load(Ordering::Relaxed) > 0);
    sector::validate(&scratch.0, &restored, &resumed).unwrap();

    // A completed immutable provider is replayed without another saved-rule
    // application, even if the runtime reducer would now fail every call.
    let mut unavailable = PreparedTerminalEquations {
        provider: Box::new(|_| panic!("completed projection requested dynamic rules")),
        reduce_equation: Box::new(|_| panic!("completed projection invoked saved reduction")),
        reduce_equation_controlled: Box::new(|_, _, _| {
            panic!("completed projection invoked controlled saved reduction")
        }),
    };
    assert_eq!(
        sector::prepare_rows(
            &options,
            &restored,
            &plan,
            Some(&mut unavailable),
            &mut resumed,
            &AtomicBool::new(false),
            &|_| {},
            Instant::now(),
        )
        .unwrap(),
        rows
    );
    assert_eq!(restored.raw_terminals(), &raw);
    assert_eq!(restored.statistics().completed_source_rows, 0);
    let mut tampered = resumed.clone();
    tampered["sector_symmetry_task_certificate"]["authority_binding"] = json!("forged");
    assert!(sector::validate(&scratch.0, &restored, &tampered).is_err());
    tampered = resumed.clone();
    tampered["sector_symmetry_reduced_certificate"]["processed_rows"] = json!(0);
    assert!(sector::validate(&scratch.0, &restored, &tampered).is_err());
}

#[test]
fn self_consistent_truncated_projection_cannot_replace_the_frozen_task_vector() {
    let scratch = Scratch::new();
    let (options, native, mut report) = assisted(&scratch.0);
    let plan = sector::prepare(
        &options,
        &native,
        &mut report,
        &AtomicBool::new(false),
        &|_| {},
        Instant::now(),
    )
    .unwrap();
    let mut saved = identity_saved(Arc::new(AtomicUsize::new(0)));
    sector::prepare_rows(
        &options,
        &native,
        &plan,
        Some(&mut saved),
        &mut report,
        &AtomicBool::new(false),
        &|_| {},
        Instant::now(),
    )
    .unwrap();
    sector::validate(&scratch.0, &native, &report).unwrap();
    load_master_relation_session(&scratch.0).unwrap();

    let descriptor = &report["sector_symmetry_reduced_certificate"];
    let authority = descriptor["authority_binding"].as_str().unwrap().to_owned();
    let path = storage::safe_child(&scratch.0, descriptor["file"].as_str().unwrap()).unwrap();
    let mut rows = terminal_import::rows::decode(
        &std::fs::read(path).unwrap(),
        native.family_owner(),
        &authority,
    )
    .unwrap();
    assert!(!rows.is_empty());
    assert_eq!(
        report["sector_symmetry_task_certificate"]["rows"],
        rows.len()
    );
    rows.pop();
    // Preserve the genuine final authority and a valid native envelope while
    // making the payload and every projection cursor agree on the wrong size.
    let bytes = terminal_import::rows::encode(native.family_owner(), &rows, &authority).unwrap();
    let digest = blake3::hash(&bytes).to_hex().to_string();
    let file = format!("truncated-sector-projection-{digest}.rrbin");
    write_file_atomically(&scratch.0.join(&file), &bytes, true).unwrap();
    let descriptor = &mut report["sector_symmetry_reduced_certificate"];
    descriptor["file"] = json!(file);
    descriptor["blake3"] = json!(digest);
    descriptor["bytes"] = json!(bytes.len());
    descriptor["processed_rows"] = json!(rows.len());
    descriptor["source_rows"] = json!(rows.len());
    report["sector_symmetry_reduction"]["processed_rows"] = json!(rows.len());
    report["sector_symmetry_reduction"]["source_rows"] = json!(rows.len());
    assert!(sector::validate(&scratch.0, &native, &report).is_err());
    write_json(&scratch.0.join("latest.json"), &report).unwrap();
    assert!(load_master_relation_session(&scratch.0).is_err());
    assert!(
        sector::prepare_rows(
            &options,
            &native,
            &plan,
            Some(&mut saved),
            &mut report,
            &AtomicBool::new(false),
            &|_| {},
            Instant::now(),
        )
        .is_err()
    );
}

#[test]
fn cancelled_projection_can_publish_existing_maps_and_seed_a_fresh_round() {
    let source = Scratch::new();
    let published = Scratch::new();
    let fresh_output = Scratch::new();
    let mut options = MasterReductionOptions::new("unused", &source.0);
    options.sector_symmetry_assistance = true;
    let mut native = sector_session();
    let mut report = json!({"schema":SCHEMA,"scope_binding":"sector-partial-scope",
        "operation":"refine","checkpoint":{"generation":0},
        "inventory":{"complete":true},"inputs":portable_inputs(&source.0)});
    assistance::configure(&options, &mut native, &mut report).unwrap();
    let expected = native.apply_all_terminals().unwrap();
    let before = native.statistics();
    assert!(before.completed_seeds < before.seeds);
    let plan = sector::prepare(
        &options,
        &native,
        &mut report,
        &AtomicBool::new(false),
        &|_| {},
        Instant::now(),
    )
    .unwrap();
    let mut saved = identity_saved(Arc::new(AtomicUsize::new(0)));
    let cancel = AtomicBool::new(false);
    assert!(
        sector::prepare_rows(
            &options,
            &native,
            &plan,
            Some(&mut saved),
            &mut report,
            &cancel,
            &|event| {
                if event["sector_symmetry_reduction"]["processed_rows"]
                    .as_u64()
                    .is_some_and(|rows| rows >= 1)
                {
                    cancel.store(true, Ordering::Relaxed);
                }
            },
            Instant::now(),
        )
        .is_err()
    );
    assert_eq!(
        report["sector_symmetry_projection_checkpoint"]["processed_rows"],
        1
    );
    assert_eq!(report["sector_symmetry_reduction"]["complete"], false);
    assert_eq!(
        report["sector_symmetry_reduction"]["normalization_outputs_outside_source_inventory"],
        0
    );
    assert_eq!(
        report["sector_symmetry_reduction"]["combined_closure_complete"],
        false
    );
    assert!(report["sector_symmetry_reduced_certificate"].is_null());
    assert_eq!(native.statistics(), before);
    save(
        &options,
        &native,
        &mut report,
        "paused",
        Instant::now(),
        &|_| {},
    )
    .unwrap();
    let original_manifest = std::fs::read(source.0.join("latest.json")).unwrap();
    let original_native = native.to_native_bytes(Default::default()).unwrap();

    let mut publishing = MasterReductionOptions::new("unused", &published.0);
    publishing.operation = MasterReductionOperation::Publish;
    let publication =
        master_publish_partial_refinement(&source.0, &publishing, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(publication["status"], "published_unrefined");
    assert_eq!(publication["sector_symmetry_assistance"], false);
    assert_eq!(
        publication["inherited_equation_authority"]["sector_symmetry_assistance"],
        true
    );
    assert_eq!(publication["sector_symmetry_reduction"]["complete"], false);
    assert!(publication["sector_symmetry_reduced_certificate"].is_null());
    let restored = load_master_reduction(&published.0).unwrap();
    assert_eq!(restored.primary_session().statistics(), before);
    assert_eq!(
        restored.primary_session().apply_all_terminals().unwrap(),
        expected
    );
    assert_eq!(
        std::fs::read(source.0.join("latest.json")).unwrap(),
        original_manifest
    );
    assert_eq!(
        load_master_relation_session(&source.0)
            .unwrap()
            .to_native_bytes(Default::default())
            .unwrap(),
        original_native
    );

    let mut fresh_options = MasterReductionOptions::new("unused", &fresh_output.0);
    fresh_options.fresh_search = true;
    fresh_options.finite_feedback = Some(false);
    let paused = master_refine_published_artifact(
        &published.0,
        &fresh_options,
        &AtomicBool::new(true),
        |_| {},
    )
    .unwrap();
    assert_eq!(paused["status"], "paused");
    assert_eq!(paused["sector_symmetry_assistance"], false);
    assert!(paused["retained_relations"].is_object());
    for field in [
        "sector_symmetry_certificate",
        "sector_symmetry_normalization_certificate",
        "sector_symmetry_task_certificate",
        "sector_symmetry_reduced_certificate",
        "sector_symmetry_projection_checkpoint",
    ] {
        assert!(paused[field].is_null(), "fresh round retained {field}");
    }
    let fresh_native = load_master_relation_session(&fresh_output.0).unwrap();
    fresh::validate(&fresh_output.0, &paused, &fresh_native).unwrap();
    assert!(fresh_native.retained_relations_binding().is_some());
    assert!(fresh_native.assistance_binding().is_none());
    assert_eq!(fresh_native.statistics().completed_source_rows, 0);
    assert_eq!(fresh_native.statistics().completed_assistance_rows, 0);
    assert_eq!(fresh_native.statistics().completed_assistance_keys, 0);
    assert_eq!(fresh_native.statistics().pending_assistance_rows, 0);
    assert_eq!(fresh_native.raw_terminals(), native.raw_terminals());
    for key in fresh_native.raw_terminals() {
        let published_map = restored.apply_terminal(native.family_owner(), key).unwrap();
        let local: std::collections::BTreeMap<_, _> = published_map
            .terms()
            .iter()
            .map(|(key, value)| (key.integral().clone(), value.clone()))
            .collect();
        assert_eq!(fresh_native.apply_terminal(key).unwrap(), local);
    }
}
