//! Lane sizegen (interfaces/genchoice.md): the generation-choice portfolio under a REAL `rr_campaign` supervisor with
//! `rr_fake_child` emulating the generate jobs (probes, first full run, economy alternatives). Spawns processes: run
//! ONLY inside a private pid namespace (`LIGHT_PIDNS=1 .../light_build.sh sizegen test -p rustred-campaign --test
//! genchoice_fake`).
//!
//! TOY node (arity 3: lines 0, 1, ISP 2; test data only). Candidates = orders {id, bfi, isp_rev} x poses {default,
//! rung}. Fake answers: bfi-default has the cheapest probe (first full run, acceptable); bfi-rung's probe is a band
//! dearer but its FULL table is the smallest (found only by the economy step); id-rung is the second alternative;
//! isp_rev-rung's probe completes the node by itself (a full run for free, never re-run); isp_rev-default meets a wall
//! stop (heavy, ranked last); id-default carries one huge rule.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

mod closeloop_support;
mod sup_common;

use closeloop_support::{blake3_file, sha256_file};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use sup_common::*;

const FAMILY: &str = "I(\n name(TOY),\n loops(k1),\n externals(q),\n dimension(d),\n prop(D1,(k1)^2,1),\n prop(D2,(k1+q)^2,1),\n prop(D3,(k1-q)^2,0),\n gram(q,q,1)\n)\n";

fn rule(fixed: [Option<i64>; 3], monomials: u64) -> Value {
    // one target per case (the seal refuses two rules with one target)
    let target = format!(
        "I({})",
        fixed
            .iter()
            .enumerate()
            .map(|(k, f)| f.map_or(format!("n{k}"), |v| v.to_string()))
            .collect::<Vec<_>>()
            .join(",")
    );
    json!({"chart":"110","fixed":fixed,"equations":[],"target":target,
        "rhs":[["I(n0-1,n1,n2)","1"]],"exceptions":[],"dispatch":"Partition","witness_loci":[],
        "monomials":monomials,"trace_rows":3})
}

/// Rules of a toy table: the given monomials on a fixed sequence of cases.
fn rules(monos: &[u64]) -> Vec<Value> {
    let cases: [[Option<i64>; 3]; 6] = [
        [None, None, None],
        [Some(1), None, None],
        [None, Some(1), None],
        [None, None, Some(0)],
        [Some(1), Some(1), None],
        [Some(2), None, Some(0)],
    ];
    monos
        .iter()
        .enumerate()
        .map(|(k, &m)| rule(cases[k % cases.len()], m))
        .collect()
}

fn material(dir: &Path, optimise: bool) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let fam = dir.join("TOY.sym");
    std::fs::write(&fam, FAMILY).unwrap();
    let bx = dir.join("box.json");
    std::fs::write(
        &bx,
        r#"{"schema":"rustred.campaign.box.v1","id":"toy-box"}"#,
    )
    .unwrap();
    let run = |probe: Value, full: Value| json!({"probe": probe, "full": full});
    let tbl = |m: &[u64], stop: Value| json!({"rules": rules(m), "stop": stop});
    let runs = json!({
        "id-default-std": run(tbl(&[100, 5_000_000, 100, 100], json!("max_cases")), tbl(&[100, 5_000_000, 100, 100, 100], Value::Null)),
        "id-rung-std": run(tbl(&[2000, 2000, 2000, 2000], json!("max_cases")), tbl(&[2000, 2000, 2000, 2000, 2000], Value::Null)),
        "bfi-default-std": run(tbl(&[300, 300, 300, 300], json!("max_cases")), tbl(&[300, 300, 300, 300, 300], Value::Null)),
        "bfi-rung-std": run(tbl(&[700, 700, 700, 700], json!("max_cases")), tbl(&[50, 50, 50, 50, 50], Value::Null)),
        "isp_rev-default-std": run(tbl(&[10, 10], json!("budget_wall")), tbl(&[10], Value::Null)),
        "isp_rev-rung-std": run(tbl(&[5000, 5000, 5000], Value::Null), tbl(&[5000], Value::Null)),
    });
    let cfg = json!({"schema":"rustred.campaign.genchoice.v1","program":"rr_node",
        "nodes":[{"node":"toy","family":fam.display().to_string(),"family_sha256":sha256_file(&fam),
                  "box":{"path":bx.display().to_string(),"sha256":sha256_file(&bx)}}],
        "policy":{"orders":["id","bfi","isp_rev"],"probe":{"cases":50},"first":{"by":"best_size"},
                  "economy":{"optimise":optimise},
                  "job":{"cap_bytes":268435456u64,"expected_peak_bytes":67108864u64,"full_cap_bytes":268435456u64,"full_expected_peak_bytes":67108864u64,"heartbeat_s":1,"stop_grace_s":3}},
        "fake":{"program":"rr_fake_child","ms":5,"runs":runs}});
    let p = dir.join("genchoice.json");
    std::fs::write(&p, serde_json::to_vec_pretty(&cfg).unwrap()).unwrap();
    p
}

