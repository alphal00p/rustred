//! In-tree samples of the T1/T2 references (no external data; files in `tests/data`, made once by
//! `implementer-scratch/core-domain/make_intree_samples.py` from the fixture tree whose checksums are in
//! `tests/data/fixtures.MANIFEST.sha256`).
//!
//! - T162: 66 bank rules (the smallest covering rule of 120 sampled covered box-B keys) and 60 uncovered keys. For
//!   every sampled key: its reference rule applies, no sampled rule with a smaller id applies; an uncovered key has no
//!   applying sampled rule.
//! - T178 order B: three small pieces verbatim (cfg positions 0, 1, 4) and 243 box keys with their reference
//!   first-match winner (piece, record). For every key: no record of a sampled piece BEFORE the winner's piece applies;
//!   if the winner's piece is sampled, the winner is exactly the first-match winner inside that piece.

use std::path::PathBuf;

use rustred_campaign_core::cover::{Dispatcher, Table};
use rustred_campaign_core::domain::{CaseDomain, DomainOptions};
use rustred_campaign_core::key::{Key, NodeFrame};
use rustred_campaign_core::record::{ReadOptions, read_bank_predicates, read_table_file};

fn data(p: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data").join(p)
}

fn jsonl(p: &str) -> Vec<serde_json::Value> {
    std::fs::read_to_string(data(p)).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

fn as_key(v: &serde_json::Value) -> Key {
    v.as_array().unwrap().iter().map(|x| x.as_i64().unwrap() as i32).collect()
}

#[test]
fn t162_sample_smallest_gid() {
    let mut bank = read_bank_predicates(data("t162_bank_sample.json")).unwrap();
    bank.sort_by_key(|p| p.global_ordinal0);
    let mut t = Table::new("T162-sample");
    for p in &bank {
        t.push(p.global_ordinal0, CaseDomain::from_bank_predicate(p, 20).unwrap());
    }
    let disp = Dispatcher::new(vec![t], vec![0]).unwrap();
    let rows = jsonl("t162_keys_sample.jsonl");
    let (mut cov, mut unc) = (0, 0);
    for r in &rows {
        let k = as_key(&r[0]);
        let want = r[1].as_i64().unwrap();
        let got = disp.first_match(&k).winner().map_or(-1, |w| w.label);
        assert_eq!(got, want, "key {k:?}");
        if want < 0 { unc += 1 } else { cov += 1 }
    }
    assert!(cov >= 100 && unc == 60, "{cov} {unc}");
}

#[test]
fn t178_sample_first_match() {
    let order: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(data("t178_piece_order.json")).unwrap()).unwrap();
    let names: Vec<String> = serde_json::from_value(order["piece_order"].clone()).unwrap();
    let pieces: Vec<String> = serde_json::from_value(order["sample_pieces"].clone()).unwrap();
    let mut tables = vec![];
    let mut pos = vec![];
    for p in &pieces {
        let tf = read_table_file(data(&format!("t178_pieces/{p}.jsonl")), ReadOptions::default()).unwrap();
        let h = tf.header.unwrap();
        let frame = NodeFrame::from_mask(h.mask.as_deref().unwrap()).unwrap().with_power_shifts(h.power_shifts);
        let mut t = Table::new(p.clone());
        for r in &tf.rules {
            t.push(r.i.unwrap(), CaseDomain::from_rule_record(r, &frame, DomainOptions::default()).unwrap());
        }
        tables.push(t);
        pos.push(names.iter().position(|n| n == p).unwrap() as i64);
    }
    assert_eq!(pos, vec![0, 1, 4]);
    let disp = Dispatcher::new(tables, vec![0, 1, 2]).unwrap();
    let rows = jsonl("t178_keys_sample.jsonl");
    assert_eq!(rows.len(), 243);
    let mut exact = 0;
    for r in &rows {
        let k = as_key(&r[0]);
        let (wp, wi) = (r[1].as_i64().unwrap(), r[2].as_i64().unwrap());
        let (all, und) = disp.union_cover(&k);
        assert!(und.is_empty());
        let first = all.first().map(|w| (pos[w.table], w.label));
        if wp < 0 {
            assert_eq!(first, None, "uncovered key {k:?} covered by a sampled piece");
        } else if pos.contains(&wp) {
            assert_eq!(first, Some((wp, wi)), "key {k:?}");
            exact += 1;
        } else {
            assert!(first.is_none_or(|(p, _)| p > wp), "key {k:?}: sampled piece before winner piece {wp} applies");
        }
    }
    assert_eq!(exact, 33);
}
