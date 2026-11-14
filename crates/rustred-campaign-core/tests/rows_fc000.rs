//! Rule f-c000 (T162 batch-01 f.faces.jsonl line 0, frozen F6 case f000; read from the handoff copy by byte offset,
//! sha256-checked): PASS at its recorded seeds, rank 1,410 on 5,351 columns at generic points (plan WP-I2 test), and
//! three corrupted copies (inventory h9: coefficient x2, a right-hand-side key shifted, the target shifted) FAIL at
//! both primes at two generic points and at the three applicable census keys. The shell diagnostic (seeds +- e_i) on
//! the doubled coefficient also fails (inventory D2: "also with shell: 45,750 rows").

mod rows_common;

use rows_common::*;
use rustred_campaign_core::ibp_rows::{PointSpec, PointStatus, RowsOptions, RowsRule, Verdict, identity_test};
use serde_json::Value;

/// (rule 0 of the line, the points, the F6 control coefficient: rhs[0] numerator doubled)
fn f000() -> Option<(Value, Vec<PointSpec>, Value)> {
    let o = oracle()?;
    let case = o
        .jsonl("span/F6_span_cases.jsonl")
        .into_iter()
        .find(|c| c["id"] == "f000")
        .unwrap();
    let line = f6_line(&o, &case)?;
    let v: Value = serde_json::from_str(line.trim_end()).unwrap();
    let mut points = vec![PointSpec::Generic, PointSpec::Generic];
    for k in [
        [3, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0],
        [4, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, -2, 0, 0, 0, 0, 0, 0, 0, 0],
        [3, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, -2, 0, 0, 0, 0, 0, 0, 0, 0],
    ] {
        points.push(PointSpec::Key(k.to_vec()));
    }
    Some((v["rules"][0].clone(), points, case["control"]["coef_after"].clone()))
}

fn rule(v: &Value) -> RowsRule {
    RowsRule::from_value(v.clone(), 20).unwrap()
}

#[test]
fn rows_fc000_passes_at_its_seeds_and_corrupted_copies_fail() {
    let Some((r, points, coef_after)) = f000() else { return };
    let fam = t162_family();
    let opts = RowsOptions::default();
    let clean = identity_test(&fam, &rule(&r), &points, &opts);
    println!("{}", summary("f-c000", &clean));
    assert_eq!(clean.verdict, Verdict::Pass);
    assert!(clean.meets_a2_policy);
    assert_eq!(clean.points_passed, 5);
    for p in &clean.points[..2] {
        for c in &p.cells {
            assert_eq!((c.rank, c.row_columns, c.rows), (1410, 5351, 1410));
        }
    }
    assert!(clean.sz_bound_log10 < -150.0, "{}", clean.sz_bound_log10);
    // the primes are verify-range primes, derived from the seed; d values recorded and never 37/7
    for p in &clean.points {
        for c in &p.cells {
            rustred_campaign_core::replay_base::primes::check_verify_prime(c.prime, &[]).unwrap();
            assert!(c.d >= 2);
        }
    }

    // corrupted copies
    let mut coef2 = r.clone();
    coef2["rhs"][0][1] = coef_after;
    let mut key = r.clone();
    let k0 = key["rhs"][0][0].as_str().unwrap().to_string();
    assert!(k0.contains("n11+1"));
    key["rhs"][0][0] = Value::String(k0.replace("n11+1", "n11+2"));
    let mut target = r.clone();
    let t0 = target["target"].as_str().unwrap().to_string();
    assert!(t0.starts_with("I(n0,"));
    target["target"] = Value::String(t0.replacen("I(n0,", "I(n0+1,", 1));
    for (name, bad) in [("coefficient x2", coef2), ("rhs key n11+1 -> n11+2", key), ("target n0 -> n0+1", target)] {
        let v = identity_test(&fam, &rule(&bad), &points, &opts);
        println!("{}", summary(&format!("f-c000 [{name}]"), &v));
        assert!(matches!(v.verdict, Verdict::Fail { .. }), "{name}: {:?}", v.verdict);
        // every point that applies fails at both primes (the shifted target moves the rule point: keys may not apply)
        for p in &v.points {
            if !matches!(p.status, PointStatus::NotApplicable(_)) {
                assert_eq!(p.status, PointStatus::Fail, "{name}");
                assert_eq!(p.cells.len(), 2);
                assert!(p.cells.iter().all(|c| !c.in_span && c.witness.is_some()));
            }
        }
        assert!(v.points[..2].iter().all(|p| p.status == PointStatus::Fail), "{name}: generic points");
    }
}

#[test]
#[ignore = "slow: ~10 min release (45,750 rows x 117,055 columns, 44 M entries, two primes)"]
fn rows_fc000_shell_diagnostic_on_the_doubled_coefficient() {
    let Some((r, _, coef_after)) = f000() else { return };
    let mut bad = r.clone();
    bad["rhs"][0][1] = coef_after;
    let fam = t162_family();
    let opts = RowsOptions {
        shell_diagnostic: true,
        ..RowsOptions::default()
    };
    let v = identity_test(&fam, &rule(&bad), &[PointSpec::Generic], &opts);
    println!("{}", summary("f-c000 [coefficient x2, shell]", &v));
    let sh: Vec<String> = v.points[0]
        .shell
        .iter()
        .map(|c| format!("{:?} rank {} cols {} rows {} entries {} {:.1}s", c.status, c.rank, c.row_columns, c.rows, c.entries, c.seconds))
        .collect();
    println!("shell cells: {sh:?}");
    assert!(matches!(v.verdict, Verdict::Fail { .. }));
    assert_eq!(v.points[0].shell.len(), 2);
    for c in &v.points[0].shell {
        assert_eq!(c.rows, 45_750);
        assert!(!c.in_span);
    }
}
