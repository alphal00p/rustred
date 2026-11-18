//! lane layers (interfaces/closeloop.md 23, closure.md 18): the closing loop closes the box LAYER BY LAYER under a REAL
//! `rr_campaign` supervisor, every earlier layer serving as LEAVES. Spawns processes and sends signals: run ONLY inside
//! a private pid namespace (`LIGHT_PIDNS=1 .../light_build.sh layers test -p rustred-campaign --test layers_fake`).
//!
//! The node is a TOY of arity 3 (lines 0, 1; numerator 2; nothing five-loop specific): a table of point rules on the
//! chain [k,1,0] -> [k-1,1,0] + the hand-down [k,0,0] (k = 2..TOP), [1,1,0] -> hand-downs only; every rule carries its
//! regularity fact and its exact per-key proof, so every pass decides at the exact tier with no child job.
//! Tests: (1) two layers composed == one walk, closed layers frozen and the node's leaf set written; (2) a later RUN
//! of the second clause with the first run's leaf set as leaves: "closed relative to" it, base + later statement ==
//! the single walk; (3) a changed / wrongly pinned leaf set and a leaf set of another table set are refused; (4) an
//! interrupt between layers: the finished layer is never walked again and the statement equals the uninterrupted
//! one; (5) a later layer that reaches an OPEN key of an earlier layer is open, the blocking key named.

mod closeloop_support;
mod sup_common;

use closeloop_support::{HEADER, blake3_file, pin, sha256_file};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;
use sup_common::*;

const TOP: i32 = 7;

fn rule_line(index: i64, k: i32) -> String {
    let rhs: Vec<Value> = if k > 1 {
        vec![
            json!([format!("I({},1,0)", k - 1), "1"]),
            json!([format!("I({k},0,0)"), "1"]),
        ]
    } else {
        vec![json!(["I(1,0,0)", "1"]), json!(["I(0,1,0)", "1"])]
    };
    json!({"type":"rule","chart":"110","index":index,"fixed":[k,1,0],"equations":[],
        "target": format!("I({k},1,0)"), "rhs": rhs,
        "exceptions":[],"dispatch":"Partition","witness_loci":[]})
    .to_string()
}

fn sha(s: &str) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(s.as_bytes());
    h.finish_hex()
}

/// Material: table (`skip` = chain keys WITHOUT a rule: uncovered), facts, proofs, start / layer files, import spec,
/// closeloop config. `coef` = a tag in every coefficient (another coefficient file = another table set).
struct Mat {
    config: PathBuf,
}

