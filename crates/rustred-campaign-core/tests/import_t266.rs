//! Lanes import2 / import3, BIG DATA (ignored; release): T266 on table jointest9 (85 GB, 33,660 rules, pre-guard
//! records mostly without witness loci) against the independent checker's delivery
//! (/common/ben/sym-investigation/k7/runs/jointest9-full, read-only). The spec is DATA: tests/data/import/
//! t266-jointest9.spec.json (+ the box B document); node names and numbers below are TEST DATA (L8).
//!
//! - `t266_jointest9_index_and_strict_load`: the STRICT load (no record removed: the four records whose exception
//!   literals exceed i128 load exactly since import3 item 1) through the persistent table index (item 2): the first run
//!   builds the index in ONE pass over the table, later runs reuse it. Time and VmHWM printed.
//! - `t266_jointest9_walk_vs_delivery`: the full keys-only (Tier K) walk of box B (item 3) with the checker's
//!   assumption for this pre-guard table (witness check OFF: DIAGNOSTIC, nothing certified), OUR verified symmetry
//!   group's credits, the checker's terminals, and the checker's OWN keys-only relations for the keys its symmetry
//!   service did not serve (symmetry rows at the key = `d0`, recipe solves) as DIAGNOSTIC relations (followed, never
//!   certified). Compared key by key with the delivery: totals, box, the (D, r, in-box, class) layers, the relation
//!   classes (symmetry image / d0 / recipe) as key sets, terminals; and per served key WHAT IT WAITS FOR in a strict run
//!   with the per-key lift (record class + evidence).
//! - `t266_recipe_requests_vs_checker_attempts` (item 4): the 52 v1.2 recipes + assignment from the delivery's steps,
//!   their explicit seed centres checked against the checker's own attempt records.
//!
//! Env: `RUSTRED_IMPORT_SCRATCH` (default /common/build-cache/$USER/p_integral_dev/implementer-scratch/import3),
//! `RUSTRED_IMPORT_INDEX_DIR` (default <scratch>/index).

mod inputs;
use rustred_campaign_core::import;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use import::{CheckOptions, Evidence, LoadOptions, TableSetSpec, load_with, write_statement};
use serde_json::{Value, json};

/// lane closeloop4 (audit-closeloop M1 / m4): the spec text with its evidence PINNED (each file's sha256, a
/// SHA256SUMS manifest beside a directory) and the family of its tables named (spec evidence is read only so).
#[allow(dead_code)]
fn pin_ev(spec: &serde_json::Value) -> String {
    use std::io::BufRead;
    let mut v = spec.clone();
    let fam = v["sources"].as_array().and_then(|a| {
        a.iter().find_map(|s| {
            let p = s["path"].as_str()?;
            let f = std::fs::File::open(p).ok()?;
            let mut l = String::new();
            std::io::BufReader::new(f).read_line(&mut l).ok()?;
            let h: serde_json::Value = serde_json::from_str(l.trim()).ok()?;
            h["family"].as_str().map(str::to_string)
        })
    });
    let sha = |p: &std::path::Path| rustred_campaign_core::import::lines::file_sha256(p).unwrap();
    if let Some(ev) = v.get_mut("evidence").and_then(|e| e.as_object_mut()) {
        for k in ["facts", "proofs", "solved"] {
            if let Some(a) = ev.get_mut(k).and_then(|x| x.as_array_mut()) {
                for e in a {
                    let p = std::path::PathBuf::from(e["path"].as_str().unwrap_or(""));
                    if p.is_file() && e.get("sha256").is_none() {
                        e["sha256"] = serde_json::json!(sha(&p));
                    }
                    if p.is_dir() && e.get("manifest").is_none() {
                        let mut names: Vec<String> = std::fs::read_dir(&p)
                            .unwrap()
                            .flatten()
                            .map(|x| x.file_name().to_string_lossy().into_owned())
                            .filter(|n| n.ends_with(".json") || n.ends_with(".jsonl"))
                            .collect();
                        names.sort();
                        let body: String = names
                            .iter()
                            .map(|n| format!("{}  {n}\n", sha(&p.join(n))))
                            .collect();
                        let m = std::env::temp_dir().join(format!(
                            "pin-ev-{}-{}.SHA256SUMS",
                            std::process::id(),
                            rustred_campaign_core::closure::sha256::sha256_tag(
                                p.display().to_string().as_bytes()
                            )
                        ));
                        std::fs::write(&m, body).unwrap();
                        e["manifest"] =
                            serde_json::json!({"path": m.display().to_string(), "sha256": sha(&m)});
                    }
                    if e.get("family").is_none()
                        && let Some(f) = &fam
                    {
                        e["family"] = serde_json::json!(f);
                    }
                }
            }
        }
    }
    v.to_string()
}

const RUN: &str = "/common/ben/sym-investigation/k7/runs/jointest9-full";
const ARITY: usize = 20;

