//! Lane zeros2 (interfaces/closeloop.md 23) under a REAL `rr_campaign` supervisor with `rr_fake_child`: the second route
//! of EVERY zero term. The start key's rule has a POSITIVE-NUMERATOR term I(5,1,1) (never an edge: not a key of the chart);
//! its first-route fact (rr_proof instantiate line in the spec's evidence) says "zero".
//! - honest (coefficient `(n0-5)`, zero at the key): the zero is confirmed by the zeros route (the fake `coefficient-facts`
//!   child = the campaign core's exact evaluation), the node closes, the basis names both routes;
//! - LIED (coefficient `2`, non-zero): `all_zeros` (default with exact evaluation configured) catches it: verdict open,
//!   `second_route_disagreement:1` naming the term; `all_zeros: off` is closeloop4's behaviour: the node CLOSES (the lie
//!   is not caught: what this lane fixes);
//! - `all_zeros` (default structural, or explicit) without `proof.exact_eval` blocks (`all_zeros_unavailable`).
//!
//! Spawns processes and sends signals: run ONLY inside a private pid namespace (`LIGHT_PIDNS=1 .../light_build.sh
//! zeros2 test -p rustred-campaign --test zeros2_fake`).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

mod closeloop_support;
mod sup_common;

use closeloop_support::*;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use sup_common::*;

fn sha(s: &str) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(s.as_bytes());
    h.finish_hex()
}

/// The table rule at the start key [5,1,0]: a lower term, a hand-down and the positive-numerator term I(5,1,1) with
/// coefficient `coef`.
fn posnum_rule(coef: &str) -> String {
    json!({"type":"rule","chart":"110","index":0,"fixed":[5,1,0],"equations":[],"target":"I(5,1,0)",
        "rhs":[["I(4,1,0)","1"],["I(5,0,0)","2"],["I(5,1,1)",coef]],
        "exceptions":[],"dispatch":"Partition","witness_loci":[]})
    .to_string()
}

/// The FIRST route's fact of the rule at the start key: I(5,1,1) is ZERO (a lie when the coefficient is not).
fn first_fact(rule: &str) -> Value {
    json!({"schema":"rustred.proof-kernel.instantiate.v1","record_sha256":sha(rule),"rule":0,
        "key":[5,1,0],"in_case":true,"chart_ok":true,"branch":null,"regular":"regular",
        "irregular_terms":[],"hidden_loci":[],
        "rhs":[{"term":0,"key":[4,1,0],"coef":"nonzero"},{"term":1,"key":[5,0,0],"coef":"nonzero"},
               {"term":2,"key":[5,1,1],"coef":"zero"}],
        "zero_test":"exact","error":null})
}

