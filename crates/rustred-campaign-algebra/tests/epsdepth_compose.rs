//! Lane epsdepth: composed pole profiles on CONSTRUCTED nodes (interfaces/epsdepth.md 4-7). Exact (Symbolica
//! `Series<Q>`); needs the Symbolica licence (`source WS/ops/env.sh`), skips otherwise.
//! t = d - 4 throughout unless stated.

use rustred_campaign_algebra::epsdepth::{SymbolicaExact, SymbolicaLaurent, TextReduction};
use rustred_campaign_algebra::{SymbolicaAlgebra, licensed};
use rustred_campaign_core::epsdepth::{
    ComposeParams, EpsDepthSpec, Order, PairVerdict, TerminalSetStatus, compose, node_report,
    onto_terminals, verdict,
};
use serde_json::json;
use symbolica::domains::rational::Rational;

fn setup(d0: &str) -> Option<TextReduction> {
    if !licensed() {
        eprintln!("SKIP: no Symbolica licence (source WS/ops/env.sh)");
        return None;
    }
    let alg = SymbolicaAlgebra::new().ok()?;
    let lau = SymbolicaLaurent::new(d0).unwrap();
    Some(TextReduction::new(alg, lau, 2))
}

const T1: &[i32] = &[1, 0];
const T2: &[i32] = &[1, -1];
const A: &[i32] = &[3, 0];
const B: &[i32] = &[2, 0];
const C: &[i32] = &[4, 0];

fn run(
    src: &mut TextReduction,
    keys: &[&[i32]],
    p: &ComposeParams,
) -> rustred_campaign_core::epsdepth::Composition {
    let mut lau = SymbolicaLaurent::new("4").unwrap();
    let sel: Vec<usize> = keys.iter().map(|k| src.index_of(k).unwrap()).collect();
    let n = src.n_terminals();
    compose(src, &mut lau, &sel, n, p).unwrap()
}

#[test]
fn a_local_pole_that_cancels_is_spurious() {
    let Some(mut s) = setup("4") else { return };
    s.terminal(T1);
    // B = (1 + t) T1 ; A = B / t - T1 / t = T1  (both of A's terms have a simple pole)
    s.relation(B, "rule-B", &[(T1, "d-3")]);
    s.relation(A, "rule-A", &[(B, "1/(d-4)"), (T1, "-1/(d-4)")]);
    let c = run(&mut s, &[A], &ComposeParams::default());
    let r = &c.results[0];
    assert_eq!(r.pairs[0].order, Order::Pole(0));
    assert_eq!(r.local_pole, 1);
    assert!(r.local_pole_in_cone);
    assert_eq!(
        r.pairs[0].bound,
        Some(1),
        "path bound = 1, composed 0: cancelled"
    );
    // target 0 decides every negative power: the coefficient is known zero below t^0, its t^0 term is not asked for
    assert_eq!(r.pairs[0].leading, None);
    assert_eq!(r.pairs[0].precision, Some(0));
    assert_eq!(
        verdict(
            &r.pairs[0],
            r.local_pole_in_cone,
            TerminalSetStatus::NotCertified
        ),
        PairVerdict::Spurious
    );
    // depth control: target 2 gives the exact coefficient to t^1: exactly 1
    let c2 = run(
        &mut s,
        &[A],
        &ComposeParams {
            target: 2,
            ..Default::default()
        },
    );
    assert_eq!(c2.results[0].pairs[0].leading.as_deref(), Some("1"));
    assert_eq!(c2.results[0].pairs[0].valuation, Some(0));
    assert_eq!(c2.results[0].pairs[0].precision, Some(2));
    assert_eq!(c.stats.singular_edges, 2);
}

