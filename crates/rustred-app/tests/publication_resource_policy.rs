use rustred_app::{
    AppErrorKind, ArtifactLoadLimits, ClosingArtifactInspectRequest, ClosingArtifactReduceRequest,
    FamilyCloseRequest, SourcePortLimits, closing_artifact_inspect, closing_artifact_reduce,
    family_close,
};

const K1: &str = r#"
schema = "rustred.project.toml.v1"
[family]
name = "resource_policy_external_family"
loop_momenta = ["q"]
external_momenta = []
dimension = "d"
[[family.denominators]]
id = "P"
expression = "q^2-1"
[target]
powers = [1]
"#;

fn report(source: &str) -> toml::Value {
    toml::from_str(source).unwrap()
}

#[test]
fn request_constructors_take_resource_defaults_from_core() {
    assert_eq!(
        FamilyCloseRequest::new(K1).publication_limits,
        SourcePortLimits::default()
    );
    assert_eq!(
        ClosingArtifactInspectRequest::new(Vec::new()).load_limits,
        ArtifactLoadLimits::default()
    );
    assert_eq!(
        ClosingArtifactReduceRequest::new(Vec::new(), vec![1]).load_limits,
        ArtifactLoadLimits::default()
    );
}

#[test]
fn publication_and_cold_load_report_independent_policies_without_changing_payload() {
    let baseline = family_close(FamilyCloseRequest::new(K1)).unwrap();
    let mut chosen = FamilyCloseRequest::new(K1);
    chosen
        .publication_limits
        .rule_derivation
        .max_domain_bound_endpoint_cells = 65_536;
    chosen.publication_limits.max_predicate_consistency_work = 67_108_864;
    chosen.publication_limits.max_predicate_atoms = 64;
    let chosen = family_close(chosen).unwrap();
    assert!(
        rustred_app::equivalent_generated_programs(
            baseline.artifact(),
            chosen.artifact(),
            Default::default()
        )
        .unwrap()
    );
    let publication = &report(chosen.to_toml())["publication_resources"];
    assert_eq!(publication["max_predicate_atoms"].as_str(), Some("64"));
    assert_eq!(
        publication["max_domain_bound_endpoint_cells"].as_str(),
        Some("65536")
    );
    assert_eq!(
        publication["max_predicate_consistency_work"].as_str(),
        Some("67108864")
    );

    let default_inspection =
        closing_artifact_inspect(ClosingArtifactInspectRequest::new(chosen.artifact())).unwrap();
    let defaults = ArtifactLoadLimits::default();
    assert_eq!(
        report(default_inspection.to_toml())["load_resources"]["max_predicate_atoms"].as_str(),
        Some("32")
    );
    assert_eq!(
        report(default_inspection.to_toml())["load_resources"]["max_predicate_consistency_work"]
            .as_str(),
        Some(defaults.max_predicate_consistency_work.to_string().as_str())
    );
    let mut limits = defaults;
    limits.rule_derivation.max_domain_bound_endpoint_cells = usize::MAX;
    limits.max_predicate_consistency_work = usize::MAX;
    limits.max_predicate_atoms = SourcePortLimits::MAX_PREDICATE_ATOMS;
    // Increasing other caller limits cannot change the app's ingress ceiling.
    limits.max_artifact_bytes = usize::MAX;
    let inspected = closing_artifact_inspect(ClosingArtifactInspectRequest {
        artifact: chosen.artifact().to_vec(),
        load_limits: limits,
    })
    .unwrap();
    let reduced = closing_artifact_reduce(ClosingArtifactReduceRequest {
        load_limits: limits,
        ..ClosingArtifactReduceRequest::new(chosen.artifact(), vec![3])
    })
    .unwrap();
    assert_eq!(
        report(inspected.to_toml())["artifact"],
        report(default_inspection.to_toml())["artifact"]
    );
    for source in [inspected.to_toml(), reduced.to_toml()] {
        let resources = &report(source)["load_resources"];
        assert_eq!(resources["max_predicate_atoms"].as_str(), Some("256"));
        for key in [
            "max_domain_bound_endpoint_cells",
            "max_predicate_consistency_work",
        ] {
            assert_eq!(
                resources[key].as_str(),
                Some(usize::MAX.to_string().as_str())
            );
        }
    }
    assert_eq!(reduced.terms().len(), 1);
    assert_eq!(reduced.terms()[0].master_powers(), &[1]);
    let mut no_consistency_work = FamilyCloseRequest::new(K1);
    no_consistency_work
        .publication_limits
        .max_predicate_consistency_work = 0;
    no_consistency_work.publication_limits.max_predicate_atoms = 0;
    let zero = family_close(no_consistency_work).unwrap();
    assert!(
        rustred_app::equivalent_generated_programs(
            zero.artifact(),
            baseline.artifact(),
            Default::default()
        )
        .unwrap()
    );
    let mut zero_load = ClosingArtifactInspectRequest::new(zero.artifact());
    zero_load.load_limits.max_predicate_consistency_work = 0;
    zero_load.load_limits.max_predicate_atoms = 0;
    let zero_report = closing_artifact_inspect(zero_load).unwrap();
    assert_eq!(
        report(zero_report.to_toml())["load_resources"]["max_predicate_atoms"].as_str(),
        Some("0")
    );
    assert_eq!(
        report(zero_report.to_toml())["load_resources"]["max_predicate_consistency_work"].as_str(),
        Some("0")
    );
}

