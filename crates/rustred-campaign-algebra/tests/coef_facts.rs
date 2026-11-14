//! Lane closeloop3: the Symbolica exact evaluation of coefficient facts (`src/coef_facts.rs`) against the campaign
//! core's own exact evaluation (`accept::facts::ExactRecordFacts`, `poly::Poly`): two independent routes, equal facts
//! on records covering zero / non-zero / vanishing-denominator terms, refused text forms, listed / vanishing / elided /
//! absent witness loci, an exception branch and keys outside the case. Needs the Symbolica licence (`source
//! WS/ops/env.sh`); SKIPS without it.

#[path = "../src/coef_facts.rs"]
#[allow(dead_code)]
mod coef_facts;

use coef_facts::{SymbolicaRecordFacts, fact_line, for_each_pair, parse_requests};
use rustred_campaign_algebra::SymbolicaAlgebra;
use rustred_campaign_core::accept::facts::{CoefficientFacts, ExactRecordFacts, RegVerdict, Regularity};
use rustred_campaign_core::accept::rule::RuleText;
use rustred_campaign_core::closure::CoefFact;
use rustred_campaign_core::key::NodeFrame;
use rustred_campaign_core::record::TableHeader;
use serde_json::{Value, json};

const HEADER: &str = r#"{"type":"header","schema":"table_dump.v1","family":"TOY","denominators":3,"mask":"110","guard_version":2}"#;

fn frame() -> NodeFrame {
    let h = TableHeader::from_value(serde_json::from_str(HEADER).unwrap()).unwrap();
    NodeFrame::from_header(&h).unwrap()
}

fn backend() -> Option<SymbolicaRecordFacts> {
    match SymbolicaAlgebra::new() {
        Ok(a) => Some(SymbolicaRecordFacts::new(a).with_threads(2)),
        Err(e) => {
            eprintln!("SKIP: {e}");
            None
        }
    }
}

/// A generic rule over n0 (line), n1 (line, fixed 1), n2 (numerator).
fn rule(rhs: &[(&str, &str)], loci: Value, exceptions: Value) -> String {
    json!({"type":"rule","chart":"110","index":7,"fixed":[null,1,null],"equations":[],"target":"I(n0,1,n2)",
        "rhs": rhs.iter().map(|(k, c)| json!([k, c])).collect::<Vec<_>>(),
        "exceptions": exceptions, "dispatch":"Partition","witness_loci": loci})
    .to_string()
}

fn records() -> Vec<String> {
    let rhs: &[(&str, &str)] = &[
        ("I(n0-1,1,n2)", "(d-4)/(d-3)"),
        // zero at n0 = 3 (any d), non-zero elsewhere
        ("I(n0,1,n2+1)", "((n0-3)*(d-2))/(d-1)"),
        // denominator vanishes identically at n0 = 2: unknown, irregular there
        ("I(n0,0,n2)", "(d+n0)/(n0-2)"),
        // identically zero for every key and d (expanded: n0*d - d*n0)
        ("I(n0-1,1,n2+1)", "(n0*d-d*n0)"),
        // a monomial over a monomial
        ("I(n0,1,n2-1)", "-2*n0*d/n2"),
        // a form split_ratio refuses (P + M/(Q)): unknown, never guessed
        ("I(n0-2,1,n2)", "d + 1/(d-5)"),
        // numerator vanishing only at a specific d (not identically): non-zero
        ("I(n0,1,n2-2)", "(d-2*n0)"),
    ];
    vec![
        rule(rhs, json!(["elided"]), json!([])),
        rule(rhs, json!(["n0-4", "d-2*n0"]), json!([])),
        rule(rhs, json!([]), json!([["n0-6"]])),
        rule(rhs, Value::Null, json!([])),
    ]
}

fn keys() -> Vec<Vec<i32>> {
    let mut v = vec![];
    for n0 in 1..=7 {
        for n2 in [-3, -2, -1, 0] {
            v.push(vec![n0, 1, n2]);
        }
    }
    v.push(vec![3, 2, 0]); // outside the case (fixed slot)
    v
}

#[test]
fn symbolica_facts_equal_the_core_exact_facts_on_every_term() {
    let Some(mut sym) = backend() else { return };
    let mut core = ExactRecordFacts::new();
    let f = frame();
    let (mut n, mut zero, mut nonzero, mut unknown) = (0, 0, 0, 0);
    for line in records() {
        let rt = RuleText::parse(line.as_bytes(), &f).unwrap();
        for k in keys() {
            if !matches!(rt.domain.as_ref().unwrap().applies(&k), rustred_campaign_core::domain::Verdict::Applies
                | rustred_campaign_core::domain::Verdict::Elided
                | rustred_campaign_core::domain::Verdict::OnPole { .. })
            {
                continue;
            }
            for j in 0..rt.rhs.len() {
                let a = sym.coefficient(&rt, &k, j);
                let b = core.coefficient(&rt, &k, j);
                assert_eq!(a, b, "record {line}\nkey {k:?} term {j}");
                n += 1;
                match a {
                    CoefFact::Zero => zero += 1,
                    CoefFact::NonZero => nonzero += 1,
                    CoefFact::Unknown => unknown += 1,
                }
            }
            // regularity: a vanishing denominator is a pole for both; listed loci decide hidden / regular alike
            let (ra, rb) = (sym.regular(&rt, &k), core.regular(&rt, &k));
            match (&ra, &rb) {
                (RegVerdict::Pole { term: x }, RegVerdict::Pole { term: y }) => assert_eq!(x, y),
                (RegVerdict::Pole { .. }, _) | (_, RegVerdict::Pole { .. }) => {
                    // the refused text form (term 5) makes ours Unknown before a later pole: both must be non-regular
                    assert!(!matches!(ra, RegVerdict::Regular) && !matches!(rb, RegVerdict::Regular), "{ra:?} {rb:?}")
                }
                _ => {}
            }
        }
    }
    assert!(n > 300, "{n}");
    assert!(zero > 20 && nonzero > 100 && unknown > 20, "{zero} {nonzero} {unknown}");
}

