//! Lane shipecon: the REAL run of the ship-economy driver (interfaces/shipecon.md) on existing material. `#[ignore]`:
//! run by hand INSIDE A PRIVATE PID NAMESPACE (`unshare -Urpf --mount-proc`, rule L10) after `source WS/ops/env.sh`
//! (licence by NAME only) and a private TMPDIR (L22), with `SHIPECON_REAL_CONFIG` = a JSON file:
//! `{"node", "import_spec", "import_base", "statement_rows": null | path, "policy": {ship policy}, "program": "<rr_node>",
//!   "invocation": "contract" | "verb", "family", "order": "<name>" | {order JSON}, "parallel", "threads_per_job",
//!   "cap_bytes", "wall_cap_s", "work_root", "out_dir", "check_threads", "previous": null | path,
//!   "table_times": null | path}`.

use rustred_campaign::adapters::job::{Invocation, Program};
use rustred_campaign::closeloop::shipecon::{ShipRunConfig, run_ship};
use rustred_campaign_core::economy::ship::ShipPolicy;
use serde_json::Value;
use std::path::PathBuf;

#[test]
#[ignore = "real children: SHIPECON_REAL_CONFIG, private pid namespace"]
fn real_ship_economy() {
    assert!(
        rustred_campaign::launch::in_private_namespace(),
        "run inside a private pid namespace (L10)"
    );
    let p = std::env::var("SHIPECON_REAL_CONFIG").expect("SHIPECON_REAL_CONFIG");
    let c: Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
    let s = |k: &str| c[k].as_str().unwrap_or_else(|| panic!("{k}")).to_string();
    let opt = |k: &str| c[k].as_str().map(PathBuf::from);
    let invocation = if c["invocation"] == "verb" {
        Invocation::VerbJobFlag {
            verb: "solve-recipe".into(),
        }
    } else {
        Invocation::Contract
    };
    let program = Program::from_file("rr_node", &PathBuf::from(s("program")), invocation).unwrap();
    let order = match &c["order"] {
        Value::String(x) => x.clone(),
        v => v.to_string(),
    };
    let cfg = ShipRunConfig {
        node: s("node"),
        import_spec: s("import_spec").into(),
        import_base: s("import_base").into(),
        statement_rows: opt("statement_rows"),
        policy: ShipPolicy::parse(&c["policy"].to_string()).unwrap(),
        program,
        family: s("family").into(),
        order,
        parallel: c["parallel"].as_u64().unwrap_or(4) as usize,
        threads_per_job: c["threads_per_job"].as_u64().unwrap_or(1) as u32,
        cap_bytes: c["cap_bytes"].as_u64().unwrap_or(8 << 30),
        wall_cap_s: c["wall_cap_s"].as_u64().unwrap_or(3600),
        work_root: s("work_root").into(),
        out_dir: s("out_dir").into(),
        check_threads: c["check_threads"].as_u64().unwrap_or(4) as usize,
        licence_env: std::env::var_os("SYMBOLICA_LICENSE").map(|_| "SYMBOLICA_LICENSE".to_string()),
        previous: opt("previous"),
        table_times: opt("table_times"),
        engine_probe_sample: c["engine_probe_sample"].as_u64().unwrap_or(2000) as usize,
        engine_first_match: opt("engine_first_match"),
        statement_format: c["statement_format"].as_str().unwrap_or("tool").to_string(),
        rule_index: opt("rule_index"),
        check_mode: c["check_mode"].as_str().unwrap_or("full").to_string(),
        later_rule_probe: opt("later_rule_probe"),
        resume_trials: opt("resume_trials"),
        previous_solved: opt("previous_solved"),
        previous_work: opt("previous_work"),
        fake: None,
        verbose: true,
    };
    let rep = run_ship(&cfg).unwrap();
    eprintln!(
        "FINAL {}: totals {} closing {}",
        cfg.node, rep["totals"], rep["closing_check"]
    );
    for d in rep["decisions"].as_array().unwrap() {
        eprintln!(
            "  {} {:<16} bytes {:>11} -> {:>11} in/out {}/{} secs {} {}",
            d["rule"]["label"],
            d["outcome"].as_str().unwrap_or(""),
            d["bytes_before"],
            d["bytes_after"],
            d["served_in_start"],
            d["served_outside_start"],
            d["seconds_added_total"],
            d["why"].as_str().unwrap_or("")
        );
    }
}