fn setup(tag: &str, optimise: bool) -> (Tmp, PathBuf, PathBuf) {
    setup_with(tag, optimise, None)
}

/// `full_cap`: replaces the policy's full-run cap (bytes).
fn setup_with(tag: &str, optimise: bool, full_cap: Option<u64>) -> (Tmp, PathBuf, PathBuf) {
    let t = Tmp::new(tag);
    let cfg = material(&t.path.join("material"), optimise);
    if let Some(c) = full_cap {
        let mut v: Value = serde_json::from_slice(&std::fs::read(&cfg).unwrap()).unwrap();
        v["policy"]["job"]["full_cap_bytes"] = json!(c);
        std::fs::write(&cfg, serde_json::to_vec_pretty(&v).unwrap()).unwrap();
    }
    let spec = json!({
        "schema": "rustred.campaign.spec.v1", "campaign": t.campaign,
        "programs": {"rr_fake_child": {"path": FAKE}, "rr_node": {"path": FAKE}},
        "resources": {"policy": "plain", "cpus": cpus(), "max_jobs": 4, "mem_total": "2G", "disk_floor": "1G",
                      "grace_seconds": 5, "sampler_ms": 300, "host_floor": "100M", "heartbeat_timeout_s": 60,
                      "start_timeout_s": 120},
        "progress": {"cadence_seconds": 1},
        "inputs": [{"id": "genchoice-config", "role": "genchoice", "path": cfg.display().to_string(),
                    "blake3": blake3_file(&cfg)}]
    });
    let sp = write_spec(&t, "spec.json", &spec);
    let cdir = t.cdir("cdir");
    (t, sp, cdir)
}

fn read_json(p: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display())))
        .unwrap()
}

fn cand<'a>(c: &'a Value, id: &str) -> &'a Value {
    c["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == id)
        .unwrap_or_else(|| panic!("candidate {id}: {c:#}"))
}

