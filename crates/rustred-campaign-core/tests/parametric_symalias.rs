//! Lane t162face: one-term symmetry aliases for gap pieces (`parametric::symalias`): purity, image pattern, the
//! shipped record read back by the core reader, and the whole-piece descent verdict checked against the concrete
//! comparator by enumeration of the piece (a LINE LAW with bounds, like a locus piece of the coverage partition).
#![allow(clippy::needless_range_loop)]

use rustred_campaign_core::key::NodeFrame;
use rustred_campaign_core::loci::contain::ProofBudget;
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::parametric::permuted::{GenSlot, PermutedVerdict, parse_general};
use rustred_campaign_core::parametric::symalias::{
    Piece, candidates, canonical_target, image_pattern, pure_map, record_domain,
};
use rustred_campaign_core::record::parse_sym_line;
use rustred_campaign_core::replay_base::symrows::{SlotImage, SymElement, SymGroup};
use rustred_order::OrderDescriptor;
use serde_json::json;
use std::cmp::Ordering;

const N: usize = 8;
const LINES: usize = 5;

fn ex(t: usize) -> SlotImage {
    SlotImage::Exact {
        target: t,
        flip: false,
    }
}

/// Element 0: lines 1 <-> 2, 3 <-> 4, numerators 5 and 7 fixed, numerator 6 to a combination (impure there).
/// Element 1: lines as element 0 but numerator 5 to a combination. Element 2: lines 0 <-> 1 (line 0 is fixed at 1 on
/// the piece: pure, moves the fixed value).
fn group() -> SymGroup {
    let chart: Vec<bool> = (0..N).map(|s| s < LINES).collect();
    let e0 = SymElement {
        matrix: Some(vec![1, 0, 0, 1]),
        action: vec![
            ex(0),
            ex(2),
            ex(1),
            ex(4),
            ex(3),
            ex(5),
            SlotImage::Unknown,
            ex(7),
        ],
    };
    let e1 = SymElement {
        matrix: Some(vec![1, 0, 0, 1]),
        action: vec![
            ex(0),
            ex(2),
            ex(1),
            ex(4),
            ex(3),
            SlotImage::Unknown,
            ex(6),
            ex(7),
        ],
    };
    let e2 = SymElement {
        matrix: Some(vec![1, 0, 0, 1]),
        action: vec![ex(1), ex(0), ex(2), ex(3), ex(4), ex(5), ex(6), ex(7)],
    };
    SymGroup {
        arity: N,
        loops: 0,
        chart,
        elements: vec![e0, e1, e2],
    }
}

/// The piece: lines 1, 2, 4 and numerator 5 free; law n5 = 1 - 2 n1, n4 = n1 + 1; bound n1 >= 2.
fn piece() -> Piece {
    Piece {
        name: "toy line law".into(),
        fixed: vec![Some(1), None, None, Some(1), None, None, Some(0), Some(0)],
        equations: vec!["n5+2*n1-1".into(), "n4-n1-1".into()],
        domain_bounds: Some(
            json!({"min": [null, 2, null, null, null, null, null, null], "max": null}),
        ),
    }
}

fn piece_keys() -> Vec<Vec<i32>> {
    let mut out = vec![];
    for a in 2..14 {
        for b in 1..12 {
            out.push(vec![1, a, b, 1, a + 1, 1 - 2 * a, 0, 0]);
        }
    }
    out
}

fn term_key(u: &[GenSlot], k: &[i32]) -> Vec<i32> {
    u.iter()
        .map(|s| match *s {
            GenSlot::Fixed(c) => c as i32,
            GenSlot::Symbol { var, offset } => k[var] + offset as i32,
        })
        .collect()
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn shuffled(r: &mut Rng, n: usize) -> Vec<usize> {
    let mut v: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        let j = (r.next() % (i as u64 + 1)) as usize;
        v.swap(i, j);
    }
    v
}