fn material(dir: &Path, start: &[[i32; 3]], layers: Option<Value>, skip: &[i32], tag: &str) -> Mat {
    std::fs::create_dir_all(dir).unwrap();
    let mut lines = vec![];
    let mut facts = String::new();
    let mut proofs = vec![];
    for (i, k) in (1..=TOP).filter(|k| !skip.contains(k)).enumerate() {
        let mut l = rule_line(i as i64, k);
        if !tag.is_empty() {
            // another coefficient file: same keys, another record content
            l = l.replacen("\"1\"]", &format!("\"1{tag}\"]"), 1);
        }
        let s = sha(&l);
        let rhs: Vec<Value> = if k > 1 {
            vec![
                json!({"term":0,"key":[k-1,1,0],"coef":"nonzero"}),
                json!({"term":1,"key":[k,0,0],"coef":"nonzero"}),
            ]
        } else {
            vec![
                json!({"term":0,"key":[1,0,0],"coef":"nonzero"}),
                json!({"term":1,"key":[0,1,0],"coef":"nonzero"}),
            ]
        };
        facts.push_str(&format!(
            "{}\n",
            json!({"schema":"rustred.proof-kernel.instantiate.v1","record_sha256":s,"rule":i,"key":[k,1,0],
                "in_case":true,"chart_ok":true,"branch":null,"regular":"regular","irregular_terms":[],
                "hidden_loci":[],"rhs":rhs,"zero_test":"exact","error":null})
        ));
        proofs.push(json!({"record_sha256": s, "key": [k,1,0], "status": "EXACT_IN_SPAN_AT_KEY"}));
        lines.push(l);
    }
    let a = dir.join("A.table.jsonl");
    let mut t = String::from(HEADER);
    t.push('\n');
    t.push_str(r#"{"type":"chart","chart":"110","guards":["n>=1","n>=1","n<=0"],"zero":false}"#);
    t.push('\n');
    for l in &lines {
        t.push_str(l);
        t.push('\n');
    }
    t.push_str(r#"{"type":"end","charts":1}"#);
    t.push('\n');
    std::fs::write(&a, t).unwrap();
    let fp = dir.join("facts.jsonl");
    std::fs::write(&fp, facts).unwrap();
    let pp = dir.join("proofs.json");
    std::fs::write(
        &pp,
        json!({"schema":"rustred.proof-kernel.span.v1","keys":proofs}).to_string(),
    )
    .unwrap();
    let st = dir.join("start.jsonl");
    std::fs::write(
        &st,
        start
            .iter()
            .map(|k| format!("{}\n", json!(k)))
            .collect::<String>(),
    )
    .unwrap();
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A","bits":6,
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "required_evidence":"per-key","policies":{"tier":"C"},
        "sources":[{"id":"A","kind":"engine_table","class":"base","path": a.display().to_string(),
                    "sha256": sha256_file(&a), "hand_name":{"template":"{source}#{label}"}}],
        "evidence": {"facts":[pin(&fp)],"proofs":[pin(&pp)]}});
    let import_spec = dir.join("import-spec.json");
    std::fs::write(&import_spec, serde_json::to_vec_pretty(&spec).unwrap()).unwrap();
    let family = dir.join("TOY.family");
    std::fs::write(&family, "toy family\n").unwrap();
    let mut node = json!({
        "node":"toy", "import_spec": import_spec.display().to_string(), "target":"boxed",
        "check_threads": 2, "default_recipe": null,
        "budgets": {"max_rounds": 10, "max_job_attempts": 2},
        "solve": {"program": "rr_fake_child", "family": family.display().to_string(), "order": "toy-order",
                  "relation_rule": "first_admissible", "max_keys_per_job": 2, "threads": 1,
                  "cap_bytes": 268435456, "expected_peak_bytes": 67108864, "heartbeat_s": 1, "stop_grace_s": 3,
                  "fake": {"oracle": dir.join("none.jsonl").display().to_string(), "ms": 10, "quirks": [],
                           "program": "rr_fake_child"}}});
    std::fs::write(dir.join("none.jsonl"), "").unwrap();
    if let Some(l) = layers {
        node["layers"] = l;
    }
    let config = dir.join("closeloop.json");
    std::fs::write(
        &config,
        serde_json::to_vec_pretty(
            &json!({"schema":"rustred.campaign.closeloop.v1","nodes":[node]}),
        )
        .unwrap(),
    )
    .unwrap();
    let _ = import_spec;
    Mat { config }
}

fn keys_file(dir: &Path, name: &str, keys: &[[i32; 3]]) -> Value {
    let p = dir.join(name);
    std::fs::write(
        &p,
        keys.iter()
            .map(|k| format!("{}\n", json!(k)))
            .collect::<String>(),
    )
    .unwrap();
    json!({"path": p.display().to_string(), "format": "keys_jsonl"})
}

fn set_node(m: &Mat, f: impl FnOnce(&mut Value)) {
    let mut c: Value = serde_json::from_slice(&std::fs::read(&m.config).unwrap()).unwrap();
    f(&mut c["nodes"][0]);
    std::fs::write(&m.config, serde_json::to_vec_pretty(&c).unwrap()).unwrap();
}

fn spec_of(t: &Tmp, m: &Mat) -> PathBuf {
    let v = json!({
        "schema": "rustred.campaign.spec.v1", "campaign": t.campaign,
        "programs": {"rr_fake_child": {"path": FAKE}},
        "resources": {"policy": "plain", "cpus": cpus(), "max_jobs": 2, "mem_total": "2G", "disk_floor": "1G",
                      "grace_seconds": 5, "sampler_ms": 300, "host_floor": "100M", "heartbeat_timeout_s": 60,
                      "start_timeout_s": 120},
        "progress": {"cadence_seconds": 1},
        "inputs": [{"id": "closeloop-config", "role": "closeloop", "path": m.config.display().to_string(),
                    "blake3": blake3_file(&m.config)}]
    });
    write_spec(t, "spec.json", &v)
}

fn result_of(cdir: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(cdir.join("closeloop/toy/final/result.json")).unwrap())
        .unwrap()
}

fn stmt(cdir: &Path) -> PathBuf {
    cdir.join("closeloop/toy/final/statement/statement.jsonl")
}

