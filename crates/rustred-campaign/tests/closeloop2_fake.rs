//! Lane closeloop2 (interfaces/closeloop.md 13): proof jobs (legacy rr_proof kinds emulated by `rr_fake_child`)
//! under a REAL `rr_campaign` supervisor. Spawns processes and sends signals: run ONLY inside a private pid namespace
//! (`LIGHT_PIDNS=1 .../light_build.sh closeloop2 test -p rustred-campaign --test closeloop2_fake`).
//!
//! The node is the TOY of closeloop_support (arity 3): our recipe relations are proved per key by `proof-instance-span`
//! jobs on their point records; without table evidence the exact tier asks `proof-instantiate` facts and the import's
//! proof request for the table rule.

mod closeloop_support;
mod sup_common;

use closeloop_support::*;
use rustred_campaign::state::{ItemPhase, NodePhase};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;
use sup_common::*;

fn setup(tag: &str, toy_cfg: &Toy) -> (Tmp, ToyFiles, std::path::PathBuf, std::path::PathBuf) {
    let t = Tmp::new(tag);
    let files = toy(&t.path.join("material"), toy_cfg, FAKE);
    std::fs::write(&files.hand, "").unwrap();
    let spec = campaign_spec(&t.campaign, &files, FAKE, &cpus());
    let sp = write_spec(&t, "spec.json", &spec);
    let cdir = t.cdir("cdir");
    (t, files, sp, cdir)
}

fn result_json(cdir: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(cdir.join("closeloop/toy/final/result.json")).unwrap())
        .unwrap()
}

fn blockers(r: &Value) -> Vec<String> {
    r["blockers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect()
}

/// (item, kind, label, phase) of every item.
fn items(cdir: &Path) -> BTreeSet<(String, String, String, String)> {
    state(cdir)
        .items
        .iter()
        .map(|(k, it)| {
            (
                k.clone(),
                it.kind.clone(),
                it.label.clone(),
                it.phase.tag().to_string(),
            )
        })
        .collect()
}

fn open_keys(cdir: &Path) -> Vec<Value> {
    std::fs::read_to_string(cdir.join("closeloop/toy/final/open_keys.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn our_relations_are_proved_per_key_and_the_node_closes_boxed_per_key() {
    let _g = serial();
    let (_t, _f, sp, cdir) = setup("cl2ok", &Toy::default());
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&cdir));
    let r = result_json(&cdir);
    assert_eq!(r["verdict"], "closed", "{r:#}");
    assert_eq!(r["label"], "closed, boxed, per-key", "{r:#}");
    assert_eq!(r["target"], "boxed");
    let rel = &r["check"]["relations"];
    assert!(rel["records"].as_u64().unwrap() >= 7, "{rel}");
    assert_eq!(rel["records"], rel["proved"], "{rel}");
    assert!(
        r["by_rule_class_level"]["recipe"]["per-key"]
            .as_u64()
            .unwrap()
            >= 7,
        "{r:#}"
    );
    assert_eq!(r["check"]["tier"], "C");
    assert_eq!(r["check"]["exact_tier"]["reached"], true);
    // every relation proof went through a proof-span item whose answers were sealed by the wrapper
    let it = items(&cdir);
    let spans = it
        .iter()
        .filter(|(_, k, l, _)| k == "proof-span" && l == "span:relation")
        .count();
    assert!(spans >= 7, "{it:?}");
    assert!(it.iter().all(|(_, _, _, ph)| ph != "attention"), "{it:?}");
    assert_eq!(state(&cdir).nodes["toy"].state.phase, NodePhase::Closed);
    assert_eq!(verify(&cdir), 0);
}

#[test]
fn a_proof_answer_for_another_record_or_key_is_refused() {
    let _g = serial();
    let cfg = Toy {
        proof_oracle: vec![
            // a child that answers about ANOTHER record (another rule) at the right key
            json!({"key": [3, 1, 0], "record_sha256": "0".repeat(64)}),
            // a child that answers the right record at ANOTHER key
            json!({"key": [2, 2, 0], "key_answered": [9, 2, 0]}),
        ],
        ..Toy::default()
    };
    let (_t, _f, sp, cdir) = setup("cl2lie", &cfg);
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&cdir));
    let r = result_json(&cdir);
    assert_eq!(r["verdict"], "open", "{r:#}");
    let b = blockers(&r);
    assert!(b.iter().any(|x| x == "relation_proofs_unbound:2"), "{b:?}");
    assert!(b.iter().any(|x| x == "proof_items_in_attention:2"), "{b:?}");
    // the lying jobs were refused by the wrapper (row 22: nothing accepted), retried, then put in attention
    let st = state(&cdir);
    let att: Vec<_> = st
        .items
        .iter()
        .filter(|(_, it)| it.kind == "proof-span" && matches!(it.phase, ItemPhase::Attention(_)))
        .collect();
    assert_eq!(att.len(), 2, "{att:?}");
    for (_, it) in &att {
        assert!(it.accepted.is_none());
        for j in &it.jobs {
            let run = st.jobs[j].run_dir.clone().unwrap();
            let why = std::fs::read_to_string(Path::new(&run).join("sup/wrapper-refusal.txt"))
                .unwrap_or_default();
            assert!(
                why.contains("no request asked for"),
                "{j}: wrapper refusal {why:?}"
            );
        }
    }
    // the two keys are served by our relations but below per-key; every other relation is proved
    let open = open_keys(&cdir);
    for k in [json!([3, 1, 0]), json!([2, 2, 0])] {
        let o = open
            .iter()
            .find(|v| v["key"] == k)
            .expect("in the open list");
        assert_eq!(o["status"], "served");
        assert_eq!(o["evidence"], "imported", "{o}");
        assert_eq!(o["basis"], "recipe_relation_unproved", "{o}");
    }
    assert_eq!(open.len(), 2, "{open:?}");
    assert_eq!(verify(&cdir), 0);
}

