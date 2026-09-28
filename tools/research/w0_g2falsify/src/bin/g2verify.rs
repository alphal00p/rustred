//! W0 G2' falsifier: independent pointwise re-check of every G2' record of a
//! walk result.json (read-only; engine-independent lattice code of lib.rs).
//!
//! usage: g2verify <result.json> [--max-points N]
//!
//! For each record with a `g2_residual_anchor` block, every lattice point of
//! the original domain Q restricted to D >= cut (the part not inspected) must
//! lie in an anchor that the record names for that point: any listed anchor
//! of a mode-u union record; otherwise the first anchor for D >= first_cut
//! and the second anchor for cut <= D <= first_cut-1. Anchor sets are the raw
//! anchor records (box, A <= max_positive_power, R <= rank, D bounds; the
//! n = x+1 active / n = -x inactive convention). The enumerated point count
//! must equal the DP count of lib.rs `points`. Stamps, owners and kinds are
//! the Python audit's job (--g2-residual-anchors). Verdict PASS iff no
//! uncovered point, no count mismatch and every union record enumerated
//! (band records that cannot be enumerated are left to the audit's exact
//! single-anchor slice containment and reported).
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use w0_g2falsify::*;

struct Box_ {
    owner: Vec<bool>,
    lower: Vec<u64>,
    upper: Vec<Option<u64>>,
    rank: Option<u32>,
    pb: PB,
}

struct Job {
    id: u64,
    q: Box_,
    cut: i64,
    first_cut: i64,
    first: u64,
    second: Option<u64>,
    union: Option<Vec<u64>>,
}