fn status_kb(field: &str) -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with(field))
                .and_then(|l| l.split_whitespace().nth(1).and_then(|x| x.parse().ok()))
        })
        .unwrap_or(0)
}

fn scratch() -> PathBuf {
    std::env::var_os("RUSTRED_IMPORT_SCRATCH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(format!(
                "/common/build-cache/{}/p_integral_dev/implementer-scratch/import3",
                std::env::var("USER").unwrap_or_else(|_| "vhirschi".into())
            ))
        })
}

fn index_dir() -> PathBuf {
    std::env::var_os("RUSTRED_IMPORT_INDEX_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| scratch().join("index"))
}

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/import")
}

fn spec_value() -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(data_dir().join("t266-jointest9.spec.json")).unwrap(),
    )
    .unwrap()
}

fn kt(t: &str) -> Vec<i32> {
    import::lines::parse_key_text(t, ARITY).unwrap_or_else(|| panic!("key text {t}"))
}

/// The checker's per-key relation classes (its pipeline: symmetry image first, then d0, then the recipe walk; first
/// wins), each with its keys-only right-hand side (top keys).
struct Delivery {
    sym_image: BTreeMap<Vec<i32>, Vec<Vec<i32>>>,
    d0: BTreeMap<Vec<i32>, Vec<Vec<i32>>>,
    recipe: BTreeMap<Vec<i32>, (String, Vec<Vec<i32>>)>,
    terminals: BTreeSet<Vec<i32>>,
    closure: Value,
    /// relations_<last>.txt: key -> rhs (the relations the checker's last closure used)
    relations: BTreeMap<Vec<i32>, Vec<Vec<i32>>>,
}

fn ints_rel(line: &str) -> (Vec<i32>, Vec<Vec<i32>>) {
    let v: Vec<i32> = line
        .split_whitespace()
        .map(|x| x.parse().unwrap())
        .collect();
    let key = v[..ARITY].to_vec();
    let n = v[ARITY] as usize;
    let rhs = (0..n)
        .map(|i| v[ARITY + 1 + i * ARITY..ARITY + 1 + (i + 1) * ARITY].to_vec())
        .collect();
    (key, rhs)
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

fn read_delivery() -> Delivery {
    let mut d = Delivery {
        sym_image: BTreeMap::new(),
        d0: BTreeMap::new(),
        recipe: BTreeMap::new(),
        terminals: BTreeSet::new(),
        closure: serde_json::from_str(
            &std::fs::read_to_string(format!("{RUN}/deliver/closure.json")).unwrap(),
        )
        .unwrap(),
        relations: BTreeMap::new(),
    };
    let taken = |d: &Delivery, k: &Vec<i32>| {
        d.sym_image.contains_key(k) || d.d0.contains_key(k) || d.recipe.contains_key(k)
    };
    for svc in sorted_dirs("svc") {
        for l in std::fs::read_to_string(svc.join("extra_sym.txt"))
            .unwrap_or_default()
            .lines()
        {
            if l.trim().is_empty() {
                continue;
            }
            let (k, r) = ints_rel(l);
            if !taken(&d, &k) {
                d.sym_image.insert(k, r);
            }
        }
        for l in std::fs::read_to_string(svc.join("d0.jsonl"))
            .unwrap_or_default()
            .lines()
        {
            let v: Value = serde_json::from_str(l).unwrap();
            if v["accepted"] == json!(true) {
                let k = kt(v["key"].as_str().unwrap());
                let r = v["top_rhs_keys"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| kt(x.as_str().unwrap()))
                    .collect();
                if !taken(&d, &k) {
                    d.d0.insert(k, r);
                }
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
                    let k = kt(v["key"].as_str().unwrap());
                    let r = v["top_rhs_keys"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|x| kt(x.as_str().unwrap()))
                        .collect();
                    if !taken(&d, &k) {
                        d.recipe
                            .insert(k, (v["recipe"].as_str().unwrap().to_string(), r));
                    }
                }
            }
        }
    }
    for l in std::fs::read_to_string(format!("{RUN}/deliver/terminals.jsonl"))
        .unwrap()
        .lines()
    {
        let v: Value = serde_json::from_str(l).unwrap();
        d.terminals.insert(kt(v["key"].as_str().unwrap()));
    }
    let last = (0..100)
        .rev()
        .find(|i| Path::new(&format!("{RUN}/relations_{i}.txt")).is_file())
        .unwrap();
    for l in std::fs::read_to_string(format!("{RUN}/relations_{last}.txt"))
        .unwrap()
        .lines()
    {
        if !l.trim().is_empty() {
            let (k, r) = ints_rel(l);
            d.relations.insert(k, r);
        }
    }
    d
}

