//! REPLAY of the checker's T178 order-B reference (k1/results/<item>/*.json, per-census-key verdicts of every
//! attempted (case, centre, floor, shells, measure)) through the ladder with a scripted engine
//! (tests/data/ladder/t178_k1_*.jsonl). Census "exception" verdicts (lower-only relation, pivot structure changed)
//! are read as the reference WORKER read them (w1: covered, "existence at every census point", e.g.
//! k1/w1/STAGES.txt:64, :72; w0 / w2: regular only, except item 14, k1/w2/STAGES.txt:33 "accepted as covered").

mod ladder_common;

use std::collections::BTreeMap;

use ladder_common::*;
use rustred_campaign_core::worklist::items::{Face, NodeParams};

/// Reference case counts and dispositions (k1 block tables B1-B6 of w0/w1/w2 STAGES.txt; masters: k1/STAGES.txt:51).
/// (item, reference cases, terminal expected, source)
const REFERENCE: &[(i64, usize, bool, &str)] = &[
    // single-line faces (R27 (1))
    (
        11,
        1,
        true,
        "w0/STAGES.txt:30 {D7}: generic unfloored + terminal D7=2",
    ),
    (
        13,
        1,
        true,
        "w1/STAGES.txt:37 {D9}: generic C (tie-break, depth 2) + terminal D9=2",
    ),
    (
        10,
        1,
        true,
        "w1/STAGES.txt:36 {D6}: generic depth 3 + terminal D6=2",
    ),
    (
        6,
        2,
        false,
        "w1/STAGES.txt:56 {D1}: generic depth 3 + point D1=2 (C, depth 3)",
    ),
    (
        7,
        2,
        false,
        "w1/STAGES.txt:57 {D2}: generic depth 3 + point D2=2",
    ),
    (
        8,
        3,
        false,
        "w2/STAGES.txt:92 {D3}: generic depth 3 + 2 points",
    ),
    (
        9,
        1,
        true,
        "w2/STAGES.txt:93 {D5}: generic depth 3 + terminal D5=2",
    ),
    (
        14,
        1,
        true,
        "w2/STAGES.txt:53 {D10}: generic depth 3 + terminal D10=2",
    ),
    (12, 1, false, "w0/STAGES.txt:29 {D8}: point by tie-break"),
    // pure numerator faces (R27 (2))
    (
        15,
        2,
        false,
        "w0/STAGES.txt:70 {D11,D12}: child n10=-1 + point",
    ),
    (
        17,
        2,
        false,
        "w0/STAGES.txt:71 {D12,D13}: child n12=-1 + point",
    ),
    (18, 1, false, "w1/STAGES.txt:58 {D12,D14}: child D14=-1"),
    (
        2,
        1,
        true,
        "w2/STAGES.txt:52 {D12}: generic + terminal D12=-1",
    ),
    // line-and-numerator faces (R27 (3), R29 (c))
    (
        21,
        2,
        false,
        "w1/STAGES.txt:96 {D1}x{D12}: combined centre + child",
    ),
    (
        47,
        3,
        false,
        "w0/STAGES.txt:73 {D10}x{D11}: numerator centre + child + point",
    ),
    (45, 1, false, "w1/STAGES.txt:95 {D9}x{D12}: LINE centre (C)"),
    (
        33,
        3,
        false,
        "w1/STAGES.txt:115 {D5}x{D12}: numerator centre + child + point",
    ),
    // two-line faces (R27 (4), R29 (a), (b))
    (
        24,
        4,
        false,
        "w1/STAGES.txt:93 {D1,D7}: union of line centres + 3 children",
    ),
    (
        28,
        1,
        false,
        "w1/STAGES.txt:94 {D3,D5}: ONE case under the full comparator (x2 C)",
    ),
    (
        39,
        3,
        false,
        "w0/STAGES.txt:121 {D6,D7}: generic + children D6=2, D6=3 (read from the rule)",
    ),
    // mixed affine child (R32)
    (
        41,
        3,
        false,
        "w2/STAGES.txt:149 {D7}x{D12}: generic d3 + child D7=2 + affine child D7+2*D12=1 (d3)",
    ),
    // corner
    (0, 0, true, "w1/STAGES.txt:34 corner: terminal"),
];

fn exception_policy(items: &[RecItem]) -> BTreeMap<i64, bool> {
    items
        .iter()
        .map(|it| (it.item, it.worker == "w1" || it.item == 14))
        .collect()
}

struct Run {
    out: BTreeMap<i64, Outcome>,
}

