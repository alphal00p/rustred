//! int7 (integration round 2, wire-in B2): the ZERO-FACT SOURCE of the ship gate. Lane pieces' `gate_check_with(FactSource)`
//! with lane zeros2's `ZeroProofFacts`: a spec WITHOUT zero proofs ships its positive-numerator terms whose coefficient
//! is not known to vanish as edges of their own class (`<class>|<server>|positive_numerator`, counted and listed); with
//! a structural zero proof of that term the gate drops it (a zero term is never an edge) and passes.
//!
//! Toy node (arity 3, lines 0 and 1, numerator slot 2; nothing five-loop specific): point rules on the chain
//! [k,1,0] -> [k-1,1,0] + hand-down [k,0,0], and at the top key one more term [3,1,1] (numerator slot at a POSITIVE
//! power: outside the chart) with the coefficient text `n2`, which vanishes on the rule point (n2 = 0): the proof line
//! says so (locus plane n2 = 0). Keys-only check (no coefficient facts: every coefficient is `Unknown` to the evidence).

use rustred_campaign_core::closure::Tier;
use rustred_campaign_core::import::check::check;
use rustred_campaign_core::import::zero_proofs::{
    LoadOptions, ZERO_PROOF_SCHEMA, ZeroProofFacts, ZeroProofs,
};
use rustred_campaign_core::import::{CheckOptions, Evidence, TableSetSpec, load};
use rustred_campaign_core::pieces::import_edges::{gate_check, gate_check_with};
use rustred_campaign_core::pieces::{GateConfig, GateVerdict};
use serde_json::json;
use std::path::Path;

const HEADER: &str =
    r#"{"type":"header","schema":"table_dump.v1","family":"TOY","denominators":3,"mask":"110"}"#;

fn sha(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

fn rule_line(index: i64, k: i32, extra: bool) -> String {
    let mut rhs = vec![
        json!([format!("I({},1,0)", k - 1), "1"]),
        json!([format!("I({k},0,0)"), "1"]),
    ];
    if extra {
        rhs.push(json!([format!("I({k},1,1)"), "n2"]));
    }
    json!({"type":"rule","chart":"110","index":index,"fixed":[k,1,0],"equations":[],
        "target": format!("I({k},1,0)"), "rhs": rhs,
        "exceptions":[],"dispatch":"Partition","witness_loci":[]})
    .to_string()
}

#[test]
fn positive_numerator_unknowns_are_their_own_class_without_zero_proofs_and_the_gate_passes_with_them() {
    let dir = std::env::temp_dir().join(format!("int7-gate-zp-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // [3,1,0] (with the positive-numerator term) -> [2,1,0] -> [1,1,0] (hand-downs only below)
    let lines = [rule_line(0, 3, true), rule_line(1, 2, false)];
    let mut t = format!("{HEADER}\n");
    t.push_str(r#"{"type":"chart","chart":"110","guards":["n>=1","n>=1","n<=0"],"zero":false}"#);
    t.push('\n');
    for l in &lines {
        t.push_str(l);
        t.push('\n');
    }
    t.push_str("{\"type\":\"end\",\"charts\":1}\n");
    let tp = dir.join("A.table.jsonl");
    std::fs::write(&tp, &t).unwrap();
    std::fs::write(dir.join("start.jsonl"), "[3,1,0]\n").unwrap();
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A","bits":6,
        "start":{"path": dir.join("start.jsonl").display().to_string(),"format":"keys_jsonl"},
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

    // WITHOUT zero proofs: the positive-numerator term is an edge of its own class
    let (r0, s0) = gate_check(&set, &ev, &res, GateConfig::memory()).unwrap();
    let pos0: Vec<(&String, &[u64; 2])> = r0
        .by_provenance
        .iter()
        .filter(|(k, _)| k.ends_with("|positive_numerator"))
        .collect();
    assert_eq!(pos0.len(), 1, "one positive-numerator class: {:?}", r0.by_provenance);
    assert_eq!(
        pos0[0].1[0],
        1,
        "its one edge: {:?} zero {} edges {} relations {} {s0:?}",
        r0.by_provenance,
        r0.zero_terms_skipped,
        r0.edges,
        r0.relations
    );
    assert_eq!(r0.zero_terms_skipped, 0);
    assert!(s0.rule_rows >= 2, "{s0:?}");
    // the edge leaves the chart: never lower than the key under the node order, so the gate does not pass
    assert_eq!(pos0[0].1[1], 1, "the positive-numerator edge is non-descending");
    assert_ne!(r0.verdict, GateVerdict::Pass, "{:?}", r0.verdict);
    assert!(
        r0.non_descending_listed
            .iter()
            .any(|e| e.provenance.ends_with("|positive_numerator")),
        "the offender is listed with its class: {:?}",
        r0.non_descending_listed
    );

    // WITH the zero proof of that term (record sha256, term 2, locus n2 = 0, denominator nowhere zero)
    let proof = json!({"schema": ZERO_PROOF_SCHEMA, "record_sha256": sha(lines[0].as_bytes()), "rule": 0, "term": 2,
        "locus": {"plane": [2, 0]}, "verdict": "zero_on_locus", "denominator": "nowhere_zero",
        "tool": "int7-test@1"});
    let pp = dir.join("toy.zero-proofs.jsonl");
    std::fs::write(&pp, format!("{proof}\n")).unwrap();
    let zp = ZeroProofs::load(&set, &[pp], &LoadOptions::default()).unwrap();
    assert_eq!(zp.stats.proved, 1);
    let facts = ZeroProofFacts::new(&set, &zp, &ev);
    let (r1, _) = gate_check_with(&set, &ev, &facts, &res, GateConfig::memory()).unwrap();
    // the class may still be named (the relation is split by term class), with no edge left in it
    assert!(
        r1.by_provenance
            .iter()
            .filter(|(k, _)| k.ends_with("|positive_numerator"))
            .all(|(_, v)| *v == [0, 0]),
        "{:?}",
        r1.by_provenance
    );
    assert_eq!(r1.zero_terms_skipped, 1);
    assert_eq!(r1.edges + 1, r0.edges);
    assert_eq!(r1.verdict, GateVerdict::Pass, "{:?}", r1.verdict);
    let _ = std::fs::remove_dir_all(&dir);
}