/// A table_dump file of point relations (keys only) in the frame's chart.
fn write_points<'a>(
    path: &Path,
    family: Option<&str>,
    mask: &str,
    rels: impl Iterator<Item = (&'a Vec<i32>, &'a Vec<Vec<i32>>)>,
) -> usize {
    let mut w = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    writeln!(
        w,
        "{}",
        json!({"type":"header","schema":"table_dump.v1","family": family})
    )
    .unwrap();
    let mut n = 0;
    for (i, (k, rhs)) in rels.enumerate() {
        let t = import::lines::key_text(k);
        let r: Vec<Value> = rhs
            .iter()
            .map(|x| json!([import::lines::key_text(x)]))
            .collect();
        writeln!(
            w,
            "{}",
            json!({"type":"rule","chart":mask,"index":i,"fixed":k,"equations":[],"target":t,
            "rhs":r,"exceptions":[],"dispatch":"Partition"})
        )
        .unwrap();
        n += 1;
    }
    writeln!(w, "{}", json!({"type":"end"})).unwrap();
    n
}

/// The checker's layer list `[((D, r, inbox, class), n), ...]` (Python tuple text in closure.json).
fn parse_layers(summary: &str) -> BTreeMap<(i32, i32, bool, i32), u64> {
    let mut m = BTreeMap::new();
    let Some(a) = summary.find("[((") else {
        return m;
    };
    let body = &summary[a..summary[a..]
        .find(")]\n")
        .map(|e| a + e + 2)
        .unwrap_or(summary.len())];
    for cell in body.split("((").skip(1) {
        let (inner, rest) = cell.split_once(')').unwrap();
        let p: Vec<&str> = inner.split(',').map(str::trim).collect();
        let n: u64 = rest
            .trim_start_matches(',')
            .trim()
            .trim_end_matches([')', ',', ']', ' '])
            .trim()
            .parse()
            .unwrap();
        m.insert(
            (
                p[0].parse().unwrap(),
                p[1].parse().unwrap(),
                p[2] == "True" || p[2] == "true",
                p[3].parse().unwrap(),
            ),
            n,
        );
    }
    m
}

fn dr(k: &[i32], lines: &[bool]) -> (i32, i32) {
    let mut d = 0;
    let mut r = 0;
    for (x, l) in k.iter().zip(lines) {
        if *l {
            d += (x - 1).max(0);
        } else {
            r += x.abs();
        }
    }
    (d, r)
}

#[test]
#[ignore = "big data: the 85 GB T266 table; first run builds the index (one pass), later runs reuse it"]
fn t266_jointest9_index_and_strict_load() {
    let sv = spec_value();
    let table = sv["sources"][0]["path"].as_str().unwrap().to_string();
    if !Path::new(&table).is_file() {
        inputs::missing(format_args!("SKIP: {table} missing"));
        return;
    }
    let t0 = std::time::Instant::now();
    let spec = TableSetSpec::from_json(&pin_ev(&sv)).unwrap();
    let set = load_with(
        &spec,
        &data_dir(),
        &LoadOptions {
            index_dir: Some(index_dir()),
        },
    )
    .unwrap();
    let secs = t0.elapsed().as_secs_f64();
    let src = &set.report.sources[0];
    let mut big = vec![];
    for label in [29474i64, 30764, 31751, 33573] {
        let k = set.rules[0]
            .iter()
            .position(|i| i.label == label)
            .expect("big-literal record dispatched");
        let d = &set.disp.tables()[0].domains[k];
        let nb = d
            .exceptions
            .iter()
            .flatten()
            .chain(&d.equations)
            .filter(|p| p.is_big())
            .count();
        big.push((label, nb, d.exceptions.len()));
        assert!(nb > 0, "label {label}: a big exact literal");
    }
    let rep = json!({"load_s": secs, "vm_hwm_kb": status_kb("VmHWM:"), "vm_rss_kb": status_kb("VmRSS:"),
        "index": src.indexes, "records_in_file": src.records_in_files, "dispatched": src.records_dispatched,
        "removed": src.removed.len(), "witness": src.witness, "big_literal_records": big,
        "bodies_bytes": set.bodies.memory_bytes(), "patterns": set.bodies.n_patterns(),
        "file_sha256": src.files.first().map(|f| f.1.clone())});
    println!("T266 STRICT LOAD {}", serde_json::to_string(&rep).unwrap());
    let out = scratch();
    std::fs::create_dir_all(&out).unwrap();
    let how = src.indexes.first().map(|o| o.how).unwrap_or("none");
    std::fs::write(
        out.join(format!("t266-load-{how}.json")),
        serde_json::to_string_pretty(&rep).unwrap(),
    )
    .unwrap();
    assert_eq!(src.removed.len(), 0);
    assert_eq!(src.records_dispatched, src.records_in_files);
}

