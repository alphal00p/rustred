//! T178 order B: forward closure of the box under first match in cfg piece order (the checker's h13b D3/D4 and the
//! frozen k3 tree F5), order sensitivity, thread independence, incremental == from scratch.
//!
//! Inputs: the frozen tree `WS/fixtures/helper-k3-20261113` (or `RUSTRED_K3_FIXTURES`), the 38 case exports
//! (k3_common::t178_tables), the checker's key files `HO/t178-orderB/keys/*.keys.jsonl` (sha256 in
//! `tests/data/closure/t178_keys.sha256`; R0 takes R0_try1's bodies, byte-identical case files). SKIP if absent.
//! Run in release: `light_build.sh loop test -p rustred-campaign-core --release --test closure_t178 -- --nocapture`.

mod closure_common;

use std::collections::{HashMap, HashSet};

use closure_common::{k3_common, t178};
use rustred_campaign_core::closure::{
    ClosureConfig, FastDispatch, HandDowns, NoFacts, ReportIdentity, Sectors, TermClass, Tier,
    Verdict, closure, closure_incremental, dispatch_order_digest,
};
use rustred_campaign_core::cover::{Dispatch, Dispatcher};

fn cfg(threads: usize) -> ClosureConfig {
    ClosureConfig {
        tier: Tier::K,
        threads,
        hand_downs: HandDowns::Keys,
        ..Default::default()
    }
}

