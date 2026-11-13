//! Shared helpers of the `order_*` integration tests (not a test crate itself).
#![allow(dead_code)]

use rustred_campaign_core::order::NodeComparator;
use rustred_order::{CoordinateGroups, Direction, OrderDescriptor};

/// T178 order B coordinate priority (h13b `common.py` PRIO; requirement R7).
pub const T178_PRIO: [usize; 20] = [6, 4, 9, 8, 5, 2, 1, 0, 7, 3, 11, 12, 10, 13, 15, 14, 16, 17, 18, 19];

/// The eleven comparator readings of the checker's d1.py (h13b `common.py` READINGS), in its insertion order, as
/// descriptors. They are exact on TOP-SECTOR keys (every line active, every numerator inactive):
/// (D + r, r, then the slots in `order`, lines with sign `dl`, numerators with sign `dn`; +1 = larger excess lower).
pub fn t178_readings() -> Vec<(&'static str, NodeComparator)> {
    let prio = T178_PRIO.to_vec();
    let rev: Vec<usize> = prio.iter().rev().copied().collect();
    let id: Vec<usize> = (0..20).collect();
    let mut invord: Vec<usize> = (0..20).collect();
    invord.sort_by_key(|&s| T178_PRIO[s]);
    let nums_first: Vec<usize> = prio[10..].iter().chain(&prio[..10]).copied().collect();
    let specs: Vec<(&'static str, Vec<usize>, i32, i32)> = vec![
        ("M1_prio_larger_lower", prio.clone(), 1, 1),
        ("M2_prio_larger_higher", prio.clone(), -1, -1),
        ("M3_revprio_larger_lower", rev.clone(), 1, 1),
        ("M4_revprio_larger_higher", rev, -1, -1),
        ("M5_identity_larger_lower", id.clone(), 1, 1),
        ("M6_identity_larger_higher", id, -1, -1),
        ("M7_prio_lines_lower_nums_higher", prio.clone(), 1, -1),
        ("M8_prio_lines_higher_nums_lower", prio, -1, 1),
        ("M9_invperm_larger_lower", invord.clone(), 1, 1),
        ("M10_invperm_larger_higher", invord, -1, -1),
        ("M11_nums_first_prio_larger_lower", nums_first, 1, 1),
    ];
    specs
        .into_iter()
        .map(|(name, order, dl, dn)| {
            let dir = |s: i32| if s > 0 { Direction::Descending } else { Direction::Ascending };
            let mut d = OrderDescriptor::spired_uncut(20);
            // every order list is a block of the 10 lines and a block of the 10 numerators
            d.coordinate_groups = if order[0] < 10 { CoordinateGroups::ActiveFirst } else { CoordinateGroups::InactiveFirst };
            d.coordinate_priority = order;
            d.active_direction = dir(dl);
            d.inactive_direction = dir(dn);
            (name, NodeComparator::from_descriptor(d).unwrap())
        })
        .collect()
}

/// The checker's O2 fallback order of T162 (z27 `order.py` rankO2) as a descriptor; exact on top-sector keys.
pub fn t162_o2() -> NodeComparator {
    let mut d = OrderDescriptor::spired_uncut(20);
    d.coordinate_groups = CoordinateGroups::InactiveFirst;
    d.coordinate_priority = vec![9, 10, 11, 12, 14, 16, 15, 13, 17, 19, 18, 0, 1, 3, 2, 4, 7, 5, 6, 8];
    d.active_direction = Direction::Ascending;
    d.inactive_direction = Direction::Ascending;
    NodeComparator::from_descriptor(d).unwrap()
}

/// One line of the pair files: `set kind expect alt target rhs`.
pub struct PairLine {
    pub set: String,
    pub kind: String,
    pub expect: bool,
    pub alt: Option<bool>,
    pub target: Vec<i32>,
    pub rhs: Vec<i32>,
}

pub fn parse_pairs(text: &str) -> Vec<PairLine> {
    let keys = |s: &str| s.split(',').map(|x| x.parse::<i32>().unwrap()).collect::<Vec<_>>();
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            assert_eq!(f.len(), 6, "{l}");
            PairLine {
                set: f[0].into(),
                kind: f[1].into(),
                expect: f[2] == "1",
                alt: match f[3] {
                    "-" => None,
                    x => Some(x == "1"),
                },
                target: keys(f[4]),
                rhs: keys(f[5]),
            }
        })
        .collect()
}

pub fn is_top(k: &[i32], lines: usize) -> bool {
    k[..lines].iter().all(|&x| x > 0) && k[lines..].iter().all(|&x| x <= 0)
}

pub fn headers() -> serde_json::Value {
    serde_json::from_str(include_str!("../data/order/headers.json")).unwrap()
}

/// The helper's wrapped T178B keys header with `denominators: 20` added (it has none; the engine requires it): how
/// the engine's selection rule reads that header (no `order_is_program`: the legacy identity order).
pub fn helper_header_with_arity() -> serde_json::Value {
    let mut h = headers()["helper_keys_t178B"].clone();
    h["header"]["denominators"] = 20.into();
    h
}
