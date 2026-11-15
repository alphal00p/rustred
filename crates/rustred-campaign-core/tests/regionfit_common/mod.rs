//! Shared helpers of the regionfit tests: an INDEPENDENT evaluator of the record's structured conditions (it does not
//! call the library's atom code) and the full soundness / partition check of a record against its face.
#![allow(dead_code)]

use rustred_campaign_core::regionfit::{FaceInput, FaceKey};
use serde_json::Value;
use std::collections::BTreeSet;

pub fn fk(key: &[i32], acc: &[&str]) -> FaceKey {
    FaceKey {
        key: key.to_vec(),
        acceptable: acc.iter().map(|s| s.to_string()).collect(),
    }
}

pub fn face(sig: &str, keys: Vec<FaceKey>) -> FaceInput {
    FaceInput {
        signature: sig.into(),
        keys,
    }
}

/// Independent evaluation of one structured atom of the record.
pub fn atom_holds(a: &Value, k: &[i32]) -> bool {
    let g = |f: &str| a[f].as_i64().unwrap();
    let n = |f: &str| i64::from(k[a[f].as_u64().unwrap() as usize]);
    match a["kind"].as_str().unwrap() {
        "le_rel" => n("i") <= n("j"),
        "lt_rel" => n("i") < n("j"),
        "eq_rel" => n("i") == n("j"),
        "eq_const" => n("i") == g("c"),
        "le_const" => n("i") <= g("c"),
        "ge_const" => n("i") >= g("c"),
        "parity" => (n("i").rem_euclid(2) == 1) == a["odd"].as_bool().unwrap(),
        "sum_le" => n("i") + n("j") <= g("c"),
        other => panic!("unknown atom kind {other}"),
    }
}

pub fn cond_holds(region: &Value, k: &[i32]) -> bool {
    region["condition"]["atoms"]
        .as_array()
        .unwrap()
        .iter()
        .all(|a| atom_holds(a, k))
}

/// Soundness (every key satisfying a region's condition accepts its choice), the partition (dispatch attribution +
/// residual + unservable = every key exactly once) and the counts of the record.
pub fn check_record(face: &FaceInput, rec: &Value) {
    let regions = rec["regions"].as_array().unwrap();
    let residual: BTreeSet<Vec<i32>> = rec["residual"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| serde_json::from_value(r["key"].clone()).unwrap())
        .collect();
    let unservable: BTreeSet<Vec<i32>> = rec["unservable"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| serde_json::from_value(r.clone()).unwrap())
        .collect();
    let mut assigned = vec![0usize; regions.len()];
    let mut verified = vec![0usize; regions.len()];
    for k in &face.keys {
        let mut first = None;
        for (r, reg) in regions.iter().enumerate() {
            if cond_holds(reg, &k.key) {
                let c = reg["choice"].as_str().unwrap();
                assert!(
                    k.acceptable.contains(c),
                    "UNSOUND: region {} / {c} holds at {:?} where {:?}",
                    reg["condition"]["text"],
                    k.key,
                    k.acceptable
                );
                verified[r] += 1;
                first.get_or_insert(r);
            }
        }
        match first {
            Some(r) => {
                assigned[r] += 1;
                assert!(!residual.contains(&k.key) && !unservable.contains(&k.key));
            }
            None => {
                if k.acceptable.is_empty() {
                    assert!(
                        unservable.contains(&k.key),
                        "{:?} not listed unservable",
                        k.key
                    );
                } else {
                    assert!(
                        residual.contains(&k.key),
                        "{:?} neither in a region nor residual",
                        k.key
                    );
                    let rc = rec["residual"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|r| {
                            serde_json::from_value::<Vec<i32>>(r["key"].clone()).unwrap() == k.key
                        })
                        .unwrap();
                    assert!(k.acceptable.contains(rc["choice"].as_str().unwrap()));
                }
            }
        }
    }
    for (r, reg) in regions.iter().enumerate() {
        assert_eq!(
            reg["keys"].as_u64().unwrap() as usize,
            assigned[r],
            "assigned count"
        );
        assert_eq!(
            reg["verified"].as_u64().unwrap() as usize,
            verified[r],
            "verified count"
        );
        assert!(assigned[r] > 0, "a region with no key");
    }
    assert_eq!(
        assigned.iter().sum::<usize>() + residual.len() + unservable.len(),
        face.keys.len()
    );
    assert_eq!(rec["keys"].as_u64().unwrap() as usize, face.keys.len());
}

/// Region texts with their choices.
pub fn summary(rec: &Value) -> Vec<(String, String)> {
    rec["regions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["condition"]["text"].as_str().unwrap().to_string(),
                r["choice"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}
