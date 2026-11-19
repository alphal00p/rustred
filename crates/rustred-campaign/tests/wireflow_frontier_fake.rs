//! Lane wireflow (b), interfaces/closeloop.md 26: FACTS FROM THE FRONTIER under a REAL `rr_campaign` supervisor with
//! `rr_fake_child` (fake rr_proof facts; test data only). Node FR (arity 3, chart 110): the start key [5,1,0] runs down
//! the chain A = [k,1,0] (k = 5..1); every A rule also has a term into region B = [m,1,-1] (m = 4..1, its own chain)
//! whose coefficient fact is ZERO. The keys-only superset is A + B (9 keys); the exact closure is A (5 keys).
//! - all-at-once (no setting): the facts of all 9 superset keys are asked;
//! - frontier mode: per round only the frontier (one key of A per round) is asked: 5 pairs, the SAME verdict and
//!   statement; with the prefetch the 8 outside-box superset keys go with round 1 (then the fixed point);
//! - the FACTS BUDGET: a frontier that does not fit ends the node typed (exit 8, the frontier saved, no final verdict);
//!   `rr_campaign control CDIR raise-facts-budget` resumes the same campaign to the same statement;
//! - an interrupted frontier run resumes to the same statement.
//!
//! Spawns processes: run ONLY inside a private pid namespace
//! (`LIGHT_PIDNS=1 .../light_build.sh wireflow test -p rustred-campaign --test wireflow_frontier_fake`).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

mod closeloop_support;
mod sup_common;

use closeloop_support::{blake3_file, recipe, rule, sha256_file, write_table};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;
use sup_common::*;

/// The material of node FR (shared by the runs that compare statements).
fn material(dir: &Path) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    std::fs::create_dir_all(dir).unwrap();
    let k = |a: i32, b: i32, c: i32| format!("I({a},{b},{c})");
    let mut rules = vec![];
    let mut oracle = vec![];
    let term = |j: usize, key: [i32; 3], zero: bool| json!({"term": j, "key": key, "coef": if zero { "zero" } else { "nonzero" }});
    let mut idx = 0i64;
    // chain A: [k,1,0] -> [k-1,1,0] (nonzero) + [k-1,1,-1] (ZERO: region B only through zero terms)
    for a in 2..=5 {
        rules.push(rule(
            idx,
            [Some(a as i64), Some(1), Some(0)],
            &[&k(a - 1, 1, 0), &k(a - 1, 1, -1)],
        ));
        idx += 1;
        oracle.push(
            json!({"key": [a, 1, 0], "line": {"rhs": [term(0, [a - 1, 1, 0], false),
            term(1, [a - 1, 1, -1], true)]}}),
        );
    }
    rules.push(rule(
        idx,
        [Some(1), Some(1), Some(0)],
        &[&k(0, 1, 0), &k(1, 0, 0)],
    ));
    idx += 1;
    oracle.push(
        json!({"key": [1, 1, 0], "line": {"rhs": [term(0, [0, 1, 0], false),
        term(1, [1, 0, 0], false)]}}),
    );
    // region B: [m,1,-1] -> [m-1,1,-1]
    for m in 2..=4 {
        rules.push(rule(
            idx,
            [Some(m as i64), Some(1), Some(-1)],
            &[&k(m - 1, 1, -1)],
        ));
        idx += 1;
        oracle.push(json!({"key": [m, 1, -1], "line": {"rhs": [term(0, [m - 1, 1, -1], false)]}}));
    }
    rules.push(rule(idx, [Some(1), Some(1), Some(-1)], &[&k(0, 1, -1)]));
    oracle.push(json!({"key": [1, 1, -1], "line": {"rhs": [term(0, [0, 1, -1], false)]}}));
    let a = write_table(dir, "A.table.jsonl", &rules);
    let st = dir.join("start.jsonl");
    std::fs::write(&st, "[5,1,0]\n").unwrap();
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"fr","frame_from":"A","bits":6,
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "required_evidence":"imported","policies":{"tier":"C"},
        "sources":[{"id":"A","kind":"engine_table","class":"base","path": a.display().to_string(),
                    "sha256": sha256_file(&a)}]});
    let import_spec = dir.join("import-spec.json");
    std::fs::write(&import_spec, serde_json::to_vec_pretty(&spec).unwrap()).unwrap();
    let po = dir.join("proof-oracle.jsonl");
    std::fs::write(
        &po,
        oracle.iter().map(|v| format!("{v}\n")).collect::<String>(),
    )
    .unwrap();
    let op = dir.join("oracle.jsonl");
    std::fs::write(&op, "").unwrap();
    let family = dir.join("FR.family");
    std::fs::write(&family, "toy family (the fake child never reads it)\n").unwrap();
    (import_spec, po, op, family)
}

