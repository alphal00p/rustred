//! Lane closeloop4 (interfaces/closeloop.md 18-20) under a REAL `rr_campaign` supervisor with `rr_fake_child`:
//! (18) the standing second route of the zero test: a DECISIVE zero (its term key is reached by nothing else) is
//! confirmed by the other route (the fake `coefficient-facts` child = the campaign core's exact evaluation); a lying
//! route blocks the key and the node; a record with elided loci gets BOTH routes (rr_proof `--elided-loci facts`);
//! (19) terminal proposals wait for the operator (exit 8) and `resolve-terminals` lets the run continue; `auto` within
//! `max_auto`; (20) a stopped campaign resumes on OTHER CPUs with the same decisions and statement.
//! Spawns processes and sends signals: run ONLY inside a private pid namespace (`LIGHT_PIDNS=1 .../light_build.sh
//! closeloop4 test -p rustred-campaign --test closeloop4_fake`).

mod closeloop_support;
mod sup_common;

use closeloop_support::*;
use rustred_campaign::ledger::Event;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;
use sup_common::*;

fn sha(s: &str) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(s.as_bytes());
    h.finish_hex()
}

fn digest(k: &[i32]) -> String {
    format!(
        "{:016x}",
        rustred_campaign_core::import::lines::term_key_digest(k)
    )
}

/// The table rule at the start key [5,1,0]: a lower term, a hand-down, and the term I(3,2,0) whose coefficient
/// `(n0-5)` vanishes identically at the key. Nothing else reaches I(3,2,0) (no second face): its zero is DECISIVE.
fn zero_rule() -> String {
    json!({"type":"rule","chart":"110","index":0,"fixed":[5,1,0],"equations":[],"target":"I(5,1,0)",
        "rhs":[["I(4,1,0)","1"],["I(5,0,0)","2"],["I(3,2,0)","(n0-5)"]],
        "exceptions":[],"dispatch":"Partition","witness_loci":[]})
    .to_string()
}

/// The elided record of lane closeloop3 (a positive-numerator term with a zero coefficient at the key).
fn elided_rule() -> String {
    json!({"type":"rule","chart":"110","index":0,"fixed":[5,1,0],"equations":[],"target":"I(5,1,0)",
        "rhs":[["I(4,1,0)","(d-4)/(d-3)"],["I(5,0,0)","2*n0"],["I(5,1,1)","((n0-5)*(d-2))/(d-1)"]],
        "exceptions":[],"dispatch":"Partition","witness_loci":["elided"]})
    .to_string()
}

/// Toy material with the table rule replaced by `rule`; `facts`: the rr_proof fact line of the rule at the start key
/// (first route) in the spec's evidence; the node gets `proof.exact_eval` (fake child, optional lies by key).
fn setup(tag: &str, rule: &str, first_fact: Option<Value>, lies: &[Value]) -> (Tmp, PathBuf, PathBuf) {
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
    let mut ev = json!({"proofs":[{"path": dir.join("proofs.json").display().to_string()}]});
    if let Some(f) = first_fact {
        std::fs::write(dir.join("facts.jsonl"), format!("{f}\n")).unwrap();
        ev["facts"] = json!([{"path": dir.join("facts.jsonl").display().to_string()}]);
    }
    let mut spec: Value =
        serde_json::from_slice(&std::fs::read(&files.import_spec).unwrap()).unwrap();
    spec["sources"][0]["sha256"] = json!(sha256_file(&a));
    spec["evidence"] = ev;
    std::fs::write(&files.import_spec, serde_json::to_vec_pretty(&spec).unwrap()).unwrap();
    let mut c: Value = serde_json::from_slice(&std::fs::read(&files.config).unwrap()).unwrap();
    let lp = dir.join("exact-lies.jsonl");
    std::fs::write(&lp, lies.iter().map(|v| format!("{v}\n")).collect::<String>()).unwrap();
    c["nodes"][0]["proof"]["exact_eval"] = json!({"program": "rr_coef_facts", "max_keys_per_job": 8,
        "wall_s": 60, "cap_bytes": 268435456, "expected_peak_bytes": 67108864,
        "fake": {"oracle": lp.display().to_string(), "ms": 20, "program": "rr_fake_child"}});
    std::fs::write(&files.config, serde_json::to_vec_pretty(&c).unwrap()).unwrap();
    let mut spec = campaign_spec(&t.campaign, &files, FAKE, &cpus());
    spec["programs"]["rr_coef_facts"] = json!({"path": FAKE});
    let sp = write_spec(&t, "spec.json", &spec);
    let cdir = t.cdir("cdir");
    (t, sp, cdir)
}