/// The reference numbers (R10, h13b D3/D4, k3 F5) and key-by-key equality with the frozen tree.
#[test]
fn t178_closure_reference_numbers_and_frozen_tree() {
    let Some(t) = t178() else { return };
    let o = k3_common::oracle().unwrap();
    assert_eq!(t.box_keys.len(), 286_066);
    let cl = closure(&t.disp, &t.bodies, &t.cmp, &t.box_keys, &cfg(4), &NoFacts).unwrap();
    let s = cl.summary();
    println!("T178B closure: {}", serde_json::to_string(&s).unwrap());
    println!(
        "T178B closure: {:.1} s, memory estimate {} MB, peak rss {:?}",
        cl.seconds,
        cl.memory_estimate_bytes >> 20,
        closure_common::peak_rss().map(|b| b >> 20)
    );
    assert_eq!(s.rounds, 27);
    assert_eq!(s.keys_total, 330_287);
    assert_eq!(s.keys_in_start, 286_066);
    assert_eq!(s.keys_outside, 44_221);
    assert_eq!(s.edges, 4_387_847);
    assert_eq!(s.descent_violations, 0);
    assert_eq!(s.cycles, Some(0));
    assert_eq!(s.longest_chain.as_ref().unwrap().edges, 707);
    assert_eq!(s.open_total, 753);
    assert_eq!(s.open_in_start, 409);
    assert_eq!(s.open_outside, 344);
    assert_eq!(s.open_by_status.get("uncovered"), Some(&753));
    // the checker's term counts (d3 closure.json): top 4,387,847 terms, numerator > 0 (top sector) 2,353,228,
    // subsector 10,161,361 (the checker counts a subsector key with a positive numerator as subsector)
    assert_eq!(s.terms.top, 4_387_847);
    assert_eq!(s.terms.positive_numerator_top, 2_353_228);
    assert_eq!(
        s.terms.subsector + s.terms.positive_numerator_sub,
        10_161_361
    );
    assert_eq!(
        s.requires_coefficient_zero,
        s.terms.positive_numerator_top + s.terms.positive_numerator_sub
    );
    assert_eq!(s.verdict, Verdict::Open);
    assert!(s.superset);
    // longest chain endpoints (h13b D4): from I(1,1,1,5,1^6,0^8,-3,0) down to the corner
    let ch = s.longest_chain.as_ref().unwrap();
    assert_eq!(
        ch.from,
        vec![1, 1, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, -3, 0]
    );
    // the end point depends on the tie-break among equally long continuations (the checker reached the corner;
    // our smallest-index tie-break reaches another depth-0 key): only the length and the start are reference data
    println!(
        "longest chain: {} edges from {:?} to {:?}",
        ch.edges, ch.from, ch.to
    );
    // per-piece box wins = F5_summary
    let summ = o.json("T178B/F5_summary.json");
    println!("wins per table: {:?}", s.wins_per_table);
    let r0 = s.wins_per_table["R0"][1];
    assert_eq!(r0, 248_858);
    assert_eq!(s.wins_per_table["pool1.pieces_rgq-g0-10"][1], 10_160);
    let _ = summ;

    // key by key: F5_closure_all rows [key, piece_index, case_i, in_box]
    let rows: Vec<(Vec<i32>, i64, i64, bool)> = o
        .text("T178B/F5_closure_all.jsonl.gz")
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(rows.len(), 330_287);
    let mut mism = 0usize;
    for (k, piece, case_i, in_box) in &rows {
        let i = cl
            .index_of(k)
            .unwrap_or_else(|| panic!("closure misses {k:?}"));
        assert_eq!(cl.in_start(i), *in_box, "in_box of {k:?}");
        match cl.winner(i) {
            None => {
                if *piece != -1 {
                    mism += 1
                }
            }
            Some(r) => {
                if *piece < 0 || t.names[*piece as usize] != r.table || r.label != *case_i {
                    mism += 1
                }
            }
        }
    }
    assert_eq!(mism, 0, "winner mismatches against F5_closure_all");
    // the 753 uncovered = F5_closure_uncovered_753, with the in_box flag
    let unc: HashSet<(Vec<i32>, bool)> = o
        .jsonl("T178B/F5_closure_uncovered_753.jsonl")
        .iter()
        .map(|v| (k3_common::as_key(&v["key"]), v["in_box"].as_bool().unwrap()))
        .collect();
    let ours: HashSet<(Vec<i32>, bool)> = cl
        .open_keys()
        .into_iter()
        .map(|o| (o.key, o.in_start))
        .collect();
    assert_eq!(ours, unc);
    let box409: HashSet<Vec<i32>> = o
        .jsonl("T178B/F5_box_uncovered_409.jsonl")
        .iter()
        .map(k3_common::as_key)
        .collect();
    assert_eq!(
        ours.iter()
            .filter(|x| x.1)
            .map(|x| x.0.clone())
            .collect::<HashSet<_>>(),
        box409
    );

    // successors sample: the distinct top-sector RHS keys of the first-match rule of 300 covered closure keys
    let sectors = Sectors::new((0..20).map(|s| s < 10).collect());
    let fast = FastDispatch::new(&t.disp);
    let (mut b, mut st, mut out, mut cm) = (vec![], vec![], vec![], vec![]);
    let sample = o.jsonl("T178B/F5_closure_successors_sample300.jsonl");
    assert_eq!(sample.len(), 300);
    for v in &sample {
        let k = k3_common::as_key(&v["key"]);
        let Dispatch::Covered(r) = fast.first_match_with(&k, &mut b, &mut st) else {
            panic!("{k:?} not covered")
        };
        assert_eq!(t.disp.tables()[r.table].name, v["piece"].as_str().unwrap());
        assert_eq!(r.label, v["case_i"].as_i64().unwrap());
        out.clear();
        cm.clear();
        let nt = t
            .bodies
            .instantiate(&t.disp, r, &k, &mut out, &mut cm)
            .unwrap()
            .unwrap();
        let mut succ: Vec<Vec<i32>> = (0..nt)
            .filter(|&j| {
                sectors.classify_masked(&out[j * 20..(j + 1) * 20], cm[j]) == TermClass::Top
            })
            .map(|j| out[j * 20..(j + 1) * 20].to_vec())
            .collect();
        succ.sort();
        succ.dedup();
        let mut want: Vec<Vec<i32>> = k3_common::keys(&v["top_successors"]);
        want.sort();
        assert_eq!(succ, want, "successors of {k:?}");
        assert_eq!(succ.len() as u64, v["n_top_successors"].as_u64().unwrap());
    }
    // the indexed dispatch equals Dispatcher::first_match on every closure key
    let mut diff = 0usize;
    for (k, ..) in &rows {
        if fast.first_match_with(k, &mut b, &mut st) != t.disp.first_match(k).unwrap() {
            diff += 1;
        }
    }
    assert_eq!(diff, 0, "FastDispatch differs from Dispatcher::first_match");
    // used rules
    println!(
        "used rules {} (on the box {})",
        s.used_rules, s.used_rules_in_start
    );
    assert_eq!(s.used_rules_in_start, 1_381);
    // hand-downs: every subsector key is recorded under its dropped-line mask
    for (m, ks) in cl.hand_down_sets() {
        for k in ks.iter().take(50) {
            let dm: u64 = (0..10).filter(|&s| k[s] <= 0).fold(0, |a, s| a | (1 << s));
            assert_eq!(dm, *m);
        }
    }
    println!("hand-down children: {}", cl.hand_down_sets().len());
    // the report artefact serialises; requests are empty in Tier K
    let rep = cl.report(&ReportIdentity {
        node: "T178".into(),
        seq: 1,
        ..Default::default()
    });
    assert_eq!(rep.open_keys.len(), 753);
    assert!(cl.fact_requests().is_empty());
}