fn config(dir: &Path, mat: &Path, frontier: Option<Value>, ms: u64) -> PathBuf {
    let (import_spec, po, op, family) = (
        mat.join("import-spec.json"),
        mat.join("proof-oracle.jsonl"),
        mat.join("oracle.jsonl"),
        mat.join("FR.family"),
    );
    let mut node = json!({
        "node":"fr", "import_spec": import_spec.display().to_string(), "target":"boxed",
        "check_threads": 2, "default_recipe": null, "exact_tier": true,
        "budgets": {"max_rounds": 20, "max_job_attempts": 2},
        "solve": {"program": "rr_fake_child", "family": family.display().to_string(), "order": "fr-order",
                  "relation_rule": "first_admissible", "max_keys_per_job": 2, "threads": 1, "cap_bytes": 268435456,
                  "expected_peak_bytes": 67108864, "heartbeat_s": 1, "stop_grace_s": 3,
                  "fake": {"oracle": op.display().to_string(), "ms": 5, "program": "rr_fake_child"}},
        "proof": {"program": "rr_proof", "rows": "engine", "family": family.display().to_string(),
                  "max_keys_per_job": 4, "max_fact_keys_per_job": 4, "wall_s": 60, "cap_bytes": 268435456,
                  "expected_peak_bytes": 67108864, "import_requests": false, "relations": false,
                  "fake": {"oracle": po.display().to_string(), "ms": ms, "program": "rr_fake_child"}},
        // test data: the zero terms are taken on the first route (the second route is not under test here)
        "second_route": {"decisive_zeros": false, "elided": false, "all_zeros": "off"}});
    if let Some(f) = frontier {
        node["facts_frontier"] = f;
    }
    std::fs::create_dir_all(dir).unwrap();
    let p = dir.join("closeloop.json");
    std::fs::write(
        &p,
        serde_json::to_vec_pretty(
            &json!({"schema":"rustred.campaign.closeloop.v1","nodes":[node]}),
        )
        .unwrap(),
    )
    .unwrap();
    p
}

