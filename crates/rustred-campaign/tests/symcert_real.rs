//! Lane symcert REAL runs (ignored; interfaces/proof-symrows-job.md 5): the relations of one producer put to the FROZEN
//! kernel (rr_proof --symrows) through `certify_symmetry_relations` (the process layer; run inside a private pid
//! namespace, L10). Driven by the JSON file named by `SYMCERT_REAL` (test data / run configuration, never code):
//! {"producer": "tworow" | "composite" | "atkey", "frame_table": path (header family_source + mask; its zero_sectors
//!  record is the census), "group_digest": the producer's, "inputs": {producer files}, "catalogue_tables": [paths],
//!  "catalogue_dirs": [dirs], "sample": n | null, "composite_only": bool, "kernel": path, "kernel_sha256": hex,
//!  "work_root": dir, "parallel": n, "mem_cap_gb": n, "wall_s": n, "arity": n, "census": "frame" | "lee" | "none",
//!  "expect_blake3": {path: hex prefix}}.
//! Writes <work_root>/summary.json (+ verdicts.jsonl / findings.jsonl of the batch).

use rustred_campaign::adapters::job::{Invocation, Program, sha256_hex};
use rustred_campaign::closeloop::symcert::{CertifyConfig, certify_symmetry_relations, summary};
use rustred_campaign_core::replay_base::family::Family;
use rustred_campaign_core::replay_base::symrows::{DeriveOptions, verify_derived};
use rustred_campaign_core::symcert::{SymRelation, ZeroSectors, census, producers};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn s(v: &Value, k: &str) -> String {
    v[k].as_str()
        .unwrap_or_else(|| panic!("config {k}"))
        .to_string()
}

fn read(p: &str) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

/// Every rule record whose raw-line SHA-256 is wanted, from table files and table directories.
fn catalogue_records(files: &[PathBuf], want: &BTreeSet<String>) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for f in files {
        let Ok(h) = std::fs::File::open(f) else {
            continue;
        };
        for l in std::io::BufReader::with_capacity(1 << 22, h)
            .lines()
            .map_while(Result::ok)
        {
            if !l.starts_with("{\"type\":\"rule\"") {
                continue;
            }
            let d = sha256_hex(l.as_bytes());
            if want.contains(&d) {
                out.insert(d, serde_json::from_str(&l).unwrap());
            }
        }
        if out.len() == want.len() {
            break;
        }
    }
    out
}

