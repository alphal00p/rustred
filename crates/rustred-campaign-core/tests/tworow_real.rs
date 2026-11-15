//! Lane tworow on REAL nodes (ignored; every path by environment, nothing node-specific in the code):
//!
//! - frame: `TWOROW_HEADER_TABLE` = a table_dump.v1 file whose first line is the node's header (mask, node order,
//!   family_source; keys in its presented frame), OR `TWOROW_FAMILY` (family text in the frame of the keys) +
//!   `TWOROW_ORDER` (node order JSON, as a header's `node_order`) + `TWOROW_CHART` (0/1 mask);
//! - `TWOROW_KEYS`: one key per line, a JSON array or `I(..)` text, optional TAB + label (`#` lines skipped);
//! - `TWOROW_POLICY`: optional policy JSON (spec object `tworow`, interfaces/tworow.md 2);
//! - `TWOROW_THREADS` (default 1, at most 4), `TWOROW_OUT` (JSONL: one line per key + a summary line), `TWOROW_REGION=1`
//!   adds the region forms per face (keys grouped by face signature).

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::Path;
use std::time::Instant;

use rustred_campaign_core::import::symmetry::{GroupSpec, RelationCheck, load_verified_group};
use rustred_campaign_core::independent_algebra::Independent;
use rustred_campaign_core::order::{NodeComparator, NodeOrderRecord};
use rustred_campaign_core::replay_base::family::Family;
use rustred_campaign_core::replay_base::symrows::VerifiedGroup;
use rustred_campaign_core::tworow::{
    KeyResult, TwoRowPolicy, import_check, region_forms, search_key,
};
use rustred_campaign_core::worklist::items::Face;
use serde_json::{Value, json};

fn parse_key(t: &str) -> Option<Vec<i32>> {
    let t = t.trim();
    if t.starts_with('[') {
        return serde_json::from_str(t).ok();
    }
    let inner = t.strip_prefix("I(")?.strip_suffix(')')?;
    inner.split(',').map(|x| x.trim().parse().ok()).collect()
}

fn frame() -> (VerifiedGroup, NodeComparator, Vec<bool>, Value, Family) {
    let base = Path::new("/");
    if let Ok(tp) = std::env::var("TWOROW_HEADER_TABLE") {
        let f = std::fs::File::open(&tp).unwrap();
        let mut l = String::new();
        std::io::BufReader::new(f).read_line(&mut l).unwrap();
        let h: Value = serde_json::from_str(&l).unwrap();
        let cmp = NodeComparator::from_header(&h).unwrap();
        let chart: Vec<bool> = h["mask"]
            .as_str()
            .unwrap()
            .bytes()
            .map(|b| b == b'1')
            .collect();
        let gs: GroupSpec = serde_json::from_value(json!({})).unwrap();
        let (g, rep) = load_verified_group(
            &gs,
            base,
            &chart,
            cmp.presentation().new_to_old(),
            h["family_source"].as_str(),
        )
        .unwrap();
        let info = json!({"frame": tp, "order": h["node_order"], "group_digest": g.digest(),
            "elements": g.group().elements.len(), "group_report": format!("{rep:?}")});
        let fam = Family::parse(h["family_source"].as_str().unwrap()).unwrap();
        (g, cmp, chart, info, fam)
    } else {
        let fam = std::env::var("TWOROW_FAMILY").expect("TWOROW_HEADER_TABLE or TWOROW_FAMILY");
        let ord: NodeOrderRecord = serde_json::from_str(
            &std::fs::read_to_string(std::env::var("TWOROW_ORDER").unwrap()).unwrap(),
        )
        .unwrap();
        let chart: Vec<bool> = std::env::var("TWOROW_CHART")
            .unwrap()
            .bytes()
            .map(|b| b == b'1')
            .collect();
        let n = chart.len();
        let cmp = NodeComparator::from_node_order(&ord, n).unwrap();
        let gs: GroupSpec = serde_json::from_value(json!({"family": {"path": fam}})).unwrap();
        let identity: Vec<usize> = (0..n).collect();
        let (g, rep) = load_verified_group(&gs, base, &chart, &identity, None).unwrap();
        let info = json!({"family": fam, "order": ord, "group_digest": g.digest(),
            "elements": g.group().elements.len(), "group_report": format!("{rep:?}")});
        let fam = Family::parse(&std::fs::read_to_string(&fam).unwrap()).unwrap();
        (g, cmp, chart, info, fam)
    }
}

