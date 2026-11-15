//! AUDIT (audit-r2-planners), brief item 1 on REAL nodes: the union of the layers (D + r ascending, each layer walked
//! with every earlier layer's leaf set, the leaf sets built exactly as `closeloop/layers.rs::leafset_of` builds them)
//! equals ONE walk of the same start, row for row: key set, status, server (table / record or relation set), evidence
//! level, and "no blocker" (every layer) == "no blocker" (single walk).
//!
//! `#[ignore]`: `AUDIT_R2_LAYERS_REAL` = `;`-separated `<import spec path>|<max D+r>|<tier K|C|auto>|<label>`; reports to
//! `AUDIT_R2_LAYERS_OUT` (a directory). Run under a memory scope (release).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustred_campaign_core::closure::{KeySet, KeySetBuilder, LeafBinding, LeafSet, LeafSetBuilder, Tier};
use rustred_campaign_core::import::check::check_streamed_with_leaves;
use rustred_campaign_core::import::{CheckOptions, CheckResult, Evidence, ImportedSet, Level, TableSetSpec, load};
use serde_json::{Value, json};

fn coarse(lines: &[bool], k: &[i32]) -> i64 {
    let mut d = 0i64;
    let mut r = 0i64;
    for (i, &x) in k.iter().enumerate() {
        if lines[i] {
            if x > 1 {
                d += (x - 1) as i64;
            }
        } else {
            r += x.unsigned_abs() as i64;
        }
    }
    d + r
}

fn certified(v: &Value, required: Level) -> bool {
    match v["status"].as_str() {
        Some("terminal") => true,
        Some("served") => {
            serde_json::from_value::<Level>(v["evidence"].clone())
                .ok()
                .is_some_and(|e| e >= required)
                && v["facts"]["regularity_unknown"] != true
                && v["facts"]["coefficient_unknown"] != true
        }
        _ => false,
    }
}

/// key -> (status, server, evidence) of a rows file (in_start ignored).
fn rows_of(p: &Path) -> BTreeMap<Vec<i32>, String> {
    let mut m = BTreeMap::new();
    for l in std::fs::read_to_string(p).unwrap().lines() {
        if l.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(l).unwrap();
        let key: Vec<i32> = serde_json::from_value(v["key"].clone()).unwrap();
        let w = format!(
            "{}|{}|{}|{}",
            v["status"],
            v["server"],
            v["evidence"],
            v["open_status"]
        );
        assert!(m.insert(key, w).is_none(), "key twice in {}", p.display());
    }
    m
}

fn leafset(set: &ImportedSet, res: &CheckResult, rows: &Path, required: Level, verdict: &str) -> LeafSet {
    let mut b = LeafSetBuilder::new(set.codec, vec![], vec![]);
    for l in std::fs::read_to_string(rows).unwrap().lines() {
        if l.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(l).unwrap();
        let key: Vec<i32> = serde_json::from_value(v["key"].clone()).unwrap();
        let level = match v["status"].as_str() {
            Some("served") => v["evidence"].as_str().unwrap_or("none").to_string(),
            Some("terminal") => "terminal".into(),
            _ => "open".into(),
        };
        b.push(&key, "-", &level, certified(&v, required)).unwrap();
    }
    for im in &res.credit_images {
        b.aux(im).unwrap();
    }
    b.finish(LeafBinding {
        statement_digest: res.streamed.as_ref().map(|s| s.sha256.clone()).unwrap_or_default(),
        source_verdict: verdict.into(),
        node: "audit".into(),
        clause: "layer".into(),
        start_digest: "audit".into(),
        walk: res.leaf_walk.clone(),
        relative_to: vec![],
        provenance: BTreeMap::new(),
    })
    .unwrap()
}

fn no_blocker(res: &CheckResult) -> bool {
    res.blocking().is_empty() && res.summary.cycles == Some(0) && res.summary.descent_violations == 0
}

