//! Regressions of the partition / loci / containment audits (audit-part-A: C1, C2, M1, M2, M3, M4, m1-m4; the
//! memoised-vs-reference cross-check on UNCAPPED small tables). Each test fails on the code before the fix (commit
//! 4c9272db) and passes after it. The auditor's confirming tests (they asserted the bugs) are ported here with the
//! assertions inverted. Synthetic tables only (lane rule L8).

use std::collections::BTreeSet;

use rustred_campaign_core::cover::{Dispatch, Dispatcher, RuleRef, Table};
use rustred_campaign_core::domain::{
    CaseDomain, PoleKind, PoleLocus, Stage, TargetMap, TargetSlot, Verdict,
};
use rustred_campaign_core::economy::{
    ContainmentProof, CoverageState, RetireError, Retirement, RuleMeasure, SupersedeReason,
};
use rustred_campaign_core::key::{Key, NodeFrame};
use rustred_campaign_core::loci::LociOptions;
use rustred_campaign_core::partition::exact::int_roots;
use rustred_campaign_core::partition::nf::NfNote;
use rustred_campaign_core::partition::{
    Budget, ClassHit, Containment, CoverageNotion, DispatchOrderStatement, DomainList,
    ElidedPolicy, Emptiness, Explored, LiteralTests, Outcome, PartitionOptions, Range,
    contained_in, dispatch_proof_method, face_of, gap_partition, is_empty,
    parse_dispatch_proof_method, removal_preserves_dispatch, shadowed,
};
use rustred_campaign_core::poly::Poly;