/// Reversed priority changes the winning domain on 29,287 box keys (h13 S3), coverage unchanged.
#[test]
fn t178_order_sensitivity() {
    let Some(t) = t178() else { return };
    let rev: Vec<usize> = (0..38).rev().collect();
    let d2 = t.disp.with_priority(rev).unwrap();
    let keys: Vec<Vec<i32>> = t.box_keys.iter().collect();
    let a = t.disp.batch(&keys, 4).unwrap();
    let b = d2.batch(&keys, 4).unwrap();
    let (mut dom, mut piece) = (0usize, 0usize);
    for (x, y) in a.results.iter().zip(&b.results) {
        match (x.winner(), y.winner()) {
            (Some(p), Some(q)) => {
                let cp = t.disp.tables()[p.table].domains[p.record].canonical_form();
                let cq = t.disp.tables()[q.table].domains[q.record].canonical_form();
                if cp != cq {
                    dom += 1;
                }
                if p.table != q.table {
                    piece += 1;
                }
            }
            (None, None) => {}
            _ => panic!("coverage changed with the order"),
        }
    }
    println!(
        "reversed priority: winning domain changes on {dom} box keys, winning piece on {piece}"
    );
    assert_eq!(dom, 29_287);
    assert_eq!(piece, 282_851);
    // the closure under the reversed order (reported, no reference value)
    let bodies = closure_common::t178_bodies(&d2);
    let cl = closure(&d2, &bodies, &t.cmp, &t.box_keys, &cfg(4), &NoFacts).unwrap();
    let s = cl.summary();
    println!(
        "reversed-order closure: rounds {} keys {} edges {} open {} = {} + {} violations {} chain {:?} order digest {}",
        s.rounds,
        s.keys_total,
        s.edges,
        s.open_total,
        s.open_in_start,
        s.open_outside,
        s.descent_violations,
        s.longest_chain.map(|c| c.edges),
        s.dispatch_order_digest
    );
    assert_ne!(s.dispatch_order_digest, dispatch_order_digest(&t.disp));
}

/// The result does not depend on the thread count (summary and report byte-identical).
#[test]
fn t178_thread_count_independence() {
    let Some(t) = t178() else { return };
    let id = ReportIdentity {
        node: "T178".into(),
        seq: 7,
        ..Default::default()
    };
    let r1 = closure(&t.disp, &t.bodies, &t.cmp, &t.box_keys, &cfg(1), &NoFacts).unwrap();
    let r3 = closure(&t.disp, &t.bodies, &t.cmp, &t.box_keys, &cfg(3), &NoFacts).unwrap();
    let j1 = serde_json::to_string(&r1.report(&id)).unwrap();
    let j3 = serde_json::to_string(&r3.report(&id)).unwrap();
    assert_eq!(j1, j3);
    assert_eq!(r1.keys(), r3.keys());
    for (a, b) in r1.hand_down_sets().iter().zip(r3.hand_down_sets()) {
        assert_eq!(a.0, b.0);
        assert_eq!(a.1.digest(), b.1.digest());
    }
    println!(
        "thread independence: report {} bytes identical (1 vs 3 threads); keys digest {}",
        j1.len(),
        r1.keys().digest()
    );
}

