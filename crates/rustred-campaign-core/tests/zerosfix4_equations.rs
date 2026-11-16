//! Lane zerosfix4 (the T162 box run of lane wireflow): a structural zero proof that used the record's CASE EQUATION
//! (`"equations": [0]`) is re-checked by the core loader as the prover derives it (the equation solved for a unit
//! parameter, core exact arithmetic) and becomes a proof; before, every such claim was "unchecked" (never a proof), so
//! the closing loop's zeros route answered `unknown` on 64 true zeros of T162 (bank#2093, bank#2099, tb-sc#100054).
//! Also: the record's constant parameters come from its target map (a permuted target), as in the prover.
//! Toy records (arity 4, mask 1110); nothing node-specific.

use rustred_campaign_core::import::zero_proofs::{
    LoadOptions, ZERO_PROOF_SCHEMA, ZeroProofs, ZeroStatus, claim_vanishes,
};
use rustred_campaign_core::import::{ImportedSet, TableSetSpec, load};
use rustred_campaign_core::poly::Poly;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const HEADER: &str =
    r#"{"type":"header","schema":"table_dump.v1","family":"TOY","denominators":4,"mask":"1110"}"#;
const CHART: &str =
    r#"{"type":"chart","chart":"1110","guards":["n>=1","n>=1","n>=1","n<=0"],"zero":false}"#;

fn sha(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("zerosfix4-eq-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// One record, its table and the set; returns (set, record sha256).
fn material(d: &Path, rec: &Value) -> (ImportedSet, String) {
    let rec = rec.to_string();
    let t = format!("{HEADER}\n{CHART}\n{rec}\n{{\"type\":\"end\",\"charts\":1}}\n");
    std::fs::write(d.join("A.table.jsonl"), &t).unwrap();
    std::fs::write(d.join("start.jsonl"), "[1,1,4,0]\n").unwrap();
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A","bits":6,
        "start":{"path": d.join("start.jsonl").display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"A","kind":"engine_table","class":"base","path": d.join("A.table.jsonl").display().to_string(),
                    "sha256": sha(t.as_bytes())}]});
    let set = load(&TableSetSpec::from_json(&spec.to_string()).unwrap(), d).unwrap();
    (set, sha(rec.as_bytes()))
}

fn proof(rsha: &str, term: u64, locus: Value, equations: Value) -> Value {
    json!({"schema": ZERO_PROOF_SCHEMA, "record_sha256": rsha, "rule": 0, "term": term, "locus": locus,
        "verdict": "zero_on_locus", "denominator": "nowhere_zero", "equations": equations,
        "method": "structural.v1", "tool": "rr_coef_facts@0.1.0"})
}

fn load_lines(set: &ImportedSet, d: &Path, lines: &[Value]) -> ZeroProofs {
    let p = d.join("proofs.jsonl");
    let body: String = lines.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(&p, body).unwrap();
    ZeroProofs::load(set, &[p], &LoadOptions::default()).unwrap()
}

/// The T162 shape: case equation n0 + 3 n1 - n2 + n3 = 0; t1 = (n0+3*n1-n2)/(d-4) is zero on the plane n3 = 0 ONLY
/// through the equation (n0 = -3 n1 + n2); t2 = (n0+3*n1+n2)/(d-4) is not (2 n2 remains): a false claim.
#[test]
fn a_claim_that_used_the_case_equation_is_a_proof_and_a_false_one_is_not() {
    let d = tmp("t162shape");
    let rec = json!({"type":"rule","chart":"1110","index":0,"fixed":[null,null,null,null],
        "equations":["n0+3*n1-n2+n3"],"target":"I(n0,n1,n2,n3)",
        "rhs":[["I(n0-1,n1,n2,n3)","1"],["I(n0,n1,n2,n3+1)","(n0+3*n1-n2)/(d-4)"],
               ["I(n0,n1-1,n2,n3+1)","(n0+3*n1+n2)/(d-4)"]],
        "exceptions":[],"dispatch":"Partition","witness_loci":[]});
    let (set, rsha) = material(&d, &rec);
    let z = load_lines(
        &set,
        &d,
        &[
            proof(&rsha, 1, json!({"plane": [3, 0]}), json!([0])),
            proof(&rsha, 2, json!({"plane": [3, 0]}), json!([0])),
        ],
    );
    // the key [1,1,4,0] satisfies the equation (1 + 3 - 4 + 0 = 0)
    assert_eq!(
        z.is_zero_term(&set, 0, 0, 1, &[1, 1, 4, 0]),
        ZeroStatus::Proved
    );
    assert_eq!(
        z.is_zero_term(&set, 0, 0, 2, &[1, 1, 4, 0]),
        ZeroStatus::NotProved
    );
    assert_eq!(z.stats.claims_with_equations, 2);
    assert_eq!(z.stats.claims_false, 1);
    assert_eq!(z.stats.claims_unchecked, 0);
    assert_eq!(z.stats.claims_confirmed, 1);
    let _ = std::fs::remove_dir_all(&d);
}

