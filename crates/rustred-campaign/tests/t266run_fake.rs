//! Lane t266run (interfaces/closeloop.md 17): the closing loop's OWN escalation of uncovered keys (symmetry relation
//! at the key in-process, per-key point recipes of a step sequence, terminal PROPOSALS under the spec's finite-terminal
//! rule) and the big-node mode (persistent table index, evidence stores for the spec's evidence and for the node's own
//! fact / proof answers, streamed keys-only pass), under a REAL `rr_campaign` supervisor with `rr_fake_child`.
//! Spawns processes and sends signals: run ONLY inside a private pid namespace
//! (`LIGHT_PIDNS=1 .../light_build.sh t266run test -p rustred-campaign --test t266run_fake`).
//!
//! The node is the TOY of closeloop_support (arity 3) WITHOUT its recipe sources: the start key's table rule hands
//! [top,1,0] down; every key [k,1,0] below is uncovered and goes through the escalation; the fake child solves the
//! first step at k >= 2 and nothing at [1,1,0], which ends as the one terminal proposal.

mod closeloop_support;
mod sup_common;

use closeloop_support::*;
use rustred_campaign::closeloop::derived::recipe_source_id;
use rustred_campaign::state::NodePhase;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;
use sup_common::*;

const STEPS: [&str; 2] = ["d0_cnone_foff_mM1C", "d1_cnone_foff_mM1C"];

/// The toy without recipe sources, with the escalation (and optionally the big-node mode) configured.
fn setup(tag: &str, top: i32, max_terminals: Option<usize>, scale: bool, ms: u64) -> (Tmp, PathBuf, PathBuf) {
    let t = Tmp::new(tag);
    let cfg = Toy {
        top,
        second_face: None,
        default_recipe: false,
        r1_from_two: false,
        table_evidence: !scale,
        ms,
        ..Toy::default()
    };
    let files = toy(&t.path.join("material"), &cfg, FAKE);
    std::fs::write(&files.hand, "").unwrap();
    // the import spec without the recipe sources R1 / R2
    let mut spec: Value = serde_json::from_slice(&std::fs::read(&files.import_spec).unwrap()).unwrap();
    let srcs: Vec<Value> = spec["sources"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["kind"] != "recipe")
        .cloned()
        .collect();
    spec["sources"] = json!(srcs);
    std::fs::write(&files.import_spec, serde_json::to_vec_pretty(&spec).unwrap()).unwrap();
    // the oracle: the first step solves [k,1,0] for k >= 2, nothing solves [1,1,0]
    let op = files.dir.join("oracle.jsonl");
    let mut o = String::new();
    for k in 2..=top {
        o.push_str(&format!(
            "{}\n",
            json!({"source": recipe_source_id(&[k, 1, 0], STEPS[0]), "key": [k, 1, 0], "verdict": "SOLVED",
                   "rhs": [[k - 1, 1, 0], [k, 0, 0]]})
        ));
    }
    std::fs::write(&op, o).unwrap();
    let mut c: Value = serde_json::from_slice(&std::fs::read(&files.config).unwrap()).unwrap();
    let node = &mut c["nodes"][0];
    node["hand"] = Value::Null;
    node["escalation"] = json!({"symmetry_at_key": true, "steps": STEPS,
        "terminals": max_terminals.map(|m| json!({"max_terminals": m}))});
    if scale {
        node["scale"] = json!({"store_budget_mib": 1, "store_chunk_files": 2});
    }
    node["budgets"]["max_rounds"] = json!(40);
    std::fs::write(&files.config, serde_json::to_vec_pretty(&c).unwrap()).unwrap();
    let spec = campaign_spec(&t.campaign, &files, FAKE, &cpus());
    let sp = write_spec(&t, "spec.json", &spec);
    let cdir = t.cdir("cdir");
    (t, sp, cdir)
}

fn result_json(cdir: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(cdir.join("closeloop/toy/final/result.json")).unwrap())
        .unwrap()
}