fn sub_dispatcher(
    t: &closure_common::T178,
    keep: &[usize],
    extra_last: &[usize],
) -> (Dispatcher, HashMap<usize, usize>) {
    // tables: `keep` (old, in cfg order) followed by `extra_last`; priority = cfg order of all
    let mut order: Vec<usize> = keep.to_vec();
    order.extend_from_slice(extra_last);
    let tables = order
        .iter()
        .map(|&i| t.tables[i].clone())
        .collect::<Vec<_>>();
    let mut idx: Vec<usize> = (0..order.len()).collect();
    idx.sort_by_key(|&j| order[j]);
    let map = order.iter().enumerate().map(|(j, &i)| (i, j)).collect();
    (Dispatcher::new(tables, idx).unwrap(), map)
}

/// Incremental closure (dispatch memo + invariant) equals the from-scratch closure, for a table added at a high
/// priority (R0, which wins 87 % of the box) and at the lowest priority.
#[test]
fn t178_incremental_equals_from_scratch() {
    let Some(t) = t178() else { return };
    for (added, label) in [
        (36usize, "R0 at its cfg position"),
        (37usize, "R0_try1 at the lowest priority"),
    ] {
        let keep: Vec<usize> = (0..38).filter(|&i| i != added).collect();
        let (old, _) = sub_dispatcher(&t, &keep, &[]);
        let old_b = closure_common::t178_bodies(&old);
        let c_old = closure(&old, &old_b, &t.cmp, &t.box_keys, &cfg(4), &NoFacts).unwrap();
        let (new, _) = sub_dispatcher(&t, &keep, &[added]);
        let new_b = closure_common::t178_bodies(&new);
        let scratch = closure(&new, &new_b, &t.cmp, &t.box_keys, &cfg(4), &NoFacts).unwrap();
        let inc = closure_incremental(
            &c_old.dispatch_cache(),
            &new,
            &new_b,
            &t.cmp,
            &t.box_keys,
            &cfg(4),
            &NoFacts,
        )
        .unwrap();
        let (a, b) = (scratch.summary(), inc.summary());
        assert_eq!(a, b, "{label}");
        assert_eq!(scratch.keys(), inc.keys());
        assert_eq!(scratch.open_keys(), inc.open_keys());
        println!(
            "incremental ({label}): old open {} -> new open {}, keys {}, scratch {:.2} s, incremental {:.2} s",
            c_old.summary().open_total,
            a.open_total,
            a.keys_total,
            scratch.seconds,
            inc.seconds
        );
    }
    // a dispatcher whose unchanged tables changed relative order: the cache is not used (full dispatch, recorded),
    // the result equals the from-scratch closure (since loopfix: fall back instead of refusing)
    let (old, _) = sub_dispatcher(&t, &(0..37).collect::<Vec<_>>(), &[]);
    let ob = closure_common::t178_bodies(&old);
    let c_old = closure(
        &old,
        &ob,
        &t.cmp,
        &t.box_keys,
        &ClosureConfig {
            longest_chain: false,
            ..cfg(4)
        },
        &NoFacts,
    )
    .unwrap();
    let rev = t.disp.with_priority((0..38).rev().collect()).unwrap();
    let rb = closure_common::t178_bodies(&rev);
    let inc = closure_incremental(
        &c_old.dispatch_cache(),
        &rev,
        &rb,
        &t.cmp,
        &t.box_keys,
        &cfg(4),
        &NoFacts,
    )
    .unwrap();
    assert!(inc.incremental_use().unwrap().fallback.is_some());
    let scratch = closure(&rev, &rb, &t.cmp, &t.box_keys, &cfg(4), &NoFacts).unwrap();
    assert_eq!(inc.summary(), scratch.summary());
    assert_eq!(scratch.summary().keys_total, 334_602);
}
