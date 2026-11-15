//! Lane poles, B2 route X: the exact check of a terminal set through its master map (src/valuation/basis.rs).
//! Constructed cases (analytic one-loop bubble, a regular rotation, a non-invertible map) and the helper's exact maps as
//! TEST DATA (read-only, /common/ben/sym-investigation; skipped when absent): T266 (w14/term, w11/l5c), T162 (reclist),
//! and the seven reclist nodes of addendum 3 (rank at d = 4 against generic, null directions, order of the zero of det,
//! pole orders of the inverse). Every disagreement is printed and fails the test. Needs the Symbolica licence.

use rustred_campaign_algebra::licensed;
use rustred_campaign_algebra::valuation::basis::{MasterMap, check_master_map};
use rustred_campaign_algebra::valuation::{D0, Valuation};
use serde_json::Value;
use std::path::Path;

fn go() -> bool {
    if !licensed() {
        eprintln!("SKIP: Symbolica licence absent (source WS/ops/env.sh)");
        return false;
    }
    true
}

fn d4() -> D0 {
    D0::parse("4").unwrap()
}

fn map(rows: &[&str], cols: &[&str], e: &[&[&str]]) -> MasterMap {
    MasterMap {
        rows: rows.iter().map(|s| s.to_string()).collect(),
        cols: cols.iter().map(|s| s.to_string()).collect(),
        entries: e
            .iter()
            .map(|r| r.iter().map(|s| s.to_string()).collect())
            .collect(),
    }
}

const HELPER: &str = "/common/ben/sym-investigation";

fn helper(rel: &str) -> Option<Value> {
    let p = Path::new(HELPER).join(rel);
    match std::fs::read(&p) {
        Ok(b) => {
            Some(serde_json::from_slice(&b).unwrap_or_else(|e| panic!("{}: {e}", p.display())))
        }
        Err(_) => {
            eprintln!("SKIP (test data absent): {}", p.display());
            None
        }
    }
}

fn texts(v: &Value) -> Vec<Vec<String>> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|r| {
            r.as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap().to_string())
                .collect()
        })
        .collect()
}

#[test]
fn constructed_cases() {
    if !go() {
        return;
    }
    // A3: one-loop massless bubble, I(3,1) = (d-3)(d-4)/2 I(1,1): T = {I(3,1)} on R = {I(1,1)} is degenerate, order 1
    let c = check_master_map(&map(&["I(3,1)"], &["I(1,1)"], &[&["(d-3)*(d-4)/2"]]), &d4());
    assert_eq!(c.verdict, "degenerate");
    assert_eq!(c.order, Some(1));
    assert_eq!(c.det_order, Some(Valuation::Order(1)));
    assert_eq!(c.rank_at_d0, Some(0));
    assert_eq!(c.null_directions, vec![vec!["1".to_string()]]);
    assert_eq!(c.inverse_orders, Some(vec![vec![Valuation::Order(-1)]]));
    // the other way round: R on T is regular (a zero, no pole, in the inverse)
    let c = check_master_map(
        &map(&["I(1,1)"], &["I(3,1)"], &[&["2/((d-3)*(d-4))"]]),
        &d4(),
    );
    assert_eq!((c.verdict.as_str(), c.order), ("regular", Some(0)));
    assert_eq!(c.rank_at_d0, None, "M has a pole at d0: M(d0) undefined");
    // a regular rotation (det constant): regular
    let c = check_master_map(
        &map(&["a", "b"], &["x", "y"], &[&["1", "d"], &["0", "1"]]),
        &d4(),
    );
    assert_eq!(
        (c.verdict.as_str(), c.rank_at_d0, c.det_order),
        ("regular", Some(2), Some(Valuation::Order(0)))
    );
    // a double zero of det from two simple zeros in different rows; rank 0 at d0; inverse poles (1, 1)
    let c = check_master_map(
        &map(&["a", "b"], &["x", "y"], &[&["d-4", "0"], &["0", "2*d-8"]]),
        &d4(),
    );
    assert_eq!(
        (c.verdict.as_str(), c.order, c.det_order),
        ("degenerate", Some(1), Some(Valuation::Order(2)))
    );
    assert_eq!(c.null_directions.len(), 2);
    // degenerate with one null direction 6a - 3b - 2c = O(d-4), sympy-style ** accepted
    let c = check_master_map(
        &map(
            &["a", "b", "c"],
            &["x", "y", "z"],
            &[
                &["1", "0", "0"],
                &["0", "1", "0"],
                &["3", "-3/2", "(d-4)**1*(d-8)"],
            ],
        ),
        &d4(),
    );
    assert_eq!(
        (c.verdict.as_str(), c.order, c.rank_at_d0),
        ("degenerate", Some(1), Some(2))
    );
    assert_eq!(
        c.null_directions,
        vec![vec!["6".to_string(), "-3".into(), "-2".into()]]
    );
    // not invertible over Q(d): unresolved, never regular
    let c = check_master_map(
        &map(&["a", "b"], &["x", "y"], &[&["1", "d"], &["2", "2*d"]]),
        &d4(),
    );
    assert_eq!(c.verdict, "unresolved");
    assert_eq!(c.generic_rank, 1);
    // bad input
    assert_eq!(
        check_master_map(&map(&["a"], &["x"], &[&["sin(d)"]]), &d4()).verdict,
        "unresolved"
    );
    assert_eq!(
        check_master_map(&map(&["a"], &["x", "y"], &[&["1"]]), &d4()).verdict,
        "unresolved"
    );
}

