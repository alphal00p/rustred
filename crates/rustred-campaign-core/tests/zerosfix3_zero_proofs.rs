//! Lane zerosfix3 (audit-r2-zeros F1-F3, M3, M5): the core zero-proof loader checks every proved CLAIM against the
//! record's own text. Toy record (arity 3, mask 110): t0 "1", t1 "n2" (zero on n2 = 0), t2 "(n2*(n0-3))/(n0-3)"
//! (zero numerator on n2 = 0, denominator vanishing at n0 = 3), t3 "(n2*(n2+1))/(d-4)" (zero on n2 = 0 and n2 = -1,
//! denominator with a constant d-coefficient). Nothing node-specific.

use rustred_campaign_core::import::zero_proofs::{
    LoadOptions, ZERO_PROOF_SCHEMA, ZeroProofRefusal, ZeroProofs, ZeroStatus,
    constant_d_coefficient_after, vanishes_after,
};
use rustred_campaign_core::import::{ImportedSet, TableSetSpec, load};
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
    let d = std::env::temp_dir().join(format!("zerosfix3-zp-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn material(d: &Path) -> (ImportedSet, String) {
    let rec = json!({"type":"rule","chart":"110","index":0,"fixed":[null,null,null],"equations":[],
        "target":"I(n0,n1,n2)",
        "rhs":[["I(n0-1,n1,n2)","1"],["I(n0,n1,n2+1)","n2"],["I(n0-1,n1,n2+1)","(n2*(n0-3))/(n0-3)"],
               ["I(n0,n1,n2+2)","(n2*(n2+1))/(d-4)"]],
        "exceptions":[],"dispatch":"Partition","witness_loci":[]})
    .to_string();
    let t = format!("{HEADER}\n{CHART}\n{rec}\n{{\"type\":\"end\",\"charts\":1}}\n");
    std::fs::write(d.join("A.table.jsonl"), &t).unwrap();
    std::fs::write(d.join("start.jsonl"), "[3,1,0]\n").unwrap();
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A","bits":6,
        "start":{"path": d.join("start.jsonl").display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"A","kind":"engine_table","class":"base","path": d.join("A.table.jsonl").display().to_string(),
                    "sha256": sha(t.as_bytes())}]});
    let set = load(&TableSetSpec::from_json(&spec.to_string()).unwrap(), d).unwrap();
    (set, sha(rec.as_bytes()))
}

fn line(rsha: &str, term: u64, locus: Value, verdict: &str, den: Option<&str>) -> Value {
    let mut v = json!({"schema": ZERO_PROOF_SCHEMA, "record_sha256": rsha, "rule": 0, "term": term,
        "locus": locus, "verdict": verdict, "method": "structural.v1", "tool": "rr_coef_facts@0.1.0"});
    if let Some(x) = den {
        v["denominator"] = json!(x);
    }
    v
}

fn plane(p: u32, v: i64) -> Value {
    json!({"plane": [p, v]})
}

fn load_lines(
    set: &ImportedSet,
    d: &Path,
    lines: &[Value],
) -> Result<ZeroProofs, ZeroProofRefusal> {
    let p = d.join("proofs.jsonl");
    let body: String = lines.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(&p, body).unwrap();
    ZeroProofs::load(set, &[p], &LoadOptions::default()).map_err(|e| e.reason)
}

#[test]
fn the_core_checks_of_a_claim_substitute_exactly() {
    assert!(vanishes_after("n2*(n0-3)", 3, &[(2, 0)]).unwrap());
    assert!(!vanishes_after("1+n2", 3, &[(2, 0)]).unwrap());
    assert!(vanishes_after("n2^2+n2", 3, &[(2, -1)]).unwrap());
    assert!(!vanishes_after("n2^2-n2", 3, &[(2, -1)]).unwrap());
    // d - 4: constant d^1 coefficient; n0 - 3 and n0*d: none (free index at every power of d)
    assert!(constant_d_coefficient_after("d-4", 3, &[(2, 0)]).unwrap());
    assert!(!constant_d_coefficient_after("n0-3", 3, &[(2, 0)]).unwrap());
    assert!(!constant_d_coefficient_after("n0*d+n1", 3, &[]).unwrap());
    assert!(constant_d_coefficient_after("n0*d+n1", 3, &[(0, 2)]).unwrap());
}

