//! Lane termmin: the REAL run of the direct driver (interfaces/termmin.md 8) on existing material, one child per held
//! CPU. `#[ignore]`: run by hand INSIDE A PRIVATE PID NAMESPACE (`unshare -Urpf --mount-proc`, rule L10) after
//! `source WS/ops/env.sh` (licence by NAME only), with `TERMMIN_REAL_CONFIG` = a JSON file:
//! `{"node", "import_spec", "import_base", "terminals": [[..]] | null, "spec": {termmin spec}, "program": "<rr_node>",
//!   "invocation": "contract" | "verb", "family", "order": "<name>" | {order JSON}, "cpus": [..] | null (HELD_CPUS),
//!   "work_root", "out_dir", "check_threads", "wall_cap_s", "extra_solved": [..]}`.

use rustred_campaign::adapters::job::{Invocation, Program};
use rustred_campaign::closeloop::termmin::direct::{DirectConfig, run_direct};
use rustred_campaign_core::termmin::TermminSpec;
use serde_json::Value;
use std::path::PathBuf;

#[test]
#[ignore = "real children: TERMMIN_REAL_CONFIG, private pid namespace"]
fn real_terminal_minimisation() {
    assert!(
        rustred_campaign::launch::in_private_namespace(),
        "run inside a private pid namespace (L10)"
    );
    let p = std::env::var("TERMMIN_REAL_CONFIG").expect("TERMMIN_REAL_CONFIG");
    let c: Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
    let s = |k: &str| c[k].as_str().unwrap_or_else(|| panic!("{k}")).to_string();
    let cpus: Vec<usize> = match c["cpus"].as_array() {
        Some(a) => a.iter().map(|x| x.as_u64().unwrap() as usize).collect(),
        None => std::env::var("HELD_CPUS")
            .expect("HELD_CPUS")
            .split(',')
            .map(|x| x.trim().parse().unwrap())
            .collect(),
    };
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
    let cfg = DirectConfig {
        node: s("node"),
        import_spec: s("import_spec").into(),
        import_base: s("import_base").into(),
        extra_solved: c["extra_solved"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|x| PathBuf::from(x.as_str().unwrap()))
            .collect(),
        terminals: serde_json::from_value(c["terminals"].clone()).unwrap_or(None),
        spec: TermminSpec::parse(&c["spec"].to_string()).unwrap(),
        program,
        family: s("family").into(),
        order,
        cpus,
        work_root: s("work_root").into(),
        out_dir: s("out_dir").into(),
        check_threads: c["check_threads"].as_u64().unwrap_or(1) as usize,
        licence_env: std::env::var_os("SYMBOLICA_LICENSE").map(|_| "SYMBOLICA_LICENSE".to_string()),
        wall_cap_s: c["wall_cap_s"].as_u64().unwrap_or(600),
        verbose: true,
    };
    let rep = run_direct(&cfg).unwrap();
    eprintln!(
        "FINAL {}: terminals {} -> {}: {}",
        cfg.node,
        rep["economy_before"]["terminals"],
        rep["economy_after"]["terminals"],
        rep["final_terminals"]
    );
    for t in rep["terminals"].as_array().unwrap() {
        eprintln!("  {} {} {}", t["key_text"], t["outcome"], t["class"]);
        for st in t["steps"].as_array().unwrap() {
            eprintln!(
                "    {:>2} {:<26} {:<12} seeds {:>6} verdict {:<22} cpu {:>8} vmhwm {:>9} {}",
                st["index"],
                st["step"].as_str().unwrap_or(""),
                st["outcome"].as_str().unwrap_or(""),
                st["seeds"],
                st["verdict"].as_str().unwrap_or("-"),
                st["cost"]["cpu_s"],
                st["cost"]["vmhwm_kb"],
                st["skipped"]
                    .as_str()
                    .or(st["extension"]["class"].as_str())
                    .unwrap_or("")
            );
        }
    }
}
