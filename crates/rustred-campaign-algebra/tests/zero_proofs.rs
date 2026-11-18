//! Lane zeros2: the structural zero proofs (`src/zero_proofs.rs`, Symbolica) on toy records, checked against the campaign
//! core's own exact per-key evaluation (`accept::facts::ExactRecordFacts`, `poly::Poly`: an independent route): a factor
//! that vanishes on the positive-numerator plane is PROVED there; a coefficient that vanishes on only PART of the locus
//! is NOT proved (the per-key evaluation decides each key); big-integer literals; index-dependent factors; a plane that
//! needs a case equation; a plane inside an exception branch (excluded); the denominator classes; the proof lines read
//! back by the core's fact-source parser. Needs the Symbolica licence (`source WS/ops/env.sh`); SKIPS without it.

#[path = "../src/coef_facts.rs"]
#[allow(dead_code)]
mod coef_facts;
mod inputs;
#[path = "../src/zero_proofs.rs"]
#[allow(dead_code)]
mod zero_proofs;

use rustred_campaign_algebra::SymbolicaAlgebra;
use rustred_campaign_core::accept::facts::{CoefficientFacts, ExactRecordFacts};
use rustred_campaign_core::accept::rule::RuleText;
use rustred_campaign_core::closure::CoefFact;
use rustred_campaign_core::import::zero_proofs::{DenStatus, Locus, parse_line};
use rustred_campaign_core::key::NodeFrame;
use rustred_campaign_core::record::TableHeader;
use serde_json::{Value, json};
use zero_proofs::{Outcome, ProverCfg, RecordProver, answer_record, prove_posnum_record};

const HEADER: &str = r#"{"type":"header","schema":"table_dump.v1","family":"TOY","denominators":3,"mask":"110","guard_version":2}"#;

fn frame() -> NodeFrame {
    let h = TableHeader::from_value(serde_json::from_str(HEADER).unwrap()).unwrap();
    NodeFrame::from_header(&h).unwrap()
}

fn alg() -> Option<SymbolicaAlgebra> {
    match SymbolicaAlgebra::new() {
        Ok(a) => Some(a),
        Err(e) => {
            inputs::missing(format_args!("SKIP: {e}"));
            None
        }
    }
}

/// A rule over n0, n1 (lines) and n2 (numerator), canonical target, with the given rhs, equations and exceptions.
fn rule(rhs: &[(&str, &str)], equations: Value, exceptions: Value) -> RuleText {
    let line = json!({"type":"rule","chart":"110","index":3,"fixed":[null,null,null],"equations":equations,
        "target":"I(n0,n1,n2)",
        "rhs": rhs.iter().map(|(k, c)| json!([k, c])).collect::<Vec<_>>(),
        "exceptions": exceptions, "dispatch":"Partition","witness_loci":[]})
    .to_string();
    RuleText::parse(line.as_bytes(), &frame()).unwrap()
}

fn keys() -> Vec<Vec<i32>> {
    let mut v = vec![];
    for n0 in 1..=5 {
        for n1 in 1..=3 {
            for n2 in -3..=0 {
                v.push(vec![n0, n1, n2]);
            }
        }
    }
    v
}

fn plane(p: u32, v: i64) -> Locus {
    Locus::Plane { param: p, value: v }
}

/// Every structural zero word agrees with the core's exact per-key evaluation at every key (and term).
fn agrees_with_core(p: &mut RecordProver<'_>, rt: &RuleText) {
    let mut ex = ExactRecordFacts::new();
    for k in keys() {
        let pt = p.point(&k).unwrap();
        for j in 0..rt.rhs.len() {
            let tk = p.term_key(j, &pt).unwrap();
            let core = ex.coefficient(rt, &k, j);
            let (w, by) = p.zero_word(j, &pt, &tk, false);
            assert_eq!(w, core, "term {j} at {k:?} ({by})");
            let (wk, _) = p.zero_word(j, &pt, &tk, true);
            assert_eq!(wk, core, "per-key term {j} at {k:?}");
        }
    }
}

