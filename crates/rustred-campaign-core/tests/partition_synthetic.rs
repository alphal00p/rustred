//! Partition, loci and containment on SYNTHETIC tables of invented structure and several arities / line counts
//! (no fixture needed; lane rule L8: nothing five-loop specific): property tests against brute force, memoised ==
//! slow reference mode, planted containment / emptiness cases, the fallback face list, and the no-floating-point
//! check of the module sources.

use std::collections::BTreeSet;

use proptest::prelude::*;
use rustred_campaign_core::cover::RuleRef;
use rustred_campaign_core::domain::{CaseDomain, Verdict};
use rustred_campaign_core::key::{Key, NodeFrame};
use rustred_campaign_core::partition::{
    ClassHit, Containment, DomainList, ElidedPolicy, Emptiness, FaceKind, KeyRange, Label,
    LiteralTests, PartitionOptions, Proof, contained_in, face_of, faces_from_uncovered_keys,
    gap_partition, is_empty, shadowed,
};
use rustred_campaign_core::poly::Poly;

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// A random table: `n` slots, the first `e` are lines.
fn random_table(seed: u64, n: usize, e: usize, rules: usize) -> DomainList {
    let mut g = Lcg(seed);
    let mask: Vec<bool> = (0..n).map(|i| i < e).collect();
    let mut l = DomainList::new(n);
    let var = |i: usize| format!("n{i}");
    for r in 0..rules {
        let mut d = CaseDomain::free(n, Some(mask.clone()));
        for s in 0..n {
            if g.below(3) == 0 {
                d.fixed[s] = Some(if s < e {
                    1 + g.below(3) as i32
                } else {
                    -(g.below(3) as i32)
                });
            }
        }
        if g.below(4) == 0 {
            let (a, b) = (g.below(n as u64) as usize, g.below(n as u64) as usize);
            if a != b {
                let c = g.below(5) as i64 - 2;
                let k = [1, 2, -1][g.below(3) as usize];
                d.equations.push(
                    Poly::parse(&format!("{}-{k}*{}-({c})", var(a), var(b)), n, false).unwrap(),
                );
            }
        }
        for _ in 0..g.below(3) {
            let mut br = vec![];
            for _ in 0..1 + g.below(2) {
                let a = g.below(n as u64) as usize;
                let b = g.below(n as u64) as usize;
                let c = g.below(7) as i64 - 3;
                let t = match g.below(6) {
                    0 | 1 => format!("{}-({c})", var(a)),
                    2 => format!("{}-{}-({c})", var(a), var(b)),
                    3 => format!("{}+2*{}-({c})", var(a), var(b)),
                    4 => format!("{}^2-{}", var(a), (c * c)),
                    _ => format!("{}*{}-({c})", var(a), var(b)),
                };
                br.push(Poly::parse(&t, n, false).unwrap());
            }
            d.exceptions.push(br);
        }
        l.push(
            RuleRef {
                table: 0,
                record: r,
                label: r as i64,
            },
            d,
        );
    }
    l
}

/// Every chart key with lines in 1..=w and numerators in -(w-1)..=0.
fn window(n: usize, e: usize, w: i32) -> Vec<Key> {
    let mut out = vec![vec![]];
    for s in 0..n {
        let vals: Vec<i32> = if s < e {
            (1..=w).collect()
        } else {
            (-(w - 1)..=0).collect()
        };
        out = out
            .into_iter()
            .flat_map(|k: Key| {
                vals.iter().map(move |&v| {
                    let mut k2 = k.clone();
                    k2.push(v);
                    k2
                })
            })
            .collect();
    }
    out
}

fn brute_covered(l: &DomainList, k: &[i32]) -> bool {
    l.items
        .iter()
        .any(|(_, d)| d.applies(k) == Verdict::Applies)
}