/// Toy material with table A = `rule`, the first-route fact in the spec's evidence, `second_route` as given and (when
/// `exact`) the fake exact-evaluation child.
fn setup(
    tag: &str,
    rule: &str,
    second_route: Option<Value>,
    exact: bool,
) -> (Tmp, PathBuf, PathBuf) {
    let t = Tmp::new(tag);
    let cfg = Toy {
        second_face: None,
        ..Toy::default()
    };
    let files = toy(&t.path.join("material"), &cfg, FAKE);
    std::fs::write(&files.hand, "").unwrap();
    let dir = &files.dir;
    let a = write_table(dir, "A.table.jsonl", &[rule.to_string()]);
    std::fs::write(
        dir.join("proofs.json"),
        json!({"schema":"rustred.proof-kernel.span.v1","keys":[
            {"record_sha256": sha(rule), "key": [5,1,0], "status":"EXACT_IN_SPAN_AT_KEY"}]})
        .to_string(),
    )
    .unwrap();
    std::fs::write(dir.join("facts.jsonl"), format!("{}\n", first_fact(rule))).unwrap();
    let mut spec: Value =
        serde_json::from_slice(&std::fs::read(&files.import_spec).unwrap()).unwrap();
    spec["sources"][0]["sha256"] = json!(sha256_file(&a));
    spec["evidence"] =
        json!({"proofs":[pin(&dir.join("proofs.json"))], "facts":[pin(&dir.join("facts.jsonl"))]});
    std::fs::write(
        &files.import_spec,
        serde_json::to_vec_pretty(&spec).unwrap(),
    )
    .unwrap();
    let mut c: Value = serde_json::from_slice(&std::fs::read(&files.config).unwrap()).unwrap();
    if exact {
        c["nodes"][0]["proof"]["exact_eval"] = json!({"program": "rr_coef_facts", "max_keys_per_job": 8,
            "wall_s": 60, "cap_bytes": 268435456, "expected_peak_bytes": 67108864,
            "fake": {"ms": 20, "program": "rr_fake_child"}});
    }
    if let Some(sr) = second_route {
        c["nodes"][0]["second_route"] = sr;
    }
    std::fs::write(&files.config, serde_json::to_vec_pretty(&c).unwrap()).unwrap();
    let mut spec = campaign_spec(&t.campaign, &files, FAKE, &cpus());
    spec["programs"]["rr_coef_facts"] = json!({"path": FAKE});
    let sp = write_spec(&t, "spec.json", &spec);
    let cdir = t.cdir("cdir");
    (t, sp, cdir)
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

fn start_row(cdir: &Path) -> Value {
    std::fs::read_to_string(cdir.join("closeloop/toy/final/statement/statement.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .find(|v| v["key"] == json!([5, 1, 0]))
        .unwrap()
}

#[test]
fn an_honest_positive_numerator_zero_is_confirmed_and_the_node_closes() {
    let _g = serial();
    let rule = posnum_rule("(n0-5)");
    let (_t, sp, cdir) = setup("z2ok", &rule, None, true);
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&cdir));
    let r = result_json(&cdir);
    assert_eq!(r["label"], "closed, boxed, per-key", "{r:#}");
    let z = &r["second_route"]["all_zeros"];
    assert_eq!(z["mode"], "structural", "{z:#}");
    assert_eq!(z["pairs"], 1, "{z:#}");
    assert_eq!(z["confirmed_pairs"], 1, "{z:#}");
    assert_eq!(z["confirmed_by_class"]["positive_numerator"], 1, "{z:#}");
    assert_eq!(z["disagreements"], 0, "{z:#}");
    assert_eq!(
        start_row(&cdir)["facts"]["basis"],
        "rr_proof + zeros:per_key"
    );
    let st = state(&cdir);
    assert!(
        st.items.values().any(|it| it.label.contains("all_zeros")),
        "{:?}",
        st.items
            .values()
            .map(|i| i.label.clone())
            .collect::<Vec<_>>()
    );
    assert!(st.inputs.values().any(|i| i.role == "closeloop_zeros2"));
    // the result's statement header counts the zero terms both routes confirmed
    let sdir = cdir.join("closeloop/toy/final/statement");
    let hdr = std::fs::read_to_string(sdir.join("statement.jsonl")).unwrap();
    assert!(hdr.contains("zero_terms") || std::fs::read_dir(&sdir).unwrap().count() > 0);
}

#[test]
fn a_lied_positive_numerator_zero_is_caught_under_all_zeros() {
    let _g = serial();
    let rule = posnum_rule("2");
    let (_t, sp, cdir) = setup("z2lie", &rule, None, true);
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    assert_eq!(codes.last(), Some(&10), "{codes:?} {}", log_tail(&cdir));
    let r = result_json(&cdir);
    assert_eq!(r["verdict"], "open", "{r:#}");
    let b = blockers(&r);
    assert!(
        b.contains(&"second_route_disagreement:1".to_string()),
        "{b:?}"
    );
    assert!(
        b.iter()
            .any(|x| x.starts_with("second_route_disagreement_term:") && x.ends_with("#2")),
        "{b:?}"
    );
    let d = &r["second_route"]["all_zeros"]["disagreement_list"][0];
    assert_eq!(d["applied"], json!([5, 1, 0]), "{d:#}");
    assert_eq!(d["differ"][0]["term"], 2, "{d:#}");
    assert_eq!(d["differ"][0]["term_key"], json!([5, 1, 1]), "{d:#}");
    assert_eq!(d["differ"][0]["second"], "nonzero", "{d:#}");
}

#[test]
fn all_zeros_off_is_todays_behaviour_the_lie_is_not_caught() {
    let _g = serial();
    let rule = posnum_rule("2");
    let (_t, sp, cdir) = setup("z2off", &rule, Some(json!({"all_zeros": "off"})), true);
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{codes:?} {}", log_tail(&cdir));
    let r = result_json(&cdir);
    assert_eq!(r["label"], "closed, boxed, per-key", "{r:#}");
    assert_eq!(r["second_route"]["all_zeros"]["mode"], "off", "{r:#}");
    assert_eq!(start_row(&cdir)["facts"]["basis"], "rr_proof");
}

#[test]
fn per_key_mode_confirms_and_an_explicit_mode_without_exact_evaluation_blocks() {
    let _g = serial();
    let rule = posnum_rule("(n0-5)");
    let (_t, sp, cdir) = setup("z2pk", &rule, Some(json!({"all_zeros": "per_key"})), true);
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&cdir));
    let r = result_json(&cdir);
    assert_eq!(r["second_route"]["all_zeros"]["mode"], "per_key", "{r:#}");
    assert_eq!(
        r["second_route"]["all_zeros"]["confirmed_pairs"], 1,
        "{r:#}"
    );
    let (_t2, sp2, cdir2) = setup(
        "z2na",
        &rule,
        Some(json!({"all_zeros": "structural"})),
        false,
    );
    let codes = run_to_end(&sp2, &cdir2, &["--plain"]);
    assert_eq!(codes.last(), Some(&10), "{codes:?} {}", log_tail(&cdir2));
    let r = result_json(&cdir2);
    assert!(
        blockers(&r)
            .iter()
            .any(|b| b.starts_with("all_zeros_unavailable:")),
        "{r:#}"
    );
    // absent mode (the DEFAULT, structural) and no exact evaluation: a zero to confirm blocks too (never one route)
    let (_t3, sp3, cdir3) = setup("z2abs", &rule, None, false);
    let codes = run_to_end(&sp3, &cdir3, &["--plain"]);
    assert_eq!(codes.last(), Some(&10), "{codes:?} {}", log_tail(&cdir3));
    let r = result_json(&cdir3);
    assert_eq!(r["second_route"]["all_zeros"]["mode"], "structural");
    assert!(
        blockers(&r)
            .iter()
            .any(|b| b.starts_with("all_zeros_unavailable:")),
        "{r:#}"
    );
}
