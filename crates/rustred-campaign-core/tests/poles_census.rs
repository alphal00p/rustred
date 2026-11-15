//! Lane poles, B4 / B2 data: the route-pole census summary (src/poles/census.rs) and the terminal-set record
//! (src/poles/basis.rs). Pure.

use rustred_campaign_core::poles::basis::{TerminalSetRecord, terminal_set_digest};
use rustred_campaign_core::poles::census::{CensusRow, summarise};
use serde_json::json;

fn row(k: i64, label: Option<&str>, in_box: bool, order: Option<u64>) -> CensusRow {
    CensusRow {
        key: vec![1, k],
        label: label.map(str::to_string),
        class: "c".into(),
        in_box,
        order,
        terms_with_pole: 0,
    }
}

#[test]
fn census_summary_counts_box_and_closure() {
    let rows = vec![
        row(0, None, true, Some(0)),
        row(1, Some("r1"), true, Some(0)),
        row(2, Some("r1"), true, Some(2)),
        row(3, Some("r1"), false, Some(1)),
        row(4, Some("r2"), true, Some(0)),
        row(5, Some("r3"), false, Some(6)),
        row(6, Some("r4"), true, None),
    ];
    let s = summarise(&rows);
    let j = s.to_json("4", json!({}));
    assert_eq!(j["box"]["keys"], 5);
    assert_eq!(j["box"]["keys_with_pole"], 1);
    assert_eq!(j["box"]["rules_used"], 3);
    assert_eq!(j["box"]["rules_with_pole"], 1);
    assert_eq!(j["box"]["undecided"], 1);
    assert_eq!(j["box"]["max_order"], 2);
    assert_eq!(j["closure"]["keys"], 7);
    assert_eq!(j["closure"]["keys_with_pole"], 3);
    assert_eq!(j["closure"]["rules_with_pole"], 2);
    assert_eq!(j["closure"]["max_order"], 6);
    assert_eq!(
        j["closure"]["keys_by_order"],
        json!({"0": 3, "1": 1, "2": 1, "6": 1})
    );
    assert_eq!(j["rules_with_pole_by_max_order"], json!({"2": 1, "6": 1}));
    assert_eq!(s.per_rule["r1"].keys, 3);
    assert_eq!(s.per_rule["r1"].keys_with_pole_in_box, 1);
    assert_eq!(s.per_rule["r1"].max_order, 2);
    // order-independent
    let mut rev = rows.clone();
    rev.reverse();
    assert_eq!(summarise(&rev).to_json("4", json!({})), j);
}

#[test]
fn terminal_set_record_digest_binds_the_set() {
    let t = vec!["I(1,1)".to_string(), "I(2,1)".to_string()];
    let rec = TerminalSetRecord {
        node: "N".into(),
        d0: "4".into(),
        label: "found".into(),
        terminals: t.clone(),
        reference: vec!["I(3,1)".into(), "I(1,2)".into()],
        verdict: "degenerate".into(),
        order: Some(2),
        route_x: json!({"det_order_at_d0": 3}),
        screen: serde_json::Value::Null,
        evidence: json!({"source": "test"}),
    };
    let a = rec.to_json();
    assert_eq!(a["terminal_set_sha256"], terminal_set_digest(&t));
    assert_eq!(a["schema"], "rustred.campaign.pole-basis.v1");
    assert_eq!(a, rec.to_json());
    assert_ne!(
        terminal_set_digest(&t),
        terminal_set_digest(&["I(2,1)".to_string(), "I(1,1)".to_string()])
    );
}
