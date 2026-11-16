//! Lane symcert, SECOND ROUTE (b) (comparison only, never evidence): the engine's `pole_order_d4` of its relation
//! records against lane poles' own valuation (`valuation::relation::pole_order_of`, exact Symbolica) of the SAME
//! records at d = 4. Equal at every relation? A difference is a FINDING (record, key and both values written).
//! The unit test checks the comparison on a constructed record; the ignored test reads `SYMCERT_POLES_RECORDS` (the
//! re-solved records of the supervisor's `symcert_poles_real`) and writes `<dir>/pole_comparison.jsonl` + a summary.

use rustred_campaign_algebra::valuation::D0;
use rustred_campaign_algebra::valuation::relation::pole_order_of;
use rustred_campaign_core::symcert::second::{PoleComparison, compare_pole};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Write;

fn compare_record(id: &str, rec: &Value, d0: &D0) -> PoleComparison {
    let key: Vec<i64> = serde_json::from_value(rec["key"].clone()).unwrap_or_default();
    let ours = pole_order_of(rec, &key, d0).ok();
    compare_pole(
        id,
        &key,
        rec["relation_digest"].as_str(),
        rec["pole_order_d4"].as_u64(),
        ours,
    )
}

#[test]
fn engine_field_and_our_valuation_are_compared_per_record() {
    let d0 = D0::parse("4").unwrap();
    let rec = |po: Value, c: &str| {
        json!({"type": "relation", "key": [1, 1, -1], "verdict": "SOLVED", "relation_digest": "x",
            "pole_order_d4": po, "rhs": [["I(1,1,0)", c, "top"], ["I(1,0,-1)", "1", "sub"]]})
    };
    // (d-4)^-1: order 1 on both sides
    assert_eq!(
        compare_record("a", &rec(json!(1), "1/(d-4)"), &d0).class,
        "equal"
    );
    // regular coefficient but the engine says 1: a finding
    assert_eq!(
        compare_record("b", &rec(json!(1), "(3*d-2)/(d-1)"), &d0).class,
        "differs"
    );
    // the engine's field absent
    assert_eq!(
        compare_record("c", &rec(Value::Null, "1"), &d0).class,
        "engine_absent"
    );
}

#[test]
#[ignore = "reads SYMCERT_POLES_RECORDS (records.jsonl of symcert_poles_real)"]
fn symcert_valuation_real() {
    let p = std::env::var("SYMCERT_POLES_RECORDS").expect("SYMCERT_POLES_RECORDS");
    let d0 = D0::parse("4").unwrap();
    let dir = std::path::Path::new(&p).parent().unwrap().to_path_buf();
    let mut out = std::fs::File::create(dir.join("pole_comparison.jsonl")).unwrap();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut findings = vec![];
    let mut not_solved = 0;
    for (i, l) in std::fs::read_to_string(&p).unwrap().lines().enumerate() {
        let v: Value = serde_json::from_str(l).unwrap();
        let rec = &v["record"];
        if rec.is_null() {
            not_solved += 1;
            continue;
        }
        let c = compare_record(&format!("pair{i}"), rec, &d0);
        *counts.entry(c.class.clone()).or_insert(0) += 1;
        let line = json!({"comparison": c, "face": v["pair"]["face"],
            "same_relation_digest_as_7a": v["same_relation_digest_as_before"],
            "coefficient_denominator_factors": rec["coefficient_denominator_factors"],
            "d_poles": rec["d_poles"], "lift_poles": rec["lift_poles"]});
        if c.class == "differs" {
            findings.push(line.clone());
        }
        writeln!(out, "{line}").unwrap();
    }
    let summary = json!({"counts": counts, "not_solved": not_solved, "findings": findings.len()});
    std::fs::write(
        dir.join("pole_comparison.summary.json"),
        serde_json::to_vec_pretty(&summary).unwrap(),
    )
    .unwrap();
    println!("{summary}");
    for f in &findings {
        println!("FINDING {f}");
    }
}
