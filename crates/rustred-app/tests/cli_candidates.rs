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
    let write_result = child.stdin.take().unwrap().write_all(input);
    let output = child.wait_with_output().unwrap();
    if let Err(error) = write_result {
        // Invalid arguments can be rejected before the child reads stdin.
        assert!(
            error.kind() == std::io::ErrorKind::BrokenPipe && !output.status.success(),
            "failed to send CLI test input: {error}; child status: {}",
            output.status
        );
    }
    output
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
fn numerical_depth_is_explicit_and_validated_before_generation() {
    let baseline = success(&["family-candidates"], INPUT.as_bytes());
    assert_eq!(
        baseline,
        success(
            &["family-candidates", "--numerical-depth", "2"],
            INPUT.as_bytes()
        )
    );
    for depth in ["0", "1"] {
        let generated = success(
            &["family-candidates", "--numerical-depth", depth],
            INPUT.as_bytes(),
        );
        let inspection =
            rustred_app::inspect_generated_candidate_bundle(&generated, Default::default())
                .unwrap();
        assert_eq!(inspection.numerical_depth, depth.parse::<u32>().unwrap());
        let artifact = success(&["certify-candidates"], &generated);
        success(
            &["campaign", "reduce", "--artifact", "-", "--powers", "3"],
            &artifact,
        );
    }
    for args in [
        vec!["family-candidates", "--numerical-depth", "-1"],
        vec!["family-candidates", "--numerical-depth", "4294967296"],
        vec!["family-candidates", "--numerical-depth", "true"],
        vec![
            "family-candidates",
            "--numerical-depth",
            "0",
            "--numerical-depth",
            "0",
        ],
        vec!["certify-candidates", "--numerical-depth", "0"],
    ] {
        let rejected = run(&args, b"not parsed");
        assert!(!rejected.status.success());
        assert!(rejected.stdout.is_empty());
        assert!(
            String::from_utf8(rejected.stderr)
                .unwrap()
                .contains("--numerical-depth")
        );
    }
}

#[test]
fn numerator_rank_survives_fresh_cli_generation_and_rejects_certification() {
    use rustred::family::IntegralKey;
    use rustred::solver::CandidateReductionError;
    let generated = success(
        &["family-candidates", "--max-numerator-rank", "0"],
        INPUT.as_bytes(),
    );
    let inspected =
        rustred_app::inspect_generated_candidate_bundle(&generated, Default::default()).unwrap();
    assert_eq!(inspected.max_numerator_rank, Some(0));
    let (_, mut reducer) = rustred_app::load_generated_candidate_bundle::<1>(
        &generated,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(reducer.max_numerator_rank(), Some(0));
    reducer
        .reduce_unit_mass(&IntegralKey::try_new([4]).unwrap())
        .unwrap();
    let before = reducer.statistics();
    assert!(matches!(
        reducer.reduce_unit_mass(&IntegralKey::try_new([-1]).unwrap()),
        Err(CandidateReductionError::OutsideNumeratorRank { .. })
    ));
    assert_eq!(reducer.statistics(), before);
    for arguments in [
        vec!["certify-candidates"],
        vec!["certify-candidates", "--max-total-excess-degree", "0"],
    ] {
        let rejected = run(&arguments, &generated);
        assert!(!rejected.status.success());
        assert!(rejected.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&rejected.stderr)
                .contains("rank-scoped candidates cannot be certified")
        );
    }
    for value in ["-1", "+1", "true", "4294967296"] {
        let rejected = run(
            &["family-candidates", "--max-numerator-rank", value],
            b"not parsed",
        );
        assert!(!rejected.status.success());
        assert!(rejected.stdout.is_empty());
        assert!(String::from_utf8_lossy(&rejected.stderr).contains("--max-numerator-rank"));
    }
}