/// What a key served in the diagnostic walk waits for in a STRICT run with the per-key lift (item 3).
fn waits_for(kind: &str, class: &str, basis: &str, image_witness: Option<&str>) -> String {
    match (kind, basis) {
        ("rule", "witness_loci") => "nothing (witness loci complete: served at imported)".into(),
        ("rule", "witness_unchecked_diagnostic") => {
            "per-key regularity fact or per-key proof at (record, key) (witness loci not carried)".into()
        }
        ("rule", "elided_unchecked_diagnostic") => "per-key proof at (record, key) (witness loci elided)".into(),
        ("rule", "witness_pole_diagnostic") => "nothing can serve it with this record: the key lies on a witness pole (strict: blocked)".into(),
        ("credit", _) => format!(
            "image evidence: the image's record ({}) needs its fact / proof at the IMAGE, and the closure an image lift for witness-absent images",
            image_witness.unwrap_or("?")
        ),
        ("relation", "symmetry_credit_via_relation") => format!(
            "the image's relation ({class}: d0 relation or recipe solve at the IMAGE); the identity I(key) = I(image) is exact (verified group)"
        ),
        ("relation", _) if class.contains("d0") => {
            "a d0 relation (symmetry rows at the key, no IBP seed: recipe v1.3) or a supplied symmetry relation re-derived from the verified group".into()
        }
        ("relation", _) => "the recipe solve (recipe, key) from the campaign's own child job".into(),
        _ => format!("{kind}/{basis}"),
    }
}

