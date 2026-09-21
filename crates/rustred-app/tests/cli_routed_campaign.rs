use rustred_app::{FamilyCandidatesRequest, family_candidates, inspect_generated_candidate_bundle};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn fixture() -> Directory {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "rustred-cli-routed-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).unwrap();
    Directory(path)
}
fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rustred"));
    for name in [
        "RAYON_NUM_THREADS",
        "OMP_NUM_THREADS",
        "OMP_THREAD_LIMIT",
        "OPENBLAS_NUM_THREADS",
        "MKL_NUM_THREADS",
        "BLIS_NUM_THREADS",
        "SYMBOLICA_HIDE_BANNER",
    ] {
        command.env(name, "1");
    }
    command.arg("routed-campaign");
    command
}

#[test]
fn cli_routed_writes_truthful_non_tty_result_and_cancel_receipt() {
    let directory = fixture();
    let source = r#"
schema="rustred.project.toml.v1"
[family]
name="public_shared_campaign_cli"
loop_momenta=["q"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P"
expression="q^2-1"
[target]
powers=[1]
"#;
    let bundle = family_candidates(FamilyCandidatesRequest::new(source)).unwrap();
    let inspection =
        inspect_generated_candidate_bundle(bundle.bundle(), Default::default()).unwrap();
    std::fs::write(directory.0.join("owner.rrbin"), bundle.bundle()).unwrap();
    let selection = json!({"family_fingerprint":inspection.family_fingerprint,
        "owners":[{"path":"owner.rrbin","bytes":bundle.bundle().len(),"mask":"1"}],"initial_frontier_routes":[]});
    std::fs::write(directory.0.join("selection.json"), selection.to_string()).unwrap();
    std::fs::write(directory.0.join("targets.csv"), "2\n3\n2\n").unwrap();
    let args = |name: &str| {
        vec![
            "--manifest".to_owned(),
            directory.0.join("selection.json").display().to_string(),
            "--targets".into(),
            directory.0.join("targets.csv").display().to_string(),
            "--owner-base".into(),
            directory.0.display().to_string(),
            "--output".into(),
            directory.0.join(name).display().to_string(),
        ]
    };
    let result = command().args(args("result.json")).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: Value =
        serde_json::from_slice(&std::fs::read(directory.0.join("result.json")).unwrap()).unwrap();
    assert_eq!(report["completed_finite_trace"], true);
    assert_eq!(report["family_closure_claim"], false);
    for line in String::from_utf8(result.stdout).unwrap().lines() {
        let event: Value = serde_json::from_str(line).unwrap();
        assert_eq!(event["event"], "heartbeat");
    }
    let stop = directory.0.join("stop");
    std::fs::write(&stop, "operator stop").unwrap();
    let cancelled = command()
        .args(args("cancelled.json"))
        .arg("--stop-file")
        .arg(stop)
        .output()
        .unwrap();
    assert!(!cancelled.status.success());
    let report: Value =
        serde_json::from_slice(&std::fs::read(directory.0.join("cancelled.json")).unwrap())
            .unwrap();
    assert_eq!(report["completed_finite_trace"], false);
    assert_eq!(report["status"], "cancelled_during_preparation");
    // Existing result is never overwritten, even on a repeated invocation.
    assert!(
        !command()
            .args(args("result.json"))
            .output()
            .unwrap()
            .status
            .success()
    );
}

#[test]
fn cli_routed_rejects_invalid_public_limits_before_loading() {
    for extra in [
        ["--workers", "51"],
        ["--max-input-targets", "0"],
        ["--timeout", "1800"],
    ] {
        let result = command()
            .args([
                "--manifest",
                "missing",
                "--targets",
                "missing",
                "--output",
                "unused",
            ])
            .args(extra)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
    }
}

#[test]
fn cli_owner_domain_scan_reuses_saved_program_without_targets() {
    let directory = fixture();
    let source = r#"
schema="rustred.project.toml.v1"
[family]
name="public_domain_scan_cli"
loop_momenta=["q"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P"
expression="q^2-1"
[target]
powers=[1]
"#;
    let bundle = family_candidates(FamilyCandidatesRequest::new(source)).unwrap();
    let inspection =
        inspect_generated_candidate_bundle(bundle.bundle(), Default::default()).unwrap();
    std::fs::write(directory.0.join("owner.rrbin"), bundle.bundle()).unwrap();
    let selection = json!({"family_fingerprint":inspection.family_fingerprint,
        "owners":[{"path":"owner.rrbin","bytes":bundle.bundle().len(),"mask":"1"}],"initial_frontier_routes":[]});
    std::fs::write(directory.0.join("selection.json"), selection.to_string()).unwrap();
    let args = |output: &str| {
        vec![
            "owner-domain-scan".to_owned(),
            "--manifest".into(),
            directory.0.join("selection.json").display().to_string(),
            "--owner-base".into(),
            directory.0.display().to_string(),
            "--max-numerator-rank".into(),
            "10".into(),
            "--output".into(),
            directory.0.join(output).display().to_string(),
        ]
    };
    let run = |arguments: Vec<String>| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_rustred"));
        for name in [
            "RAYON_NUM_THREADS",
            "OMP_NUM_THREADS",
            "OMP_THREAD_LIMIT",
            "OPENBLAS_NUM_THREADS",
            "MKL_NUM_THREADS",
            "BLIS_NUM_THREADS",
            "SYMBOLICA_HIDE_BANNER",
        ] {
            command.env(name, "1");
        }
        command.args(arguments).output().unwrap()
    };
    let output = run(args("domains.json"));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&std::fs::read(directory.0.join("domains.json")).unwrap()).unwrap();
    assert_eq!(report["scan_complete"], true);
    assert_eq!(report["positive_powers_unbounded"], true);
    assert_eq!(report["family_closure_claim"], false);
    assert_eq!(report["ibp_generation"], false);
    let heartbeats: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(!heartbeats.is_empty());
    assert!(
        heartbeats
            .iter()
            .all(|record| record["event"] == "heartbeat")
    );
    assert_eq!(
        heartbeats.last().unwrap()["progress"]["operation"],
        "owner_domain_scan"
    );
    assert!(!run(args("domains.json")).status.success());
    let stop = directory.0.join("stop");
    std::fs::write(&stop, "stop").unwrap();
    let mut stopped = args("cancelled-domains.json");
    stopped.extend(["--stop-file".into(), stop.display().to_string()]);
    assert!(!run(stopped).status.success());
    let report: Value =
        serde_json::from_slice(&std::fs::read(directory.0.join("cancelled-domains.json")).unwrap())
            .unwrap();
    assert_eq!(report["scan_complete"], false);
    assert_eq!(report["status"], "cancelled_during_preparation");
}