#[test]
fn unsupported_atom_policies_fail_early_on_all_application_entry_points() {
    for atoms in [257, usize::MAX] {
        let mut generation = FamilyCloseRequest::new("not a project");
        generation.publication_limits.max_predicate_atoms = atoms;
        let limits = ArtifactLoadLimits {
            max_predicate_atoms: atoms,
            ..Default::default()
        };
        for error in [
            family_close(generation).unwrap_err(),
            closing_artifact_inspect(ClosingArtifactInspectRequest {
                artifact: b"not an artifact".to_vec(),
                load_limits: limits,
            })
            .unwrap_err(),
            closing_artifact_reduce(ClosingArtifactReduceRequest {
                load_limits: limits,
                ..ClosingArtifactReduceRequest::new(b"not an artifact".to_vec(), vec![1])
            })
            .unwrap_err(),
        ] {
            assert_eq!(error.kind(), AppErrorKind::Limit);
            assert!(error.message().contains("supported"));
            assert!(error.message().contains("256"));
        }
    }
}

#[test]
fn restrictive_endpoint_and_byte_limits_fail_without_artifact_selected_policy() {
    let generated = family_close(FamilyCloseRequest::new(K1)).unwrap();
    let mut publication = FamilyCloseRequest::new(K1);
    publication
        .publication_limits
        .rule_derivation
        .max_domain_bound_endpoint_cells = 0;
    let error = family_close(publication).unwrap_err();
    assert!(error.message().contains("requested"));
    assert!(error.message().contains("limit 0"));
    let mut limits = ArtifactLoadLimits::default();
    limits.rule_derivation.max_domain_bound_endpoint_cells = 0;
    for error in [
        closing_artifact_inspect(ClosingArtifactInspectRequest {
            artifact: generated.artifact().to_vec(),
            load_limits: limits,
        })
        .unwrap_err(),
        closing_artifact_reduce(ClosingArtifactReduceRequest {
            load_limits: limits,
            ..ClosingArtifactReduceRequest::new(generated.artifact(), vec![3])
        })
        .unwrap_err(),
    ] {
        assert!(error.message().contains("requested"));
        assert!(error.message().contains("limit 0"));
    }
    limits = ArtifactLoadLimits::default();
    limits.max_artifact_bytes = generated.artifact().len() - 1;
    let error = closing_artifact_inspect(ClosingArtifactInspectRequest {
        artifact: generated.artifact().to_vec(),
        load_limits: limits,
    })
    .unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Limit);
    assert!(error.message().contains("program bytes"));
}
