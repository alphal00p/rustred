//! Lane parametric, T178-UNBOXED item B: the exact comparison of a SLOT-PERMUTED term with its target on a record's
//! whole domain (`parametric::permuted`), checked against the concrete comparator (`NodeComparator::cmp`, i.e.
//! `rustred_order::CompiledOrder::compare`) by enumeration, on random orders / patterns / domains, and on T178's two
//! one-term symmetry rules F3 / F4 under T178's order.
#![allow(clippy::needless_range_loop)]

use rustred_campaign_core::domain::CaseDomain;
use rustred_campaign_core::key::NodeFrame;
use rustred_campaign_core::loci::contain::ProofBudget;
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::parametric::permuted::{
    GenSlot, PermutedVerdict, parse_general, permuted_descent,
};
use rustred_campaign_core::poly::Poly;
use rustred_order::{DegreeRow, Direction, OrderDescriptor};
use std::cmp::Ordering;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// The term key at key `k` (target value = key; term slot `n_a + o` = `k_a - target offset + o`, target offsets 0 here).
fn term_key(u: &[GenSlot], k: &[i32]) -> Vec<i32> {
    u.iter()
        .map(|s| match *s {
            GenSlot::Fixed(c) => c as i32,
            GenSlot::Symbol { var, offset } => k[var] + offset as i32,
        })
        .collect()
}

fn keys(n_lines: usize, n: usize, lmax: i32, nmin: i32) -> Vec<Vec<i32>> {
    let mut out = vec![vec![]];
    for s in 0..n {
        let vals: Vec<i32> = if s < n_lines {
            (1..=lmax).collect()
        } else {
            (nmin..=0).collect()
        };
        let mut next = vec![];
        for p in &out {
            for &v in &vals {
                let mut q = p.clone();
                q.push(v);
                next.push(q);
            }
        }
        out = next;
    }
    out
}

#[test]
fn permuted_comparison_agrees_with_the_concrete_comparator() {
    let (n, nl) = (5usize, 3usize);
    let f = NodeFrame::without_header_lines_first(n, nl);
    let chart: Vec<bool> = (0..n).map(|i| i < nl).collect();
    let mut rng = Rng(0x5eed_0001);
    let box_keys = keys(nl, n, 7, -6);
    let (mut lower, mut not_lower, mut undecided) = (0, 0, 0);
    for case in 0..150 {
        // a random program order of the spired family
        let mut d = OrderDescriptor::spired_uncut(n);
        let mut prio: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() {
            prio.swap(i, rng.below(i as u64 + 1) as usize);
        }
        d.coordinate_priority = prio;
        if rng.below(2) == 0 {
            d.active_direction = Direction::Ascending;
        }
        if rng.below(2) == 0 {
            d.inactive_direction = Direction::Ascending;
        }
        if rng.below(3) == 0 {
            d.degree_rows[0] = DegreeRow {
                active: (0..n).map(|_| rng.below(3)).collect(),
                inactive: (0..n).map(|_| rng.below(3)).collect(),
            };
        }
        if rng.below(4) == 0 {
            d.pre_support_degree_rows = vec![DegreeRow {
                active: (0..n).map(|_| rng.below(2)).collect(),
                inactive: (0..n).map(|_| rng.below(2)).collect(),
            }];
        }
        let Ok(cmp) = NodeComparator::from_descriptor(d) else {
            continue;
        };
        // target: some slots fixed, the others symbolic (own symbol, offset 0)
        let mut fixed: Vec<Option<i32>> = vec![None; n];
        let mut tslots: Vec<String> = vec![];
        for s in 0..n {
            if rng.below(3) == 0 {
                let v = if s < nl {
                    1 + rng.below(2) as i32
                } else {
                    -(rng.below(2) as i32)
                };
                fixed[s] = Some(v);
                tslots.push(v.to_string());
            } else {
                tslots.push(format!("n{s}"));
            }
        }
        let target = format!("I({})", tslots.join(","));
        // term: symbols permuted among the symbolic slots of the same kind, small offsets; constants elsewhere
        let sym_lines: Vec<usize> = (0..nl).filter(|&s| fixed[s].is_none()).collect();
        let sym_nums: Vec<usize> = (nl..n).filter(|&s| fixed[s].is_none()).collect();
        let mut perm_l = sym_lines.clone();
        let mut perm_n = sym_nums.clone();
        for v in [&mut perm_l, &mut perm_n] {
            for i in (1..v.len()).rev() {
                v.swap(i, rng.below(i as u64 + 1) as usize);
            }
        }
        let mut uslots: Vec<String> = vec![];
        let (mut il, mut inum) = (0, 0);
        for s in 0..n {
            if fixed[s].is_some() {
                uslots.push(fixed[s].unwrap().to_string());
                continue;
            }
            let var = if s < nl {
                il += 1;
                perm_l[il - 1]
            } else {
                inum += 1;
                perm_n[inum - 1]
            };
            let o = rng.below(3) as i64 - 1;
            uslots.push(match o {
                0 => format!("n{var}"),
                o if o > 0 => format!("n{var}+{o}"),
                o => format!("n{var}{o}"),
            });
        }
        let term = format!("I({})", uslots.join(","));
        let u = parse_general(&term).unwrap();
        // domain: the target's fixed slots, symbolic slots kept far enough inside the chart for the offsets
        let mut dom = CaseDomain::free(n, Some(chart.clone()));
        dom.fixed = fixed.clone();
        for s in 0..n {
            if fixed[s].is_some() {
                continue;
            }
            let p = if s < nl {
                Poly::var(s).sub(&Poly::constant(2)).unwrap() // n_s >= 2
            } else {
                Poly::constant(-1).sub(&Poly::var(s)).unwrap() // n_s <= -1
            };
            dom.bounds.push(p);
        }
        if rng.below(3) == 0 && sym_lines.len() >= 2 {
            // a relation between two symbols (a half-space domain)
            let (a, b) = (sym_lines[0], sym_lines[1]);
            dom.bounds.push(Poly::var(a).sub(&Poly::var(b)).unwrap());
        }
        let v = permuted_descent(&cmp, &f, &dom, &target, &term, &ProofBudget::default());
        let in_dom = |k: &[i32]| dom.applies(k).applies();
        match &v {
            PermutedVerdict::Lower { .. } => {
                lower += 1;
                for k in box_keys.iter().filter(|k| in_dom(k)) {
                    let t = term_key(&u, k);
                    assert_eq!(
                        cmp.cmp(&t, k),
                        Ordering::Less,
                        "case {case}: {target} -> {term} at {k:?}: {v:?}"
                    );
                }
            }
            PermutedVerdict::NotLower {
                witness,
                lower_elsewhere,
                ..
            } => {
                not_lower += 1;
                assert!(in_dom(witness), "case {case}: witness {witness:?} outside");
                let t = term_key(&u, witness);
                assert_ne!(
                    cmp.cmp(&t, witness),
                    Ordering::Less,
                    "case {case}: {target} -> {term}"
                );
                if !lower_elsewhere {
                    for k in box_keys.iter().filter(|k| in_dom(k)) {
                        assert_ne!(cmp.cmp(&term_key(&u, k), k), Ordering::Less, "case {case}");
                    }
                }
            }
            PermutedVerdict::Undecided { .. } => undecided += 1,
        }
    }
    assert!(
        lower > 20 && not_lower > 20,
        "{lower} / {not_lower} / {undecided}"
    );
    assert!(undecided < 15, "{lower} / {not_lower} / {undecided}");
}

