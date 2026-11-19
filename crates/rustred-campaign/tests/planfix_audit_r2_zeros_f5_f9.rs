//! planfix: the audit's F5 and F9 reproductions (audit/r2-zeros baefd399, WS/notes/implementer.audit-r2-zeros.md:
//! tests/audit_r2_zeros_forgery.rs `f5_*` and tests/audit_r2_zeros_pass.rs `f9_*`, copied verbatim with their helpers
//! and UN-IGNORED here: lane zerosfix3 merges the audit files themselves (where these two stay ignored until the
//! integration step removes the duplicates)). F5 fixed by the ship gate's two-route rule (planfix G1) and by
//! `import::zero_proofs::VerifiedZeroProofs` (an unverified proof file gives no fact); F9 by the leaf binding's zero-route
//! guarantees (core `LeafProducer::zero_route`, closing loop `layers::zero_route_words`).
//! Toy material only (arity 3, line mask 110); nothing five-loop specific.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

use rustred_campaign::closeloop::pass::{PassIn, run_pass};
use rustred_campaign_core::closure::Tier;
use rustred_campaign_core::import::check::check;
use rustred_campaign_core::import::zero_proofs::{
    LoadOptions, ZERO_PROOF_SCHEMA, ZeroProofFacts, ZeroProofs,
};
use rustred_campaign_core::import::{CheckOptions, Evidence, TableSetSpec, load};
use rustred_campaign_core::pieces::import_edges::{gate_check, gate_check_with};
use rustred_campaign_core::pieces::{GateConfig, GateVerdict};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const HEADER: &str =
    r#"{"type":"header","schema":"table_dump.v1","family":"TOY","denominators":3,"mask":"110"}"#;
const CHART: &str =
    r#"{"type":"chart","chart":"110","guards":["n>=1","n>=1","n<=0"],"zero":false}"#;

fn sha(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("planfix-r2z-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn root(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("planfix-r2z-pass-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn write_lines(dir: &Path, name: &str, lines: &[Value]) -> PathBuf {
    let p = dir.join(name);
    let mut s = String::new();
    for l in lines {
        s.push_str(&l.to_string());
        s.push('\n');
    }
    std::fs::write(&p, s).unwrap();
    p
}

/// Two point records on ONE table (shared by both runs, so the walk binding of a leaf set matches): R3 at [3,1,0] as in
/// `material` (t2 a FALSE zero of the first route, rr_proof instantiate facts), R4 at [4,1,0] -> [3,1,0] (non-zero).
fn material2(dir: &Path, start: &str) -> String {
    let shared = dir.join("shared");
    std::fs::create_dir_all(&shared).unwrap();
    let r3 = json!({"type":"rule","chart":"110","index":0,"fixed":[3,1,0],"equations":[],"target":"I(3,1,0)",
        "rhs":[["I(2,1,0)","1"],["I(3,1,1)","n2"],["I(2,1,1)","1+n2"]],
        "exceptions":[],"dispatch":"Partition","witness_loci":[]})
    .to_string();
    let r4 = json!({"type":"rule","chart":"110","index":1,"fixed":[4,1,0],"equations":[],"target":"I(4,1,0)",
        "rhs":[["I(3,1,0)","1"]],"exceptions":[],"dispatch":"Partition","witness_loci":[]})
    .to_string();
    let t = format!("{HEADER}\n{CHART}\n{r3}\n{r4}\n{{\"type\":\"end\",\"charts\":1}}\n");
    std::fs::write(shared.join("A.table.jsonl"), &t).unwrap();
    let inst = |rec: &str, idx: i64, key: [i32; 3], rhs: Value| {
        json!({"schema":"rustred.proof-kernel.instantiate.v1","record_sha256": sha(rec.as_bytes()), "rule": idx,
            "key": key, "in_case": true, "chart_ok": true, "branch": null, "regular": "regular", "irregular_terms": [],
            "hidden_loci": [], "rhs": rhs, "zero_test": "exact", "error": null})
        .to_string()
    };
    let facts = format!(
        "{}\n{}\n",
        inst(
            &r3,
            0,
            [3, 1, 0],
            json!([{"term":0,"key":[2,1,0],"coef":"nonzero"},
            {"term":1,"key":[3,1,1],"coef":"zero"},{"term":2,"key":[2,1,1],"coef":"zero"}])
        ),
        inst(
            &r4,
            1,
            [4, 1, 0],
            json!([{"term":0,"key":[3,1,0],"coef":"nonzero"}])
        )
    );
    std::fs::write(shared.join("facts.jsonl"), &facts).unwrap();
    let proofs = format!(
        "{}\n{}\n",
        json!({"record_sha256": sha(r3.as_bytes()), "index": 0, "key": [3,1,0], "verdict": "EXACT_IN_SPAN_AT_KEY"}),
        json!({"record_sha256": sha(r4.as_bytes()), "index": 1, "key": [4,1,0], "verdict": "EXACT_IN_SPAN_AT_KEY"})
    );
    std::fs::write(shared.join("proofs.jsonl"), &proofs).unwrap();
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("start.jsonl"), format!("{start}\n")).unwrap();
    json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A","bits":6,
        "start":{"path": dir.join("start.jsonl").display().to_string(),"format":"keys_jsonl"},
        "required_evidence":"imported","policies":{"tier":"C"},
        "sources":[{"id":"A","kind":"engine_table","class":"base","path": shared.join("A.table.jsonl").display().to_string(),
                    "sha256": sha(t.as_bytes())},
                   {"id":"T","kind":"terminals","keys":[[2,1,0]]}],
        "evidence":{"facts":[{"path": shared.join("facts.jsonl").display().to_string(),
                              "sha256": sha(facts.as_bytes()), "family": "TOY"}],
                    "proofs":[{"path": shared.join("proofs.jsonl").display().to_string(),
                               "sha256": sha(proofs.as_bytes()), "family": "TOY"}]}})
    .to_string()
}

