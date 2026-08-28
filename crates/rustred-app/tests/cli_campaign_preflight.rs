use std::io::Write;
use std::process::{Command, Output, Stdio};

fn profile(
    enclosing: &str,
    hydrated: &str,
    retained_visible: &str,
    transient_visible: &str,
) -> String {
    format!(
        r#"schema = "rustred.campaign-execution-resource-profile.v1"
estimator_revision = 19
enclosing_memory_limit = "{enclosing}"

[fixed_memory]
process_runtime_and_shared_catalogs = "20B"
coordinator_stack_tls_workspace = "10B"
per_worker_stack_tls_workspace = "10B"
explicitly_admitted_inner_threads = "5B"
hydrated_retained_lanes = "{hydrated}"
staged_results = "10B"
checkpoint_and_output_buffers = "20B"
safety_reserve = "35B"

[minimum_runnable_task.retained_output]
visible_logical = "{retained_visible}"
opaque_native_reserve = "0B"

[minimum_runnable_task.transient_excluding_output]
visible_logical = "{transient_visible}"
opaque_native_reserve = "0B"
"#
    )
}

fn zero_profile(enclosing: &str) -> String {
    r#"schema = "rustred.campaign-execution-resource-profile.v1"
estimator_revision = 1
enclosing_memory_limit = "ENCLOSING"

[fixed_memory]
process_runtime_and_shared_catalogs = "0B"
coordinator_stack_tls_workspace = "0B"
per_worker_stack_tls_workspace = "0B"
explicitly_admitted_inner_threads = "0B"
hydrated_retained_lanes = "0B"
staged_results = "0B"
checkpoint_and_output_buffers = "0B"
safety_reserve = "0B"

[minimum_runnable_task.retained_output]
visible_logical = "0B"
opaque_native_reserve = "0B"

[minimum_runnable_task.transient_excluding_output]
visible_logical = "0B"
opaque_native_reserve = "0B"
"#
    .replace("ENCLOSING", enclosing)
}

fn rustred(arguments: &[&str], input: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rustred"))
        .args(arguments)
        .env_remove("SYMBOLICA_LICENSE")
        .env("SYMBOLICA_HIDE_BANNER", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn RustRed campaign preflight CLI");
    child
        .stdin
        .take()
        .expect("preflight stdin")
        .write_all(input.as_bytes())
        .expect("write preflight profile");
    child.wait_with_output().expect("wait for preflight CLI")
}

fn successful_toml(arguments: &[&str], input: &str) -> (Output, toml::Value) {
    let output = rustred(arguments, input);
    assert_eq!(
        output.status.code(),
        Some(0),
        "RustRed campaign preflight failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let document = toml::from_str(std::str::from_utf8(&output.stdout).expect("UTF-8 output"))
        .expect("valid campaign-preflight TOML");
    (output, document)
}

#[test]
fn preflight_is_deterministic_license_free_and_memory_limits_width() {
    let input = profile("1024B", "0B", "60B", "40B");
    let arguments = [
        "campaign",
        "preflight",
        "--profile",
        "-",
        "--n-cores",
        "100",
        "--max-memory",
        "900B",
    ];
    let (first, document) = successful_toml(&arguments, &input);
    let (second, _) = successful_toml(&arguments, &input);
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(
        document["schema"].as_str(),
        Some("rustred.campaign-execution-preflight-output.toml.v1")
    );
    assert_eq!(document["status"].as_str(), Some("ready"));
    assert_eq!(
        document["profile_schema"].as_str(),
        Some("rustred.campaign-execution-resource-profile.v1")
    );
    assert_eq!(
        document["unsigned_integer_encoding"].as_str(),
        Some("unsigned-decimal-string")
    );
    assert_eq!(document["requested_core_ceiling"].as_str(), Some("100"));
    assert_eq!(document["ready"]["effective_width"].as_str(), Some("70"));
    assert_eq!(
        document["ready"]["worker_thread_count"].as_str(),
        Some("70")
    );
    assert_eq!(
        document["ready"]["selected_fixed_memory_bytes"].as_str(),
        Some("800")
    );
    assert_eq!(
        document["ready"]["minimum_required_memory_bytes"].as_str(),
        Some("900")
    );
    assert!(document.get("pause").is_none());
}

#[test]
fn exact_inline_fit_is_ready_and_one_byte_below_is_a_successful_typed_pause() {
    let input = profile("1024B", "0B", "60B", "40B");
    let common = [
        "campaign",
        "preflight",
        "--profile",
        "-",
        "--n-cores",
        "100",
    ];
    let mut exact_arguments = common.to_vec();
    exact_arguments.extend(["--max-memory", "200B"]);
    let (_, exact) = successful_toml(&exact_arguments, &input);
    assert_eq!(exact["status"].as_str(), Some("ready"));
    assert_eq!(exact["ready"]["effective_width"].as_str(), Some("1"));
    assert_eq!(exact["ready"]["worker_thread_count"].as_str(), Some("0"));

    let mut below_arguments = common.to_vec();
    below_arguments.extend(["--max-memory", "199B"]);
    let (_, below) = successful_toml(&below_arguments, &input);
    assert_eq!(below["status"].as_str(), Some("paused_for_memory_capacity"));
    assert_eq!(below["pause"]["kind"].as_str(), Some("memory_capacity"));
    assert_eq!(
        below["pause"]["inline_fixed_memory_bytes"].as_str(),
        Some("100")
    );
    assert_eq!(
        below["pause"]["inline_minimum_required_memory_bytes"].as_str(),
        Some("200")
    );
    assert_eq!(below["pause"]["memory_shortfall_bytes"].as_str(), Some("1"));
    assert!(below.get("ready").is_none());
}

#[test]
fn unsigned_byte_values_above_toml_i64_are_losslessly_reported() {
    let input = zero_profile("18446744073709551615B");
    let (_, document) = successful_toml(
        &[
            "campaign",
            "preflight",
            "--profile",
            "-",
            "--max-memory",
            "9223372036854775808B",
        ],
        &input,
    );
    assert_eq!(document["status"].as_str(), Some("ready"));
    assert_eq!(
        document["enclosing_memory_limit_bytes"].as_str(),
        Some("18446744073709551615")
    );
    assert_eq!(
        document["operational_memory_limit_bytes"].as_str(),
        Some("9223372036854775808")
    );
}

#[test]
fn malformed_or_unbootstrappable_profiles_fail_before_output() {
    let unknown = format!(
        "{}\nunknown_top_level = \"rejected\"\n",
        profile("1024B", "0B", "60B", "40B")
    );
    let bad_unit = profile("1GB", "0B", "60B", "40B");
    let hydrated = profile("1024B", "1B", "60B", "40B");
    for (input, detail) in [
        (unknown.as_str(), "unknown field"),
        (bad_unit.as_str(), "enclosing_memory_limit"),
        (hydrated.as_str(), "hydrated"),
    ] {
        let output = rustred(
            &[
                "campaign",
                "preflight",
                "--profile",
                "-",
                "--max-memory",
                "900B",
            ],
            input,
        );
        assert_eq!(output.status.code(), Some(4));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.starts_with("rustred: input:"), "{stderr}");
        assert!(stderr.contains(detail), "missing {detail:?}: {stderr}");
    }
}
