//! Lane regbasis, Symbolica-free half: spec, keys, preference, needs, record sealing.

use std::collections::HashMap;

use rustred_campaign_core::regbasis::record::{Binding, RECORD_SCHEMA, band_verdict, seal, verify};
use rustred_campaign_core::regbasis::spec::CandidateOrder;
use rustred_campaign_core::regbasis::{
    MapAvailability, Preference, RegBasisSpec, SeriesText, candidate_family, dot_free_keys,
    dots_of, need_table, rank_of, sort_by_preference,
};
use serde_json::json;

const CHART: [bool; 5] = [true, true, true, false, false];

#[test]
fn spec_defaults_and_refusals() {
    let s = RegBasisSpec::from_value(&serde_json::Value::Null).unwrap();
    assert_eq!(s, RegBasisSpec::default());
    assert_eq!(s.d0, "4");
    assert_eq!((s.max_dots, s.max_rank, s.max_weight), (2, 1, 2));
    let s = RegBasisSpec::from_value(
        &json!({"d0": "17/7", "base_order": 1, "candidates": {"order": "node", "max_rank": 3},
        "certify": {"keys": "none"}, "tower": {"max_rank": 4, "max_seconds": 2.5}}),
    )
    .unwrap();
    assert_eq!(s.d0, "17/7");
    assert_eq!(s.base_order, 1);
    assert_eq!(s.order, CandidateOrder::Node);
    assert_eq!(s.max_rank, 3);
    assert_eq!(s.tower_max_rank, Some(4));
    assert_eq!(s.tower_max_seconds, Some(2.5));
    // round trip of the effective section
    let back = RegBasisSpec::from_value(&s.to_value());
    assert!(
        back.is_err(),
        "max_seconds is written as text in the record (no floats in canonical JSON)"
    );
    assert!(RegBasisSpec::from_value(&json!({"bogus": 1})).is_err());
    assert!(RegBasisSpec::from_value(&json!({"candidates": {"max_dot": 1}})).is_err());
    assert!(RegBasisSpec::from_value(&json!({"d0": "4.0"})).is_err());
    assert!(RegBasisSpec::from_value(&json!({"d0": "1/0"})).is_err());
    assert!(RegBasisSpec::from_value(&json!({"chunk_keys": 0})).is_err());
}

#[test]
fn ranks_dots_and_dot_free_keys() {
    assert_eq!(dots_of(&CHART, &[2, 1, 3, 0, -1]), Some(3));
    assert_eq!(rank_of(&CHART, &[2, 1, 3, 0, -1]), Some(1));
    assert_eq!(rank_of(&CHART, &[0, 1, 1, 0, 0]), None, "subsector");
    assert_eq!(
        rank_of(&CHART, &[1, 1, 1, 1, 0]),
        None,
        "positive numerator"
    );
    for r in 0..5u32 {
        let k = dot_free_keys(&CHART, r);
        assert_eq!(
            k.len() as u32,
            r + 1,
            "two numerator slots: r + 1 compositions"
        );
        assert!(
            k.iter()
                .all(|k| dots_of(&CHART, k) == Some(0) && rank_of(&CHART, k) == Some(i64::from(r)))
        );
    }
}

#[test]
fn candidate_family_respects_bounds() {
    let s = RegBasisSpec {
        max_dots: 2,
        max_rank: 1,
        max_weight: 2,
        ..Default::default()
    };
    let fam = candidate_family(&CHART, &s, &|_| true);
    assert!(fam.iter().all(|k| {
        let (d, r) = (dots_of(&CHART, k).unwrap(), rank_of(&CHART, k).unwrap());
        d <= 2 && r <= 1 && d + r <= 2
    }));
    // D0 r0: 1; D1: 3 (r0) + 3*2 (r1); D2 r0: 6; D0 r1: 2  -> 1 + 2 + 3 + 6 + 6 = 18
    assert_eq!(fam.len(), 18);
    let only = candidate_family(&CHART, &s, &|k| k[0] == 1);
    assert!(only.iter().all(|k| k[0] == 1));
}