/// The rr_proof instantiate fact line of the zero rule at the start key (the FIRST route: I(3,2,0) is zero).
fn zero_rule_fact() -> Value {
    json!({"schema":"rustred.proof-kernel.instantiate.v1","record_sha256":sha(&zero_rule()),"rule":0,
        "key":[5,1,0],"in_case":true,"chart_ok":true,"branch":null,"regular":"regular",
        "irregular_terms":[],"hidden_loci":[],
        "rhs":[{"term":0,"key":[4,1,0],"coef":"nonzero"},{"term":1,"key":[5,0,0],"coef":"nonzero"},
               {"term":2,"key":[3,2,0],"coef":"zero"}],
        "zero_test":"exact","error":null})
}

fn result_json(cdir: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(cdir.join("closeloop/toy/final/result.json")).unwrap())
        .unwrap()
}

fn statement_rows(cdir: &Path) -> Vec<Value> {
    std::fs::read_to_string(cdir.join("closeloop/toy/final/statement/statement.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .collect()
}

fn blockers(r: &Value) -> Vec<String> {
    r["blockers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect()
}

fn run_once(sp: &Path, cdir: &Path, extra: &[&str]) -> i32 {
    let mut c = start(sp, cdir, extra, &[]);
    wait(&mut c, Duration::from_secs(600))
}

#[test]
fn a_decisive_zero_confirmed_by_the_second_route_closes_and_both_sources_are_recorded() {
    let _g = serial();
    let (_t, sp, cdir) = setup("cl4ok", &zero_rule(), Some(zero_rule_fact()), &[]);
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&cdir));
    let r = result_json(&cdir);
    assert_eq!(r["label"], "closed, boxed, per-key", "{r:#}");
    let s = &r["second_route"];
    assert_eq!(s["decisive_zeros"], 1, "{s:#}");
    assert_eq!(s["confirmed"], 1, "{s:#}");
    assert_eq!(s["disagreements"], 0, "{s:#}");
    // the term key I(3,2,0) is NOT in the closure (its only edge is the confirmed zero)
    let rows = statement_rows(&cdir);
    assert!(!rows.iter().any(|v| v["key"] == json!([3, 2, 0])));
    // the statement's decisive-zero table names both sources
    let dz = std::fs::read_to_string(cdir.join("closeloop/toy/final/statement/decisive_zeros.jsonl")).unwrap();
    let lines: Vec<Value> = dz.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(lines.len(), 1, "{dz}");
    assert_eq!(lines[0]["term_key"], json!([3, 2, 0]));
    assert_eq!(lines[0]["first"]["producer"], "rr_proof");
    assert_eq!(lines[0]["second"]["producer"], "exact_eval");
    assert_eq!(lines[0]["second"]["coef"], "zero");
    assert_eq!(lines[0]["status"], "confirmed");
    // the start key's fact basis is both routes
    let row = rows.iter().find(|v| v["key"] == json!([5, 1, 0])).unwrap();
    assert_eq!(row["facts"]["basis"], "rr_proof + exact_eval", "{row}");
    // the second route ran as its own item (reason second_route), recorded apart
    let st = state(&cdir);
    assert!(
        st.items.values().any(|it| it.label == "exact:second_route"),
        "{:?}",
        st.items.values().map(|i| i.label.clone()).collect::<Vec<_>>()
    );
    assert!(st.inputs.values().any(|i| i.role == "closeloop_facts2"));
}

#[test]
fn a_lying_zero_from_one_route_blocks_the_key_and_the_node() {
    let _g = serial();
    // the exact evaluation (second route) LIES: every term non-zero at the key
    let lie = json!({"key": [5,1,0], "line": {"nonzero": [digest(&[4,1,0]), digest(&[5,0,0]), digest(&[3,2,0])]}});
    let (_t, sp, cdir) = setup("cl4lie", &zero_rule(), Some(zero_rule_fact()), &[lie]);
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    assert_eq!(codes.last(), Some(&10), "{codes:?} {}", log_tail(&cdir));
    let r = result_json(&cdir);
    assert_eq!(r["verdict"], "open", "{r:#}");
    let b = blockers(&r);
    assert!(b.contains(&"second_route_disagreement:1".to_string()), "{b:?}");
    let d = &r["second_route"]["disagreement_list"][0];
    assert_eq!(d["key"], json!([5, 1, 0]), "{d:#}");
    assert_eq!(d["differ"][0]["term_key"], json!([3, 2, 0]), "{d:#}");
    assert_eq!(d["differ"][0]["first"], "zero");
    assert_eq!(d["differ"][0]["second"], "nonzero");
    // the disputed term is an edge: its key STAYS in the closure and the node does not close
    let rows = statement_rows(&cdir);
    assert!(rows.iter().any(|v| v["key"] == json!([3, 2, 0])), "the disputed key left the closure");
}

#[test]
fn an_elided_record_gets_both_routes_and_the_basis_names_them() {
    let _g = serial();
    let (_t, sp, cdir) = setup("cl4eli", &elided_rule(), None, &[]);
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&cdir));
    let r = result_json(&cdir);
    assert_eq!(r["label"], "closed, boxed, per-key", "{r:#}");
    assert_eq!(r["second_route"]["elided_pairs"], 1, "{:#}", r["second_route"]);
    assert_eq!(r["second_route"]["confirmed"], 1, "{:#}", r["second_route"]);
    let rows = statement_rows(&cdir);
    let row = rows.iter().find(|v| v["key"] == json!([5, 1, 0])).unwrap();
    assert_eq!(row["facts"]["basis"], "exact_eval + rr_proof_elided", "{row}");
    // the rr_proof job ran in facts mode (its answer is coefficient facts only: producer rr_proof_elided)
    let st = state(&cdir);
    let f2: Vec<String> = st
        .inputs
        .values()
        .filter(|i| i.role == "closeloop_facts2")
        .map(|i| std::fs::read_to_string(&i.path).unwrap())
        .collect();
    assert!(f2.iter().any(|t| t.contains("\"producer\":\"rr_proof_elided\"")), "{f2:?}");
}

