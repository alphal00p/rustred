use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use rustred::persistence::{
    BinaryIoLimits, BinaryProgramKind, BinarySection, CoefficientId, CoefficientTableBuilder,
    DecodedCoefficientTable, SectionTag, encode_program, inspect_program,
};

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
fn explicit_reconstruction_backend_preserves_small_case_candidate_payload() {
    let sparse = success(&["family-candidates"], INPUT.as_bytes());
    let reconstructed = success(
        &["family-candidates", "--exact-backend", "semi-numerical"],
        INPUT.as_bytes(),
    );
    assert_eq!(sparse, reconstructed);
    let failed = run(
        &["family-candidates", "--exact-backend", "invalid"],
        INPUT.as_bytes(),
    );
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
}

#[test]
fn candidate_progress_preserves_data_and_never_claims_certification() {
    let quiet = success(&["family-candidates"], INPUT.as_bytes());
    let observed = run(&["family-candidates", "--progress"], INPUT.as_bytes());
    assert!(
        observed.status.success(),
        "{}",
        String::from_utf8_lossy(&observed.stderr)
    );
    assert_eq!(quiet, observed.stdout);
    let progress = String::from_utf8(observed.stderr).unwrap();
    assert!(progress.contains("preparing K=1"));
    assert!(progress.contains("generated"));
    assert!(progress.contains("output written"));
    for forbidden in [
        "\x1b",
        "replay",
        "checking",
        "install",
        "closed",
        "certified",
        "artifact written",
    ] {
        assert!(!progress.contains(forbidden), "{forbidden}: {progress}");
    }
    for args in [
        vec!["family-candidates", "--progress", "--progress"],
        vec!["certify-candidates", "--progress"],
    ] {
        let failed = run(&args, b"not parsed");
        assert!(!failed.status.success());
        assert!(failed.stdout.is_empty());
    }
}

#[test]
fn saved_candidates_are_not_artifacts_and_can_be_certified_in_a_fresh_process() {
    let bundle = success(
        &["family-candidates", "--input-format", "symbolica"],
        INPUT.as_bytes(),
    );
    let envelope = inspect_program(&bundle, BinaryIoLimits::default()).unwrap();
    assert_eq!(envelope.kind(), BinaryProgramKind::Candidates);
    assert!(envelope.section(SectionTag::SYMBOLICA_STATE).is_some());
    assert!(envelope.section(SectionTag::COEFFICIENTS).is_some());
    let inspection = rustred_app::inspect_generated_candidate_bundle(
        &bundle,
        rustred_app::CandidateBundleLimits::default(),
    )
    .unwrap();
    assert_eq!(inspection.schema, rustred_app::CANDIDATE_BUNDLE_SCHEMA);
    assert_eq!(inspection.status, "uncertified-candidates");
    assert_eq!(inspection.arity, 1);
    assert_eq!(inspection.generated_rules, 1);
    assert_eq!(inspection.finite_residuals, 1);
    assert!(inspection.unique_coefficients > 0);
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
    // Re-encode valid native data with a false rational RHS. This exercises
    // mathematical replay, not only corrupt-byte or wrong-envelope rejection.
    let limits = BinaryIoLimits::default();
    let envelope = inspect_program(&bundle, limits).unwrap();
    let table = DecodedCoefficientTable::import_generated(
        envelope.section(SectionTag::SYMBOLICA_STATE).unwrap(),
        envelope.section(SectionTag::COEFFICIENTS).unwrap(),
        limits,
    )
    .unwrap();
    let mut changed = 0;
    let mut builder = CoefficientTableBuilder::new(limits);
    for index in 0..table.len() {
        let id = CoefficientId::try_from_index(index).unwrap();
        let coefficient = table.coefficient(id).unwrap();
        let replacement = if !coefficient.denominator.is_constant() {
            changed += 1;
            coefficient + coefficient
        } else {
            coefficient.clone()
        };
        // This fixture has one rational RHS; all polynomial guards and any
        // structural coefficient references retain their original IDs.
        assert_eq!(builder.intern(&replacement).unwrap(), id);
    }
    assert_eq!(changed, 1, "expected one rational one-loop RHS");
    let replacement = builder.finish().unwrap();
    let sections: Vec<_> = envelope
        .sections()
        .iter()
        .map(|section| BinarySection {
            tag: section.tag,
            bytes: if section.tag == SectionTag::SYMBOLICA_STATE {
                &replacement.state
            } else if section.tag == SectionTag::COEFFICIENTS {
                &replacement.atoms
            } else {
                section.bytes
            },
        })
        .collect();
    let tampered = encode_program(envelope.kind(), &sections, limits).unwrap();
    let failed = run(&["certify-candidates"], &tampered);
    assert!(
        !failed.status.success(),
        "modified coefficient must not retain source replay authority"
    );
    assert!(failed.stdout.is_empty());
}

#[test]
fn rank_scoped_certification_is_explicitly_fail_closed_without_an_unbounded_fallback() {
    let bundle = success(&["family-candidates"], INPUT.as_bytes());
    let failed = run(
        &["certify-candidates", "--max-negative-index-degree", "30"],
        &bundle,
    );
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&failed.stderr);
    assert!(stderr.contains("rank-scoped certification"), "{stderr}");
    assert!(
        stderr.contains("refusing an unbounded certification fallback"),
        "{stderr}"
    );

    let failed = run(
        &["certify-candidates", "--max-negative-index-degree", "31"],
        &bundle,
    );
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&failed.stderr).contains("exceeds the supported rank-scoped limit")
    );
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
