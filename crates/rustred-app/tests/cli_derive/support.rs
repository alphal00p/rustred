use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) const ONE_LOOP: &str = r#"
I(
  name(tadpole),
  loops(k),
  externals(),
  dimension(d),
  prop(D1,k^2-m2,1),
  numerator(sp(k,k))
)
"#;
pub(crate) const TWO_LOOP_HYBRID: &str = r#"
schema = "rustred.project.toml.v1"

integral = """
I(
  name(sunset),
  loops(k1,k2),
  externals(),
  dimension(d),
  prop(D1,k1^2-m2,1),
  prop(D2,k2^2-m2,1),
  prop(D3,(k1+k2)^2-m2,1)
)
"""

[metadata]
tags = ["vacuum", "two-loop"]
"#;
pub(crate) const TWO_LOOP_EXPLICIT: &str = r#"
schema = "rustred.project.toml.v1"

[family]
name = "sunset"
loop_momenta = ["k1", "k2"]
external_momenta = []
dimension = "d"

[[family.denominators]]
id = "D1"
expression = "k1^2-m2"

[[family.denominators]]
id = "D2"
expression = "k2^2-m2"

[[family.denominators]]
id = "D3"
expression = "(k1+k2)^2-m2"

[kinematics]
external_gram = []

[target]
powers = [1, 1, 1]
numerator = "1"
"#;
pub(crate) const TWO_LOOP_RAW: &str = r#"
I(
  name(sunset),
  loops(k1,k2),
  externals(),
  dimension(d),
  prop(D1,k1^2-m2,1),
  prop(D2,k2^2-m2,1),
  prop(D3,(k1+k2)^2-m2,1)
)
"#;
pub(crate) const TWO_LOOP_ALTERNATE_TARGET: &str = r#"
I(
  name(sunset),
  loops(k1,k2),
  externals(),
  dimension(d),
  prop(D1,k1^2-m2,2),
  prop(D2,k2^2-m2,-1),
  prop(D3,(k1+k2)^2-m2,3),
  numerator(sp(k1,k2)^2)
)
"#;
pub(crate) const ONE_LOOP_TWO_EXTERNALS: &str = r#"
I(
  name(one_loop_two_externals),
  loops(k),
  externals(p,q),
  dimension(d),
  prop(D1,k^2,1),
  prop(D2,(k+p)^2,1),
  prop(D3,(k+q)^2,1),
  gram(p,p,s),
  gram(p,q,t),
  gram(q,q,u)
)
"#;

pub(crate) fn rustred_with_environment(
    arguments: &[&str],
    input: &str,
    environment: &[(&str, &str)],
    remove_symbolica_license: bool,
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rustred"));
    command.args(arguments).env("SYMBOLICA_HIDE_BANNER", "1");
    for &(name, value) in environment {
        command.env(name, value);
    }
    if remove_symbolica_license {
        command.env_remove("SYMBOLICA_LICENSE");
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the RustRed CLI");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(input.as_bytes())
        .expect("write CLI input");
    child.wait_with_output().expect("wait for RustRed")
}

pub(crate) fn rustred(arguments: &[&str], input: &str) -> Output {
    rustred_with_environment(arguments, input, &[], false)
}

pub(crate) fn rustred_without_input(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rustred"))
        .args(arguments)
        .env("SYMBOLICA_HIDE_BANNER", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run the RustRed CLI")
}

pub(crate) fn successful_toml(arguments: &[&str], input: &str) -> (Output, toml::Value) {
    successful_toml_with_environment(arguments, input, &[])
}

pub(crate) fn successful_toml_with_environment(
    arguments: &[&str],
    input: &str,
    environment: &[(&str, &str)],
) -> (Output, toml::Value) {
    let output = rustred_with_environment(arguments, input, environment, false);
    assert_eq!(
        output.status.code(),
        Some(0),
        "RustRed failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "successful diagnostics must be empty: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = std::str::from_utf8(&output.stdout).expect("UTF-8 derive output");
    let document = toml::from_str(stdout).expect("valid derive-output TOML");
    (output, document)
}

pub(crate) fn assert_input_error(output: Output, expected_detail: &str) {
    assert_eq!(output.status.code(), Some(4));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.starts_with("rustred: input:"),
        "unexpected diagnostic category: {stderr}"
    );
    assert!(
        stderr.contains(expected_detail),
        "missing diagnostic detail {expected_detail:?}: {stderr}"
    );
}

pub(crate) fn assert_usage_error(arguments: &[&str], expected_detail: &str) {
    let output = rustred_without_input(arguments);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.starts_with("rustred: usage:"),
        "unexpected diagnostic category: {stderr}"
    );
    assert!(
        stderr.contains(expected_detail),
        "missing diagnostic detail {expected_detail:?}: {stderr}"
    );
    assert!(
        stderr.contains("run `rustred --help` for the command contract"),
        "usage failure omitted the help hint: {stderr}"
    );
}

pub(crate) fn semantic_denominators(document: &toml::Value) -> Vec<toml::Value> {
    document["denominators"]
        .as_array()
        .expect("denominator array")
        .iter()
        .cloned()
        .map(|mut denominator| {
            let table = denominator.as_table_mut().expect("denominator table");
            table.remove("source_expression");
            table.remove("normalized_expression");
            denominator
        })
        .collect()
}

pub(crate) fn semantic_relations(document: &toml::Value, kind: Option<&str>) -> Vec<toml::Value> {
    document["relations"]
        .as_array()
        .expect("relation array")
        .iter()
        .filter(|relation| {
            kind.map(|wanted| relation["id"]["kind"].as_str() == Some(wanted))
                .unwrap_or(true)
        })
        .cloned()
        .map(|mut relation| {
            relation
                .as_table_mut()
                .expect("relation table")
                .remove("ordinal");
            relation
        })
        .collect()
}

pub(crate) fn integer_array(value: &toml::Value) -> Vec<i64> {
    value
        .as_array()
        .expect("integer array")
        .iter()
        .map(|entry| entry.as_integer().expect("integer array entry"))
        .collect()
}

pub(crate) fn unique_test_directory() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "rustred-cli-derive-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

pub(crate) fn hybrid_with_parameter_declarations(outer: &str) -> String {
    format!(
        r#"schema = "rustred.project.toml.v1"
parameters = {outer}
integral = """
I(
  name(sunset),
  parameters(d,m2),
  loops(k1,k2),
  externals(),
  dimension(d),
  prop(D1,k1^2-m2,1),
  prop(D2,k2^2-m2,1),
  prop(D3,(k1+k2)^2-m2,1)
)
"""
"#
    )
}

pub(crate) fn hybrid_with_outer_only(outer: &str) -> String {
    format!(
        r#"schema = "rustred.project.toml.v1"
parameters = {outer}
integral = """
I(
  name(sunset),
  loops(k1,k2),
  externals(),
  dimension(d),
  prop(D1,k1^2-m2,1),
  prop(D2,k2^2-m2,1),
  prop(D3,(k1+k2)^2-m2,1)
)
"""
"#
    )
}

pub(crate) fn hybrid_with_outer_and_numerator(outer: &str) -> String {
    format!(
        r#"schema = "rustred.project.toml.v1"
parameters = {outer}
integral = """
I(
  name(sunset),
  loops(k1,k2),
  externals(),
  dimension(d),
  prop(D1,k1^2-m2,1),
  prop(D2,k2^2-m2,1),
  prop(D3,(k1+k2)^2-m2,1),
  numerator(xi*sp(k1,k2))
)
"""
"#
    )
}