#[test]
fn a_factor_vanishing_on_the_plane_is_proved_and_part_of_the_locus_is_not() {
    let Some(a) = alg() else { return };
    let rt = rule(
        &[
            ("I(n0-1,n1,n2)", "(d-4)/(d-3)"),
            // raises n2: n2 + 1 >= 1 on the plane n2 = 0; the factor n2 vanishes there
            ("I(n0,n1,n2+1)", "(n2*(d-2*n0))/(d-1)"),
            // vanishes on the plane n2 = 0 only where n0 = 3: NOT proved; the per-key evaluation decides
            ("I(n0,n1-1,n2+1)", "(n2+n0-3)"),
            // raises n2 by 2: planes n2 = -1 and n2 = 0; (n2+1)*n2 vanishes on both
            ("I(n0,n1,n2+2)", "((n2+1)*n2*n1)/(d+n0)"),
            // raises n2 by 2; (n2+1) vanishes on n2 = -1 only
            ("I(n0-1,n1,n2+2)", "(n2+1)"),
            // big-integer literals: the factor n2 times a 30-digit constant (proved) / plus one (not proved)
            (
                "I(n0,n1-1,n2+2)",
                "123456789012345678901234567890*n2*(n2+1)",
            ),
            ("I(n0-1,n1-1,n2+1)", "(123456789012345678901234567890*n2-1)"),
        ],
        json!([]),
        json!([]),
    );
    let mut p = RecordProver::new(&a, ProverCfg::default(), &rt, frame().mask()).unwrap();
    assert!(p.posnum_loci(0).0.is_empty());
    assert_eq!(p.posnum_loci(1).0, vec![plane(2, 0)]);
    assert_eq!(p.posnum_loci(3).0, vec![plane(2, -1), plane(2, 0)]);
    assert!(matches!(
        p.prove(1, plane(2, 0)),
        Outcome::Proved {
            den: DenStatus::NowhereZero,
            ..
        }
    ));
    assert!(matches!(p.prove(2, plane(2, 0)), Outcome::NotProved(_)));
    assert!(matches!(p.prove(3, plane(2, -1)), Outcome::Proved { .. }));
    assert!(matches!(p.prove(3, plane(2, 0)), Outcome::Proved { .. }));
    // (d + n0) with n0 >= 1: d-coefficient 1 is a constant: nowhere zero
    assert!(matches!(
        p.prove(3, plane(2, 0)),
        Outcome::Proved {
            den: DenStatus::NowhereZero,
            ..
        }
    ));
    assert!(matches!(p.prove(4, plane(2, -1)), Outcome::Proved { .. }));
    assert!(matches!(p.prove(4, plane(2, 0)), Outcome::NotProved(_)));
    assert!(matches!(p.prove(5, plane(2, 0)), Outcome::Proved { .. }));
    assert!(matches!(p.prove(5, plane(2, -1)), Outcome::Proved { .. }));
    assert!(matches!(p.prove(6, plane(2, 0)), Outcome::NotProved(_)));
    assert!(p.prove_posnum(1));
    assert!(!p.prove_posnum(2));
    assert!(p.prove_posnum(3));
    assert!(!p.prove_posnum(4));
    agrees_with_core(&mut p, &rt);
    // a key on the plane where the partial term vanishes: decided per key (zero), elsewhere non-zero
    let (w, by) = p.zero_word(2, &[3, 2, 0], &[3, 1, 1], false);
    assert_eq!((w, by.as_str()), (CoefFact::Zero, "key"));
    let (w, _) = p.zero_word(2, &[4, 2, 0], &[4, 1, 1], false);
    assert_eq!(w, CoefFact::NonZero);
    let (w, by) = p.zero_word(1, &[4, 2, 0], &[4, 2, 1], false);
    assert_eq!((w, by.as_str()), (CoefFact::Zero, "plane:2=0"));
}

