use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

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
const TWO_LOOP_HYBRID: &str = r#"
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
const TWO_LOOP_EXPLICIT: &str = r#"
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
const TWO_LOOP_RAW: &str = r#"
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
const TWO_LOOP_ALTERNATE_TARGET: &str = r#"
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
const ONE_LOOP_TWO_EXTERNALS: &str = r#"
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

fn rustred_with_environment(
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

fn rustred(arguments: &[&str], input: &str) -> Output {
    rustred_with_environment(arguments, input, &[], false)
}

fn rustred_without_input(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rustred"))
        .args(arguments)
        .env("SYMBOLICA_HIDE_BANNER", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run the RustRed CLI")
}

fn successful_toml(arguments: &[&str], input: &str) -> (Output, toml::Value) {
    successful_toml_with_environment(arguments, input, &[])
}

fn successful_toml_with_environment(
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

fn assert_input_error(output: Output, expected_detail: &str) {
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

fn assert_usage_error(arguments: &[&str], expected_detail: &str) {
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

fn semantic_denominators(document: &toml::Value) -> Vec<toml::Value> {
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

fn semantic_relations(document: &toml::Value, kind: Option<&str>) -> Vec<toml::Value> {
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

fn integer_array(value: &toml::Value) -> Vec<i64> {
    value
        .as_array()
        .expect("integer array")
        .iter()
        .map(|entry| entry.as_integer().expect("integer array entry"))
        .collect()
}

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

    // The index symbol is deliberately scoped by a dynamic family fingerprint.
    // Recover that exact qualified symbol from the zero-shift coefficient, then
    // require the independently serialized raised-power coefficient to reuse it
    // with the expected massive-tadpole sign and factor.
    let zero_shift = terms[0]["coefficient"]
        .as_str()
        .expect("zero-shift coefficient");
    let index = zero_shift
        .strip_prefix("-2*")
        .and_then(|value| value.strip_suffix("+rustred::{}::d"))
        .expect("(d-2*n0) in canonical Symbolica order");
    assert!(
        index.starts_with("rustred::parametric_s") && index.ends_with("::{}::n0"),
        "unexpected fully qualified parametric index {index:?}"
    );
    let expected_raised = format!("-2*{index}*rustred::{{}}::m2");
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
fn licensed_available_core_widths_through_four_are_byte_identical() {
    let serial_arguments = [
        "derive",
        "--input-format",
        "symbolica",
        "--relations",
        "all",
        "--n-cores",
        "1",
    ];
    let (serial, serial_document) = successful_toml(&serial_arguments, ONE_LOOP_TWO_EXTERNALS);
    let available = std::thread::available_parallelism().unwrap().get();
    for n_cores in [2_usize, 3, 4]
        .into_iter()
        .filter(|width| *width <= available)
    {
        let n_cores = n_cores.to_string();
        let parallel_arguments = [
            "derive",
            "--input-format",
            "symbolica",
            "--relations",
            "all",
            "--n-cores",
            n_cores.as_str(),
        ];
        let (parallel, parallel_document) =
            successful_toml(&parallel_arguments, ONE_LOOP_TWO_EXTERNALS);
        assert_eq!(
            parallel.stdout, serial.stdout,
            "--n-cores {n_cores} changed the canonical derive output"
        );
        assert_eq!(parallel_document, serial_document);
    }
}

#[test]
fn rayon_global_environment_cannot_override_explicit_n_cores() {
    let requested = std::thread::available_parallelism().unwrap().get().min(4);
    if requested < 2 {
        return;
    }
    let requested = requested.to_string();
    let serial_arguments = ["derive", "--input-format", "symbolica", "--n-cores", "1"];
    let parallel_arguments = [
        "derive",
        "--input-format",
        "symbolica",
        "--n-cores",
        requested.as_str(),
    ];
    let (serial, serial_document) = successful_toml_with_environment(
        &serial_arguments,
        ONE_LOOP_TWO_EXTERNALS,
        &[("RAYON_NUM_THREADS", "32")],
    );
    let (parallel, parallel_document) = successful_toml_with_environment(
        &parallel_arguments,
        ONE_LOOP_TWO_EXTERNALS,
        &[("RAYON_NUM_THREADS", "1")],
    );
    assert_eq!(parallel.stdout, serial.stdout);
    assert_eq!(parallel_document, serial_document);

    // The global Rayon setting must not silently downgrade the explicit CLI
    // request to one core. Removing only the process-local Symbolica license
    // therefore still makes the multicore request fail its license policy.
    let unlicensed = rustred_with_environment(
        &parallel_arguments,
        ONE_LOOP_TWO_EXTERNALS,
        &[("RAYON_NUM_THREADS", "1")],
        true,
    );
    assert_eq!(unlicensed.status.code(), Some(8));
    assert!(unlicensed.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&unlicensed.stderr);
    assert!(stderr.starts_with("rustred: execution:"), "{stderr}");
    assert!(
        stderr.contains(&format!("n_cores {requested} requires a Symbolica license")),
        "{stderr}"
    );
}

#[test]
fn n_cores_rejects_zero_missing_duplicate_and_malformed_values() {
    assert_usage_error(
        &["derive", "--n-cores", "0"],
        "invalid value \"0\" for --n-cores; expected a positive integer",
    );
    assert_usage_error(&["derive", "--n-cores"], "option --n-cores needs a value");
    assert_usage_error(
        &["derive", "--n-cores", "2", "--n-cores", "3"],
        "option --n-cores was supplied twice",
    );
    assert_usage_error(
        &["derive", "--n-cores", "many"],
        "invalid value \"many\" for --n-cores; expected a positive integer",
    );
}

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

#[test]
fn file_output_is_atomic_and_requires_force_to_replace() {
    let directory = unique_test_directory();
    std::fs::create_dir(&directory).expect("create isolated CLI test directory");
    let destination = directory.join("relations.toml");
    std::fs::write(&destination, b"sentinel\n").expect("seed destination");
    let path = destination.to_str().expect("UTF-8 test path");

    let refused = rustred(
        &["derive", "--input-format", "symbolica", "--output", path],
        ONE_LOOP,
    );
    assert_eq!(refused.status.code(), Some(7));
    assert!(refused.stdout.is_empty());
    assert_eq!(std::fs::read(&destination).unwrap(), b"sentinel\n");

    let replaced = rustred(
        &[
            "derive",
            "--input-format",
            "symbolica",
            "--output",
            path,
            "--force",
        ],
        ONE_LOOP,
    );
    assert_eq!(
        replaced.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&replaced.stderr)
    );
    assert!(replaced.stdout.is_empty());
    let installed = std::fs::read_to_string(&destination).expect("installed output");
    let document: toml::Value = toml::from_str(&installed).expect("installed TOML");
    assert_eq!(document["status"].as_str(), Some("ok"));

    std::fs::remove_file(&destination).expect("remove isolated output");
    std::fs::remove_dir(&directory).expect("remove isolated test directory");
}

fn unique_test_directory() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "rustred-cli-derive-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn hybrid_with_parameter_declarations(outer: &str) -> String {
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

fn hybrid_with_outer_only(outer: &str) -> String {
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

fn hybrid_with_outer_and_numerator(outer: &str) -> String {
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
