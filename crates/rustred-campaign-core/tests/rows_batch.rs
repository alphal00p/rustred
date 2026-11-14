//! Batch API (`identity_test_batch`): streaming input with a header line, an export wrapper line, a bare rule line
//! and an unreadable line; one output line per rule in input order; deterministic for the same input and options
//! (only `cost.seconds` may differ).

mod rows_common;

use rows_common::*;
use rustred_campaign_core::ibp_rows::{
    LineContext, PointSpec, RowsOptions, RowsRule, identity_test_batch,
};
use serde_json::Value;

fn strip_seconds(v: &mut Value) {
    match v {
        Value::Object(m) => {
            m.remove("seconds");
            m.values_mut().for_each(strip_seconds);
        }
        Value::Array(a) => a.iter_mut().for_each(strip_seconds),
        _ => {}
    }
}

#[test]
fn rows_batch_stream_order_and_determinism() {
    let Some(o) = oracle() else { return };
    let f111 = String::from_utf8(o.bytes("span/records/f111.json")).unwrap();
    let bp010 = String::from_utf8(o.bytes("span/records/bp010.json")).unwrap();
    let input = format!(
        "{{\"type\":\"header\",\"schema\":\"table_dump.v1\"}}\n{}{}{{\"type\":\"rule\",\"chart\":\"1\"}}\n",
        f111, bp010
    );
    let fam = t162_family();
    let opts = RowsOptions::default();
    let plan = |_: &RowsRule, ctx: &LineContext| {
        let mut p = vec![PointSpec::Generic, PointSpec::Generic];
        p.extend(ctx.gap_points.iter().cloned().map(PointSpec::Key));
        p
    };
    let run = || {
        let mut out = Vec::new();
        let sum = identity_test_batch(&fam, input.as_bytes(), &plan, &opts, &mut out).unwrap();
        let mut lines: Vec<Value> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        lines.iter_mut().for_each(strip_seconds);
        (sum, lines)
    };
    let (sum, a) = run();
    let (_, b) = run();
    println!("batch: {}", serde_json::to_string(&sum).unwrap());
    assert_eq!(a, b, "deterministic output");
    assert_eq!((sum.lines, sum.rules), (4, 3));
    assert_eq!(sum.by_verdict.get("pass"), Some(&1));
    assert_eq!(
        sum.by_verdict.get("unsupported_needs_symmetry_rows"),
        Some(&1)
    );
    assert_eq!(sum.by_verdict.get("unsupported"), Some(&1));
    let lines: Vec<u64> = a.iter().map(|v| v["line_no"].as_u64().unwrap()).collect();
    assert_eq!(lines, vec![2, 3, 4]);
    assert_eq!(a[0]["result"]["points_passed"], 3); // 2 generic + 1 gap point
    assert_eq!(a[0]["run"], "f-c111");
}
