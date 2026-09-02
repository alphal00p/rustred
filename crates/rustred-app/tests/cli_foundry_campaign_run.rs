use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMPORARY_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new(label: &str) -> Self {
        let sequence = TEMPORARY_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "rustred-foundry-cli-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create temporary foundry CLI directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn rustred(arguments: &[&str], input: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rustred"))
        .args(arguments)
        .env("SYMBOLICA_HIDE_BANNER", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn RustRed foundry campaign CLI");
    child
        .stdin
        .take()
        .expect("foundry campaign stdin")
        .write_all(input.as_bytes())
        .expect("write foundry campaign input");
    child
        .wait_with_output()
        .expect("wait for foundry campaign CLI")
}

#[test]
fn malformed_strict_config_fails_without_report_bytes() {
    let config = r#"schema = "rustred.foundry-campaign-config.toml.v2"
preset = "three-loop-unit-mass-vacuum-k6-orbit-0"
mode = "autonomous"
max_task_reports = 1
max_reported_uncovered_boxes = 1
legacy = true
"#;
    let output = rustred(
        &["campaign", "run", "--config", "-", "--output", "-"],
        config,
    );
    assert_eq!(output.status.code(), Some(4));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.starts_with("rustred: input:"), "{stderr}");
    assert!(stderr.contains("unknown field"), "{stderr}");
}

#[test]
fn autonomous_mode_cannot_be_relabelled_over_external_hints() {
    let config = r#"schema = "rustred.foundry-campaign-config.toml.v2"
preset = "three-loop-unit-mass-vacuum-k6-orbit-0"
mode = "autonomous"
max_task_reports = 1
max_reported_uncovered_boxes = 1

[hints]
itinerary = "single-sector-fixed-point"
interior_margin = 2
polynomial_degree_ceiling = 0
ordering_policy = "rustred.unshifted-sector-order.v1"

[[hints.probes]]
modulus = 1000000007
base_parameters = [37]
chart_offsets = [0, 0, 0, 0, 0, 0]
"#;
    let output = rustred(
        &["campaign", "run", "--config", "-", "--output", "-"],
        config,
    );
    assert_eq!(output.status.code(), Some(4));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.starts_with("rustred: input:"), "{stderr}");
    assert!(
        stderr.contains("autonomous foundry campaigns cannot contain"),
        "{stderr}"
    );
}

#[test]
fn ambiguous_double_stdout_is_a_usage_error_before_input() {
    let output = rustred(
        &[
            "campaign",
            "run",
            "--config",
            "-",
            "--output",
            "-",
            "--measurements-output",
            "-",
        ],
        "",
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.starts_with("rustred: usage:"), "{stderr}");
    assert!(stderr.contains("must not name the same stream"), "{stderr}");
}

#[test]
fn force_cannot_replace_artifact_through_lexically_aliased_report_path() {
    let temporary = TemporaryDirectory::new("lexical-report-alias");
    let nested = temporary.path().join("nested");
    fs::create_dir(&nested).expect("create lexical-alias directory");
    let artifact = temporary.path().join("artifact.rribp");
    let report_alias = nested.join("..").join("artifact.rribp");
    let sentinel = b"existing artifact sentinel";
    fs::write(&artifact, sentinel).expect("write artifact sentinel");

    let output = rustred(
        &[
            "campaign",
            "run-waves",
            "--config",
            "-",
            "--output",
            report_alias.to_str().expect("UTF-8 report alias"),
            "--artifact-output",
            artifact.to_str().expect("UTF-8 artifact path"),
            "--force",
        ],
        "",
    );

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--artifact-output must differ"), "{stderr}");
    assert_eq!(fs::read(&artifact).unwrap(), sentinel);
}

#[cfg(unix)]
#[test]
fn force_cannot_replace_artifact_through_symlinked_measurement_parent() {
    use std::os::unix::fs::symlink;

    let temporary = TemporaryDirectory::new("symlink-measurement-alias");
    let real = temporary.path().join("real");
    let alias = temporary.path().join("alias");
    fs::create_dir(&real).expect("create real output directory");
    symlink(&real, &alias).expect("create output-directory symlink");
    let artifact = real.join("artifact.rribp");
    let measurement_alias = alias.join("artifact.rribp");
    let report = real.join("report.toml");
    let sentinel = b"existing artifact sentinel";
    fs::write(&artifact, sentinel).expect("write artifact sentinel");

    let output = rustred(
        &[
            "campaign",
            "run-waves",
            "--config",
            "-",
            "--output",
            report.to_str().expect("UTF-8 report path"),
            "--measurements-output",
            measurement_alias.to_str().expect("UTF-8 measurement alias"),
            "--artifact-output",
            artifact.to_str().expect("UTF-8 artifact path"),
            "--force",
        ],
        "",
    );

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--artifact-output and --measurements-output must differ"),
        "{stderr}"
    );
    assert_eq!(fs::read(&artifact).unwrap(), sentinel);
}

#[test]
fn general_help_advertises_the_bounded_diagnostic_contract() {
    let output = rustred(&["--help"], "");
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 help");
    assert!(stdout.contains("rustred campaign run [OPTIONS]"));
    assert!(stdout.contains("report is not a closing artifact"));
}

#[test]
fn successful_non_tty_run_is_quiet_and_contains_no_ansi_even_with_color_always() {
    let config = r#"schema = "rustred.foundry-campaign-config.toml.v2"
preset = "three-loop-unit-mass-vacuum-k6-orbit-0"
mode = "autonomous"
max_task_reports = 1
max_reported_uncovered_boxes = 1
"#;
    let output = rustred(
        &[
            "campaign", "run", "--config", "-", "--output", "-", "--color", "always",
        ],
        config,
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{:?}", output.stderr);
    assert!(!output.stderr.windows(2).any(|bytes| bytes == b"\x1b["));
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 foundry report");
    assert!(stdout.contains("schema = \"rustred.foundry-campaign-report.toml.v2\""));
    assert!(stdout.contains("kind = \"operationally_bounded\""));
}

#[test]
fn full_rank_wave_command_reports_incomplete_without_publishing_an_artifact() {
    let config = r#"schema = "rustred.foundry-campaign-config.toml.v2"
preset = "three-loop-unit-mass-vacuum-k6-orbit-0"
mode = "autonomous"
max_task_reports = 1
max_reported_uncovered_boxes = 1
"#;
    let output = rustred(
        &[
            "campaign",
            "run-waves",
            "--config",
            "-",
            "--output",
            "-",
            "--n-cores",
            "1",
            "--color",
            "always",
        ],
        config,
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert!(!output.stderr.windows(2).any(|bytes| bytes == b"\x1b["));
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 wave report");
    assert!(stdout.contains("rustred.foundry-wave-campaign-report.toml.v2"));
    assert!(stdout.contains("outcome = \"incomplete\""));
    assert!(stdout.contains("artifact_installed = false"));
    assert!(stdout.contains("durable_artifact_published = false"));
}