/// The toy whose key [4,1,0] nothing can serve (its recipe answers no relation, no default recipe).
fn proposal_setup(tag: &str, terminals: Option<Value>) -> (Tmp, PathBuf, PathBuf) {
    let t = Tmp::new(tag);
    let cfg = Toy {
        default_recipe: false,
        second_face: None,
        overrides: vec![("R1".into(), [4, 1, 0], "NO_RELATION_IN_SEEDS".into(), vec![])],
        ..Toy::default()
    };
    let files = toy(&t.path.join("material"), &cfg, FAKE);
    std::fs::write(&files.hand, "").unwrap();
    if let Some(tc) = terminals {
        let mut c: Value = serde_json::from_slice(&std::fs::read(&files.config).unwrap()).unwrap();
        c["nodes"][0]["terminals"] = tc;
        std::fs::write(&files.config, serde_json::to_vec_pretty(&c).unwrap()).unwrap();
    }
    let spec = campaign_spec(&t.campaign, &files, FAKE, &cpus());
    let sp = write_spec(&t, "spec.json", &spec);
    let cdir = t.cdir("cdir");
    (t, sp, cdir)
}

#[test]
fn a_terminal_proposal_waits_for_the_operator_and_resolve_terminals_continues_the_run() {
    let _g = serial();
    let (_t, sp, cdir) = proposal_setup("cl4tp", None);
    let code = run_once(&sp, &cdir, &["--plain"]);
    assert_eq!(code, ATTENTION, "{}", log_tail(&cdir));
    assert!(log_tail(&cdir).contains("awaiting operator decision on 1 terminal proposals"), "{}", log_tail(&cdir));
    // no verdict yet: the node is not finished
    assert!(!cdir.join("closeloop/toy/final/result.json").exists());
    let p = latest_proposals(&cdir, "toy").expect("proposals record");
    assert_eq!(p["proposals"][0]["key"], json!([4, 1, 0]), "{p:#}");
    // a key that was not proposed is refused (the run stays waiting)
    assert_eq!(resolve_terminals(&cdir, Some("toy"), Some(&[[9, 9, 0]])), 0);
    assert_eq!(run_once(&sp, &cdir, &["--plain"]), ATTENTION, "{}", log_tail(&cdir));
    // the operator accepts it: the run continues to its verdict
    assert_eq!(resolve_terminals(&cdir, Some("toy"), Some(&[[4, 1, 0]])), 0);
    let code = run_once(&sp, &cdir, &["--plain"]);
    assert_eq!(code, 0, "{}", log_tail(&cdir));
    let r = result_json(&cdir);
    assert_eq!(r["label"], "closed, boxed, per-key", "{r:#}");
    let ta = r["terminals_accepted"].as_array().unwrap();
    assert_eq!(ta.len(), 1, "{ta:?}");
    assert_eq!(ta[0]["key"], json!([4, 1, 0]));
    assert_eq!(ta[0]["accepted_by"], "operator");
    assert!(ta[0]["pole_diagnostics"].is_null());
    let st = state(&cdir);
    assert!(st.inputs.values().any(|i| i.role == "closeloop_terminal_decision"));
}

