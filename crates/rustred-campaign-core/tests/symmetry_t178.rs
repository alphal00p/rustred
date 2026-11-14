//! T178 order B symmetry credits under the ADOPTED condition (h13b D2; COORDINATOR_REQUIREMENTS R9 (b)):
//! of the 189 numerator-free box gaps 170 have a covered image and 75 are credited (box 409 -> 334); the REJECTED
//! condition "image lower than key" (implemented here in the test only) credits 42 (h13 S2). Also the checker's
//! envelope / loci point lists (716 -> 685 -> 310, 416 -> 216) when `h13/out/s1_points.json` is readable.

mod closure_common;

use std::collections::HashSet;

use closure_common::{k3_common, t178, t178_group};
use rustred_campaign_core::closure::{ClosureConfig, HandDowns, NoFacts, Sectors, Tier, closure};
use rustred_campaign_core::cover::Dispatch;
use rustred_campaign_core::symmetry::{CreditChecker, CreditFrame, CreditPolicy, Refusal};

const S1: &str = "/common/ben/sym-investigation/h13/out/s1_points.json";

fn numerator_free(k: &[i32]) -> bool {
    k[10..].iter().all(|&x| x == 0)
}

#[test]
fn t178_safe_symmetry_list() {
    let Some(t) = t178() else { return };
    let o = k3_common::oracle().unwrap();
    let box409: Vec<Vec<i32>> = o.jsonl("T178B/F5_box_uncovered_409.jsonl").iter().map(k3_common::as_key).collect();
    assert_eq!(box409.len(), 409);
    let group = t178_group();
    assert_eq!(group.len(), 8);
    let sectors = Sectors::new((0..20).map(|s| s < 10).collect());
    let chk = CreditChecker::new(&t.disp, &t.bodies, &t.cmp, sectors, &NoFacts, true);
    let pol = CreditPolicy { group: group.clone(), tier: Tier::K, frame: CreditFrame::Direct };
    let polm = CreditPolicy { frame: CreditFrame::MappedBack, ..pol.clone() };

    let count = |keys: &[Vec<i32>]| -> (usize, usize, usize, usize) {
        let (mut with_cov, mut adopted, mut rejected_image_lower, mut mapped_back) = (0, 0, 0, 0);
        for k in keys {
            let r = chk.serve(k, &pol).unwrap();
            let covered_image = match &r {
                Ok(_) => true,
                Err(refs) => refs.iter().any(|(_, x)| {
                    !matches!(x, Refusal::NoImage | Refusal::ImageEqualsKey | Refusal::ImageNotCovered(_))
                }),
            };
            if covered_image {
                with_cov += 1;
            }
            if r.is_ok() {
                adopted += 1;
            }
            // REJECTED condition (test only): some covered image g k with g k lower than k
            if group.iter().any(|g| {
                g.apply(k).is_some_and(|gk| {
                    gk != *k && matches!(chk.dispatch(&gk), Dispatch::Covered(_)) && t.cmp.lower(&gk, k)
                })
            }) {
                rejected_image_lower += 1;
            }
            if chk.serve(k, &polm).unwrap().is_ok() {
                mapped_back += 1;
            }
        }
        (with_cov, adopted, rejected_image_lower, mapped_back)
    };
    let nf: Vec<Vec<i32>> = box409.iter().filter(|k| numerator_free(k)).cloned().collect();
    assert_eq!(nf.len(), 189);
    let (wc, ad, rj, mb) = count(&nf);
    println!(
        "T178 box numerator-free gaps: 189, with covered image {wc}, credited (adopted) {ad}, image-lower (rejected) {rj}, mapped-back variant {mb}"
    );
    assert_eq!(wc, 170);
    assert_eq!(ad, 75);
    assert_eq!(rj, 42);
    println!("T178 safe symmetry list: box 409 -> {}", 409 - ad);
    assert_eq!(409 - ad, 334);

    // the typical failure (h13b D2): k = I(1^9, D10=2): its image I(D4=2) goes to a rule producing I(D2=2), higher
    let k: Vec<i32> = (0..20).map(|s| if s == 9 { 2 } else if s < 10 { 1 } else { 0 }).collect();
    let refs = chk.serve(&k, &pol).unwrap().unwrap_err();
    assert!(refs.iter().any(|(_, x)| matches!(x, Refusal::TermNotBelow { .. })), "{refs:?}");
    assert!(refs.iter().all(|(_, x)| !matches!(x, Refusal::NoBody)));

    // the checker's envelope and loci point lists (read-only source; SKIP if absent)
    if let Ok(txt) = std::fs::read_to_string(S1) {
        let v: serde_json::Value = serde_json::from_str(&txt).unwrap();
        for (tag, want) in [("env_numfree", (716, 685, 310)), ("loci", (416, 416, 216))] {
            let keys: Vec<Vec<i32>> =
                v[tag]["rows"].as_array().unwrap().iter().map(|r| k3_common::as_key(&r["k"])).collect();
            let (wc, ad, rj, mb) = count(&keys);
            println!("T178 {tag}: {} points, with covered image {wc}, credited {ad}, image-lower {rj}, mapped-back {mb}", keys.len());
            assert_eq!((keys.len(), wc, ad), want);
        }
    } else {
        println!("SKIP envelope/loci lists: {S1} not readable");
    }

    // the closure with credits as a dispatch fallback: the box's 75 credited, 334 box keys open
    let cfg = ClosureConfig {
        tier: Tier::K,
        threads: 4,
        hand_downs: HandDowns::Counts,
        credits: Some(pol.clone()),
        ..Default::default()
    };
    let cl = closure(&t.disp, &t.bodies, &t.cmp, &t.box_keys, &cfg, &NoFacts).unwrap();
    let s = cl.summary();
    println!(
        "T178 closure with credits: rounds {} keys {} edges {} open {} = {} + {} credited {} (box {}) violations {} cycles {:?} chain {:?}",
        s.rounds,
        s.keys_total,
        s.edges,
        s.open_total,
        s.open_in_start,
        s.open_outside,
        s.credited,
        s.credited_in_start,
        s.descent_violations,
        s.cycles,
        s.longest_chain.as_ref().map(|c| c.edges)
    );
    assert_eq!(s.credited_in_start, 75);
    assert_eq!(s.open_in_start, 334);
    assert_eq!(s.descent_violations, 0);
    assert_eq!(s.cycles, Some(0));
    // every credit names element, image and rule, and the image's dispatch winner is that rule
    let credited: HashSet<Vec<i32>> = cl.credits().iter().map(|c| c.key.clone()).collect();
    for c in cl.credits().iter().filter(|c| c.in_start) {
        let Dispatch::Covered(r) = chk.dispatch(&c.image) else { panic!("credit image not covered") };
        assert_eq!((t.disp.tables()[r.table].name.clone(), r.label), (c.rule.table.clone(), c.rule.label));
        assert_eq!(group[c.element].apply(&c.key).unwrap(), c.image);
    }
    assert_eq!(credited.len() as u64, s.credited);
}
