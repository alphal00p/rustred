//! Raw rr_node `table_dump.v1` tables (`F.table.jsonl`) against their case-domain exports (T178 order B).
//!
//! Needs `RUSTRED_CAMPAIGN_FIXTURES` with `T178B/raw/{R0_try1,pool1.pieces_rgq-g0-10}.F.table.jsonl` (copies of
//! handoff/sym-investigation-for-ben/t178-orderB/bodies; sha256 c0f2394f...b4b and 23d777bb...794) next to
//! `T178B/cases/` (see cover_fixtures.rs). The rgq-g0-10 table has a 20 MB rule line and one record with elided witness
//! loci: the reader must skip its RHS coefficients and the domain must report `Elided` where the record would apply.

use std::path::PathBuf;

use rustred_campaign_core::domain::{CaseDomain, DomainOptions, Verdict};
use rustred_campaign_core::key::NodeFrame;
use rustred_campaign_core::record::{ReadOptions, RhsMode, read_table_file};

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
        let frame = NodeFrame::from_mask(h.mask.as_deref().unwrap())
            .unwrap()
            .with_power_shifts(h.power_shifts.clone());
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
        let mut matched = 0;
        for r in &raw.rules {
            let c = cases
                .rules
                .iter()
                .find(|c| c.index == r.index)
                .expect("case record with the same index");
            let a = CaseDomain::from_rule_record(r, &frame, no_w)
                .unwrap()
                .canonical_form();
            let b = CaseDomain::from_rule_record(c, &frame, no_w)
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
