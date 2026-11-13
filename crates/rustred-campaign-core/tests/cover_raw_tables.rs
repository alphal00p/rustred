//! Raw rr_node `table_dump.v1` tables (`F.table.jsonl`) against their case-domain exports (T178 order B).
//!
//! Needs `RUSTRED_CAMPAIGN_FIXTURES` with `T178B/raw/{R0_try1,pool1.pieces_rgq-g0-10}.F.table.jsonl` (copies of
//! handoff/sym-investigation-for-ben/t178-orderB/bodies; sha256 c0f2394f...b4b and 23d777bb...794) next to
//! `T178B/cases/` (see cover_fixtures.rs). The rgq-g0-10 table has a 20 MB rule line and one record with elided witness
//! loci: the reader must skip its RHS coefficients and the domain must report `Elided` where the record would apply.

use std::collections::{HashMap, HashSet};
use std::io::BufRead;
use std::path::PathBuf;

use rustred_campaign_core::cover::{Dispatch, Dispatcher, Table, WitnessProvenance};
use rustred_campaign_core::domain::{CaseDomain, DomainOptions, TargetMap, TargetSource, Verdict};
use rustred_campaign_core::key::{Key, NodeFrame};
use rustred_campaign_core::record::{ReadOptions, RhsMode, read_table_file};

const EXPORT_TARGETS: TargetSource = TargetSource::CanonicalByConstruction {
    attestation: "T178B case exports of engine table_dump.v1 tables",
};

fn fx() -> PathBuf {
    PathBuf::from(
        std::env::var("RUSTRED_CAMPAIGN_FIXTURES").expect("set RUSTRED_CAMPAIGN_FIXTURES"),
    )
}

#[test]
#[ignore = "needs RUSTRED_CAMPAIGN_FIXTURES (raw tables up to 28 MB); run with --ignored"]
fn raw_tables_match_case_exports() {
    for piece in ["R0_try1", "pool1.pieces_rgq-g0-10"] {
        let t0 = std::time::Instant::now();
        let raw = read_table_file(
            fx().join(format!("T178B/raw/{piece}.F.table.jsonl")),
            ReadOptions::default(),
        )
        .unwrap();
        let t_skip = t0.elapsed().as_secs_f64();
        let t1 = std::time::Instant::now();
        let raw_k = read_table_file(
            fx().join(format!("T178B/raw/{piece}.F.table.jsonl")),
            ReadOptions {
                rhs: RhsMode::KeysOnly,
            },
        )
        .unwrap();
        let t_keys = t1.elapsed().as_secs_f64();
        let cases = read_table_file(
            fx().join(format!("T178B/cases/{piece}.jsonl")),
            ReadOptions::default(),
        )
        .unwrap();
        let h = raw.header.as_ref().unwrap();
        assert_eq!(h.schema.as_deref(), Some("table_dump.v1"));
        let frame = NodeFrame::from_header(h).unwrap();
        let n_rhs: usize = raw_k
            .rules
            .iter()
            .map(|r| r.rhs_keys.as_ref().map_or(0, Vec::len))
            .sum();
        let elided = raw
            .rules
            .iter()
            .filter(|r| {
                r.witness_loci
                    .as_ref()
                    .is_some_and(|w| w.iter().any(|s| s == "elided"))
            })
            .count();
        println!(
            "{piece}: {} raw rules (read {t_skip:.2}s skipping rhs, {t_keys:.2}s with {n_rhs} rhs keys), {} case records, {elided} with elided witness loci, others {:?}",
            raw.rules.len(),
            cases.rules.len(),
            raw.others
        );
        // every raw record has a case record with the same index and the same canonical domain (pole loci aside)
        let no_w = DomainOptions {
            witness: false,
            ..Default::default()
        };
        let no_w_export = DomainOptions {
            targets: EXPORT_TARGETS,
            ..no_w
        };
        let mut matched = 0;
        for r in &raw.rules {
            let c = cases
                .rules
                .iter()
                .find(|c| c.index == r.index)
                .expect("case record with the same index");
            let da = CaseDomain::from_rule_record(r, &frame, no_w).unwrap();
            // audit A M1: every raw target is the canonical one (zero offsets), so the export's attestation holds
            assert_eq!(
                da.target,
                TargetMap::Canonical,
                "{piece} index {:?}",
                r.index
            );
            let a = da.canonical_form();
            let b = CaseDomain::from_rule_record(c, &frame, no_w_export)
                .unwrap()
                .canonical_form();
            assert_eq!(a, b, "{piece} index {:?}", r.index);
            matched += 1;
        }
        println!("{piece}: {matched} raw records have identical case domains in the export");
        // elided record: Elided wherever it would otherwise apply, at a key built from its own fixed slots
        for r in raw.rules.iter().filter(|r| {
            r.witness_loci
                .as_ref()
                .is_some_and(|w| w.iter().any(|s| s == "elided"))
        }) {
            let d = CaseDomain::from_rule_record(r, &frame, DomainOptions::default()).unwrap();
            assert!(d.witness_elided);
            let dn = CaseDomain::from_rule_record(r, &frame, no_w).unwrap();
            // search a small window for a key where the record applies without the witness check
            let mut found = None;
            'w: for a in 0..3i32 {
                for b in 0..3i32 {
                    let mut k: Vec<i32> = (0..20).map(|i| if i < 10 { 2 } else { -1 }).collect();
                    for (i, f) in r.fixed.iter().enumerate() {
                        if let Some(v) = f {
                            k[i] = *v as i32;
                        }
                    }
                    for (i, (ki, f)) in k.iter_mut().zip(&r.fixed).take(10).enumerate() {
                        if f.is_none() {
                            *ki += (a + i as i32 * b) % 3;
                        }
                    }
                    if dn.applies(&k) == Verdict::Applies {
                        found = Some(k);
                        break 'w;
                    }
                }
            }
            let k = found.expect("a key in the elided record's domain");
            assert_eq!(d.applies(&k), Verdict::Elided);
            println!(
                "{piece}: elided record index {:?} gives Elided at {k:?}",
                r.index
            );
        }
    }
}

