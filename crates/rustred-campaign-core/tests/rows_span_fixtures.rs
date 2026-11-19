//! A2 identity test (module `ibp_rows`) on the frozen k3 span cases `span/F6_span_cases.jsonl` (11 cases). Replaces
//! the two `span` placeholders of tests/k3_oracle.rs (`k3_f6_span_verdicts`, `k3_f4_nearcorner_span_16_verified`
//! partly: see rows_t162_rules.rs).
//!
//! Expected (k3 README, checker h3d): C1 pass for every clean rule with the listed rank and column count at a random
//! symbolic point; per census key the listed verdict (covered_regular with rank / ncols per prime, excepted,
//! not_in_case); FAIL for every corrupted control (C1 fail, every covered_regular key span_fail). Plan WP-I2 final
//! version: a rule that declares `sym_rows` is UNKNOWN (needs symmetry rows): fa001, f1003, bp010, bp002 here.

mod inputs;
mod rows_common;

use rows_common::*;
use rustred_campaign_core::ibp_rows::{
    NotApplicableReason, PointSpec, PointStatus, RowsOptions, Unsupported, Verdict, identity_test,
};
use serde_json::Value;

fn census_points(clean0: &Value) -> Vec<(Vec<i64>, String, Value)> {
    let mut out = Vec::new();
    for k in ["C2", "C5"] {
        for p in clean0[k].as_array().into_iter().flatten() {
            let key: Vec<i64> = p["point"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_i64().unwrap())
                .collect();
            out.push((
                key,
                p["verdict"].as_str().unwrap().to_string(),
                p["span"].clone(),
            ));
        }
    }
    out
}

/// The corrupted copy: the inline `.corrupt.json`, else the clean line with rhs[0]'s coefficient replaced by the
/// control's `coef_after` (numerator doubled), checked against `coef_before_sha256`.
fn corrupted_rule(
    o: &Oracle,
    case: &Value,
    line: &str,
) -> rustred_campaign_core::ibp_rows::RowsRule {
    if let Some(rel) = case["corrupt_record"].as_str() {
        let t = String::from_utf8(o.bytes(&format!("span/{rel}"))).unwrap();
        return rule0(&t);
    }
    let ctl = &case["control"];
    let mut v: Value = serde_json::from_str(line.trim_end()).unwrap();
    let r = if v.get("rules").is_some() {
        &mut v["rules"][0]
    } else {
        &mut v
    };
    let rhs = r["rhs"].as_array_mut().expect("rhs list");
    let before = rhs[0][1].as_str().unwrap().to_string();
    assert_eq!(
        sha256_hex(before.as_bytes()),
        ctl["coef_before_sha256"].as_str().unwrap()
    );
    assert_eq!(
        rhs[0][0].as_str().unwrap(),
        ctl["rhs_key"].as_str().unwrap()
    );
    rhs[0][1] = ctl["coef_after"].clone();
    rule0(&serde_json::to_string(&v).unwrap())
}