/// T178's order (`t178_orderB`: spired, coordinate priority [6,4,9,8,5,2,1,0,7,3,11,12,10,13,15,14,16..19]) and the
/// helper's F3 / F4 one-term symmetry rules (faces {2,5,9} -> {1,4,8} and {1,2,4} -> {2,4,5}, every dotted slot >= 2).
#[test]
fn t178_symmetry_rules_f3_f4_descend_on_their_whole_faces() {
    let n = 20;
    let f = NodeFrame::without_header_lines_first(n, 10);
    let mut d = OrderDescriptor::spired_uncut(n);
    d.coordinate_priority = vec![
        6, 4, 9, 8, 5, 2, 1, 0, 7, 3, 11, 12, 10, 13, 15, 14, 16, 17, 18, 19,
    ];
    let cmp = NodeComparator::from_descriptor(d).unwrap();
    let chart: Vec<bool> = (0..n).map(|i| i < 10).collect();
    let cases = [
        (
            "I(1,1,n2,1,1,n5,1,1,1,n9,0,0,0,0,0,0,0,0,0,0)",
            "I(1,n2,1,1,n5,1,1,1,n9,1,0,0,0,0,0,0,0,0,0,0)",
            vec![2usize, 5, 9],
        ),
        (
            "I(1,n1,n2,1,n4,1,1,1,1,1,0,0,0,0,0,0,0,0,0,0)",
            "I(1,1,n2,1,n4,n1,1,1,1,1,0,0,0,0,0,0,0,0,0,0)",
            vec![1usize, 2, 4],
        ),
    ];
    for (target, term, dotted) in cases {
        let mut dom = CaseDomain::free(n, Some(chart.clone()));
        dom.fixed = (0..n)
            .map(|s| {
                if dotted.contains(&s) {
                    None
                } else if s < 10 {
                    Some(1)
                } else {
                    Some(0)
                }
            })
            .collect();
        for &s in &dotted {
            dom.bounds
                .push(Poly::var(s).sub(&Poly::constant(2)).unwrap());
        }
        let v = permuted_descent(&cmp, &f, &dom, target, term, &ProofBudget::default());
        assert!(
            matches!(v, PermutedVerdict::Lower { .. }),
            "{target}: {v:?}"
        );
        // concrete check on a grid of the face
        let u = parse_general(term).unwrap();
        for a in 2..12 {
            for b in 2..12 {
                for c in 2..12 {
                    let mut k: Vec<i32> = (0..n).map(|s| if s < 10 { 1 } else { 0 }).collect();
                    k[dotted[0]] = a;
                    k[dotted[1]] = b;
                    k[dotted[2]] = c;
                    assert_eq!(cmp.cmp(&term_key(&u, &k), &k), Ordering::Less, "{k:?}");
                }
            }
        }
    }
}