fn lines(p: PathBuf) -> Vec<serde_json::Value> {
    let f = std::io::BufReader::new(
        std::fs::File::open(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display())),
    );
    f.lines()
        .map(|l| serde_json::from_str(&l.unwrap()).unwrap())
        .collect()
}

fn as_key(v: &serde_json::Value) -> Key {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_i64().unwrap() as i32)
        .collect()
}

/// Ruling F1 of the coordinator: a record with ELIDED witness loci is unusable. T178 order B, first match over the
/// 38 pieces in cfg order, with the two pieces for which the RAW `table_dump.v1` table is at hand (R0_try1,
/// rgq-g0-10) loaded from the raw table with the witness check on (the 36 others from their case exports, which carry
/// no witness loci: provenance `NotCarried`): 409 uncovered + 21 blocked keys, the 21 blocked keys are exactly the
/// keys whose reference winner is the elided record (rgq-g0-10 index 125), the blocking rule is named, and every
/// other key has the reference winner.
#[test]
#[ignore = "needs RUSTRED_CAMPAIGN_FIXTURES (raw tables, case exports, F5_box_firstmatch); run with --ignored"]
fn f1_raw_tables_witness_check_409_uncovered_21_blocked() {
    let cfg: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(fx().join("T178B/cfg_t178_regenB.json")).unwrap(),
    )
    .unwrap();
    let mut tables = vec![];
    let mut elided_ref: Option<(usize, i64)> = None;
    for (ti, t) in cfg["tables"].as_array().unwrap().iter().enumerate() {
        let rel = t.as_str().unwrap().trim_start_matches("cases/t178B/");
        let name = rel.trim_end_matches(".jsonl").to_string();
        let cases =
            read_table_file(fx().join("T178B/cases").join(rel), ReadOptions::default()).unwrap();
        let raw_path = fx().join(format!("T178B/raw/{name}.F.table.jsonl"));
        let table = if raw_path.is_file() {
            let raw = read_table_file(&raw_path, ReadOptions::default()).unwrap();
            let mut tb = Table::from_table_file(
                name.clone(),
                raw_path.display().to_string(),
                &raw,
                DomainOptions::default(),
            )
            .unwrap();
            assert_eq!(tb.provenance.witness, WitnessProvenance::Checked);
            // labels: the export's record number `i` of the record with the same `index`
            let by_index: HashMap<i64, i64> = cases
                .rules
                .iter()
                .map(|c| (c.index.unwrap(), c.i.unwrap()))
                .collect();
            for (k, r) in raw.rules.iter().enumerate() {
                tb.labels[k] = by_index[&r.index.unwrap()];
                if tb.domains[k].witness_elided {
                    assert!(elided_ref.is_none());
                    elided_ref = Some((ti, tb.labels[k]));
                }
            }
            tb
        } else {
            let tb = Table::from_table_file(
                name.clone(),
                rel,
                &cases,
                DomainOptions {
                    targets: EXPORT_TARGETS,
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(tb.provenance.witness, WitnessProvenance::NotCarried);
            tb
        };
        tables.push(table);
    }
    assert_eq!(tables.len(), 38);
    let elided_ref = elided_ref.expect("the elided record");
    let frame = NodeFrame::without_header_lines_first(20, 10);
    let mut keys = vec![];
    for d in 0..=4u32 {
        for r in 0..=(if d == 0 { 2 } else { 3 }) {
            keys.extend(frame.chart_layer(d, r, None));
        }
    }
    assert_eq!(keys.len(), 286_066);
    let disp = Dispatcher::new(tables, (0..38).collect()).unwrap();
    let rep = disp.batch(&keys, 4).unwrap();
    println!(
        "F1: covered {} uncovered {} blocked {} undecided {} (elided record: piece {} i {})",
        rep.covered, rep.uncovered, rep.blocked, rep.undecided, elided_ref.0, elided_ref.1
    );
    assert_eq!((rep.uncovered, rep.blocked, rep.undecided), (409, 21, 0));
    let reff: HashMap<Key, (i64, i64)> = lines(fx().join("T178B/F5_box_firstmatch.jsonl"))
        .iter()
        .map(|v| {
            (
                as_key(&v[0]),
                (v[1].as_i64().unwrap(), v[2].as_i64().unwrap()),
            )
        })
        .collect();
    let mut blocked_keys = HashSet::new();
    for (k, r) in keys.iter().zip(&rep.results) {
        match r {
            Dispatch::Blocked { at, reason } => {
                assert_eq!(*reason, Verdict::Elided);
                assert_eq!((at.table, at.label), elided_ref);
                assert_eq!(reff[k], (elided_ref.0 as i64, elided_ref.1));
                blocked_keys.insert(k.clone());
            }
            _ => {
                let w = r.winner().map_or((-1, -1), |w| (w.table as i64, w.label));
                assert_eq!(w, reff[k], "{k:?}");
            }
        }
    }
    let n_ref = reff
        .values()
        .filter(|w| **w == (elided_ref.0 as i64, elided_ref.1))
        .count();
    assert_eq!((blocked_keys.len(), n_ref), (21, 21));
}
