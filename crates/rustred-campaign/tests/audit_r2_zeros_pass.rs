//! Independent audit of lane zeros2 (audit-r2-zeros, 2026-11-15): COVERAGE of the zero-term second route in the
//! closing loop's pass (`closeloop::pass::run_pass`, pure: no process), memory mode versus BIG-NODE mode (`scale`:
//! the spec's facts and the node's own answer files live in disk-backed evidence stores).
//!
//! In big-node mode every fact's basis is the word `store` (`Evidence::fact_basis`, FactRef::Store: the producer of
//! the stored fact is not kept), and `zeros::second_route_of("store")` is None. Consequences reproduced here:
//! - F6: a served pair of a record with ELIDED witness loci (first route = the exact evaluation) is put in closeloop4's
//!   `pairs` (so the zeros route skips it), then gets status `no_fact` (not blocking): its zero terms have NO second
//!   route, and the node closes on a false zero that memory mode blocks.
//! - F7: a pair whose stored facts come from the exact evaluation (`producer: exact_eval`) is NOT routed to rr_proof
//!   (closeloop.md 23.3: the zeros route runs on the same Symbolica algebra) but to the zeros route.
//!
//! `fN_*` tests are `#[ignore = "AUDIT-R2-ZEROS ..."]`: they assert the specified behaviour and FAIL on int/7 d60f3839.
//! `ok_*` tests pass. Toy node: arity 3, line mask 110 (lines n0, n1; numerator n2); nothing five-loop specific.

use rustred_campaign::closeloop::config::ScaleCfg;
use rustred_campaign::closeloop::pass::{PassIn, run_pass};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const HEADER: &str =
    r#"{"type":"header","schema":"table_dump.v1","family":"TOY","denominators":3,"mask":"110"}"#;
const CHART: &str = r#"{"type":"chart","chart":"110","guards":["n>=1","n>=1","n<=0"],"zero":false}"#;

fn sha(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

/// The term-key digest of the compact fact lines (core `import::lines::term_key_digest`).
fn digest(k: &[i32]) -> String {
    format!("{:016x}", rustred_campaign_core::import::lines::term_key_digest(k))
}

/// One point record at [3,1,0]: t0 -> [2,1,0] (a terminal) "1", t1 -> [3,1,1] "n2" (positive numerator, TRULY zero
/// at the key), t2 -> [2,1,1] "1+n2" (positive numerator, truly NON-ZERO: = 1). The first route (a compact
/// `producer: exact_eval` fact) lists only t0 as non-zero: t1 a true zero, t2 a FALSE zero.
fn material(dir: &Path, elided: bool) -> String {
    std::fs::create_dir_all(dir).unwrap();
    let witness = if elided { json!(["elided"]) } else { json!([]) };
    let rec = json!({"type":"rule","chart":"110","index":0,"fixed":[3,1,0],"equations":[],"target":"I(3,1,0)",
        "rhs":[["I(2,1,0)","1"],["I(3,1,1)","n2"],["I(2,1,1)","1+n2"]],
        "exceptions":[],"dispatch":"Partition","witness_loci": witness})
    .to_string();
    let t = format!("{HEADER}\n{CHART}\n{rec}\n{{\"type\":\"end\",\"charts\":1}}\n");
    std::fs::write(dir.join("A.table.jsonl"), &t).unwrap();
    let fact = json!({"schema":"rustred.campaign.fact-verdict.v1","producer":"exact_eval",
        "record_sha256": sha(rec.as_bytes()), "rule": 0, "key": [3,1,0], "verdict": "regular",
        "nonzero": [digest(&[2, 1, 0])], "unknown": [], "n_terms": 3});
    let facts = format!("{fact}\n");
    std::fs::write(dir.join("facts.jsonl"), &facts).unwrap();
    // the exact per-key proof at the key (an elided record is served only with it: closure PerKeyLift::Proof)
    let proofs = format!(
        "{}\n",
        json!({"record_sha256": sha(rec.as_bytes()), "index": 0, "key": [3,1,0], "verdict": "EXACT_IN_SPAN_AT_KEY"})
    );
    std::fs::write(dir.join("proofs.jsonl"), &proofs).unwrap();
    std::fs::write(dir.join("start.jsonl"), "[3,1,0]\n").unwrap();
    json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A","bits":6,
        "start":{"path": dir.join("start.jsonl").display().to_string(),"format":"keys_jsonl"},
        "required_evidence":"imported","policies":{"tier":"C"},
        "sources":[{"id":"A","kind":"engine_table","class":"base","path": dir.join("A.table.jsonl").display().to_string(),
                    "sha256": sha(t.as_bytes())},
                   {"id":"T","kind":"terminals","keys":[[2,1,0]]}],
        "evidence":{"facts":[{"path": dir.join("facts.jsonl").display().to_string(),
                              "sha256": sha(facts.as_bytes()), "family": "TOY"}],
                    "proofs":[{"path": dir.join("proofs.jsonl").display().to_string(),
                               "sha256": sha(proofs.as_bytes()), "family": "TOY"}]}})
    .to_string()
}

