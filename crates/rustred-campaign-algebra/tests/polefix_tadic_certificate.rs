//! Lane polefix (audit r2-poles F12): lane tadic EXPORTS its dual (lower-bound) certificate next to the relation
//! (upper bound) in a self-contained file, and `check_certificate` verifies BOTH from the file alone (no oracle).
//! Systems with known p* by construction (the audit's chain, t = d - 4). Needs the licence; SKIPS without it.

mod common;

use rustred_campaign_algebra::tadic::SymbolicaTadic;
use rustred_campaign_algebra::tadic::check::{
    Certificate, certificate, check_certificate, read_certificate, write_certificate,
};
use rustred_campaign_core::tadic::{
    Arith, ExactSystem, Interest, PStar, PoleOrderOracle, PoleQuery, Rat, TadicBudget, Verdict,
};
use serde_json::json;

fn lower(k: &[i64]) -> bool {
    k[0] < 0
}

/// Entry `c0 + c1 t` at d0 = 4 as (a + b d).
fn ent(key: i64, c0: i128, c1: i128) -> (Vec<i64>, i128, i128) {
    (vec![key], c0 - 4 * c1, c1)
}

fn certify(rows: &[Vec<(Vec<i64>, i128, i128)>]) -> (PStar, Certificate) {
    let sys = ExactSystem::from_rows(1, rows, "polefix-constructed");
    let target = [0i64];
    let q = PoleQuery {
        target: &target,
        lower: &lower,
        d0: Rat::int(4),
        interest: Interest::AllLower,
        arith: Arith::Exact,
        budget: TadicBudget::default(),
        want_relation: true,
    };
    let o = SymbolicaTadic::new().expect("licensed");
    let r = o.best_pole_order(&sys, &q);
    assert_eq!(r.verdict, Verdict::Exact, "{:?}", r.notes);
    (r.p_star.unwrap(), certificate(&sys, &q, &r).unwrap())
}

fn scratch(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "rr-polefix-cert-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d.join(name)
}

#[test]
fn the_dual_certificate_is_exported_and_checked_from_the_file_alone() {
    if !common::licensed_or_skip("polefix_tadic_certificate") {
        return;
    }
    // chain (K: t, U1: 1), (U1: t, U2: 1), (U2: t, S1: 1): I(K) = -I(S1)/t^3, p* = 3
    let chain = vec![
        vec![ent(0, 0, 1), ent(1, 1, 0)],
        vec![ent(1, 0, 1), ent(2, 1, 0)],
        vec![ent(2, 0, 1), ent(-1, 1, 0)],
    ];
    let (p, cert) = certify(&chain);
    assert_eq!(p, PStar::Order(3));
    assert_eq!(cert.p_star, json!({"order": 3}));
    let dual = cert
        .dual
        .as_ref()
        .expect("p* > 0: a dual certificate is exported");
    assert_eq!(dual.len(), 1, "{dual:?}");
    assert!(
        cert.relation.is_some(),
        "the upper-bound relation is exported"
    );
    // the file alone
    let path = scratch("chain.json");
    write_certificate(&path, &cert).unwrap();
    let back = read_certificate(&path).unwrap();
    assert_eq!(back, cert);
    let rep = check_certificate(&back, 1_000_000).unwrap();
    assert!(rep.consistent, "{rep:?}");
    assert_eq!(rep.dual_ok, Some(true));
    assert_eq!(rep.dual_bound, Some(3));
    assert_eq!(rep.relation_in_span, Some(true));
    assert_eq!(rep.relation_order, Some(3));
    // a claim above the truth: the dual bound (3) does not reach it and the relation does not attain it
    let mut over = back.clone();
    over.p_star = json!({"order": 4});
    assert!(!check_certificate(&over, 1_000_000).unwrap().consistent);
    // a tampered dual vector (not orthogonal to V) is refused
    let mut bad = back.clone();
    bad.dual = Some(vec![(vec![-1], "1".into())]);
    let r = check_certificate(&bad, 1_000_000).unwrap();
    assert!(!r.consistent && r.dual_ok == Some(false), "{r:?}");
    // a claim p > 0 without its dual is not certified
    let mut none = back.clone();
    none.dual = None;
    assert!(!check_certificate(&none, 1_000_000).unwrap().consistent);
    // a claim below the truth: the relation does not attain it (the upper bound is the relation's own order)
    let mut under = back.clone();
    under.p_star = json!({"order": 2});
    assert!(!check_certificate(&under, 1_000_000).unwrap().consistent);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_second_path_lowers_pstar_and_its_certificate_checks() {
    if !common::licensed_or_skip("polefix_tadic_certificate_two") {
        return;
    }
    // + (K: t, S2: 1): p* = 1 (audit_r2_poles_tadic, chain_and_min_over_paths)
    let rows = vec![
        vec![ent(0, 0, 1), ent(1, 1, 0)],
        vec![ent(1, 0, 1), ent(2, 1, 0)],
        vec![ent(2, 0, 1), ent(-1, 1, 0)],
        vec![ent(0, 0, 1), ent(-2, 1, 0)],
    ];
    let (p, cert) = certify(&rows);
    assert_eq!(p, PStar::Order(1));
    let rep = check_certificate(&cert, 1_000_000).unwrap();
    assert!(rep.consistent, "{rep:?}");
    assert_eq!(rep.dual_bound, Some(1));
    // p* = 0: no dual is needed (none exported), the relation certifies
    let mut three = rows.clone();
    three.push(vec![ent(-2, 1, 0), ent(-3, 0, 1)]);
    let (p0, c0) = certify(&three);
    assert_eq!(p0, PStar::Order(0));
    assert!(c0.dual.is_none());
    assert!(check_certificate(&c0, 1_000_000).unwrap().consistent);
}