#[test]
#[ignore = "big data: full keys-only walk of T266 box B over jointest9 (needs the index; ~3 GB scope)"]
fn t266_jointest9_walk_vs_delivery() {
    let sv0 = spec_value();
    let table = sv0["sources"][0]["path"].as_str().unwrap().to_string();
    if !Path::new(&table).is_file() || !Path::new(RUN).is_dir() {
        inputs::missing(format_args!("SKIP: T266 material missing"));
        return;
    }
    let t00 = std::time::Instant::now();
    let del = read_delivery();
    // the checker's classes reproduce its own last relation file exactly (same keys, same rhs)
    let mut union: BTreeMap<Vec<i32>, Vec<Vec<i32>>> = BTreeMap::new();
    for (k, r) in &del.sym_image {
        union.insert(k.clone(), r.clone());
    }
    for (k, r) in &del.d0 {
        union.insert(k.clone(), r.clone());
    }
    for (k, (_, r)) in &del.recipe {
        union.insert(k.clone(), r.clone());
    }
    let rel_same = union
        .iter()
        .filter(|(k, r)| {
            del.relations.get(*k).is_some_and(|x| {
                x.iter().collect::<BTreeSet<_>>() == r.iter().collect::<BTreeSet<_>>()
            })
        })
        .count();
    println!(
        "T266 DELIVERY classes: symmetry image {}, d0 {}, recipe {} (union {}); relations file {} keys, {} equal \
         (key and rhs set); terminals {}",
        del.sym_image.len(),
        del.d0.len(),
        del.recipe.len(),
        union.len(),
        del.relations.len(),
        rel_same,
        del.terminals.len()
    );
    let out = scratch().join("t266-walk");
    std::fs::create_dir_all(&out).unwrap();
    let fhead = import::lines::read_header("jointest9", Path::new(&table)).unwrap();
    let mask = fhead.mask.clone().unwrap();
    let lines: Vec<bool> = mask.chars().map(|c| c == '1').collect();
    let pd0 = out.join("delivered-d0.table.jsonl");
    let prc = out.join("delivered-recipe.table.jsonl");
    write_points(&pd0, fhead.family.as_deref(), &mask, del.d0.iter());
    write_points(
        &prc,
        fhead.family.as_deref(),
        &mask,
        del.recipe.iter().map(|(k, (_, r))| (k, r)),
    );
    let mut sv = sv0.clone();
    let srcs = sv["sources"].as_array_mut().unwrap();
    let tail = srcs.split_off(1);
    srcs.push(
        json!({"id":"delivered_d0","kind":"supplied_relations","class":"delivered:d0",
        "path": pd0.display().to_string(),"diagnostic":true}),
    );
    srcs.push(
        json!({"id":"delivered_recipe","kind":"supplied_relations","class":"delivered:recipe",
        "path": prc.display().to_string(),"diagnostic":true}),
    );
    srcs.extend(tail);
    sv["policies"]["witness_check"] = json!("off_diagnostic");
    std::fs::write(
        out.join("spec.json"),
        serde_json::to_string_pretty(&sv).unwrap(),
    )
    .unwrap();
    let t0 = std::time::Instant::now();
    let spec = TableSetSpec::from_json(&pin_ev(&sv)).unwrap();
    let set = load_with(
        &spec,
        &data_dir(),
        &LoadOptions {
            index_dir: Some(index_dir()),
        },
    )
    .unwrap();
    let t_load = t0.elapsed().as_secs_f64();
    let hwm_load = status_kb("VmHWM:");
    println!(
        "T266W load {t_load:.1}s VmHWM {} MB VmRSS {} MB; index {:?}; records {}; strict domains {}; group {:?}",
        hwm_load / 1024,
        status_kb("VmRSS:") / 1024,
        set.report.sources[0].indexes,
        set.report.records,
        set.strict_domains.len(),
        set.report
            .sources
            .iter()
            .find_map(|s| s.group.clone())
            .map(|g| (g.elements, g.provenance))
    );
    let ev = Evidence::from_spec(&set, &data_dir()).unwrap();
    let t1 = std::time::Instant::now();
    let rows_path = out.join("rows.jsonl");
    let res =
        import::check::check_streamed(&set, &ev, &CheckOptions::default(), &rows_path).unwrap();
    let t_check = t1.elapsed().as_secs_f64();
    let hwm_check = status_kb("VmHWM:");
    println!(
        "T266W check {t_check:.1}s VmHWM {} MB: keys {} in_start {} rounds {} edges {} open {} credited {} verdict {:?}; counts {:?}",
        hwm_check / 1024,
        res.summary.keys_total,
        res.summary.keys_in_start,
        res.summary.rounds,
        res.summary.edges,
        res.summary.open_total,
        res.summary.credited,
        res.summary.verdict,
        res.counts()
    );
    let dir = out.join("statement");
    write_statement(
        &set,
        &ev,
        &res,
        &dir,
        Some(json!({"load_s": t_load, "check_s": t_check,
        "vm_hwm_kb": hwm_check, "index": set.report.sources[0].indexes})),
    )
    .unwrap();
    // ---- key by key over the streamed rows
    let witness_of: HashMap<i64, String> = set.rules[0]
        .iter()
        .map(|i| (i.label, format!("{:?}", i.witness)))
        .collect();
    let mut layers: BTreeMap<(i32, i32, bool, i32), u64> = BTreeMap::new();
    let mut credited: BTreeSet<Vec<i32>> = BTreeSet::new();
    let mut by_rel: BTreeMap<String, BTreeSet<Vec<i32>>> = BTreeMap::new();
    let mut terminals: BTreeSet<Vec<i32>> = BTreeSet::new();
    let mut open: BTreeMap<Vec<i32>, String> = BTreeMap::new();
    let mut waits: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    let mut by_witness: BTreeMap<String, u64> = BTreeMap::new();
    let (mut n, mut n_box, mut credited_via_relation) = (0u64, 0u64, 0u64);
    let rd = BufReader::with_capacity(
        1 << 22,
        std::fs::File::open(dir.join("statement.jsonl")).unwrap(),
    );
    for l in rd.lines() {
        let v: Value = serde_json::from_str(&l.unwrap()).unwrap();
        let k: Vec<i32> = serde_json::from_value(v["key"].clone()).unwrap();
        let inb = v["in_start"] == json!(true);
        n += 1;
        n_box += u64::from(inb);
        let st = v["status"].as_str().unwrap();
        let kind = v["server"]["kind"].as_str().unwrap_or("");
        let cls = match (st, kind) {
            _ if v["basis"].as_str() == Some("symmetry_credit_via_relation") => 3,
            ("served", "rule") => 1,
            ("served", _) => 3,
            _ => 0,
        };
        let (d, r) = dr(&k, &lines);
        *layers.entry((d, r, inb, cls)).or_default() += 1;
        let via = v["basis"].as_str() == Some("symmetry_credit_via_relation");
        match (st, kind) {
            ("served", "credit") => {
                credited.insert(k.clone());
            }
            ("served", "relation") if via => {
                credited.insert(k.clone());
                credited_via_relation += 1;
            }
            ("served", "relation") => {
                by_rel
                    .entry(v["server"]["class"].as_str().unwrap_or("?").to_string())
                    .or_default()
                    .insert(k.clone());
            }
            ("terminal", _) => {
                terminals.insert(k.clone());
            }
            ("open", _) => {
                open.insert(
                    k.clone(),
                    v["open"]["status"].as_str().unwrap_or("?").to_string(),
                );
            }
            _ => {}
        }
        if st == "served" {
            let class = v["server"]["class"].as_str().unwrap_or("?").to_string();
            let basis = v["basis"].as_str().unwrap_or("-");
            let label = v["server"]["label"].as_i64();
            let w = label.and_then(|l| witness_of.get(&l)).map(String::as_str);
            if kind == "rule" {
                *by_witness
                    .entry(format!("{}:{basis}", w.unwrap_or("?")))
                    .or_default() += 1;
            }
            let wf = waits_for(kind, &class, basis, if kind == "credit" { w } else { None });
            *waits
                .entry(format!("{kind}:{class}"))
                .or_default()
                .entry(wf)
                .or_default() += 1;
        }
    }
    // ---- comparison
    let del_layers = parse_layers(del.closure["closure_summary"].as_str().unwrap_or(""));
    let mut layer_diff = vec![];
    for k in del_layers
        .keys()
        .chain(layers.keys())
        .collect::<BTreeSet<_>>()
    {
        let (a, b) = (
            del_layers.get(k).copied().unwrap_or(0),
            layers.get(k).copied().unwrap_or(0),
        );
        if a != b {
            layer_diff.push(
                json!({"D": k.0, "r": k.1, "in_box": k.2, "class": k.3, "delivery": a, "ours": b}),
            );
        }
    }
    let set_cmp = |name: &str, ours: &BTreeSet<Vec<i32>>, theirs: &BTreeSet<Vec<i32>>| -> Value {
        let only_ours: Vec<String> = ours
            .difference(theirs)
            .map(|k| import::lines::key_text(k))
            .collect();
        let only_theirs: Vec<String> = theirs
            .difference(ours)
            .map(|k| import::lines::key_text(k))
            .collect();
        json!({"class": name, "ours": ours.len(), "delivery": theirs.len(), "equal": only_ours.is_empty() && only_theirs.is_empty(),
               "only_ours": only_ours.len(), "only_delivery": only_theirs.len(),
               "only_ours_keys": only_ours.iter().take(50).collect::<Vec<_>>(),
               "only_delivery_keys": only_theirs.iter().take(50).collect::<Vec<_>>()})
    };
    let sym: BTreeSet<Vec<i32>> = del.sym_image.keys().cloned().collect();
    let d0: BTreeSet<Vec<i32>> = del.d0.keys().cloned().collect();
    let rcp: BTreeSet<Vec<i32>> = del.recipe.keys().cloned().collect();
    let empty = BTreeSet::new();
    // where do the delivery's symmetry-image keys sit in OUR walk
    let mut sym_where: BTreeMap<String, u64> = BTreeMap::new();
    for k in &sym {
        let w = if credited.contains(k) {
            "credited".to_string()
        } else if let Some(o) = open.get(k) {
            format!("open:{o}")
        } else if by_rel.values().any(|s| s.contains(k)) {
            "relation".into()
        } else if terminals.contains(k) {
            "terminal".into()
        } else {
            "rule_or_absent".into()
        };
        *sym_where.entry(w).or_default() += 1;
    }
    let cmp = json!({
        "keys": {"ours": n, "delivery": 14249517},
        "box": {"ours": n_box, "delivery": 5250168},
        "rounds": {"ours": res.summary.rounds, "delivery": 59},
        "edges_ours": res.summary.edges,
        "delivery_rhs_top": 207367985u64, "delivery_extra_edges": 517599u64,
        "layers": {"cells_delivery": del_layers.len(), "cells_ours": layers.len(), "cells_differing": layer_diff.len(), "diff": layer_diff},
        "symmetry_image": set_cmp("symmetry_image(credit)", &credited, &sym),
        "symmetry_image_keys_in_our_walk": sym_where,
        "d0": set_cmp("d0", by_rel.get("delivered:d0").unwrap_or(&empty), &d0),
        "recipe": set_cmp("recipe", by_rel.get("delivered:recipe").unwrap_or(&empty), &rcp),
        "terminals": set_cmp("terminals", &terminals, &del.terminals),
        "credited_via_relation_image": credited_via_relation,
        "open_keys": open.len(),
        "open_examples": open.iter().take(30).map(|(k, s)| format!("{} {s}", import::lines::key_text(k))).collect::<Vec<_>>(),
        "waits_for": waits,
        "rule_keys_by_record_witness": by_witness,
        "evidence_by_class": res.evidence_by_class(&set),
        "blocking": res.blocking(),
        "seconds": {"load": t_load, "check": t_check, "total": t00.elapsed().as_secs_f64()},
        "vm_hwm_mb": {"after_load": hwm_load / 1024, "after_check": hwm_check / 1024},
    });
    std::fs::write(
        out.join("comparison.json"),
        serde_json::to_string_pretty(&cmp).unwrap(),
    )
    .unwrap();
    let mut short = cmp.clone();
    for f in ["d0", "recipe", "symmetry_image", "terminals"] {
        short[f]["only_ours_keys"] = json!(
            short[f]["only_ours_keys"].as_array().map(|a| a
                .iter()
                .take(5)
                .cloned()
                .collect::<Vec<_>>())
        );
        short[f]["only_delivery_keys"] = json!(
            short[f]["only_delivery_keys"].as_array().map(|a| a
                .iter()
                .take(5)
                .cloned()
                .collect::<Vec<_>>())
        );
    }
    short["layers"]["diff"] = json!(
        short["layers"]["diff"]
            .as_array()
            .map(|a| a.iter().take(20).cloned().collect::<Vec<_>>())
    );
    short["evidence_by_class"] = Value::Null;
    println!(
        "T266W COMPARISON {}",
        serde_json::to_string(&short).unwrap()
    );
}