#[test]
fn preference_order() {
    let terminals = vec![vec![1, 2, 1, 0, 0], vec![1, 1, 1, 0, 0]];
    let cmp = |a: &[i32], b: &[i32]| a.cmp(b);
    let mut keys = vec![
        vec![1, 1, 2, 0, 0],
        vec![1, 1, 1, 0, -1],
        vec![1, 1, 1, 0, 0],
        vec![1, 2, 1, 0, 0],
        vec![3, 1, 1, 0, 0],
    ];
    let p = Preference {
        chart: &CHART,
        order: CandidateOrder::WeightThenNode,
        terminals: &terminals,
        terminals_first: true,
        cmp: &cmp,
    };
    sort_by_preference(&mut keys, &p);
    assert_eq!(
        keys[0],
        vec![1, 2, 1, 0, 0],
        "terminals first, in closure order"
    );
    assert_eq!(keys[1], vec![1, 1, 1, 0, 0]);
    assert_eq!(
        keys[2],
        vec![1, 1, 1, 0, -1],
        "weight 1 before weight 2; then the comparator"
    );
    assert_eq!(keys[3], vec![1, 1, 2, 0, 0]);
    assert_eq!(keys[4], vec![3, 1, 1, 0, 0]);
    let p = Preference {
        terminals_first: false,
        ..p
    };
    sort_by_preference(&mut keys, &p);
    assert_eq!(
        keys[0],
        vec![1, 1, 1, 0, 0],
        "corner first without the terminal preference"
    );
}

#[test]
fn need_tables() {
    let labels = vec!["c".to_string(), "Q".to_string()];
    let keys = vec![Some(vec![1, 1, 1, 0, 0]), None];
    let poles = vec![Some(2), Some(1)];
    let t = need_table("T", 0, &labels, &keys, &poles, None, "x");
    assert_eq!(t.rows[0].need, 2);
    assert_eq!(t.missing_total, None);
    let mut a = MapAvailability {
        default: Some(1),
        keys: HashMap::new(),
        labels: HashMap::new(),
    };
    a.keys.insert(vec![1, 1, 1, 0, 0], 4);
    let t = need_table("T", 0, &labels, &keys, &poles, Some(&a), "x");
    assert_eq!(t.rows[0].missing, Some(0));
    assert_eq!(t.rows[1].missing, Some(0));
    let t = need_table("T", 1, &labels, &keys, &poles, Some(&a), "x");
    assert_eq!(
        t.rows[1].missing,
        Some(1),
        "base 1 + pole 1 against default 1"
    );
    assert_eq!(t.missing_total, Some(1));
    assert_eq!(t.rows_exceeding, 1);
    let v = t.to_value();
    assert_eq!(v["rows"][1]["status"], "exceeds by 1");
}

#[test]
fn record_seal_and_verify() {
    let b = Binding {
        node_id: "toy".into(),
        node_identity_digest: "abc".into(),
        box_id: "box".into(),
        box_digest: "def".into(),
        closure_statement_digest: "123".into(),
        d0: "4".into(),
    };
    let v =
        json!({"schema": RECORD_SCHEMA, "binding": b.to_value(), "regular_set": [[1, 1, 1, 0, 0]]});
    let (s, text) = seal(v.clone()).unwrap();
    verify(&s).unwrap();
    assert!(text.contains("\"digest\""));
    let mut t = s.clone();
    t["binding"]["d0"] = json!("6");
    assert!(verify(&t).is_err(), "the digest binds d0");
    let b2 = Binding {
        closure_statement_digest: "124".into(),
        ..b.clone()
    };
    let (s2, _) = seal(json!({"schema": RECORD_SCHEMA, "binding": b2.to_value(), "regular_set": [[1, 1, 1, 0, 0]]})).unwrap();
    assert_ne!(
        s["digest"], s2["digest"],
        "the digest binds the closure statement"
    );
    assert!(seal(json!({"schema": "other", "binding": {}})).is_err());
    assert!(
        Binding {
            box_digest: " ".into(),
            ..b
        }
        .validate()
        .is_err()
    );
    assert_eq!(
        band_verdict(Some(2), 2, true),
        "regular on the full band (through rank 2)"
    );
    assert!(band_verdict(Some(1), 2, true).contains("through rank 1 (lower bound"));
}

#[test]
fn series_text_validate() {
    assert!(
        SeriesText {
            start: -2,
            coeffs: vec!["1".into(), "0".into(), "3".into()],
            prec: 1
        }
        .validate()
        .is_ok()
    );
    assert!(
        SeriesText {
            start: -2,
            coeffs: vec!["1".into(), "0".into(), "3".into()],
            prec: 0
        }
        .validate()
        .is_err()
    );
    assert!(SeriesText::zero(1).validate().is_ok());
}