#[test]
fn equations_exceptions_and_denominators() {
    let Some(a) = alg() else { return };
    // the case n0 = n1 + 1 (equation); exception branch n2 + 3 = 0
    let rt = rule(
        &[
            // vanishes on the plane n2 = 0 only with the equation (n0 - n1 - 1 = 0)
            ("I(n0,n1,n2+1)", "((n0-n1-1)*d)/(d-4)"),
            // vanishes nowhere; but the plane n2 = -3 lies in the exception branch: excluded
            ("I(n0,n1,n2+4)", "(n0+1)"),
            // denominators: (n0-n1-1+2*n2) has no constant d-coefficient and is not an exception: where_regular
            ("I(n0-1,n1,n2+1)", "(n2*d)/(n0+n2-n1+1)"),
            // n0 + n1 with lines >= 1 is positive (sign test): nowhere zero
            ("I(n0,n1-1,n2+1)", "(n2)/(n0+n1)"),
            // the denominator vanishes identically on the plane n2 = 0: not proved (coefficient undefined)
            ("I(n0-1,n1-1,n2+1)", "(n2*d)/(n2*d)"),
        ],
        json!(["n0-n1-1"]),
        json!([["n2+3"]]),
    );
    let mut p = RecordProver::new(&a, ProverCfg::default(), &rt, frame().mask()).unwrap();
    match p.prove(0, plane(2, 0)) {
        Outcome::Proved { equations, den } => {
            assert_eq!(equations, vec![0]);
            assert_eq!(den, DenStatus::NowhereZero);
        }
        o => panic!("{o:?}"),
    }
    assert!(matches!(p.prove(1, plane(2, -3)), Outcome::Excluded(_)));
    assert!(matches!(p.prove(1, plane(2, -2)), Outcome::NotProved(_)));
    match p.prove(2, plane(2, 0)) {
        Outcome::Proved { den, .. } => assert_eq!(den, DenStatus::WhereRegular),
        o => panic!("{o:?}"),
    }
    match p.prove(3, plane(2, 0)) {
        Outcome::Proved { den, .. } => assert_eq!(den, DenStatus::NowhereZero),
        o => panic!("{o:?}"),
    }
    assert!(matches!(p.prove(4, plane(2, 0)), Outcome::NotProved(_)));
    // every proof line reads back through the core's fact-source parser
    let lines = p.take_new();
    assert!(!lines.is_empty());
    // a where_regular proof carries the term's denominator text (consumers evaluate it at each key)
    assert!(
        lines
            .iter()
            .any(|l| l["denominator"] == "where_regular" && l["den"] == "(n0+n2-n1+1)"),
        "{lines:?}"
    );
    for l in &lines {
        let pl = parse_line(l).unwrap_or_else(|e| panic!("{e}: {l}"));
        assert_eq!(pl.record_sha256, p.sha);
        assert_eq!(pl.rule, Some(3));
    }
    // keys in the case only (n0 = n1 + 1): the structural word equals the core's
    let mut ex = ExactRecordFacts::new();
    for k in keys()
        .into_iter()
        .filter(|k| k[0] == k[1] + 1 && k[2] != -3)
    {
        let pt = p.point(&k).unwrap();
        for j in [0usize, 2, 3] {
            let tk = p.term_key(j, &pt).unwrap();
            assert_eq!(
                p.zero_word(j, &pt, &tk, false).0,
                ex.coefficient(&rt, &k, j),
                "{j} {k:?}"
            );
        }
    }
}

#[test]
fn zeros_requests_answer_every_asked_term_and_the_posnum_census() {
    let Some(a) = alg() else { return };
    let rt = rule(
        &[
            ("I(n0-1,n1,n2)", "(d-4)/(d-3)"),
            ("I(n0,n1,n2+1)", "(n2*(d-2*n0))/(d-1)"),
            ("I(n0,n1-1,n2+1)", "(n2+n0-3)"),
        ],
        json!([]),
        json!([]),
    );
    let mut p = RecordProver::new(&a, ProverCfg::default(), &rt, frame().mask()).unwrap();
    let keys = vec![vec![3, 2, 0], vec![4, 2, 0], vec![4, 2, -1]];
    let terms = vec![vec![1, 2], vec![1], vec![0, 1]];
    let (v, proofs) = answer_record(&mut p, &keys, &terms, false);
    assert_eq!(v.len(), 3);
    assert_eq!(
        v[0]["terms"],
        json!([[1, "zero", "plane:2=0"], [2, "zero", "key"]])
    );
    assert_eq!(v[1]["terms"], json!([[1, "zero", "plane:2=0"]]));
    // n2 = -1: term 1 is not positive there; its coefficient n2*(d-2n0) is non-zero; term 0 non-zero
    assert_eq!(v[2]["terms"][0][1], "nonzero");
    assert_eq!(v[2]["terms"][1][1], "nonzero");
    assert!(proofs.iter().any(|l| l["verdict"] == "zero_on_locus"));
    let mut p2 = RecordProver::new(&a, ProverCfg::default(), &rt, frame().mask()).unwrap();
    let mut c = zero_proofs::PosnumCounts::default();
    let lines = prove_posnum_record(&mut p2, &mut c);
    assert_eq!(c.posnum_terms, 2);
    assert_eq!(c.terms_fully_proved, 1);
    assert_eq!(c.proved, 1, "{lines:?}");
    // the per-key method decides every term at the key
    let mut p3 = RecordProver::new(&a, ProverCfg::default(), &rt, frame().mask()).unwrap();
    let (v, proofs) = answer_record(&mut p3, &keys, &terms, true);
    assert!(proofs.is_empty());
    assert_eq!(
        v[0]["terms"],
        json!([[1, "zero", "key"], [2, "zero", "key"]])
    );
}