#[test]
fn purity_and_image_pattern() {
    let g = group();
    let p = piece();
    let chart: Vec<bool> = (0..N).map(|s| s < LINES).collect();
    let m0 = pure_map(&g.elements[0], &p.fixed, &chart).expect("element 0 is pure on the piece");
    assert_eq!(image_pattern(&p.fixed, &m0), "I(1,n2,n1,n4,1,n5,0,0)");
    assert!(
        pure_map(&g.elements[1], &p.fixed, &chart).is_none(),
        "numerator 5 can be non-zero and has no exact image"
    );
    let m2 = pure_map(&g.elements[2], &p.fixed, &chart).unwrap();
    assert_eq!(image_pattern(&p.fixed, &m2), "I(n1,1,n2,1,n4,n5,0,0)");
    assert_eq!(canonical_target(&p.fixed), "I(1,n1,n2,1,n4,n5,0,0)");
}

#[test]
fn shipped_record_is_the_checked_domain() {
    let g = group();
    let p = piece();
    let f = NodeFrame::without_header_lines_first(N, LINES);
    let cmp = NodeComparator::from_descriptor(OrderDescriptor::spired_uncut(N)).unwrap();
    let c = candidates(&g, &p, &f, &cmp, &ProofBudget::default(), 7).unwrap();
    assert_eq!(c.len(), 2, "elements 0 and 2 are pure, element 1 is not");
    let dom = record_domain(&c[0].rule, &f).unwrap();
    for k in piece_keys() {
        assert!(dom.applies(&k).applies(), "{k:?}");
    }
    // off the bound, off the law
    assert!(!dom.applies(&[1, 1, 3, 1, 2, -1, 0, 0]).applies());
    assert!(!dom.applies(&[1, 3, 3, 1, 5, -5, 0, 0]).applies());
    assert!(!dom.applies(&[1, 3, 3, 1, 4, -4, 0, 0]).applies());
    // the engine sym record parses and names the same image
    let line = serde_json::to_vec(&c[0].sym).unwrap();
    let s = parse_sym_line(&line, 1).unwrap();
    assert_eq!(s.image, c[0].image);
    assert_eq!(s.equations, p.equations);
    assert_eq!(c[0].rule["rhs"][0][1], "1");
    assert_eq!(c[0].rule["index"], 7);
}

/// The whole-piece verdict agrees with the concrete comparator at every enumerated key of the piece, on random
/// coordinate priorities (spired descriptor), for every pure element.
#[test]
fn descent_verdict_agrees_with_the_concrete_comparator_on_the_piece() {
    let g = group();
    let p = piece();
    let f = NodeFrame::without_header_lines_first(N, LINES);
    let mut r = Rng(0x9e37_79b9_7f4a_7c15);
    let (mut lower, mut not_lower, mut undecided) = (0, 0, 0);
    for _ in 0..60 {
        let mut d = OrderDescriptor::spired_uncut(N);
        d.coordinate_priority = shuffled(&mut r, N);
        let cmp = NodeComparator::from_descriptor(d).unwrap();
        for c in candidates(&g, &p, &f, &cmp, &ProofBudget::default(), 0).unwrap() {
            let u = parse_general(&c.image).unwrap();
            match &c.descent {
                PermutedVerdict::Lower { .. } => {
                    lower += 1;
                    for k in piece_keys() {
                        assert_eq!(
                            cmp.cmp(&term_key(&u, &k), &k),
                            Ordering::Less,
                            "{} at {k:?}",
                            c.image
                        );
                    }
                }
                PermutedVerdict::NotLower { witness, .. } => {
                    not_lower += 1;
                    let dom = record_domain(&c.rule, &f).unwrap();
                    assert!(
                        dom.applies(witness).applies(),
                        "witness {witness:?} off the piece"
                    );
                    assert_ne!(cmp.cmp(&term_key(&u, witness), witness), Ordering::Less);
                }
                PermutedVerdict::Undecided { .. } => undecided += 1,
            }
        }
    }
    assert!(
        lower > 5 && not_lower > 5,
        "{lower} / {not_lower} / {undecided}"
    );
    assert_eq!(undecided, 0, "{lower} / {not_lower} / {undecided}");
}