/// F9: a run with `all_zeros: off` (explicit) closes [3,1,0] on the false zero and writes its leaf set; a LATER run
/// with the DEFAULT second route (structural) takes that leaf set as a prior clause and closes relative to it: the leaf
/// set (core closure/leaves.rs LeafBinding) and the frozen-layer record (layers.rs LayerRecord) carry no zero-route
/// mode, so a default-on run inherits zeros that were confirmed by no second route.
/// F9: a run with `all_zeros: off` (explicit) closes [3,1,0] on the false zero and writes its leaf set; a LATER run
/// with the DEFAULT second route (structural) takes that leaf set as a prior clause and closes relative to it: the leaf
/// set (core closure/leaves.rs LeafBinding) and the frozen-layer record (layers.rs LayerRecord) carry no zero-route
/// mode, so a default-on run inherits zeros that were confirmed by no second route.
#[test]
fn f9_a_default_run_refuses_a_leaf_set_closed_without_the_zero_route() {
    use rustred_campaign::closeloop::config::{AllZeros, LayersCfg, LeafSetRef, SecondRouteCfg};
    let root = root("leafset");
    let da = root.join("a");
    let spec_a = material2(&root, "[3,1,0]");
    std::fs::create_dir_all(&da).unwrap();
    let out_a = run_pass(
        &PassIn {
            threads: 1,
            exact_tier: true,
            rows_path: da.join("rows.jsonl"),
            second_route: SecondRouteCfg {
                all_zeros: Some(AllZeros::Off),
                ..SecondRouteCfg::default()
            },
            layers: Some(LayersCfg {
                order: "coarse".into(),
                list: vec![],
                freeze: false,
            }),
            ..PassIn::first("toy", spec_a, da.clone(), da.join("final"))
        },
        None,
    )
    .unwrap();
    let fa = out_a.final_.expect("run A final");
    assert_eq!(
        fa.verdict, "closed",
        "run A (all_zeros off) closes: {:#}",
        fa.result["blockers"]
    );
    let ls = &fa.result["leaf_set"];
    let lref = LeafSetRef {
        path: PathBuf::from(ls["path"].as_str().expect("leaf set written")),
        sha256: ls["sha256"].as_str().unwrap().to_string(),
        statement: None,
    };
    let db = root.join("b");
    std::fs::create_dir_all(&db).unwrap();
    let spec_b = material2(&root.join("bspec"), "[4,1,0]");
    let out_b = run_pass(
        &PassIn {
            threads: 1,
            exact_tier: true,
            rows_path: db.join("rows.jsonl"),
            prior_leaf_sets: vec![("A".into(), lref)],
            ..PassIn::first("toy", spec_b, db.clone(), db.join("final"))
        },
        None,
    );
    let _ = std::fs::remove_dir_all(&root);
    let out_b = out_b.expect("run B pass");
    let fb = out_b.final_.expect("run B final");
    assert_ne!(
        fb.verdict, "closed",
        "the default-on run CLOSES relative to a leaf set certified with all_zeros=off: {:#}",
        fb.result["relative_to"]
    );
    // planfix: refused for the right reason (the typed word of the leaf binding's zero-route guarantees)
    assert!(
        fb.result.to_string().contains("leaf_zero_route_weaker"),
        "{:#}",
        fb.result["layers"]
    );
}

