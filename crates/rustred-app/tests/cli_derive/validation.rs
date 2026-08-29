use super::support::*;

#[test]
fn invalid_input_has_typed_stderr_and_never_partially_writes_stdout() {
    let output = rustred(
        &["derive", "--input-format", "symbolica"],
        "I(loops(k),externals(),dimension(d),mystery(x),prop(D1,k^2,1))",
    );
    assert_eq!(output.status.code(), Some(4));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).starts_with("rustred: input:"));
}

#[test]
fn declared_parameter_allowlists_reject_missing_family_scalars() {
    let raw = rustred(
        &["derive", "--input-format", "symbolica"],
        "I(loops(k),externals(),parameters(d),dimension(d),prop(D1,k^2-m2,1))",
    );
    assert_input_error(raw, "scalar symbol m2 is not present in parameters(...)");

    let hybrid = r#"schema = "rustred.project.toml.v1"
parameters = ["d"]
integral = """I(loops(k),externals(),dimension(d),prop(D1,k^2-m2,1))"""
"#;
    let hybrid = rustred(&["derive", "--input-format", "toml"], hybrid);
    assert_input_error(
        hybrid,
        "invalid hybrid Symbolica integral input: scalar symbol m2 is not present in parameters(...)",
    );
}

#[test]
fn hybrid_toml_rejects_explicit_sections_and_unknown_top_level_fields() {
    const BASE: &str = r#"schema = "rustred.project.toml.v1"
integral = """I(loops(k),externals(),dimension(d),prop(D1,k^2-m2,1))"""
"#;
    for forbidden_section in [
        "[kinematics]\nexternal_gram = []\n",
        "[target]\npowers = [1]\n",
    ] {
        let input = format!("{BASE}{forbidden_section}");
        let output = rustred(&["derive", "--input-format", "toml"], &input);
        assert_input_error(output, "only parameters and metadata may supplement it");
    }

    let unknown = r#"schema = "rustred.project.toml.v1"
bogus = "misspelled"
integral = """I(loops(k),externals(),dimension(d),prop(D1,k^2-m2,1))"""
"#;
    let output = rustred(&["derive", "--input-format", "toml"], unknown);
    assert_input_error(output, "unknown field `bogus`");
}

#[test]
fn hybrid_metadata_rejects_size_count_and_value_type_violations() {
    const BASE: &str = r#"schema = "rustred.project.toml.v1"
integral = """I(loops(k),externals(),dimension(d),prop(D1,k^2-m2,1))"""
"#;

    let oversized = format!("{BASE}[metadata]\noversized = \"{}\"\n", "x".repeat(65_537));
    let output = rustred(&["derive", "--input-format", "toml"], &oversized);
    assert_input_error(output, "exceeds 65536 bytes");

    let mut too_many = String::from(BASE);
    too_many.push_str("[metadata]\n");
    for ordinal in 0..1_025 {
        too_many.push_str(&format!("key_{ordinal} = \"x\"\n"));
    }
    let output = rustred(&["derive", "--input-format", "toml"], &too_many);
    assert_input_error(
        output,
        "metadata has 1025 entries, exceeding the limit 1024",
    );

    let invalid_type = format!("{BASE}[metadata]\ninvalid = 42\n");
    let output = rustred(&["derive", "--input-format", "toml"], &invalid_type);
    assert_input_error(
        output,
        "data did not match any variant of untagged enum MetadataValue",
    );
}