fn check_against_brute(seed: u64, n: usize, e: usize, rules: usize, w: i32) {
    let l = random_table(seed, n, e, rules);
    let frame = NodeFrame::without_header_lines_first(n, e);
    let opts = PartitionOptions {
        elided: ElidedPolicy::Ordinary,
        ..Default::default()
    };
    let rep = gap_partition(&l, &frame, &opts, None).unwrap();
    let faces: BTreeSet<(Vec<usize>, Vec<usize>)> = rep.faces.iter().map(|f| f.key()).collect();
    let generic: BTreeSet<(Vec<usize>, Vec<usize>)> = rep
        .faces
        .iter()
        .filter(|f| f.kind == FaceKind::Generic)
        .map(|f| f.key())
        .collect();
    for k in window(n, e, w) {
        let cov = brute_covered(&l, &k);
        assert_eq!(
            rep.uncovered(&k).unwrap(),
            !cov,
            "oracle at {k:?} (seed {seed})"
        );
        let hit = rep.class_of(&k).unwrap();
        if !cov {
            assert_ne!(
                hit,
                ClassHit::Covered,
                "uncovered key in a covered class {k:?} (seed {seed})"
            );
            let f = face_of(&rep.nf.lines, &k);
            assert!(
                faces.contains(&f),
                "face of uncovered {k:?} not listed (seed {seed})"
            );
            if hit == ClassHit::Uncovered {
                assert!(generic.contains(&f));
            }
        }
    }
    // every listed face has an uncovered point somewhere (generic faces: the witness search), never a covered witness
    for f in &rep.faces {
        if let Some(w) = &f.witness {
            assert!(rep.uncovered(w).unwrap());
            assert_eq!(face_of(&rep.nf.lines, w), f.key());
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]
    #[test]
    fn partition_vs_brute_force_arity5(seed in any::<u64>()) {
        check_against_brute(seed, 5, 2, 12, 5);
    }
    #[test]
    fn partition_vs_brute_force_arity9(seed in any::<u64>()) {
        check_against_brute(seed, 9, 4, 30, 3);
    }
    #[test]
    fn partition_vs_brute_force_arity7_three_lines(seed in any::<u64>()) {
        check_against_brute(seed, 7, 3, 20, 4);
    }
}

#[test]
fn memoised_equals_reference_mode_capped() {
    for seed in 0..12u64 {
        for (n, e, cap) in [(9usize, 4usize, 6usize), (7, 3, 7)] {
            let l = random_table(seed * 7919 + 3, n, e, 25);
            let frame = NodeFrame::without_header_lines_first(n, e);
            let base = PartitionOptions {
                elided: ElidedPolicy::Ordinary,
                resolve_loci: false,
                max_off_floor: Some(cap),
                ..Default::default()
            };
            let a = gap_partition(&l, &frame, &base, None).unwrap();
            let b = gap_partition(
                &l,
                &frame,
                &PartitionOptions {
                    memo: false,
                    ..base.clone()
                },
                None,
            )
            .unwrap();
            let ca: BTreeSet<_> = a.classes.iter().cloned().collect();
            let cb: BTreeSet<_> = b.classes.iter().cloned().collect();
            assert_eq!(
                ca, cb,
                "seed {seed} arity {n}: memo {} nodes vs reference {} nodes",
                a.stats.nodes, b.stats.nodes
            );
            // and the uncapped memoised run restricted to <= cap off-floor slots has the same classes
            let full = gap_partition(
                &l,
                &frame,
                &PartitionOptions {
                    max_off_floor: None,
                    ..base.clone()
                },
                None,
            )
            .unwrap();
            let off = |c: &rustred_campaign_core::partition::Class| {
                c.states.iter().filter(|s| s.is_none_or(|v| v != 0)).count()
            };
            let cf: BTreeSet<_> = full
                .classes
                .iter()
                .filter(|c| off(c) <= cap && c.states.iter().all(|s| s.is_some()))
                .cloned()
                .collect();
            let ca2: BTreeSet<_> = ca
                .iter()
                .filter(|c| c.states.iter().all(|s| s.is_some()))
                .cloned()
                .collect();
            assert_eq!(cf, ca2, "seed {seed}");
        }
    }
}

#[test]
fn exact_and_reference_literal_tests_same_uncovered_set() {
    for seed in 0..10u64 {
        let l = random_table(seed + 100, 7, 3, 20);
        let frame = NodeFrame::without_header_lines_first(7, 3);
        let mk = |t| PartitionOptions {
            elided: ElidedPolicy::Ordinary,
            literal_tests: t,
            ..Default::default()
        };
        let a = gap_partition(&l, &frame, &mk(LiteralTests::Exact), None).unwrap();
        let b = gap_partition(&l, &frame, &mk(LiteralTests::ReferenceCompatible), None).unwrap();
        let ua: BTreeSet<_> = a
            .classes
            .iter()
            .filter(|c| c.label == Label::Uncovered)
            .collect();
        let ub: BTreeSet<_> = b
            .classes
            .iter()
            .filter(|c| c.label == Label::Uncovered)
            .collect();
        assert_eq!(ua, ub);
        assert!(a.n_f() <= b.n_f());
    }
}

fn dom(n: usize, e: usize, fixed: &[(usize, i32)], eqs: &[&str], exc: &[&[&str]]) -> CaseDomain {
    let mut d = CaseDomain::free(n, Some((0..n).map(|i| i < e).collect()));
    for &(s, v) in fixed {
        d.fixed[s] = Some(v);
    }
    d.equations = eqs
        .iter()
        .map(|t| Poly::parse(t, n, false).unwrap())
        .collect();
    d.exceptions = exc
        .iter()
        .map(|b| {
            b.iter()
                .map(|t| Poly::parse(t, n, false).unwrap())
                .collect()
        })
        .collect();
    d
}

#[test]
fn emptiness_planted() {
    let f = NodeFrame::without_header_lines_first(6, 3);
    // lines >= 1: n0 + n1 = 1 is impossible
    assert!(matches!(
        is_empty(&dom(6, 3, &[], &["n0+n1-1"], &[]), &f),
        Emptiness::ProvedEmpty { .. }
    ));
    // non-unit affine across a line and a numerator: n0 + 2 n3 = 1 has points (3, -1), excluded by the exceptions
    // n0 = 3 and n0 - 5 = 0 ... only on (1,0),(3,-1),(5,-2),...: exclude n3 = 0 and n3 <= -1 via n0 >= 3 cases
    let d = dom(6, 3, &[], &["n0+2*n3-1"], &[&["n3"], &["n0-3"], &["n0-5"]]);
    match is_empty(&d, &f) {
        Emptiness::NonEmpty { witness } => assert_eq!(d.applies(&witness), Verdict::Applies),
        o => panic!("{o:?}"),
    }
    // the T178B-style empty case: n3 + n8 - n12 = 3, n8 != 1, n8 - n12 != 2, n11 != 0, in a 14-slot frame (10 lines)
    let f14 = NodeFrame::without_header_lines_first(14, 10);
    let d = dom(
        14,
        10,
        &[
            (0, 1),
            (1, 1),
            (2, 1),
            (4, 1),
            (5, 1),
            (6, 1),
            (7, 1),
            (9, 1),
            (10, 0),
            (13, 0),
        ],
        &["-3-n12+n8+n3"],
        &[&["-2-n12+n8"], &["-1+n8"], &["n11"]],
    );
    assert!(
        matches!(is_empty(&d, &f14), Emptiness::ProvedEmpty { .. }),
        "{:?}",
        is_empty(&d, &f14)
    );
    // a nonlinear exception that cannot be decided: never a guess
    let d = dom(6, 3, &[], &[], &[&["n0*n1-n2^2"]]);
    assert!(matches!(
        is_empty(&d, &f),
        Emptiness::NonEmpty { .. } | Emptiness::Unknown { .. }
    ));
}

#[test]
fn containment_planted() {
    let f = NodeFrame::without_header_lines_first(5, 2);
    let d = dom(5, 2, &[(1, 2)], &[], &[]);
    let a = dom(5, 2, &[(1, 2)], &[], &[&["n0-3"]]);
    let b = dom(5, 2, &[(1, 2)], &["n0-3"], &[]);
    assert!(matches!(
        contained_in(&d, &[&a, &b], &f),
        Containment::Proved { .. }
    ));
    match contained_in(&d, &[&a], &f) {
        Containment::NotContained { witness } => {
            assert_eq!(witness[0], 3);
            assert_eq!(d.applies(&witness), Verdict::Applies);
        }
        o => panic!("{o:?}"),
    }
    // diagonal locus covered by a general affine equation with non-unit coefficients: 2 n0 - 2 n1 = 0
    let g = dom(5, 2, &[], &[], &[&["n0-n1"]]);
    let h = dom(5, 2, &[], &["2*n0-2*n1"], &[]);
    let all = dom(5, 2, &[], &[], &[]);
    assert!(matches!(
        contained_in(&all, &[&g, &h], &f),
        Containment::Proved { .. }
    ));
    // shadowed under the dispatch order
    let mut l = DomainList::new(5);
    for (i, x) in [g.clone(), h.clone(), all.clone()].into_iter().enumerate() {
        l.push(
            RuleRef {
                table: 0,
                record: i,
                label: i as i64,
            },
            x,
        );
    }
    assert!(matches!(shadowed(&l, 2, &f), Containment::Proved { .. }));
    assert!(!matches!(shadowed(&l, 1, &f), Containment::Proved { .. }));
}

#[test]
fn fallback_faces_lower_bound() {
    let f = NodeFrame::without_header_lines_first(6, 2);
    let keys: Vec<Key> = vec![
        vec![2, 1, 0, 0, -1, 0],
        vec![3, 1, 0, 0, -2, 0],
        vec![1, 1, 0, 0, 0, 0],
    ];
    let faces = faces_from_uncovered_keys(&f, &keys, KeyRange::Measured);
    assert_eq!(faces.len(), 2);
    assert!(faces.iter().all(|x| x.proof == Proof::ClosureLowerBound && x.key_range == Some(KeyRange::Measured)));
}

#[test]
fn locus_with_non_unit_affine_equation_is_resolved_exactly() {
    // one rule covering everything except on n0 + 2 n3 = 1 (line and numerator, non-unit coefficient): the
    // partition must list the locus with that equation, infinite, with uncovered witnesses on it
    let n = 5;
    let f = NodeFrame::without_header_lines_first(n, 2);
    let mut l = DomainList::new(n);
    l.push(
        RuleRef {
            table: 0,
            record: 0,
            label: 0,
        },
        dom(n, 2, &[], &[], &[&["n0+2*n3-1"]]),
    );
    let rep = gap_partition(
        &l,
        &f,
        &PartitionOptions {
            elided: ElidedPolicy::Ordinary,
            ..Default::default()
        },
        None,
    )
    .unwrap();
    let res = rep.resolution.as_ref().unwrap();
    assert!(!res.loci.is_empty());
    for loc in &res.loci {
        assert_eq!(loc.equations, vec!["n0+2*n3-1".to_string()]);
        assert!(!loc.unresolved);
        for w in &loc.witness {
            assert_eq!(w[0] + 2 * w[3], 1);
            assert!(rep.uncovered(w).unwrap());
        }
    }
    assert!(
        res.loci
            .iter()
            .any(|l| l.pieces.iter().any(|p| p.unbounded == Some(true)))
    );
    // n0 = 1, n3 = 0 solves the equation for every value of the other slots: faces without slot 0 and slot 3 are
    // generic gaps, every other face is locus-only
    for x in &rep.faces {
        if !x.sd.contains(&0) && !x.sn.contains(&3) {
            assert_eq!(x.kind, FaceKind::Generic);
        } else {
            assert_eq!(x.kind, FaceKind::LocusOnly, "{:?}", x.key());
        }
    }
}

#[test]
fn nonlinear_locus_is_unresolved_with_window() {
    let n = 4;
    let f = NodeFrame::without_header_lines_first(n, 2);
    let mut l = DomainList::new(n);
    // a conic exception in two free off-floor slots
    l.push(
        RuleRef {
            table: 0,
            record: 0,
            label: 0,
        },
        dom(n, 2, &[], &[], &[&["n0^2-2*n1^2-1"]]),
    );
    let rep = gap_partition(
        &l,
        &f,
        &PartitionOptions {
            elided: ElidedPolicy::Ordinary,
            ..Default::default()
        },
        None,
    )
    .unwrap();
    let res = rep.resolution.as_ref().unwrap();
    assert!(res.n_unresolved() > 0, "Pell conic must stay UNRESOLVED");
    let pts: Vec<&Key> = res
        .loci
        .iter()
        .flat_map(|l| l.unresolved_parts.iter().flat_map(|u| u.points.iter()))
        .collect();
    // (3, 2) and (17, 12) solve n0^2 - 2 n1^2 = 1
    assert!(pts.iter().any(|k| k[0] == 3 && k[1] == 2));
    assert!(pts.iter().any(|k| k[0] == 17 && k[1] == 12));
    let js = rep.to_json(&Default::default(), &f);
    assert_eq!(js["outcome"], "unresolved");
}

#[test]
fn no_floating_point_in_partition_and_loci() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = vec![];
    for dir in ["partition", "loci"] {
        for e in std::fs::read_dir(root.join(dir)).unwrap() {
            files.push(e.unwrap().path());
        }
    }
    assert!(files.len() >= 6);
    for p in files {
        let s = std::fs::read_to_string(&p).unwrap();
        for (i, line) in s.lines().enumerate() {
            let code = line.split("//").next().unwrap();
            for bad in ["f32", "f64", "as_f64", "powf", "sqrt("] {
                let hit = code.match_indices(bad).any(|(j, _)| {
                    let prev = code[..j].chars().last();
                    let next = code[j + bad.len()..].chars().next();
                    !prev.is_some_and(|c| c.is_alphanumeric() || c == '_')
                        && !next.is_some_and(|c| c.is_alphanumeric() || c == '_')
                });
                assert!(
                    !hit,
                    "{}:{}: floating point `{bad}`: {line}",
                    p.display(),
                    i + 1
                );
            }
        }
    }
}