fn dom(n: usize, e: usize, eqs: &[&str], exc: &[&[&str]]) -> CaseDomain {
    let mut d = CaseDomain::free(n, Some((0..n).map(|i| i < e).collect()));
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

fn list(ds: Vec<CaseDomain>, n: usize) -> DomainList {
    let mut l = DomainList::new(n);
    for (i, d) in ds.into_iter().enumerate() {
        l.push(RuleRef::bare(0, i, i as i64), d);
    }
    l
}

fn ordinary() -> PartitionOptions {
    PartitionOptions {
        elided: ElidedPolicy::Ordinary,
        ..Default::default()
    }
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

/// Every uncovered window key (brute force over `applies`) lies in a non-covered class and on a listed face.
fn assert_faces_complete(
    l: &DomainList,
    rep: &rustred_campaign_core::partition::PartitionReport,
    n: usize,
    e: usize,
    w: i32,
) {
    let faces: BTreeSet<_> = rep.faces.iter().map(|f| f.key()).collect();
    for k in window(n, e, w) {
        let cov = l
            .items
            .iter()
            .any(|(_, d)| d.applies(&k) == Verdict::Applies);
        assert_eq!(rep.uncovered(&k).unwrap(), !cov, "oracle at {k:?}");
        if !cov {
            assert_ne!(rep.class_of(&k).unwrap(), ClassHit::Covered, "{k:?}");
            assert!(
                faces.contains(&face_of(&rep.nf.lines, &k)),
                "face of the uncovered key {k:?} is not listed"
            );
        }
    }
}

// ------------------------------------------------------------------------------------------------------------------
// C1: a real gap dropped by witness SAMPLING
// ------------------------------------------------------------------------------------------------------------------

/// A covers all except n0 = n1; B1/B2/B3 cover exactly the three points the old witness search probed on the locus
/// {n0 = n1 = s, n2 = 1} (s = 2, 3, 7). Every other s >= 2 is uncovered, e.g. (4,4,1): face ({0,1},{}) must be
/// listed with a locus on it (it was dropped, outcome "complete").
#[test]
fn c1_locus_not_dropped_by_witness_sampling() {
    let n = 3;
    let f = NodeFrame::without_header_lines_first(n, 3);
    let l = list(
        vec![
            dom(n, 3, &[], &[&["n0-n1"]]),
            dom(n, 3, &["n0+n2-3"], &[]),
            dom(n, 3, &["n0+n2-4"], &[]),
            dom(n, 3, &["n0+n2-8"], &[]),
        ],
        n,
    );
    let rep = gap_partition(&l, &f, &ordinary(), None).unwrap();
    assert!(rep.uncovered(&[4, 4, 1]).unwrap());
    assert_eq!(rep.class_of(&[4, 4, 1]).unwrap(), ClassHit::Fragile);
    assert!(
        rep.faces.iter().any(|x| x.key() == (vec![0, 1], vec![])),
        "face ({{0,1}},{{}}) must be listed"
    );
    let res = rep.resolution.as_ref().unwrap();
    let on_face: Vec<_> = res
        .loci
        .iter()
        .filter(|l| l.face == (vec![0, 1], vec![]))
        .collect();
    assert!(!on_face.is_empty());
    // the piece is the locus MINUS the covered hyperplanes, exactly; its witnesses are uncovered keys
    for l in &on_face {
        for w in &l.witness {
            assert!(rep.uncovered(w).unwrap());
        }
        assert!(l.pieces.iter().any(|p| !p.minus.is_empty()));
    }
    assert!(rep.outcome.is_complete(), "{:?}", rep.outcome);
    assert_faces_complete(&l, &rep, n, 3, 12);
}

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

/// C1 property: a fragile rule (exception on a coupled linear locus) plus rules whose linear case equations cover
/// SCATTERED points of that locus: every uncovered window key's face is listed and its class is not Covered.
#[test]
fn c1_scattered_covered_points_on_a_locus_never_hide_the_gap() {
    for seed in 0..40u64 {
        let mut g = Lcg(seed * 7 + 1);
        let n = 4;
        let e = 3;
        let mut ds = vec![];
        let a = g.below(3) as usize;
        let b = (a + 1 + g.below(2) as usize) % 3;
        ds.push(dom(n, e, &[], &[&[&format!("n{a}-n{b}")]]));
        for _ in 0..(2 + g.below(4)) {
            let c = 2 + g.below(9) as i64;
            let s = g.below(n as u64) as usize;
            let t = (s + 1) % n;
            ds.push(dom(n, e, &[&format!("n{s}+n{t}-{c}")], &[]));
        }
        let l = list(ds, n);
        let f = NodeFrame::without_header_lines_first(n, e);
        let rep = gap_partition(&l, &f, &ordinary(), None).unwrap();
        assert_faces_complete(&l, &rep, n, e, 6);
    }
}

// ------------------------------------------------------------------------------------------------------------------
// C2: non-canonical target excluded from the normal form and then reported PROVED empty / shadowed
// ------------------------------------------------------------------------------------------------------------------

#[test]
fn c2_pattern_target_domain_is_modelled_not_proved_empty() {
    let n = 2;
    let f = NodeFrame::without_header_lines_first(n, 2);
    let mut d = CaseDomain::free(n, Some(vec![true, true]));
    d.target = TargetMap::Pattern(vec![
        TargetSlot::Param {
            param: 0,
            offset: 1,
        },
        TargetSlot::Param {
            param: 1,
            offset: 0,
        },
    ]);
    assert_eq!(d.applies(&[2, 1]), Verdict::Applies);
    match is_empty(&d, &f) {
        Emptiness::NonEmpty { witness } => assert_eq!(d.applies(&witness), Verdict::Applies),
        o => panic!("must not be proved empty: {o:?}"),
    }
    let l = list(vec![d.clone()], n);
    assert!(!matches!(shadowed(&l, 0, &f), Containment::Proved { .. }));
    // and the partition covers what it applies to
    let rep = gap_partition(&l, &f, &ordinary(), None).unwrap();
    assert_eq!(rep.nf.stats.pattern_targets, 1);
    assert!(!rep.uncovered(&[2, 1]).unwrap());
}

/// Random target patterns (offsets, constants, unmapped parameters) with equations / exceptions evaluated AT THE
/// RULE POINT: the oracle equals `CaseDomain::applies` key by key, and faces are complete.
#[test]
fn c2_pattern_targets_equal_applies_key_by_key() {
    for seed in 0..60u64 {
        let mut g = Lcg(seed * 31 + 5);
        let n = 4;
        let e = 2;
        let mut ds = vec![];
        for _ in 0..(1 + g.below(4)) {
            let mut d = dom(n, e, &[], &[]);
            let mut perm: Vec<usize> = (0..n).collect();
            for i in (1..n).rev() {
                perm.swap(i, g.below(i as u64 + 1) as usize);
            }
            let mut slots = vec![];
            for (j, &pj) in perm.iter().enumerate() {
                if g.below(5) == 0 {
                    let c = if j < e {
                        1 + g.below(3) as i64
                    } else {
                        -(g.below(3) as i64)
                    };
                    slots.push(TargetSlot::Const(c));
                } else {
                    slots.push(TargetSlot::Param {
                        param: pj,
                        offset: g.below(5) as i64 - 2,
                    });
                }
            }
            d.target = TargetMap::Pattern(slots);
            if g.below(2) == 0 {
                let (a, b) = (g.below(n as u64), g.below(n as u64));
                d.equations.push(
                    Poly::parse(&format!("n{a}-n{b}-({})", g.below(3) as i64 - 1), n, false)
                        .unwrap(),
                );
            }
            if g.below(2) == 0 {
                let a = g.below(n as u64);
                d.exceptions.push(vec![
                    Poly::parse(&format!("n{a}-({})", g.below(4) as i64 - 1), n, false).unwrap(),
                ]);
            }
            ds.push(d);
        }
        let l = list(ds, n);
        let f = NodeFrame::without_header_lines_first(n, e);
        let rep = gap_partition(&l, &f, &ordinary(), None).unwrap();
        assert_faces_complete(&l, &rep, n, e, 5);
        // emptiness of each domain: never Proved while it applies somewhere in the window
        for (_, d) in &l.items {
            let some = window(n, e, 5)
                .iter()
                .any(|k| d.applies(k) == Verdict::Applies);
            match is_empty(d, &f) {
                Emptiness::ProvedEmpty { certificate } => {
                    assert!(
                        !some,
                        "seed {seed}: proved empty but applies ({certificate})"
                    )
                }
                Emptiness::NonEmpty { witness } => {
                    assert_eq!(d.applies(&witness), Verdict::Applies)
                }
                Emptiness::Unknown { .. } => {}
            }
        }
    }
}

/// The engine computes the rule point in i32: where `n_j - o` leaves i32 the verdict is an evaluation error (the
/// search stops). The normal form models it as a range bound: such keys are not covered.
#[test]
fn c2_rule_point_range_bound_is_modelled() {
    let n = 2;
    let f = NodeFrame::without_header_lines_first(n, 2);
    let mut d = CaseDomain::free(n, Some(vec![true, true]));
    d.target = TargetMap::Pattern(vec![
        TargetSlot::Param {
            param: 0,
            offset: -3,
        },
        TargetSlot::Param {
            param: 1,
            offset: 0,
        },
    ]);
    let k = [i32::MAX - 1, 1];
    assert_eq!(d.applies(&k).stage(), Stage::Undecided);
    let l = list(vec![d.clone()], n);
    let rep = gap_partition(&l, &f, &ordinary(), None).unwrap();
    assert!(rep.uncovered(&k).unwrap());
    assert!(!rep.uncovered(&[5, 1]).unwrap());
    assert_ne!(rep.class_of(&k).unwrap(), ClassHit::Covered);
}

/// Unrepresentable literals: an equation -> the domain is EXCLUDED (credits nothing), an exception member -> the
/// domain is WEAKENED (the member may vanish anywhere); both visible as notes and never the subject of a proof.
#[test]
fn c2_exclusions_are_visible_and_never_prove() {
    let n = 2;
    let f = NodeFrame::without_header_lines_first(n, 2);
    // a literal containing d cannot be a d-free case literal
    let mut ex = CaseDomain::free(n, Some(vec![true, true]));
    ex.equations.push(Poly::parse("n0-d", n, true).unwrap());
    let mut weak = CaseDomain::free(n, Some(vec![true, true]));
    weak.exceptions.push(vec![
        Poly::parse("n0-2", n, false).unwrap(),
        Poly::parse("n1-d", n, true).unwrap(),
    ]);
    let l = list(vec![ex.clone(), weak.clone()], n);
    let rep = gap_partition(&l, &f, &ordinary(), None).unwrap();
    assert!(matches!(
        rep.nf.notes_of(0).next(),
        Some(NfNote::Excluded(_))
    ));
    assert!(matches!(
        rep.nf.notes_of(1).next(),
        Some(NfNote::Weakened(_))
    ));
    // weakened: the branch [n0-2] now "vanishes" on all of n0 = 2: those keys are reported uncovered (conservative)
    assert!(rep.uncovered(&[2, 5]).unwrap());
    assert!(!rep.uncovered(&[3, 5]).unwrap());
    let js = rep.to_json(&Default::default(), &f);
    assert_eq!(js["input_notes"].as_array().unwrap().len(), 2);
    assert!(matches!(is_empty(&ex, &f), Emptiness::Unknown { .. }));
    assert!(matches!(is_empty(&weak, &f), Emptiness::Unknown { .. }));
    let all = CaseDomain::free(n, Some(vec![true, true]));
    assert!(matches!(
        contained_in(&weak, &[&all], &f),
        Containment::Unknown { .. }
    ));
    // as an OTHER an excluded domain only shrinks: containment in {all, ex} is still proved by `all`
    assert!(matches!(
        contained_in(&all, &[&ex, &all], &f),
        Containment::Proved { .. }
    ));
}

/// `contained_in` counts an elided other as NOT applying (it never serves); `shadowed` counts it (it is selected).
#[test]
fn c2_elided_others_in_containment_and_shadowing() {
    let n = 2;
    let f = NodeFrame::without_header_lines_first(n, 2);
    let mut el = CaseDomain::free(n, Some(vec![true, true]));
    el.witness_elided = true;
    let d = dom(n, 2, &["n0-3"], &[]);
    assert!(!matches!(
        contained_in(&d, &[&el], &f),
        Containment::Proved { .. }
    ));
    let l = list(vec![el, d], n);
    assert!(matches!(shadowed(&l, 1, &f), Containment::Proved { .. }));
}

// ------------------------------------------------------------------------------------------------------------------
// M1: capped / errored locus resolution silently dropped
// ------------------------------------------------------------------------------------------------------------------

#[test]
fn m1_capped_locus_resolution_is_reported_open() {
    let n = 3;
    let f = NodeFrame::without_header_lines_first(n, 3);
    let l = list(vec![dom(n, 3, &[], &[&["n0-n1"]])], n);
    let base = gap_partition(&l, &f, &ordinary(), None).unwrap();
    assert!(base.outcome.is_complete());
    let o = PartitionOptions {
        loci: LociOptions {
            max_nodes: 0,
            ..Default::default()
        },
        ..ordinary()
    };
    let rep = gap_partition(&l, &f, &o, None).unwrap();
    assert!(rep.uncovered(&[4, 4, 1]).unwrap());
    let fb: BTreeSet<_> = base.faces.iter().map(|x| x.key()).collect();
    let fr: BTreeSet<_> = rep.faces.iter().map(|x| x.key()).collect();
    assert!(
        fb.is_subset(&fr),
        "a capped resolution must keep every face as an open region"
    );
    assert!(
        matches!(rep.outcome, Outcome::Incomplete { .. }),
        "{:?}",
        rep.outcome
    );
    let js = rep.to_json(&Default::default(), &f);
    assert_ne!(js["outcome"], "complete");
    assert!(rep.resolution.as_ref().unwrap().n_unresolved_parts() > 0);
    assert!(rep.proves_no_gap(CoverageNotion::UnionCoverage).is_err());
    // the depth cap too
    let o = PartitionOptions {
        loci: LociOptions {
            max_depth: 0,
            ..Default::default()
        },
        ..ordinary()
    };
    let rep = gap_partition(&l, &f, &o, None).unwrap();
    assert!(matches!(rep.outcome, Outcome::Incomplete { .. }));
}

// ------------------------------------------------------------------------------------------------------------------
// M2: capped / aborted partitions presented as complete with pruned regions Covered
// ------------------------------------------------------------------------------------------------------------------

#[test]
fn m2_capped_partition_is_partial_and_unexplored() {
    let n = 3;
    let f = NodeFrame::without_header_lines_first(n, 3);
    let l = list(vec![dom(n, 3, &["n0-1"], &[])], n); // covers only n0 = 1
    let o = PartitionOptions {
        max_off_floor: Some(0),
        ..ordinary()
    };
    let rep = gap_partition(&l, &f, &o, None).unwrap();
    assert!(rep.uncovered(&[2, 2, 2]).unwrap());
    assert_eq!(rep.class_of(&[2, 2, 2]).unwrap(), ClassHit::Unexplored);
    assert_eq!(
        rep.outcome,
        Outcome::Partial {
            explored: Explored::OffFloorUpTo(0),
            reasons: vec![]
        }
    );
    assert_eq!(rep.to_json(&Default::default(), &f)["outcome"], "partial");
    assert!(rep.n_unexplored() > 0);
    assert!(rep.proves_no_gap(CoverageNotion::UnionCoverage).is_err());
}

#[test]
fn m2_budget_abort_is_partial_aborted() {
    let n = 7;
    let f = NodeFrame::without_header_lines_first(n, 3);
    let ds: Vec<CaseDomain> = (0..40)
        .map(|i| {
            let s = i % n;
            let mut d = dom(
                n,
                3,
                &[],
                &[&[&format!("n{}-n{}-{}", s, (s + 1) % n, i % 3)]],
            );
            d.fixed[(s + 2) % n] = Some(if (s + 2) % n < 3 {
                1 + (i % 2) as i32
            } else {
                -((i % 2) as i32)
            });
            d
        })
        .collect();
    let l = list(ds, n);
    let o = PartitionOptions {
        budget: Budget {
            max_nodes: Some(1),
            ..Default::default()
        },
        memo: false,
        ..ordinary()
    };
    let rep = gap_partition(&l, &f, &o, None).unwrap();
    if rep.stats.aborted {
        assert!(rep.outcome.is_aborted(), "{:?}", rep.outcome);
        assert_eq!(
            rep.to_json(&Default::default(), &f)["outcome"],
            "budget_exceeded"
        );
        assert!(rep.proves_no_gap(CoverageNotion::UnionCoverage).is_err());
        // no key is reported Covered where the run did not look
        for k in window(n, 3, 2) {
            if rep.uncovered(&k).unwrap() {
                assert_ne!(rep.class_of(&k).unwrap(), ClassHit::Covered, "{k:?}");
            }
        }
    }
}

#[test]
fn m2_levels_report_which_levels_are_proved() {
    use rustred_campaign_core::partition::gap_partition_levels;
    let n = 3;
    let f = NodeFrame::without_header_lines_first(n, 3);
    let l = list(vec![dom(n, 3, &["n0-1"], &[])], n);
    let lv = gap_partition_levels(&l, &f, &ordinary(), 3, None, None).unwrap();
    assert_eq!(lv.len(), 4);
    assert!(lv.iter().all(|x| x.proved && x.incomplete.is_empty()));
}

// ------------------------------------------------------------------------------------------------------------------
// M3: union coverage vs first-match dispatch
// ------------------------------------------------------------------------------------------------------------------

fn pole_tables() -> (CaseDomain, CaseDomain) {
    let n = 2;
    let mut a = CaseDomain::free(n, Some(vec![true, true]));
    a.poles.push(PoleLocus::new(
        PoleKind::Denominator,
        &Poly::parse("(n0-2)*d", n, true).unwrap(),
    ));
    let b = CaseDomain::free(n, Some(vec![true, true]));
    (a, b)
}

#[test]
fn m3_pole_blocking_modelled_in_dispatch_mode() {
    let n = 2;
    let f = NodeFrame::without_header_lines_first(n, 2);
    let (a, b) = pole_tables();
    let mut t = Table::new("t", n);
    t.push(0, a.clone()).unwrap();
    t.push(1, b.clone()).unwrap();
    let disp = Dispatcher::new(vec![t], vec![0]).unwrap();
    assert!(matches!(
        disp.first_match(&[2, 1]).unwrap(),
        Dispatch::Blocked { .. }
    ));
    let l = list(vec![a.clone(), b.clone()], n);
    // union mode: covered (that is what union coverage says) and it says so
    let u = gap_partition(&l, &f, &ordinary(), None).unwrap();
    assert!(!u.uncovered(&[2, 1]).unwrap());
    assert_eq!(u.coverage, CoverageNotion::UnionCoverage);
    assert!(u.proves_no_gap(CoverageNotion::UnionCoverage).is_ok());
    assert!(u.proves_no_gap(CoverageNotion::DispatchCoverage).is_err());
    // dispatch mode: the key is open, key by key equal to the dispatcher
    let o = PartitionOptions {
        coverage: CoverageNotion::DispatchCoverage,
        ..Default::default()
    };
    let d = gap_partition(&l, &f, &o, None).unwrap();
    for k in window(n, 2, 8) {
        let open = disp.first_match(&k).unwrap().is_open();
        assert_eq!(d.uncovered(&k).unwrap(), open, "{k:?}");
        if open {
            assert_ne!(d.class_of(&k).unwrap(), ClassHit::Covered, "{k:?}");
            assert!(
                d.faces.iter().any(|x| x.key() == face_of(&d.nf.lines, &k)),
                "{k:?}"
            );
        }
    }
    assert!(d.proves_no_gap(CoverageNotion::DispatchCoverage).is_err());
    assert_eq!(d.to_json(&Default::default(), &f)["coverage"], "dispatch");
}

/// Dispatch mode against the dispatcher on random tables with poles, recenter bounds and elided records.
#[test]
fn m3_dispatch_mode_equals_first_match_random() {
    for seed in 0..40u64 {
        let mut g = Lcg(seed * 13 + 7);
        let n = 3;
        let e = 2;
        let mask: Vec<bool> = (0..n).map(|i| i < e).collect();
        let mut t = Table::new("t", n);
        let mut ds = vec![];
        for r in 0..(2 + g.below(5)) {
            let mut d = CaseDomain::free(n, Some(mask.clone()));
            if g.below(3) == 0 {
                let s = g.below(n as u64) as usize;
                d.fixed[s] = Some(if s < e {
                    1 + g.below(2) as i32
                } else {
                    -(g.below(2) as i32)
                });
            }
            if g.below(3) == 0 {
                let (a, b) = (g.below(n as u64), g.below(n as u64));
                d.exceptions.push(vec![
                    Poly::parse(&format!("n{a}-n{b}-({})", g.below(3) as i64 - 1), n, false)
                        .unwrap(),
                ]);
            }
            if g.below(2) == 0 {
                let s = g.below(n as u64);
                let c = g.below(4) as i64 - 1;
                d.poles.push(PoleLocus::new(
                    PoleKind::Denominator,
                    &Poly::parse(&format!("(n{s}-({c}))*d+n{s}-({c})"), n, true).unwrap(),
                ));
            }
            if g.below(4) == 0 {
                let s = g.below(e as u64);
                d.recenter_bounds
                    .push(Poly::parse(&format!("n{s}-{}", 2 + g.below(2)), n, false).unwrap());
            }
            if g.below(6) == 0 {
                d.witness_elided = true;
            }
            t.push(r as i64, d.clone()).unwrap();
            ds.push(d);
        }
        let disp = Dispatcher::new(vec![t], vec![0]).unwrap();
        let l = list(ds, n);
        let f = NodeFrame::without_header_lines_first(n, e);
        let o = PartitionOptions {
            coverage: CoverageNotion::DispatchCoverage,
            ..Default::default()
        };
        let rep = gap_partition(&l, &f, &o, None).unwrap();
        let faces: BTreeSet<_> = rep.faces.iter().map(|x| x.key()).collect();
        for k in window(n, e, 6) {
            let open = disp.first_match(&k).unwrap().is_open();
            assert_eq!(rep.uncovered(&k).unwrap(), open, "seed {seed} key {k:?}");
            if open {
                assert_ne!(
                    rep.class_of(&k).unwrap(),
                    ClassHit::Covered,
                    "seed {seed} {k:?}"
                );
                assert!(
                    faces.contains(&face_of(&rep.nf.lines, &k)),
                    "seed {seed} {k:?}"
                );
            }
        }
    }
}

/// audit-part-A M3 (c), with re-audit ladder-A M3 (lane ladderfix2): a FULL-node proof is minted ONLY by
/// `economy::prove_coverage_preserved`, which runs the DISPATCH-mode partition of the new table set itself (whatever
/// coverage the options name) and binds both table-set digests; `Retirement::new` accepts it on a FULL node; a new
/// table set with a gap (a blocked pole key) or an empty new-order digest gives none. (The former public
/// `assemble::containment_proof(report, ..)` minted a proof from a caller-supplied report and is gone.)
#[test]
fn m3_economy_requires_a_dispatch_proof() {
    use rustred_campaign_core::economy::{prove_coverage_preserved, table_set_digest};
    let m = |t: u64| RuleMeasure {
        cases: 1,
        rhs_terms: t,
        monomials: t,
        monomials_estimated: false,
        max_degree: None,
        served_keys: None,
        bytes: 1,
    };
    let old = vec![("a".to_string(), m(100))];
    let retire = |p: Option<ContainmentProof>| {
        Retirement::new(
            "N",
            CoverageState::Full,
            &old,
            ("c", &m(50)),
            SupersedeReason::FaceRetiresPoints,
            p,
        )
    };
    assert_eq!(retire(None).unwrap_err(), RetireError::FullNodeNeedsProof);
    let n = 2;
    let f = NodeFrame::without_header_lines_first(n, 2);
    // one record covering the whole chart: no gap under either notion
    let l = list(vec![CaseDomain::free(n, Some(vec![true, true]))], n);
    let u = gap_partition(&l, &f, &ordinary(), None).unwrap();
    assert!(u.proves_no_gap(CoverageNotion::UnionCoverage).is_ok());
    // the checking function runs the DISPATCH-mode partition even when the options name union coverage
    let p = prove_coverage_preserved(&l, &l, &f, &ordinary(), &DispatchOrderStatement::SameOrder).unwrap();
    assert_eq!(
        p.method(),
        dispatch_proof_method(&DispatchOrderStatement::SameOrder)
    );
    assert_eq!(p.new_tables_digest(), table_set_digest(&l));
    let r = retire(Some(p)).unwrap();
    assert!(r.check_tables(&l, &l).is_ok());
    let p2 = prove_coverage_preserved(
        &l,
        &l,
        &f,
        &PartitionOptions::default(),
        &DispatchOrderStatement::NewOrder("abc123".into()),
    )
    .unwrap();
    assert_eq!(
        parse_dispatch_proof_method(p2.method()),
        Some(DispatchOrderStatement::NewOrder("abc123".into()))
    );
    assert!(retire(Some(p2)).is_ok());
    assert!(
        prove_coverage_preserved(
            &l,
            &l,
            &f,
            &PartitionOptions::default(),
            &DispatchOrderStatement::NewOrder(String::new())
        )
        .is_err()
    );
    // a new table set WITH a gap (blocked pole key) gives none; a retirement bound to other tables is refused
    let (a, b) = pole_tables();
    let gap = list(vec![a, b], n);
    assert!(
        prove_coverage_preserved(&l, &gap, &f, &PartitionOptions::default(), &DispatchOrderStatement::SameOrder)
            .is_err()
    );
    assert_eq!(
        r.check_tables(&l, &gap).unwrap_err(),
        RetireError::ProofMismatch
    );
    assert!(parse_dispatch_proof_method("dispatch_partition.v1;order=new:").is_none());
}

#[test]
fn m3_removal_preserves_dispatch_only_without_blocked_set() {
    let n = 2;
    let f = NodeFrame::without_header_lines_first(n, 2);
    let (a, b) = pole_tables();
    let all = CaseDomain::free(n, Some(vec![true, true]));
    // b after `all`: shadowed and no blocked set -> removal preserves dispatch
    let l = list(vec![all.clone(), b.clone(), a.clone()], n);
    assert!(matches!(
        removal_preserves_dispatch(&l, 1, &f),
        Containment::Proved { .. }
    ));
    // a (pole) after `all`: shadowed, but it has a blocked set -> Unknown (not offered as a proof)
    assert!(matches!(shadowed(&l, 2, &f), Containment::Proved { .. }));
    assert!(matches!(
        removal_preserves_dispatch(&l, 2, &f),
        Containment::Unknown { .. }
    ));
    // shadowed counts an earlier record that is selected but BLOCKED (a's pole): b never fires on n0 = 2 either
    let l2 = list(vec![a.clone(), dom(n, 2, &["n0-2"], &[])], n);
    assert!(matches!(shadowed(&l2, 1, &f), Containment::Proved { .. }));
}

// ------------------------------------------------------------------------------------------------------------------
// M4: more than 24 `any` slots silently truncated
// ------------------------------------------------------------------------------------------------------------------

/// Arity 27 (synthetic): one rule fixes n0 = 1, so the U class {n0 off floor} has 26 `any` slots: it is listed as
/// a face family and the outcome says so (it was silently truncated to 2^24 combinations).
#[test]
fn m4_arity_27_more_than_24_free_slots_is_a_family() {
    let n = 27;
    let e = 14;
    let f = NodeFrame::without_header_lines_first(n, e);
    let mut d = CaseDomain::free(n, Some((0..n).map(|i| i < e).collect()));
    d.fixed[0] = Some(1);
    let l = list(vec![d], n);
    let rep = gap_partition(&l, &f, &ordinary(), None).unwrap();
    assert_eq!(rep.families.len(), 1);
    assert_eq!(rep.families[0].free.len(), 26);
    assert_eq!(rep.families[0].base_sd, vec![0]);
    assert!(
        matches!(rep.outcome, Outcome::Incomplete { .. }),
        "{:?}",
        rep.outcome
    );
    let mut k: Key = (0..n).map(|i| if i < e { 1 } else { 0 }).collect();
    k[0] = 2;
    k[26] = -5;
    assert!(rep.uncovered(&k).unwrap());
    assert_eq!(rep.class_of(&k).unwrap(), ClassHit::Uncovered);
    assert_eq!(
        rep.to_json(&Default::default(), &f)["face_families"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

/// Arity 27: rules n_i = 1 for i < 22 leave the U class {n0..n21 off floor} with the 5 `any` slots 22..26 (slot
/// indices above 24): expanded into its 32 faces, each with every off-floor combination of the high slots.
#[test]
fn m4_arity_27_expansion_uses_high_slots() {
    let n = 27;
    let e = 24;
    let f = NodeFrame::without_header_lines_first(n, e);
    let ds: Vec<CaseDomain> = (0..22)
        .map(|i| {
            let mut d = CaseDomain::free(n, Some((0..n).map(|s| s < e).collect()));
            d.fixed[i] = Some(1);
            d
        })
        .collect();
    let l = list(ds, n);
    let rep = gap_partition(&l, &f, &ordinary(), None).unwrap();
    assert!(rep.families.is_empty());
    assert_eq!(rep.faces.len(), 32);
    assert!(
        rep.faces
            .iter()
            .any(|x| x.sd.contains(&26) || x.sn.contains(&26))
    );
    assert!(rep.outcome.is_complete());
    // with the parameter below 5 the same class is a family (explicit, not truncated)
    let o = PartitionOptions {
        max_any_slots: 4,
        ..ordinary()
    };
    let rep = gap_partition(&l, &f, &o, None).unwrap();
    assert_eq!(rep.families.len(), 1);
    assert!(rep.faces.is_empty());
    assert!(matches!(rep.outcome, Outcome::Incomplete { .. }));
}

// ------------------------------------------------------------------------------------------------------------------
// minors
// ------------------------------------------------------------------------------------------------------------------

/// m2: integer roots beyond i32 (parameter space of the loci) were missed.
#[test]
fn m2_int_roots_beyond_i32() {
    assert_eq!(
        int_roots(&[-3_000_000_000, 1]).unwrap(),
        vec![3_000_000_000]
    );
    // m1: a cubic with a root near 2^42 (Fujiwara bound with large coefficients): either all roots or a typed
    // refusal (constant term above the trial-division cap), never a root set that misses one
    let r: i128 = (1 << 42) + 1;
    match int_roots(&[0, -(r * r * r), 0, 0, 1]) {
        Ok(roots) => assert!(roots.contains(&(r as i64)), "{roots:?}"),
        Err(e) => println!("refused (conservative): {e}"),
    }
    let r: i128 = 99_991; // prime, r^3 < 10^15: the divisor path
    assert!(
        int_roots(&[-(r * r * r), 0, 0, 1])
            .unwrap()
            .contains(&(r as i64))
    );
}

/// m3: the reference-compatible linear test used unchecked i128 arithmetic.
#[test]
fn m3_reference_compatible_arithmetic_is_checked() {
    use rustred_campaign_core::partition::classes::{Status, literal_status};
    use rustred_campaign_core::partition::exact::MPoly;
    let big: i128 = 1 << 100;
    let mut p = MPoly::var(2, 0).scale(big).unwrap();
    p = p.sub(&MPoly::var(2, 1).scale(big).unwrap()).unwrap();
    let r = Range {
        lo: Some(1 << 62),
        hi: None,
    };
    let (st, unknown) = literal_status(&p, &[(0, r), (1, r)], LiteralTests::ReferenceCompatible);
    assert_eq!((st, unknown), (Status::V, true));
}

// ------------------------------------------------------------------------------------------------------------------
// memoised == reference mode on UNCAPPED small tables (audit-part-A note)
// ------------------------------------------------------------------------------------------------------------------

fn coupled_table(seed: u64, n: usize, e: usize, rules: usize) -> DomainList {
    let mut g = Lcg(seed);
    let mask: Vec<bool> = (0..n).map(|i| i < e).collect();
    let mut l = DomainList::new(n);
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
        if g.below(3) == 0 {
            let (a, b) = (g.below(n as u64), g.below(n as u64));
            if a != b {
                d.equations.push(
                    Poly::parse(
                        &format!(
                            "n{a}-{}*n{b}-({})",
                            [1, 2, -1][g.below(3) as usize],
                            g.below(5) as i64 - 2
                        ),
                        n,
                        false,
                    )
                    .unwrap(),
                );
            }
        }
        for _ in 0..g.below(3) {
            let (a, b, c) = (g.below(n as u64), g.below(n as u64), g.below(7) as i64 - 3);
            let t = match g.below(4) {
                0 => format!("n{a}-({c})"),
                1 => format!("n{a}-n{b}-({c})"),
                2 => format!("n{a}+2*n{b}-({c})"),
                _ => format!("n{a}*n{b}-({c})"),
            };
            d.exceptions.push(vec![Poly::parse(&t, n, false).unwrap()]);
        }
        l.push(RuleRef::bare(0, r, r as i64), d);
    }
    l
}

#[test]
fn memo_equals_reference_uncapped_small_tables() {
    for seed in 0..10u64 {
        for (n, e, rules) in [(6usize, 3usize, 14usize), (7, 3, 16), (8, 4, 16)] {
            let l = coupled_table(seed * 104729 + 11, n, e, rules);
            let frame = NodeFrame::without_header_lines_first(n, e);
            let base = PartitionOptions {
                elided: ElidedPolicy::Ordinary,
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
            assert!(a.outcome.is_complete() || matches!(a.outcome, Outcome::Incomplete { .. }));
            let ca: BTreeSet<_> = a.classes.iter().cloned().collect();
            let cb: BTreeSet<_> = b.classes.iter().cloned().collect();
            assert_eq!(ca, cb, "seed {seed} arity {n}");
            let fa: BTreeSet<_> = a.faces.iter().map(|f| (f.key(), f.kind)).collect();
            let fb: BTreeSet<_> = b.faces.iter().map(|f| (f.key(), f.kind)).collect();
            assert_eq!(fa, fb, "seed {seed} arity {n}");
            assert_eq!(a.outcome, b.outcome);
        }
    }
}

// ------------------------------------------------------------------------------------------------------------------
// audit-part-B C1: completeness and soundness of the DESCRIPTION (faces, generic cells, locus pieces)
// ------------------------------------------------------------------------------------------------------------------

/// Exact integer parameters t with x0 + M t = key (Gauss-Jordan over the rationals, i128 fractions); `None` if
/// there is none (the parametrisations are injective).
fn solve_params(r: &rustred_campaign_core::loci::Region, key: &[i32]) -> Option<Vec<i128>> {
    fn g(a: i128, b: i128) -> i128 {
        let (mut a, mut b) = (a.abs(), b.abs());
        while b != 0 {
            let t = a % b;
            a = b;
            b = t;
        }
        a
    }
    let k = r.k;
    let n = r.x.len();
    // rows: M_i | key_i - c_i, as fractions with a common row denominator (integer rows, fraction-free)
    let mut rows: Vec<Vec<i128>> = (0..n)
        .map(|i| {
            let mut v: Vec<i128> = r.x[i].coef.clone();
            v.push(key[i] as i128 - r.x[i].c);
            v
        })
        .collect();
    let mut piv = vec![];
    let mut row = 0;
    for col in 0..k {
        let Some(p) = (row..n).find(|&i| rows[i][col] != 0) else {
            continue;
        };
        rows.swap(row, p);
        for i in 0..n {
            if i != row && rows[i][col] != 0 {
                let (a, b) = (rows[row][col], rows[i][col]);
                let pivot_row = rows[row].clone();
                for (dst, src) in rows[i].iter_mut().zip(&pivot_row) {
                    *dst = *dst * a - src * b;
                }
                let gg = rows[i].iter().fold(0, |acc, &x| g(acc, x));
                if gg > 1 {
                    for x in rows[i].iter_mut() {
                        *x /= gg;
                    }
                }
            }
        }
        piv.push(col);
        row += 1;
    }
    if rows[row..].iter().any(|r| r[k] != 0) {
        return None;
    }
    if piv.len() < k {
        return None; // not injective (never for the crate's regions)
    }
    let mut t = vec![0i128; k];
    for (i, &c) in piv.iter().enumerate() {
        if rows[i][k] % rows[i][c] != 0 {
            return None;
        }
        t[c] = rows[i][k] / rows[i][c];
    }
    (r.at(&t).ok()? == key.iter().map(|&v| v as i128).collect::<Vec<_>>()).then_some(t)
}

/// Is the uncovered key described: on a listed face AND in a listed generic cell (U class) or in a locus piece /
/// unresolved region of its face (F class)?
fn described(
    rep: &rustred_campaign_core::partition::PartitionReport,
    k: &[i32],
) -> Result<(), String> {
    use rustred_campaign_core::partition::ANY_OFF;
    let fk = face_of(&rep.nf.lines, k);
    let Some(face) = rep.faces.iter().find(|f| f.key() == fk) else {
        return Err(format!("face {fk:?} not listed"));
    };
    let slots: Vec<usize> = face.sd.iter().chain(&face.sn).copied().collect();
    let cell: Vec<u16> = slots
        .iter()
        .map(|&s| rep.state_index(s, k[s] as i64).unwrap())
        .collect();
    let in_cell = face.generic_cells.iter().any(|c| {
        c.iter()
            .zip(&cell)
            .all(|(&cs, &st)| cs == st || (cs == ANY_OFF && st != 0))
    });
    if in_cell {
        return Ok(());
    }
    if let Some(res) = &rep.resolution {
        for l in res.loci.iter().filter(|l| l.face == fk) {
            for p in &l.pieces {
                if let Some(t) = solve_params(&p.region, k)
                    && p.region.contains_t(&t).unwrap()
                {
                    return Ok(()); // on the region (the minus hyperplanes hold covered keys only)
                }
            }
            for u in &l.unresolved_parts {
                if let Some(t) = solve_params(&u.region, k)
                    && u.region.contains_t(&t).unwrap()
                {
                    return Ok(());
                }
            }
        }
    }
    if rep.class_of(k).unwrap() == ClassHit::Uncovered {
        return Err(format!("U key not in a generic cell of face {fk:?}"));
    }
    Err(format!("F key on no locus piece of face {fk:?}"))
}

/// Completeness (every uncovered window key described) and soundness (every piece point found in the window off
/// its minus hyperplanes, and every witness, is uncovered unless the piece is flagged).
fn assert_description(
    l: &DomainList,
    rep: &rustred_campaign_core::partition::PartitionReport,
    n: usize,
    e: usize,
    w: i32,
    what: &str,
) {
    for k in window(n, e, w) {
        let cov = l
            .items
            .iter()
            .any(|(_, d)| d.applies(&k) == Verdict::Applies);
        assert_eq!(rep.uncovered(&k).unwrap(), !cov, "{what}: oracle at {k:?}");
        if !cov && let Err(why) = described(rep, &k) {
            panic!("{what}: uncovered {k:?} not described: {why}");
        }
    }
    if let Some(res) = &rep.resolution {
        for loc in &res.loci {
            for p in &loc.pieces {
                for wk in &p.witness {
                    assert!(rep.uncovered(wk).unwrap(), "{what}: witness {wk:?} covered");
                }
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------------------------
// audit-part-B: its saved minimal cases (runs/*.json of the harness, embedded; the audit was still running)
// ------------------------------------------------------------------------------------------------------------------

const AUDIT_B_CASES: &[(&str, &str)] = &[
    (
        "k2",
        r#"{"arity":4,"mask":"1100","rules":[{"fixed":[null,null,null,null],"eqs":["n3+1"],"bounds":[],"exc":[]},{"fixed":[null,null,null,null],"eqs":[],"bounds":[],"exc":[["n0-n1"]]},{"fixed":[null,null,null,null],"eqs":["n2+1"],"bounds":[],"exc":[]}]}"#,
    ),
    (
        "g4",
        r#"{"arity":4,"mask":"1100","rules":[{"fixed":[null,null,null,null],"eqs":["n3+1"],"bounds":[],"exc":[]},{"fixed":[null,null,null,null],"eqs":[],"bounds":[],"exc":[["n0*n1-6"]]},{"fixed":[null,null,null,null],"eqs":["n2+1"],"bounds":[],"exc":[]}]}"#,
    ),
    (
        "min_iv_miss1",
        r#"{"arity":3,"mask":"110","rules":[{"bounds":[],"elided":false,"eqs":[],"exc":[["-10+3*n1+n1^2+2*n0-n0*n1"]],"fixed":[null,3,null]},{"bounds":[],"elided":false,"eqs":["36*n2+96*n2^2+97*n2^3+47*n2^4+11*n2^5+n2^6"],"exc":[],"fixed":[null,null,null]},{"bounds":[],"elided":false,"eqs":["4-2*n2-2*n2^2-5*n0-n0*n2+n0^2"],"exc":[],"fixed":[null,null,null]}]}"#,
    ),
    (
        "min_iv_miss2",
        r#"{"arity":3,"mask":"110","rules":[{"bounds":[],"elided":false,"eqs":[],"exc":[["-35+7*n2+14*n1+12*n0-n0*n2-2*n0*n1-n0^2"]],"fixed":[null,null,null]},{"bounds":[],"elided":false,"eqs":["-7+7*n2+n1-n1*n2+7*n0-n0*n1"],"exc":[["4+n1+n0","15*n2+26*n2^2+13*n2^3+2*n2^4","-6+n1"]],"fixed":[null,null,null]},{"bounds":[],"elided":false,"eqs":[],"exc":[["112-24*n1-9*n1^2+2*n1^3"]],"fixed":[null,null,-2]},{"bounds":[],"elided":false,"eqs":["-2+n1"],"exc":[],"fixed":[null,null,null]}]}"#,
    ),
    (
        "c1",
        r#"{"arity":3,"mask":"110","rules":[{"bounds":[],"eqs":[],"exc":[],"fixed":[null,null,null]},{"bounds":[],"eqs":["n1^2-3*n1+2"],"exc":[],"fixed":[null,null,null]}]}"#,
    ),
    (
        "c1",
        r#"{"arity":3,"mask":"110","rules":[{"bounds":[],"eqs":[],"exc":[],"fixed":[null,null,null]},{"bounds":[],"eqs":["n1^2-3*n1+2"],"exc":[],"fixed":[null,null,null]}]}"#,
    ),
    (
        "c2",
        r#"{"arity":3,"mask":"110","rules":[{"bounds":[],"eqs":["n1^2-3*n1+2"],"exc":[],"fixed":[null,null,null]}]}"#,
    ),
    (
        "c3",
        r#"{"arity":3,"mask":"110","rules":[{"bounds":[],"eqs":[],"exc":[["n1^2-3*n1+2"]],"fixed":[null,null,null]},{"bounds":[],"eqs":["n1-1"],"exc":[],"fixed":[null,null,null]}]}"#,
    ),
    (
        "g1",
        r#"{"arity":4,"mask":"1100","rules":[{"fixed":[null,null,null,null],"eqs":["n3+1"],"bounds":[],"exc":[]},{"fixed":[null,null,null,null],"eqs":[],"bounds":[],"exc":[["n0*n1-6"]]},{"fixed":[null,null,null,null],"eqs":["n2"],"bounds":[],"exc":[]}]}"#,
    ),
    (
        "g2",
        r#"{"arity":4,"mask":"1100","rules":[{"fixed":[null,null,null,null],"eqs":["n3+1"],"bounds":[],"exc":[]},{"fixed":[null,null,null,null],"eqs":[],"bounds":[],"exc":[["n0+n1-7"]]},{"fixed":[null,null,null,null],"eqs":["n2"],"bounds":[],"exc":[]}]}"#,
    ),
    (
        "g3",
        r#"{"arity":4,"mask":"1100","rules":[{"fixed":[null,null,null,null],"eqs":["n3+1"],"bounds":[],"exc":[]},{"fixed":[null,null,null,null],"eqs":[],"bounds":[],"exc":[["n0-6"]]},{"fixed":[null,null,null,null],"eqs":["n2"],"bounds":[],"exc":[]}]}"#,
    ),
    (
        "g4",
        r#"{"arity":4,"mask":"1100","rules":[{"fixed":[null,null,null,null],"eqs":["n3+1"],"bounds":[],"exc":[]},{"fixed":[null,null,null,null],"eqs":[],"bounds":[],"exc":[["n0*n1-6"]]},{"fixed":[null,null,null,null],"eqs":["n2+1"],"bounds":[],"exc":[]}]}"#,
    ),
    (
        "g5",
        r#"{"arity":4,"mask":"1100","rules":[{"fixed":[null,null,null,null],"eqs":["n3"],"bounds":[],"exc":[]},{"fixed":[null,null,null,null],"eqs":[],"bounds":[],"exc":[["n0*n1-6"]]},{"fixed":[null,null,null,null],"eqs":["n2"],"bounds":[],"exc":[]}]}"#,
    ),
    (
        "g6",
        r#"{"arity":4,"mask":"1100","rules":[{"fixed":[null,null,null,null],"eqs":["n3+1"],"bounds":[],"exc":[]},{"fixed":[null,null,null,null],"eqs":[],"bounds":[],"exc":[["n1-6"]]},{"fixed":[null,null,null,null],"eqs":["n2+1"],"bounds":[],"exc":[]}]}"#,
    ),
    (
        "g7",
        r#"{"arity":4,"mask":"1100","rules":[{"fixed":[null,null,null,null],"eqs":["n3+1"],"bounds":[],"exc":[]},{"fixed":[null,null,null,null],"eqs":[],"bounds":[],"exc":[["n0-n1"]]},{"fixed":[null,null,null,null],"eqs":["n2"],"bounds":[],"exc":[]}]}"#,
    ),
    (
        "h1",
        r#"{"arity":3,"mask":"110","rules":[{"fixed":[null,null,null],"eqs":["n2+1"],"bounds":[],"exc":[]},{"fixed":[null,null,null],"eqs":[],"bounds":[],"exc":[["n0*n1-6"]]}]}"#,
    ),
    (
        "h2",
        r#"{"arity":3,"mask":"110","rules":[{"fixed":[null,null,null],"eqs":["n2+1"],"bounds":[],"exc":[]},{"fixed":[null,null,null],"eqs":[],"bounds":[],"exc":[["n0+n1-7"]]}]}"#,
    ),
    (
        "h3",
        r#"{"arity":3,"mask":"110","rules":[{"fixed":[null,null,null],"eqs":["n2"],"bounds":[],"exc":[]},{"fixed":[null,null,null],"eqs":[],"bounds":[],"exc":[["n0*n1-6"]]}]}"#,
    ),
    (
        "s",
        r#"{"arity":3,"mask":"110","rules":[{"bounds":[],"eqs":["24-50*n1+35*n1^2-10*n1^3+n1^4"],"exc":[],"fixed":[null,null,null]},{"bounds":[],"eqs":[],"exc":[],"fixed":[null,null,null]}]}"#,
    ),
    (
        "sh2",
        r#"{"arity":3,"mask":"110","rules":[{"bounds":[],"eqs":["n1-2"],"exc":[],"fixed":[null,null,null]},{"bounds":[],"eqs":[],"exc":[],"fixed":[null,null,0]}]}"#,
    ),
    (
        "sh3",
        r#"{"arity":3,"mask":"110","rules":[{"bounds":[],"eqs":[],"exc":[],"fixed":[1,null,null]},{"bounds":[],"eqs":[],"exc":[],"fixed":[null,null,null]}]}"#,
    ),
    (
        "shadow_hang",
        r#"{"arity":5,"mask":"11110","rules":[{"bounds":[],"eqs":["24-50*n1+35*n1^2-10*n1^3+n1^4"],"exc":[],"fixed":[null,null,null,null,null]},{"bounds":[],"eqs":["-3+n2"],"exc":[["-36*n2^2+33*n2^3-10*n2^4+n2^5"]],"fixed":[null,null,null,null,null]},{"bounds":[],"eqs":[],"exc":[["-4*n4+n4^2"]],"fixed":[1,null,null,null,0]},{"bounds":[],"eqs":[],"exc":[],"fixed":[4,null,null,2,0]},{"bounds":[],"eqs":[],"exc":[],"fixed":[null,null,null,null,0]}]}"#,
    ),
    (
        "shadow_hang_min",
        r#"{"arity":5,"mask":"11110","rules":[{"bounds":[],"eqs":["24-50*n1+35*n1^2-10*n1^3+n1^4"],"exc":[],"fixed":[null,null,null,null,null]},{"bounds":[],"eqs":[],"exc":[],"fixed":[null,null,null,null,null]}]}"#,
    ),
];

fn audit_b_table(js: &str) -> (DomainList, NodeFrame, usize, usize) {
    let v: serde_json::Value = serde_json::from_str(js).unwrap();
    let n = v["arity"].as_u64().unwrap() as usize;
    let mask: Vec<bool> = v["mask"]
        .as_str()
        .unwrap()
        .chars()
        .map(|c| c == '1')
        .collect();
    let e = mask.iter().filter(|&&b| b).count();
    assert!(mask[..e].iter().all(|&b| b), "lines first");
    let p = |t: &serde_json::Value| Poly::parse(t.as_str().unwrap(), n, false).unwrap();
    let ds = v["rules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let mut d = CaseDomain::free(n, Some(mask.clone()));
            for (s, x) in r["fixed"].as_array().unwrap().iter().enumerate() {
                d.fixed[s] = x.as_i64().map(|y| y as i32);
            }
            d.equations = r["eqs"].as_array().unwrap().iter().map(p).collect();
            d.bounds = r["bounds"].as_array().unwrap().iter().map(p).collect();
            d.exceptions = r["exc"]
                .as_array()
                .unwrap()
                .iter()
                .map(|b| b.as_array().unwrap().iter().map(p).collect())
                .collect();
            d
        })
        .collect();
    (
        list(ds, n),
        NodeFrame::without_header_lines_first(n, e),
        n,
        e,
    )
}

#[test]
fn audit_b_saved_cases_faces_complete_and_proofs_terminate() {
    for (name, js) in AUDIT_B_CASES {
        let (l, f, n, e) = audit_b_table(js);
        let rep = gap_partition(&l, &f, &ordinary(), None).unwrap();
        println!(
            "{name}: faces {} outcome {:?}",
            rep.faces.len(),
            rep.outcome
        );
        assert_description(&l, &rep, n, e, if n <= 3 { 9 } else { 6 }, name);
        for i in 0..l.items.len() {
            let d = &l.items[i].1;
            match is_empty(d, &f) {
                Emptiness::ProvedEmpty { .. } => assert!(
                    !window(n, e, 6)
                        .iter()
                        .any(|k| d.applies(k) == Verdict::Applies),
                    "{name}: rule {i} proved empty"
                ),
                Emptiness::NonEmpty { witness } => {
                    assert_eq!(d.applies(&witness), Verdict::Applies)
                }
                Emptiness::Unknown { .. } => {}
            }
            if let Containment::Proved { .. } = shadowed(&l, i, &f) {
                for k in window(n, e, 6) {
                    if d.applies(&k) == Verdict::Applies {
                        assert!(
                            (0..i).any(|j| l.items[j].1.applies(&k).stage() != Stage::NotSelected),
                            "{name}: rule {i} proved shadowed but fires at {k:?}"
                        );
                    }
                }
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------------------------
// audit-part-B M2 / M3 / C2
// ------------------------------------------------------------------------------------------------------------------

/// B-M2: the levels checkpoint is rewritten atomically, and a torn last line (an old non-atomic append killed
/// mid-write) is dropped on resume instead of failing.
#[test]
fn b_m2_torn_checkpoint_line_resumes() {
    use rustred_campaign_core::partition::gap_partition_levels;
    let l = coupled_table(77, 6, 3, 12);
    let f = NodeFrame::without_header_lines_first(6, 3);
    let cp = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("partfix_torn_{}.jsonl", std::process::id()));
    let _ = std::fs::remove_file(&cp);
    let a = gap_partition_levels(&l, &f, &ordinary(), 2, Some(&cp), None).unwrap();
    assert_eq!(a.len(), 3);
    let mut text = std::fs::read_to_string(&cp).unwrap();
    text.push_str("{\"faces\":[[[],[4]],[[0],[");
    std::fs::write(&cp, text).unwrap();
    let b = gap_partition_levels(&l, &f, &ordinary(), 4, Some(&cp), None).unwrap();
    assert_eq!(b.len(), 5);
    let full = gap_partition_levels(&l, &f, &ordinary(), 4, None, None).unwrap();
    for (x, y) in b.iter().zip(&full) {
        assert_eq!(x.faces, y.faces, "level {}", x.level);
    }
    // the rewritten file parses line by line
    for line in std::fs::read_to_string(&cp).unwrap().lines() {
        serde_json::from_str::<serde_json::Value>(line).unwrap();
    }
    let _ = std::fs::remove_file(&cp);
}

/// B-M3: the off-floor cap bounds the face expansion (no face beyond the cap is listed) and the face budget stops
/// the face list with an explicit Incomplete outcome.
#[test]
fn b_m3_cap_and_face_budget_bound_the_face_list() {
    let n = 10;
    let e = 5;
    let f = NodeFrame::without_header_lines_first(n, e);
    // sparse: one rule fixing n0 = 1 leaves a U class with 9 any slots (512 faces uncapped)
    let mut d = CaseDomain::free(n, Some((0..n).map(|i| i < e).collect()));
    d.fixed[0] = Some(1);
    let l = list(vec![d], n);
    let full = gap_partition(&l, &f, &ordinary(), None).unwrap();
    assert_eq!(full.faces.len(), 512);
    let o = PartitionOptions {
        max_off_floor: Some(2),
        ..ordinary()
    };
    let capped = gap_partition(&l, &f, &o, None).unwrap();
    assert!(capped.faces.iter().all(|x| x.sd.len() + x.sn.len() <= 2));
    assert_eq!(capped.faces.len(), 1 + 9); // {0} and {0, s}
    let o = PartitionOptions {
        budget: Budget {
            max_faces: Some(100),
            ..Default::default()
        },
        ..ordinary()
    };
    let b = gap_partition(&l, &f, &o, None).unwrap();
    assert!(b.faces_truncated);
    assert!(matches!(b.outcome, Outcome::Incomplete { .. }));
}

/// B-C2: every proof function takes a budget and answers Unknown beyond it (never a hang, never a verdict).
#[test]
fn b_c2_proof_budget_gives_unknown() {
    use rustred_campaign_core::partition::{ProofBudget, contained_in_with};
    let n = 3;
    let f = NodeFrame::without_header_lines_first(n, 2);
    let free = CaseDomain::free(n, Some(vec![true, true, false]));
    let others: Vec<CaseDomain> = (0..6)
        .map(|i| {
            dom(
                n,
                2,
                &[],
                &[&[&format!("n0-n1-{i}")], &[&format!("n0+n2-{}", i + 2)]],
            )
        })
        .collect();
    let os: Vec<&CaseDomain> = others.iter().collect();
    let tiny = ProofBudget {
        max_nodes: 2,
        ..Default::default()
    };
    assert!(matches!(
        contained_in_with(&free, &os, &f, &tiny),
        Containment::Unknown { .. }
    ));
    // the audit's hang: a free domain behind {n1^2 - 3 n1 + 2 = 0} decides at once (roots 1, 2)
    let l = list(vec![dom(n, 2, &["n1^2-3*n1+2"], &[]), free.clone()], n);
    assert!(matches!(
        shadowed(&l, 1, &f),
        Containment::NotContained { .. }
    ));
    assert!(matches!(
        is_empty(&l.items[0].1, &f),
        Emptiness::NonEmpty { .. }
    ));
}
