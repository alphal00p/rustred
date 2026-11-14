//! Lane import, BIG DATA (ignored): reproduction of the T178 hand statement statement-20261114T040105Z (boxed close
//! with recipes, confirmed key by key by the independent checker) from the EXISTING material through the import:
//! 37 order-B piece tables (generation order, the 16 elided records removed as the statement did), the 110 lane rules
//! in NUMERIC FACE ID order (the coordinator's addendum), the 4 supplied symmetry relations (with their verification
//! record), the 3 terminals. Closure under the STRICT core dispatcher, Tier K (the hand statement follows every top
//! rhs key; T178's facts carry no record_sha256 and cannot be bound), both elided policies; comparison typed.
//!
//! Paths (test data only): `RUSTRED_IMPORT_WS` (default the p_integral_dev workspace), scratch output
//! `RUSTRED_IMPORT_SCRATCH` (default /common/build-cache/$USER/p_integral_dev/implementer-scratch/import).
//! Run: `light_build.sh import test -p rustred-campaign-core --release --test import_t178 -- --ignored --nocapture`.

// ---- shim until I-INT adds `pub mod import;` to lib.rs (interfaces/import.md s.10)
#[allow(unused_imports)]
use rustred_campaign_core::{closure, cover, domain, key, order, poly, record, schema, symmetry};
#[allow(dead_code, unused_imports)]
#[path = "../src/import/mod.rs"]
mod import;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use import::{CheckOptions, Evidence, HandFormat, TableSetSpec, check, compare, load, read_hand_statement, write_statement};
use serde_json::{Value, json};

fn ws() -> PathBuf {
    std::env::var_os("RUSTRED_IMPORT_WS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/shared/vhirschi/t3code/vakint_dev/p_integral_dev"))
}

fn scratch() -> PathBuf {
    std::env::var_os("RUSTRED_IMPORT_SCRATCH").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(format!(
            "/common/build-cache/{}/p_integral_dev/implementer-scratch/import",
            std::env::var("USER").unwrap_or_else(|_| "vhirschi".into())
        ))
    })
}

pub fn vm_hwm_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1).and_then(|x| x.parse().ok()))
        })
        .unwrap_or(0)
}

fn piece_path(regen: &Path, piece: &str) -> PathBuf {
    let (dir, sub) = piece.split_once('_').expect("piece name dir_sub");
    regen.join(dir).join(sub).join("node/F.table.jsonl")
}

/// Leading number of a face id ("F12" -> 12, "F5x" -> 5); ids without one sort after every numbered one.
fn face_num(face: &str) -> u64 {
    let d: String = face.trim_start_matches('F').chars().take_while(|c| c.is_ascii_digit()).collect();
    d.parse().unwrap_or(u64::MAX)
}

fn hand_format() -> HandFormat {
    HandFormat { key: 0, in_box: 1, label: 2, rule: 3, round: Some(4), round_index: None, terminal_label: "terminal".into() }
}

fn verify_sums(dir: &Path) {
    for l in std::fs::read_to_string(dir.join("SHA256SUMS")).unwrap().lines() {
        let (h, n) = l.split_once("  ").unwrap();
        assert_eq!(import::lines::file_sha256(&dir.join(n)).unwrap(), h, "{n} differs from its SHA256SUMS");
    }
}

