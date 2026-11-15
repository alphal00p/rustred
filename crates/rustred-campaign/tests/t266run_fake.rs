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
fn setup(
    tag: &str,
    top: i32,
    max_terminals: Option<usize>,
    scale: bool,
    ms: u64,
) -> (Tmp, PathBuf, PathBuf) {
    setup_with(tag, top, max_terminals, scale, ms, false)
}

/// The terminal rule: auto-accept up to `m` (None: every proposal waits for the operator), the proposal cap.
fn rule(m: Option<usize>, cap: Option<usize>) -> Value {
    let mut r = json!({});
    if let Some(m) = m {
        r["max_terminals"] = json!(m);
    }
    if let Some(c) = cap {
        r["max_proposals"] = json!(c);
    }
    r
}

/// `clauses`: the start set is the union of clause "box" = {[top+1,1,0]} and clause "layer" = {[top+2,1,0]} (a key
/// outside the first clause's closure, uncovered: escalated and solved onto [top+1,1,0]).
fn setup_with(
    tag: &str,
    top: i32,
    max_terminals: Option<usize>,
    scale: bool,
    ms: u64,
    clauses: bool,
) -> (Tmp, PathBuf, PathBuf) {
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
    let mut spec: Value =
        serde_json::from_slice(&std::fs::read(&files.import_spec).unwrap()).unwrap();
    let srcs: Vec<Value> = spec["sources"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["kind"] != "recipe")
        .cloned()
        .collect();
    spec["sources"] = json!(srcs);
    std::fs::write(
        &files.import_spec,
        serde_json::to_vec_pretty(&spec).unwrap(),
    )
    .unwrap();
    // the oracle: the first step solves [k,1,0] for k >= 2, nothing solves [1,1,0]
    let op = files.dir.join("oracle.jsonl");
    let mut o = String::new();
    for k in 2..=top + if clauses { 2 } else { 0 } {
        if k == top + 1 {
            continue;
        }
        o.push_str(&format!(
            "{}\n",
            json!({"source": recipe_source_id(&[k, 1, 0], STEPS[0], &[true, true, false]), "key": [k, 1, 0], "verdict": "SOLVED",
                   "rhs": [[k - 1, 1, 0], [k, 0, 0]]})
        ));
    }
    std::fs::write(&op, o).unwrap();
    let mut c: Value = serde_json::from_slice(&std::fs::read(&files.config).unwrap()).unwrap();
    let node = &mut c["nodes"][0];
    node["hand"] = Value::Null;
    node["escalation"] = json!({"symmetry_at_key": true, "steps": STEPS,
    "terminals": match max_terminals {
        // usize::MAX: no auto-acceptance (operator decision); 1000 + c: cap c, no auto-acceptance
        Some(usize::MAX) => rule(None, None),
        Some(m) if m >= 1000 => rule(None, Some(m - 1000)),
        m => rule(m, None),
    }});
    if scale {
        // the clause test also runs the exact tier at once (no keys-only pass first)
        node["scale"] =
            json!({"store_budget_mib": 1, "store_chunk_files": 2, "keys_only_first": !clauses});
    }
    node["budgets"]["max_rounds"] = json!(40);
    if clauses {
        let a = files.dir.join("clause-box.jsonl");
        let b = files.dir.join("clause-layer.jsonl");
        std::fs::write(&a, format!("{}\n", json!([top + 1, 1, 0]))).unwrap();
        std::fs::write(&b, format!("{}\n", json!([top + 2, 1, 0]))).unwrap();
        let mut sp: Value =
            serde_json::from_slice(&std::fs::read(&files.import_spec).unwrap()).unwrap();
        let st = PathBuf::from(sp["start"]["path"].as_str().unwrap());
        std::fs::write(
            &st,
            format!("{}\n{}\n", json!([top + 1, 1, 0]), json!([top + 2, 1, 0])),
        )
        .unwrap();
        sp["start"] = json!({"path": st.display().to_string(), "format": "keys_jsonl"});
        std::fs::write(&files.import_spec, serde_json::to_vec_pretty(&sp).unwrap()).unwrap();
        node["clauses"] = json!([
            {"id": "box", "start": {"path": a.display().to_string(), "format": "keys_jsonl"}},
            {"id": "layer", "start": {"path": b.display().to_string(), "format": "keys_jsonl"}}]);
    }
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
        .map(|(id, r)| {
            (
                r.registered_seq,
                id.clone(),
                std::fs::read(&r.path).unwrap(),
            )
        })
        .collect();
    v.sort();
    v.into_iter().map(|(_, id, b)| (id, b)).collect()
}

