//! T1 and T2: the independent checker's reference numbers, key by key.
//!
//! These tests need the fixture directory named by the environment variable `RUSTRED_CAMPAIGN_FIXTURES` and are
//! `#[ignore]`d by default. Run:
//!
//! ```text
//! RUSTRED_CAMPAIGN_FIXTURES=/common/build-cache/$USER/p_integral_dev/implementer-scratch/fixtures \
//!   cargo test -p rustred-campaign-core --release --test cover_fixtures -- --ignored --nocapture
//! ```
//!
//! Expected layout (sha256 of every file in `tests/data/fixtures.MANIFEST.sha256`):
//!
//! ```text
//! T162/RULE-PREDICATES2440.json            handoff/astra-uncovered-domain-T162 (2,440 qualified bank rules)
//! T162/F1_uncovered_990.jsonl              k3 fixture: the 990 uncovered box-B keys
//! T162/F1_box_all.jsonl                    k3 fixture F1_box_all.jsonl.gz, gunzipped: [key, smallest covering gid | -1]
//! T162/F1_negctl_unqualified8.jsonl        k3 fixture: the 8 unqualified rules (must NOT be used)
//! T162/F1_uncovered_with_unqualified8.jsonl  k3 fixture: 969 keys left if they are (wrongly) added
//! T178B/cfg_t178_regenB.json               piece order (38 case files)
//! T178B/cases/*.jsonl                      case-domain exports of the 38 pieces
//! T178B/F5_box_uncovered_409.jsonl         k3 fixture
//! T178B/F5_box_firstmatch.jsonl            [key, piece_index, case_i] for every box key (in_box rows of k3
//!                                          F5_closure_all.jsonl.gz)
//! T178B/h13_s3_box.jsonl                   [key, wp, wi, wpR, wiR, bestk, bestkR] from h13/out/s3_box.npz
//! ```
//!
//! Reference checksums (sha256) of the sources: k3 `F1_uncovered_990.jsonl` 12296c92...0eca, `F1_box_all.jsonl.gz`
//! 16ca0dc3...c1d (uncompressed 38546432...ba66), `F1_negctl_unqualified8.jsonl` 6dba2448...1c1d,
//! `F1_uncovered_with_unqualified8.jsonl` 0d1cc5f8...ef91, `F5_box_uncovered_409.jsonl` d8355231...16a58c16,
//! `F5_closure_all.jsonl.gz` f152f277...d60b5; `RULE-PREDICATES2440.json` 0cd3d2ca...18f2.

use std::collections::{HashMap, HashSet};
use std::io::BufRead;
use std::path::PathBuf;

use rustred_campaign_core::cover::{Dispatcher, Table};
use rustred_campaign_core::domain::{CaseDomain, DomainOptions};
use rustred_campaign_core::key::{Key, NodeFrame};
use rustred_campaign_core::record::{ReadOptions, RuleRecord, read_bank_predicates, read_table_file};

fn fixtures() -> PathBuf {
    PathBuf::from(std::env::var("RUSTRED_CAMPAIGN_FIXTURES").expect("set RUSTRED_CAMPAIGN_FIXTURES (see file doc)"))
}

fn lines(p: PathBuf) -> Vec<serde_json::Value> {
    let f = std::io::BufReader::new(std::fs::File::open(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display())));
    f.lines().map(|l| serde_json::from_str(&l.unwrap()).unwrap()).collect()
}

fn as_key(v: &serde_json::Value) -> Key {
    v.as_array().unwrap().iter().map(|x| x.as_i64().unwrap() as i32).collect()
}

fn box_keys(frame: &NodeFrame, max_r: impl Fn(u32) -> u32, max_line_power: Option<i32>) -> Vec<Key> {
    let mut k = vec![];
    for d in 0..=4 {
        for r in 0..=max_r(d) {
            k.extend(frame.chart_layer(d, r, max_line_power));
        }
    }
    k.sort();
    k
}