#[test]
#[ignore = "real nodes: AUDIT_R2_LAYERS_REAL, release, memory scope"]
fn layered_equals_single_on_real_nodes() {
    let cfg = std::env::var("AUDIT_R2_LAYERS_REAL").expect("AUDIT_R2_LAYERS_REAL");
    let out = PathBuf::from(std::env::var("AUDIT_R2_LAYERS_OUT").expect("AUDIT_R2_LAYERS_OUT"));
    std::fs::create_dir_all(&out).unwrap();
    let mut report = vec![];
    for item in cfg.split(';').filter(|s| !s.trim().is_empty()) {
        let f: Vec<&str> = item.split('|').collect();
        let (spec_path, max_dr, tier, label) = (f[0], f[1].parse::<i64>().unwrap(), f[2], f[3]);
        let t0 = std::time::Instant::now();
        let spec = TableSetSpec::from_json(&std::fs::read_to_string(spec_path).unwrap()).unwrap();
        let base = Path::new(spec_path).parent().unwrap();
        let set = load(&spec, base).unwrap();
        let ev = Evidence::from_spec(&set, base).unwrap();
        let load_s = t0.elapsed().as_secs_f64();
        let lines = set.frame.mask().to_vec();
        let opts = CheckOptions {
            threads: 4,
            tier: match tier {
                "K" => Some(Tier::K),
                "C" => Some(Tier::C),
                _ => None,
            },
            max_rounds: None,
        };
        let required = set.spec.required_evidence;
        // start = the import start keys with D + r <= max_dr; layers by D + r ascending
        let mut by: BTreeMap<i64, KeySetBuilder> = BTreeMap::new();
        let mut all = KeySetBuilder::new(set.codec);
        for k in set.start.iter() {
            let c = coarse(&lines, &k);
            if c <= max_dr {
                by.entry(c).or_insert_with(|| KeySetBuilder::new(set.codec)).push(&k).unwrap();
                all.push(&k).unwrap();
            }
        }
        let start: KeySet = all.finish();
        let dir = out.join(label);
        std::fs::create_dir_all(&dir).unwrap();
        let t1 = std::time::Instant::now();
        let single_rows = dir.join("single.rows.jsonl");
        let single = check_streamed_with_leaves(&set, &ev, &opts, &single_rows, &start, &[]).unwrap();
        let single_s = t1.elapsed().as_secs_f64();
        let t2 = std::time::Instant::now();
        let mut leaves: Vec<Arc<LeafSet>> = vec![];
        let mut composed: BTreeMap<Vec<i32>, String> = BTreeMap::new();
        let mut all_clean = true;
        let mut layers = vec![];
        for (c, b) in by {
            let ks = b.finish();
            let rp = dir.join(format!("layer{c}.rows.jsonl"));
            let res = check_streamed_with_leaves(&set, &ev, &opts, &rp, &ks, &leaves).unwrap();
            let clean = no_blocker(&res);
            all_clean &= clean;
            let rows = rows_of(&rp);
            for (k, w) in rows {
                assert!(composed.insert(k.clone(), w).is_none(), "{label}: key {k:?} walked in two layers");
            }
            layers.push(json!({"dr": c, "start": ks.len(), "own_keys": res.streamed.as_ref().map(|s| s.rows),
                "leaves_reached": res.leaves.as_ref().map(|h| h.summary.reached), "clean": clean,
                "blocking": res.blocking()}));
            leaves.push(Arc::new(leafset(&set, &res, &rp, required, if clean { "closed" } else { "open" })));
        }
        let layered_s = t2.elapsed().as_secs_f64();
        let single_map = rows_of(&single_rows);
        let only_single: Vec<&Vec<i32>> = single_map.keys().filter(|k| !composed.contains_key(*k)).take(5).collect();
        let only_layered: Vec<&Vec<i32>> = composed.keys().filter(|k| !single_map.contains_key(*k)).take(5).collect();
        let differ: Vec<&Vec<i32>> = single_map
            .iter()
            .filter(|(k, w)| composed.get(*k).is_some_and(|x| x != *w))
            .map(|(k, _)| k)
            .take(5)
            .collect();
        let rec = json!({"label": label, "spec": spec_path, "max_dr": max_dr, "tier": tier, "required": required,
            "start_keys": start.len(), "single_keys": single_map.len(), "layered_keys": composed.len(),
            "only_single": only_single, "only_layered": only_layered, "differing_rows": differ,
            "single_clean": no_blocker(&single), "single_blocking": single.blocking(), "layers_all_clean": all_clean,
            "layers": layers, "load_s": load_s, "single_s": single_s, "layered_s": layered_s});
        eprintln!("{rec}");
        std::fs::write(dir.join("report.json"), serde_json::to_vec_pretty(&rec).unwrap()).unwrap();
        report.push(rec);
        assert_eq!(single_map, composed, "{label}: layered rows != single rows");
        assert_eq!(no_blocker(&single), all_clean, "{label}: verdicts differ");
    }
    std::fs::write(out.join("summary.json"), serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}