#[test]
fn rows_f6_span_cases_clean_and_corrupted() {
    let Some(o) = oracle() else { return };
    let fam = t162_family();
    let opts = RowsOptions::default();
    let cases = o.jsonl("span/F6_span_cases.jsonl");
    assert_eq!(cases.len(), 11);
    let (mut passed, mut sym, mut corrupted_failed, mut keys_checked) = (0, 0, 0, 0);
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        let Some(line) = f6_line(&o, case) else {
            continue;
        };
        let rule = rule0(&line);
        let clean0 = &case["clean"][0];
        let census = census_points(clean0);
        let mut points = vec![PointSpec::Generic];
        points.extend(census.iter().map(|(k, _, _)| PointSpec::Key(k.clone())));
        let v = identity_test(&fam, &rule, &points, &opts);
        println!("{}", summary(id, &v));
        if rule.sym_rows.is_some() {
            assert!(
                matches!(
                    v.verdict,
                    Verdict::Unsupported(Unsupported::NeedsSymmetryRows { .. })
                ),
                "{id}: a sym_rows rule must be UNKNOWN (needs symmetry rows), got {:?}",
                v.verdict
            );
            sym += 1;
            continue;
        }
        assert_eq!(v.verdict, Verdict::Pass, "{id}: clean rule must pass");
        assert!(
            v.meets_a2_policy || census.iter().all(|c| c.1 != "covered_regular"),
            "{id}: A2 policy"
        );
        assert!(v.sz_bound < 1e-25, "{id}: sz bound {}", v.sz_bound);
        // C1: rank and column count at the generic point, both primes
        let c1 = &clean0["C1_points"][0];
        for c in &v.points[0].cells {
            assert_eq!(c.rank as u64, c1["rank"].as_u64().unwrap(), "{id}: C1 rank");
            assert_eq!(
                c.row_columns as u64,
                c1["ncols"].as_u64().unwrap(),
                "{id}: C1 ncols"
            );
            assert_eq!(c.mutation_rejected, Some(true), "{id}: mutation control");
            assert!(c.corners_in_span.is_empty(), "{id}: corner control");
        }
        // census keys
        for (i, (key, verdict, span)) in census.iter().enumerate() {
            let p = &v.points[i + 1];
            match verdict.as_str() {
                "covered_regular" => {
                    assert_eq!(p.status, PointStatus::Pass, "{id} {key:?}");
                    let want = span.as_object().unwrap().values().next().unwrap();
                    for c in &p.cells {
                        assert_eq!(
                            c.rank as u64,
                            want["rank"].as_u64().unwrap(),
                            "{id} {key:?} rank"
                        );
                        assert_eq!(
                            c.row_columns as u64,
                            want["ncols"].as_u64().unwrap(),
                            "{id} {key:?} ncols"
                        );
                    }
                    keys_checked += 1;
                }
                "excepted" => assert!(
                    matches!(
                        p.status,
                        PointStatus::NotApplicable(NotApplicableReason::OnException { .. })
                    ),
                    "{id} {key:?}: {:?}",
                    p.status
                ),
                "not_in_case" => assert!(
                    matches!(
                        p.status,
                        PointStatus::NotApplicable(
                            NotApplicableReason::FixedSlot { .. }
                                | NotApplicableReason::TargetConstant { .. }
                                | NotApplicableReason::Equation(_)
                        )
                    ),
                    "{id} {key:?}: {:?}",
                    p.status
                ),
                other => panic!("{id}: unexpected reference verdict {other}"),
            }
        }
        passed += 1;
        // corrupted control: FAIL at the generic point and at every covered_regular key, both primes
        let bad = corrupted_rule(&o, case, &line);
        let vb = identity_test(&fam, &bad, &points, &opts);
        println!("{}", summary(&format!("{id}.corrupt"), &vb));
        assert!(
            matches!(vb.verdict, Verdict::Fail { .. }),
            "{id}: corrupted must FAIL, got {:?}",
            vb.verdict
        );
        assert_eq!(
            vb.points[0].status,
            PointStatus::Fail,
            "{id}: corrupted generic point"
        );
        for (i, (_, verdict, _)) in census.iter().enumerate() {
            if verdict == "covered_regular" {
                assert_eq!(
                    vb.points[i + 1].status,
                    PointStatus::Fail,
                    "{id}: corrupted key {i}"
                );
                assert!(vb.points[i + 1].cells.iter().all(|c| c.witness.is_some()));
            }
        }
        corrupted_failed += 1;
    }
    println!(
        "F6: {passed} clean rules pass, {sym} UNKNOWN (needs symmetry rows), {corrupted_failed} corrupted copies FAIL, \
         {keys_checked} covered_regular keys with equal rank/ncols"
    );
    if passed + sym > 0 {
        assert_eq!((passed, sym, corrupted_failed), (7, 4, 7));
    }
}