/// Honest lines are held; a FALSE claim, a where_regular line with another den text, a claim that used case equations
/// are never proofs (counted); a forged `nowhere_zero` on the term whose denominator vanishes at n0 = 3 is re-checked
/// by the core at the key (M3): 0/0 there, a proof elsewhere.
#[test]
fn claims_are_checked_against_the_record_text() {
    let d = tmp("claims");
    let (set, rsha) = material(&d);
    let mut eqs = line(
        &rsha,
        3,
        plane(2, -1),
        "zero_on_locus",
        Some("nowhere_zero"),
    );
    eqs["equations"] = json!([0]);
    let mut wrong_den = line(
        &rsha,
        2,
        plane(2, 0),
        "zero_on_locus",
        Some("where_regular"),
    );
    wrong_den["den"] = json!("(n0-4)");
    let z = load_lines(
        &set,
        &d,
        &[
            line(&rsha, 1, plane(2, 0), "zero_on_locus", Some("nowhere_zero")),
            line(&rsha, 3, plane(2, 0), "zero_on_locus", Some("nowhere_zero")),
            // false: t0 = 1 does not vanish
            line(&rsha, 0, plane(2, 0), "zero_on_locus", Some("nowhere_zero")),
            wrong_den,
            eqs,
        ],
    )
    .unwrap();
    let st = |term: usize, k: [i32; 3]| z.is_zero_term(&set, 0, 0, term, &k);
    assert_eq!(st(1, [3, 1, 0]), ZeroStatus::Proved);
    assert_eq!(st(3, [3, 1, 0]), ZeroStatus::Proved);
    assert_eq!(st(0, [3, 1, 0]), ZeroStatus::NotProved);
    assert_eq!(st(2, [4, 1, 0]), ZeroStatus::NotProved, "another den text");
    // lane zerosfix4: a claim that names case equations is re-checked like any other (here the numerator vanishes
    // on n2 = -1 by itself): a proof (was: unchecked)
    assert_eq!(
        st(3, [3, 1, -1]),
        ZeroStatus::Proved,
        "equations: re-checked"
    );
    assert_eq!(z.stats.claims_false, 1);
    assert_eq!(z.false_claims.len(), 1);
    assert!(matches!(
        z.false_claims[0].reason,
        ZeroProofRefusal::NumeratorDoesNotVanish { term: 0, .. }
    ));
    assert_eq!(z.stats.den_text_mismatch, 1);
    assert_eq!(z.stats.claims_unchecked, 0);
    assert_eq!(z.stats.claims_with_equations, 1);
    assert_eq!(z.stats.claims_confirmed, 4);
    assert_eq!(z.stats.den_independent, 3, "t1 (no den), t3 (d - 4) twice");
    // a forged nowhere_zero on t2: the core evaluates the record's denominator at each key
    let z = load_lines(
        &set,
        &d,
        &[line(
            &rsha,
            2,
            plane(2, 0),
            "zero_on_locus",
            Some("nowhere_zero"),
        )],
    )
    .unwrap();
    assert_eq!(z.stats.den_checked_at_key, 1);
    assert_eq!(
        z.is_zero_term(&set, 0, 0, 2, &[3, 1, 0]),
        ZeroStatus::DenominatorVanishes
    );
    assert_eq!(
        z.is_zero_term(&set, 0, 0, 2, &[4, 1, 0]),
        ZeroStatus::Proved
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// Typed refusals (M5): contradicting lines of one locus (a proved claim beside another statement), a term beyond the
/// record, a wrong rule echo; two statements that prove nothing (not_proved / excluded) and an identical repeat are
/// harmless.
#[test]
fn refusals_are_typed() {
    let d = tmp("typed");
    let (set, rsha) = material(&d);
    let a = line(&rsha, 1, plane(2, 0), "zero_on_locus", Some("nowhere_zero"));
    let mut b = a.clone();
    b["denominator"] = json!("where_regular");
    b["den"] = json!("1");
    assert!(matches!(
        load_lines(&set, &d, &[a.clone(), b]),
        Err(ZeroProofRefusal::ContradictingLines { term: 1, .. })
    ));
    assert!(matches!(
        load_lines(
            &set,
            &d,
            &[a.clone(), line(&rsha, 1, plane(2, 0), "not_proved", None)]
        ),
        Err(ZeroProofRefusal::ContradictingLines { .. })
    ));
    let mut far = a.clone();
    far["term"] = json!(9);
    assert!(matches!(
        load_lines(&set, &d, &[far]),
        Err(ZeroProofRefusal::TermOutOfRange { term: 9, .. })
    ));
    let mut echo = a.clone();
    echo["rule"] = json!(5);
    assert!(matches!(
        load_lines(&set, &d, &[echo]),
        Err(ZeroProofRefusal::RuleEcho { .. })
    ));
    let z = load_lines(
        &set,
        &d,
        &[
            a.clone(),
            a,
            line(&rsha, 0, plane(2, 0), "not_proved", None),
            line(&rsha, 0, plane(2, 0), "excluded", None),
        ],
    )
    .unwrap();
    assert_eq!(z.len(), 1);
    let _ = std::fs::remove_dir_all(&d);
}