#[test]
#[ignore]
fn tworow_on_real_keys() {
    let (g, cmpr, chart, info, fam) = frame();
    let cmp = |a: &[i32], b: &[i32]| cmpr.cmp(a, b);
    // TWOROW_ZERO=family: zero-sector terms pruned by the family's own Lee criterion (a downset)
    let zf = |k: &[i32]| fam.zero_key(&k.iter().map(|&x| x as i64).collect::<Vec<_>>());
    let zero: Option<&(dyn Fn(&[i32]) -> bool + Sync)> =
        (std::env::var("TWOROW_ZERO").as_deref() == Ok("family")).then_some(&zf);
    let policy = match std::env::var("TWOROW_POLICY") {
        Ok(p) => TwoRowPolicy::from_spec(
            &serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap(),
        )
        .unwrap(),
        Err(_) => TwoRowPolicy::default(),
    };
    let mut keys: Vec<(Vec<i32>, String)> = vec![];
    for l in std::fs::read_to_string(std::env::var("TWOROW_KEYS").unwrap())
        .unwrap()
        .lines()
    {
        if l.trim().is_empty() || l.starts_with('#') {
            continue;
        }
        let (k, lab) = l.split_once('\t').unwrap_or((l, ""));
        keys.push((parse_key(k).unwrap(), lab.to_string()));
    }
    let threads: usize = std::env::var("TWOROW_THREADS")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(1)
        .clamp(1, 4);
    let t0 = Instant::now();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: std::sync::Mutex<Vec<Option<(KeyResult, f64, Value)>>> =
        std::sync::Mutex::new(vec![None; keys.len()]);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if i >= keys.len() {
                        break;
                    }
                    let t = Instant::now();
                    let r = search_key(
                        &Independent,
                        &keys[i].0,
                        &g,
                        &cmp,
                        &policy,
                        zero.map(|z| z as &dyn Fn(&[i32]) -> bool),
                    );
                    let secs = t.elapsed().as_secs_f64();
                    // route (A) on the best relation
                    let a = match r.outcome.best() {
                        Some(_) if zero.is_some() => {
                            json!({"skipped": "zero-sector terms dropped: not in the row span"})
                        }
                        Some(c) => {
                            match import_check(&Independent, &g, c, policy.max_terms_per_row) {
                                RelationCheck::Verified { rows, rank, .. } => {
                                    json!({"verified": true, "rows": rows, "rank": rank})
                                }
                                x => json!({"verified": false, "check": format!("{x:?}")}),
                            }
                        }
                        None => Value::Null,
                    };
                    results.lock().unwrap()[i] = Some((r, secs, a));
                }
            });
        }
    });
    let results: Vec<(KeyResult, f64, Value)> = results
        .into_inner()
        .unwrap()
        .into_iter()
        .map(|x| x.unwrap())
        .collect();
    let mut out: Box<dyn Write> = match std::env::var("TWOROW_OUT") {
        Ok(p) => Box::new(std::io::BufWriter::new(std::fs::File::create(p).unwrap())),
        Err(_) => Box::new(std::io::sink()),
    };
    let mut by_outcome: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_label: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut sizes: Vec<usize> = vec![];
    let mut rows3 = 0usize;
    let mut a_ok = 0usize;
    for ((r, secs, a), (_, lab)) in results.iter().zip(&keys) {
        *by_outcome.entry(r.outcome.word().into()).or_default() += 1;
        *by_label
            .entry(lab.clone())
            .or_default()
            .entry(r.outcome.word().into())
            .or_default() += 1;
        if let Some(b) = r.outcome.best() {
            *by_kind.entry(b.kind()).or_default() += 1;
            sizes.push(b.rhs.len());
            if b.rows.len() >= 3 {
                rows3 += 1;
            }
            assert!(b.rhs.iter().all(|(t, _)| cmp(t, &r.key) == Ordering::Less));
        }
        if a["verified"] == json!(true) {
            a_ok += 1;
        }
        let mut j = r.to_json();
        j["label"] = json!(lab);
        j["seconds"] = json!(secs);
        j["import_check"] = a.clone();
        writeln!(out, "{j}").unwrap();
    }
    sizes.sort_unstable();
    let line_perms: Vec<Value> = g
        .group()
        .elements
        .iter()
        .map(|e| json!(e.line_perm(&chart)))
        .collect();
    let mut summary = json!({"summary": true, "info": info, "line_perms": line_perms, "policy": serde_json::to_value(&policy).unwrap(),
        "keys": keys.len(), "outcomes": by_outcome, "best_kind": by_kind, "by_label": by_label,
        "best_rhs_terms": {"min": sizes.first(), "median": sizes.get(sizes.len() / 2), "max": sizes.last()},
        "best_with_3_or_more_rows": rows3, "zero_sectors": if zero.is_some() { "family (Lee), pruned" } else { "kept" }, "import_check_verified": a_ok,
        "seconds": t0.elapsed().as_secs_f64(), "threads": threads});
    if std::env::var("TWOROW_REGION").as_deref() == Ok("1") {
        let mut faces: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, (k, _)) in keys.iter().enumerate() {
            let f = Face::of_key(k, &chart)
                .map(|f| f.label())
                .unwrap_or_default();
            faces.entry(f).or_default().push(i);
        }
        let mut regs = serde_json::Map::new();
        for (f, idx) in faces {
            let rk: Vec<Vec<i32>> = idx.iter().map(|&i| keys[i].0.clone()).collect();
            let rr: Vec<KeyResult> = idx.iter().map(|&i| results[i].0.clone()).collect();
            let rep = region_forms(
                &Independent,
                &g,
                &rk,
                &rr,
                &cmp,
                &policy,
                zero.map(|z| z as &dyn Fn(&[i32]) -> bool),
            );
            regs.insert(f, rep.to_json());
        }
        summary["regions"] = Value::Object(regs);
    }
    writeln!(out, "{summary}").unwrap();
    println!("{}", serde_json::to_string_pretty(&summary).unwrap());
}
