//! core-order: sampled reference key pairs (in tree, `tests/data/order/order_pairs.txt`, 2,200 lines) from the
//! checker's fixtures (k3 F3 T162, F5 T178B) and from the regenerated O1/O2 instantiations. Every pair carries the
//! checker's verdict; the comparator selected from the node's table header must reproduce it.

mod order_common;

use order_common::*;
use rustred_campaign_core::order::NodeComparator;

#[test]
fn order_sampled_pairs_match_checker_verdicts() {
    let pairs = parse_pairs(include_str!("data/order/order_pairs.txt"));
    assert_eq!(pairs.len(), 2200);
    let h = headers();
    let t162 = NodeComparator::from_header(&h["t162_bank"]).unwrap();
    let t178 = NodeComparator::from_header(&h["t178B_table"]).unwrap();
    let t178_identity = NodeComparator::from_header(&helper_header_with_arity()).unwrap();
    let o2 = t162_o2();
    let (mut n162, mut n178, mut lower, mut controls) = (0, 0, 0, 0);
    let (mut alt_checked, mut alt_disagree_with_node) = (0, 0);
    for p in &pairs {
        let (c, lines) = match p.set.as_str() {
            "T162" => {
                n162 += 1;
                (&t162, 9)
            }
            "T178B" => {
                n178 += 1;
                (&t178, 10)
            }
            other => panic!("set {other}"),
        };
        assert_eq!(c.lower(&p.rhs, &p.target), p.expect, "{} {} {:?} -> {:?}", p.set, p.kind, p.target, p.rhs);
        lower += p.expect as usize;
        if p.kind.starts_with("negctl") {
            controls += 1;
            assert!(!p.expect);
        }
        // the alternative reading: T178B identity priority (the helper keys header read by the engine rule),
        // T162 O2 (exact on top-sector pairs only)
        if let Some(alt) = p.alt {
            let alt_c = if p.set == "T178B" { &t178_identity } else { &o2 };
            if p.set == "T178B" || (is_top(&p.target, lines) && is_top(&p.rhs, lines)) {
                alt_checked += 1;
                assert_eq!(alt_c.lower(&p.rhs, &p.target), alt, "alt {} {} {:?} -> {:?}", p.set, p.kind, p.target, p.rhs);
                alt_disagree_with_node += (alt != p.expect) as usize;
            }
        }
        // swapped arguments: never both lower
        assert!(!(c.lower(&p.rhs, &p.target) && c.lower(&p.target, &p.rhs)));
    }
    assert_eq!((n162, n178), (1130, 1070));
    assert_eq!(controls, 430 + 270);
    assert_eq!(lower, 2200 - 700);
    assert!(alt_checked > 1000);
    // the 200 T178B priority discriminators are lower under order B and NOT under the identity priority
    assert!(alt_disagree_with_node >= 200, "{alt_disagree_with_node}");
}

/// Negative control (O3 i, ii) on the sample: an altered direction or priority fails on pairs the node accepts.
#[test]
fn order_sampled_pairs_negative_controls() {
    let pairs = parse_pairs(include_str!("data/order/order_pairs.txt"));
    let readings = t178_readings();
    let mut fails = vec![0usize; readings.len()];
    let mut swapped_lower = 0;
    for p in pairs.iter().filter(|p| p.set == "T178B" && p.expect) {
        for (i, (_, c)) in readings.iter().enumerate() {
            fails[i] += !c.lower(&p.rhs, &p.target) as usize;
        }
        swapped_lower += readings[0].1.lower(&p.target, &p.rhs) as usize;
    }
    assert_eq!(fails[0], 0, "M1 = order B");
    assert!(fails[1..].iter().all(|&f| f > 0), "{fails:?}");
    assert_eq!(swapped_lower, 0);
}
