//! The T266 comparator of R19 built from the real table HEADER DATA (no order is coded): the case-domain export's
//! header (`node_order` phys_rev, `coordinate_priority` null -> the engine's legacy order in the presented frame),
//! checked on a SAMPLE of the closure's edges: the 814 edges of the longest descending chain of the checker's
//! keys-only closure (k2 `out/close/chain.txt`: each key is a top-sector RHS key of the previous key's first-match
//! rule). The full 201,470,598-edge check belongs to the closure lane. Data in `tests/data/box/t266/` (sha256 in its
//! MANIFEST); the 85 GB joined table is not read.
//! R19 reading: support; dots + rank; rank; then slots 0..13 (lines) and 14..19 (numerators) in the presented
//! frame's own order, larger excess on the first differing slot = lower.

use std::cmp::Ordering;

use rustred_campaign_core::key::Key;
use rustred_campaign_core::order::{NodeComparator, Selection};

const CASES_HEADER: &str = include_str!("data/box/t266/cases_header.json");
const FKEYS_HEADER: &str = include_str!("data/box/t266/fkeys_header.json");
const CHAIN: &str = include_str!("data/box/t266/k2_longest_chain.txt");

fn chain() -> Vec<Key> {
    CHAIN
        .lines()
        .filter(|l| l.starts_with('['))
        .map(|l| {
            let a = &l[..=l.find(']').unwrap()];
            serde_json::from_str::<Vec<i32>>(a).unwrap()
        })
        .collect()
}

/// R19 in words, written out independently (test oracle, not library code): rank tuple, smaller = lower.
fn r19_rank(k: &[i32]) -> (usize, Vec<bool>, i64, i64, Vec<i64>) {
    let active: Vec<bool> = k.iter().map(|&x| x > 0).collect();
    let n_active = active.iter().filter(|&&a| a).count();
    let exc: Vec<i64> = k
        .iter()
        .map(|&x| {
            if x > 0 {
                i64::from(x) - 1
            } else {
                -i64::from(x)
            }
        })
        .collect();
    let total: i64 = exc.iter().sum();
    let inactive: i64 = exc
        .iter()
        .zip(&active)
        .filter(|(_, a)| !**a)
        .map(|(e, _)| e)
        .sum();
    // support bits compared so that MORE active slots in earlier positions is higher (as rankC)
    (
        n_active,
        active,
        total,
        inactive,
        exc.iter().map(|e| -e).collect(),
    )
}

fn r19_cmp(a: &[i32], b: &[i32]) -> Ordering {
    r19_rank(a).cmp(&r19_rank(b))
}

fn cases_comparator() -> NodeComparator {
    let h: serde_json::Value = serde_json::from_str(CASES_HEADER).unwrap();
    NodeComparator::from_header(&h).unwrap()
}

#[test]
fn header_selection_is_the_node_order() {
    let c = cases_comparator();
    assert_eq!(c.arity(), 20);
    assert_eq!(c.selection(), &Selection::NodeOrder);
    // the header's slot_permutation is recorded as the presentation; the keys at hand are already presented
    assert_eq!(
        c.presentation().new_to_old(),
        &[
            13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 14, 15, 16, 17, 18, 19
        ]
    );
    println!(
        "T266 comparator from the cases header: {} ({:?})",
        c.name(),
        c.selection()
    );
}

#[test]
fn chain_edges_all_descend_under_the_header_comparator() {
    let c = cases_comparator();
    let ks = chain();
    assert_eq!(ks.len(), 815);
    let edges: Vec<(&Key, &Key)> = ks.windows(2).map(|w| (&w[0], &w[1])).collect();
    assert_eq!(edges.len(), 814);
    let not_lower = edges.iter().filter(|(t, r)| !c.lower(r, t)).count();
    let swapped_lower = edges.iter().filter(|(t, r)| c.lower(t, r)).count();
    let r19_not_lower = edges
        .iter()
        .filter(|(t, r)| r19_cmp(r, t) != Ordering::Less)
        .count();
    println!(
        "T266 sample: 814 chain edges; not lower: header comparator {not_lower}, R19 oracle {r19_not_lower}; swapped pairs lower: {swapped_lower}"
    );
    assert_eq!(not_lower, 0);
    assert_eq!(r19_not_lower, 0);
    assert_eq!(swapped_lower, 0);
    // the header comparator and the R19 oracle agree on every pair of chain keys (815 x 815)
    let mut disagree = 0;
    for a in &ks {
        for b in &ks {
            if c.cmp(a, b) != r19_cmp(a, b) {
                disagree += 1;
            }
        }
    }
    println!(
        "header comparator vs R19 oracle: {disagree} disagreements on {} ordered pairs",
        ks.len() * ks.len()
    );
    assert_eq!(disagree, 0);
}

