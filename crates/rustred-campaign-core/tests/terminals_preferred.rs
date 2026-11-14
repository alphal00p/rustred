//! Preferred terminals (R41 P-4; interfaces/preferred-terminals.md): allowed terminals from the start (stripped from
//! the gap points, no ladder effort), kept by exclusion inside a rule's domain, reported; malformed entries are
//! warnings; never a reason to fail.

use std::collections::BTreeSet;

use rustred_campaign_core::repair::{Coverage, CoverageVerdict};
use rustred_campaign_core::terminals::{check_terminals, preferred_exclusions, preferred_report};
use rustred_campaign_core::worklist::items::{
    NodeParams, PreferredEntry, PreferredTerminals, items_from_closure_keys, repair_item,
};

fn k(a: i32, b: i32) -> Vec<i32> {
    vec![a, b, 1, 0, 0]
}

#[test]
fn preferred_keys_are_allowed_terminals_and_reported() {
    let mut p = NodeParams::new("N", 5, 3);
    p.preferred = PreferredTerminals {
        entries: vec![
            PreferredEntry::Key(k(2, 1)),
            PreferredEntry::Key(k(9, 9)),           // never reached: not needed
            PreferredEntry::Key(vec![1, 1]),        // wrong arity: warning only
            PreferredEntry::Sector { lines: vec![0, 4] }, // slot 4 is not a line: warning only
        ],
        count_against_bound: false,
    };
    // (a) never a gap point
    let items = items_from_closure_keys(&[k(2, 1), k(3, 1), k(1, 2)], &p).unwrap();
    assert!(items.iter().all(|it| !it.gap_points.contains(&k(2, 1))));
    let it = repair_item(&k(3, 1), &[k(2, 1)], &p).unwrap();
    assert_eq!(it.gap_points, vec![k(3, 1)]);
    // (b) kept inside a rule's domain by an exclusion
    struct All;
    impl Coverage for All {
        fn verdict(&self, _k: &[i32]) -> CoverageVerdict {
            CoverageVerdict::Covered { rule: "t#3".into() }
        }
    }
    let ex = preferred_exclusions(&[k(2, 1)], &All);
    assert_eq!(ex.len(), 1);
    assert_eq!(ex[0].reason, "keeps_terminal");
    // bound: preferred terminals reported separately
    let terms = vec![(k(2, 1), None), (k(1, 2), None)];
    assert!(check_terminals(&terms, 1, &rustred_campaign_core::repair::NoTables, &p.preferred, &p.lines).is_empty());
    // (d), (e)
    let reached: BTreeSet<Vec<i32>> = [k(2, 1), k(1, 2)].into_iter().collect();
    let r = preferred_report(&p.preferred, &p.lines, &terms, &reached, &ex, &[]);
    assert_eq!(r.used, vec![k(2, 1)]);
    assert_eq!(r.not_needed, vec![k(9, 9), vec![1, 1]]);
    assert_eq!(r.other_terminals.len(), 1);
    assert_eq!(r.warnings.len(), 2, "{:?}", r.warnings);
    assert_eq!(r.kept_by_exclusion.len(), 1);
}