fn scale(dir: &Path) -> ScaleCfg {
    ScaleCfg {
        index_dir: Some(dir.join("index")),
        store_dir: Some(dir.join("store")),
        store_budget_mib: 16,
        store_threads: 1,
        store_chunk_files: 1,
        stream_keys_only: true,
        keys_only_first: true,
        require_pinned_evidence: true,
    }
}

/// One pass (default `second_route`: all_zeros ABSENT = structural); (verdict word or "none", blockers, check record).
fn pass(dir: &Path, elided: bool, big: bool) -> (String, Vec<String>, Value) {
    let spec = material(dir, elided);
    let out = run_pass(
        &PassIn {
            threads: 1,
            exact_tier: true,
            rows_path: dir.join("rows.jsonl"),
            scale: big.then(|| scale(dir)),
            ..PassIn::first("toy", spec, dir.to_path_buf(), dir.join("final"))
        },
        None,
    )
    .unwrap();
    let (verdict, blockers) = match &out.final_ {
        Some(f) => (
            f.verdict.clone(),
            f.result["blockers"]
                .as_array()
                .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                .unwrap_or_default(),
        ),
        None => ("none".to_string(), vec![]),
    };
    (verdict, blockers, out.check)
}

fn root(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("audit-r2-zeros-pass-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

/// Memory mode: the elided pair's first route is the exact evaluation; its second route (rr_proof, not configured
/// here) is unavailable: the node does NOT close on the false zero.
#[test]
fn ok_memory_mode_an_elided_pair_without_its_second_route_blocks() {
    let d = root("mem-elided");
    let (verdict, blockers, check) = pass(&d, true, false);
    let _ = std::fs::remove_dir_all(&d);
    assert_ne!(verdict, "closed", "{blockers:?} {:#}", check["second_route"]);
    assert!(
        blockers.iter().any(|b| b.starts_with("second_route_")),
        "{blockers:?} {:#}",
        check["second_route"]
    );
    assert_eq!(check["second_route"]["elided_pairs"], 1, "{:#}", check["second_route"]);
}

/// F6: the same material in BIG-NODE mode closes: the elided pair is `no_fact` (basis `store`), not blocking, and
/// skipped by the zeros route; the truly non-zero positive-numerator term t2 is dropped on ONE route.
#[test]
#[ignore = "AUDIT-R2-ZEROS F6: big-node mode (scale): elided pairs get no second route (basis 'store' -> no_fact), node closes"]
fn f6_big_node_mode_gives_an_elided_pair_its_second_route() {
    let d = root("big-elided");
    let (verdict, blockers, check) = pass(&d, true, true);
    let _ = std::fs::remove_dir_all(&d);
    assert_ne!(
        verdict, "closed",
        "big-node mode CLOSES on a first-route-only zero of an elided record: blockers {blockers:?}, second_route {:#}",
        check["second_route"]
    );
}

/// Memory mode: a non-elided pair whose first route is the exact evaluation is sent to rr_proof (closeloop.md 23.3,
/// `same_backend_to_rr_proof`), never to the zeros route (same Symbolica algebra).
#[test]
fn ok_memory_mode_exact_eval_first_pairs_go_to_rr_proof() {
    let d = root("mem-exact");
    let (verdict, _, check) = pass(&d, false, false);
    let _ = std::fs::remove_dir_all(&d);
    assert_ne!(verdict, "closed");
    let az = &check["second_route"]["all_zeros"];
    assert_eq!(az["same_backend_to_rr_proof"], 1, "{:#}", check["second_route"]);
    assert_eq!(az["pairs"], 0, "{az:#}");
}

/// F7: in BIG-NODE mode the same pair (stored facts of producer exact_eval) is sent to the zeros route (same backend:
/// rr_coef_facts on Symbolica), and a confirmation would be labelled `rr_proof + zeros:*` (zeros::zeros_basis).
#[test]
#[ignore = "AUDIT-R2-ZEROS F7: big-node mode: exact_eval facts in a store are confirmed by the same-backend zeros route"]
fn f7_big_node_mode_routes_exact_eval_first_pairs_to_rr_proof() {
    let d = root("big-exact");
    let (_, _, check) = pass(&d, false, true);
    let _ = std::fs::remove_dir_all(&d);
    let az = &check["second_route"]["all_zeros"];
    assert_eq!(
        az["same_backend_to_rr_proof"], 1,
        "the exact_eval-first pair is not routed to rr_proof in big-node mode: {:#}",
        check["second_route"]
    );
}
