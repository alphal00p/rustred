use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const INPUT: &str = r#"I(loops(q),externals(),dimension(d),prop(D1,q^2-1,1))"#;

fn run(arguments: &[&str], input: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rustred"))
        .args(arguments)
        .env("SYMBOLICA_HIDE_BANNER", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

fn success(arguments: &[&str], input: &[u8]) -> Vec<u8> {
    let output = run(arguments, input);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

#[test]
fn saved_candidates_are_not_artifacts_and_can_be_certified_in_a_fresh_process() {
    let bundle = success(
        &["family-candidates", "--input-format", "symbolica"],
        INPUT.as_bytes(),
    );
    let report: toml::Value = toml::from_str(std::str::from_utf8(&bundle).unwrap()).unwrap();
    assert_eq!(report["status"].as_str(), Some("uncertified-candidates"));
    assert_eq!(
        report["schema"].as_str(),
        Some("rustred.uncertified-candidates.toml.v1")
    );
    let rejected = run(&["campaign", "inspect", "--artifact", "-"], &bundle);
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
    let artifact = success(&["certify-candidates"], &bundle);
    let reference = success(
        &["family-close", "--input-format", "symbolica"],
        INPUT.as_bytes(),
    );
    assert_eq!(artifact, reference);
    let reduction = success(
        &["campaign", "reduce", "--artifact", "-", "--powers", "3"],
        &artifact,
    );
    let report: toml::Value = toml::from_str(std::str::from_utf8(&reduction).unwrap()).unwrap();
    assert_eq!(report["status"].as_str(), Some("reduced"));
}

#[test]
fn certification_replays_saved_coefficients_and_honors_separate_resource_policy() {
    let bundle = success(&["family-candidates"], INPUT.as_bytes());
    let chosen = success(
        &[
            "certify-candidates",
            "--max-predicate-atoms",
            "64",
            "--max-domain-bound-endpoint-cells",
            "65536",
        ],
        &bundle,
    );
    assert_eq!(chosen, success(&["certify-candidates"], &bundle));
    let failed = run(
        &[
            "certify-candidates",
            "--max-domain-bound-endpoint-cells",
            "0",
        ],
        &bundle,
    );
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    let mut parsed: toml::Value = toml::from_str(std::str::from_utf8(&bundle).unwrap()).unwrap();
    let sectors = parsed["sectors"].as_array_mut().unwrap();
    let rules = sectors[0]["rules"].as_array_mut().unwrap();
    assert!(!rules.is_empty());
    rules[0]["rhs"][0]["coefficient"] = toml::Value::String("1".into());
    let tampered = toml::to_string(&parsed).unwrap();
    let failed = run(&["certify-candidates"], tampered.as_bytes());
    assert!(
        !failed.status.success(),
        "modified coefficient must not retain source replay authority"
    );
    assert!(failed.stdout.is_empty());
}

#[test]
fn candidate_command_help_and_policy_errors_are_explicit() {
    let help = success(&["--help"], b"");
    let help = std::str::from_utf8(&help).unwrap();
    assert!(help.contains("family-candidates"));
    assert!(help.contains("certify-candidates"));
    for command in [
        vec!["family-candidates", "--max-predicate-atoms", "64"],
        vec!["certify-candidates", "--n-cores", "2"],
        vec!["certify-candidates", "--max-predicate-atoms", "257"],
    ] {
        let failed = run(&command, b"not read");
        assert!(!failed.status.success());
        assert!(failed.stdout.is_empty());
    }
}

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rustred-candidate-cli-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn separate_reports_expose_phase_timings_and_never_replace_data_outputs() {
    let directory = Directory::new();
    let generation_report = directory.0.join("generation.toml");
    let artifact_path = directory.0.join("family.rribp");
    let bundle = success(
        &[
            "family-candidates",
            "--report-output",
            generation_report.to_str().unwrap(),
        ],
        INPUT.as_bytes(),
    );
    let report: toml::Value =
        toml::from_str(&std::fs::read_to_string(&generation_report).unwrap()).unwrap();
    assert_eq!(report["status"].as_str(), Some("uncertified-candidates"));
    assert!(report["solve_us"].as_integer().is_some());
    let certified = success(
        &[
            "certify-candidates",
            "--output",
            artifact_path.to_str().unwrap(),
            "--report-output",
            "-",
        ],
        &bundle,
    );
    let report: toml::Value = toml::from_str(std::str::from_utf8(&certified).unwrap()).unwrap();
    assert_eq!(report["status"].as_str(), Some("generated-durable"));
    assert!(report["certification_us"].as_integer().is_some());
    let bytes = std::fs::read(&artifact_path).unwrap();
    assert!(!bytes.is_empty());
    let failed = run(
        &[
            "certify-candidates",
            "--output",
            artifact_path.to_str().unwrap(),
            "--report-output",
            artifact_path.to_str().unwrap(),
            "--force",
        ],
        &bundle,
    );
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    assert_eq!(std::fs::read(&artifact_path).unwrap(), bytes);
    // Existing report destinations also reject before generation writes its
    // otherwise-valid data stdout; both outputs use the established preflight.
    let failed = run(
        &[
            "family-candidates",
            "--report-output",
            generation_report.to_str().unwrap(),
        ],
        INPUT.as_bytes(),
    );
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
}