/// T266 (helper P7, w14/term/T266/REGULAR_TERMINALS.json): T = {c, R, Q} on L = {c, R, I3}: simple zero, null (6,-3,-2),
/// inverse orders [[0,-,-],[-,0,-],[-1,-1,-1]].
#[test]
fn t266_closure_terminals_on_the_regular_set() {
    if !go() {
        return;
    }
    let Some(v) = helper("w14/term/T266/REGULAR_TERMINALS.json") else {
        return;
    };
    // M[i][j] = {"exact": "<text>", "ord_d4": ..}
    let m: Vec<Vec<String>> = v["M"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            r.as_array()
                .unwrap()
                .iter()
                .map(|x| x["exact"].as_str().unwrap().to_string())
                .collect()
        })
        .collect();
    let n = m.len();
    let mm = MasterMap {
        rows: (0..n).map(|i| format!("T{i}")).collect(),
        cols: (0..n).map(|i| format!("L{i}")).collect(),
        entries: m,
    };
    let c = check_master_map(&mm, &d4());
    eprintln!("T266 route X: {}", c.to_json());
    assert_eq!(c.verdict, "degenerate");
    assert_eq!(c.order, Some(1));
    assert_eq!(c.det_order, Some(Valuation::Order(1)));
    assert_eq!(c.rank_at_d0, Some(2));
    assert_eq!(
        c.null_directions,
        vec![vec!["6".to_string(), "-3".into(), "-2".into()]]
    );
    let io = c.inverse_orders.unwrap();
    assert_eq!(io[2], vec![Valuation::Order(-1); 3]);
    // the helper's own orders of M and M^-1 (pole_orders member): equal entry by entry
    let conv = |x: &Value| x.as_i64().map_or(Valuation::Zero, Valuation::Order);
    let want_m: Vec<Vec<Valuation>> = v["pole_orders"]["M"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r.as_array().unwrap().iter().map(conv).collect())
        .collect();
    let want_i: Vec<Vec<Valuation>> = v["pole_orders"]["Minv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r.as_array().unwrap().iter().map(conv).collect())
        .collect();
    assert_eq!(c.entry_orders, want_m);
    assert_eq!(io, want_i);
}

