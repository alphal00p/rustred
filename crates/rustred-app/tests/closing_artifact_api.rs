use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use rustred_app::{
    AppErrorKind, ClosingArtifactGenerateRequest, ClosingArtifactInspectRequest,
    ClosingArtifactReduceRequest, ClosingFamilySelector, MAX_CLOSING_RULE_APPLICATIONS,
    closing_artifact_generate, closing_artifact_inspect, closing_artifact_reduce,
};

const SELECTOR: &str = "unit-mass-vacuum-k1";
const SUNSET_SELECTOR: &str = "unit-mass-vacuum-k3";

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rustred-closing-artifact-test-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("create test directory");
        Self(path)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn rustred(arguments: &[&str], input: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rustred"))
        .args(arguments)
        .env("SYMBOLICA_HIDE_BANNER", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn RustRed CLI");
    child
        .stdin
        .take()
        .expect("CLI stdin")
        .write_all(input)
        .expect("write CLI input");
    child.wait_with_output().expect("wait for RustRed CLI")
}

fn successful_cli(arguments: &[&str], input: &[u8]) -> Vec<u8> {
    let output = rustred(arguments, input);
    assert!(
        output.status.success(),
        "RustRed CLI failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    output.stdout
}

fn generate_artifact() -> rustred_app::ClosingArtifactGenerateResult {
    closing_artifact_generate(ClosingArtifactGenerateRequest {
        family: ClosingFamilySelector::UnitMassVacuumK1,
    })
    .expect("generate durable one-loop closure")
}

fn with_durable_schema(artifact: &[u8], schema: u32) -> Vec<u8> {
    let mut crafted = artifact.to_vec();
    crafted[8..12].copy_from_slice(&schema.to_le_bytes());
    crafted
}

fn with_durable_arity(artifact: &[u8], arity: u64) -> Vec<u8> {
    let mut crafted = artifact.to_vec();
    let metadata_section_offset = 16;
    assert_eq!(
        u16::from_le_bytes(
            crafted[metadata_section_offset..metadata_section_offset + 2]
                .try_into()
                .unwrap()
        ),
        1
    );
    let metadata_payload_offset = metadata_section_offset + 2 + 8;
    let algorithm_bytes = usize::try_from(u64::from_le_bytes(
        crafted[metadata_payload_offset..metadata_payload_offset + 8]
            .try_into()
            .unwrap(),
    ))
    .unwrap();
    let arity_offset = metadata_payload_offset + 8 + algorithm_bytes;
    crafted[arity_offset..arity_offset + 8].copy_from_slice(&arity.to_le_bytes());
    crafted
}

#[test]
fn family_selectors_are_semantic_and_typed() {
    assert_eq!(
        SELECTOR.parse(),
        Ok(ClosingFamilySelector::UnitMassVacuumK1)
    );
    assert_eq!(
        SUNSET_SELECTOR.parse(),
        Ok(ClosingFamilySelector::UnitMassVacuumK3)
    );
    let family_error = "I1L".parse::<ClosingFamilySelector>().unwrap_err();
    assert_eq!(family_error.value(), "I1L");
    assert!(family_error.to_string().contains(SELECTOR));
}

#[test]
fn two_loop_generation_and_application_surfaces_publish_the_closed_sunset() {
    let generated = closing_artifact_generate(ClosingArtifactGenerateRequest {
        family: ClosingFamilySelector::UnitMassVacuumK3,
    })
    .expect("generate durable sunset closure");
    assert!(
        generated
            .to_toml()
            .contains("family_selector = \"unit-mass-vacuum-k3\"")
    );
    assert!(generated.to_toml().contains("source_rows = 4"));
    assert!(generated.to_toml().contains("guarded_rules = 5"));
    assert!(generated.to_toml().contains("arity = 3"));
    let generation_document: toml::Value =
        toml::from_str(generated.to_toml()).expect("sunset generation TOML");
    let rendered_rule_sizes = generation_document["rules"]
        .as_array()
        .expect("rendered sunset rules")
        .iter()
        .map(|rule| {
            rule["right_hand_side"]
                .as_array()
                .expect("rendered retained RHS")
                .len()
        })
        .collect::<Vec<_>>();
    assert_eq!(rendered_rule_sizes, [5, 4, 3, 2, 1]);

    let reduction = closing_artifact_reduce(ClosingArtifactReduceRequest {
        artifact: generated.artifact().to_vec(),
        target_powers: vec![2, 2, 1],
        max_rule_applications: MAX_CLOSING_RULE_APPLICATIONS,
    })
    .expect("reduce the two-loop dotted sunset");
    assert_eq!(reduction.target_powers(), [2, 2, 1]);
    assert_eq!(reduction.terms().len(), 2);
    assert!(reduction.terms().iter().any(|term| {
        term.master_powers() == [1, 1, 1] && term.common_mass_squared_power() == -2
    }));
    assert!(reduction.terms().iter().any(|term| {
        term.master_powers() == [0, 1, 1] && term.common_mass_squared_power() == -3
    }));

    assert_eq!(
        successful_cli(&["campaign", "generate", "--family", SUNSET_SELECTOR], b""),
        generated.artifact()
    );
}

#[test]
fn generation_is_deterministic_and_owns_durable_bytes() {
    let generated = generate_artifact();
    let generated_again = generate_artifact();
    assert_eq!(generated.artifact(), generated_again.artifact());
    assert_eq!(generated.to_toml(), generated_again.to_toml());
    assert!(!generated.artifact().is_empty());
    assert_eq!(
        generated.schema(),
        "rustred.closing-artifact-generate-output.toml.v1"
    );
    assert_eq!(generated.status(), "generated-durable");
    assert!(generated.to_toml().ends_with('\n'));
    assert!(generated.to_toml().contains("durable = true"));
    assert!(
        generated
            .to_toml()
            .contains("schema = \"rustred.closing-artifact.v3\"")
    );
    assert!(generated.to_toml().contains("schema_version = 3"));
    assert!(generated.to_toml().contains("source_rows = 1"));
    assert!(generated.to_toml().contains("guarded_rules = 1"));
    assert!(generated.to_toml().contains("domain_lower = [1]"));
    assert!(generated.to_toml().contains("pivot = [1]"));
    assert!(
        generated
            .to_toml()
            .contains(&format!("bytes = {}", generated.artifact().len()))
    );
}

#[test]
fn inspection_authenticates_supplied_durable_bytes() {
    let generated = generate_artifact();
    let inspected = closing_artifact_inspect(ClosingArtifactInspectRequest {
        artifact: generated.artifact().to_vec(),
    })
    .expect("inspect durable one-loop closure");
    assert_eq!(inspected.status(), "inspected");
    assert!(
        inspected
            .to_toml()
            .contains("decoded-authenticated-durable-bytes")
    );
    assert!(inspected.to_toml().contains("master_terminals = 1"));
    assert!(inspected.to_toml().contains("zero_sector_terminals = 1"));
    assert!(
        inspected
            .to_toml()
            .contains("schema = \"rustred.closing-artifact.v3\"")
    );
    assert!(inspected.to_toml().contains("schema_version = 3"));

    let invalid = closing_artifact_inspect(ClosingArtifactInspectRequest {
        artifact: b"not a RustRed artifact".to_vec(),
    })
    .unwrap_err();
    assert_eq!(invalid.kind(), AppErrorKind::Input);
    assert!(invalid.message().contains("invalid closing artifact"));
}

#[test]
fn durable_schema_and_load_resource_failures_keep_typed_categories() {
    let artifact = generate_artifact().into_artifact();
    let unsupported_schema = closing_artifact_inspect(ClosingArtifactInspectRequest {
        artifact: with_durable_schema(&artifact, 2),
    })
    .unwrap_err();
    assert_eq!(unsupported_schema.kind(), AppErrorKind::Schema);
    assert!(unsupported_schema.message().contains("schema version 2"));
    let obsolete_schema = closing_artifact_inspect(ClosingArtifactInspectRequest {
        artifact: with_durable_schema(&artifact, 1),
    })
    .unwrap_err();
    assert_eq!(obsolete_schema.kind(), AppErrorKind::Schema);
    assert!(obsolete_schema.message().contains("schema version 1"));

    let excessive_arity = closing_artifact_inspect(ClosingArtifactInspectRequest {
        artifact: with_durable_arity(&artifact, u64::MAX),
    })
    .unwrap_err();
    assert_eq!(excessive_arity.kind(), AppErrorKind::Limit);
    assert!(excessive_arity.message().contains("artifact arity"));
}

#[test]
fn reduction_exposes_exact_typed_masters_and_common_mass_homogeneity() {
    let artifact = generate_artifact().into_artifact();
    let result = closing_artifact_reduce(ClosingArtifactReduceRequest {
        artifact: artifact.clone(),
        target_powers: vec![3],
        max_rule_applications: MAX_CLOSING_RULE_APPLICATIONS,
    })
    .expect("reduce I(3)");
    assert_eq!(result.status(), "reduced");
    assert_eq!(result.target_powers(), [3]);
    assert_eq!(result.terms().len(), 1);
    let term = &result.terms()[0];
    assert_eq!(term.master_powers(), [1]);
    assert_eq!(term.common_mass_squared_power(), -2);
    assert_eq!(
        term.unit_mass_coefficient(),
        "(-6*rustred::{}::d+8+rustred::{}::d^2)*1/8"
    );
    let document: toml::Value = toml::from_str(result.to_toml()).expect("reduction TOML");
    assert_eq!(
        document["terms"][0]["master"]["powers"][0].as_integer(),
        Some(1)
    );
    assert!(
        result
            .to_toml()
            .contains("common_mass_squared_power = \"-2\"")
    );
    assert!(
        result
            .to_toml()
            .contains("common_mass_squared_factor = \"mass_squared^(-2)\"")
    );

    let zero = closing_artifact_reduce(ClosingArtifactReduceRequest {
        artifact,
        target_powers: vec![0],
        max_rule_applications: MAX_CLOSING_RULE_APPLICATIONS,
    })
    .expect("reduce zero terminal");
    assert!(zero.terms().is_empty());
}

#[test]
fn application_rejects_wrong_arity_and_enforces_resource_limits() {
    let artifact = generate_artifact().into_artifact();
    let wrong_arity = closing_artifact_reduce(ClosingArtifactReduceRequest {
        artifact: artifact.clone(),
        target_powers: vec![2, 1],
        max_rule_applications: MAX_CLOSING_RULE_APPLICATIONS,
    })
    .unwrap_err();
    assert_eq!(wrong_arity.kind(), AppErrorKind::Input);
    assert!(wrong_arity.message().contains("arity 1"));

    let excessive_limit = closing_artifact_reduce(ClosingArtifactReduceRequest {
        artifact: artifact.clone(),
        target_powers: vec![1],
        max_rule_applications: MAX_CLOSING_RULE_APPLICATIONS + 1,
    })
    .unwrap_err();
    assert_eq!(excessive_limit.kind(), AppErrorKind::Limit);

    let exhausted = closing_artifact_reduce(ClosingArtifactReduceRequest {
        artifact,
        target_powers: vec![3],
        max_rule_applications: 1,
    })
    .unwrap_err();
    assert_eq!(exhausted.kind(), AppErrorKind::Limit);
    assert!(exhausted.message().contains("configured limit 1"));
}

#[test]
fn campaign_cli_bytes_match_the_application_surfaces() {
    let generated = generate_artifact();
    assert_eq!(
        successful_cli(&["campaign", "generate", "--family", SELECTOR], b""),
        generated.artifact()
    );

    let inspected = closing_artifact_inspect(ClosingArtifactInspectRequest {
        artifact: generated.artifact().to_vec(),
    })
    .unwrap();
    assert_eq!(
        successful_cli(
            &["campaign", "inspect", "--artifact", "-"],
            generated.artifact(),
        ),
        inspected.to_toml().as_bytes()
    );

    let reduced = closing_artifact_reduce(ClosingArtifactReduceRequest {
        artifact: generated.artifact().to_vec(),
        target_powers: vec![3],
        max_rule_applications: MAX_CLOSING_RULE_APPLICATIONS,
    })
    .unwrap();
    assert_eq!(
        successful_cli(
            &["campaign", "reduce", "--artifact", "-", "--powers", "3",],
            generated.artifact(),
        ),
        reduced.to_toml().as_bytes()
    );
}

#[test]
fn campaign_cli_round_trips_artifact_and_toml_files_atomically() {
    let directory = TestDirectory::new();
    let artifact_path = directory.join("one-loop.rr");
    let inspection_path = directory.join("inspection.toml");
    let reduction_path = directory.join("reduction.toml");
    let artifact_arg = artifact_path.to_str().expect("UTF-8 artifact path");
    let inspection_arg = inspection_path.to_str().expect("UTF-8 inspection path");
    let reduction_arg = reduction_path.to_str().expect("UTF-8 reduction path");

    let generated = generate_artifact();
    let generate_output = rustred(
        &[
            "campaign",
            "generate",
            "--family",
            SELECTOR,
            "--output",
            artifact_arg,
        ],
        b"",
    );
    assert!(generate_output.status.success());
    assert!(generate_output.stdout.is_empty());
    assert_eq!(
        std::fs::read(&artifact_path).expect("read durable artifact"),
        generated.artifact()
    );

    let inspect_output = rustred(
        &[
            "campaign",
            "inspect",
            "--artifact",
            artifact_arg,
            "--output",
            inspection_arg,
        ],
        b"",
    );
    assert!(inspect_output.status.success());
    assert!(inspect_output.stdout.is_empty());
    let expected_inspection = closing_artifact_inspect(ClosingArtifactInspectRequest {
        artifact: generated.artifact().to_vec(),
    })
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(&inspection_path).expect("read inspection TOML"),
        expected_inspection.to_toml()
    );

    let reduce_output = rustred(
        &[
            "campaign",
            "reduce",
            "--artifact",
            artifact_arg,
            "--powers",
            "3",
            "--output",
            reduction_arg,
        ],
        b"",
    );
    assert!(reduce_output.status.success());
    assert!(reduce_output.stdout.is_empty());
    assert!(
        std::fs::read_to_string(&reduction_path)
            .expect("read reduction TOML")
            .contains("common_mass_squared_power = \"-2\"")
    );
}

#[test]
fn invalid_artifact_file_never_creates_partial_output() {
    let directory = TestDirectory::new();
    let artifact_path = directory.join("invalid.rr");
    let output_path = directory.join("must-not-exist.toml");
    std::fs::write(&artifact_path, b"invalid artifact").expect("write invalid fixture");
    let output = rustred(
        &[
            "campaign",
            "inspect",
            "--artifact",
            artifact_path.to_str().expect("UTF-8 artifact path"),
            "--output",
            output_path.to_str().expect("UTF-8 output path"),
        ],
        b"",
    );
    assert_eq!(output.status.code(), Some(4));
    assert!(output.stdout.is_empty());
    assert!(!output_path.exists());
}

#[test]
fn campaign_cli_reports_input_and_resource_failures_without_partial_output() {
    let invalid_selector = rustred(&["campaign", "generate", "--family", "I1L"], b"");
    assert_eq!(invalid_selector.status.code(), Some(2));
    assert!(invalid_selector.stdout.is_empty());
    assert!(String::from_utf8_lossy(&invalid_selector.stderr).contains(SELECTOR));

    let invalid_artifact = rustred(
        &["campaign", "inspect", "--artifact", "-"],
        b"invalid artifact",
    );
    assert_eq!(invalid_artifact.status.code(), Some(4));
    assert!(invalid_artifact.stdout.is_empty());
    assert!(String::from_utf8_lossy(&invalid_artifact.stderr).contains("invalid closing artifact"));

    let artifact = generate_artifact().into_artifact();
    let exhausted = rustred(
        &[
            "campaign",
            "reduce",
            "--artifact",
            "-",
            "--powers",
            "3",
            "--max-rule-applications",
            "1",
        ],
        &artifact,
    );
    assert_eq!(exhausted.status.code(), Some(4));
    assert!(exhausted.stdout.is_empty());
    assert!(String::from_utf8_lossy(&exhausted.stderr).contains("configured limit 1"));
}
