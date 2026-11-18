//! Shared loaders of the partition tests (lane I-PART). Fixture directory: `RUSTRED_CAMPAIGN_FIXTURES`, else the
//! frozen copy `/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/fixtures` if it exists, else the
//! tests print SKIP and pass. Layout: see `tests/cover_fixtures.rs`.
#![allow(dead_code)]

use std::collections::HashMap;
use std::io::BufRead;
use std::path::PathBuf;

use rustred_campaign_core::cover::{Dispatcher, Table};
use rustred_campaign_core::domain::{DomainOptions, TargetSource};
use rustred_campaign_core::key::{Key, NodeFrame};
use rustred_campaign_core::record::{ReadOptions, read_bank_predicates, read_table_file};

pub const EXPORT_TARGETS: TargetSource = TargetSource::CanonicalByConstruction {
    attestation: "T178B case exports of engine table_dump.v1 tables",
};

pub fn fx() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("RUSTRED_CAMPAIGN_FIXTURES") {
        return Some(PathBuf::from(p));
    }
    let d =
        PathBuf::from("/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/fixtures");
    if d.is_dir() {
        Some(d)
    } else {
        crate::inputs::missing(format_args!(
            "SKIP: no fixture directory (set RUSTRED_CAMPAIGN_FIXTURES)"
        ));
        None
    }
}

pub fn k3() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("RUSTRED_K3_FIXTURES") {
        return Some(PathBuf::from(p));
    }
    let d = PathBuf::from(
        "/shared/vhirschi/t3code/vakint_dev/p_integral_dev/fixtures/helper-k3-20261113",
    );
    d.is_dir().then_some(d)
}

pub fn lines(p: PathBuf) -> Vec<serde_json::Value> {
    let f = std::io::BufReader::new(
        std::fs::File::open(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display())),
    );
    f.lines()
        .map(|l| serde_json::from_str(&l.unwrap()).unwrap())
        .collect()
}

pub fn as_key(v: &serde_json::Value) -> Key {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_i64().unwrap() as i32)
        .collect()
}

pub fn box_keys(
    frame: &NodeFrame,
    max_r: impl Fn(u32) -> u32,
    max_line_power: Option<i32>,
) -> Vec<Key> {
    let mut k = vec![];
    for d in 0..=4 {
        for r in 0..=max_r(d) {
            k.extend(frame.chart_layer(d, r, max_line_power));
        }
    }
    k.sort();
    k
}

pub fn t178_frame() -> NodeFrame {
    NodeFrame::without_header_lines_first(20, 10)
}
pub fn t162_frame() -> NodeFrame {
    NodeFrame::without_header_lines_first(20, 9)
}

pub fn t178_box() -> Vec<Key> {
    let k = box_keys(&t178_frame(), |d| if d == 0 { 2 } else { 3 }, None);
    assert_eq!(k.len(), 286_066);
    k
}
pub fn t162_box() -> Vec<Key> {
    let k = box_keys(&t162_frame(), |d| if d == 0 { 1 } else { 2 }, Some(5));
    assert_eq!(k.len(), 55_704);
    k
}

/// T178 order B: the 38 pieces in cfg order. `raw_where_available`: R0_try1 and rgq-g0-10 from their RAW tables
/// with the witness check on (the elided record of rgq-g0-10 carries `witness_elided`), the others from their case
/// exports. Returns (tables, (table index, record index) of the elided record if loaded raw).
pub fn t178_tables(
    fx: &std::path::Path,
    raw_where_available: bool,
) -> (Vec<Table>, Option<(usize, usize)>) {
    let cfg: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(fx.join("T178B/cfg_t178_regenB.json")).unwrap(),
    )
    .unwrap();
    let mut tables = vec![];
    let mut elided = None;
    for (ti, t) in cfg["tables"].as_array().unwrap().iter().enumerate() {
        let rel = t.as_str().unwrap().trim_start_matches("cases/t178B/");
        let name = rel.trim_end_matches(".jsonl").to_string();
        let cases =
            read_table_file(fx.join("T178B/cases").join(rel), ReadOptions::default()).unwrap();
        let raw_path = fx.join(format!("T178B/raw/{name}.F.table.jsonl"));
        let table = if raw_where_available && raw_path.is_file() {
            let raw = read_table_file(&raw_path, ReadOptions::default()).unwrap();
            let mut tb = Table::from_table_file(
                name.clone(),
                raw_path.display().to_string(),
                &raw,
                DomainOptions::default(),
            )
            .unwrap();
            let by_index: HashMap<i64, i64> = cases
                .rules
                .iter()
                .map(|c| (c.index.unwrap(), c.i.unwrap()))
                .collect();
            for (k, r) in raw.rules.iter().enumerate() {
                tb.labels[k] = by_index[&r.index.unwrap()];
                if tb.domains[k].witness_elided {
                    assert!(elided.is_none());
                    elided = Some((ti, k));
                }
            }
            tb
        } else {
            Table::from_table_file(
                name.clone(),
                rel,
                &cases,
                DomainOptions {
                    targets: EXPORT_TARGETS,
                    // legacy reference numbers: exports carry no witness loci (re-audit RA-MAJOR-1, lane corefix2)
                    witness: false,
                    ..Default::default()
                },
            )
            .unwrap()
        };
        tables.push(table);
    }
    assert_eq!(tables.len(), 38);
    (tables, elided)
}

pub fn t178_dispatcher(fx: &std::path::Path, raw: bool) -> (Dispatcher, Option<(usize, usize)>) {
    let (t, e) = t178_tables(fx, raw);
    (Dispatcher::new(t, (0..38).collect()).unwrap(), e)
}

/// T162: the 2,440 qualified bank rules (bank id ascending).
pub fn t162_dispatcher(fx: &std::path::Path) -> Dispatcher {
    let mut bank = read_bank_predicates(fx.join("T162/RULE-PREDICATES2440.json")).unwrap();
    bank.sort_by_key(|p| p.global_ordinal0);
    assert_eq!(bank.len(), 2440);
    let table = Table::from_bank(
        "T162-bank-qualified",
        "T162/RULE-PREDICATES2440.json",
        &bank,
        20,
        TargetSource::CanonicalByConstruction {
            attestation: "T162 bank: no target field, presented slots, no recenter or permutation",
        },
    )
    .unwrap();
    Dispatcher::new(vec![table], vec![0]).unwrap()
}

pub fn face_str(f: &(Vec<usize>, Vec<usize>)) -> String {
    format!("{:?}x{:?}", f.0, f.1)
}