#[test]
fn key_facts_verdicts_and_fact_lines() {
    let Some(mut sym) = backend() else { return };
    let f = frame();
    let recs = records();
    // elided loci: hidden (regularity from the bound per-key proof), terms decided
    let rt = RuleText::parse(recs[0].as_bytes(), &f).unwrap();
    let kf = sym.key_facts(&rt, Some(2), &[3, 1, -1]);
    assert_eq!(kf.verdict, "hidden");
    assert!(kf.in_case && kf.branch.is_none());
    let words: Vec<CoefFact> = kf.terms.iter().map(|(_, c)| *c).collect();
    use CoefFact::*;
    assert_eq!(words, vec![NonZero, Zero, NonZero, Zero, NonZero, Unknown, NonZero]);
    assert_eq!(kf.terms[1].0, vec![3, 1, 0]);
    // a vanishing denominator: irregular, the term unknown
    let kf2 = sym.key_facts(&rt, Some(2), &[2, 1, -1]);
    assert_eq!(kf2.verdict, "irregular");
    assert_eq!(kf2.terms[2].1, Unknown);
    let v = fact_line(7, "ab", &[3, 1, -1], &kf, true, 0.0);
    assert_eq!(v["producer"], "exact_eval");
    assert_eq!(v["backend"], "symbolica");
    assert_eq!(v["schema"], "rustred.campaign.fact-verdict.v1");
    assert_eq!(v["zero_terms"], 2);
    assert_eq!(v["nonzero"].as_array().unwrap().len(), 4);
    assert_eq!(v["unknown"].as_array().unwrap().len(), 1);
    assert_eq!(v["coef"][1], "zero");
    // listed loci with guard 2: regular away from them, hidden on n0 = 4 (locus n0-4)
    let rt1 = RuleText::parse(recs[1].as_bytes(), &f).unwrap();
    assert_eq!(sym.key_facts(&rt1, Some(2), &[5, 1, -1]).verdict, "regular");
    assert_eq!(sym.key_facts(&rt1, Some(2), &[4, 1, -1]).verdict, "hidden");
    // the same loci without a guard version (pre-guard): not complete -> hidden
    assert_eq!(sym.key_facts(&rt1, None, &[5, 1, -1]).verdict, "hidden");
    // an exception branch vanishing at the key: unusable
    let rt2 = RuleText::parse(recs[2].as_bytes(), &f).unwrap();
    let e = sym.key_facts(&rt2, Some(2), &[6, 1, -1]);
    assert_eq!(e.branch, Some(0));
    assert!(e.verdict.starts_with("unusable"), "{}", e.verdict);
    // outside the case: unusable, no terms
    let o = sym.key_facts(&rt, Some(2), &[3, 2, 0]);
    assert!(!o.in_case && o.terms.is_empty() && o.verdict.starts_with("unusable"));
    // absent loci: hidden (never read as regular)
    let rt3 = RuleText::parse(recs[3].as_bytes(), &f).unwrap();
    assert_eq!(sym.key_facts(&rt3, Some(2), &[5, 1, -1]).verdict, "hidden");
}

#[test]
fn requests_are_answered_from_the_table_at_their_offsets() {
    let Some(mut sym) = backend() else { return };
    let dir = std::env::temp_dir().join(format!("cl3-coef-facts-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let recs = records();
    let t = dir.join("T.table.jsonl");
    let body = format!("{HEADER}\n{}\n{}\n", recs[0], recs[1].replace("\"index\":7", "\"index\":8"));
    std::fs::write(&t, &body).unwrap();
    let off1 = (HEADER.len() + 1) as u64;
    let off2 = off1 + recs[0].len() as u64 + 1;
    let req = format!(
        "{}\n{}\n",
        json!({"table": t, "offset": off1, "rule": 7, "keys": [[3,1,-1],[5,1,0]]}),
        json!({"table": t, "offset": off2, "rule": 8, "keys": [[5,1,-1]]})
    );
    let reqs = parse_requests(&req).unwrap();
    let mut seen = vec![];
    for_each_pair(&reqs, &mut sym, |rule, sha, k, f, _| {
        seen.push((rule, sha.to_string(), k.to_vec(), f.verdict.clone()))
    })
    .unwrap();
    assert_eq!(seen.len(), 3);
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(recs[0].as_bytes());
    assert_eq!(seen[0].1, h.finish_hex());
    assert_eq!(seen[0].3, "hidden");
    assert_eq!(seen[2].0, 8);
    assert_eq!(seen[2].3, "regular");
    // a request naming another rule at the offset is refused as a whole
    let bad = parse_requests(&json!({"table": t, "offset": off1, "rule": 8, "keys": [[3,1,-1]]}).to_string()).unwrap();
    assert!(for_each_pair(&bad, &mut sym, |_, _, _, _, _| {}).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
