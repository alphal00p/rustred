//! Lane tadicjob (interfaces/tadic-job.md 4): the rank step on results whose rank is known BY CONSTRUCTION (node-level
//! bases, synthetic series, true primes): two keys with the same polar row give rank 1; a dependence that cancels that
//! row gives rank 0; an independent key gives rank 2; typed NOT DECIDED cases; the Gamma unit against
//! Gamma(1+x) Gamma(1-x) = pi x / sin(pi x); catalogue rows parsed by Symbolica. Needs the Symbolica licence (skips
//! without it, like every test of this crate). Test data only (L8).

use rustred_campaign_algebra::epsdepth::SymbolicaLaurent;
use rustred_campaign_algebra::polerank::{
    DepthTable, Dependences, Fp, RankData, TerminalMap, cell_rank, row_products,
};
use rustred_campaign_algebra::{SymbolicaAlgebra, licensed};
use serde_json::{Value, json};
use symbolica::domains::finite_field::FiniteFieldCore;
use std::collections::BTreeMap;

const P: [(u64, u64); 4] = [
    (2_305_843_009_213_693_951, 1),
    (4_611_686_018_427_387_847, 1),
    (1_152_921_504_606_846_883, 2),
    (576_460_752_303_423_433, 2),
];

fn md(x: i128, p: u64) -> String {
    x.rem_euclid(p as i128).to_string()
}

/// A term with residue `a` at t^v (precision `prec`) at every prime of the seeds used.
fn term(basis: &str, v: i64, a: i128, prec: i64, seeds: &[u64]) -> Value {
    let pp: Vec<Value> = P
        .iter()
        .filter(|(_, s)| seeds.contains(s))
        .map(|&(p, s)| {
            let n = (prec - v) as usize;
            let mut c = vec!["0".to_string(); n];
            c[0] = md(a, p);
            if a.rem_euclid(p as i128) == 0 {
                json!({"prime": p.to_string(), "seed": s, "valuation": null, "precision": prec, "coefficients": []})
            } else {
                json!({"prime": p.to_string(), "seed": s, "valuation": v, "precision": prec, "coefficients": c})
            }
        })
        .collect();
    json!({"basis": basis, "tag": "", "kind": "master", "per_prime": pp})
}

/// Request = sum_k w_k key_k, key_k = (a_k at t^-1 on master A, b_k at t^-2 on master B).
fn request(label: &str, keys: &[(i128, i128)], w: &[i128], seeds: &[u64]) -> Value {
    let a: i128 = keys.iter().zip(w).map(|(k, w)| k.0 * w).sum();
    let b: i128 = keys.iter().zip(w).map(|(k, w)| k.1 * w).sum();
    json!({"label": label, "status": "OK", "terms": [term("I(1,1,0)", -1, a, 1, seeds), term("I(1,0,1)", -2, b, 1, seeds)],
        "certificate": seeds.iter().map(|s| json!({"seed": s, "graph_digest": "g", "plan_digest": "p", "plan": {"max_need": 3, "max_length": 4}})).collect::<Vec<_>>()})
}

fn result(reqs: Vec<Value>) -> Value {
    json!({"schema": "helper-vh2.tadic-result.v1", "status": "OK", "expansion": {"d0": 4, "order": 1}, "requests": reqs})
}

fn depths() -> DepthTable {
    DepthTable::from_json(&json!({"schema": "rustred.campaign.known-depth.v1", "label": "toy",
        "masters": {"I(1,1,0)": {"known_through": 0}, "I(1,0,1)": {"known_through": 1}}})).unwrap()
}

fn combos(keys: &[(i128, i128)], m: usize, seeds: &[u64]) -> Vec<Value> {
    (0..m)
        .map(|c| {
            let w: Vec<i128> = (0..keys.len()).map(|k| (1 + 7 * c as i128 + 13 * k as i128 + (c * k) as i128 * 31) % 1009).collect();
            request(&format!("c{c}"), keys, &w, seeds)
        })
        .collect()
}

#[test]
fn ranks_known_by_construction() {
    if !licensed() {
        eprintln!("SKIP: Symbolica licence absent");
        return;
    }
    let lau = SymbolicaLaurent::new("4").unwrap();
    let map = TerminalMap::default();
    let dep = depths();
    let data = RankData { map: &map, depths: &dep, dependences: None, lau: &lau };
    // two keys with the SAME polar row (proportional): rank 1, pivot on the first symbol
    let same = [(5, 7), (10, 14)];
    let r = cell_rank(&data, &result(combos(&same, 4, &[1, 2])), "c", 1);
    assert_eq!(r["rank"], 1, "{r}");
    assert_eq!(r["verdict"], "count zero: no (rank 1)");
    assert_eq!(r["per_prime"].as_array().unwrap().len(), 4);
    // needs: A pole order 1 > q 0, B pole order 2 > q 1
    let beyond: Vec<&str> = r["cell_beyond"].as_array().unwrap().iter().map(|b| b["master"].as_str().unwrap()).collect();
    assert_eq!(beyond, vec!["I(1,0,1)", "I(1,1,0)"]);
    // a dependence that cancels that row: (A,1) -> S, (B,2) -> (2 a / b) S with eps entries -a/2 and b/4 -> rank 0
    let deps = Dependences::from_json(&json!({"schema": "rustred.campaign.dependences.v1", "label": "toy",
        "entries": [{"master": "I(1,1,0)", "order": 1, "symbols": {"S": "1"}},
                    {"master": "I(1,0,1)", "order": 2, "symbols": {"S": "10/7"}}]})).unwrap();
    let data_d = RankData { map: &map, depths: &dep, dependences: Some(&deps), lau: &lau };
    let r0 = cell_rank(&data_d, &result(combos(&same, 4, &[1, 2])), "c", 1);
    assert_eq!(r0["rank"], 0, "{r0}");
    assert_eq!(r0["verdict"], "count zero: yes");
    assert!(r0["rank_kind"].as_str().unwrap().starts_with("rank of rho"));
    // an independent key: rank 2 (needs >= 5 combinations to be stable)
    let indep = [(5, 7), (1, 3)];
    let r2 = cell_rank(&data, &result(combos(&indep, 5, &[1, 2])), "c", 1);
    assert_eq!(r2["rank"], 2, "{r2}");
    let r2s = cell_rank(&data, &result(combos(&indep, 4, &[1, 2])), "c", 1);
    assert!(r2s["verdict"].as_str().unwrap().contains("NEEDS_MORE_COMBINATIONS"), "{r2s}");
}