fn build_spec(ws: &Path, stmt: &Path) -> Value {
    let summary: Value = serde_json::from_str(&std::fs::read_to_string(stmt.join("summary.json")).unwrap()).unwrap();
    let rules: BTreeMap<String, Value> =
        serde_json::from_str(&std::fs::read_to_string(stmt.join("rules.json")).unwrap()).unwrap();
    let regen = ws.join("SCRATCHPAD/t178/regenB");
    // removed: piece#seq (record order) -> label of that record
    let mut removed: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for s in summary["skipped_elided_records"].as_array().unwrap() {
        let (piece, seq) = s.as_str().unwrap().split_once('#').unwrap();
        let seq: usize = seq.parse().unwrap();
        let raw = import::lines::scan_table(piece, &piece_path(&regen, piece), false, &|o, _| o == seq).unwrap();
        let r = &raw.records[0];
        assert_eq!(r.ordinal, seq);
        removed.entry(piece.to_string()).or_default().push(json!({"label": r.rec.index.unwrap(),
            "reason": "elided witness loci: removed from the dispatch by the hand statement (skip_elided)"}));
    }
    let mut sources = vec![];
    for n in summary["dispatch_order"].as_array().unwrap() {
        let n = n.as_str().unwrap();
        if n == "new" {
            continue;
        }
        let p = piece_path(&regen, n);
        assert!(p.is_file(), "{}", p.display());
        sources.push(json!({"id": n, "kind":"engine_table", "class":"orderB", "path": p.display().to_string(),
            "removed": removed.remove(n).unwrap_or_default(), "hand_name":{"template":"{source}#{ordinal}"}}));
    }
    assert!(removed.is_empty(), "every skipped record belongs to a dispatched piece");
    // the 110 lane rules in numeric face id order
    let mut new: Vec<(u64, String, &Value)> = rules
        .iter()
        .filter(|(k, _)| k.starts_with("faces"))
        .map(|(k, v)| (face_num(k.split('/').nth(1).unwrap()), k.clone(), v))
        .collect();
    new.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    assert_eq!(new.len(), 110);
    for (_, name, v) in &new {
        sources.push(json!({"id": name, "kind":"engine_table", "class":"lane", "path": v["path"],
            "records":[v["index"]], "hand_name":{"template":"{source}"}}));
    }
    let sym = rules["symmetry#0"]["path"].as_str().unwrap().to_string();
    sources.push(json!({"id":"symmetry","kind":"supplied_relations","class":"symmetry","path": sym,
        "verification":{"path": ws.join("SCRATCHPAD/t178-close/data/symrules.verify.json").display().to_string()},
        "hand_name":{"template":"symmetry#{index}"}}));
    sources.push(json!({"id":"terminals","kind":"terminals","path": stmt.join("terminals.json").display().to_string()}));
    json!({"schema":"rustred.campaign.import-spec.v1","node":"T178","frame_from":"R0_try1","bits":6,
        "start":{"path": stmt.join("statement.jsonl").display().to_string(),"format":"hand_statement",
                 "hand_format": serde_json::to_value(hand_format()).unwrap()},
        "required_evidence":"per-key","policies":{"tier":"K"},
        "sources": sources})
}

#[test]
#[ignore = "big data: the T178 hand statement and its material; run with --ignored in release"]
fn t178_reproduction_of_the_hand_statement() {
    let ws = ws();
    let stmt = ws.join("SCRATCHPAD/t178-close/statements/statement-20261114T040105Z");
    if !stmt.join("statement.jsonl").is_file() {
        println!("SKIP: {} missing", stmt.display());
        return;
    }
    verify_sums(&stmt);
    let out = scratch().join("t178");
    std::fs::create_dir_all(&out).unwrap();
    let spec_v = build_spec(&ws, &stmt);
    std::fs::write(out.join("spec.T178.json"), serde_json::to_string_pretty(&spec_v).unwrap()).unwrap();
    let hand = read_hand_statement(&stmt.join("statement.jsonl"), &hand_format(), 20).unwrap();
    assert_eq!(hand.rows.len(), 335_306);
    for policy in ["block", "lift_with_per_key_proof"] {
        let t0 = std::time::Instant::now();
        let mut sv = spec_v.clone();
        sv["policies"]["elided"] = json!(policy);
        let spec = TableSetSpec::from_json(&sv.to_string()).unwrap();
        let set = load(&spec, Path::new("/")).unwrap();
        let t_load = t0.elapsed().as_secs_f64();
        assert_eq!(set.start.len(), 286_066);
        let ev = Evidence::from_spec(&set, Path::new("/")).unwrap();
        let res = check(&set, &ev, &CheckOptions { threads: 4, ..Default::default() }).unwrap();
        let dir = out.join(format!("statement-{policy}"));
        let timing = json!({"load_s": t_load, "check_s": res.seconds, "vm_hwm_kb": vm_hwm_kb()});
        write_statement(&set, &ev, &res, &dir, Some(timing)).unwrap();
        let cmp = compare(&set, &res, &hand);
        cmp.write(&dir).unwrap();
        println!(
            "T178 [{policy}]: load {:.1}s check {:.1}s VmHWM {} MB; tables {} records {}; keys ours {} hand {}; \
             counts {:?}; closure verdict {:?} rounds {} edges {} violations {}; blocking {:?}",
            t_load,
            res.seconds,
            vm_hwm_kb() / 1024,
            set.report.tables,
            set.report.records,
            res.rows.len(),
            hand.rows.len(),
            res.counts(),
            res.summary.verdict,
            res.summary.rounds,
            res.summary.edges,
            res.summary.descent_violations,
            res.blocking()
        );
        println!(
            "T178 [{policy}] comparison: key sets equal {} same server {} terminals equal {} ({:?} vs {:?}) rounds {:?} vs {}; \
             differences {:?}; unmapped hand rules {}; label matrix {:?}; requests solve {} facts {} proofs {}",
            cmp.key_sets_equal,
            cmp.same_server,
            cmp.terminals_equal,
            cmp.terminals_hand,
            cmp.terminals_ours,
            cmp.rounds_hand,
            cmp.rounds_ours,
            cmp.counts,
            cmp.unmapped_hand_rules.len(),
            cmp.label_matrix,
            res.solve_requests.len(),
            res.fact_requests.len(),
            res.proof_requests.len()
        );
        for d in cmp.differences.iter().take(12) {
            println!("  diff {:?}", d);
        }
    }
}

