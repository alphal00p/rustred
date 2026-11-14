//! Winner diff on T178 order B (cfg order vs reversed piece order) over the ENUMERATED box: the winning domain
//! changes on 29,287 keys, the winning piece on 282,851 (the numbers of `cover_fixtures::t2_t178_order_b`, = h13).
//! Needs `RUSTRED_CAMPAIGN_FIXTURES` (layout in `tests/cover_fixtures.rs`).

mod box_common;

use std::collections::HashMap;
use std::path::PathBuf;

use box_common::*;
use rustred_campaign_core::boxdef::winner::{StreamOptions, WinnerState, rule_id, winner_diff};
use rustred_campaign_core::cover::{Dispatcher, Table};
use rustred_campaign_core::domain::{CanonicalDomain, DomainOptions, TargetSource};
use rustred_campaign_core::record::{ReadOptions, read_table_file};

fn t178_tables() -> Vec<Table> {
    let fx = PathBuf::from(std::env::var("RUSTRED_CAMPAIGN_FIXTURES").expect("RUSTRED_CAMPAIGN_FIXTURES"));
    let cfg: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(fx.join("T178B/cfg_t178_regenB.json")).unwrap(),
    )
    .unwrap();
    cfg["tables"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            let rel = t.as_str().unwrap().trim_start_matches("cases/t178B/");
            let tf = read_table_file(fx.join("T178B/cases").join(rel), ReadOptions::default()).unwrap();
            Table::from_table_file(
                rel.trim_end_matches(".jsonl"),
                rel,
                &tf,
                DomainOptions {
                    targets: TargetSource::CanonicalByConstruction {
                        attestation: "T178B case exports of engine table_dump.v1 tables",
                    },
                    ..Default::default()
                },
            )
            .unwrap()
        })
        .collect()
}

#[test]
#[ignore = "needs RUSTRED_CAMPAIGN_FIXTURES; run with --release -- --ignored"]
fn t178_cfg_vs_reversed_winner_diff() {
    let b = rstar_union(&lines_first(20, 10));
    let tables = t178_tables();
    let n = tables.len();
    assert_eq!(n, 38);
    let mut canon: HashMap<CanonicalDomain, usize> = HashMap::new();
    let dom_id: Vec<Vec<usize>> = tables
        .iter()
        .map(|t| {
            t.domains
                .iter()
                .map(|d| {
                    let k = canon.len();
                    *canon.entry(d.canonical_form()).or_insert(k)
                })
                .collect()
        })
        .collect();
    let old = Dispatcher::new(tables, (0..n).collect()).unwrap();
    let new = old.with_priority((0..n).rev().collect()).unwrap();
    let o4 = StreamOptions { threads: 4, chunk: 8192 };
    let t0 = std::time::Instant::now();
    let dom = winner_diff(&old, &new, b.iter(), |_d: &Dispatcher, r| dom_id[r.table][r.record], &o4, true)
        .unwrap();
    let t_dom = t0.elapsed().as_secs_f64();
    let piece = winner_diff(
        &old,
        &new,
        b.iter(),
        |d: &Dispatcher, r| d.tables()[r.table].name.clone(),
        &o4,
        false,
    )
    .unwrap();
    let rule = winner_diff(&old, &new, b.iter(), rule_id, &o4, false).unwrap();
    println!(
        "T178 cfg vs reversed over the enumerated box ({} keys): winning domain changes on {} keys ({} groups, {t_dom:.2}s), winning piece on {} ({} groups), winning record on {}",
        dom.keys,
        dom.changed,
        dom.groups.len(),
        piece.changed,
        piece.groups.len(),
        rule.changed
    );
    assert_eq!(dom.keys, 286_066);
    assert_eq!(dom.changed, 29_287);
    assert_eq!(piece.changed, 282_851);
    assert!(dom.groups.keys().all(|(x, y)| matches!((x, y), (WinnerState::Covered(_), WinnerState::Covered(_)))));
    let kept: u64 = dom.groups.values().map(|g| g.keys.len() as u64).sum();
    assert_eq!(kept, 29_287);
    // thread-count independence on the full set
    let dom1 = winner_diff(
        &old,
        &new,
        b.iter(),
        |_d: &Dispatcher, r| dom_id[r.table][r.record],
        &StreamOptions { threads: 1, chunk: 8192 },
        true,
    )
    .unwrap();
    assert_eq!(dom1, dom);
    let largest = dom.groups.iter().max_by_key(|(_, g)| g.count).unwrap();
    println!("largest group: {:?} -> {:?}: {} keys", largest.0.0, largest.0.1, largest.1.count);
}
