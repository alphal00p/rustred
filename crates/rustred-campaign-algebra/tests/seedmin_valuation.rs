//! Lane seedmin: lane poles' REAL per-relation valuation (`valuation::relation::pole_order_of`, Symbolica) behind the
//! minimiser's `RelationPoles` trait, and its agreement with the TEMPORARY `LocalValuationJudge` used by the supervisor
//! until the `rr_poles` child exists (layering: only this crate links Symbolica). The ignored test compares both on
//! every relation record a real seedmin run kept (`SEEDMIN_RELATIONS` = its relations.jsonl).

use rustred_campaign_algebra::valuation::D0;
use rustred_campaign_algebra::valuation::relation::pole_order_of;
use rustred_campaign_core::seedmin::{LocalValuationJudge, PoleOrder, RelationPoles};
use serde_json::{Value, json};

/// The real judge as a seedmin `RelationPoles`.
struct PolesJudge;

impl RelationPoles for PolesJudge {
    fn pole_order_of(&mut self, record: &Value, key: &[i32], d0: &str) -> PoleOrder {
        let Ok(d) = D0::parse(d0) else {
            return PoleOrder::Undecided;
        };
        let k: Vec<i64> = key.iter().map(|&x| x as i64).collect();
        match pole_order_of(record, &k, &d) {
            Ok(o) => PoleOrder::Order { order: o as u32 },
            Err(_) => PoleOrder::Undecided,
        }
    }
    fn name(&self) -> String {
        "valuation::relation::pole_order_of (lane poles)".into()
    }
}

fn rec(key: &[i64], rhs: &[(&str, &str)]) -> Value {
    json!({"type": "relation", "key": key, "verdict": "SOLVED", "target": "I(..)",
        "rhs": rhs.iter().map(|(k, c)| json!([k, c, "top"])).collect::<Vec<_>>()})
}

#[test]
fn the_real_valuation_and_the_temporary_judge_agree_on_engine_coefficient_texts() {
    let key = [2i64, 1, 0];
    let cases: Vec<(Vec<(&str, &str)>, u32)> = vec![
        (vec![("I(1,1,0)", "(8-3*d)/(-14+6*d)")], 0),
        (vec![("I(1,1,0)", "(8-3*d)/(-8+2*d)"), ("I(1,0,0)", "2")], 1),
        (
            vec![("I(1,1,0)", "1/(d-4)^2"), ("I(1,0,0)", "(d-4)/(d-1)")],
            2,
        ),
        (vec![("I(1,1,0)", "(d-4)/((d-4)^3*(d-1))")], 2),
        (vec![("I(1,1,0)", "-1/(-14+6*d)")], 0),
    ];
    for (rhs, want) in cases {
        let r = rec(&key, &rhs);
        let k32 = [2, 1, 0];
        let a = PolesJudge.pole_order_of(&r, &k32, "4");
        let b = LocalValuationJudge.pole_order_of(&r, &k32, "4");
        assert_eq!(a, PoleOrder::Order { order: want }, "{rhs:?}");
        assert_eq!(a, b, "{rhs:?}");
    }
}

#[test]
#[ignore = "real data: SEEDMIN_RELATIONS = a seedmin run's relations.jsonl"]
fn the_real_valuation_and_the_temporary_judge_agree_on_a_runs_records() {
    let p = std::env::var("SEEDMIN_RELATIONS").expect("SEEDMIN_RELATIONS");
    let text = std::fs::read_to_string(&p).unwrap();
    let (mut n, mut agree, mut regular) = (0u64, 0u64, 0u64);
    let mut dis: Vec<String> = vec![];
    for l in text.lines().filter(|l| !l.trim().is_empty()) {
        let r: Value = serde_json::from_str(l).unwrap();
        let key: Vec<i32> = serde_json::from_value(r["key"].clone()).unwrap();
        let a = PolesJudge.pole_order_of(&r, &key, "4");
        let b = LocalValuationJudge.pole_order_of(&r, &key, "4");
        n += 1;
        if a == b {
            agree += 1;
        } else if dis.len() < 10 {
            dis.push(format!(
                "{} {key:?}: poles {a:?} vs local {b:?}",
                r["relation_digest"]
            ));
        }
        if a == PoleOrder::regular() {
            regular += 1;
        }
    }
    eprintln!(
        "SEEDMIN VALUATION: {agree} of {n} records agree; {regular} regular (lane poles); disagreements {dis:?}"
    );
    assert_eq!(agree, n, "{dis:?}");
}