#[test]
fn portfolio_ranks_probes_runs_economy_and_records_the_choice() {
    let _g = serial();
    let (_t, sp, cdir) = setup("gcflow", false);
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&cdir));
    let c = read_json(&cdir.join("genchoice/toy/choice.json"));
    // ranking: status first (the wall-stopped probe last), then the use-cost band, then the largest rule
    assert_eq!(
        c["ranking"],
        json!([
            "bfi-default-std",
            "bfi-rung-std",
            "id-rung-std",
            "isp_rev-rung-std",
            "id-default-std",
            "isp_rev-default-std"
        ]),
        "{c:#}"
    );
    // the first full run is the best probe; the economy alternatives are the next two (the completed probe is not re-run)
    assert_eq!(c["economy"]["first_acceptable"], 2, "{c:#}");
    let alts: Vec<u64> = c["economy"]["alternatives"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["cand"].as_u64().unwrap())
        .collect();
    assert_eq!(alts, vec![1, 3], "id-rung (1) and bfi-rung (3): {c:#}");
    // the smallest acceptable table wins: found by the economy step
    assert_eq!(c["winner"], "bfi-rung-std", "{c:#}");
    assert_eq!(cand(&c, "bfi-rung-std")["verdict"], "winner");
    assert_eq!(
        cand(&c, "bfi-default-std")["verdict"],
        "acceptable, lost the choice"
    );
    assert_eq!(
        cand(&c, "isp_rev-rung-std")["verdict"],
        "acceptable, lost the choice"
    );
    assert_eq!(
        cand(&c, "id-default-std")["verdict"],
        "ranked, not run (economy / first acceptable found earlier)"
    );
    assert_eq!(cand(&c, "isp_rev-default-std")["probe"]["status"], "heavy");
    // every alternative carried its wall cap (factor x C1 / 2, at least 1 s)
    let alt_full = &cand(&c, "bfi-rung-std")["full"][0];
    assert_eq!(alt_full["alternative"], true);
    assert!(alt_full["wall_job_s_cap"].as_u64().unwrap() >= 1);
    // orders: comparator-only priorities from the family alone; bfi voted over the base probe
    let cands = read_json(&cdir.join("genchoice/toy/candidates.json"));
    assert_eq!(cands["shape"]["lines"], json!([0, 1]));
    assert_eq!(cands["bfi_base"], "id-default-std");
    assert_eq!(cands["bfi_cases_voted"], 4);
    let o: Value =
        serde_json::from_str(cands["orders"]["isp_rev-default-std"].as_str().unwrap()).unwrap();
    assert_eq!(o["coordinate_priority"], json!([0, 1, 2]));
    // the per-rule comparison names the same cases in the losing tables
    let pr = std::fs::read_to_string(cdir.join("genchoice/toy/per_rule.jsonl")).unwrap();
    let first: Value = serde_json::from_str(pr.lines().next().unwrap()).unwrap();
    assert_eq!(first["winner"], 50);
    assert_eq!(first["others"]["bfi-default-std:full"], 300);
    assert_eq!(first["others"]["id-default-std:probe"], 100);
    // the import spec of the winner for the closing loop
    let imp = read_json(&cdir.join("genchoice/toy/import.json"));
    assert_eq!(imp["sources"][0]["kind"], "engine_table");
    assert_eq!(imp["sources"][0]["table_terminals"], "ledger_and_residuals");
    let tp = imp["sources"][0]["path"].as_str().unwrap();
    assert_eq!(imp["sources"][0]["sha256"], sha256_file(Path::new(tp)));
    assert_eq!(c["winner_table"].as_str().unwrap(), tp);
    // TRICKS 31 / 35: the winner table's rules carry their effective derivation (one line per rule)
    let dref = &c["files"]["derivations"];
    assert!(
        dref["path"]
            .as_str()
            .unwrap()
            .ends_with("derivations.jsonl"),
        "{dref:#}"
    );
    // resume answers the same: a second start finds everything decided and exits 0 at once
    let again = run_to_end(&sp, &cdir, &["--plain"]);
    assert_eq!(again.last(), Some(&0), "{}", log_tail(&cdir));
    let c2 = read_json(&cdir.join("genchoice/toy/choice.json"));
    assert_eq!(c2, c);
}

#[test]
fn optimise_mode_runs_every_ranked_candidate_without_cap() {
    let _g = serial();
    let (_t, sp, cdir) = setup("gcopt", true);
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&cdir));
    let c = read_json(&cdir.join("genchoice/toy/choice.json"));
    assert_eq!(c["economy"]["optimise"], true);
    let alts: Vec<u64> = c["economy"]["alternatives"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["cand"].as_u64().unwrap())
        .collect();
    // every ranked candidate after the first acceptable that was not run already (isp_rev-rung completed as a probe),
    // the heavy probe included
    assert_eq!(alts, vec![0, 1, 3, 4], "{c:#}");
    assert!(c["economy"]["budget_cpu_ms"].is_null());
    for a in c["candidates"].as_array().unwrap() {
        for f in a["full"].as_array().unwrap() {
            assert!(
                f["wall_job_s_cap"].is_null(),
                "no cap in optimise mode: {f:#}"
            );
        }
    }
    assert_eq!(
        c["winner"], "isp_rev-default-std",
        "the smallest full table of all: {c:#}"
    );
}

#[test]
fn a_job_cap_above_the_campaign_memory_total_is_refused_at_start_not_waited_for() {
    let _g = serial();
    let (_t, sp, cdir) = setup_with("gccap", false, Some(16 << 30));
    let mut c = start(&sp, &cdir, &["--plain"], &[]);
    let code = wait(&mut c, std::time::Duration::from_secs(120));
    assert_ne!(code, 0);
    let log = log_tail(&cdir);
    assert!(log.contains("exceeds the campaign memory total"), "{log}");
}

