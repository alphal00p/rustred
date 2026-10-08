use super::*;
use MasterNormalizationProfile::{ConservativeV1, StandardV1};

fn wide_geometry() -> (Arc<IntegralFamily>, BTreeSet<IntegralKey>) {
    let context = CoefficientContext::try_new(["d"]).unwrap();
    let loops = 5;
    let mut momenta: Vec<Vec<i64>> = (0..loops)
        .map(|i| (0..loops).map(|j| i64::from(i == j)).collect())
        .collect();
    momenta.push(vec![1; loops]);
    for i in 0..loops {
        for j in i + 1..loops {
            if (i, j) != (0, 1) {
                let mut q = vec![0; loops];
                q[i] = 1;
                q[j] = 1;
                momenta.push(q);
            }
        }
    }
    let denominators = momenta
        .iter()
        .map(|q| {
            AffineDenominator::new(
                context.integer(-1),
                (0..loops)
                    .flat_map(|i| (i..loops).map(move |j| q[i] * q[j] * if i == j { 1 } else { 2 }))
                    .map(|n| context.integer(n))
                    .collect(),
            )
        })
        .collect();
    let family = Arc::new(
        IntegralFamily::new(
            "profile-five-loop-fixture",
            (0..loops).map(|i| format!("k{i}")).collect(),
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            denominators,
            vec![],
            vec![context.zero(); momenta.len()],
        )
        .unwrap(),
    );
    let mut scalar = vec![0; momenta.len()];
    scalar[..=loops].fill(1);
    let mut numerator = scalar.clone();
    numerator[loops + 1] = -1;
    let mut raw = BTreeSet::from([
        IntegralKey::try_new(scalar.clone()).unwrap(),
        IntegralKey::try_new(numerator).unwrap(),
    ]);
    for slot in 0..=loops {
        let mut pinch = scalar.clone();
        pinch[slot] = 0;
        raw.insert(IntegralKey::try_new(pinch).unwrap());
    }
    (family, raw)
}

#[test]
fn versioned_profiles_pin_every_limit_and_legacy_hash_identity() {
    let legacy = json!({});
    assert_eq!(profile::from_report(&legacy).unwrap(), ConservativeV1);
    let baseline = blake3::Hasher::new();
    let mut conservative = baseline.clone();
    ConservativeV1.hash(&mut conservative);
    let mut standard = baseline.clone();
    StandardV1.hash(&mut standard);
    assert_eq!(baseline.finalize(), conservative.finalize());
    assert_ne!(baseline.finalize(), standard.finalize());
    for selected in [ConservativeV1, StandardV1] {
        let mut report = json!({});
        profile::record(&mut report, selected);
        assert_eq!(profile::from_report(&report).unwrap(), selected);
        assert_eq!(
            report["normalization_limits"]["parametric"]["symanzik"]["max_polynomial_terms"],
            if selected == StandardV1 {
                4_000_000
            } else {
                20_000
            }
        );
        assert_eq!(
            report["normalization_limits"]["parametric"]["max_supports"],
            4096
        );
        let mut missing = report.clone();
        missing
            .as_object_mut()
            .unwrap()
            .remove("normalization_limits");
        assert!(profile::from_report(&missing).is_err());
        report["normalization_limits"]["parametric"]["symanzik"]["max_polynomial_terms"] = json!(1);
        assert!(profile::from_report(&report).is_err());
    }
    assert!(profile::from_report(&json!({"normalization_profile":"standard-v2"})).is_err());
    assert!(profile::from_report(&json!({"normalization_limits":{}})).is_err());
    assert_eq!(
        TerminalRelationLimits::default()
            .normalization
            .parametric
            .symanzik
            .max_polynomial_terms,
        20_000
    );
    assert_eq!(
        profile::for_import(None, StandardV1, MasterReductionOperation::Publish).unwrap(),
        StandardV1
    );
    assert!(
        profile::for_import(
            Some(ConservativeV1),
            StandardV1,
            MasterReductionOperation::Publish
        )
        .is_err()
    );
}

#[test]
fn standard_profile_replays_a_genuinely_different_normalization_plan() {
    let (family, raw) = wide_geometry();
    let conservative = TerminalRelationSession::new(
        family.clone(),
        raw.clone(),
        0,
        ConservativeV1.relation_limits(),
    )
    .unwrap();
    let standard =
        TerminalRelationSession::new(family, raw, 0, StandardV1.relation_limits()).unwrap();
    assert_eq!(standard.normalization().canonical_terminals().len(), 2);
    assert_ne!(
        conservative.normalization().canonical_terminals(),
        standard.normalization().canonical_terminals()
    );
    for (selected, native) in [(ConservativeV1, conservative), (StandardV1, standard)] {
        let scratch = Scratch::new();
        let options = MasterReductionOptions::new("unused", &scratch.0);
        let mut report = json!({"schema":SCHEMA,"checkpoint":{"generation":0}});
        profile::record(&mut report, selected);
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
        assert_eq!(
            restored.normalization().terms(),
            native.normalization().terms()
        );
        let opposite = if selected == StandardV1 {
            ConservativeV1
        } else {
            StandardV1
        };
        let mut wrong = report.clone();
        profile::record(&mut wrong, opposite);
        write_json(&scratch.0.join("latest.json"), &wrong).unwrap();
        assert!(load_master_relation_session(&scratch.0).is_err());
        if selected == ConservativeV1 {
            report
                .as_object_mut()
                .unwrap()
                .remove("normalization_profile");
            report
                .as_object_mut()
                .unwrap()
                .remove("normalization_limits");
            write_json(&scratch.0.join("latest.json"), &report).unwrap();
            assert_eq!(
                load_master_relation_session(&scratch.0)
                    .unwrap()
                    .normalization()
                    .terms(),
                native.normalization().terms()
            );
        }
    }
}