#[test]
fn a_pole_that_survives_is_on_t_only_unless_the_set_is_certified() {
    let Some(mut s) = setup("4") else { return };
    s.terminal(T1);
    s.relation(B, "rule-B", &[(T1, "1")]);
    s.relation(A, "rule-A", &[(B, "1/(d-4)")]);
    s.relation(C, "rule-C", &[(A, "2")]);
    let c = run(&mut s, &[A, C], &ComposeParams::default());
    for r in &c.results {
        assert_eq!(r.pairs[0].order, Order::Pole(1));
        assert_eq!(
            r.pairs[0].blame_server.as_deref(),
            Some("rule-A"),
            "the pole enters at A's relation"
        );
        assert_eq!(
            verdict(
                &r.pairs[0],
                r.local_pole_in_cone,
                TerminalSetStatus::NotCertified
            ),
            PairVerdict::OnTOnly(1)
        );
        assert_eq!(
            verdict(
                &r.pairs[0],
                r.local_pole_in_cone,
                TerminalSetStatus::CertifiedRegular
            ),
            PairVerdict::Inherent(1)
        );
    }
    assert_eq!(c.results[1].pairs[0].leading.as_deref(), Some("2"));
    // the API for other lanes: the exact vector of C on the terminals, to t^1 (target 2)
    let mut lau = SymbolicaLaurent::new("4").unwrap();
    let ci = s.index_of(C).unwrap();
    let v = onto_terminals(
        &mut s,
        &mut lau,
        ci,
        1,
        &ComposeParams {
            target: 2,
            ..Default::default()
        },
    )
    .unwrap()
    .unwrap();
    let s0 = v[0].as_ref().unwrap();
    assert_eq!(SymbolicaLaurent::series_order(s0), (Some(-1), 2));
    assert_eq!(
        SymbolicaLaurent::series_coefficient(s0, -1),
        Some(Rational::from(2))
    );
    assert_eq!(
        SymbolicaLaurent::series_coefficient(s0, 0),
        Some(Rational::from(0))
    );
    assert_eq!(
        SymbolicaLaurent::series_coefficient(s0, 2),
        None,
        "beyond the precision"
    );
}

#[test]
fn index_dependent_poles_are_decided_at_the_key() {
    let Some(mut s) = setup("4") else { return };
    s.terminal(T1);
    // one "rule text" (d - 2 n0) in the denominator: a pole at n0 = 2 only (TRICKS 9)
    s.relation(B, "rule-R", &[(T1, "1/(d-2*n0)")]);
    s.relation(A, "rule-R", &[(T1, "1/(d-2*n0)")]);
    let c = run(&mut s, &[A, B], &ComposeParams::default());
    assert_eq!(
        c.results[0].pairs[0].order,
        Order::Pole(0),
        "n0 = 3: 1/(d-6) regular at 4"
    );
    assert_eq!(
        c.results[1].pairs[0].order,
        Order::Pole(1),
        "n0 = 2: 1/(d-4)"
    );
    assert_eq!(c.results[0].local_pole, 0);
    assert_eq!(c.results[1].local_pole, 1);
}

#[test]
fn order_two_poles_cancel_only_with_enough_depth_and_a_rerun_decides() {
    let Some(mut s) = setup("4") else { return };
    s.terminal(T1);
    // B = (1 + t + t^2) T1 ; A = B/t^2 - T1/t^2 - T1/t = T1: needs vec(B) to t^1 exactly (R(B) = 2)
    s.relation(B, "rule-B", &[(T1, "1 + (d-4) + (d-4)^2")]);
    s.relation(
        A,
        "rule-A",
        &[(B, "1/(d-4)^2"), (T1, "-1/(d-4)^2 - 1/(d-4)")],
    );
    let c = run(&mut s, &[A], &ComposeParams::default());
    assert_eq!(c.results[0].pairs[0].order, Order::Pole(0));
    assert_eq!(c.results[0].local_pole, 2);
    assert_eq!(c.stats.max_required, 2);
    // a depth budget of 1 cannot decide it: undecided (never a wrong order) ...
    let capped = ComposeParams {
        max_depth: 1,
        ..Default::default()
    };
    let c = run(&mut s, &[A], &capped);
    assert!(
        matches!(&c.results[0].pairs[0].order, Order::Undecided(r) if r.contains("depth")),
        "{:?}",
        c.results[0]
    );
    assert_eq!(c.stats.capped_keys, 1);
    // ... and the rerun with the budget raised decides it
    let c = run(
        &mut s,
        &[A],
        &ComposeParams {
            max_depth: 2,
            ..Default::default()
        },
    );
    assert_eq!(c.results[0].pairs[0].order, Order::Pole(0));
    // a surviving order-2 pole
    s.relation(C, "rule-C", &[(T1, "(d-1)/(d-4)^2")]);
    let c = run(&mut s, &[C], &ComposeParams::default());
    assert_eq!(c.results[0].pairs[0].order, Order::Pole(2));
    assert_eq!(c.results[0].pairs[0].leading.as_deref(), Some("3"));
}