/// Inputs of a role: (id, file bytes).
fn role_inputs(cdir: &Path, role: &str) -> Vec<(String, Vec<u8>)> {
    let st = state(cdir);
    let mut v: Vec<_> = st
        .inputs
        .iter()
        .filter(|(_, r)| r.role == role)
        .map(|(id, r)| (r.registered_seq, id.clone(), std::fs::read(&r.path).unwrap()))
        .collect();
    v.sort();
    v.into_iter().map(|(_, id, b)| (id, b)).collect()
}

/// The decisions of a run: every item (id, kind, phase) and the escalation records.
fn decisions(cdir: &Path) -> (BTreeSet<(String, String, String)>, Vec<(String, Vec<u8>)>) {
    let items = state(cdir)
        .items
        .iter()
        .map(|(k, it)| (k.clone(), it.kind.clone(), it.phase.tag().to_string()))
        .collect();
    let mut esc = vec![];
    for role in ["closeloop_symrel", "closeloop_recipes", "closeloop_terminals"] {
        esc.extend(role_inputs(cdir, role));
    }
    (items, esc)
}

fn terminals_of(cdir: &Path) -> Value {
    serde_json::from_slice(
        &std::fs::read(cdir.join("closeloop/toy/final/statement/terminals.json")).unwrap(),
    )
    .unwrap()
}

#[test]
fn escalation_and_a_terminal_proposal_close_the_node_boxed_per_key() {
    let _g = serial();
    let (_t, sp, cdir) = setup("t2ok", 3, Some(2), true, 20);
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&cdir));
    let r = result_json(&cdir);
    assert_eq!(r["verdict"], "closed", "{r:#}");
    assert_eq!(r["label"], "closed, boxed, per-key", "{r:#}");
    assert_eq!(state(&cdir).nodes["toy"].state.phase, NodePhase::Closed);
    // the one terminal is the proposal [1,1,0] (accepted: 1 <= 2), with its attempt record
    let t = role_inputs(&cdir, "closeloop_terminals");
    assert_eq!(t.len(), 1, "{t:?}");
    let tv: Value = serde_json::from_slice(&t[0].1).unwrap();
    assert_eq!(tv["terminals"], json!([[1, 1, 0]]), "{tv:#}");
    let att = &tv["attempts"][0];
    assert_eq!(att["symmetry_at_key_tried"], true);
    assert_eq!(att["recipe_answers"].as_array().unwrap().len(), 2, "{att}");
    assert_eq!(terminals_of(&cdir).to_string().contains("I(1,1,0)"), true);
    // the escalation: symmetry attempted at every uncovered key (no group: recorded unavailable), recipes per key in
    // step order, keys 3, 2 served by their first step, key 1 by none
    let sym: Vec<Value> = role_inputs(&cdir, "closeloop_symrel")
        .iter()
        .flat_map(|(_, b)| {
            String::from_utf8_lossy(b)
                .lines()
                .map(|l| serde_json::from_str::<Value>(l).unwrap())
                .collect::<Vec<_>>()
        })
        .collect();
    let keys: BTreeSet<String> = sym.iter().map(|v| v["key"].to_string()).collect();
    assert_eq!(keys.len(), 3, "{sym:?}");
    assert!(sym.iter().all(|v| v["verdict"] == "unavailable"), "{sym:?}");
    let rec: Vec<Value> = role_inputs(&cdir, "closeloop_recipes")
        .iter()
        .flat_map(|(_, b)| {
            String::from_utf8_lossy(b)
                .lines()
                .map(|l| serde_json::from_str::<Value>(l).unwrap())
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(rec.len(), 3);
    for v in &rec {
        let ids: Vec<&str> = v["recipes"].as_array().unwrap().iter().map(|x| x["id"].as_str().unwrap()).collect();
        let k: Vec<i32> = serde_json::from_value(v["key"].clone()).unwrap();
        assert_eq!(ids, STEPS.iter().map(|s| recipe_source_id(&k, s)).collect::<Vec<_>>());
    }
    let ebc = &r["by_rule_class_level"];
    assert_eq!(ebc["escalation"]["per-key"], 2, "{ebc:#}");
    // big-node mode: the spec's evidence is absent (no table evidence), the node's own fact / proof answers were read
    // through evidence stores, the keys-only pass was streamed
    let checks: Vec<Value> = role_inputs(&cdir, "closeloop_check")
        .iter()
        .map(|(_, b)| serde_json::from_slice(b).unwrap())
        .collect();
    assert!(
        checks
            .iter()
            .any(|c| c["evidence_import"]["stores"].as_array().is_some_and(|s| !s.is_empty())),
        "no check read an evidence store"
    );
    assert!(cdir.join("closeloop/toy/store").is_dir());
    assert_eq!(verify(&cdir), 0);
}

#[test]
fn exhausted_keys_over_the_terminal_rule_end_the_node_open() {
    let _g = serial();
    let (_t, sp, cdir) = setup("t2over", 3, Some(0), false, 20);
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&cdir));
    let r = result_json(&cdir);
    assert_eq!(r["verdict"], "open", "{r:#}");
    let b: Vec<String> = r["blockers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect();
    assert!(
        b.iter().any(|x| x.starts_with("terminal_proposals_over_rule: 1 ")),
        "{b:?}"
    );
    assert!(role_inputs(&cdir, "closeloop_terminals").is_empty());
    assert_eq!(state(&cdir).nodes["toy"].state.phase, NodePhase::Open);
}

