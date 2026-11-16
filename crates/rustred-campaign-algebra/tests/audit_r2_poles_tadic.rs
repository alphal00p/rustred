//! AUDIT r2-poles (independent auditor, question 3): lane tadic's system pole order p* on systems whose answer is
//! known BY CONSTRUCTION (hand derivations in the comments; t = d - d0), including systems where a naive elimination
//! over Q(d) (one pivot row for the target) gives a worse order than the optimum, rank drops of the upper block at d0,
//! long chains (p* = 3), min over alternative paths, d0 rational, monotonicity in the rows and invariance under row /
//! column relabelling and constant row scaling. Every result is also passed through lane tadic's own check mode (the
//! dual certificate) and the claimed relation's pole order is compared with p*. Tests SKIP without the licence.

mod common;

use rustred_campaign_algebra::tadic::SymbolicaTadic;
use rustred_campaign_algebra::tadic::check::check_result;
use rustred_campaign_core::tadic::{
    Arith, ExactSystem, Interest, PStar, PoleOrderOracle, PoleQuery, Rat, TadicBudget, Verdict,
};

/// One-slot keys: [0] = K, [-1..-9] lower (rhs allowed), [1..] upper (must be eliminated).
fn lower(k: &[i64]) -> bool {
    k[0] < 0
}

/// Entry `c0 + c1 t` with t = d - p/q: as (a + b d) with integer a, b after scaling by q (rows may be scaled).
fn ent(key: i64, c0: i128, c1: i128, p: i128, q: i128) -> (Vec<i64>, i128, i128) {
    // q (c0 + c1 (d - p/q)) = q c0 - c1 p + q c1 d
    (vec![key], q * c0 - c1 * p, q * c1)
}

fn run(rows: &[Vec<(Vec<i64>, i128, i128)>], d0: Rat) -> (PStar, Option<bool>, u32) {
    let sys = ExactSystem::from_rows(1, rows, "audit-constructed");
    let target = [0i64];
    let q = PoleQuery {
        target: &target,
        lower: &lower,
        d0,
        interest: Interest::AllLower,
        arith: Arith::Exact,
        budget: TadicBudget::default(),
        want_relation: true,
    };
    let mut o = SymbolicaTadic::new().expect("licensed");
    o.relation_orders = 3;
    let r = o.best_pole_order(&sys, &q);
    assert_eq!(r.verdict, Verdict::Exact, "{:?}", r.notes);
    let p = r.p_star.expect("p*");
    let rep = check_result(&sys, &q, &r, 2_000_000).expect("check mode");
    assert!(rep.consistent, "check mode: {rep:?} for {p:?}");
    let rel_order = r
        .relation
        .as_ref()
        .map_or(u32::MAX, |x| x.profile.max_order);
    if let PStar::Order(x) = p {
        assert_eq!(rel_order, x, "the returned relation does not attain p*");
    }
    (p, r.screen_at_d0, rel_order)
}

fn four() -> (i128, i128) {
    (4, 1)
}

#[test]
fn audit_tadic_naive_single_pivot_is_worse_than_the_optimum() {
    if !common::licensed_or_skip("audit_tadic_naive") {
        return;
    }
    let (p, q) = four();
    // r1 = (K: t, S1: 1), r2 = (S1: 1, S2: t). A pivot on r1 for K gives I(K) = -I(S1)/t (order 1); the optimum is
    // (r1 - r2)/t = (K: 1, S2: -1): I(K) = I(S2), REGULAR. p* = 0.
    let rows = vec![
        vec![ent(0, 0, 1, p, q), ent(-1, 1, 0, p, q)],
        vec![ent(-1, 1, 0, p, q), ent(-2, 0, 1, p, q)],
    ];
    assert_eq!(run(&rows, Rat::int(4)).0, PStar::Order(0));
    // same with the lower columns introduced in the other order (column indices permuted)
    let rows2 = vec![rows[1].clone(), rows[0].clone()];
    assert_eq!(run(&rows2, Rat::int(4)).0, PStar::Order(0));
}

#[test]
fn audit_tadic_chain_and_min_over_paths() {
    if !common::licensed_or_skip("audit_tadic_chain") {
        return;
    }
    let (p, q) = four();
    // chain: (K: t, U1: 1), (U1: t, U2: 1), (U2: t, S1: 1)  =>  U1 = -t K, U2 = t^2 K, S1 = -t^3 K: I(K) = -I(S1)/t^3.
    let chain = vec![
        vec![ent(0, 0, 1, p, q), ent(1, 1, 0, p, q)],
        vec![ent(1, 0, 1, p, q), ent(2, 1, 0, p, q)],
        vec![ent(2, 0, 1, p, q), ent(-1, 1, 0, p, q)],
    ];
    assert_eq!(run(&chain, Rat::int(4)).0, PStar::Order(3));
    // a second path (K: t, S2: 1): V = span{(t^3, -1, 0), (t, 0, 1)} on (K, S1, S2); integral x has x_S1, x_S2 in R,
    // so x_K in t R: p* = 1 (monotone: more rows, p* can only fall)
    let mut two = chain.clone();
    two.push(vec![ent(0, 0, 1, p, q), ent(-2, 1, 0, p, q)]);
    assert_eq!(run(&two, Rat::int(4)).0, PStar::Order(1));
    // a third row making the two singular relations combine regularly: (S1: 1, S2: -t^2 ... ) is not affine, so use
    // (S2: 1, S3: t): then (K: t, S2: 1) - (S2: 1, S3: t) = t (K: 1, S3: -1): I(K) = I(S3), p* = 0
    let mut three = two.clone();
    three.push(vec![ent(-2, 1, 0, p, q), ent(-3, 0, 1, p, q)]);
    assert_eq!(run(&three, Rat::int(4)).0, PStar::Order(0));
}

