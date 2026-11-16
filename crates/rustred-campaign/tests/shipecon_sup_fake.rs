//! Lane shipecon (interfaces/shipecon.md 13): the ship economy's GUARD as SUPERVISED work items. A real `rr_campaign`
//! supervisor runs a campaign whose input of role `shipecon` names one toy node (arity 3, mask 110; test data only, the
//! material of tests/shipecon_fake.rs); the guard's solve-recipe jobs are ledger items (`ship-guard`) run by fake children
//! (`rr_fake_child` emulating kind solve-recipe) through the supervisor's launch path. Two steps: the first certifies
//! nothing (the item escalates, the next round is a new item), the second certifies the heavy point rule's key.
//! RESUME = the ledger: the supervisor is aborted right AFTER the first settle commit (the answer is in the ledger) and,
//! separately, right BEFORE it (the attempt is settled again after the restart, before the state is loaded); both
//! campaigns end with the decisions and totals of the uninterrupted one, and a further `run` launches nothing.
//! Spawns processes: run ONLY under `LIGHT_PIDNS=1` (private pid namespace, rule L10):
//! `LIGHT_PIDNS=1 light_build.sh shipecon test -p rustred-campaign --test shipecon_sup_fake`

mod sup_common;

use rustred_campaign::ledger::{Event, ItemEvent};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;
use sup_common::*;

const S1: &str = "d2_cfirst_fon";
const S2: &str = "d2_clast_fon";

fn rule(index: i64, fixed: [Option<i64>; 3], rhs: &[&str], extra: Value) -> String {
    let t: Vec<String> = (0..3)
        .map(|i| fixed[i].map(|v| v.to_string()).unwrap_or(format!("n{i}")))
        .collect();
    let mut v = json!({"type":"rule","chart":"110","index":index,"fixed":fixed,"equations":[],
        "target": format!("I({})", t.join(",")),
        "rhs": rhs.iter().map(|k| json!([k, "1"])).collect::<Vec<_>>(),
        "exceptions":[],"dispatch":"Partition","witness_loci":[]});
    if let Value::Object(m) = extra {
        for (k, x) in m {
            v[k] = x;
        }
    }
    v.to_string()
}

/// The toy node (as tests/shipecon_fake.rs) and its ship campaign file.
fn campaign_file(dir: &Path) -> PathBuf {
    let pad = "x".repeat(4000);
    let lines = [
        r#"{"type":"header","schema":"table_dump.v1","family":"TOY","denominators":3,"mask":"110"}"#.to_string(),
        r#"{"type":"chart","chart":"110","guards":["n>=1","n>=1","n<=0"],"zero":false}"#.to_string(),
        rule(
            0,
            [Some(2), Some(1), Some(0)],
            &["I(1,1,0)", "I(2,0,0)"],
            json!({"origin": pad}),
        ),
        rule(
            1,
            [None, Some(1), Some(0)],
            &["I(n0-1,1,0)", "I(n0,0,0)"],
            json!({"exceptions": [["n0-2"]]}),
        ),
        rule(2, [Some(9), Some(9), Some(0)], &["I(8,9,0)"], json!({"origin": pad})),
        r#"{"type":"end","charts":1}"#.to_string(),
    ];
    let t = dir.join("A.table.jsonl");
    std::fs::write(&t, lines.join("\n") + "\n").unwrap();
    let st = dir.join("start.jsonl");
    std::fs::write(&st, "[3,1,0]\n[2,1,0]\n").unwrap();
    let sha = rustred_campaign_core::import::lines::file_sha256(&t).unwrap();
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A","bits":6,
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"A","kind":"engine_table","path":t.display().to_string(),"sha256": sha}]});
    let sp = dir.join("import-spec.json");
    std::fs::write(&sp, spec.to_string()).unwrap();
    let family = dir.join("family.sym");
    std::fs::write(&family, "fake family\n").unwrap();
    let c = json!({"schema": "rustred.campaign.shipecon-campaign.v1", "nodes": [{
        "node": "toy", "import_spec": sp, "import_base": dir, "program": "rr_fake_child",
        "family": family,
        "order": {"name":"o","slot_permutation":null,"coordinate_priority":null,"source_order":"Input"},
        "policy": {"schema": "rustred.campaign.shipecon.v1", "mode": "apply",
            "candidates": {"min_rule_bytes": 1000}, "bars": {"min_bytes_saved": 100},
            "recipe": {"steps": [S1, S2], "relation_rule": "first_admissible"}},
        "engine_probe_sample": 10, "check_threads": 1, "cap_bytes": 268435456u64, "expected_peak_bytes": 67108864u64,
        "fake": {S2: [[[2, 1, 0], [[1, 1, 0]]]]}
    }]});
    let p = dir.join("shipecon-campaign.json");
    std::fs::write(&p, serde_json::to_vec_pretty(&c).unwrap()).unwrap();
    p
}

fn blake3_file(p: &Path) -> String {
    blake3::hash(&std::fs::read(p).unwrap())
        .to_hex()
        .to_string()
}

fn campaign_spec(t: &Tmp, file: &Path) -> PathBuf {
    let v = json!({
        "schema": "rustred.campaign.spec.v1", "campaign": t.campaign,
        "programs": {"rr_fake_child": {"path": FAKE}},
        "resources": {"policy": "plain", "cpus": cpus(), "max_jobs": 4, "mem_total": "2G", "disk_floor": "1G",
                      "grace_seconds": 5, "sampler_ms": 300, "host_floor": "100M", "heartbeat_timeout_s": 60,
                      "start_timeout_s": 120},
        "progress": {"cadence_seconds": 1},
        "inputs": [{"id": "shipecon-campaign", "role": "shipecon", "path": file.display().to_string(),
                    "blake3": blake3_file(file)}]
    });
    write_spec(t, "spec.json", &v)
}