#[test]
fn rejected_proposals_end_open_and_auto_accepts_within_max_auto_only() {
    let _g = serial();
    // reject: the node ends open (exit 10) with the rejected proposal named
    let (_t, sp, cdir) = proposal_setup("cl4rej", None);
    assert_eq!(run_once(&sp, &cdir, &["--plain"]), ATTENTION, "{}", log_tail(&cdir));
    assert_eq!(resolve_terminals(&cdir, None, None), 0);
    assert_eq!(run_once(&sp, &cdir, &["--plain"]), FINISHED_OPEN, "{}", log_tail(&cdir));
    let r = result_json(&cdir);
    assert!(blockers(&r).contains(&"terminal_proposals_rejected:1".to_string()), "{r:#}");
    // auto, max_auto 8: accepted automatically, recorded as such
    let (_t2, sp2, cdir2) = proposal_setup("cl4auto", Some(json!({"accept": "auto", "max_auto": 8})));
    assert_eq!(run_once(&sp2, &cdir2, &["--plain"]), 0, "{}", log_tail(&cdir2));
    let r2 = result_json(&cdir2);
    assert_eq!(r2["terminals_accepted"][0]["accepted_by"], "auto", "{r2:#}");
    // auto, max_auto 0: more proposals than the rule allows wait for the operator
    let (_t3, sp3, cdir3) = proposal_setup("cl4auto0", Some(json!({"accept": "auto", "max_auto": 0})));
    assert_eq!(run_once(&sp3, &cdir3, &["--plain"]), ATTENTION, "{}", log_tail(&cdir3));
}

type Decisions = (BTreeSet<(String, String, u64, String)>, BTreeMap<String, Vec<Value>>);

fn decisions(cdir: &Path) -> Decisions {
    let st = state(cdir);
    let items = st
        .items
        .iter()
        .map(|(k, it)| (k.clone(), it.label.clone(), it.weight, it.phase.tag().to_string()))
        .collect();
    let mut cert = BTreeMap::new();
    for (k, it) in st.items.iter() {
        if let Some(a) = &it.accepted
            && it.kind == "recipe-solve"
        {
            let b = std::fs::read(cdir.join(&a.record.path)).unwrap();
            let lines: Vec<Value> = String::from_utf8_lossy(&b)
                .lines()
                .map(|l| serde_json::from_str::<Value>(l).unwrap())
                .filter(|v| v["verdict"] == "SOLVED")
                .map(|v| json!([v["source"], v["key"], v["relation_digest"]]))
                .collect();
            cert.insert(k.clone(), lines);
        }
    }
    (items, cert)
}

