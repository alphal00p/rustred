//! Row generator comparisons (lane I-ROWS):
//! - our raw IBP identities of T162 (the copied engine `Family::exact_row`) == the checker's own generator
//!   (W/z13c/pool `sym_rows`, regenerated read-only into tests/data/rows/checker_T162_rows.json by
//!   implementer-scratch/rows/scripts/dump_checker_rows.py; pool.py sha256 ce4aba1f..., ibp_shift.py 8f5e3324...),
//!   identity by identity, exactly over Q, in the same operator order (j over loops, v over loops then q);
//! - our scaleless line sectors (Lee criterion of the copied `Family::zero_mask`) == the checker's
//!   zero_sectors_T162P.json (490 of 512 line masks; sha256 352690b4...);
//! - the ADDED `rows_at_mod` (generic points) == the engine's `rows_at` at integer points, and both == the exact rows.
//!
//! The engine replay path itself is used unchanged for integer keys (`replay_base::rows::rows_for_p`, a copy).

mod rows_common;

use rows_common::*;
use rustred_campaign_core::replay_base::arith::{self, Q};
use rustred_campaign_core::replay_base::primes::{SplitMix64, verify_primes};
use serde_json::Value;

#[test]
fn rows_t162_raw_identities_equal_the_checkers() {
    let fam = t162_family();
    let n = fam.n;
    let ck: Value = serde_json::from_str(&data("checker_T162_rows.json")).unwrap();
    let rows = ck["rows"].as_array().unwrap();
    assert_eq!((n, fam.loops, fam.n_ops(), rows.len()), (20, 5, 30, 30));
    let mut entries = 0;
    for (op, r) in rows.iter().enumerate() {
        let ours = fam.exact_row(op).unwrap();
        let jv: Vec<u64> = r["op"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_u64().unwrap())
            .collect();
        assert_eq!(jv, vec![(op / 6) as u64, (op % 6) as u64], "operator order");
        let theirs: Vec<(Vec<i64>, Vec<Q>)> = r["row"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                let shift: Vec<i64> = e[0]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_i64().unwrap())
                    .collect();
                let mut form = vec![Q::ZERO; n + 2];
                for (var, c) in e[1].as_object().unwrap() {
                    let idx = match var.as_str() {
                        "d" => n + 1,
                        "c" => 0,
                        v => v.parse::<usize>().unwrap() + 1,
                    };
                    form[idx] = Q::parse(c.as_str().unwrap()).unwrap();
                }
                (shift, form)
            })
            .collect();
        assert_eq!(ours.len(), theirs.len(), "op {op}: number of shifts");
        for (shift, form) in &theirs {
            assert_eq!(ours.get(shift), Some(form), "op {op} shift {shift:?}");
            entries += 1;
        }
    }
    println!(
        "T162: 30 raw identities, {entries} (shift, linear form) entries equal to the checker's generator"
    );
}

#[test]
fn rows_t162_zero_sectors_equal_the_checkers() {
    let fam = t162_family();
    let ck: Value = serde_json::from_str(&data("checker_zero_sectors_T162P.json")).unwrap();
    let zero: std::collections::BTreeSet<String> = ck
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect();
    assert_eq!(zero.len(), 490);
    let mut count = 0;
    for m in 0u32..512 {
        let mut mask = vec![false; 20];
        let mut s = String::new();
        for (i, slot) in mask.iter_mut().enumerate().take(9) {
            *slot = m >> (8 - i) & 1 == 1;
            s.push(if *slot { '1' } else { '0' });
        }
        let z = fam.zero_mask(&mask);
        assert_eq!(z, zero.contains(&s), "line mask {s}");
        count += z as usize;
    }
    assert_eq!(count, 490);
    println!("T162: 490 of 512 line sectors scaleless, equal to the checker's list");
}

#[test]
fn rows_at_mod_equals_engine_rows_at_and_exact_rows() {
    let fam = t162_family();
    let n = fam.n;
    let [p, _] = verify_primes(11, &[]);
    let mut s = SplitMix64::new(5);
    for _ in 0..20 {
        let a: Vec<i64> = (0..n).map(|_| s.range(0, 13) as i64 - 6).collect();
        let d = s.range(2, p);
        let am: Vec<u64> = a.iter().map(|&x| arith::from_i128(p, x as i128)).collect();
        let r1 = fam.rows_at(&a, d, p).unwrap();
        let r2 = fam.rows_at_mod(&am, d, p).unwrap();
        assert_eq!(r1, r2);
        for (op, row) in r1.iter().enumerate() {
            let ex = fam.exact_row(op).unwrap();
            for (shift, form) in &ex {
                let mut v = form[0].modp(p).unwrap();
                for i in 0..n {
                    v = arith::add(p, v, arith::mul(p, form[i + 1].modp(p).unwrap(), am[i]));
                }
                v = arith::add(p, v, arith::mul(p, form[n + 1].modp(p).unwrap(), d));
                assert_eq!(row.get(shift).copied().unwrap_or(0), v);
            }
            assert!(row.keys().all(|k| ex.contains_key(k)));
        }
    }
}