/// T266 (w11/l5c basis_change_exact.json): published -> L: det order +1, left null (15,-2,3); ours -> L: (15,-15/2,-5)
/// up to scale = (6,-3,-2).
#[test]
fn t266_l5c_published_and_ours() {
    if !go() {
        return;
    }
    let Some(v) = helper("w11/l5c/deliver/basis_change_exact.json") else {
        return;
    };
    for (which, null) in [
        ("published", vec!["15", "-2", "3"]),
        ("ours", vec!["6", "-3", "-2"]),
    ] {
        let m = texts(&v[which]["matrix_rows_in_L"]);
        let mm = MasterMap {
            rows: vec!["a".into(), "b".into(), "c".into()],
            cols: vec!["x".into(), "y".into(), "z".into()],
            entries: m,
        };
        let c = check_master_map(&mm, &d4());
        assert_eq!(c.verdict, "degenerate", "{which}");
        assert_eq!(
            c.det_order,
            Some(Valuation::Order(
                v[which]["zero_order_at_4"].as_i64().unwrap()
            )),
            "{which}"
        );
        assert_eq!(
            c.null_directions,
            vec![null.iter().map(|s| s.to_string()).collect::<Vec<_>>()],
            "{which}"
        );
        assert_eq!(c.order, Some(1), "{which}");
    }
}

/// The seven nodes of addendum 3 (w13/reclist/nodes/<N>.json, A = published masters on the regular basis): rank at
/// d = 4 / generic, null vectors, order of the zero of det A, pole orders of A^-1: all reproduced exactly.
#[test]
fn reclist_seven_nodes() {
    if !go() {
        return;
    }
    let mut disagreements = vec![];
    let mut done = 0;
    for node in ["T266", "T241", "T256", "T162", "T178", "T157", "T160"] {
        let Some(v) = helper(&format!("w13/reclist/nodes/{node}.json")) else {
            continue;
        };
        let a = &v["A"];
        let mm = MasterMap {
            rows: a["rows_published_masters"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap().to_string())
                .collect(),
            cols: a["cols_regular_basis"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap().to_string())
                .collect(),
            entries: texts(&a["matrix"]),
        };
        let c = check_master_map(&mm, &d4());
        let want_rank = v["rank_at_d4"].as_u64().map(|x| x as usize);
        let want_gen = v["generic_rank"].as_u64().unwrap() as usize;
        let want_zero = v["zero_order_at_d4"].as_i64().unwrap();
        let want_null: Vec<Vec<String>> = texts(&v["null_vectors"]);
        let want_inv: Vec<Vec<Option<i64>>> = v["inverse_pole_orders"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r.as_array().unwrap().iter().map(Value::as_i64).collect())
            .collect();
        let got_inv: Vec<Vec<Option<i64>>> = c
            .inverse_orders
            .clone()
            .unwrap_or_default()
            .iter()
            .map(|r| {
                r.iter()
                    .map(|x| match x {
                        Valuation::Zero => None,
                        Valuation::Order(k) => Some((-k).max(0)),
                    })
                    .collect()
            })
            .collect();
        let mut d = vec![];
        if c.rank_at_d0 != want_rank {
            d.push(format!("rank_at_d4 {:?} vs {:?}", c.rank_at_d0, want_rank));
        }
        if c.generic_rank != want_gen {
            d.push(format!("generic_rank {} vs {want_gen}", c.generic_rank));
        }
        if c.det_order != Some(Valuation::Order(want_zero)) {
            d.push(format!("det zero order {:?} vs {want_zero}", c.det_order));
        }
        if c.null_directions != want_null {
            d.push(format!("null {:?} vs {:?}", c.null_directions, want_null));
        }
        if got_inv != want_inv {
            d.push(format!("inverse pole orders {got_inv:?} vs {want_inv:?}"));
        }
        if c.verdict != "degenerate" {
            d.push(format!("verdict {}", c.verdict));
        }
        eprintln!(
            "{node}: verdict {} order {:?} det_order {:?} rank {:?}/{} null {:?} {}",
            c.verdict,
            c.order,
            c.det_order,
            c.rank_at_d0,
            c.generic_rank,
            c.null_directions,
            if d.is_empty() {
                "AGREES".into()
            } else {
                d.join("; ")
            }
        );
        if !d.is_empty() {
            disagreements.push(format!("{node}: {}", d.join("; ")));
        }
        done += 1;
    }
    assert!(
        disagreements.is_empty(),
        "disagreements with the helper's data: {disagreements:#?}"
    );
    eprintln!("reclist nodes checked: {done}");
}