#[test]
fn explicit_exact_backends_preserve_small_case_candidate_payload() {
    let sparse = success(&["family-candidates"], INPUT.as_bytes());
    for backend in [
        "semi-numerical",
        "sparse-factorized",
        "sparse-target-factorized",
    ] {
        let generated = success(
            &["family-candidates", "--exact-backend", backend],
            INPUT.as_bytes(),
        );
        assert_eq!(sparse, generated);
    }
    let help = success(&["--help"], b"");
    assert!(String::from_utf8(help).unwrap().contains(
        "sparse, sparse-factorized, sparse-target-factorized, or semi-numerical [default: sparse]"
    ));
    let failed = run(
        &["family-candidates", "--exact-backend", "invalid"],
        INPUT.as_bytes(),
    );
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    assert!(
        String::from_utf8(failed.stderr)
            .unwrap()
            .contains("sparse-factorized")
    );
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
    // The independent presenter coalesces intermediate events, not aggregate
    // counters. Short solves need not emit a Preparing/Generated snapshot.
    assert!(progress.contains("sector generation [####################] 1/1 (not closure)"));
    assert!(progress.contains("completed-sector totals: rules=1"));
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
    assert!(
        rustred::persistence::equivalent_generated_programs(
            &artifact,
            &reference,
            Default::default(),
        )
        .unwrap()
    );
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
fn total_excess_scope_matches_rust_and_survives_fresh_cli_loading() {
    const SUNSET: &str = "I(loops(p,q),externals(),dimension(d),prop(A,p^2-1,1),prop(B,q^2-1,1),prop(C,(p-q)^2-1,1))";
    for (source, admitted, outside) in [
        (INPUT, vec!["3"], "4"),
        (SUNSET, vec!["3,1,1", "-1,2,1"], "4,1,1"),
    ] {
        let directory = Directory::new();
        let report_path = directory.0.join("bounded.toml");
        let bundle = success(&["family-candidates"], source.as_bytes());
        let bounded = success(
            &[
                "certify-candidates",
                "--max-total-excess-degree",
                "2",
                "--report-output",
                report_path.to_str().unwrap(),
            ],
            &bundle,
        );
        let rust = rustred_app::certify_candidates(
            rustred_app::CandidateCertificationRequest::new(bundle.as_slice())
                .with_max_total_excess_degree(2),
        )
        .unwrap();
        assert!(
            rustred::persistence::equivalent_generated_programs(
                &bounded,
                rust.artifact(),
                Default::default(),
            )
            .unwrap()
        );
        assert_eq!(
            inspect_program(&bounded, Default::default())
                .unwrap()
                .kind(),
            BinaryProgramKind::BoundedCertified
        );
        let report: toml::Value =
            toml::from_str(&std::fs::read_to_string(&report_path).unwrap()).unwrap();
        assert_eq!(report["max_total_excess_degree"].as_integer(), Some(2));
        let inspected = success(&["campaign", "inspect", "--artifact", "-"], &bounded);
        let inspected: toml::Value =
            toml::from_str(std::str::from_utf8(&inspected).unwrap()).unwrap();
        let scope = &inspected["artifact"]["total_excess_scope"];
        assert_eq!(scope["max_entry_total_excess_degree"].as_integer(), Some(2));
        assert_eq!(
            scope["successor_sector_count"],
            report["successor_sector_count"]
        );
        assert_eq!(
            scope["max_successor_total_excess_degree"],
            report["max_successor_total_excess_degree"]
        );
        let unbounded = success(&["certify-candidates"], &bundle);
        for powers in admitted {
            let command = ["campaign", "reduce", "--artifact", "-", "--powers", powers];
            let bounded_result: toml::Value =
                toml::from_str(std::str::from_utf8(&success(&command, &bounded)).unwrap()).unwrap();
            let unbounded_result: toml::Value =
                toml::from_str(std::str::from_utf8(&success(&command, &unbounded)).unwrap())
                    .unwrap();
            for key in [
                "status",
                "target",
                "family_fingerprint",
                "common_mass_squared_symbol",
                "terms",
            ] {
                assert_eq!(bounded_result[key], unbounded_result[key]);
            }
        }
        let rejected = run(
            &["campaign", "reduce", "--artifact", "-", "--powers", outside],
            &bounded,
        );
        assert!(!rejected.status.success());
        assert!(rejected.stdout.is_empty());
        assert!(String::from_utf8_lossy(&rejected.stderr).contains("certified entry maximum 2"));
        let failed = run(
            &[
                "certify-candidates",
                "--max-total-excess-degree",
                "2",
                "--max-domain-bound-endpoint-cells",
                "0",
            ],
            &bundle,
        );
        assert!(!failed.status.success());
        assert!(failed.stdout.is_empty());
    }

    let bundle = success(&["family-candidates"], INPUT.as_bytes());
    for (degree, accepted, rejected) in [("0", "1", "2"), ("31", "32", "33")] {
        let artifact = success(
            &["certify-candidates", "--max-total-excess-degree", degree],
            &bundle,
        );
        success(
            &[
                "campaign",
                "reduce",
                "--artifact",
                "-",
                "--powers",
                accepted,
            ],
            &artifact,
        );
        let output = run(
            &[
                "campaign",
                "reduce",
                "--artifact",
                "-",
                "--powers",
                rejected,
            ],
            &artifact,
        );
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn total_excess_invalid_requests_fail_before_payload_decode_and_write_no_files() {
    let directory = Directory::new();
    let output = directory.0.join("must-not-exist.rrbin");
    for args in [
        vec!["certify-candidates", "--max-total-excess-degree", "-1"],
        vec![
            "certify-candidates",
            "--max-total-excess-degree",
            "18446744073709551616",
        ],
        vec![
            "certify-candidates",
            "--max-total-excess-degree",
            "2",
            "--max-negative-index-degree",
            "1",
        ],
        vec!["family-candidates", "--max-total-excess-degree", "2"],
    ] {
        let mut command = args;
        command.extend(["--output", output.to_str().unwrap()]);
        let failure = run(&command, b"not a native bundle");
        assert!(!failure.status.success());
        assert!(failure.stdout.is_empty());
        assert!(String::from_utf8_lossy(&failure.stderr).contains("--max-total-excess-degree"));
        assert!(!output.exists());
    }
    let help = success(&["--help"], b"");
    assert!(String::from_utf8_lossy(&help).contains("--max-total-excess-degree"));
    std::fs::write(&output, b"preserve existing output").unwrap();
    let rejected = run(
        &[
            "certify-candidates",
            "--max-total-excess-degree",
            "2",
            "--output",
            output.to_str().unwrap(),
        ],
        b"not decoded",
    );
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
    assert_eq!(std::fs::read(&output).unwrap(), b"preserve existing output");
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

#[test]
fn checkpoints_resume_across_worker_counts_with_unchanged_candidate_semantics() {
    let directory = Directory::new();
    let checkpoint = directory.0.join("sectors");
    let report = directory.0.join("first.toml");
    let resume_report = directory.0.join("resumed.toml");
    let ordinary = success(&["family-candidates"], INPUT.as_bytes());
    let initial = success(
        &[
            "family-candidates",
            "--checkpoint-dir",
            checkpoint.to_str().unwrap(),
            "--checkpoint-max-bytes",
            "1048576",
            "--report-output",
            report.to_str().unwrap(),
        ],
        INPUT.as_bytes(),
    );
    let before: Vec<_> = std::fs::read_dir(&checkpoint)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (path.clone(), std::fs::read(path).unwrap())
        })
        .collect();
    let resumed = success(
        &[
            "family-candidates",
            "--checkpoint-dir",
            checkpoint.to_str().unwrap(),
            "--resume",
            "--n-cores",
            "6",
            "--report-output",
            resume_report.to_str().unwrap(),
        ],
        INPUT.as_bytes(),
    );
    for bytes in [&initial, &resumed] {
        assert!(
            rustred::persistence::equivalent_generated_programs(
                &ordinary,
                bytes,
                BinaryIoLimits::default()
            )
            .unwrap()
        );
    }
    for (path, bytes) in before {
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
    let report: toml::Value = toml::from_str(&std::fs::read_to_string(report).unwrap()).unwrap();
    let resumed_report: toml::Value =
        toml::from_str(&std::fs::read_to_string(resume_report).unwrap()).unwrap();
    assert_eq!(report["status"].as_str(), Some("uncertified-candidates"));
    assert_eq!(resumed_report["status"], report["status"]);
    assert_eq!(report["checkpoint"]["reused_sectors"].as_integer(), Some(0));
    let new_sectors = report["checkpoint"]["newly_solved_sectors"]
        .as_integer()
        .unwrap();
    assert!(new_sectors > 0);
    assert_eq!(
        resumed_report["checkpoint"]["reused_sectors"].as_integer(),
        Some(new_sectors)
    );
    assert_eq!(
        resumed_report["checkpoint"]["newly_solved_sectors"].as_integer(),
        Some(0)
    );
    for key in ["disk_bytes", "resume_validation_us", "assembly_us"] {
        assert!(resumed_report["checkpoint"].get(key).is_some());
    }
    // Changed generation policy cannot reuse a previously committed sector.
    let rejected = run(
        &[
            "family-candidates",
            "--checkpoint-dir",
            checkpoint.to_str().unwrap(),
            "--resume",
            "--numerical-depth",
            "0",
        ],
        INPUT.as_bytes(),
    );
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
}

#[test]
fn checkpoint_flags_validate_before_input_and_preserve_default_generation() {
    for args in [
        vec!["family-candidates", "--resume"],
        vec!["family-candidates", "--checkpoint-max-bytes", "1"],
        vec![
            "family-candidates",
            "--checkpoint-dir",
            "x",
            "--checkpoint-max-bytes",
            "0",
        ],
        vec![
            "family-candidates",
            "--checkpoint-dir",
            "x",
            "--checkpoint-max-bytes",
            "-1",
        ],
        vec![
            "family-candidates",
            "--checkpoint-dir",
            "x",
            "--checkpoint-max-bytes",
            "true",
        ],
        vec![
            "family-candidates",
            "--checkpoint-dir",
            "x",
            "--checkpoint-max-bytes",
            "18446744073709551616",
        ],
        vec!["certify-candidates", "--checkpoint-dir", "x"],
        vec!["certify-candidates", "--resume"],
    ] {
        let rejected = run(&args, b"not parsed");
        assert!(!rejected.status.success());
        assert!(rejected.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&rejected.stderr).contains("--checkpoint")
                || String::from_utf8_lossy(&rejected.stderr).contains("--resume")
        );
    }
    let help = String::from_utf8(success(&["--help"], b"")).unwrap();
    assert!(help.contains("--checkpoint-dir"));
    assert!(help.contains("--checkpoint-max-bytes"));
    assert!(help.contains("outside that dedicated"));
}

#[test]
fn managed_checkpoint_paths_cannot_be_used_for_final_outputs_or_input() {
    let directory = Directory::new();
    let checkpoint = directory.0.join("not-created-yet");
    for option in ["--input", "--output", "--report-output"] {
        for path in [
            checkpoint.clone(),
            checkpoint.join("sector-0.rrbin"),
            checkpoint.join("missing/../checkpoint.toml"),
        ] {
            let rejected = run(
                &[
                    "family-candidates",
                    "--checkpoint-dir",
                    checkpoint.to_str().unwrap(),
                    option,
                    path.to_str().unwrap(),
                    "--force",
                ],
                b"not parsed",
            );
            assert!(!rejected.status.success());
            assert!(rejected.stdout.is_empty());
            assert!(
                String::from_utf8_lossy(&rejected.stderr)
                    .contains("outside the dedicated checkpoint"),
                "{}",
                String::from_utf8_lossy(&rejected.stderr)
            );
            assert!(!checkpoint.exists());
        }
    }
    #[cfg(unix)]
    {
        std::fs::create_dir(&checkpoint).unwrap();
        let alias = directory.0.join("alias");
        std::os::unix::fs::symlink(&checkpoint, &alias).unwrap();
        let output = alias.join("bundle.rrbin");
        let rejected = run(
            &[
                "family-candidates",
                "--checkpoint-dir",
                checkpoint.to_str().unwrap(),
                "--output",
                output.to_str().unwrap(),
                "--force",
            ],
            b"not parsed",
        );
        assert!(!rejected.status.success());
        assert!(
            String::from_utf8_lossy(&rejected.stderr).contains("outside the dedicated checkpoint")
        );
        assert_eq!(std::fs::read_dir(&checkpoint).unwrap().count(), 0);
        // Canonicalize the existing symlink ancestor before resolving '..':
        // lexically collapsing alias/.. first would incorrectly permit this.
        let inner = checkpoint.join("inner");
        std::fs::create_dir(&inner).unwrap();
        let inner_alias = directory.0.join("inner-alias");
        std::os::unix::fs::symlink(&inner, &inner_alias).unwrap();
        let escaped_lexically = inner_alias.join("../missing/final.rrbin");
        for option in ["--input", "--output", "--report-output"] {
            let rejected = run(
                &[
                    "family-candidates",
                    "--checkpoint-dir",
                    checkpoint.to_str().unwrap(),
                    option,
                    escaped_lexically.to_str().unwrap(),
                    "--force",
                ],
                b"not parsed",
            );
            assert!(!rejected.status.success());
            assert!(
                String::from_utf8_lossy(&rejected.stderr)
                    .contains("outside the dedicated checkpoint")
            );
            assert!(!checkpoint.join("missing").exists());
        }
    }
}

#[test]
fn basename_outputs_sync_the_child_working_directory_and_cold_load() {
    for checkpointed in [false, true] {
        let directory = Directory::new();
        let invoke = |arguments: &[&str], input: &[u8]| {
            let mut child = Command::new(env!("CARGO_BIN_EXE_rustred"))
                .args(arguments)
                .current_dir(&directory.0)
                .env("SYMBOLICA_HIDE_BANNER", "1")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child.stdin.take().unwrap().write_all(input).unwrap();
            let output = child.wait_with_output().unwrap();
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
        };
        let mut args = vec![
            "family-candidates",
            "--output",
            "bundle.rrbin",
            "--report-output",
            "report.toml",
        ];
        if checkpointed {
            args.extend(["--checkpoint-dir", "sectors"]);
        }
        assert!(invoke(&args, INPUT.as_bytes()).is_empty());
        let bytes = std::fs::read(directory.0.join("bundle.rrbin")).unwrap();
        let inspection =
            rustred_app::inspect_generated_candidate_bundle(&bytes, Default::default()).unwrap();
        assert_eq!(inspection.arity, 1);
        let report: toml::Value =
            toml::from_str(&std::fs::read_to_string(directory.0.join("report.toml")).unwrap())
                .unwrap();
        assert_eq!(report["status"].as_str(), Some("uncertified-candidates"));
        assert_eq!(report.get("checkpoint").is_some(), checkpointed);
        if checkpointed {
            assert!(directory.0.join("sectors/checkpoint.toml").is_file());
        }
        let report_bytes = std::fs::read(directory.0.join("report.toml")).unwrap();
        let rejected = Command::new(env!("CARGO_BIN_EXE_rustred"))
            .args(&args)
            .current_dir(&directory.0)
            .env("SYMBOLICA_HIDE_BANNER", "1")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(!rejected.status.success());
        assert!(rejected.stdout.is_empty());
        assert_eq!(
            std::fs::read(directory.0.join("bundle.rrbin")).unwrap(),
            bytes
        );
        assert_eq!(
            std::fs::read(directory.0.join("report.toml")).unwrap(),
            report_bytes
        );
        args.push("--force");
        if checkpointed {
            args.push("--resume");
        }
        assert!(invoke(&args, INPUT.as_bytes()).is_empty());
        // A distinct process cold-loads the saved basename; successful native
        // bytes are not inferred merely from the presence of an installed file.
        let artifact = invoke(&["certify-candidates", "--input", "bundle.rrbin"], b"");
        let reduced = invoke(
            &["campaign", "reduce", "--artifact", "-", "--powers", "3"],
            &artifact,
        );
        let reduced: toml::Value = toml::from_str(std::str::from_utf8(&reduced).unwrap()).unwrap();
        assert_eq!(reduced["status"].as_str(), Some("reduced"));
    }
}
