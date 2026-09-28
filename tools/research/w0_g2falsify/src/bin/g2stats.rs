//! W0 G2' falsifier run summary from a walk result.json (read-only).
//!
//! usage: g2stats <result.json> [--union]
//!
//! One JSON object: record kinds, per-phase native counts and record seconds,
//! and per Apply owner class (the hot owners 000011001001011 and
//! 011101110111000, then the rest): full native inspections, G2' residual
//! inspections (partial records with a `g2_residual_anchor` block), G2' full
//! covers (empty residual), initial-overlap partials, record seconds, and
//! lattice points: of the inspected sets (full domain or residual) and of the
//! original domains of planned records. With --union also the distinct
//! inspected Apply points per class (finite domains below 5e7 points only)
//! and new points per Apply inspection. `g2_anchor_reference_kinds` classifies
//! every anchor reference of a G2' record (union list, or first/second anchor)
//! by the referenced record's kind: full native inspection, G2' residual
//! (partial) record, G2' full cover, initial-overlap partial, other.
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use w0_g2falsify::*;

const CLASSES: [&str; 3] = ["000011001001011", "011101110111000", "other"];

#[derive(Default)]
struct Class {
    native: u64,
    native_seconds: f64,
    g2_partial: u64,
    g2_partial_seconds: f64,
    g2_full: u64,
    g2_full_seconds: f64,
    g2_second_anchor: u64,
    g2_second_anchor_full: u64,
    initial_partial: u64,
    initial_partial_seconds: f64,
    inspected_points: f64,
    planned_original_points: f64,
    planned_residual_points: f64,
    infinite: u64,
    successors: f64,
    events: f64,
    d_levels_original: u64,
    d_levels_residual: u64,
    union: HashSet<u128>,
    union_skipped: u64,
}

fn class_of(owner: &str) -> usize {
    CLASSES.iter().position(|c| *c == owner).unwrap_or(2)
}