#[test]
fn a_relation_without_a_bound_proof_stays_below_per_key() {
    let _g = serial();
    // (a) no proof jobs at all: every relation of ours stays at `imported`
    let cfg = Toy {
        proof: false,
        ..Toy::default()
    };
    let (_t, _f, sp, cdir) = setup("cl2noproof", &cfg);
    assert_eq!(run_to_end(&sp, &cdir, &["--plain"]).last(), Some(&0));
    let r = result_json(&cdir);
    assert_eq!(r["verdict"], "open", "{r:#}");
    let lvl = &r["by_rule_class_level"]["recipe"];
    assert!(lvl["imported"].as_u64().unwrap() >= 7, "{r:#}");
    assert!(lvl.get("per-key").is_none(), "{r:#}");
    assert!(
        blockers(&r)
            .iter()
            .any(|x| x.starts_with("relation_proofs_unbound:")),
        "{r:#}"
    );
    // (b) the proof of one relation is not EXACT at its key: that key alone stays below per-key
    let cfg = Toy {
        proof_oracle: vec![
            json!({"key": [2, 1, 0], "status": "EXACT_NOT_IN_SPAN_AT_KEY (exact certificate u at d=7)"}),
        ],
        ..Toy::default()
    };
    let (_t2, _f2, sp2, cdir2) = setup("cl2notspan", &cfg);
    assert_eq!(run_to_end(&sp2, &cdir2, &["--plain"]).last(), Some(&0));
    let r = result_json(&cdir2);
    assert_eq!(r["verdict"], "open", "{r:#}");
    let open = open_keys(&cdir2);
    assert_eq!(open.len(), 1, "{open:?}");
    assert_eq!(open[0]["key"], json!([2, 1, 0]));
    assert_eq!(open[0]["basis"], "recipe_relation_unproved");
    assert_eq!(
        r["check"]["relations"]["proof_verdicts"]["EXACT_NOT_IN_SPAN_AT_KEY"], 1,
        "{r:#}"
    );
    assert_eq!(verify(&cdir2), 0);
}

#[test]
fn the_exact_tier_gets_its_facts_and_the_table_proof_from_proof_jobs() {
    let _g = serial();
    // no imported evidence for the table rule: its regularity / coefficient facts come from a proof-instantiate job,
    // its per-key proof from a proof-instance-span job of the import's proof request
    let cfg = Toy {
        table_evidence: false,
        proof_oracle: vec![json!({"key": [5, 1, 0], "line": {"rhs": [
            {"term": 0, "key": [4, 1, 0], "coef": "nonzero"},
            {"term": 1, "key": [5, 0, 0], "coef": "nonzero"}]}})],
        ..Toy::default()
    };
    let (_t, _f, sp, cdir) = setup("cl2facts", &cfg);
    assert_eq!(
        run_to_end(&sp, &cdir, &["--plain"]).last(),
        Some(&0),
        "{}",
        log_tail(&cdir)
    );
    let r = result_json(&cdir);
    assert_eq!(r["verdict"], "closed", "{r:#}");
    assert_eq!(r["check"]["tier"], "C");
    let it = items(&cdir);
    assert!(
        it.iter().any(|(_, k, l, ph)| k == "proof-facts"
            && l == "facts:import_fact_request"
            && ph == "accepted"),
        "{it:?}"
    );
    assert!(
        it.iter().any(|(_, k, l, ph)| k == "proof-span"
            && l == "span:import_proof_request"
            && ph == "accepted"),
        "{it:?}"
    );
    assert_eq!(r["by_rule_class_level"]["base"]["per-key"], 1, "{r:#}");
    assert_eq!(verify(&cdir), 0);
}

#[test]
fn interrupt_and_resume_across_the_proof_phase_gives_the_same_statement() {
    let _g = serial();
    let cfg = Toy {
        top: 5,
        second_face: Some(3),
        ms: 200,
        table_evidence: false,
        proof_oracle: vec![json!({"key": [6, 1, 0], "line": {"rhs": [
            {"term": 0, "key": [5, 1, 0], "coef": "nonzero"},
            {"term": 1, "key": [6, 0, 0], "coef": "nonzero"}]}})],
        ..Toy::default()
    };
    let (_ta, _fa, spa, ca) = setup("cl2ref", &cfg);
    assert_eq!(run_to_end(&spa, &ca, &["--plain"]).last(), Some(&0));
    let ref_items = items(&ca);
    let ref_stmt = std::fs::read(ca.join("closeloop/toy/final/statement/statement.jsonl")).unwrap();
    assert_eq!(result_json(&ca)["verdict"], "closed");
    assert!(ref_items.iter().any(|(_, k, _, _)| k == "proof-span"));
    for seed in [5u64, 23, 41] {
        let (_tb, _fb, spb, cb) = setup(&format!("cl2int{seed}"), &cfg);
        let mut x = seed;
        let mut codes = vec![];
        for round in 0..14 {
            let mut c = start(&spb, &cb, &["--plain"], &[]);
            if round < 5 {
                x = mix(x);
                std::thread::sleep(Duration::from_millis(400 + x % 2600));
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
        assert_eq!(items(&cb), ref_items, "seed {seed}: codes {codes:?}");
        let stmt = std::fs::read(cb.join("closeloop/toy/final/statement/statement.jsonl")).unwrap();
        assert!(stmt == ref_stmt, "seed {seed}: statements differ");
        assert_eq!(result_json(&cb)["verdict"], "closed");
        assert_eq!(verify(&cb), 0);
    }
}