#[test]
fn audit_tadic_rank_drop_of_the_upper_block_at_d0() {
    if !common::licensed_or_skip("audit_tadic_rank_drop") {
        return;
    }
    let (p, q) = four();
    // r1 = (K: t, U: t), r2 = (U: t, S1: 1): upper block [t; t] has rank 1 over Q(d), 0 at d0. r1 - r2 = (K: t, S1: -1):
    // I(K) = I(S1)/t, the only relation: p* = 1. At d0 the rows are (0) and (S1: 1): no relation at d0.
    let rows = vec![
        vec![ent(0, 0, 1, p, q), ent(1, 0, 1, p, q)],
        vec![ent(1, 0, 1, p, q), ent(-1, 1, 0, p, q)],
    ];
    let (ps, screen, _) = run(&rows, Rat::int(4));
    assert_eq!(ps, PStar::Order(1));
    assert_ne!(
        screen,
        Some(true),
        "no admissible relation exists at d0 here"
    );
    // r1 = (K: 1, U: t), r2 = (U: t, S1: 1): r1 - r2 = (K: 1, S1: -1): p* = 0, while at d0 r1 alone reads "I(K) = 0"
    // (a d0 relation that does NOT lift: the exact relation is I(K) = I(S1))
    let rows = vec![
        vec![ent(0, 1, 0, p, q), ent(1, 0, 1, p, q)],
        vec![ent(1, 0, 1, p, q), ent(-1, 1, 0, p, q)],
    ];
    assert_eq!(run(&rows, Rat::int(4)).0, PStar::Order(0));
    // no relation at all: K appears only together with an upper column that nothing else eliminates
    let rows = vec![
        vec![ent(0, 1, 0, p, q), ent(1, 1, 0, p, q)],
        vec![ent(2, 0, 1, p, q), ent(-1, 1, 0, p, q)],
    ];
    assert_eq!(run(&rows, Rat::int(4)).0, PStar::NoRelation);
}

#[test]
fn audit_tadic_rational_d0_and_relabelling_invariance() {
    if !common::licensed_or_skip("audit_tadic_rational_d0") {
        return;
    }
    // the chain of the previous test with t = d - 9/2 (entries 2d - 9): p* = 3 at 9/2 and 0 at d0 = 4 (t is a unit)
    let (p, q) = (9, 2);
    let chain = vec![
        vec![ent(0, 0, 1, p, q), ent(1, 1, 0, p, q)],
        vec![ent(1, 0, 1, p, q), ent(2, 1, 0, p, q)],
        vec![ent(2, 0, 1, p, q), ent(-1, 1, 0, p, q)],
    ];
    assert_eq!(run(&chain, Rat::new(9, 2).unwrap()).0, PStar::Order(3));
    assert_eq!(run(&chain, Rat::int(4)).0, PStar::Order(0));
    // relabelling: rows reversed, each row scaled by a constant, upper keys renamed, lower keys renamed
    let scaled: Vec<Vec<(Vec<i64>, i128, i128)>> = chain
        .iter()
        .rev()
        .enumerate()
        .map(|(i, r)| {
            r.iter()
                .map(|(k, a, b)| {
                    let k2 = if k[0] > 0 {
                        k[0] + 10
                    } else if k[0] < 0 {
                        k[0] - 5
                    } else {
                        0
                    };
                    (vec![k2], a * (i as i128 + 2) * -3, b * (i as i128 + 2) * -3)
                })
                .collect()
        })
        .collect();
    assert_eq!(run(&scaled, Rat::new(9, 2).unwrap()).0, PStar::Order(3));
}

#[test]
fn audit_tadic_redundant_rows_do_not_change_pstar() {
    if !common::licensed_or_skip("audit_tadic_redundant") {
        return;
    }
    let (p, q) = four();
    // the min-over-paths system (p* = 1) plus the sum of its first two rows and a constant multiple of its last one
    let mut rows = vec![
        vec![ent(0, 0, 1, p, q), ent(1, 1, 0, p, q)],
        vec![ent(1, 0, 1, p, q), ent(2, 1, 0, p, q)],
        vec![ent(2, 0, 1, p, q), ent(-1, 1, 0, p, q)],
        vec![ent(0, 0, 1, p, q), ent(-2, 1, 0, p, q)],
    ];
    let mut sum = rows[0].clone();
    sum.extend(rows[1].clone());
    rows.push(sum);
    rows.push(
        rows[3]
            .iter()
            .map(|(k, a, b)| (k.clone(), 7 * a, 7 * b))
            .collect(),
    );
    assert_eq!(run(&rows, Rat::int(4)).0, PStar::Order(1));
}