/// T1: T162 box B, 2,440 qualified bank rules: 54,714 covered / 990 uncovered, the same keys; the winner under the
/// priority "bank id ascending" is the smallest covering id of the reference for every key; negative control with
/// the 8 unqualified rules gives 969.
#[test]
#[ignore = "needs RUSTRED_CAMPAIGN_FIXTURES (large reference files); run with --ignored"]
fn t1_t162_box_b() {
    let fx = fixtures();
    let frame = NodeFrame::lines_first(20, 9);
    let keys = box_keys(&frame, |d| if d == 0 { 1 } else { 2 }, Some(5));
    assert_eq!(keys.len(), 55_704);

    let t0 = std::time::Instant::now();
    let mut bank = read_bank_predicates(fx.join("T162/RULE-PREDICATES2440.json")).unwrap();
    bank.sort_by_key(|p| p.global_ordinal0);
    assert_eq!(bank.len(), 2440);
    let mut table = Table::new("T162-bank-qualified");
    for p in &bank {
        table.push(p.global_ordinal0, CaseDomain::from_bank_predicate(p, 20).unwrap());
    }
    let load = t0.elapsed().as_secs_f64();
    let disp = Dispatcher::new(vec![table.clone()], vec![0]).unwrap();
    let rep = disp.batch(&keys, 4);
    println!(
        "T1: load+parse {load:.2}s, {} patterns, dispatch {:.2}s: covered {} uncovered {} undecided {}",
        disp.n_patterns(),
        rep.seconds,
        rep.covered,
        rep.uncovered,
        rep.undecided
    );
    assert_eq!(rep.undecided, 0);
    assert_eq!(rep.covered, 54_714);
    assert_eq!(rep.uncovered, 990);
    let mine: HashSet<Key> =
        keys.iter().zip(&rep.results).filter(|(_, r)| r.winner().is_none()).map(|(k, _)| k.clone()).collect();
    let refu: HashSet<Key> = lines(fx.join("T162/F1_uncovered_990.jsonl")).iter().map(as_key).collect();
    assert_eq!(refu.len(), 990);
    assert_eq!(mine, refu, "uncovered set differs from the reference");
    // smallest covering gid, every key
    let first: HashMap<Key, i64> = lines(fx.join("T162/F1_box_all.jsonl"))
        .iter()
        .map(|v| (as_key(&v[0]), v[1].as_i64().unwrap()))
        .collect();
    assert_eq!(first.len(), 55_704);
    let mut mism = 0;
    for (k, r) in keys.iter().zip(&rep.results) {
        if r.winner().map_or(-1, |w| w.label) != first[k] {
            mism += 1;
        }
    }
    println!("T1: smallest-gid winner mismatches vs F1_box_all: {mism}");
    assert_eq!(mism, 0);

    // negative control: the 8 unqualified rules, domains from their records (chart + recenter bounds)
    let mut bad = Table::new("T162-unqualified8");
    for v in lines(fx.join("T162/F1_negctl_unqualified8.jsonl")) {
        let rec = RuleRecord {
            chart: v["chart"].as_str().map(str::to_string),
            fixed: v["fixed"].as_array().unwrap().iter().map(|x| x.as_i64()).collect(),
            equations: serde_json::from_value(v["equations"].clone()).unwrap(),
            exceptions: serde_json::from_value(v["exceptions_OR_of_AND"].clone()).unwrap(),
            recenter: serde_json::from_value(v["recenter"].clone()).unwrap(),
            ..Default::default()
        };
        let o = DomainOptions { recenter_bounds: true, ..Default::default() };
        bad.push(v["gid"].as_i64().unwrap(), CaseDomain::from_rule_record(&rec, &frame, o).unwrap());
    }
    assert_eq!(bad.len(), 8);
    let disp8 = Dispatcher::new(vec![table, bad], vec![0, 1]).unwrap();
    let rep8 = disp8.batch(&keys, 4);
    let mine8: HashSet<Key> =
        keys.iter().zip(&rep8.results).filter(|(_, r)| r.winner().is_none()).map(|(k, _)| k.clone()).collect();
    let ref8: HashSet<Key> = lines(fx.join("T162/F1_uncovered_with_unqualified8.jsonl")).iter().map(as_key).collect();
    println!("T1 negative control: with the 8 unqualified rules {} uncovered (wins of the 8: {})", rep8.uncovered, rep8.wins[1]);
    assert_eq!(rep8.uncovered, 969);
    assert_eq!(mine8, ref8);
}

struct T178 {
    names: Vec<String>,
    tables: Vec<Table>,
    keys: Vec<Key>,
}

fn load_t178() -> T178 {
    let fx = fixtures();
    let cfg: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(fx.join("T178B/cfg_t178_regenB.json")).unwrap()).unwrap();
    let mut names = vec![];
    let mut tables = vec![];
    let mut frame0: Option<NodeFrame> = None;
    for t in cfg["tables"].as_array().unwrap() {
        let rel = t.as_str().unwrap().trim_start_matches("cases/t178B/");
        let tf = read_table_file(fx.join("T178B/cases").join(rel), ReadOptions::default()).unwrap();
        let h = tf.header.expect("header");
        let frame =
            NodeFrame::from_mask(h.mask.as_deref().unwrap()).unwrap().with_power_shifts(h.power_shifts.clone());
        if let Some(f0) = &frame0 {
            assert_eq!(f0, &frame);
        }
        frame0 = Some(frame.clone());
        let name = rel.trim_end_matches(".jsonl").to_string();
        let mut table = Table::new(name.clone());
        for r in &tf.rules {
            assert_eq!(r.dispatch.as_deref(), Some("Partition"));
            table.push(r.i.unwrap(), CaseDomain::from_rule_record(r, &frame, DomainOptions::default()).unwrap());
        }
        names.push(name);
        tables.push(table);
    }
    assert_eq!(tables.len(), 38);
    let frame = frame0.unwrap();
    assert_eq!(frame.n_lines(), 10);
    let keys = box_keys(&frame, |d| if d == 0 { 2 } else { 3 }, None);
    T178 { names, tables, keys }
}