/// claim_vanishes directly: the bank#2093 numerator of T162 (-n9-3*n8+n6 on the plane n4 = 2 through the equation
/// -2+n9+3*n8-n6+n4 = 0), a non-linear equation (no unit solution: not used), an equation that is a non-zero constant
/// on the locus (no rule point: never a proof), a numerator zero without any equation.
#[test]
fn claim_vanishes_solves_unit_equations_like_the_prover() {
    let eq = |s: &str| Poly::parse_d_free_exact(s, 20).unwrap();
    assert!(claim_vanishes("-n9-3*n8+n6", 20, &[(4, 2)], &[eq("-2+n9+3*n8-n6+n4")]).unwrap());
    assert!(!claim_vanishes("-n9-3*n8+n6", 20, &[(4, 3)], &[eq("-2+n9+3*n8-n6+n4")]).unwrap());
    assert!(!claim_vanishes("-n9-3*n8+n6", 20, &[(4, 2)], &[]).unwrap());
    // no unit coefficient anywhere: the equation is not used
    assert!(!claim_vanishes("n1-n2", 4, &[], &[eq("2*n1-2*n2")]).unwrap());
    assert!(!claim_vanishes("n1-n2", 4, &[], &[eq("n1^2-n2")]).unwrap());
    // the equation 1 = 0 on the locus: no rule point there, never a proof
    assert!(!claim_vanishes("n1", 4, &[(2, 1)], &[eq("n2")]).unwrap());
    assert!(claim_vanishes("n3*(n1-1)", 4, &[(3, 0)], &[]).unwrap());
    // two equations solved one after the other (n0 = n1 + n2, then n1 = -n3 on the plane n3 = 1)
    assert!(claim_vanishes("n0-n2+1", 4, &[(3, 1)], &[eq("n0-n1-n2"), eq("n1+n3")]).unwrap());
}

/// The record's constants come from the TARGET MAP (as the prover): target I(n1,n0,n2,n3) with the key slot 0 fixed at
/// 3 makes the PARAMETER n1 the constant 3, so t1 = (n1-3)/(d-4) is zero on the whole domain.
#[test]
fn constants_follow_the_target_map() {
    let d = tmp("pattern");
    let rec = json!({"type":"rule","chart":"1110","index":0,"fixed":[3,null,null,null],"equations":[],
        "target":"I(n1,n0,n2,n3)",
        "rhs":[["I(n1-1,n0,n2,n3)","1"],["I(n1,n0,n2,n3+1)","(n1-3)/(d-4)"]],
        "exceptions":[],"dispatch":"Partition","witness_loci":[]});
    let (set, rsha) = material(&d, &rec);
    let z = load_lines(&set, &d, &[proof(&rsha, 1, json!("domain"), json!([]))]);
    assert_eq!(z.stats.claims_false, 0, "{:?}", z.false_claims);
    assert_eq!(z.stats.claims_confirmed, 1);
    let _ = std::fs::remove_dir_all(&d);
}