fn replay(descent_full: bool) -> Run {
    let items: Vec<RecItem> = read_jsonl("t178_k1_items.jsonl");
    let recs: Vec<Rec> = read_jsonl("t178_k1_attempts.jsonl");
    let params = NodeParams::new("T178", 20, 10);
    let cmp = t178_comparator();
    let mut solver = Scripted::new(params.clone(), recs, exception_policy(&items));
    let mut facts = ScriptedFacts {
        cmp: &cmp,
        n_lines: 10,
        line_priority: vec![6, 4, 9, 8, 5, 2, 1, 0, 7, 3],
    };
    let dr = DotsRankOnly { n_lines: 10 };
    let mut out = BTreeMap::new();
    for it in &items {
        let (Some(sd), Some(sn)) = (it.sd.clone(), it.sn.clone()) else {
            continue;
        };
        let face = Face::new(sd, sn);
        let (_rep, o) = if descent_full {
            run_item(
                &params,
                &mut solver,
                &cmp,
                &mut facts,
                face,
                it.census.clone(),
            )
        } else {
            run_item(
                &params,
                &mut solver,
                &dr,
                &mut facts,
                face,
                it.census.clone(),
            )
        };
        out.insert(it.item, o);
    }
    Run { out }
}

#[test]
fn t178_reference_table_r27_r29() {
    let run = replay(true);
    let masters: Vec<Vec<i32>> = {
        let m = |i: usize, v: i32| {
            let mut k = vec![1; 10];
            k.extend(vec![0; 10]);
            k[i] = v;
            k
        };
        vec![
            m(0, 1),
            m(6, 2),
            m(4, 2),
            m(9, 2),
            m(8, 2),
            m(5, 2),
            m(11, -1),
        ]
    };
    let mut all_terms = Vec::new();
    for (item, o) in &run.out {
        eprintln!(
            "item {item}: cases {} points {} terminals {:?} attempts {} rules {:?} {:?}",
            o.cases, o.point_rules, o.terminals, o.attempts, o.rules, o.attention
        );
        all_terms.extend(o.terminals.iter().cloned());
    }
    for &(item, cases, terminal, src) in REFERENCE {
        let o = &run.out[&item];
        assert!(
            o.cases <= cases,
            "item {item} ({src}): {} cases > reference {cases}: {:?}",
            o.cases,
            o.rules
        );
        assert_eq!(
            !o.terminals.is_empty(),
            terminal,
            "item {item} ({src}): terminals {:?}",
            o.terminals
        );
    }
    // every terminal proposal of the whole replay is one of the 7 IBP masters (k1/STAGES.txt:51)
    all_terms.sort();
    all_terms.dedup();
    for t in &all_terms {
        assert!(masters.contains(t), "terminal {t:?} is not an IBP master");
    }
    let total_cases: usize = run.out.values().map(|o| o.cases).sum();
    let total_points: usize = run.out.values().map(|o| o.point_rules).sum();
    let total_attempts: usize = run.out.values().map(|o| o.attempts).sum();
    eprintln!(
        "T178 replay: {} items, {total_cases} cases ({total_points} point rules), {total_attempts} attempts, {} terminal proposals {:?}",
        run.out.len(),
        all_terms.len(),
        all_terms
    );
    // the replay ends on exactly the 7 IBP masters, as the reference (R29 (e), R34)
    assert_eq!(all_terms.len(), 7);
    // specific closing settings
    let has = |item: i64, pat: &str| {
        run.out[&item]
            .rules
            .iter()
            .any(|(c, s, _)| format!("{c} {s}").contains(pat))
    };
    assert!(
        has(11, "unfl/d2"),
        "{{D7}} closes UNFLOORED at depth 2 (R26)"
    );
    assert!(has(10, "/d3"), "{{D6}} needs depth 3 (R25)");
    assert!(
        has(24, "union_lines") || has(24, "explicit(0,6)"),
        "{{D1,D7}} only by the union (R29 (a))"
    );
    assert_eq!(
        run.out[&28].cases, 1,
        "{{D3,D5}} ONE case under the full comparator (R29 (b))"
    );
    assert!(
        has(41, "eq=n6+2*n11-1"),
        "{{D7}}x{{D12}}: affine child read from the rule (R32)"
    );
    assert!(has(41, "fixed=6:2"), "{{D7}}x{{D12}}: child D7=2");
    assert!(
        has(39, "fixed=5:3"),
        "{{D6,D7}}: child at line value 3 (R39)"
    );
    // R39 / k1/w2/STAGES.txt:133: {D10}x{D12} children at D10 = 2, D10 = 3 AND D12 = -2, read from the rule
    assert!(
        has(48, "fixed=9:3") && has(48, "fixed=11:-2"),
        "{:?}",
        run.out[&48].rules
    );
    assert!(run.out[&48].terminals.is_empty());
}

#[test]
fn dots_rank_only_acceptance_needs_more_cases_planted_control() {
    // R29 (b): a dots-plus-rank comparator (never used by the library) rejects the tie-break relations
    let full = replay(true);
    let dr = replay(false);
    assert!(dr.out[&28].cases > full.out[&28].cases || !dr.out[&28].terminals.is_empty());
    assert!(
        !dr.out[&13].terminals.is_empty()
            && dr.out[&13].terminals.len() >= full.out[&13].terminals.len()
    );
}