/// The single-walk reference: one campaign without layers on `start`.
fn single(tag: &str, start: &[[i32; 3]]) -> (Tmp, PathBuf) {
    let t = Tmp::new(tag);
    let m = material(&t.path.join("material"), start, None, &[], "");
    let sp = spec_of(&t, &m);
    let c = t.cdir("cdir");
    assert_eq!(
        run_to_end(&sp, &c, &["--plain"]).last(),
        Some(&0),
        "{}",
        log_tail(&c)
    );
    assert_eq!(result_of(&c)["verdict"], "closed");
    (t, c)
}

fn layer_cfg(dir: &Path) -> Value {
    json!({"order": "list", "list": [
        {"id": "low", "start": keys_file(dir, "low.jsonl", &[[3, 1, 0]])},
        {"id": "high", "start": keys_file(dir, "high.jsonl", &[[6, 1, 0]])}]})
}

fn layer_inputs(cdir: &Path) -> Vec<String> {
    let st = state(cdir);
    st.inputs
        .iter()
        .filter(|(_, r)| r.role == "closeloop_layer")
        .map(|(id, _)| id.clone())
        .collect()
}

#[test]
fn two_layers_compose_to_the_single_walk_and_closed_layers_are_frozen() {
    let _g = serial();
    let (_ts, cs) = single("lay1s", &[[3, 1, 0], [6, 1, 0]]);
    let t = Tmp::new("lay1l");
    let md = t.path.join("material");
    std::fs::create_dir_all(&md).unwrap();
    let m = material(&md, &[[3, 1, 0], [6, 1, 0]], Some(layer_cfg(&md)), &[], "");
    let sp = spec_of(&t, &m);
    let c = t.cdir("cdir");
    assert_eq!(
        run_to_end(&sp, &c, &["--plain"]).last(),
        Some(&0),
        "{}",
        log_tail(&c)
    );
    let r = result_of(&c);
    assert_eq!(r["verdict"], "closed", "{r:#}");
    let ls = r["layers"]["layers"].as_array().unwrap();
    assert_eq!(ls.len(), 2);
    assert_eq!(ls[0]["id"], "low");
    assert_eq!(ls[0]["keys"], 3); // [3,1,0], [2,1,0], [1,1,0]
    assert_eq!(ls[1]["keys"], 3); // [6..4,1,0]; [3,1,0] is a leaf
    assert_eq!(ls[1]["leaves_reached"], 1);
    assert_eq!(ls[1]["verdict"], "closed");
    // both layers frozen and recorded in the ledger
    let li = layer_inputs(&c);
    assert_eq!(li.len(), 2, "{li:?}");
    // the composed statement equals the single walk's (route-free) and is byte-identical (same key order? no: compare
    // as views)
    let cmp =
        rustred_campaign::closeloop::layers::compare_composed(&stmt(&cs), &[stmt(&c)]).unwrap();
    assert_eq!(cmp["equal"], true, "{cmp:#}");
    assert_eq!(cmp["single"]["in_start"], cmp["composed"]["in_start_union"]);
    // the node's leaf set: every key, certified, bound to the composed statement
    let lf = r["leaf_set"].clone();
    assert_eq!(lf["keys"], 6);
    assert_eq!(lf["uncertified"], 0);
    let set =
        rustred_campaign_core::closure::LeafSet::read_path(Path::new(lf["path"].as_str().unwrap()))
            .unwrap();
    assert_eq!(set.digest(), lf["digest"].as_str().unwrap());
    assert_eq!(set.binding().statement_digest, sha256_file(&stmt(&c)));
    assert!(set.is_closed());
    assert_eq!(verify(&c), 0);
}