/// Item 6: the T266 delivery's per-key recipe steps (solves.jsonl: the serving step of each relation-served key) mapped
/// onto explicit recipes: v1.2 (one recipe per (step, explicit seed set), `domain.keys`, `relation_rule`) and v1.1 (point
/// recipes; the M1C steps are refused: v1.1 cannot state the relation rule). Writes the recipes and the per-key
/// assignment side file under the scratch directory. Line / numerator priorities: the table header's coordinate
/// priority (test data; [inferred] equal to the checker's).
#[test]
fn t266_recipe_steps_map_onto_explicit_recipes() {
    use import::recipe_steps::{RecipeForm, assignment_lines, parse_step, recipes_for_steps};
    let p = Path::new("/common/ben/sym-investigation/k7/runs/jointest9-full/deliver/solves.jsonl");
    if !p.is_file() {
        inputs::missing(format_args!("SKIP: {} missing", p.display()));
        return;
    }
    assert_eq!(
        import::lines::file_sha256(p).unwrap(),
        "7dc5ffa9c368d7939c24b2a36a5444b7c61423b90bb7b7d9f49850bf8748809e"
    );
    let mut asg: Vec<(String, Vec<i32>)> = vec![];
    let mut none = 0;
    for l in std::fs::read_to_string(p).unwrap().lines() {
        let v: Value = serde_json::from_str(l).unwrap();
        let k = import::lines::parse_key_text(v["key"].as_str().unwrap(), 20).unwrap();
        match (v["result"].as_str(), v["recipe"].as_str()) {
            (Some("served"), Some(r)) => asg.push((r.to_string(), k)),
            _ => none += 1,
        }
    }
    assert_eq!((asg.len(), none), (2117, 3));
    // the DEFAULT sequence's step names all parse
    for s in [
        "d0",
        "d1_cnone_foff_sfirst_mM1C",
        "d2_cunion_fon_sfirst_mM1C",
        "d3_cunion_fon_sfirst_mM1C",
    ] {
        parse_step(s).unwrap();
    }
    let lines: Vec<bool> = (0..20).map(|i| i < 14).collect();
    let lp: Vec<usize> = (0..14).collect();
    let np: Vec<usize> = (14..20).collect();
    let v12 =
        recipes_for_steps(&asg, &lines, &lp, &np, RecipeForm::V1_2, "T266/jointest9/").unwrap();
    let mut by_step: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
    for g in &v12 {
        let e = by_step.entry(g.step.clone()).or_default();
        e.0 += 1;
        e.1 += g.keys.len();
    }
    println!(
        "T266 steps -> v1.2 recipes: {} recipes for {} keys; per step (recipes, keys) {:?}",
        v12.len(),
        asg.len(),
        by_step
    );
    assert_eq!(v12.iter().map(|g| g.keys.len()).sum::<usize>(), 2117);
    let v11 = recipes_for_steps(&asg, &lines, &lp, &np, RecipeForm::V1_1, "T266/jointest9/");
    println!(
        "T266 steps -> v1.1: {:?}",
        v11.as_ref().err().map(|e| e.to_string())
    );
    assert!(
        v11.is_err(),
        "every T266 step carries M1C: v1.1 cannot state it"
    );
    let out = scratch().join("t266-recipes");
    std::fs::create_dir_all(out.join("recipes")).unwrap();
    for (i, g) in v12.iter().enumerate() {
        std::fs::write(
            out.join("recipes").join(format!("r{i:05}.json")),
            serde_json::to_string_pretty(&g.recipe).unwrap(),
        )
        .unwrap();
    }
    std::fs::write(out.join("assignment.jsonl"), assignment_lines(&v12)).unwrap();
}

