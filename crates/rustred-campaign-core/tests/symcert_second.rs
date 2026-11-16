//! Lane symcert, SECOND ROUTE (a) (ignored; comparison only, never evidence): the frozen engine's `rr_node symface`
//! answers per key (face mode with `--check-keys`, run by hand in a pid namespace, outputs read here) against OUR
//! symmetry choices on the same keys (the repair flow's `choices.jsonl` + its shipped relations): served by both /
//! ours only / engine only / neither, same element (by line permutation: the two group enumerations differ), same
//! relation term by term. Driven by `SYMCERT_SYMFACE` = JSON {"engine_out": dir, "keys_dir": dir, "choices": path,
//! "shipped": path, "frame_table": path, "out": path}; writes `<out>` (one line per key) and `<out>.summary.json`.

use rustred_campaign_core::replay_base::family::Family;
use rustred_campaign_core::replay_base::symrows::{DeriveOptions, verify_derived};
use rustred_campaign_core::symcert::producers::{Rhs, point_record};
use rustred_campaign_core::symcert::second::{
    EngineKey, OurKey, compare_symface, counts, engine_keys,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;

#[test]
#[ignore = "reads SYMCERT_SYMFACE (engine symface outputs + the repair flow's choices)"]
fn symcert_symface_second_route() {
    let c: Value = serde_json::from_str(
        &std::fs::read_to_string(std::env::var("SYMCERT_SYMFACE").unwrap()).unwrap(),
    )
    .unwrap();
    let st = |k: &str| c[k].as_str().unwrap().to_string();
    // our group (the frame's, re-derived) -> line permutations
    let h =
        rustred_campaign_core::import::lines::read_header("frame", Path::new(&st("frame_table")))
            .unwrap();
    let chart: Vec<bool> = h
        .mask
        .as_deref()
        .unwrap()
        .chars()
        .map(|x| x == '1')
        .collect();
    let fam = Family::parse(h.raw["family_source"].as_str().unwrap()).unwrap();
    let vg = verify_derived(&fam, &chart, &DeriveOptions::default()).unwrap();
    let ours_perm: Vec<Option<Vec<usize>>> = vg
        .group()
        .elements
        .iter()
        .map(|e| e.line_perm(&chart))
        .collect();
    // our side: every key's choices, and the shipped relation by target
    let mut shipped: BTreeMap<Vec<i64>, (usize, Rhs)> = BTreeMap::new();
    for l in std::fs::read_to_string(st("shipped")).unwrap().lines() {
        let Ok(v) = serde_json::from_str::<Value>(l) else {
            continue;
        };
        if v["type"] != "rule" {
            continue;
        }
        let (k, rhs) = point_record(&v).unwrap();
        let e = v["symmetry_elements"][0].as_u64().unwrap() as usize;
        shipped.insert(k, (e, rhs));
    }
    let mut ours: BTreeMap<Vec<i64>, OurKey> = BTreeMap::new();
    for l in std::fs::read_to_string(st("choices")).unwrap().lines() {
        let v: Value = serde_json::from_str(l).unwrap();
        let k: Vec<i64> = serde_json::from_value(v["key"].clone()).unwrap();
        let choices = v["choices"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|x| matches!(x["kind"].as_str(), Some("plain" | "composite")))
            .map(|x| {
                (
                    x["element"].as_u64().map(|e| e as usize),
                    x["kind"].as_str().unwrap().to_string(),
                    x["rhs"].as_array().map_or(0, |r| r.len()),
                )
            })
            .collect();
        ours.insert(
            k.clone(),
            OurKey {
                choices,
                shipped: shipped.get(&k).cloned(),
            },
        );
    }
    let out_dir = st("engine_out");
    let faces: Vec<Value> = std::fs::read_to_string(format!("{out_dir}/faces.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let mut w = std::fs::File::create(st("out")).unwrap();
    let mut summary = serde_json::Map::new();
    for f in &faces {
        let lab = f["label"].as_str().unwrap();
        let keys: BTreeSet<Vec<i64>> =
            std::fs::read_to_string(format!("{}/{lab}.keys.jsonl", st("keys_dir")))
                .unwrap()
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| serde_json::from_str(l).unwrap())
                .collect();
        let eng: Vec<EngineKey> =
            std::fs::read_to_string(format!("{out_dir}/{lab}.out/check_keys.jsonl"))
                .map(|t| engine_keys(&t).unwrap())
                .unwrap_or_default();
        // the engine element's line permutation from its slice records
        let mut eperm: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        if let Ok(t) = std::fs::read_to_string(format!("{out_dir}/{lab}.out/F.table.jsonl")) {
            for l in t.lines().filter(|l| l.contains("\"symface\"")) {
                let v: Value = serde_json::from_str(l).unwrap();
                let el = &v["symface"]["element"];
                if let (Some(i), Ok(p)) = (
                    el["index"].as_u64(),
                    serde_json::from_value::<Vec<usize>>(el["line_permutation"].clone()),
                ) {
                    eperm.insert(i as usize, p);
                }
            }
        }
        let face_ours: BTreeMap<Vec<i64>, OurKey> = keys
            .iter()
            .map(|k| (k.clone(), ours.get(k).cloned().unwrap_or_default()))
            .collect();
        let eng: Vec<EngineKey> = eng.into_iter().filter(|e| keys.contains(&e.key)).collect();
        let cmp = compare_symface(&eng, &face_ours, &|i| eperm.get(&i).cloned(), &|i| {
            ours_perm.get(i).cloned().flatten()
        });
        for x in &cmp {
            writeln!(w, "{}", json!({"face": lab, "cmp": x})).unwrap();
        }
        let n = counts(&cmp);
        println!("{lab} exit {} keys {}: {n:?}", f["exit"], keys.len());
        summary.insert(
            lab.into(),
            json!({"exit": f["exit"], "seconds": f["seconds"], "keys": keys.len(), "counts": n}),
        );
    }
    std::fs::write(
        format!("{}.summary.json", st("out")),
        serde_json::to_vec_pretty(&Value::Object(summary)).unwrap(),
    )
    .unwrap();
}