/// T2: T178 order B, first match over the 38 pieces in cfg order: 409 uncovered (the same keys), winner (piece,
/// record) identical to the reference for every box key; reversed priority: winner identical to h13's reversed
/// winner for every key, and the winning DOMAIN changes on 29,287 keys.
#[test]
#[ignore = "needs RUSTRED_CAMPAIGN_FIXTURES (large reference files); run with --ignored"]
fn t2_t178_order_b() {
    let fx = fixtures();
    let t0 = std::time::Instant::now();
    let T178 { names, tables, keys } = load_t178();
    let n_records: usize = tables.iter().map(Table::len).sum();
    let load = t0.elapsed().as_secs_f64();
    assert_eq!(keys.len(), 286_066);
    assert_eq!(n_records, 5_938);

    // distinct domains (normal form of the checker: records with a fixed value outside the chart dropped first)
    let mut canon: HashMap<rustred_campaign_core::domain::CanonicalDomain, usize> = HashMap::new();
    let mut dom_id: Vec<Vec<usize>> = vec![];
    for t in &tables {
        let mut ids = vec![];
        for d in &t.domains {
            let n = canon.len();
            ids.push(*canon.entry(d.canonical_form()).or_insert(n));
        }
        dom_id.push(ids);
    }
    let in_chart: HashSet<usize> = tables
        .iter()
        .zip(&dom_id)
        .flat_map(|(t, ids)| {
            t.domains.iter().zip(ids).filter(|(d, _)| {
                d.fixed.iter().enumerate().all(|(i, f)| f.is_none_or(|v| if i < 10 { v >= 1 } else { v <= 0 }))
            })
        })
        .map(|(_, id)| *id)
        .collect();
    println!("T2: {n_records} records, {} distinct domains ({} with fixed values in the chart)", canon.len(), in_chart.len());

    let prio: Vec<usize> = (0..38).collect();
    let disp = Dispatcher::new(tables, prio.clone()).unwrap();
    let rep = disp.batch(&keys, 4);
    println!(
        "T2: load {load:.2}s, {} patterns, dispatch {:.2}s: covered {} uncovered {} undecided {}",
        disp.n_patterns(),
        rep.seconds,
        rep.covered,
        rep.uncovered,
        rep.undecided
    );
    assert_eq!(rep.undecided, 0);
    assert_eq!(rep.uncovered, 409);
    let mine: HashSet<Key> =
        keys.iter().zip(&rep.results).filter(|(_, r)| r.winner().is_none()).map(|(k, _)| k.clone()).collect();
    let refu: HashSet<Key> = lines(fx.join("T178B/F5_box_uncovered_409.jsonl")).iter().map(as_key).collect();
    assert_eq!(mine, refu);
    let reff: HashMap<Key, (i64, i64)> = lines(fx.join("T178B/F5_box_firstmatch.jsonl"))
        .iter()
        .map(|v| (as_key(&v[0]), (v[1].as_i64().unwrap(), v[2].as_i64().unwrap())))
        .collect();
    assert_eq!(reff.len(), 286_066);
    let mut mism = 0;
    for (k, r) in keys.iter().zip(&rep.results) {
        let w = r.winner().map_or((-1, -1), |w| (w.table as i64, w.label));
        if w != reff[k] {
            mism += 1;
        }
    }
    println!("T2: first-match (piece, record) mismatches vs F5 reference: {mism}");
    assert_eq!(mism, 0);
    for (i, n) in names.iter().enumerate() {
        if rep.wins[i] > 0 {
            println!("  wins {n}: {}", rep.wins[i]);
        }
    }

    // reversed priority (within-piece record order kept)
    let rev: Vec<usize> = prio.iter().rev().cloned().collect();
    let disp_r = disp.with_priority(rev).unwrap();
    let rep_r = disp_r.batch(&keys, 4);
    let h13: HashMap<Key, (i64, i64)> = lines(fx.join("T178B/h13_s3_box.jsonl"))
        .iter()
        .map(|v| (as_key(&v[0]), (v[3].as_i64().unwrap(), v[4].as_i64().unwrap())))
        .collect();
    let (mut mism_r, mut dom_change, mut piece_change) = (0, 0, 0);
    for ((k, a), b) in keys.iter().zip(&rep.results).zip(&rep_r.results) {
        let wb = b.winner().map_or((-1, -1), |w| (w.table as i64, w.label));
        if wb != h13[k] {
            mism_r += 1;
        }
        if let (Some(x), Some(y)) = (a.winner(), b.winner()) {
            if dom_id[x.table][x.record] != dom_id[y.table][y.record] {
                dom_change += 1;
            }
            if x.table != y.table {
                piece_change += 1;
            }
        }
    }
    println!(
        "T2 reversed: {:.2}s, winner mismatches vs h13 reversed winner: {mism_r}; winning domain changes on {dom_change} keys, winning piece on {piece_change}",
        rep_r.seconds
    );
    assert_eq!(mism_r, 0);
    assert_eq!(dom_change, 29_287);
    assert_eq!(piece_change, 282_851);
}
