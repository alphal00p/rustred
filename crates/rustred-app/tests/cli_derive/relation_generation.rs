use super::support::*;

#[test]
fn raw_symbolica_is_deterministic_and_emits_the_complete_one_loop_ibp() {
    let (first, document) = successful_toml(&["derive", "--input-format", "symbolica"], ONE_LOOP);
    let (second, _) = successful_toml(&["derive", "--input-format", "symbolica"], ONE_LOOP);
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(
        document["schema"].as_str(),
        Some("rustred.derive-output.toml.v1")
    );
    assert_eq!(document["status"].as_str(), Some("ok"));
    assert_eq!(
        document["provenance"]["detected_input_form"].as_str(),
        Some("raw_symbolica")
    );
    assert_eq!(
        document["family"]["denominator_count"].as_integer(),
        Some(1)
    );
    assert_eq!(
        document["relation_counts"]["generated_ordinary"].as_integer(),
        Some(1)
    );
    assert_eq!(
        document["relation_counts"]["generated_li"].as_integer(),
        Some(0)
    );
    assert_eq!(document["relations"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        document["equation_convention"].as_str(),
        Some("sum(term.coefficient * I(n + term.shift) for term in relation.terms) = 0")
    );
    let relation = &document["relations"][0];
    assert_eq!(relation["id"]["kind"].as_str(), Some("ordinary_ibp"));
    assert_eq!(relation["id"]["contraction_momentum"].as_integer(), Some(0));
    assert_eq!(relation["id"]["differentiated_loop"].as_integer(), Some(0));
    let terms = relation["terms"].as_array().expect("one-loop IBP terms");
    assert_eq!(terms.len(), 2);
    assert_eq!(integer_array(&terms[0]["shift"]), vec![0]);
    assert_eq!(integer_array(&terms[1]["shift"]), vec![1]);

    // Every context reuses the deterministic private positional-symbol pool.
    // Recover that qualified symbol from the zero-shift coefficient, then
    // require the independently serialized raised-power coefficient to reuse
    // it with the expected sign and factor.
    let zero_shift = terms[0]["coefficient"]
        .as_str()
        .expect("zero-shift coefficient");
    let index = zero_shift
        .strip_prefix("-2*")
        .and_then(|value| value.strip_suffix("+rustred::{}::d"))
        .expect("(d-2*n0) in canonical Symbolica order");
    assert_eq!(index, "rustred_indexed_coefficient_v1::{}::n0");
    let expected_raised = format!("-2*rustred::{{}}::m2*{index}");
    assert_eq!(
        terms[1]["coefficient"].as_str(),
        Some(expected_raised.as_str())
    );
    assert_eq!(
        document["target"]["disposition"].as_str(),
        Some("not_processed_by_derive")
    );
}

#[test]
fn auto_detection_and_relation_filters_are_effective() {
    let (_, raw_auto) = successful_toml(&["derive"], TWO_LOOP_RAW);
    assert_eq!(
        raw_auto["provenance"]["detected_input_form"].as_str(),
        Some("raw_symbolica")
    );
    for qualified_head in ["rustred::I", "rustred::{}::I"] {
        let qualified = TWO_LOOP_RAW.replacen("I(", &format!("{qualified_head}("), 1);
        let (_, qualified_auto) = successful_toml(&["derive"], &qualified);
        assert_eq!(
            qualified_auto["provenance"]["detected_input_form"].as_str(),
            Some("raw_symbolica")
        );
        assert_eq!(qualified_auto["family"], raw_auto["family"]);
        assert_eq!(qualified_auto["target"], raw_auto["target"]);
        assert_eq!(qualified_auto["relations"], raw_auto["relations"]);
        assert_eq!(
            qualified_auto["provenance"]["canonical_integral"],
            raw_auto["provenance"]["canonical_integral"]
        );
    }
    let (_, toml_auto) = successful_toml(&["derive"], TWO_LOOP_HYBRID);
    assert_eq!(
        toml_auto["provenance"]["detected_input_form"].as_str(),
        Some("hybrid_toml")
    );

    let (_, all) = successful_toml(&["derive", "--relations", "all"], ONE_LOOP_TWO_EXTERNALS);
    let (_, ordinary) = successful_toml(
        &["derive", "--relations", "ordinary"],
        ONE_LOOP_TWO_EXTERNALS,
    );
    assert_eq!(
        ordinary["relation_counts"]["generated_ordinary"].as_integer(),
        Some(3)
    );
    assert_eq!(
        ordinary["relation_counts"]["generated_li"].as_integer(),
        Some(0)
    );
    assert_eq!(ordinary["relations"].as_array().map(Vec::len), Some(3));
    assert_eq!(
        semantic_relations(&ordinary, None),
        semantic_relations(&all, Some("ordinary_ibp"))
    );

    let (_, li) = successful_toml(&["derive", "--relations", "li"], ONE_LOOP_TWO_EXTERNALS);
    assert_eq!(
        li["relation_counts"]["generated_ordinary"].as_integer(),
        Some(0)
    );
    assert_eq!(li["relation_counts"]["generated_li"].as_integer(), Some(1));
    assert_eq!(li["relations"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        li["relations"][0]["id"]["kind"].as_str(),
        Some("lorentz_invariance")
    );
    assert_eq!(
        semantic_relations(&li, None),
        semantic_relations(&all, Some("lorentz_invariance"))
    );
}