/// POLE OBJECTIVE (interfaces/genchoice.md 6) in the decision step: optimise mode gives every candidate a full table;
/// table-scope pole scores make the regular `bfi-rung-std` win over the smallest `isp_rev-default-std` (order 2) under an
/// unbounded tolerance; with `poles.use = false` the choice and its record are those of the size choice, with nothing
/// about poles written.
#[test]
fn pole_scores_choose_the_regular_table_and_the_ignore_switch_restores_the_size_choice() {
    let _g = serial();
    for (tag, poles, want) in [
        (
            "gcpoles",
            json!({"max_size_ratio": "unbounded"}),
            "bfi-rung-std",
        ),
        ("gcpolesoff", json!({"use": false}), "isp_rev-default-std"),
    ] {
        let t = Tmp::new(tag);
        let m = t.path.join("material");
        let cfg = material(&m, true);
        let sc = |id: &str, max: u32| {
            json!({"schema":"rustred.campaign.pole-score.v1","node":"toy","scope":"table","cand":id,
                "score":{"keys":100,"keys_with_pole":u64::from(max) * 7,"undecided":0,"max_order":max,"composed_max_order":null}})
        };
        let scores = m.join("scores.jsonl");
        std::fs::write(
            &scores,
            [
                sc("isp_rev-default-std", 2),
                sc("bfi-rung-std", 0),
                sc("id-rung-std", 1),
            ]
            .iter()
            .map(|l| format!("{l}\n"))
            .collect::<String>(),
        )
        .unwrap();
        let mut v: Value = serde_json::from_slice(&std::fs::read(&cfg).unwrap()).unwrap();
        v["nodes"][0]["pole_scores"] =
            json!({"path": scores.display().to_string(), "sha256": sha256_file(&scores)});
        v["policy"]["poles"] = poles;
        std::fs::write(&cfg, serde_json::to_vec_pretty(&v).unwrap()).unwrap();
        let spec = json!({
            "schema": "rustred.campaign.spec.v1", "campaign": t.campaign,
            "programs": {"rr_fake_child": {"path": FAKE}, "rr_node": {"path": FAKE}},
            "resources": {"policy": "plain", "cpus": cpus(), "max_jobs": 4, "mem_total": "2G", "disk_floor": "1G",
                          "grace_seconds": 5, "sampler_ms": 300, "host_floor": "100M", "heartbeat_timeout_s": 60,
                          "start_timeout_s": 120},
            "progress": {"cadence_seconds": 1},
            "inputs": [{"id": "genchoice-config", "role": "genchoice", "path": cfg.display().to_string(),
                        "blake3": blake3_file(&cfg)}]
        });
        let sp = write_spec(&t, "spec.json", &spec);
        let cdir = t.cdir("cdir");
        let codes = run_to_end(&sp, &cdir, &["--plain"]);
        assert_eq!(codes.last(), Some(&0), "{}", log_tail(&cdir));
        let c = read_json(&cdir.join("genchoice/toy/choice.json"));
        assert_eq!(c["winner"], want, "{tag}: {:#}", c["poles"]);
        assert_eq!(cand(&c, want)["verdict"], "winner");
        if tag == "gcpoles" {
            let d = &c["poles"]["decision"];
            assert_eq!(
                (d["reason"].clone(), d["changed"].clone()),
                (json!("regular"), json!(true)),
                "{d:#}"
            );
            assert_eq!(
                (
                    d["score_form"].clone(),
                    c["policy"]["poles"]["score"].clone()
                ),
                (json!("count"), json!("count"))
            );
            assert_eq!(
                (d["score_level"].clone(), d["claim"].clone()),
                (json!("rule"), json!("screen"))
            );
            assert_eq!(c["poles"]["dependences"], "upper_bound");
            assert!(
                c["poles"]["scores"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|s| s["score"] == "undecided"),
                "unscored acceptable tables are recorded as undecided"
            );
        } else {
            assert!(
                c.get("poles").is_none(),
                "ignored: nothing about poles in the choice"
            );
            assert!(
                c["policy"].get("poles").is_none(),
                "ignored: the effective policy as before"
            );
        }
    }
}