#[test]
fn typed_not_decided_cases() {
    if !licensed() {
        eprintln!("SKIP: Symbolica licence absent");
        return;
    }
    let lau = SymbolicaLaurent::new("4").unwrap();
    let map = TerminalMap::default();
    let dep = depths();
    let data = RankData { map: &map, depths: &dep, dependences: None, lau: &lau };
    // one seed only
    let r = cell_rank(&data, &result(combos(&[(5, 7)], 4, &[1])), "c", 1);
    assert_eq!(r["verdict"], "not decided (SINGLE_SEED)");
    // a request not OK
    let mut reqs = combos(&[(5, 7)], 4, &[1, 2]);
    reqs[2] = json!({"label": "c2", "status": "ERROR", "error": {"code": "LENGTH_BUDGET"}});
    let r = cell_rank(&data, &result(reqs), "c", 1);
    assert!(r["verdict"].as_str().unwrap().contains("REQUEST:LENGTH_BUDGET"), "{r}");
    // a master without a known depth that has a pole
    let mut reqs = combos(&[(5, 7)], 4, &[1, 2]);
    reqs[0]["terms"].as_array_mut().unwrap().push(term("I(0,1,1)", -1, 3, 1, &[1, 2]));
    let r = cell_rank(&data, &result(reqs), "c", 1);
    assert!(r["verdict"].as_str().unwrap().contains("MISSING_DEPTH:I(0,1,1)"), "{r}");
    // precision not down to -q: B (q = 1) known only below t^-3
    let mut reqs = combos(&[(5, 7)], 4, &[1, 2]);
    reqs[1]["terms"][1] = term("I(1,0,1)", -4, 2, -3, &[1, 2]);
    let r = cell_rank(&data, &result(reqs), "c", 1);
    assert!(r["verdict"].as_str().unwrap().contains("PRECISION:I(1,0,1)"), "{r}");
}

#[test]
fn gamma_unit_is_pi_x_over_sin_pi_x() {
    if !licensed() {
        eprintln!("SKIP: Symbolica licence absent");
        return;
    }
    let p = P[0].0;
    let fp = Fp::new(p, 9).unwrap();
    let u = fp.gamma_unit(&BTreeMap::from([(1, 1), (-1, 1)]), 6).unwrap();
    let pi2 = fp.pi2() as u128;
    let c = |k: i64| fp.f.from_element(&fp.coeff(&u, k).unwrap()) as u128;
    let inv = |x: u128| {
        // x^(p-2) mod p
        let (mut b, mut e, mut r) = (x % p as u128, p as u128 - 2, 1u128);
        while e > 0 {
            if e & 1 == 1 {
                r = r * b % p as u128;
            }
            b = b * b % p as u128;
            e >>= 1;
        }
        r
    };
    let pm = p as u128;
    assert_eq!(c(0), 1);
    assert_eq!(c(1), 0);
    assert_eq!(c(2), pi2 * inv(6) % pm);
    assert_eq!(c(3), 0);
    assert_eq!(c(4), pi2 * pi2 % pm * 7 % pm * inv(360) % pm);
    // the empty tag is exactly 1
    let one = fp.gamma_unit(&BTreeMap::new(), 4).unwrap();
    assert_eq!(fp.val(&one), 0);
    assert_eq!(fp.f.from_element(&fp.coeff(&one, 2).unwrap()), 0);
}

#[test]
fn catalogue_rows_are_parsed_by_symbolica() {
    if !licensed() {
        eprintln!("SKIP: Symbolica licence absent");
        return;
    }
    let _alg = SymbolicaAlgebra::new().unwrap();
    let p = row_products("(1/(d - 3)^5)*PGamma(-2)^(-5)*PGamma(-1)^(10)*PGamma(1)^(5)").unwrap();
    assert_eq!(p.len(), 1);
    assert_eq!(p[0].2, BTreeMap::from([(-2, -5), (-1, 10), (1, 5)]));
    // a PGamma factor inside the rational part and a sum of two tags
    let p = row_products("PGamma(1)^5/((d - 6)) + (d)*PGamma(2)").unwrap();
    assert_eq!(p.len(), 2);
    let tags: Vec<BTreeMap<i64, i64>> = p.iter().map(|x| x.2.clone()).collect();
    assert!(tags.contains(&BTreeMap::from([(1, 5)])));
    assert!(tags.contains(&BTreeMap::from([(2, 1)])));
    // a denominator with two different Gamma monomials is refused
    assert!(row_products("1/(PGamma(1) + PGamma(2))").is_err());
}
