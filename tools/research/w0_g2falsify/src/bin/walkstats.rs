//! W0.9(a) run summary from a walk result.json (read-only).
//!
//! usage: walkstats <result.json> [--union]
//!
//! Prints one JSON object: per-phase native counts and record seconds, Apply
//! record seconds per owner, lattice points summed over native Apply
//! inspections, frontier records, and the number of admitted Apply domains
//! with the G1 widened shape (box [0,inf)^N, i.e. every lower 0 and every
//! upper null), split by record kind. With --union also the distinct Apply
//! points (finite domains only; skipped for domains above 5e7 points).
use serde_json::json;
use std::collections::{BTreeMap, HashMap, HashSet};
use w0_g2falsify::*;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = &args[1];
    let union = args.iter().any(|a| a == "--union");
    // --scale OWNER: per log10(points) decade of that owner's native Apply
    // records: count, mean points, mean and total record seconds.
    let scale_owner = args.iter().position(|a| a == "--scale").map(|i| args[i + 1].clone());
    let mut scale: BTreeMap<i32, (u64, f64, f64)> = BTreeMap::new();
    let mut native: BTreeMap<String, (u64, f64)> = BTreeMap::new();
    let mut kinds: BTreeMap<String, u64> = BTreeMap::new();
    let mut owner_secs: HashMap<String, (u64, f64)> = HashMap::new();
    let (mut apply_points, mut apply_points_inf) = (0f64, 0u64);
    let mut widened_shape: BTreeMap<String, u64> = BTreeMap::new();
    let mut frontier_records = 0u64;
    let mut frontier_items = 0u64;
    let mut error_records = 0u64;
    let mut not_closed = 0u64;
    let mut sets: HashMap<String, HashSet<u128>> = HashMap::new();
    let mut union_skipped = 0u64;
    let mut max_record_seconds = 0f64;
    let mut succ = 0f64;
    // Memory guard: stop the union beyond 2e8 distinct points (~6-8 GB).
    const UNION_CAP: usize = 200_000_000;
    let mut union_total = 0usize;
    let mut union_aborted = false;
    let n = for_each_record(path, |r| {
        let phase = r.phase_str().to_owned();
        *kinds.entry(format!("{}|{}", phase, r.record_kind)).or_default() += 1;
        if r.descendant_closed == Some(false) {
            not_closed += 1;
        }
        if let Some(f) = &r.frontiers {
            if !f.is_empty() {
                frontier_records += 1;
                frontier_items += f.len() as u64;
            }
        }
        if r.error.as_ref().is_some_and(|e| !e.is_null()) {
            error_records += 1;
        }
        if phase == "Apply" && r.lower.iter().all(|&x| x == 0) && r.upper.iter().all(Option::is_none) {
            *widened_shape.entry(r.record_kind.clone()).or_default() += 1;
        }
        if r.record_kind == "delegated_not_inspected" {
            return;
        }
        let s = r.seconds.unwrap_or(0.0);
        max_record_seconds = max_record_seconds.max(s);
        let e = native.entry(phase.clone()).or_default();
        e.0 += 1;
        e.1 += s;
        if phase == "Apply" {
            succ += r.stat("successors");
            let e = owner_secs.entry(r.owner.clone()).or_default();
            e.0 += 1;
            e.1 += s;
            let bits = r.owner_bits();
            match points(&bits, &r.lower, &r.upper, r.rank, r.pb()) {
                Some(p) => {
                    apply_points += p as f64;
                    if scale_owner.as_deref() == Some(r.owner.as_str()) && p > 0 {
                        let e = scale.entry((p as f64).log10().floor() as i32).or_default();
                        e.0 += 1;
                        e.1 += p as f64;
                        e.2 += s;
                    }
                    if union && union_total > UNION_CAP {
                        union_aborted = true;
                    } else if union {
                        if p > 50_000_000 {
                            union_skipped += 1;
                        } else {
                            let set = sets.entry(r.owner.clone()).or_default();
                            let before = set.len();
                            if !enumerate(&bits, &r.lower, &r.upper, r.rank, r.pb(), &mut |k| {
                                set.insert(k);
                            }) {
                                union_skipped += 1;
                            }
                            union_total += set.len() - before;
                        }
                    }
                }
                None => apply_points_inf += 1,
            }
        }
    });
    let mut owners: Vec<_> = owner_secs.into_iter().collect();
    owners.sort_by(|a, b| b.1 .1.partial_cmp(&a.1 .1).unwrap());
    let distinct: Option<u64> = (union && !union_aborted).then(|| sets.values().map(|s| s.len() as u64).sum());
    let out = json!({
        "path": path,
        "records": n,
        "record_kinds": kinds,
        "native_by_phase": native.iter().map(|(k, v)| (k.clone(), json!({"inspections": v.0, "record_seconds": v.1}))).collect::<BTreeMap<_, _>>(),
        "apply_successors": succ,
        "apply_points_summed": apply_points,
        "apply_points_infinite_domains": apply_points_inf,
        "apply_distinct_points": distinct,
        "apply_union_skipped_domains": union.then_some(union_skipped),
        "apply_union_aborted_at_cap": union.then_some(union_aborted),
        "apply_widened_shape_records": widened_shape,
        "frontier_records": frontier_records,
        "frontier_items": frontier_items,
        "error_records": error_records,
        "records_not_descendant_closed": not_closed,
        "max_record_seconds": max_record_seconds,
        "scale_owner": scale_owner,
        "scale": scale.iter().map(|(d, (n, p, sec))| json!({"log10_points": d, "n": n, "mean_points": p / *n as f64, "mean_seconds": sec / *n as f64, "total_seconds": sec, "us_per_point": 1e6 * sec / p})).collect::<Vec<_>>(),
        "apply_owner_seconds_top": owners.iter().take(20).map(|(o, (c, s))| json!({"owner": o, "inspections": c, "record_seconds": s})).collect::<Vec<_>>(),
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}