#[test]
fn negative_controls_other_readings_fail_on_the_sample() {
    let ks = chain();
    let edges: Vec<(&Key, &Key)> = ks.windows(2).map(|w| (&w[0], &w[1])).collect();
    // the slot_permutation order as tie order (one of the ten failing readings of k2)
    let perm: Vec<usize> = (0..14).rev().chain(14..20).collect();
    let p = NodeComparator::legacy(20, Some(perm)).unwrap();
    let fail_perm = edges.iter().filter(|(t, r)| !p.lower(r, t)).count();
    // reversed numerator order
    let rn: Vec<usize> = (0..14).chain((14..20).rev()).collect();
    let q = NodeComparator::legacy(20, Some(rn)).unwrap();
    let fail_rn = edges.iter().filter(|(t, r)| !q.lower(r, t)).count();
    println!(
        "negative controls on 814 edges: slot_permutation ties {fail_perm} not lower; reversed numerators {fail_rn}"
    );
    assert!(fail_perm > 0, "the control must be able to fail");
    assert!(fail_rn > 0, "the control must be able to fail");
}

#[test]
fn keys_header_without_denominators_is_refused_and_with_them_is_the_identity_order() {
    let h: serde_json::Value = serde_json::from_str(FKEYS_HEADER).unwrap();
    assert!(
        NodeComparator::from_header(&h).is_err(),
        "no denominators: refused as the engine"
    );
    let mut h2 = h.clone();
    h2["header"]["denominators"] = serde_json::json!(20);
    let c = NodeComparator::from_header(&h2).unwrap();
    assert_eq!(c.selection(), &Selection::LegacyNoProgram);
    let ks = chain();
    let not_lower = ks.windows(2).filter(|w| !c.lower(&w[1], &w[0])).count();
    assert_eq!(not_lower, 0);
}

/// Frame of 14 lines + 6 numerators (arity 20): hand-made keys for each level of the R19 reading.
#[test]
fn fourteen_line_frame_units() {
    let c = cases_comparator();
    let top = |d: &[(usize, i32)]| {
        let mut k = vec![1i32; 14];
        k.extend([0; 6]);
        for &(s, v) in d {
            k[s] = v;
        }
        k
    };
    let corner = top(&[]);
    // a subsector key (line 3 removed) is lower than every top-sector key, even with many dots
    let sub = {
        let mut k = top(&[(0, 6), (1, 6)]);
        k[3] = 0;
        k
    };
    assert!(c.lower(&sub, &corner));
    // larger D + r is higher
    assert!(c.lower(&top(&[(0, 2)]), &top(&[(0, 2), (1, 2)])));
    assert!(c.lower(&top(&[(0, 3)]), &top(&[(14, -1), (15, -1), (16, -1)])));
    // same D + r: smaller r is lower
    assert!(c.lower(&top(&[(0, 2)]), &top(&[(19, -1)])));
    // same D, r: first differing slot, larger excess lower (slot 0 before slot 13; lines before numerators)
    assert!(c.lower(&top(&[(0, 2)]), &top(&[(13, 2)])));
    assert!(c.lower(&top(&[(14, -1)]), &top(&[(19, -1)])));
    assert!(c.lower(&top(&[(0, 3), (1, 1)]), &top(&[(0, 2), (1, 2)])));
    // a numerator > 0 counts as an active slot: higher than the chart key of the same support otherwise
    let pos = top(&[(14, 1)]);
    assert!(c.lower(&corner, &pos));
    assert_eq!(c.cmp(&corner, &corner), Ordering::Equal);
}
