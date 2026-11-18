//! Economy measure on real rule records (T178 order-B R0_try1 table from RUSTRED_CAMPAIGN_FIXTURES; SKIP if absent):
//! streaming per-rule measure (the `monomials` field is used, coefficients are not parsed), node accumulation and the
//! total order.

mod inputs;
use std::io::BufRead;

use rustred_campaign_core::economy::{NodeMeasure, RuleMeasure};

#[test]
fn r0_table_measure() {
    let base = std::env::var("RUSTRED_CAMPAIGN_FIXTURES").unwrap_or_else(|_| {
        "/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/fixtures".into()
    });
    let path = format!("{base}/T178B/raw/R0_try1.F.table.jsonl");
    let Ok(f) = std::fs::File::open(&path) else {
        inputs::missing(format_args!("SKIP: {path} absent"));
        return;
    };
    let mut node = NodeMeasure::default();
    let mut half = NodeMeasure::default();
    for line in std::io::BufReader::new(f).lines() {
        let line = line.unwrap();
        if !line.contains("\"type\":\"rule\"") {
            continue;
        }
        let m = RuleMeasure::from_record_json(&line).unwrap();
        assert!(!m.monomials_estimated, "R0 records carry `monomials`");
        node.add(&m, true);
        if node.rules % 2 == 0 {
            half.add(&m, false);
        }
    }
    assert_eq!(node.rules, 800);
    assert!(node.rhs_terms > 0 && node.monomials > 0);
    assert_eq!(node.used_share(), 1.0);
    assert_eq!(half.economy_cmp(&node), std::cmp::Ordering::Less);
    eprintln!(
        "R0_try1: {} rules, {} RHS terms, {} monomials, {} bytes",
        node.rules, node.rhs_terms, node.monomials, node.bytes
    );
}