/// Item 4: the recipe requests for the campaign's own child jobs: the delivery's 2,117 recipe-served keys as the 52
/// v1.2 recipes + assignment side file (import::recipe_steps), CHECKED against the checker's own attempt records
/// (work*/att/k_<key>__<step>.json: its explicit centre list `recipe.recenter`, depth, floor, measure and the top-key
/// support of the accepted relation), and the solve-request lines (rustred.campaign.solve-request.v1, one per (recipe,
/// key)). Also lists the keys whose step needs the proposed v1.3 fields (symmetry rows at the key first: every `s` step;
/// `d0` without an IBP seed).
#[test]
fn t266_recipe_requests_vs_checker_attempts() {
    use import::recipe_steps::{
        RecipeForm, assignment_lines, explicit_seeds, parse_step, recipes_for_steps,
    };
    if !Path::new(RUN).is_dir() {
        inputs::missing(format_args!("SKIP: {RUN} missing"));
        return;
    }
    let del = read_delivery();
    let asg: Vec<(String, Vec<i32>)> = del
        .recipe
        .iter()
        .map(|(k, (s, _))| (s.clone(), k.clone()))
        .collect();
    let lines: Vec<bool> = (0..ARITY).map(|i| i < 14).collect();
    let lp: Vec<usize> = (0..14).collect();
    let np: Vec<usize> = (14..ARITY).collect();
    let rec =
        recipes_for_steps(&asg, &lines, &lp, &np, RecipeForm::V1_2, "T266/jointest9/").unwrap();
    // every key against the checker's accepted attempt record for its step
    let (
        mut checked,
        mut centre_eq,
        mut depth_eq,
        mut floor_eq,
        mut meas_eq,
        mut top_eq,
        mut missing,
    ) = (0, 0, 0, 0, 0, 0, vec![]);
    let mut mism: Vec<String> = vec![];
    for (step, key) in &asg {
        let st = parse_step(step).unwrap();
        let name = format!(
            "k_{}__{step}.json",
            key.iter()
                .map(|x| if *x < 0 {
                    format!("m{}", -x)
                } else {
                    x.to_string()
                })
                .collect::<Vec<_>>()
                .join("_")
        );
        let f = sorted_dirs("work")
            .into_iter()
            .map(|w| w.join("att").join(&name))
            .find(|p| p.is_file());
        let Some(f) = f else {
            missing.push(name);
            continue;
        };
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        checked += 1;
        let theirs: BTreeSet<Vec<i32>> = v["recipe"]["recenter"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| serde_json::from_value(c["recenter"].clone()).unwrap())
            .collect();
        let (c, _floor, shells, _sym) = explicit_seeds(&st, key, &lines, &lp, &np);
        let ours: BTreeSet<Vec<i32>> = c.into_iter().collect();
        if ours == theirs {
            centre_eq += 1;
        } else if mism.len() < 10 {
            mism.push(format!(
                "{step} {}: ours {ours:?} checker {theirs:?}",
                import::lines::key_text(key)
            ));
        }
        depth_eq += usize::from(v["recipe"]["depth"].as_u64() == Some((shells.len() - 1) as u64));
        floor_eq += usize::from((v["recipe"]["floor"] == json!("on")) == st.floor);
        meas_eq += usize::from(v["recipe"]["meas"].as_str() == st.relation.as_deref());
        let top: BTreeSet<Vec<i32>> = v["top_rhs_keys"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|x| kt(x.as_str().unwrap()))
            .collect();
        top_eq += usize::from(top == del.recipe[key].1.iter().cloned().collect::<BTreeSet<_>>());
    }
    let mut by_step: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for g in &rec {
        let e = by_step.entry(g.step.clone()).or_default();
        e.0 += 1;
        e.1 += g.keys.len();
    }
    // write the recipes, the assignment and the solve requests
    let out = scratch().join("t266-requests");
    std::fs::create_dir_all(out.join("recipes")).unwrap();
    let mut req = String::new();
    for (i, g) in rec.iter().enumerate() {
        let p = out.join("recipes").join(format!("r{i:05}.json"));
        let txt = serde_json::to_string_pretty(&g.recipe).unwrap();
        std::fs::write(&p, &txt).unwrap();
        let sha = import::lines::file_sha256(&p).unwrap();
        for k in &g.keys {
            req.push_str(
                &json!({"schema": import::SOLVE_REQUEST_SCHEMA, "source": format!("recipe:{i:05}"),
                "recipe_path": p.display().to_string(), "recipe_sha256": sha, "engine_digest": null,
                "key": k, "reason": "in_domain_uncovered", "in_start": null, "step": g.step})
                .to_string(),
            );
            req.push('\n');
        }
    }
    std::fs::write(out.join("assignment.jsonl"), assignment_lines(&rec)).unwrap();
    std::fs::write(out.join("solve-requests.jsonl"), &req).unwrap();
    // v1.3 needs: symmetry rows at the key first (every step with `s`), d0 without an IBP seed
    let sym_first: Vec<String> = asg
        .iter()
        .filter(|(s, _)| parse_step(s).unwrap().symmetry_first)
        .map(|(_, k)| import::lines::key_text(k))
        .collect();
    let d0_keys: Vec<String> = del.d0.keys().map(|k| import::lines::key_text(k)).collect();
    std::fs::write(
        out.join("v13-symmetry-rows-first.keys.txt"),
        sym_first.join("\n") + "\n",
    )
    .unwrap();
    std::fs::write(
        out.join("v13-d0-no-ibp-seed.keys.txt"),
        d0_keys.join("\n") + "\n",
    )
    .unwrap();
    let rep = json!({"recipes": rec.len(), "keys": asg.len(), "by_step": by_step,
        "checked_against_attempts": checked, "missing_attempt_files": missing.len(),
        "centres_equal": centre_eq, "depth_equal": depth_eq, "floor_equal": floor_eq, "measure_equal": meas_eq,
        "top_support_equal_to_walk_record": top_eq, "mismatch_examples": mism,
        "relation_digests_checkable": 0,
        "why_no_digest": "the checker's attempt records carry modular relations (2^61-1, 2^31-1) with top-key support and counts, no exact coefficient text or digest",
        "v13_symmetry_rows_first_keys": sym_first.len(), "v13_d0_keys": d0_keys.len(),
        "solve_requests": asg.len()});
    std::fs::write(
        out.join("report.json"),
        serde_json::to_string_pretty(&rep).unwrap(),
    )
    .unwrap();
    println!("T266 REQUESTS {}", serde_json::to_string(&rep).unwrap());
    assert_eq!((rec.len(), asg.len()), (52, 2117));
    assert_eq!(missing.len(), 0);
}