#[test]
fn a_later_run_of_the_second_clause_closes_relative_to_the_first_runs_leaf_set() {
    let _g = serial();
    let (_ts, cs) = single("lay2s", &[[3, 1, 0], [6, 1, 0]]);
    // run 1: the base clause alone (one layer), writes its leaf set
    let t1 = Tmp::new("lay2a");
    let d1 = t1.path.join("material");
    std::fs::create_dir_all(&d1).unwrap();
    let base = json!({"order": "list", "list": [{"id": "low", "start": keys_file(&d1, "low.jsonl", &[[3, 1, 0]])}]});
    let m1 = material(&d1, &[[3, 1, 0]], Some(base), &[], "");
    let c1 = t1.cdir("cdir");
    assert_eq!(
        run_to_end(&spec_of(&t1, &m1), &c1, &["--plain"]).last(),
        Some(&0),
        "{}",
        log_tail(&c1)
    );
    let r1 = result_of(&c1);
    let lpath = PathBuf::from(r1["leaf_set"]["path"].as_str().unwrap());
    // run 2: the added clause alone, the first run's leaf set pinned as a prior clause's leaves
    let t2 = Tmp::new("lay2b");
    let d2 = t2.path.join("material");
    let m2 = material(&d2, &[[6, 1, 0]], None, &[], "");
    set_node(&m2, |n| {
        n["box_clause"] = json!("high");
        n["prior_clauses"] = json!([{"id": "low", "result": c1.join("closeloop/toy/final/result.json"),
            "leaf_set": {"path": lpath, "sha256": sha256_file(&lpath), "statement": stmt(&c1)}}]);
    });
    let c2 = t2.cdir("cdir");
    assert_eq!(
        run_to_end(&spec_of(&t2, &m2), &c2, &["--plain"]).last(),
        Some(&0),
        "{}",
        log_tail(&c2)
    );
    let r2 = result_of(&c2);
    assert_eq!(r2["verdict"], "closed", "{r2:#}");
    let digest = r1["leaf_set"]["digest"].as_str().unwrap();
    assert_eq!(r2["relative_to"], json!([digest]));
    assert!(
        r2["label"]
            .as_str()
            .unwrap()
            .contains("relative to leaf set"),
        "{}",
        r2["label"]
    );
    assert_eq!(r2["leaves"]["reached"], 1);
    // its own statement holds only its own keys; base + later == the single walk of the union
    let cmp =
        rustred_campaign::closeloop::layers::compare_composed(&stmt(&cs), &[stmt(&c1), stmt(&c2)])
            .unwrap();
    assert_eq!(cmp["equal"], true, "{cmp:#}");
    assert_eq!(cmp["composed"]["parts"][1]["keys"], 3);
    assert_eq!(verify(&c2), 0);
}

#[test]
fn a_changed_or_wrongly_pinned_leaf_set_and_another_table_sets_leaf_set_are_refused() {
    let _g = serial();
    let t1 = Tmp::new("lay3a");
    let d1 = t1.path.join("material");
    std::fs::create_dir_all(&d1).unwrap();
    let base = json!({"order": "list", "list": [{"id": "low", "start": keys_file(&d1, "low.jsonl", &[[3, 1, 0]])}]});
    let m1 = material(&d1, &[[3, 1, 0]], Some(base.clone()), &[], "");
    let c1 = t1.cdir("cdir");
    assert_eq!(
        run_to_end(&spec_of(&t1, &m1), &c1, &["--plain"]).last(),
        Some(&0),
        "{}",
        log_tail(&c1)
    );
    let lpath = PathBuf::from(result_of(&c1)["leaf_set"]["path"].as_str().unwrap());
    let later = |tag: &str, leaf: &Path, pinned: &str, coef: &str| -> (Tmp, PathBuf, i32) {
        let t = Tmp::new(tag);
        let m = material(&t.path.join("material"), &[[6, 1, 0]], None, &[], coef);
        set_node(&m, |n| {
            n["prior_clauses"] = json!([{"id": "low", "result": c1.join("closeloop/toy/final/result.json"),
                "leaf_set": {"path": leaf, "sha256": pinned}}]);
        });
        let c = t.cdir("cdir");
        let code = *run_to_end(&spec_of(&t, &m), &c, &["--plain"])
            .last()
            .unwrap();
        (t, c, code)
    };
    // (a) the honest file under a wrong pin
    let (_a, ca, code) = later("lay3b", &lpath, &"0".repeat(64), "");
    assert_ne!(code, 0);
    assert!(
        log_tail(&ca).contains("differs from the pinned")
            || format!("{:?}", records(&ca)).contains("differs from the pinned"),
        "{}",
        log_tail(&ca)
    );
    // (b) a changed byte (a flag), re-pinned: the leaf set's own digest refuses it
    let altered = t1.path.join("altered.leafset.v1");
    let mut b = std::fs::read(&lpath).unwrap();
    let n = b.len();
    b[n - 1] ^= 1;
    std::fs::write(&altered, &b).unwrap();
    let (_b2, cb, code) = later("lay3c", &altered, &sha256_file(&altered), "");
    assert_ne!(code, 0);
    assert!(
        format!("{:?}", records(&cb)).contains("digest"),
        "{}",
        log_tail(&cb)
    );
    // (c) the honest leaf set under ANOTHER table set (another coefficient file of table A): the walk binding refuses
    let (_c3, cc, code) = later("lay3d", &lpath, &sha256_file(&lpath), "x");
    assert_ne!(code, 0);
    let recs = format!("{:?}", records(&cc));
    assert!(
        recs.contains("walk components differ") && recs.contains("table_sources"),
        "{}",
        log_tail(&cc)
    );
}