/// F5 (ship gate, consequence of F1): int7_gate_zero_proofs' toy with the positive-numerator term's coefficient "1"
/// (truly non-zero): a forged proof line drops the term and the gate PASSES.
#[test]
fn f5_ship_gate_keeps_a_nonzero_positive_numerator_term_despite_a_forged_proof() {
    let d = tmp("f5");
    let rule_line = |index: i64, k: i32, extra: bool| -> String {
        let mut rhs = vec![
            json!([format!("I({},1,0)", k - 1), "1"]),
            json!([format!("I({k},0,0)"), "1"]),
        ];
        if extra {
            rhs.push(json!([format!("I({k},1,1)"), "1"]));
        }
        json!({"type":"rule","chart":"110","index":index,"fixed":[k,1,0],"equations":[],
            "target": format!("I({k},1,0)"), "rhs": rhs, "exceptions":[],"dispatch":"Partition","witness_loci":[]})
        .to_string()
    };
    let lines = [rule_line(0, 3, true), rule_line(1, 2, false)];
    let mut t = format!("{HEADER}\n{CHART}\n");
    for l in &lines {
        t.push_str(l);
        t.push('\n');
    }
    t.push_str("{\"type\":\"end\",\"charts\":1}\n");
    let tp = d.join("A.table.jsonl");
    std::fs::write(&tp, &t).unwrap();
    std::fs::write(d.join("start.jsonl"), "[3,1,0]\n").unwrap();
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A","bits":6,
        "start":{"path": d.join("start.jsonl").display().to_string(),"format":"keys_jsonl"},
        "required_evidence":"imported","policies":{"tier":"K","witness_check":"off_diagnostic"},
        "sources":[{"id":"A","kind":"engine_table","class":"base","path": tp.display().to_string(),
                    "sha256": sha(t.as_bytes())}]});
    let spec = TableSetSpec::from_json(&spec.to_string()).unwrap();
    let set = load(&spec, Path::new("/")).unwrap();
    let ev = Evidence::from_spec(&set, Path::new("/")).unwrap();
    let res = check(
        &set,
        &ev,
        &CheckOptions {
            threads: 1,
            tier: Some(Tier::K),
            max_rounds: None,
        },
    )
    .unwrap();
    let (r0, _) = gate_check(&set, &ev, &res, GateConfig::memory()).unwrap();
    assert_ne!(
        r0.verdict,
        GateVerdict::Pass,
        "without proofs the positive-numerator edge blocks the gate"
    );
    let forged = json!({"schema": ZERO_PROOF_SCHEMA, "record_sha256": sha(lines[0].as_bytes()), "rule": 0, "term": 2,
        "locus": "domain", "verdict": "zero_on_locus", "denominator": "nowhere_zero", "tool": "forger@1"});
    let pp = write_lines(&d, "forged.zero-proofs.jsonl", &[forged]);
    let verdict_with_forgery = match ZeroProofs::load(&set, &[pp], &LoadOptions::default()) {
        Err(_) => None,
        Ok(zp) => {
            let facts = ZeroProofFacts::new(&set, &zp, &ev);
            Some(
                gate_check_with(&set, &ev, &facts, &res, GateConfig::memory())
                    .unwrap()
                    .0
                    .verdict,
            )
        }
    };
    let _ = std::fs::remove_dir_all(&d);
    assert_ne!(
        verdict_with_forgery,
        Some(GateVerdict::Pass),
        "the gate passes on a forged proof of a coefficient that is the constant 1"
    );
}