#[test]
fn open_keys_cycles_zero_coefficients_and_other_d0() {
    let Some(mut s) = setup("4") else { return };
    s.terminal(T1);
    s.open(B, "uncovered");
    s.relation(A, "rule-A", &[(B, "1"), (T1, "1")]);
    let c = run(&mut s, &[A], &ComposeParams::default());
    assert!(matches!(&c.results[0].pairs[0].order, Order::Undecided(r) if r.contains("open")));
    // a zero coefficient is no edge: the open key is not reached
    s.relation(A, "rule-A", &[(B, "d-d"), (T1, "1")]);
    let c = run(&mut s, &[A], &ComposeParams::default());
    assert_eq!(c.results[0].pairs[0].order, Order::Pole(0));
    // a cycle is an error
    let Some(mut s) = setup("4") else { return };
    s.terminal(T1);
    s.relation(B, "rule-B", &[(A, "1")]);
    s.relation(A, "rule-A", &[(B, "1")]);
    let mut lau = SymbolicaLaurent::new("4").unwrap();
    let a = s.index_of(A).unwrap();
    assert!(
        compose(&mut s, &mut lau, &[a], 1, &ComposeParams::default())
            .unwrap_err()
            .contains("cycle")
    );
    // d0 = 17/7: the pole moves
    let Some(mut s) = setup("17/7") else { return };
    s.terminal(T1);
    s.relation(A, "rule-A", &[(T1, "1/(7*d-17)")]);
    let mut lau = SymbolicaLaurent::new("17/7").unwrap();
    let a = s.index_of(A).unwrap();
    let c = compose(&mut s, &mut lau, &[a], 1, &ComposeParams::default()).unwrap();
    assert_eq!(c.results[0].pairs[0].order, Order::Pole(1));
    assert_eq!(c.results[0].pairs[0].leading.as_deref(), Some("1/7"));
}

#[test]
fn eps_box_within_and_exceeds_and_the_node_report() {
    let Some(mut s) = setup("4") else { return };
    s.terminal(T1);
    s.terminal(T2);
    s.relation(B, "rule-B", &[(T1, "1/(d-4)^2"), (T2, "d")]);
    s.relation(A, "rule-A", &[(B, "1"), (T2, "1/(d-4)")]);
    let spec = EpsDepthSpec::from_value(&json!({
        "eps_box": {"available": {"default": 1, "keys": [{"key": [1, -1], "order": 0}]}, "needed_order": 0}
    }))
    .unwrap();
    let c = run(&mut s, &[A, B], &ComposeParams::default());
    let keys = s.keys.clone();
    let (h, rows) = node_report(
        &c,
        &spec,
        &s.terminal_keys(),
        &|i| keys[i].clone(),
        &[true, false],
        "toy",
    )
    .unwrap();
    // A: T1 order 2 (needs 2 > 1: exceeds by 1), T2 order 1 (needs 1 > 0: exceeds by 1); B: T1 order 2, T2 regular
    let ra = rows.iter().find(|r| r["key"] == json!(A)).unwrap();
    assert_eq!(ra["orders"], json!([2, 1]));
    assert_eq!(ra["eps"], json!(["exceeds:1", "exceeds:1"]));
    let rb = rows.iter().find(|r| r["key"] == json!(B)).unwrap();
    assert_eq!(rb["eps"], json!(["exceeds:1", "within"]));
    let t1 = &h["per_terminal"][0];
    assert_eq!(t1["max_order"], json!(2));
    assert_eq!(t1["eps_box"]["max_required"], json!(2));
    assert_eq!(t1["eps_box"]["keys_setting_it"], json!(2));
    assert_eq!(t1["eps_box"]["exceeding"], json!(2));
    assert_eq!(h["per_terminal"][1]["eps_box"]["max_required"], json!(1));
    assert_eq!(h["verdicts"]["on_t_only"], json!(3));
    // a terminal the box does not cover is a typed error, never a default
    let spec2 = EpsDepthSpec::from_value(
        &json!({"eps_box": {"available": {"keys": [{"key": [1, 0], "order": 3}]}}}),
    )
    .unwrap();
    assert!(
        node_report(
            &c,
            &spec2,
            &s.terminal_keys(),
            &|i| keys[i].clone(),
            &[true, false],
            "toy"
        )
        .is_err()
    );
}

