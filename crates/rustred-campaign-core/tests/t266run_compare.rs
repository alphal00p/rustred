//! Lane t266run, DATA TOOLING (ignored): OUR final T266 statement (the closing loop's, Tier C) against the independent
//! checker's delivery classes (symmetry image / d0 = symmetry at the key alone / recipe / terminal; read-only
//! /common/ben/sym-investigation/k7/runs/jointest9-full). Node names and numbers here are test data (L8).
//! Env: `T266RUN_STATEMENT` (our statement.jsonl), `T266RUN_COMPARE_OUT` (report JSON path).

use rustred_campaign_core::import;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::BufRead;
use std::path::{Path, PathBuf};

const RUN: &str = "/common/ben/sym-investigation/k7/runs/jointest9-full";
const ARITY: usize = 20;

fn kt(t: &str) -> Vec<i32> {
    import::lines::parse_key_text(t, ARITY).unwrap_or_else(|| panic!("key text {t}"))
}

fn ints_key(line: &str) -> Vec<i32> {
    line.split_whitespace()
        .take(ARITY)
        .map(|x| x.parse().unwrap())
        .collect()
}

fn sorted_dirs(prefix: &str) -> Vec<PathBuf> {
    let mut v: Vec<(usize, PathBuf)> = std::fs::read_dir(RUN)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            n.strip_prefix(prefix)
                .and_then(|x| x.parse::<usize>().ok())
                .map(|i| (i, e.path()))
        })
        .collect();
    v.sort();
    v.into_iter().map(|x| x.1).collect()
}

/// The checker's per-key class of its relation-served keys (first class wins: image, then d0, then recipe) and terminals.
fn checker_classes() -> (BTreeMap<Vec<i32>, String>, BTreeSet<Vec<i32>>, Value) {
    let mut m: BTreeMap<Vec<i32>, String> = BTreeMap::new();
    for svc in sorted_dirs("svc") {
        for l in std::fs::read_to_string(svc.join("extra_sym.txt"))
            .unwrap_or_default()
            .lines()
        {
            if !l.trim().is_empty() {
                m.entry(ints_key(l))
                    .or_insert_with(|| "symmetry image".into());
            }
        }
        for l in std::fs::read_to_string(svc.join("d0.jsonl"))
            .unwrap_or_default()
            .lines()
        {
            let v: Value = serde_json::from_str(l).unwrap();
            if v["accepted"] == json!(true) {
                m.entry(kt(v["key"].as_str().unwrap()))
                    .or_insert_with(|| "d0".into());
            }
        }
    }
    for w in sorted_dirs("work") {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&w)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("walk_"))
            })
            .collect();
        files.sort();
        for f in files {
            for l in std::fs::read_to_string(&f).unwrap().lines() {
                let v: Value = serde_json::from_str(l).unwrap();
                if v["result"] == json!("served") {
                    m.entry(kt(v["key"].as_str().unwrap()))
                        .or_insert_with(|| "recipe".into());
                }
            }
        }
    }
    let mut t = BTreeSet::new();
    for l in std::fs::read_to_string(format!("{RUN}/deliver/terminals.jsonl"))
        .unwrap()
        .lines()
    {
        let v: Value = serde_json::from_str(l).unwrap();
        t.insert(kt(v["key"].as_str().unwrap()));
    }
    let closure: Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{RUN}/deliver/closure.json")).unwrap(),
    )
    .unwrap();
    (m, t, closure)
}

/// Our class of a statement row.
fn our_class(v: &Value) -> String {
    match v["status"].as_str() {
        Some("terminal") => "terminal".into(),
        Some("open") => format!("open:{}", v["open"]["status"].as_str().unwrap_or("?")),
        Some("served") => {
            let s = &v["server"];
            match (s["kind"].as_str(), s["class"].as_str()) {
                (Some("credit"), _) => "symmetry image".into(),
                (Some("rule"), _) => "rule".into(),
                (Some("relation"), Some("symmetry")) => "symmetry at the key".into(),
                (Some("relation"), Some(c)) => format!("relation:{c}"),
                (k, c) => format!("{k:?}/{c:?}"),
            }
        }
        x => format!("{x:?}"),
    }
}

#[test]
#[ignore = "data tooling: our final statement vs the checker's delivery classes (needs the env variables)"]
fn compare_t266_statement_with_the_checker() {
    let st = PathBuf::from(std::env::var("T266RUN_STATEMENT").expect("T266RUN_STATEMENT"));
    let out = PathBuf::from(std::env::var("T266RUN_COMPARE_OUT").expect("T266RUN_COMPARE_OUT"));
    let (ck, cterm, closure) = checker_classes();
    let mut ours: BTreeMap<Vec<i32>, (String, String)> = BTreeMap::new(); // only keys of interest
    let mut by_class: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    let mut total = 0u64;
    let rd = std::io::BufReader::with_capacity(1 << 22, std::fs::File::open(&st).unwrap());
    for l in rd.lines() {
        let l = l.unwrap();
        if l.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(&l).unwrap();
        total += 1;
        let key: Vec<i32> = serde_json::from_value(v["key"].clone()).unwrap();
        let c = our_class(&v);
        let lvl = v["evidence"]
            .as_str()
            .unwrap_or(if v["status"] == "terminal" {
                "terminal"
            } else {
                "none"
            })
            .to_string();
        *by_class
            .entry(c.clone())
            .or_default()
            .entry(lvl.clone())
            .or_default() += 1;
        if c != "rule" || ck.contains_key(&key) || cterm.contains(&key) {
            ours.insert(key, (c, lvl));
        }
    }
    // checker class x our class over the checker's relation-served keys and terminals
    let mut cross: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    for (k, c) in &ck {
        let o = ours
            .get(k)
            .map(|x| x.0.clone())
            .unwrap_or_else(|| "rule or not in our closure".into());
        *cross.entry(c.clone()).or_default().entry(o).or_default() += 1;
    }
    for k in &cterm {
        let o = ours
            .get(k)
            .map(|x| x.0.clone())
            .unwrap_or_else(|| "rule or not in our closure".into());
        *cross
            .entry("terminal".into())
            .or_default()
            .entry(o)
            .or_default() += 1;
    }
    // our non-rule keys the checker serves by a table rule (or that lie outside its relation classes)
    let mut ours_only: BTreeMap<String, u64> = BTreeMap::new();
    let mut examples: Vec<Value> = vec![];
    for (k, (c, _)) in &ours {
        if c != "rule" && !ck.contains_key(k) && !cterm.contains(k) {
            *ours_only.entry(c.clone()).or_default() += 1;
            if examples.len() < 40 {
                examples.push(json!({"key": import::lines::key_text(k), "ours": c}));
            }
        }
    }
    let rep = json!({"our_closure_keys": total, "checker_closure": closure, "our_class_level": by_class,
        "checker_class_x_our_class": cross, "our_relation_keys_outside_the_checker_classes": ours_only,
        "examples": examples, "our_terminals": ours.iter().filter(|(_, v)| v.0 == "terminal")
            .map(|(k, _)| import::lines::key_text(k)).collect::<Vec<_>>(),
        "checker_terminals": cterm.iter().map(|k| import::lines::key_text(k)).collect::<Vec<_>>()});
    std::fs::write(&out, serde_json::to_string_pretty(&rep).unwrap()).unwrap();
    println!(
        "T266RUN COMPARE {}",
        serde_json::to_string(&rep["checker_class_x_our_class"]).unwrap()
    );
    assert!(Path::new(&out).is_file());
}