/// The FIRST END-TO-END shape (coordinator 07:11Z): T178's order-B material + RECIPES instead of the lane's symbolic /
/// point rules (the 136 explicit `rustred.recipe.v1` recipes of lane engine-line, derived from the checker's
/// reference) + the supplied symmetry relations + terminals. No relation is solved here: the check lists the (recipe,
/// key) SOLVE REQUESTS for lane `adapters` (interfaces/import.md 6.1) and the keys no recipe domain contains.
#[test]
#[ignore = "big data: T178 order-B material and the engine-line recipes; run with --ignored in release"]
fn t178_recipe_variant_solve_requests() {
    let ws = ws();
    let stmt = ws.join("SCRATCHPAD/t178-close/statements/statement-20261114T040105Z");
    let rdir = ws.join("SCRATCHPAD/engine-line/t178");
    if !stmt.join("statement.jsonl").is_file() || !rdir.join("index.json").is_file() {
        println!("SKIP: material missing");
        return;
    }
    let mut sv = build_spec(&ws, &stmt);
    let srcs = sv["sources"].as_array_mut().unwrap();
    srcs.retain(|s| s["class"] != "lane");
    let tail: Vec<Value> = srcs.split_off(srcs.len() - 2); // symmetry, terminals
    let index: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(rdir.join("index.json")).unwrap()).unwrap();
    for e in &index {
        let id = e["id"].as_str().unwrap();
        let p = rdir.join("recipes").join(format!("{id}.json"));
        assert!(p.is_file(), "{}", p.display());
        srcs.push(json!({"id": format!("recipe:{id}"), "kind":"recipe", "class":"recipe", "path": p.display().to_string()}));
    }
    srcs.extend(tail);
    let out = scratch().join("t178-recipes");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("spec.T178-recipes.json"), serde_json::to_string_pretty(&sv).unwrap()).unwrap();
    let t0 = std::time::Instant::now();
    let spec = TableSetSpec::from_json(&sv.to_string()).unwrap();
    let set = load(&spec, Path::new("/")).unwrap();
    let ev = Evidence::from_spec(&set, Path::new("/")).unwrap();
    let res = check(&set, &ev, &CheckOptions { threads: 4, ..Default::default() }).unwrap();
    write_statement(&set, &ev, &res, &out.join("statement"), Some(json!({"total_s": t0.elapsed().as_secs_f64(), "vm_hwm_kb": vm_hwm_kb()}))).unwrap();
    let per_recipe: BTreeMap<&str, usize> = res.solve_requests.iter().fold(BTreeMap::new(), |mut m, r| {
        *m.entry(r.source.as_str()).or_default() += 1;
        m
    });
    let no_domain = res
        .rows
        .iter()
        .filter(|r| r.open_status.as_deref() == Some("uncovered") && !r.open_reason.as_deref().unwrap_or("").contains("recipe"))
        .count();
    println!(
        "T178 recipe variant: {:.1}s VmHWM {} MB; keys {} counts {:?}; solve requests {} over {} recipes; uncovered keys in no recipe domain {}; blocking {:?}",
        t0.elapsed().as_secs_f64(),
        vm_hwm_kb() / 1024,
        res.rows.len(),
        res.counts(),
        res.solve_requests.len(),
        per_recipe.len(),
        no_domain,
        res.blocking()
    );
}