#[test]
#[ignore = "real kernel run: SYMCERT_REAL=<config.json>, LIGHT_PIDNS=1"]
fn symcert_real() {
    let cfgp = std::env::var("SYMCERT_REAL").expect("SYMCERT_REAL");
    let c: Value = serde_json::from_str(&read(&cfgp)).unwrap();
    let t0 = Instant::now();
    // pinned inputs (copies by reading): expected digests
    if let Some(m) = c["expect_blake3"].as_object() {
        for (p, want) in m {
            let b = std::fs::read(p).unwrap();
            let d = rustred_campaign_contract::blake3_hex(&b);
            assert!(d.starts_with(want.as_str().unwrap()), "{p}: blake3 {d}");
        }
    }
    // the kernel: frozen binary, sha256 from its directory's SHA256SUMS
    let kpath = PathBuf::from(s(&c, "kernel"));
    let prog = Program::from_file("rr_proof", &kpath, Invocation::Legacy).unwrap();
    assert_eq!(
        prog.sha256.as_deref(),
        Some(s(&c, "kernel_sha256").as_str()),
        "kernel sha256"
    );
    // the frame: family, chart, group (re-derived; digest must be the producer's), census
    let frame = s(&c, "frame_table");
    let h = rustred_campaign_core::import::lines::read_header("frame", Path::new(&frame)).unwrap();
    let fam_src = h.raw["family_source"]
        .as_str()
        .expect("family_source")
        .to_string();
    let chart: Vec<bool> = h
        .mask
        .as_deref()
        .expect("mask")
        .chars()
        .map(|ch| ch == '1')
        .collect();
    let fam = Family::parse(&fam_src).unwrap();
    let vg = verify_derived(&fam, &chart, &DeriveOptions::default()).unwrap();
    assert_eq!(vg.digest(), s(&c, "group_digest"), "group digest");
    let mats: Vec<Option<Vec<i64>>> = vg.proofs().iter().map(|p| Some(p.matrix.clone())).collect();
    let lee = census::from_predicate(
        &chart,
        &|k: &[i64]| fam.zero_key(k),
        "campaign-core Family::zero_key over the chart's subsets",
        24,
    )
    .unwrap();
    let frame_census = census::table_record(Path::new(&frame)).unwrap().map(|l| {
        census::from_record_line(
            &l,
            &format!(
                "engine table {frame} (zero_sectors record sha256 {})",
                sha256_hex(l.as_bytes())
            ),
        )
        .unwrap()
    });
    let census_cmp = frame_census.as_ref().map(|e| {
        let (a, b) = census::compare(&lee, e, &chart);
        json!({"only_lee": a.len(), "only_engine": b.len(), "lee": lee.masks.len(), "engine": e.masks.len(),
            "examples_only_lee": a.iter().take(5).collect::<Vec<_>>(),
            "examples_only_engine": b.iter().take(5).collect::<Vec<_>>()})
    });
    let zero: Option<ZeroSectors> = match c["census"].as_str().unwrap_or("frame") {
        "frame" => Some(
            frame_census
                .clone()
                .expect("the frame table has no zero_sectors record"),
        ),
        "lee" => Some(lee.clone()),
        _ => None,
    };
    // the producer's relations
    let inputs = &c["inputs"];
    let mut rels: Vec<SymRelation> = vec![];
    let mut not_built: Vec<Value> = vec![];
    match s(&c, "producer").as_str() {
        "tworow" => {
            for t in producers::tworow(&read(&s(inputs, "results"))).unwrap() {
                match producers::tworow_relation(&t, &mats, vg.digest(), zero.clone()) {
                    Ok(r) => rels.push(r),
                    Err(e) => not_built.push(json!({"key": t.key, "why": e.why, "class": e.class})),
                }
            }
        }
        "composite" => {
            let mut all = producers::composites(
                &read(&s(inputs, "table")),
                &read(&s(inputs, "verification")),
            )
            .unwrap();
            if c["composite_only"].as_bool() == Some(true) {
                all.retain(|x| !x.plain);
            }
            all.sort_by(|a, b| a.record_sha256.cmp(&b.record_sha256));
            if let Some(n) = c["sample"].as_u64() {
                all.truncate(n as usize);
            }
            let want: BTreeSet<String> = all
                .iter()
                .flat_map(|x| x.replacements.iter().map(|r| r.record_sha256.clone()))
                .collect();
            let mut files: Vec<PathBuf> = c["catalogue_tables"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|x| PathBuf::from(x.as_str().unwrap()))
                .collect();
            for d in c["catalogue_dirs"].as_array().into_iter().flatten() {
                let mut v: Vec<PathBuf> = std::fs::read_dir(d.as_str().unwrap())
                    .unwrap()
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
                    .collect();
                v.sort();
                files.extend(v);
            }
            let cat = catalogue_records(&files, &want);
            println!("catalogue: {} of {} records found", cat.len(), want.len());
            for x in &all {
                match producers::composite_relation(
                    x,
                    &mats,
                    &|d| cat.get(d).cloned(),
                    zero.clone(),
                ) {
                    Ok(r) => rels.push(r),
                    Err(e) => not_built.push(json!({"key": x.key, "why": e.why, "class": e.class})),
                }
            }
        }
        "atkey" => {
            let mut all = producers::symmetry_at_key(
                &read(&s(inputs, "table")),
                &read(&s(inputs, "escalation")),
            )
            .unwrap();
            all.sort_by(|a, b| a.record_sha256.cmp(&b.record_sha256));
            if let Some(n) = c["sample"].as_u64() {
                all.truncate(n as usize);
            }
            for a in &all {
                assert_eq!(a.group_digest, vg.digest(), "escalation group digest");
                match producers::at_key_relation(a, &mats, zero.clone()) {
                    Ok(r) => rels.push(r),
                    Err(e) => not_built.push(json!({"key": a.key, "why": e.why, "class": e.class})),
                }
            }
        }
        p => panic!("producer {p}"),
    }
    let work = PathBuf::from(s(&c, "work_root"));
    let cfg = CertifyConfig {
        work_root: work.clone(),
        program: prog,
        licence_env: Some("SYMBOLICA_LICENSE".into()),
        family_source: fam_src.clone(),
        caps: serde_json::from_value(json!({"arity": c["arity"]})).unwrap(),
        parallel: c["parallel"].as_u64().unwrap_or(4) as usize,
        cpus: vec![],
        memory_cap_bytes: Some(c["mem_cap_gb"].as_u64().unwrap_or(8) << 30),
        wall_cap: Some(Duration::from_secs(c["wall_s"].as_u64().unwrap_or(3600))),
        resource_retries: 2,
        resource_backoff: Duration::from_secs(10),
        max_primes: c["max_primes"].as_u64().map(|x| x as u32),
        fake: None,
        fake_by_id: BTreeMap::new(),
    };
    println!(
        "{} relations to certify ({} not built)",
        rels.len(),
        not_built.len()
    );
    let v = certify_symmetry_relations(&rels, &cfg).unwrap();
    let sum = json!({"config": c, "summary": summary(&v), "not_built": not_built, "census_comparison": census_cmp,
        "group": {"digest": vg.digest(), "elements": vg.proofs().len()}, "wall_s": t0.elapsed().as_secs_f64()});
    std::fs::write(
        work.join("summary.json"),
        serde_json::to_vec_pretty(&sum).unwrap(),
    )
    .unwrap();
    println!("{}", serde_json::to_string_pretty(&sum["summary"]).unwrap());
    println!("census: {}", sum["census_comparison"]);
}
