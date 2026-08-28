use std::io::Write;
use std::process::{Command, Output, Stdio};

const ONE_LOOP: &str = r#"
I(
  name(tadpole),
  loops(k),
  externals(),
  dimension(d),
  prop(D1,k^2-m2,1),
  numerator(sp(k,k))
)
"#;

const COMPACT_ROOT: &str = r#"
[[roots]]
id = "compact"
integral = """
I(
  name(tadpole),
  loops(k),
  externals(),
  dimension(d),
  prop(D1,k^2-m2,1)
)
"""

[roots.metadata]
spelling = "Symbolica"
"#;

const EXPLICIT_ROOT: &str = r#"
[[roots]]
id = "explicit"

[roots.project]
schema = "rustred.project.toml.v1"

[roots.project.family]
name = "tadpole"
loop_momenta = ["k"]
external_momenta = []
dimension = "d"

[[roots.project.family.denominators]]
id = "D1"
expression = "k^2-m2"

[roots.project.kinematics]
external_gram = []

[roots.project.target]
powers = [2]
numerator = "1"

[roots.project.metadata]
spelling = "explicit"
"#;

fn rustred(arguments: &[&str], input: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rustred"))
        .args(arguments)
        .env("SYMBOLICA_HIDE_BANNER", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn RustRed campaign CLI");
    child
        .stdin
        .take()
        .expect("campaign stdin")
        .write_all(input.as_bytes())
        .expect("write campaign input");
    child.wait_with_output().expect("wait for campaign CLI")
}

fn successful_toml(arguments: &[&str], input: &str) -> (Output, toml::Value) {
    let output = rustred(arguments, input);
    assert_eq!(
        output.status.code(),
        Some(0),
        "RustRed campaign plan failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let document: toml::Value =
        toml::from_str(std::str::from_utf8(&output.stdout).expect("UTF-8 output"))
            .expect("valid campaign-plan TOML");
    assert!(
        !document
            .as_table()
            .expect("campaign-plan document")
            .contains_key("phases"),
        "roots-only output must not advertise unimplemented future phases"
    );
    (output, document)
}

fn assert_input_error(arguments: &[&str], input: &str, detail: &str) {
    let output = rustred(arguments, input);
    assert_eq!(output.status.code(), Some(4));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.starts_with("rustred: input:"), "{stderr}");
    assert!(stderr.contains(detail), "missing {detail:?}: {stderr}");
}

fn compact_campaign_root(id: &str, family: &str, power: i64) -> String {
    format!(
        r#"
[[roots]]
id = "{id}"
integral = """
I(
  name({family}),
  loops(k),
  externals(),
  dimension(d),
  prop(D1,k^2-m2,{power})
)
"""
"#
    )
}

#[test]
fn raw_symbolica_convenience_is_deterministic_and_truthfully_roots_only() {
    let arguments = ["campaign", "plan", "--root-id", "raw-tadpole"];
    let (first, document) = successful_toml(&arguments, ONE_LOOP);
    let (second, _) = successful_toml(&arguments, ONE_LOOP);
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(
        document["schema"].as_str(),
        Some("rustred.campaign-plan-output.toml.v1")
    );
    assert_eq!(document["status"].as_str(), Some("ok"));
    assert_eq!(document["scope"].as_str(), Some("roots_only"));
    assert_eq!(document["counts"]["roots"].as_integer(), Some(1));
    assert_eq!(document["counts"]["unique_families"].as_integer(), Some(1));
    assert_eq!(
        document["counts"]["declared_power_jobs"].as_integer(),
        Some(1)
    );
    assert!(
        !document["counts"]
            .as_table()
            .expect("campaign counts")
            .contains_key("dependency_edges")
    );
    assert_eq!(document["roots"][0]["id"].as_str(), Some("raw-tadpole"));
    assert_eq!(
        document["roots"][0]["declared_power_sector"].as_str(),
        Some("1")
    );
    assert_eq!(
        document["roots"][0]["detected_input_form"].as_str(),
        Some("raw_symbolica")
    );
    assert!(
        !document["declared_power_jobs"][0]
            .as_table()
            .expect("declared-power job")
            .contains_key("dependency_count")
    );
}

#[test]
fn compact_and_nested_explicit_roots_reuse_one_family_and_job() {
    let input =
        format!("schema = \"rustred.campaign-input.toml.v1\"\n{COMPACT_ROOT}\n{EXPLICIT_ROOT}");
    let (_, document) = successful_toml(&["campaign", "plan", "--input-format", "toml"], &input);
    assert_eq!(document["counts"]["roots"].as_integer(), Some(2));
    assert_eq!(document["counts"]["unique_families"].as_integer(), Some(1));
    assert_eq!(
        document["counts"]["declared_power_jobs"].as_integer(),
        Some(1)
    );
    let roots = document["roots"].as_array().expect("campaign roots");
    assert_eq!(roots[0]["id"].as_str(), Some("compact"));
    assert_eq!(roots[1]["id"].as_str(), Some("explicit"));
    assert_eq!(roots[0]["family"], roots[1]["family"]);
    assert_eq!(
        roots[0]["declared_power_job"],
        roots[1]["declared_power_job"]
    );
    assert_eq!(roots[0]["declared_power_sector"].as_str(), Some("1"));
    assert_eq!(roots[1]["declared_power_sector"].as_str(), Some("1"));
    assert_eq!(
        roots[0]["detected_input_form"].as_str(),
        Some("campaign_symbolica")
    );
    assert_eq!(
        roots[1]["detected_input_form"].as_str(),
        Some("explicit_toml")
    );
    assert_eq!(roots[0]["metadata"]["spelling"].as_str(), Some("Symbolica"));
    assert_eq!(roots[1]["metadata"]["spelling"].as_str(), Some("explicit"));
}

#[test]
fn root_order_does_not_change_distinct_family_or_sector_job_bytes() {
    let active = compact_campaign_root("z-active", "ordering_family", 1);
    let inactive = compact_campaign_root("a-inactive", "ordering_family", 0);
    let other_family = compact_campaign_root("m-other", "other_family", 1);
    let first = format!(
        "schema = \"rustred.campaign-input.toml.v1\"\n{active}\n{other_family}\n{inactive}"
    );
    let second = format!(
        "schema = \"rustred.campaign-input.toml.v1\"\n{inactive}\n{active}\n{other_family}"
    );
    let arguments = ["campaign", "plan", "--input-format", "toml"];
    let (first, document) = successful_toml(&arguments, &first);
    let (second, _) = successful_toml(&arguments, &second);
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(document["counts"]["unique_families"].as_integer(), Some(2));
    assert_eq!(
        document["counts"]["declared_power_jobs"].as_integer(),
        Some(3)
    );
    let jobs = document["declared_power_jobs"]
        .as_array()
        .expect("declared-power jobs");
    assert_eq!(jobs.len(), 3);
    assert!(
        jobs.iter()
            .any(|job| job["declared_power_sector"].as_str() == Some("0"))
    );
    assert_eq!(
        jobs.iter()
            .filter(|job| job["declared_power_sector"].as_str() == Some("1"))
            .count(),
        2
    );
}

#[test]
fn numerator_cancellation_is_retained_but_not_claimed_as_a_normalized_sector() {
    let input = r#"
I(
  name(cancellation),
  loops(k),
  externals(),
  dimension(d),
  prop(D1,k^2-m2,1),
  numerator(k^2-m2)
)
"#;
    let (_, document) = successful_toml(
        &[
            "campaign",
            "plan",
            "--input-format",
            "symbolica",
            "--root-id",
            "cancellation",
        ],
        input,
    );
    let root = document["roots"][0].as_table().expect("cancellation root");
    assert_eq!(
        root["declared_power_sector"].as_str(),
        Some("1"),
        "the declared denominator power is active before cancellation"
    );
    assert!(!root.contains_key("target_sector"));
    assert!(!root.contains_key("normalized_sector"));
    assert!(
        root["canonical_integral"]
            .as_str()
            .expect("canonical cancellation input")
            .contains("numerator")
    );
}

#[test]
fn raw_and_toml_root_identifier_contracts_are_strict() {
    assert_input_error(
        &["campaign", "plan", "--input-format", "symbolica"],
        ONE_LOOP,
        "requires root_id",
    );
    let input = format!("schema = \"rustred.campaign-input.toml.v1\"\n{COMPACT_ROOT}");
    assert_input_error(
        &[
            "campaign",
            "plan",
            "--input-format",
            "toml",
            "--root-id",
            "override",
        ],
        &input,
        "only valid for one raw Symbolica campaign input",
    );
    let duplicate = format!(
        "schema = \"rustred.campaign-input.toml.v1\"\n{COMPACT_ROOT}\n{}",
        COMPACT_ROOT.replace("spelling = \"Symbolica\"", "spelling = \"duplicate\"")
    );
    assert_input_error(
        &["campaign", "plan", "--input-format", "toml"],
        &duplicate,
        "occurs more than once",
    );
}

#[test]
fn malformed_root_choice_diagnostics_name_the_root_once() {
    let both = r#"
schema = "rustred.campaign-input.toml.v1"
[[roots]]
id = "both"
integral = "I(loops(k))"
[roots.project]
schema = "rustred.project.toml.v1"
"#;
    let neither = r#"
schema = "rustred.campaign-input.toml.v1"
[[roots]]
id = "neither"
"#;
    for (input, id, detail) in [
        (both, "both", "must choose exactly one"),
        (neither, "neither", "needs either an integral"),
    ] {
        let output = rustred(&["campaign", "plan", "--input-format", "toml"], input);
        assert_eq!(output.status.code(), Some(4));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        let root_label = format!("campaign root \"{id}\"");
        assert_eq!(
            stderr.matches(&root_label).count(),
            1,
            "root label was duplicated in diagnostic: {stderr}"
        );
        assert!(stderr.contains(detail), "{stderr}");
    }
}

#[test]
fn plan_only_command_rejects_execution_resource_options() {
    for (option, value) in [("--n-cores", "4"), ("--max-memory", "1TiB")] {
        let output = rustred(&["campaign", "plan", option, value], "");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.starts_with("rustred: usage:"), "{stderr}");
        assert!(stderr.contains("unknown option"), "{stderr}");
    }
}
