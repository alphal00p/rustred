use std::io::Write;
use std::process::{Command, Output, Stdio};

use rustred_app::{
    AppErrorKind, CampaignPlanRequest, CampaignPreflightRequest, DeriveRequest, InputFormat,
    MAX_INPUT_BYTES, RelationSelection, campaign_plan, campaign_preflight, derive,
};

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

const CAMPAIGN: &str = r#"
schema = "rustred.campaign-input.toml.v1"

[[roots]]
id = "tadpole"
integral = """
I(
  name(tadpole),
  loops(k),
  externals(),
  dimension(d),
  prop(D1,k^2-m2,1)
)
"""
"#;

const PROFILE: &str = r#"schema = "rustred.campaign-execution-resource-profile.v1"
estimator_revision = 19
enclosing_memory_limit = "1024B"

[fixed_memory]
process_runtime_and_shared_catalogs = "20B"
coordinator_stack_tls_workspace = "10B"
per_worker_stack_tls_workspace = "10B"
explicitly_admitted_inner_threads = "5B"
hydrated_retained_lanes = "0B"
staged_results = "10B"
checkpoint_and_output_buffers = "20B"
safety_reserve = "35B"

[minimum_runnable_task.retained_output]
visible_logical = "60B"
opaque_native_reserve = "0B"

[minimum_runnable_task.transient_excluding_output]
visible_logical = "40B"
opaque_native_reserve = "0B"
"#;

fn rustred(arguments: &[&str], input: &str) -> Output {
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
        .write_all(input.as_bytes())
        .expect("write CLI input");
    child.wait_with_output().expect("wait for RustRed CLI")
}

fn assert_cli_parity(arguments: &[&str], input: &str, application_toml: &str) {
    let output = rustred(arguments, input);
    assert_eq!(
        output.status.code(),
        Some(0),
        "RustRed CLI failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, application_toml.as_bytes());
}

#[test]
fn public_application_operations_are_byte_identical_to_the_cli() {
    let derived = derive(DeriveRequest {
        source: ONE_LOOP.to_owned(),
        input_format: InputFormat::Symbolica,
        relations: RelationSelection::All,
        n_cores: 1,
    })
    .expect("derive through public application API");
    assert_eq!(derived.schema(), "rustred.derive-output.toml.v1");
    assert_eq!(derived.status(), "ok");
    assert!(derived.to_toml().ends_with('\n'));
    assert_cli_parity(
        &[
            "derive",
            "--input",
            "-",
            "--input-format",
            "symbolica",
            "--relations",
            "all",
            "--n-cores",
            "1",
        ],
        ONE_LOOP,
        derived.to_toml(),
    );
    let serial_toml = derived.into_toml();
    let available = std::thread::available_parallelism().unwrap().get();
    for n_cores in (2_usize..=4).filter(|width| *width <= available) {
        let parallel = derive(DeriveRequest {
            source: ONE_LOOP.to_owned(),
            input_format: InputFormat::Symbolica,
            relations: RelationSelection::All,
            n_cores,
        })
        .expect("derive through public application API at parallel width");
        assert_eq!(
            parallel.to_toml(),
            serial_toml,
            "n_cores {n_cores} changed the canonical application output"
        );
        let width = n_cores.to_string();
        assert_cli_parity(
            &[
                "derive",
                "--input",
                "-",
                "--input-format",
                "symbolica",
                "--relations",
                "all",
                "--n-cores",
                width.as_str(),
            ],
            ONE_LOOP,
            parallel.to_toml(),
        );
    }

    let planned = campaign_plan(CampaignPlanRequest {
        source: CAMPAIGN.to_owned(),
        input_format: InputFormat::Toml,
        root_id: None,
    })
    .expect("plan campaign through public application API");
    assert_eq!(planned.schema(), "rustred.campaign-plan-output.toml.v1");
    assert_eq!(planned.status(), "ok");
    assert!(planned.to_toml().ends_with('\n'));
    assert_cli_parity(
        &["campaign", "plan", "--input", "-", "--input-format", "toml"],
        CAMPAIGN,
        planned.to_toml(),
    );

    let preflight = campaign_preflight(CampaignPreflightRequest {
        profile: PROFILE.to_owned(),
        n_cores: 4,
        max_memory_bytes: 900,
    })
    .expect("preflight campaign through public application API");
    assert_eq!(
        preflight.schema(),
        "rustred.campaign-execution-preflight-output.toml.v1"
    );
    assert_eq!(preflight.status(), "ready");
    assert!(preflight.to_toml().ends_with('\n'));
    assert_cli_parity(
        &[
            "campaign",
            "preflight",
            "--profile",
            "-",
            "--n-cores",
            "4",
            "--max-memory",
            "900B",
        ],
        PROFILE,
        preflight.to_toml(),
    );
}

#[test]
fn public_application_limits_fail_before_semantic_work() {
    let oversized = "x".repeat(MAX_INPUT_BYTES + 1);
    let oversized_error = derive(DeriveRequest::new(oversized)).unwrap_err();
    assert_eq!(oversized_error.kind(), AppErrorKind::Limit);
    assert!(oversized_error.message().contains("application limit"));

    let core_count_error = derive(DeriveRequest {
        source: ONE_LOOP.to_owned(),
        input_format: InputFormat::Symbolica,
        relations: RelationSelection::All,
        n_cores: 0,
    })
    .unwrap_err();
    assert_eq!(core_count_error.kind(), AppErrorKind::Input);
    assert!(core_count_error.message().contains("positive integer"));

    let memory_error = campaign_preflight(CampaignPreflightRequest {
        profile: String::new(),
        n_cores: 1,
        max_memory_bytes: 0,
    })
    .unwrap_err();
    assert_eq!(memory_error.kind(), AppErrorKind::Input);
    assert!(memory_error.message().contains("max_memory_bytes"));
    assert_eq!("symbolica".parse(), Ok(InputFormat::Symbolica));
    assert_eq!("li".parse(), Ok(RelationSelection::LorentzInvariance));

    let input_format_error = "json".parse::<InputFormat>().unwrap_err();
    assert_eq!(input_format_error.value(), "json");
    assert_eq!(
        input_format_error.to_string(),
        "invalid input format \"json\"; expected auto, toml, or symbolica"
    );
    let relation_error = "laporta".parse::<RelationSelection>().unwrap_err();
    assert_eq!(relation_error.value(), "laporta");
    assert_eq!(
        relation_error.to_string(),
        "invalid relation selection \"laporta\"; expected all, ordinary, or li"
    );
}