fn report_text(cdir: &Path) -> String {
    std::fs::read_to_string(cdir.join("shipecon/toy/report.json")).unwrap_or_default()
}

fn report(cdir: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(cdir.join("shipecon/toy/report.json")).unwrap())
        .unwrap_or_else(|e| panic!("report: {e}; log {}", log_tail(cdir)))
}

/// The checks of one finished campaign; returns (decisions, totals) for the comparison across runs.
fn check(cdir: &Path) -> (Value, Value) {
    let rep = report(cdir);
    assert!(rep["error"].is_null(), "{rep}");
    assert_eq!(rep["census"]["rules"], 3, "{}", rep["census"]);
    assert_eq!(rep["examined"], 1);
    let dec = rep["decisions"].as_array().unwrap();
    let by = |l: i64| {
        dec.iter()
            .find(|d| d["rule"]["label"] == l)
            .unwrap_or_else(|| panic!("rule {l}: {}", rep["decisions"]))
    };
    assert_eq!(by(2)["outcome"], "kept_aside");
    let o0 = by(0)["outcome"].as_str().unwrap();
    assert!(
        o0 == "stored_relations" || o0 == "recipe",
        "rule 0: {}",
        by(0)
    );
    assert_eq!(by(0)["guard_holds"], true);
    assert_eq!(
        rep["closing_check"]["holds"], true,
        "{}",
        rep["closing_check"]
    );
    let last = rep["closing_iterations"]
        .as_array()
        .unwrap()
        .last()
        .unwrap();
    assert_eq!(last["served_set"]["holds"], true, "{last}");
    assert_eq!(last["served_set"]["lost"], 0);
    // the guard ran as ledger items: round 0 (step 1, nothing certified: escalated), round 1 (step 2: accepted)
    let st = state(cdir);
    let mut items: Vec<(&String, String)> = st
        .items
        .iter()
        .filter(|(_, it)| it.kind == "ship-guard")
        .map(|(k, it)| (k, it.phase.tag().to_string()))
        .collect();
    items.sort();
    assert_eq!(items.len(), 2, "{items:?}");
    assert_eq!(items[0].0, "toy.ship.r0.0");
    assert_eq!(items[0].1, "escalated", "{items:?}");
    assert_eq!(items[1].0, "toy.ship.r1.0");
    assert_eq!(items[1].1, "accepted", "{items:?}");
    assert!(st.inputs.contains_key("shipecon:toy:report"));
    let mut d = rep["decisions"].clone();
    // the run-local fields (paths of this campaign directory) aside
    if let Some(a) = d.as_array_mut() {
        for x in a {
            if let Some(m) = x.as_object_mut() {
                m.remove("recipes");
            }
        }
    }
    (d, rep["totals"].clone())
}

fn jobs_queued(cdir: &Path) -> usize {
    count(cdir, |e| matches!(e, Event::JobQueued { .. }))
}

#[test]
fn the_guard_runs_as_supervised_items_and_resumes_from_the_ledger() {
    let _g = serial();
    assert!(
        rustred_campaign::launch::in_private_namespace(),
        "run inside a private pid namespace (L10): LIGHT_PIDNS=1"
    );
    // (a) uninterrupted
    let t = Tmp::new("shipecon");
    let file = campaign_file(&t.path);
    let spec = campaign_spec(&t, &file);
    let cdir = t.cdir("c");
    let codes = run_to_end(&spec, &cdir, &[]);
    assert_eq!(
        codes.last(),
        Some(&0),
        "{codes:?}; log {}; report {}",
        log_tail(&cdir),
        report_text(&cdir)
    );
    let (d0, t0) = check(&cdir);
    assert!(t0["bytes_saved"].as_i64().unwrap() > 4000, "{t0}");
    // a further run on the finished campaign: nothing new
    let q0 = jobs_queued(&cdir);
    let codes = run_to_end(&spec, &cdir, &[]);
    assert_eq!(codes, vec![0], "log {}", log_tail(&cdir));
    assert_eq!(
        jobs_queued(&cdir),
        q0,
        "a finished ship campaign launched work again"
    );
    let accepts = count(&cdir, |e| {
        matches!(
            e,
            Event::Item {
                ev: ItemEvent::Accept { .. },
                ..
            }
        )
    });
    assert_eq!(accepts, 1);

    // (b), (c): the supervisor aborted at the first settle (after / before its commit), then resumed
    for site in ["after_settle_commit:1", "before_settle_commit:1"] {
        let t = Tmp::new("shipecon-fp");
        let file = campaign_file(&t.path);
        let spec = campaign_spec(&t, &file);
        let cdir = t.cdir("c");
        let mut c = start(&spec, &cdir, &[], &[("RR_CAMPAIGN_FAILPOINT", site)]);
        let code = wait(&mut c, Duration::from_secs(600));
        assert!(
            !finished(code),
            "{site}: the failpoint did not stop the run (exit {code}); log {}",
            log_tail(&cdir)
        );
        assert!(
            !cdir.join("shipecon/toy/report.json").exists(),
            "{site}: report before the abort"
        );
        let codes = run_to_end(&spec, &cdir, &[]);
        assert_eq!(
            codes.last(),
            Some(&0),
            "{site}: {codes:?}; log {}; report {}",
            log_tail(&cdir),
            report_text(&cdir)
        );
        let (d, tt) = check(&cdir);
        assert_eq!(tt, t0, "{site}: totals differ from the uninterrupted run");
        assert_eq!(d, d0, "{site}: decisions differ from the uninterrupted run");
    }
}
