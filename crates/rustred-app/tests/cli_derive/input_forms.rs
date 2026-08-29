use super::support::*;

#[test]
fn raw_hybrid_and_explicit_lower_to_the_same_semantic_family_and_rows() {
    let (_, raw) = successful_toml(&["derive", "--input-format", "symbolica"], TWO_LOOP_RAW);
    let (_, hybrid) = successful_toml(&["derive", "--input-format", "toml"], TWO_LOOP_HYBRID);
    let (_, explicit) = successful_toml(&["derive", "--input-format", "toml"], TWO_LOOP_EXPLICIT);
    for candidate in [&hybrid, &explicit] {
        assert_eq!(raw["family"], candidate["family"]);
        assert_eq!(raw["target"], candidate["target"]);
        assert_eq!(raw["coordinates"], candidate["coordinates"]);
        assert_eq!(
            semantic_denominators(&raw),
            semantic_denominators(candidate)
        );
        assert_eq!(raw["domain_conditions"], candidate["domain_conditions"]);
        assert_eq!(raw["relations"], candidate["relations"]);
        assert_eq!(
            raw["provenance"]["canonical_integral"],
            candidate["provenance"]["canonical_integral"]
        );
    }
    for condition in raw["domain_conditions"]
        .as_array()
        .expect("family domain-condition array")
    {
        assert!(
            !condition["sources"]
                .as_array()
                .expect("family condition sources")
                .is_empty()
        );
        assert!(condition.get("origins").is_none());
    }
    let (_, conditioned) = successful_toml(
        &["derive", "--input-format", "symbolica"],
        "I(name(conditioned),loops(k),externals(),parameters(d,s),dimension(d/s),prop(D1,k^2,1))",
    );
    let relation_conditions = conditioned["relations"][0]["nonzero_conditions"]
        .as_array()
        .expect("relation nonzero-condition array");
    assert!(!relation_conditions.is_empty());
    for condition in relation_conditions {
        assert!(
            !condition["sources"]
                .as_array()
                .expect("relation condition sources")
                .is_empty()
        );
        assert!(condition.get("origins").is_none());
    }
    assert_eq!(
        hybrid["relation_counts"]["generated_ordinary"].as_integer(),
        Some(4)
    );
    assert_eq!(
        raw["family"]["parameters"]
            .as_array()
            .expect("parameter array")
            .iter()
            .map(|value| value.as_str().expect("parameter string"))
            .collect::<Vec<_>>(),
        vec!["d", "m2"]
    );
    assert_eq!(
        raw["provenance"]["parameter_source"].as_str(),
        Some("inferred")
    );
    assert_eq!(
        raw["provenance"]["input_parameters"],
        raw["family"]["parameters"]
    );
    assert_eq!(
        hybrid["provenance"]["metadata"]["tags"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn matching_outer_parameters_are_retained_and_order_conflicts_are_rejected() {
    let outer_only = hybrid_with_outer_only("[\"m2\", \"d\"]");
    let (_, supplied) = successful_toml(&["derive", "--input-format", "toml"], &outer_only);
    assert_eq!(
        supplied["provenance"]["parameter_source"].as_str(),
        Some("declared")
    );
    assert_eq!(
        supplied["provenance"]["input_parameters"]
            .as_array()
            .expect("source parameter array")
            .iter()
            .map(|value| value.as_str().expect("source parameter string"))
            .collect::<Vec<_>>(),
        vec!["m2", "d"]
    );
    assert_eq!(
        supplied["family"]["parameters"]
            .as_array()
            .expect("operational parameter array")
            .iter()
            .map(|value| value.as_str().expect("operational parameter string"))
            .collect::<Vec<_>>(),
        vec!["d", "m2"]
    );

    let matching = hybrid_with_parameter_declarations("[\"d\", \"m2\"]");
    let (_, document) = successful_toml(&["derive", "--input-format", "toml"], &matching);
    assert_eq!(
        document["provenance"]["parameter_source"].as_str(),
        Some("declared")
    );
    assert_eq!(
        document["provenance"]["input_parameters"],
        document["family"]["parameters"]
    );

    let conflicting = hybrid_with_parameter_declarations("[\"m2\", \"d\"]");
    let output = rustred(&["derive", "--input-format", "toml"], &conflicting);
    assert_eq!(output.status.code(), Some(4));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("conflict"));
}

#[test]
fn numerator_only_parameter_is_provenance_not_an_ibp_base_variable() {
    let active_only = hybrid_with_outer_and_numerator("[\"m2\", \"d\"]");
    let declared_superset = hybrid_with_outer_and_numerator("[\"xi\", \"m2\", \"d\"]");
    let (_, baseline) = successful_toml(&["derive", "--input-format", "toml"], &active_only);
    let (_, superset) = successful_toml(&["derive", "--input-format", "toml"], &declared_superset);

    assert_eq!(
        superset["provenance"]["input_parameters"]
            .as_array()
            .expect("declared parameter array")
            .iter()
            .map(|value| value.as_str().expect("declared parameter string"))
            .collect::<Vec<_>>(),
        vec!["xi", "m2", "d"]
    );
    assert_eq!(
        superset["family"]["parameters"]
            .as_array()
            .expect("active parameter array")
            .iter()
            .map(|value| value.as_str().expect("active parameter string"))
            .collect::<Vec<_>>(),
        vec!["d", "m2"]
    );
    assert_eq!(baseline["family"], superset["family"]);
    assert_eq!(baseline["relations"], superset["relations"]);
    assert_ne!(
        baseline["provenance"]["canonical_integral"],
        superset["provenance"]["canonical_integral"]
    );
    assert!(
        superset["provenance"]["canonical_integral"]
            .as_str()
            .expect("canonical Symbolica input")
            .contains("xi")
    );
}

#[test]
fn concrete_target_metadata_never_specializes_universal_rows() {
    let (_, baseline) = successful_toml(&["derive", "--input-format", "symbolica"], TWO_LOOP_RAW);
    let (_, alternate) = successful_toml(
        &["derive", "--input-format", "symbolica"],
        TWO_LOOP_ALTERNATE_TARGET,
    );
    assert_eq!(baseline["family"], alternate["family"]);
    assert_eq!(baseline["relations"], alternate["relations"]);
    assert_ne!(baseline["target"], alternate["target"]);
    assert_ne!(
        baseline["provenance"]["canonical_integral"],
        alternate["provenance"]["canonical_integral"]
    );
}