/// Every item (id, kind, phase) and the escalation records of a run.
type Decisions = (BTreeSet<(String, String, String)>, Vec<(String, Vec<u8>)>);

/// The decisions of a run: every item (id, kind, phase) and the escalation records.
fn decisions(cdir: &Path) -> Decisions {
    let items = state(cdir)
        .items
        .iter()
        .map(|(k, it)| (k.clone(), it.kind.clone(), it.phase.tag().to_string()))
        .collect();
    let mut esc = vec![];
    for role in [
        "closeloop_symrel",
        "closeloop_recipes",
        "closeloop_terminals",
    ] {
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
    assert!(terminals_of(&cdir).to_string().contains("I(1,1,0)"));
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
    // one recipe set per FACE: [3,1,0] and [2,1,0] share the face L0|N (one manifest, made when [3,1,0] was first
    // met), [1,1,0] is the face L|N
    assert_eq!(rec.len(), 2, "{rec:?}");
    for v in &rec {
        let ids: Vec<&str> = v["recipes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["id"].as_str().unwrap())
            .collect();
        let k: Vec<i32> = serde_json::from_value(v["keys"][0].clone()).unwrap();
        assert_eq!(
            ids,
            STEPS
                .iter()
                .map(|s| recipe_source_id(&k, s, &[true, true, false]))
                .collect::<Vec<_>>()
        );
    }
    let faces: Value = serde_json::from_slice(
        &std::fs::read(cdir.join("closeloop/toy/final/faces.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(faces["faces"], 2, "{faces:#}");
    let row = |f: &str| {
        faces["table"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["face"] == f)
            .unwrap()
            .clone()
    };
    assert_eq!(row("L0|N")["keys"], 2, "{faces:#}");
    assert_eq!(row("L0|N")["served_by_step"][STEPS[0]], 2, "{faces:#}");
    assert_eq!(row("L|N")["terminal"], 1, "{faces:#}");
    assert_eq!(faces["closed_by_one_recipe_or_symmetry"], 1);
    assert_eq!(r["faces"]["with_open_or_terminal_keys"], 1);
    let ebc = &r["by_rule_class_level"];
    assert_eq!(ebc["escalation"]["per-key"], 2, "{ebc:#}");
    // big-node mode: the spec's evidence is absent (no table evidence), the node's own fact / proof answers were read
    // through evidence stores, the keys-only pass was streamed
    let checks: Vec<Value> = role_inputs(&cdir, "closeloop_check")
        .iter()
        .map(|(_, b)| serde_json::from_slice(b).unwrap())
        .collect();
    assert!(
        checks.iter().any(|c| c["evidence_import"]["stores"]
            .as_array()
            .is_some_and(|s| !s.is_empty())),
        "no check read an evidence store"
    );
    assert!(cdir.join("closeloop/toy/store").is_dir());
    assert_eq!(verify(&cdir), 0);
}

/// Run once (no resume): the exit code.
fn run_once(sp: &Path, cdir: &Path, extra: &[&str]) -> i32 {
    let mut c = start(sp, cdir, extra, &[]);
    wait(&mut c, Duration::from_secs(900))
}

fn blockers(r: &Value) -> Vec<String> {
    r["blockers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn terminal_proposals_wait_for_the_operator_and_a_cap_stops_a_failing_escalation() {
    let _g = serial();
    // no auto-acceptance, nothing pre-approved: the run ends open, exit 8, the proposal reported
    let (_t, sp, cdir) = setup("t2op", 3, Some(usize::MAX), false, 20);
    let code = run_once(&sp, &cdir, &["--plain"]);
    assert_eq!(code, ATTENTION, "{}", log_tail(&cdir));
    // lane closeloop4 (closeloop.md 19): no final verdict while the proposals wait; the operator decides with
    // `resolve-terminals` and the run CONTINUES in the same campaign directory
    assert!(!cdir.join("closeloop/toy/final/result.json").exists());
    let p = closeloop_support::latest_proposals(&cdir, "toy").expect("proposals");
    assert_eq!(p["proposals"][0]["key"], json!([1, 1, 0]), "{p:#}");
    assert!(p["attempts"].is_array(), "{p:#}");
    assert!(role_inputs(&cdir, "closeloop_terminals").is_empty());
    assert_eq!(
        closeloop_support::resolve_terminals(&cdir, Some("toy"), Some(&[[1, 1, 0]])),
        0
    );
    let code = run_once(&sp, &cdir, &["--plain"]);
    assert_eq!(code, 0, "{}", log_tail(&cdir));
    let r = result_json(&cdir);
    assert_eq!(r["verdict"], "closed", "{r:#}");
    assert_eq!(
        r["terminals_accepted"][0]["accepted_by"], "operator",
        "{r:#}"
    );
    // cap 0: one exhausted key is already over the cap: typed stop, nothing accepted
    let (_t2, sp2, cdir2) = setup("t2cap", 3, Some(1000), false, 20);
    let code = run_once(&sp2, &cdir2, &["--plain"]);
    // closeloop3's outcome(): an open verdict is exit 10
    assert_eq!(code, FINISHED_OPEN, "{}", log_tail(&cdir2));
    let r = result_json(&cdir2);
    assert!(
        blockers(&r)
            .iter()
            .any(|x| x.starts_with("terminal_proposals_over_cap: 1 ")),
        "{r:#}"
    );
    assert!(role_inputs(&cdir2, "closeloop_terminals").is_empty());
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
        // resumes run on ANOTHER CPU list (a launch-time resource, not part of the campaign identity)
        let all: Vec<String> = cpus().split(',').map(str::to_string).collect();
        let other = all
            .iter()
            .rev()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join(",");
        for round in 0..14 {
            let args: Vec<&str> = if round == 0 {
                vec!["--plain"]
            } else {
                vec!["--plain", "--cpus", other.as_str(), "--max-jobs", "2"]
            };
            let mut c = start(&spb, &cb, &args, &[]);
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
            let strip = |v: &[u8], d: &Path| {
                String::from_utf8_lossy(v).replace(&d.display().to_string(), "CDIR")
            };
            assert_eq!(strip(&a.1, &cb), strip(&b.1, &ca), "seed {seed}: {}", a.0);
        }
        let stmt = std::fs::read(cb.join("closeloop/toy/final/statement/statement.jsonl")).unwrap();
        assert!(stmt == ref_stmt, "seed {seed}: statements differ");
        assert_eq!(result_json(&cb)["verdict"], "closed");
        assert_eq!(verify(&cb), 0);
    }
}

#[test]
fn two_box_clauses_get_their_own_verdicts_and_every_key_its_clause() {
    let _g = serial();
    let (_t, sp, cdir) = setup_with("t2cl", 3, Some(2), true, 20, true);
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&cdir));
    let r = result_json(&cdir);
    assert_eq!(r["verdict"], "closed", "{r:#}");
    let cl = r["clauses"].as_array().unwrap_or_else(|| panic!("{r:#}"));
    assert_eq!(cl.len(), 2);
    assert_eq!(
        (cl[0]["id"].as_str(), cl[0]["verdict"].as_str()),
        (Some("box"), Some("closed")),
        "{cl:?}"
    );
    assert_eq!(
        (cl[1]["id"].as_str(), cl[1]["verdict"].as_str()),
        (Some("layer"), Some("closed")),
        "{cl:?}"
    );
    assert_eq!(cl[1]["label"], "layer: closed");
    // the layer key [5,1,0] is the only key outside the first clause's closure; the layer's own closure contains the
    // whole first clause's chain below it
    assert_eq!(cl[1]["brought_in"], 1, "{cl:?}");
    assert!(
        cl[1]["closure_keys"].as_u64().unwrap() > cl[0]["closure_keys"].as_u64().unwrap(),
        "{cl:?}"
    );
    let ck = std::fs::read_to_string(cdir.join("closeloop/toy/final/statement/clause-keys.jsonl"))
        .unwrap();
    let v: Vec<Value> = ck
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(v, vec![json!({"key": [5, 1, 0], "clause": "layer"})]);
    assert_eq!(
        r["clause_attribution"]["union_keys_outside_every_clause"],
        0
    );
    assert_eq!(verify(&cdir), 0);
}

// ------------------------------------------------------------------------------------------- resume after escalation
// Defect of run T266run5 (2026-11-15): a restarted session loaded the BASE import spec (no escalation recipes), so
// every escalation solve settled after the restart was refused ("recipe source ... not in the imported set") and its
// item stayed in progress forever. Fixed: the restarted session loads the last pass's effective spec (re-derived from
// the ledger, verified against the digest the pass recorded); a refused routing is a typed item event.

use rustred_campaign::closeloop::ids::{KIND_FACTS, KIND_PROOF, KIND_SOLVE};
use rustred_campaign::state::{AttemptState, CampaignState, ItemPhase};

/// Items left IN PROGRESS over a sealed-success attempt (a settle without routing): none at any exit.
fn unrouted(cdir: &Path) -> Vec<String> {
    let st = state(cdir);
    st.items
        .iter()
        .filter(|(_, it)| {
            it.phase == ItemPhase::InProgress
                && it
                    .jobs
                    .last()
                    .and_then(|j| st.jobs.get(j))
                    .is_some_and(|r| matches!(r.state, AttemptState::SucceededSealed(_)))
        })
        .map(|(k, _)| k.clone())
        .collect()
}

/// Poll the READ-ONLY ledger view until `pred` holds (false after `limit`).
fn wait_state(cdir: &Path, limit: Duration, pred: impl Fn(&CampaignState) -> bool) -> bool {
    let t0 = std::time::Instant::now();
    while t0.elapsed() < limit {
        if cdir.join("ledger").is_dir() && pred(&state(cdir)) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

/// A job of an item of one of `kinds` is running.
fn running_kind(st: &CampaignState, kinds: &[&str]) -> bool {
    st.jobs.values().any(|j| {
        j.state == AttemptState::Running
            && st
                .items
                .get(&j.item)
                .is_some_and(|it| kinds.contains(&it.kind.as_str()))
    })
}

/// An escalation round committed (step recipes recorded).
fn escalated(st: &CampaignState) -> bool {
    st.inputs.values().any(|r| r.role == "closeloop_recipes")
}

/// Interrupt (SIGTERM) when `when` holds, then resume to the end; every exit leaves no unrouted item; the decisions,
/// escalation records and statement equal the uninterrupted reference; no routing was refused or requeued.
fn interrupt_when_and_resume(tag: &str, when: impl Fn(&CampaignState) -> bool) {
    let (_ta, spa, ca) = setup(&format!("{tag}ref"), 4, Some(3), true, 1500);
    assert_eq!(
        run_to_end(&spa, &ca, &["--plain"]).last(),
        Some(&0),
        "{}",
        log_tail(&ca)
    );
    let (ref_items, ref_esc) = decisions(&ca);
    let ref_stmt = std::fs::read(ca.join("closeloop/toy/final/statement/statement.jsonl")).unwrap();
    let (_tb, spb, cb) = setup(&format!("{tag}int"), 4, Some(3), true, 1500);
    let mut c = start(&spb, &cb, &["--plain"], &[]);
    let hit = wait_state(&cb, Duration::from_secs(600), &when);
    signal(&c, libc::SIGTERM);
    let code = wait(&mut c, Duration::from_secs(300));
    assert!(hit, "the interrupt window never came: {}", log_tail(&cb));
    assert_ne!(code, 0, "interrupted session ended 0");
    assert_eq!(unrouted(&cb), Vec::<String>::new(), "after the interrupt");
    let mut codes = vec![code];
    for _ in 0..6 {
        let mut c = start(&spb, &cb, &["--plain"], &[]);
        let code = wait(&mut c, Duration::from_secs(900));
        codes.push(code);
        assert_eq!(unrouted(&cb), Vec::<String>::new(), "after exit {code}");
        if code == 0 {
            break;
        }
    }
    assert_eq!(codes.last(), Some(&0), "{codes:?} {}", log_tail(&cb));
    let st = state(&cb);
    assert!(
        st.items
            .values()
            .all(|it| it.phase != ItemPhase::InProgress),
        "an item in progress at the end"
    );
    let recs = format!("{:?}", records(&cb));
    assert!(
        !recs.contains("routing_refused"),
        "a routing was refused after the resume"
    );
    let (items, esc) = decisions(&cb);
    assert_eq!(items, ref_items, "{codes:?}");
    assert_eq!(esc.len(), ref_esc.len());
    for (a, b) in esc.iter().zip(&ref_esc) {
        assert_eq!(a.0, b.0);
        let strip = |v: &[u8], d: &Path| {
            String::from_utf8_lossy(v).replace(&d.display().to_string(), "CDIR")
        };
        assert_eq!(strip(&a.1, &cb), strip(&b.1, &ca), "{}", a.0);
    }
    let stmt = std::fs::read(cb.join("closeloop/toy/final/statement/statement.jsonl")).unwrap();
    assert!(stmt == ref_stmt, "statements differ");
    assert_eq!(result_json(&cb)["verdict"], "closed");
    assert_eq!(verify(&cb), 0);
}

#[test]
fn an_interrupt_during_escalation_solves_resumes_with_every_solve_routed() {
    let _g = serial();
    interrupt_when_and_resume("t2rs", |st| {
        escalated(st) && running_kind(st, &[KIND_SOLVE])
    });
}

#[test]
fn an_interrupt_during_the_proof_phase_after_an_escalation_round_resumes_routed() {
    let _g = serial();
    interrupt_when_and_resume("t2rp", |st| {
        escalated(st)
            && st
                .items
                .values()
                .any(|it| it.kind == KIND_SOLVE && matches!(it.phase, ItemPhase::Accepted(_)))
            && running_kind(st, &[KIND_PROOF, KIND_FACTS])
    });
}

#[test]
fn a_refused_or_dropped_routing_ends_typed_and_a_new_attempt_closes() {
    let _g = serial();
    // test-build failpoints: every FIRST solve attempt's routing refused (Err) / dropped (no events: an older build's
    // state); the item must be requeued (typed reason) and its new attempt routed through the normal acceptance
    for (site, tag) in [
        ("closeloop_route_refused:1", "t2rr"),
        ("closeloop_route_dropped:1", "t2rd"),
    ] {
        let (_t, sp, cdir) = setup(tag, 3, Some(2), true, 20);
        let mut codes = vec![];
        for _ in 0..4 {
            let mut c = start(&sp, &cdir, &["--plain"], &[("RR_CAMPAIGN_FAILPOINT", site)]);
            let code = wait(&mut c, Duration::from_secs(900));
            codes.push(code);
            assert_eq!(
                unrouted(&cdir),
                Vec::<String>::new(),
                "{site}: after exit {code}"
            );
            if code == 0 {
                break;
            }
        }
        assert_eq!(
            codes.last(),
            Some(&0),
            "{site}: {codes:?} {}",
            log_tail(&cdir)
        );
        assert_eq!(result_json(&cdir)["verdict"], "closed", "{site}");
        let st = state(&cdir);
        let solves: Vec<_> = st
            .items
            .iter()
            .filter(|(_, it)| it.kind == KIND_SOLVE)
            .collect();
        assert!(!solves.is_empty(), "{site}: no solve item");
        for (k, it) in &solves {
            assert_eq!(it.jobs.len(), 2, "{site}: {k} jobs {:?}", it.jobs);
            assert!(
                it.phase != ItemPhase::InProgress && !matches!(it.phase, ItemPhase::Attention(_)),
                "{site}: {k} {:?}",
                it.phase
            );
        }
        let recs = format!("{:?}", records(&cdir));
        assert!(
            recs.matches("routing_refused").count() >= solves.len(),
            "{site}: typed requeues missing"
        );
        assert_eq!(verify(&cdir), 0);
    }
}