fn session_notes(cdir: &Path) -> Vec<String> {
    records(cdir)
        .iter()
        .filter_map(|r| match &r.event {
            Event::Note { text } if text.starts_with("session_cpus ") => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_stopped_campaign_resumes_on_other_cpus_with_the_same_decisions_and_statement() {
    let _g = serial();
    let all: Vec<String> = cpus().split(',').map(str::to_string).collect();
    assert!(all.len() >= 4, "need 4 CPUs for this test, have {all:?}");
    let (a, b) = (all[..2].join(","), all[2..4].join(","));
    let cfg = Toy {
        top: 6,
        second_face: Some(4),
        ms: 150,
        ..Toy::default()
    };
    let mk = |tag: &str, list: &str| {
        let t = Tmp::new(tag);
        let files = toy(&t.path.join("material"), &cfg, FAKE);
        std::fs::write(&files.hand, "").unwrap();
        let spec = campaign_spec(&t.campaign, &files, FAKE, list);
        let sp = write_spec(&t, "spec.json", &spec);
        let cdir = t.cdir("cdir");
        (t, files, sp, cdir)
    };
    // reference: one session on CPUs A
    let (_ta, _fa, spa, ca) = mk("cl4cpuref", &a);
    assert_eq!(run_to_end(&spa, &ca, &["--plain", "--cpus", &a]).last(), Some(&0));
    let (ref_items, ref_cert) = decisions(&ca);
    let ref_stmt = std::fs::read(ca.join("closeloop/toy/final/statement/statement.jsonl")).unwrap();
    // interrupted on CPUs A, resumed on CPUs B: the spec file itself names B now (a launch-time field)
    let (tb, files_b, spb, cb) = mk("cl4cpu", &a);
    let mut c = start(&spb, &cb, &["--plain", "--cpus", &a], &[]);
    std::thread::sleep(Duration::from_millis(900));
    signal(&c, libc::SIGTERM);
    let first = wait(&mut c, Duration::from_secs(300));
    assert_ne!(first, 0, "the run finished before the interrupt");
    let spec_b = campaign_spec(&tb.campaign, &files_b, FAKE, &b);
    let spb2 = write_spec(&tb, "spec.json", &spec_b);
    assert_eq!(spb2, spb);
    let codes = run_to_end(&spb, &cb, &["--plain", "--cpus", &b]);
    assert_eq!(codes.last(), Some(&0), "{codes:?} {}", log_tail(&cb));
    let (items, cert) = decisions(&cb);
    assert_eq!(items, ref_items, "same decisions");
    assert_eq!(cert, ref_cert, "same certified relations");
    let stmt = std::fs::read(cb.join("closeloop/toy/final/statement/statement.jsonl")).unwrap();
    assert!(stmt == ref_stmt, "statements differ");
    // every session recorded its own CPUs: A first, B at the resume
    let notes = session_notes(&cb);
    assert!(notes.len() >= 2, "{notes:?}");
    let fmt = |l: &str| {
        let v: BTreeSet<usize> = l.split(',').map(|x| x.parse().unwrap()).collect();
        rustred_campaign::resources::format_cpu_list(&v)
    };
    assert!(notes[0].contains(&format!("\"cpus\":\"{}\"", fmt(&a))), "{notes:?}");
    assert!(notes.last().unwrap().contains(&format!("\"cpus\":\"{}\"", fmt(&b))), "{notes:?}");
    // every launch was pinned per job inside its session's CPUs: A before the resume, B after
    let bset: BTreeSet<u32> = b.split(',').map(|x| x.parse().unwrap()).collect();
    let aset: BTreeSet<u32> = a.split(',').map(|x| x.parse().unwrap()).collect();
    let (mut sessions, mut seen_b) = (0u32, false);
    for r in records(&cb) {
        match &r.event {
            Event::SupervisorStarted { .. } => sessions += 1,
            Event::Job {
                ev: rustred_campaign::ledger::JobEvent::Launch { cpus, .. },
                ..
            } => {
                let want = if sessions <= 1 { &aset } else { &bset };
                assert!(cpus.iter().all(|c| want.contains(c)), "session {sessions}: {cpus:?}");
                seen_b |= sessions > 1;
            }
            _ => {}
        }
    }
    assert!(seen_b, "no job ran on the resumed session's CPUs");
}