#[test]
fn an_interrupt_between_layers_never_walks_a_finished_layer_again() {
    let _g = serial();
    let (_ts, cs) = single("lay4s", &[[3, 1, 0], [6, 1, 0]]);
    let t = Tmp::new("lay4i");
    let md = t.path.join("material");
    std::fs::create_dir_all(&md).unwrap();
    let m = material(&md, &[[3, 1, 0], [6, 1, 0]], Some(layer_cfg(&md)), &[], "");
    let sp = spec_of(&t, &m);
    let c = t.cdir("cdir");
    let mut ch = start(
        &sp,
        &c,
        &["--plain"],
        &[("RR_LAYERS_TEST_LAYER_DELAY_MS", "4000")],
    );
    // the first layer is frozen and recorded while the pass sleeps before the second
    wait_until("layer 0 recorded", Duration::from_secs(120), || {
        ledger_ready(&c) && layer_inputs(&c).len() == 1
    });
    signal(&ch, libc::SIGTERM);
    let code = wait(&mut ch, Duration::from_secs(120));
    assert_ne!(code, 0, "the interrupted session ended 0");
    assert_eq!(layer_inputs(&c), vec!["closeloop:toy:layer:0".to_string()]);
    let codes = run_to_end(&sp, &c, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{codes:?} {}", log_tail(&c));
    let r = result_of(&c);
    assert_eq!(r["verdict"], "closed");
    let ls = r["layers"]["layers"].as_array().unwrap();
    // layer 0 came back from the ledger (frozen), only layer 1 was walked after the resume
    assert_eq!(ls[0]["status"], "frozen", "{:#}", r["layers"]);
    assert_eq!(ls[1]["status"], "walked, frozen");
    assert_eq!(r["layers"]["walked"], 1);
    let cmp =
        rustred_campaign::closeloop::layers::compare_composed(&stmt(&cs), &[stmt(&c)]).unwrap();
    assert_eq!(cmp["equal"], true, "{cmp:#}");
    assert_eq!(verify(&c), 0);
}

#[test]
fn a_later_layer_reaching_an_open_key_of_an_earlier_layer_is_open_with_the_key_named() {
    let _g = serial();
    let t = Tmp::new("lay5o");
    let md = t.path.join("material");
    std::fs::create_dir_all(&md).unwrap();
    // no rule at [2,1,0]: layer `low` = {[2,1,0]} is open (uncovered); layer `high` walks [6..3] and meets it
    let lc = json!({"order": "list", "list": [
        {"id": "low", "start": keys_file(&md, "low.jsonl", &[[2, 1, 0]])},
        {"id": "high", "start": keys_file(&md, "high.jsonl", &[[6, 1, 0]])}]});
    let m = material(&md, &[[2, 1, 0], [6, 1, 0]], Some(lc), &[2], "");
    let sp = spec_of(&t, &m);
    let c = t.cdir("cdir");
    let codes = run_to_end(&sp, &c, &["--plain"]);
    assert_ne!(codes.last(), Some(&0), "{codes:?}");
    // the check of the first round: per layer
    let st = state(&c);
    let rec = st.inputs.get("closeloop:toy:check:r1").expect("check r1");
    let chk: Value = serde_json::from_slice(&std::fs::read(&rec.path).unwrap()).unwrap();
    let ls = chk["layers"]["layers"].as_array().unwrap();
    assert_eq!(ls[0]["verdict"], "open");
    assert_eq!(ls[1]["verdict"], "open", "{:#}", chk["layers"]);
    let why = format!("{}", ls[1]["blocking"]);
    assert!(
        why.contains("leaf_open:1") && why.contains("leaf_set_not_closed"),
        "{why}"
    );
    assert_eq!(ls[1]["leaf_open_keys"], json!([[2, 1, 0]]));
    // neither layer is frozen
    assert!(layer_inputs(&c).is_empty());
}