fn pb_of(v: &Value) -> PB {
    serde_json::from_value(v.clone()).unwrap_or_default()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = &args[1];
    let union = args.iter().any(|a| a == "--union");
    let mut kinds: BTreeMap<String, u64> = BTreeMap::new();
    let mut native: BTreeMap<String, (u64, f64)> = BTreeMap::new();
    let mut classes: Vec<Class> = (0..3).map(|_| Class::default()).collect();
    // Per Apply owner: records, record seconds, native calls (G2' full covers excluded).
    let mut owner_secs: HashMap<String, (u64, f64, u64)> = HashMap::new();
    let (mut frontier_records, mut error_records, mut not_closed) = (0u64, 0u64, 0u64);
    let mut plan_micros = 0f64;
    let mut anchors_scanned = 0f64;
    const UNION_CAP: usize = 200_000_000;
    let mut union_total = 0usize;
    let mut union_aborted = false;
    // Anchor-kind census: kind per record id, anchor references per class.
    let mut kind_of: HashMap<u64, u8> = HashMap::new();
    let mut references: Vec<(usize, u64)> = Vec::new();
    let mut referencing_records: Vec<(usize, Vec<u64>)> = Vec::new();
    let n = for_each_record(path, |r| {
        if let Some(id) = r.id_u64() {
            let code = match (r.record_kind.as_str(), &r.g2_residual_anchor) {
                ("partial_initial_overlap_inspection", Some(g2)) => {
                    if g2.get("full_cover") == Some(&Value::Bool(true)) { 2 } else { 1 }
                }
                ("partial_initial_overlap_inspection", None) => 3,
                (k, _) if k.starts_with("native") => 0,
                _ => 4,
            };
            kind_of.insert(id, code);
            if let Some(g2) = &r.g2_residual_anchor {
                let class = class_of(&r.owner);
                let mut ids: Vec<u64> = match g2.get("union_anchors").and_then(Value::as_array) {
                    Some(list) => list.iter().filter_map(|a| a.get("anchor_id").and_then(Value::as_u64)).collect(),
                    None => r
                        .initial_overlap
                        .as_ref()
                        .and_then(|o| o.get("anchor_id"))
                        .and_then(Value::as_u64)
                        .into_iter()
                        .chain(g2.get("second_anchor").and_then(|s| s.get("anchor_id")).and_then(Value::as_u64))
                        .collect(),
                };
                ids.dedup();
                references.extend(ids.iter().map(|&a| (class, a)));
                referencing_records.push((class, ids));
            }
        }
        let phase = r.phase_str().to_owned();
        *kinds.entry(format!("{}|{}", phase, r.record_kind)).or_default() += 1;
        if r.descendant_closed == Some(false) {
            not_closed += 1;
        }
        if r.frontiers.as_ref().is_some_and(|f| !f.is_empty()) {
            frontier_records += 1;
        }
        if r.error.as_ref().is_some_and(|e| !e.is_null()) {
            error_records += 1;
        }
        if r.record_kind == "delegated_not_inspected" {
            return;
        }
        let s = r.seconds.unwrap_or(0.0);
        let e = native.entry(phase.clone()).or_default();
        e.0 += 1;
        e.1 += s;
        if phase != "Apply" {
            return;
        }
        let full_cover = r
            .g2_residual_anchor
            .as_ref()
            .is_some_and(|g2| g2.get("full_cover") == Some(&Value::Bool(true)));
        let o = owner_secs.entry(r.owner.clone()).or_default();
        o.0 += 1;
        o.1 += s;
        o.2 += u64::from(!full_cover);
        let c = &mut classes[class_of(&r.owner)];
        c.successors += r.stat("successors");
        c.events += r.stat("events");
        let bits = r.owner_bits();
        let original = points(&bits, &r.lower, &r.upper, r.rank, r.pb());
        // The inspected set: the whole domain, or the residual of a partial.
        let mut inspected_pb = Some(r.pb());
        if r.record_kind == "partial_initial_overlap_inspection" {
            let residual = r
                .initial_overlap
                .as_ref()
                .and_then(|o| o.get("residual_power_bounds"))
                .map(pb_of)
                .unwrap_or_default();
            match &r.g2_residual_anchor {
                Some(g2) => {
                    plan_micros += g2.get("plan_micros").and_then(Value::as_f64).unwrap_or(0.0);
                    anchors_scanned += g2.get("anchors_scanned").and_then(Value::as_f64).unwrap_or(0.0);
                    c.d_levels_original += g2.get("original_d_levels").and_then(Value::as_u64).unwrap_or(0);
                    c.d_levels_residual += g2.get("residual_d_levels").and_then(Value::as_u64).unwrap_or(0);
                    let second = g2.get("second_anchor").is_some_and(|v| !v.is_null());
                    c.g2_second_anchor += u64::from(second);
                    if g2.get("full_cover") == Some(&Value::Bool(true)) {
                        c.g2_second_anchor_full += u64::from(second);
                        c.g2_full += 1;
                        c.g2_full_seconds += s;
                        inspected_pb = None;
                    } else {
                        c.g2_partial += 1;
                        c.g2_partial_seconds += s;
                        inspected_pb = Some(residual);
                    }
                    if let Some(p) = original {
                        c.planned_original_points += p as f64;
                    }
                    if let Some(pb) = inspected_pb {
                        if let Some(p) = points(&bits, &r.lower, &r.upper, r.rank, pb) {
                            c.planned_residual_points += p as f64;
                        }
                    }
                }
                None => {
                    c.initial_partial += 1;
                    c.initial_partial_seconds += s;
                    inspected_pb = Some(residual);
                }
            }
        } else {
            c.native += 1;
            c.native_seconds += s;
        }
        let Some(pb) = inspected_pb else {
            return;
        };
        match points(&bits, &r.lower, &r.upper, r.rank, pb) {
            Some(p) => {
                c.inspected_points += p as f64;
                if union && union_total > UNION_CAP {
                    union_aborted = true;
                } else if union {
                    if p > 50_000_000 {
                        c.union_skipped += 1;
                    } else {
                        let before = c.union.len();
                        if !enumerate(&bits, &r.lower, &r.upper, r.rank, pb, &mut |k| {
                            // Owner classes pool several owners: key by owner too.
                            c.union.insert(k ^ hash_owner(&r.owner));
                        }) {
                            c.union_skipped += 1;
                        }
                        union_total += c.union.len() - before;
                    }
                }
            }
            None => c.infinite += 1,
        }
    });
    let class_json: BTreeMap<String, Value> = classes
        .iter()
        .zip(CLASSES)
        .map(|(c, name)| {
            let apply = c.native + c.g2_partial + c.g2_full + c.initial_partial;
            let seconds =
                c.native_seconds + c.g2_partial_seconds + c.g2_full_seconds + c.initial_partial_seconds;
            let distinct = (union && !union_aborted).then_some(c.union.len() as u64);
            (
                name.to_owned(),
                json!({
                    "apply_records_inspected_or_planned": apply,
                    "apply_record_seconds": seconds,
                    "full_native": c.native, "full_native_seconds": c.native_seconds,
                    "g2_residual": c.g2_partial, "g2_residual_seconds": c.g2_partial_seconds,
                    "g2_full_cover": c.g2_full, "g2_full_cover_seconds": c.g2_full_seconds,
                    "g2_with_second_anchor": c.g2_second_anchor,
                    "g2_full_cover_with_second_anchor": c.g2_second_anchor_full,
                    "initial_overlap_partial": c.initial_partial,
                    "initial_overlap_partial_seconds": c.initial_partial_seconds,
                    "share_g2_residual": ratio(c.g2_partial as f64, apply as f64),
                    "share_g2_full_cover": ratio(c.g2_full as f64, apply as f64),
                    "native_calls": c.native + c.g2_partial + c.initial_partial,
                    "inspected_points": c.inspected_points,
                    "planned_original_points": c.planned_original_points,
                    "planned_residual_points": c.planned_residual_points,
                    "planned_residual_point_fraction": ratio(c.planned_residual_points, c.planned_original_points),
                    "planned_d_levels_original": c.d_levels_original,
                    "planned_d_levels_residual": c.d_levels_residual,
                    "infinite_domains": c.infinite,
                    "successors": c.successors, "events": c.events,
                    "distinct_inspected_points": distinct,
                    "distinct_points_per_native_call": distinct.map(|d| ratio(d as f64, (c.native + c.g2_partial + c.initial_partial) as f64)),
                    "union_skipped_domains": union.then_some(c.union_skipped),
                }),
            )
        })
        .collect();
    const KIND_NAMES: [&str; 6] =
        ["native_inspection", "g2_residual_partial", "g2_full_cover", "initial_overlap_partial", "other", "missing"];
    let mut ref_counts = [[0u64; 6]; 3];
    for &(class, anchor) in &references {
        let code = kind_of.get(&anchor).map_or(5, |&k| k as usize);
        ref_counts[class][code] += 1;
    }
    let mut records_non_native = [0u64; 3];
    let mut records_total = [0u64; 3];
    for (class, ids) in &referencing_records {
        records_total[*class] += 1;
        records_non_native[*class] +=
            u64::from(ids.iter().any(|a| kind_of.get(a).is_none_or(|&k| k != 0)));
    }
    let kinds_json = |counts: &[u64; 6]| {
        let total: u64 = counts.iter().sum();
        let mut m: BTreeMap<String, Value> = KIND_NAMES
            .iter()
            .zip(counts)
            .map(|(k, v)| ((*k).to_owned(), json!(v)))
            .collect();
        m.insert("total".into(), json!(total));
        m.insert("non_native_share".into(), json!(ratio((total - counts[0]) as f64, total as f64)));
        m
    };
    let mut all = [0u64; 6];
    for counts in &ref_counts {
        for (a, b) in all.iter_mut().zip(counts) {
            *a += b;
        }
    }
    let anchor_kinds = json!({
        "all": kinds_json(&all),
        "by_owner_class": CLASSES.iter().enumerate().map(|(i, name)| (name.to_owned(), json!({
            "references": kinds_json(&ref_counts[i]),
            "g2_records": records_total[i],
            "g2_records_with_a_non_native_anchor": records_non_native[i],
        }))).collect::<BTreeMap<_, _>>(),
        "g2_records": records_total.iter().sum::<u64>(),
        "g2_records_with_a_non_native_anchor": records_non_native.iter().sum::<u64>(),
    });
    let apply_by_owner: BTreeMap<String, Value> = owner_secs
        .iter()
        .map(|(o, (c, s, calls))| (o.clone(), json!({"records": c, "record_seconds": s, "native_calls": calls})))
        .collect();
    let mut owners: Vec<_> = owner_secs.into_iter().collect();
    owners.sort_by(|a, b| b.1 .1.partial_cmp(&a.1 .1).unwrap());
    let out = json!({
        "path": path,
        "records": n,
        "record_kinds": kinds,
        "native_by_phase": native.iter().map(|(k, v)| (k.clone(), json!({"records": v.0, "record_seconds": v.1}))).collect::<BTreeMap<_, _>>(),
        "apply_by_owner_class": class_json,
        "g2_plan_seconds_in_records": plan_micros * 1e-6,
        "g2_anchors_scanned_in_records": anchors_scanned,
        "g2_anchor_reference_kinds": anchor_kinds,
        "frontier_records": frontier_records,
        "error_records": error_records,
        "records_not_descendant_closed": not_closed,
        "union_aborted_at_cap": union.then_some(union_aborted),
        "apply_owner_seconds_top": owners.iter().take(12).map(|(o, (c, s, _))| json!({"owner": o, "records": c, "record_seconds": s})).collect::<Vec<_>>(),
        "apply_by_owner": apply_by_owner,
        "notes": "record seconds include the G2' anchor search of planned and unplanned jobs in the flag-on arm; points are exact lattice counts (tools/research/w0_g2falsify/src/lib.rs)",
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}

fn ratio(a: f64, b: f64) -> f64 {
    if b == 0.0 { 0.0 } else { a / b }
}

fn hash_owner(owner: &str) -> u128 {
    // FNV-1a over the mask, placed above the 6-bit-per-axis packed key
    // (at most 15 axes = 90 bits).
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in owner.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    ((h & 0x3f_ffff) as u128) << 96
}