#[test]
fn interrupt_and_resume_through_the_escalation_gives_the_same_decisions() {
    let _g = serial();
    let (_ta, spa, ca) = setup("t2ref", 4, Some(3), true, 120);
    assert_eq!(run_to_end(&spa, &ca, &["--plain"]).last(), Some(&0));
    let (ref_items, ref_esc) = decisions(&ca);
    let ref_stmt = std::fs::read(ca.join("closeloop/toy/final/statement/statement.jsonl")).unwrap();
    assert_eq!(result_json(&ca)["verdict"], "closed");
    for seed in [5u64, 23] {
        let (_tb, spb, cb) = setup(&format!("t2int{seed}"), 4, Some(3), true, 120);
        let mut x = seed;
        let mut codes = vec![];
        for round in 0..14 {
            let mut c = start(&spb, &cb, &["--plain"], &[]);
            if round < 5 {
                x = mix(x);
                std::thread::sleep(Duration::from_millis(300 + x % 2200));
                signal(&c, libc::SIGTERM);
            }
            let code = wait(&mut c, Duration::from_secs(300));
            codes.push(code);
            if code == 0 {
                break;
            }
        }
        assert_eq!(codes.last(), Some(&0), "{codes:?} {}", log_tail(&cb));
        assert!(codes.len() > 1, "no interrupt happened: {codes:?}");
        let (items, esc) = decisions(&cb);
        assert_eq!(items, ref_items, "seed {seed}: codes {codes:?}");
        assert_eq!(esc.len(), ref_esc.len(), "seed {seed}");
        for (a, b) in esc.iter().zip(&ref_esc) {
            assert_eq!(a.0, b.0, "seed {seed}");
            // the recipe manifests name absolute paths under each campaign directory: compare without them
            let strip = |v: &[u8], d: &Path| String::from_utf8_lossy(v).replace(&d.display().to_string(), "CDIR");
            assert_eq!(strip(&a.1, &cb), strip(&b.1, &ca), "seed {seed}: {}", a.0);
        }
        let stmt = std::fs::read(cb.join("closeloop/toy/final/statement/statement.jsonl")).unwrap();
        assert!(stmt == ref_stmt, "seed {seed}: statements differ");
        assert_eq!(result_json(&cb)["verdict"], "closed");
        assert_eq!(verify(&cb), 0);
    }
}