#[test]
fn deterministic_report_and_streaming_equals_memoised() {
    let Some(mut s) = setup("4") else { return };
    s.terminal(T1);
    s.terminal(T2);
    s.relation(B, "rule-B", &[(T1, "(d-3)/(d-4)"), (T2, "1")]);
    s.relation(A, "rule-A", &[(B, "(d-4)"), (T1, "-1"), (T2, "1/(d-4)")]);
    s.relation(C, "rule-C", &[(A, "1"), (B, "1/(d-4)")]);
    let spec = EpsDepthSpec::default();
    let keys = s.keys.clone();
    let one = |s: &mut TextReduction, order: &[&[i32]]| {
        let c = run(s, order, &ComposeParams::default());
        let (h, mut rows) = node_report(
            &c,
            &spec,
            &s.terminal_keys(),
            &|i| keys[i].clone(),
            &[true, false],
            "toy",
        )
        .unwrap();
        rows.sort_by_key(|r| r["key"].to_string());
        let mut h = h;
        h["seconds"] = json!(null);
        (h, rows)
    };
    let x = one(&mut s, &[A, B, C]);
    // phase 3 re-reading the source (keep_coefficients false) gives the same composition
    let c1 = run(&mut s, &[A, B, C], &ComposeParams::default());
    let c2 = run(
        &mut s,
        &[A, B, C],
        &ComposeParams {
            keep_coefficients: false,
            ..Default::default()
        },
    );
    assert_eq!(c1.results, c2.results);
    let y = one(&mut s, &[C, B, A]);
    assert_eq!(x.1, y.1);
    assert_eq!(x.0["per_terminal"], y.0["per_terminal"]);
    assert_eq!(x, one(&mut s, &[A, B, C]));
}