fn completed_source(directory: &Path, selected: MasterNormalizationProfile) -> Value {
    let sample = session();
    let mut native = TerminalRelationSession::new(
        sample.family_owner().clone(),
        sample.raw_terminals().clone(),
        1,
        selected.relation_limits(),
    )
    .unwrap();
    while !native.is_complete() {
        native.step(&AtomicBool::new(false)).unwrap();
    }
    let options = MasterReductionOptions::new("unused", directory);
    let mut report = json!({"schema":SCHEMA,"checkpoint":{"generation":0},"operation":"refine",
        "inventory":{"complete":true},"inputs":portable_inputs(directory),"containing_sector_depth":1});
    profile::record(&mut report, selected);
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

#[test]
fn profile_switch_is_fresh_durable_and_omitted_resume_inherits_checkpoint() {
    for (old, new) in [(ConservativeV1, StandardV1), (StandardV1, ConservativeV1)] {
        let source = Scratch::new();
        completed_source(&source.0, old);
        let source_bytes = std::fs::read(source.0.join("latest.json")).unwrap();
        let output = Scratch::new();
        let mut options = MasterReductionOptions::new("unused", &output.0);
        options.normalization_profile = Some(new);
        let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            master_refine_published_artifact(
                &source.0,
                &options,
                &AtomicBool::new(false),
                |event| {
                    if event["event"] == "master_reduction_checkpoint" {
                        panic!("durable profile checkpoint");
                    }
                },
            )
        }));
        assert!(interrupted.is_err());
        let report = master_reduction_inspect(&output.0).unwrap();
        assert_eq!(profile::from_report(&report).unwrap(), new);
        assert_eq!(
            report["finite_search_restarted_for_normalization_profile"],
            true
        );
        assert_eq!(report["seed_depth"], 1);
        assert_eq!(report["containing_sector_depth"], 1);
        assert_eq!(
            load_master_relation_session(&output.0).unwrap().raw_terminals(),
            load_master_relation_session(&source.0).unwrap().raw_terminals()
        );
        assert_eq!(
            load_master_relation_session(&output.0)
                .unwrap()
                .statistics()
                .completed_source_rows,
            0
        );
        options.resume = true;
        options.normalization_profile = Some(old);
        assert!(
            master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {})
                .is_err()
        );
        options.normalization_profile = None;
        let done =
            master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {})
                .unwrap();
        assert_eq!(profile::from_report(&done).unwrap(), new);
        assert_eq!(done["status"], "completed_nonminimal");
        options.normalization_profile = Some(old);
        assert!(
            master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {})
                .is_err()
        );
        assert_eq!(
            std::fs::read(source.0.join("latest.json")).unwrap(),
            source_bytes
        );
    }
}

#[test]
fn omitted_new_profile_inherits_source_and_provider_policy_resets_keep_it() {
    let source = Scratch::new();
    completed_source(&source.0, StandardV1);
    let output = Scratch::new();
    let options = MasterReductionOptions::new("unused", &output.0);
    let report =
        master_refine_published_artifact(&source.0, &options, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(profile::from_report(&report).unwrap(), StandardV1);
    assert_eq!(
        report["finite_search_restarted_for_normalization_profile"],
        false
    );
    let (family, raw) = wide_geometry();
    let mut native =
        TerminalRelationSession::new(family, raw, 0, StandardV1.relation_limits()).unwrap();
    native.step(&AtomicBool::new(false)).unwrap();
    let mut options = options;
    options.circuit_symmetry_assistance = true;
    let mut report = json!({});
    profile::record(&mut report, StandardV1);
    assistance::configure(&options, &mut native, &mut report).unwrap();
    assert_eq!(native.statistics().completed_source_rows, 0);
    assert_eq!(native.normalization().canonical_terminals().len(), 2);
    let standard_binding = native.assistance_binding().unwrap().to_owned();
    let mut wrong = report.clone();
    profile::record(&mut wrong, ConservativeV1);
    assert!(assistance::validate(&options, &native, &wrong).is_err());
    assert_eq!(native.assistance_binding(), Some(standard_binding.as_str()));
}