fn setup(tag: &str, mat: &Path, frontier: Option<Value>, ms: u64) -> (Tmp, PathBuf, PathBuf) {
    let t = Tmp::new(tag);
    let cfg = config(&t.path.join("cfg"), mat, frontier, ms);
    let spec = json!({
        "schema": "rustred.campaign.spec.v1", "campaign": t.campaign,
        "programs": {"rr_fake_child": {"path": FAKE}},
        "resources": {"policy": "plain", "cpus": cpus(), "max_jobs": 4, "mem_total": "2G", "disk_floor": "1G",
                      "grace_seconds": 5, "sampler_ms": 300, "host_floor": "100M", "heartbeat_timeout_s": 60,
                      "start_timeout_s": 120},
        "progress": {"cadence_seconds": 1},
        "inputs": [{"id": "closeloop-config", "role": "closeloop", "path": cfg.display().to_string(),
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

/// The check records of the node in round order.
fn checks(cdir: &Path) -> Vec<Value> {
    let mut v = vec![];
    for k in 1..=40 {
        let p = cdir.join(format!("closeloop/fr/checks/r{k}.json"));
        if !p.is_file() {
            break;
        }
        v.push(read_json(&p));
    }
    v
}

fn statement(cdir: &Path) -> Vec<u8> {
    std::fs::read(cdir.join("closeloop/fr/final/statement/statement.jsonl"))
        .unwrap_or_else(|e| panic!("statement: {e} {}", log_tail(cdir)))
}

fn result(cdir: &Path) -> Value {
    read_json(&cdir.join("closeloop/fr/final/result.json"))
}

/// Fact pairs the node's fact jobs asked, over every round.
fn pairs_asked(cdir: &Path) -> u64 {
    checks(cdir)
        .iter()
        .map(|c| c["proof_batches"]["facts"].as_u64().unwrap_or(0))
        .sum()
}

fn reference(mat: &Path) -> (Tmp, PathBuf) {
    let (t, sp, cdir) = setup("wffref", mat, None, 5);
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{codes:?} {}", log_tail(&cdir));
    (t, cdir)
}

#[test]
fn the_frontier_mode_asks_only_the_exact_closure_and_gives_the_all_at_once_statement() {
    let _g = serial();
    let m = Tmp::new("wffmat");
    let mat = m.path.join("material");
    material(&mat);
    let (_ta, ca) = reference(&mat);
    let ra = result(&ca);
    assert_eq!(ra["verdict"], "closed", "{ra:#}");
    // all at once: the 9 superset keys' facts
    assert_eq!(pairs_asked(&ca), 9, "{:#}", checks(&ca)[0]);
    assert!(checks(&ca).iter().all(|c| c.get("frontier").is_none()));
    let (_tb, spb, cb) = setup(
        "wfffr",
        &mat,
        Some(json!({"max_pairs": 100, "max_jobs": 100, "prefetch_outside": false})),
        5,
    );
    let codes = run_to_end(&spb, &cb, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{codes:?} {}", log_tail(&cb));
    let rb = result(&cb);
    assert_eq!(rb["verdict"], ra["verdict"]);
    assert_eq!(rb["keys"], ra["keys"], "{rb:#}");
    assert!(statement(&cb) == statement(&ca), "statements differ");
    // the frontier: one key of chain A per round, then the fixed point
    let cs = checks(&cb);
    let fr: Vec<&Value> = cs.iter().map(|c| &c["frontier"]).collect();
    assert_eq!(pairs_asked(&cb), 5, "{fr:#?}");
    for (i, f) in fr.iter().enumerate().take(5) {
        assert_eq!(f["pairs_requested"], 1, "round {}: {f:#}", i + 1);
        assert_eq!(f["frontier_keys"], 1, "round {}: {f:#}", i + 1);
        assert_eq!(f["closure_keys"], (i + 1) as u64, "round {}: {f:#}", i + 1);
        assert_eq!(f["end"], "continue");
    }
    let last = fr.last().unwrap();
    assert_eq!(last["end"], "fixed_point", "{last:#}");
    assert_eq!(last["frontier_keys"], 0);
    assert_eq!(last["closure_keys"], 5);
    assert_eq!(last["zero_terms_dropped"], 4, "{last:#}");
    assert_eq!(last["budget"]["pairs_used_before"], 5);
    assert_eq!(verify(&cb), 0);
}

#[test]
fn the_prefetch_asks_the_outside_box_superset_keys_with_round_one() {
    let _g = serial();
    let m = Tmp::new("wffmat2");
    let mat = m.path.join("material");
    material(&mat);
    let (_ta, ca) = reference(&mat);
    let (_tb, spb, cb) = setup(
        "wffpf",
        &mat,
        Some(json!({"max_pairs": 100, "max_jobs": 100})),
        5,
    );
    let codes = run_to_end(&spb, &cb, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{codes:?} {}", log_tail(&cb));
    assert!(statement(&cb) == statement(&ca), "statements differ");
    let cs = checks(&cb);
    let f1 = &cs[0]["frontier"];
    assert_eq!(f1["prefetch"]["considered"], true, "{f1:#}");
    assert_eq!(f1["prefetch"]["superset_keys"], 9, "{f1:#}");
    assert_eq!(f1["prefetch"]["outside_pairs"], 8, "{f1:#}");
    assert_eq!(f1["prefetch"]["issued"], true, "{f1:#}");
    assert_eq!(f1["pairs_requested"], 9);
    assert_eq!(cs.len(), 2, "round 2 is the fixed point");
    assert_eq!(cs[1]["frontier"]["end"], "fixed_point");
    assert_eq!(cs[1]["frontier"]["prefetch"]["considered"], false);
    // a budget the prefetch does not fit: skipped (recorded), the frontier alone is asked
    let (_tc, spc, cc) = setup(
        "wffpf2",
        &mat,
        Some(json!({"max_pairs": 6, "max_jobs": 100})),
        5,
    );
    let codes = run_to_end(&spc, &cc, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{codes:?} {}", log_tail(&cc));
    let f1 = &checks(&cc)[0]["frontier"];
    assert_eq!(f1["prefetch"]["issued"], false, "{f1:#}");
    assert_eq!(f1["prefetch"]["skipped"], "facts budget", "{f1:#}");
    assert_eq!(f1["pairs_requested"], 1);
    assert!(statement(&cc) == statement(&ca), "statements differ");
    assert_eq!(pairs_asked(&cc), 5);
}

#[test]
fn the_facts_budget_ends_typed_and_a_raise_resumes_to_the_same_statement() {
    let _g = serial();
    let m = Tmp::new("wffmat3");
    let mat = m.path.join("material");
    material(&mat);
    let (_ta, ca) = reference(&mat);
    let (_tb, spb, cb) = setup(
        "wffbud",
        &mat,
        Some(json!({"max_pairs": 3, "max_jobs": 10, "prefetch_outside": false})),
        5,
    );
    let mut c = start(&spb, &cb, &["--plain"], &[]);
    let code = wait(&mut c, Duration::from_secs(900));
    assert_eq!(code, ATTENTION, "{}", log_tail(&cb));
    // typed end: no final verdict, the frontier saved (one pair: [4-th key of A] = [2,1,0])
    assert!(!cb.join("closeloop/fr/final/result.json").exists());
    let st = state(&cb);
    assert!(
        st.inputs.contains_key("closeloop:fr:facts-budget:r4"),
        "{:?}",
        st.inputs.keys().collect::<Vec<_>>()
    );
    assert!(st.inputs.contains_key("closeloop:fr:facts-frontier:r4"));
    let rec = read_json(&cb.join("closeloop/fr/frontier/facts-budget.r4.json"));
    assert_eq!(rec["end"], "facts budget", "{rec:#}");
    assert_eq!(rec["wanted"]["pairs"], 1);
    assert_eq!(rec["budget"]["max_pairs"], 3);
    assert_eq!(rec["budget"]["pairs_used_before"], 3);
    let pairs =
        std::fs::read_to_string(cb.join("closeloop/fr/frontier/facts-frontier.r4.jsonl")).unwrap();
    let lines: Vec<Value> = pairs
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["key"], json!([2, 1, 0]));
    assert_eq!(checks(&cb)[3]["frontier"]["end"], "facts_budget");
    // a second session without a raise stays where it is
    let mut c = start(&spb, &cb, &["--plain"], &[]);
    assert_eq!(wait(&mut c, Duration::from_secs(300)), ATTENTION);
    assert!(!cb.join("closeloop/fr/checks/r5.json").exists());
    // the operator's raise: the same campaign resumes to the reference statement
    let sup = env!("CARGO_BIN_EXE_rr_campaign");
    let out = std::process::Command::new(sup)
        .args([
            "control",
            cb.to_str().unwrap(),
            "raise-facts-budget",
            "--node",
            "fr",
            "--max-pairs",
            "10",
            "--max-jobs",
            "10",
            "--reason",
            "test",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let codes = run_to_end(&spb, &cb, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{codes:?} {}", log_tail(&cb));
    assert_eq!(result(&cb)["verdict"], "closed");
    assert!(statement(&cb) == statement(&ca), "statements differ");
    let last = checks(&cb).last().unwrap()["frontier"].clone();
    assert_eq!(last["budget"]["max_pairs"], 10, "{last:#}");
    assert!(last["budget"]["raised_by"].is_string(), "{last:#}");
    assert_eq!(pairs_asked(&cb), 5);
    assert_eq!(verify(&cb), 0);
}

#[test]
fn an_interrupted_frontier_run_resumes_to_the_same_statement() {
    let _g = serial();
    let m = Tmp::new("wffmat4");
    let mat = m.path.join("material");
    material(&mat);
    let (_ta, ca) = reference(&mat);
    let fr = json!({"max_pairs": 100, "max_jobs": 100, "prefetch_outside": false});
    let (_tb, spb, cb) = setup("wffint", &mat, Some(fr), 300);
    let mut x = 29u64;
    let mut codes = vec![];
    for round in 0..16 {
        let mut c = start(&spb, &cb, &["--plain"], &[]);
        if round < 5 {
            x = mix(x);
            std::thread::sleep(Duration::from_millis(1200 + x % 2500));
            signal(&c, libc::SIGTERM);
        }
        let code = wait(&mut c, Duration::from_secs(900));
        codes.push(code);
        if code == 0 {
            break;
        }
    }
    assert_eq!(codes.last(), Some(&0), "{codes:?} {}", log_tail(&cb));
    assert!(codes.len() > 1, "no interrupt happened: {codes:?}");
    assert!(statement(&cb) == statement(&ca), "statements differ");
    assert_eq!(pairs_asked(&cb), 5);
    assert_eq!(verify(&cb), 0);
}

/// The FR material plus face R2 = [k,2,0] (a recipe): the start key [3,2,0] is uncovered by tables and served by SLOW
/// recipe solves (the test sets the solve fake's ms) whose relations reach chain A ([k,1,0]).
fn material_with_solves(dir: &Path) {
    material(dir);
    let r2 = recipe(dir, "R2.recipe.json", [None, Some(2), Some(0)]);
    std::fs::write(dir.join("start.jsonl"), "[5,1,0]\n[3,2,0]\n").unwrap();
    let sp = dir.join("import-spec.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(&sp).unwrap()).unwrap();
    v["sources"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id": "R2", "kind": "recipe", "class": "recipe",
        "path": r2.display().to_string(), "sha256": sha256_file(&r2)}));
    std::fs::write(&sp, serde_json::to_vec_pretty(&v).unwrap()).unwrap();
    let mut o = String::new();
    for k in 1..=3 {
        let rhs = if k > 1 {
            json!([[k - 1, 2, 0], [k, 1, 0]])
        } else {
            json!([[0, 2, 0], [1, 1, 0]])
        };
        o.push_str(&format!(
            "{}\n",
            json!({"source": "R2", "key": [k, 2, 0], "verdict": "SOLVED", "rhs": rhs})
        ));
    }
    std::fs::write(dir.join("oracle.jsonl"), o).unwrap();
}

#[test]
fn the_frontier_advances_while_slow_solves_run_and_ends_where_all_at_once_ends() {
    let _g = serial();
    let m = Tmp::new("wffmat5");
    let mat = m.path.join("material");
    material_with_solves(&mat);
    let patch = |cdir_cfg: &Path| {
        // slow solves: the solve fake's ms
        let p = cdir_cfg.join("closeloop.json");
        let mut v: Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
        v["nodes"][0]["solve"]["fake"]["ms"] = json!(2500);
        v["nodes"][0]["solve"]["max_keys_per_job"] = json!(1);
        // our recipe relations are per-key only with their bound proofs (fake span answers: exact in span)
        v["nodes"][0]["proof"]["relations"] = json!(true);
        std::fs::write(&p, serde_json::to_vec_pretty(&v).unwrap()).unwrap();
    };
    let run = |tag: &str, fr: Option<Value>| -> (Tmp, PathBuf) {
        let t = Tmp::new(tag);
        let cfg_dir = t.path.join("cfg");
        let cfg = config(&cfg_dir, &mat, fr, 5);
        patch(&cfg_dir);
        let spec = json!({
            "schema": "rustred.campaign.spec.v1", "campaign": t.campaign,
            "programs": {"rr_fake_child": {"path": FAKE}},
            "resources": {"policy": "plain", "cpus": cpus(), "max_jobs": 4, "mem_total": "2G", "disk_floor": "1G",
                          "grace_seconds": 5, "sampler_ms": 300, "host_floor": "100M", "heartbeat_timeout_s": 60,
                          "start_timeout_s": 120},
            "progress": {"cadence_seconds": 1},
            "inputs": [{"id": "closeloop-config", "role": "closeloop", "path": cfg.display().to_string(),
                        "blake3": blake3_file(&cfg)}]
        });
        let sp = write_spec(&t, "spec.json", &spec);
        let cdir = t.cdir("cdir");
        let codes = run_to_end(&sp, &cdir, &["--plain"]);
        assert_eq!(codes.last(), Some(&0), "{codes:?} {}", log_tail(&cdir));
        (t, cdir)
    };
    let (_ta, ca) = run("wffslow0", None);
    let (_tb, cb) = run(
        "wffslow1",
        Some(json!({"max_pairs": 100, "max_jobs": 100, "prefetch_outside": false})),
    );
    assert_eq!(result(&ca)["verdict"], "closed", "{:#}", result(&ca));
    assert_eq!(result(&cb)["verdict"], "closed", "{:#}", result(&cb));
    assert!(statement(&cb) == statement(&ca), "statements differ");
    // at least one frontier pass ran while a solve was in flight; no final was written before the solves ended
    let cs = checks(&cb);
    assert!(
        cs.iter()
            .any(|c| c["frontier"]["solves_in_flight"].as_u64().unwrap_or(0) > 0),
        "{:#?}",
        cs.iter().map(|c| &c["frontier"]).collect::<Vec<_>>()
    );
    assert_eq!(cs.last().unwrap()["frontier"]["solves_in_flight"], 0);
    assert_eq!(verify(&cb), 0);
}