#[test]
fn the_exact_backend_agrees_with_the_truncated_one() {
    let Some(mut s) = setup("4") else { return };
    s.terminal(T1);
    s.terminal(T2);
    s.relation(
        B,
        "rule-B",
        &[(T1, "1 + (d-4) + (d-4)^2"), (T2, "(d-3)/(d-4)")],
    );
    s.relation(
        A,
        "rule-A",
        &[
            (B, "1/(d-4)^2"),
            (T1, "-1/(d-4)^2 - 1/(d-4)"),
            (T2, "(d-5)/(d-4)^3"),
        ],
    );
    s.relation(C, "rule-C", &[(A, "(d-4)"), (B, "1/(d-4)")]);
    let sel: Vec<usize> = [A, B, C].iter().map(|k| s.index_of(k).unwrap()).collect();
    let mut lau = SymbolicaLaurent::new("4").unwrap();
    let a = compose(&mut s, &mut lau, &sel, 2, &ComposeParams::default()).unwrap();
    let mut ex = SymbolicaExact::new().unwrap();
    let b = compose(&mut s, &mut ex, &sel, 2, &ComposeParams::default()).unwrap();
    for (x, y) in a.results.iter().zip(&b.results) {
        let ox: Vec<_> = x.pairs.iter().map(|p| p.order.clone()).collect();
        let oy: Vec<_> = y.pairs.iter().map(|p| p.order.clone()).collect();
        assert_eq!(ox, oy, "key {}", x.key);
        assert!(
            y.pairs.iter().all(|p| p.precision.is_none()),
            "exact: infinite precision"
        );
    }
    // A = T1 + (d-5)/t^3 T2 + B-part (d-3)/t^3 T2: T2 coefficient (2d-8)/t^3 = 2/t^2: order 2, leading 2
    assert_eq!(b.results[0].pairs[1].order, Order::Pole(2));
    assert_eq!(b.results[0].pairs[1].leading.as_deref(), Some("2"));
    // the exact vector of A on the terminals
    let ai = s.index_of(A).unwrap();
    let v = onto_terminals(&mut s, &mut ex, ai, 2, &ComposeParams::default())
        .unwrap()
        .unwrap();
    assert_eq!(SymbolicaExact::order_of(v[0].as_ref().unwrap()), Some(0));
    assert_eq!(
        SymbolicaExact::leading(v[0].as_ref().unwrap()),
        Some(Rational::from(1))
    );
}

/// Lane polefix (audit r2-poles F8, F9). F8: two NON-TERMINAL paths whose poles cancel only in their SUM: the
/// composition must add the contributions (the auditor's mutation M2, "sum replaced by the larger pole", turns the
/// regular coefficient 2 into a simple pole). F9: an exactly cancelled coefficient is `Zero` with the exact backend,
/// never `Pole(0)`; the truncated backend can only say "no pole to its precision".
#[test]
fn poles_of_two_paths_cancel_in_the_sum_and_an_exact_cancellation_is_zero() {
    let Some(mut s) = setup("4") else { return };
    const D: &[i32] = &[7, 0];
    const E: &[i32] = &[6, 0];
    s.terminal(T1);
    // A = T1 / t ; B = (2 - 1/t) T1 ; C = A + B = 2 T1 (both paths singular, the sum regular)
    s.relation(A, "rule-A", &[(T1, "1/(d-4)")]);
    s.relation(B, "rule-B", &[(T1, "2 - 1/(d-4)")]);
    s.relation(C, "rule-C", &[(A, "1"), (B, "1")]);
    // E = -T1 / t ; D = A + E = 0 exactly
    s.relation(E, "rule-E", &[(T1, "-1/(d-4)")]);
    s.relation(D, "rule-D", &[(A, "1"), (E, "1")]);
    let sel: Vec<usize> = [C, D].iter().map(|k| s.index_of(k).unwrap()).collect();
    let mut lau = SymbolicaLaurent::new("4").unwrap();
    let tr = compose(&mut s, &mut lau, &sel, 1, &ComposeParams::default()).unwrap();
    let mut ex = SymbolicaExact::new().unwrap();
    let exr = compose(&mut s, &mut ex, &sel, 1, &ComposeParams::default()).unwrap();
    // C: regular on both backends (M2 would give Pole(1)); the exact one gives the coefficient 2
    assert_eq!(tr.results[0].pairs[0].order, Order::Pole(0));
    assert_eq!(exr.results[0].pairs[0].order, Order::Pole(0));
    assert_eq!(exr.results[0].pairs[0].leading.as_deref(), Some("2"));
    assert_eq!(
        exr.results[0].pairs[0].bound,
        Some(1),
        "each path has a pole"
    );
    // D: exactly zero on the exact backend (F9); "no pole to its precision" on the truncated one
    assert_eq!(exr.results[1].pairs[0].order, Order::Zero);
    assert_eq!(exr.results[1].pairs[0].leading, None);
    assert_eq!(tr.results[1].pairs[0].order, Order::Pole(0));
    assert!(tr.results[1].pairs[0].precision.is_some());
}