fn contains_point(b: &Box_, x: &[u64]) -> bool {
    let mut a: i64 = 0;
    let mut r: i64 = 0;
    for (i, &v) in x.iter().enumerate() {
        if v < b.lower[i] || b.upper[i].is_some_and(|u| v > u) {
            return false;
        }
        if b.owner[i] {
            a += v as i64 + 1;
        } else {
            r += v as i64;
        }
    }
    let d = a - r;
    b.pb.max_positive_power.is_none_or(|m| a <= m as i64)
        && b.rank.is_none_or(|m| r <= m as i64)
        && b.pb.min_power_difference.is_none_or(|m| d >= m)
        && b.pb.max_power_difference.is_none_or(|m| d <= m)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = &args[1];
    let max_points: u128 = args
        .iter()
        .position(|a| a == "--max-points")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(5_000_000);
    let mut jobs: Vec<Job> = Vec::new();
    let mut needed: HashSet<u64> = HashSet::new();
    for_each_record(path, |r| {
        let Some(g2) = &r.g2_residual_anchor else {
            return;
        };
        let Some(id) = r.id_u64() else {
            return;
        };
        let link = r.initial_overlap.clone().unwrap_or(Value::Null);
        let cut = link.get("cut").and_then(Value::as_i64).unwrap_or(i64::MIN);
        let first = link.get("anchor_id").and_then(Value::as_u64).unwrap_or(u64::MAX);
        let first_cut = g2.get("first_cut").and_then(Value::as_i64).unwrap_or(cut);
        let second = g2
            .get("second_anchor")
            .and_then(|s| s.get("anchor_id"))
            .and_then(Value::as_u64);
        let union = g2.get("union_anchors").and_then(Value::as_array).map(|list| {
            list.iter()
                .filter_map(|a| a.get("anchor_id").and_then(Value::as_u64))
                .collect::<Vec<_>>()
        });
        needed.insert(first);
        needed.extend(second);
        if let Some(list) = &union {
            needed.extend(list.iter().copied());
        }
        jobs.push(Job {
            id,
            q: Box_ {
                owner: r.owner_bits(),
                lower: r.lower.clone(),
                upper: r.upper.clone(),
                rank: r.rank,
                pb: r.pb(),
            },
            cut,
            first_cut,
            first,
            second,
            union,
        });
    });
    let mut anchors: HashMap<u64, Box_> = HashMap::new();
    for_each_record(path, |r| {
        let id = r.id_u64().unwrap_or(u64::MAX);
        if needed.contains(&id) {
            anchors.insert(
                id,
                Box_ {
                    owner: r.owner_bits(),
                    lower: r.lower.clone(),
                    upper: r.upper.clone(),
                    rank: r.rank,
                    pb: r.pb(),
                },
            );
        }
    });
    let (mut checked, mut points_checked, mut uncovered, mut mismatched, mut unverifiable) =
        (0u64, 0u128, 0u64, 0u64, 0u64);
    let (mut union_records, mut band_records) = (0u64, 0u64);
    let mut band_unverifiable = 0u64;
    let mut failures: Vec<Value> = Vec::new();
    for job in &jobs {
        let n = job.q.owner.len();
        let mut high = job.q.pb;
        high.min_power_difference = Some(high.min_power_difference.map_or(job.cut, |m| m.max(job.cut)));
        let expected = points(&job.q.owner, &job.q.lower, &job.q.upper, job.q.rank, high);
        let Some(expected) = expected.filter(|&p| p <= max_points) else {
            if job.union.is_some() {
                unverifiable += 1;
            } else {
                band_unverifiable += 1;
            }
            continue;
        };
        let get = |id: u64| anchors.get(&id).filter(|a| a.owner == job.q.owner);
        let allowed_union: Option<Vec<&Box_>> = job.union.as_ref().map(|l| l.iter().filter_map(|&i| get(i)).collect());
        let first = get(job.first);
        let second = job.second.and_then(get);
        let mut count = 0u128;
        let mut bad = 0u64;
        let mut x = vec![0u64; n];
        let t = job.q.owner.iter().filter(|&&b| b).count() as i64;
        let ok = enumerate(&job.q.owner, &job.q.lower, &job.q.upper, job.q.rank, high, &mut |key| {
            count += 1;
            let mut r = 0i64;
            let mut a = t;
            for (i, xi) in x.iter_mut().enumerate() {
                *xi = ((key >> (6 * (n - 1 - i))) & 63) as u64;
                if job.q.owner[i] {
                    a += *xi as i64;
                } else {
                    r += *xi as i64;
                }
            }
            let d = a - r;
            let covered = match &allowed_union {
                Some(list) => list.iter().any(|b| contains_point(b, &x)),
                None if d >= job.first_cut => first.is_some_and(|b| contains_point(b, &x)),
                None => second.is_some_and(|b| contains_point(b, &x)),
            };
            if !covered {
                bad += 1;
            }
        });
        if !ok {
            if job.union.is_some() {
                unverifiable += 1;
            } else {
                band_unverifiable += 1;
            }
            continue;
        }
        checked += 1;
        if job.union.is_some() {
            union_records += 1;
        } else {
            band_records += 1;
        }
        points_checked += count;
        if count != expected {
            mismatched += 1;
        }
        if bad > 0 {
            uncovered += bad;
            if failures.len() < 20 {
                failures.push(json!({"id": job.id, "uncovered_points": bad, "points": count as u64}));
            }
        }
    }
    let verdict = if uncovered == 0 && mismatched == 0 && unverifiable == 0 { "PASS" } else { "FAIL" };
    let out = json!({
        "path": path,
        "g2_records": jobs.len(),
        "checked": checked,
        "union_records_checked": union_records,
        "band_records_checked": band_records,
        "points_checked": points_checked as f64,
        "uncovered_points": uncovered,
        "count_mismatches": mismatched,
        "unverifiable_union_records": unverifiable,
        "band_records_not_enumerable": band_unverifiable,
        "band_records_not_enumerable_scope": "infinite or >max-points or coordinate >= 63 band slices; their single-anchor slice containment is exact in the Python audit (--g2-residual-anchors)",
        "max_points_per_record": max_points as f64,
        "failures": failures,
        "verdict": verdict,
        "scope": "every lattice point of Q restricted to D >= cut lies in an anchor the record names for it; independent of the engine's summary code",
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}
