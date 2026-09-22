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
fn cli_owner_guarded_apply_preserves_complement_and_partial_diagnostic_status() {
    use rustred_app::{OwnerDomainMatchRequest, owner_domain_match_with_progress};
    let directory = fixture();
    let source = r#"
schema="rustred.project.toml.v1"
[family]
name="guarded_cli_one_hop"
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
    let selection = json!({"family_fingerprint":inspection.family_fingerprint,"owners":[{"path":"owner.rrbin","bytes":bundle.bundle().len(),"mask":"1"}],"initial_frontier_routes":[]});
    std::fs::write(directory.0.join("selection.json"), selection.to_string()).unwrap();
    let mut local = OwnerDomainMatchRequest::new(
        selection.to_string(),
        json!({"schema":"rustred.owner-domain-queries.json.v2","queries":[{
        "id":"local","owner":"1","lower":[1],"upper":[null],"max_numerator_rank":11}]})
        .to_string(),
    );
    local.owner_base = directory.0.clone();
    let classified =
        owner_domain_match_with_progress(local, &std::sync::atomic::AtomicBool::new(false), |_| {})
            .unwrap();
    let selector = &classified.document["queries"][0]["pieces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["disposition"]["kind"] == "selected_rule")
        .unwrap()["disposition"];
    let queries = json!({"schema":"rustred.owner-guarded-rule-queries.json.v1","queries":[{
        "id":"own\"rule\\display","owner":"1","lower":[1],"upper":[null],"max_numerator_rank":11,
        "batch":selector["batch"],"rule":selector["rule"]}]});
    std::fs::write(directory.0.join("queries.json"), queries.to_string()).unwrap();
    std::fs::write(
        directory.0.join("limits.json"),
        r#"{"max_events":12345,"applied":{"max_events":23456}}"#,
    )
    .unwrap();
    let run = |output: &str, extra: &[&str]| {
        let mut c = Command::new(env!("CARGO_BIN_EXE_rustred"));
        for key in [
            "RAYON_NUM_THREADS",
            "OMP_NUM_THREADS",
            "OMP_THREAD_LIMIT",
            "OPENBLAS_NUM_THREADS",
            "MKL_NUM_THREADS",
            "BLIS_NUM_THREADS",
            "SYMBOLICA_HIDE_BANNER",
        ] {
            c.env(key, "1");
        }
        c.arg("owner-guarded-apply")
            .arg("--manifest")
            .arg(directory.0.join("selection.json"))
            .arg("--queries")
            .arg(directory.0.join("queries.json"))
            .arg("--owner-base")
            .arg(&directory.0)
            .arg("--output")
            .arg(directory.0.join(output))
            .arg("--work-limits")
            .arg(directory.0.join("limits.json"))
            .args(extra)
            .output()
            .unwrap()
    };
    let done = run("done.json", &[]);
    assert!(
        done.status.success(),
        "{}",
        String::from_utf8_lossy(&done.stderr)
    );
    let bytes = std::fs::read(directory.0.join("done.json")).unwrap();
    let document: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(document["diagnostic_complete"], true);
    assert_eq!(document["family_closure_claim"], false);
    assert_eq!(document["work_limits"]["max_events"], 12345);
    assert_eq!(document["work_limits"]["applied"]["max_events"], 23456);
    assert!(bytes.len() <= document["report_payload_bytes_charged"].as_u64().unwrap() as usize);
    assert!(
        document["queries"][0]["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["residual"] == "IncomingComplement")
    );
    let partial = run("partial.json", &["--max-report-events", "1"]);
    assert_eq!(partial.status.code(), Some(4));
    let document: Value =
        serde_json::from_slice(&std::fs::read(directory.0.join("partial.json")).unwrap()).unwrap();
    assert_eq!(document["diagnostic_complete"], false);
    assert_eq!(document["retained_events"], 1);
    assert_eq!(document["queries"][0]["inspection_finished"], false);
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

#[test]
fn cli_owner_domain_match_emits_compact_local_results_and_retains_cancelled_prefix() {
    let directory = fixture();
    let source = r#"
schema="rustred.project.toml.v1"
[family]
name="public_domain_match_cli"
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
        "owners":[{"path":"owner.rrbin","bytes":bundle.bundle().len(),"mask":"1"}],
        "initial_frontier_routes":[]});
    let queries = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
        {"id":"unbounded-ray", "owner":"1", "lower":[0], "upper":[null],
            "max_numerator_rank":11}]});
    std::fs::write(directory.0.join("selection.json"), selection.to_string()).unwrap();
    std::fs::write(directory.0.join("queries.json"), queries.to_string()).unwrap();
    let invoke = |name: &str, extra: &[&str]| {
        let mut process = Command::new(env!("CARGO_BIN_EXE_rustred"));
        for variable in [
            "RAYON_NUM_THREADS",
            "OMP_NUM_THREADS",
            "OMP_THREAD_LIMIT",
            "OPENBLAS_NUM_THREADS",
            "MKL_NUM_THREADS",
            "BLIS_NUM_THREADS",
            "SYMBOLICA_HIDE_BANNER",
        ] {
            process.env(variable, "1");
        }
        process
            .arg("owner-domain-match")
            .arg("--manifest")
            .arg(directory.0.join("selection.json"))
            .arg("--queries")
            .arg(directory.0.join("queries.json"))
            .arg("--owner-base")
            .arg(&directory.0)
            .arg("--output")
            .arg(directory.0.join(name))
            .args(extra)
            .output()
            .unwrap()
    };
    let output = invoke("matched.json", &[]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&std::fs::read(directory.0.join("matched.json")).unwrap()).unwrap();
    assert_eq!(report["classification_complete"], true);
    assert_eq!(report["all_queries_locally_applicable"], true);
    assert_eq!(report["bounded_refinement_axes"], "inactive-only");
    assert_eq!(report["max_bounded_refinement_cells"], 0);
    for flag in [
        "family_closure_claim",
        "ibp_generation",
        "rhs_successors_expanded",
    ] {
        assert_eq!(report[flag], false);
    }
    let heartbeats: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(!heartbeats.is_empty());
    let final_event = &heartbeats.last().unwrap()["progress"];
    assert_eq!(final_event["operation"], "owner_domain_match");
    assert_eq!(final_event["bounded_refinement_axes"], "inactive-only");
    assert!(final_event.get("queries").is_none());
    assert!(serde_json::to_vec(final_event).unwrap().len() < 8192);
    assert!(!invoke("matched.json", &[]).status.success());
    let walked = invoke(
        "walked.json",
        &[
            "--follow-successors",
            "--workers",
            "1",
            "--max-frontiers",
            "19",
            "--max-successor-events",
            "100000001",
            "--max-rhs-events-per-query",
            "100003",
            "--max-shift-groups-per-query",
            "100007",
            "--max-sign-splits-per-query",
            "100009",
            "--bounded-refinement-axes",
            "finite-axes",
            "--max-bounded-refinement-cells-per-query",
            "23",
        ],
    );
    assert!(
        walked.status.success(),
        "{}",
        String::from_utf8_lossy(&walked.stderr)
    );
    let walk: Value =
        serde_json::from_slice(&std::fs::read(directory.0.join("walked.json")).unwrap()).unwrap();
    assert_eq!(walk["schema"], "rustred.owner-domain-walk.json.v2");
    assert_eq!(walk["workers"], 1);
    assert_eq!(walk["max_frontiers"], 19);
    assert_eq!(walk["max_events"], 100000001);
    assert!(walk.get("max_containment_checks").unwrap().is_null());
    assert_eq!(
        walk["containment_check_policy"],
        "general_comparisons_only; null_is_unlimited; checked_counter"
    );
    assert_eq!(walk["applied_limits"]["max_events"], 100003);
    assert_eq!(walk["applied_limits"]["max_shift_groups"], 100007);
    assert_eq!(walk["applied_limits"]["max_sign_splits"], 100009);
    assert_eq!(walk["bounded_refinement_axes"], "finite-axes");
    assert_eq!(walk["max_bounded_refinement_cells"], 23);
    assert_eq!(
        walk["applied_limits"]["matching"]["refinement_axes"],
        "finite-axes"
    );
    assert_eq!(
        walk["applied_limits"]["matching"]["max_bounded_refinement_cells"],
        23
    );
    assert_eq!(walk["all_scheduled_domains_resolved"], true);
    assert_eq!(walk["family_closure_claim"], false);
    assert_eq!(walk["committed_events"], walk["events"]);
    let limited = invoke("limited.json", &["--max-total-pieces", "1"]);
    assert!(!limited.status.success());
    let report: Value =
        serde_json::from_slice(&std::fs::read(directory.0.join("limited.json")).unwrap()).unwrap();
    assert_eq!(report["classification_complete"], false);
    assert_eq!(report["retained_pieces"], 1);
    let stop = directory.0.join("stop");
    std::fs::write(&stop, "stop").unwrap();
    assert!(
        !invoke("cancelled.json", &["--stop-file", stop.to_str().unwrap()])
            .status
            .success()
    );
    let report: Value =
        serde_json::from_slice(&std::fs::read(directory.0.join("cancelled.json")).unwrap())
            .unwrap();
    assert_eq!(report["classification_complete"], false);
    assert_eq!(report["status"], "cancelled_during_preparation");
}
