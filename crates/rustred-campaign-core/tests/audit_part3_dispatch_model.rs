//! Re-audit part 3 (lane audit-part3): the DISPATCH-mode partition vs the crate's own first-match dispatcher
//! (`cover::Dispatcher::first_match`), and the one minting point of a FULL-node containment proof
//! (`economy::prove_coverage_preserved`). The auditor's tests asserted the defects found on campaign ebe48378; lane
//! partfix3 INVERTED each of them (they now assert the correct behaviour: no proof, the oracle reports the key open)
//! and added the cases the stop-set model must also cover (a record dropped as never applying still stops dispatch;
//! a risky record that is never reached does not block a proof; the cross-check's probe keys hit every open key of
//! the three audit tables). Synthetic tables only (lane rule L8).

use rustred_campaign_core::cover::{Dispatch, Dispatcher, RuleRef, Table};
use rustred_campaign_core::domain::{CaseDomain, TargetMap, TargetSlot};
use rustred_campaign_core::economy::{ProofWork, prove_coverage_preserved, table_set_digest};
use rustred_campaign_core::key::NodeFrame;
use rustred_campaign_core::partition::{
    CoverageNotion, DispatchOrderStatement, DispatcherSource, DomainList, Explored, Outcome,
    PartitionOptions, gap_partition,
};
use rustred_campaign_core::poly::{ENGINE_PRIMES, Poly};

fn chart(n: usize, e: usize) -> Option<Vec<bool>> {
    Some((0..n).map(|i| i < e).collect())
}

fn disp(ds: &[CaseDomain], n: usize) -> Dispatcher {
    let mut t = Table::new("t", n);
    for (i, d) in ds.iter().enumerate() {
        t.push(i as i64, d.clone()).unwrap();
    }
    Dispatcher::new(vec![t], vec![0]).unwrap()
}

fn list(ds: &[CaseDomain], n: usize) -> DomainList {
    let mut l = DomainList::new(n);
    for (i, d) in ds.iter().enumerate() {
        l.push(RuleRef::bare(0, i, i as i64), d.clone());
    }
    l
}

fn dispatch_opts() -> PartitionOptions {
    PartitionOptions {
        coverage: CoverageNotion::DispatchCoverage,
        ..Default::default()
    }
}

fn open(d: &Dispatcher, k: &[i32]) -> bool {
    !matches!(d.first_match(k).unwrap(), Dispatch::Covered(_))
}

fn mint(d: &Dispatcher, f: &NodeFrame) -> Result<(), String> {
    let src = DispatcherSource::new(d);
    prove_coverage_preserved(
        &src,
        &src,
        f,
        &ProofWork::default(),
        &DispatchOrderStatement::SameOrder,
    )
    .map(|_| ())
}

/// The cross-check's probe keys (built from the dispatch-mode report) contain a key the dispatcher reports open.
fn probes_hit_open(d: &Dispatcher, f: &NodeFrame) -> bool {
    let src = DispatcherSource::new(d);
    let rep = gap_partition(&src, f, &dispatch_opts(), None).unwrap();
    let meter = rustred_campaign_core::partition::WorkMeter::unlimited();
    rep.dispatch_probes(&meter).iter().any(|k| open(d, k))
}

/// P3-C1 (CRITICAL), INVERTED by partfix3: the minting function takes NO partition options (a work budget only) and
/// builds its own, so a caller cannot narrow or reorder what is proved; and `proves_no_gap` refuses a SUBSET run by
/// construction (no `CompleteRun` token), whatever the dispatch order of the subset.
#[test]
fn p3_c1_rule_subset_never_mints_and_never_proves() {
    let n = 2;
    let f = NodeFrame::without_header_lines_first(n, 1);
    let mut blocker = CaseDomain::free(n, chart(n, 1));
    blocker.witness_elided = true; // selected everywhere, never usable
    let all = CaseDomain::free(n, chart(n, 1));
    let ds = vec![blocker, all];
    let d = disp(&ds, n);
    let src = DispatcherSource::new(&d);
    for k in [[1, 0], [2, -3], [40, -40]] {
        assert!(
            matches!(d.first_match(&k).unwrap(), Dispatch::Blocked { .. }),
            "every key is blocked by record 0"
        );
    }
    assert!(mint(&d, &f).is_err(), "the honest call refuses");
    for subset in [vec![1], vec![1, 0]] {
        let o = PartitionOptions {
            rule_subset: Some(subset.clone()),
            ..dispatch_opts()
        };
        let rep = gap_partition(&src, &f, &o, None).unwrap();
        let why = rep
            .proves_no_gap(CoverageNotion::DispatchCoverage)
            .expect_err("a subset run never proves no gap");
        assert!(why.contains("subset"), "{why}");
        assert!(rep.complete_run().is_err());
    }
    assert!(probes_hit_open(&d, &f));
}

/// P3-C2 (CRITICAL), INVERTED by partfix3: the engine's modular zero test (dispatcher `Undecided`) is modelled: a
/// record whose equation / exception member / pole coefficient can reach a nonzero multiple of p1*p2 STOPS dispatch
/// on its fixed-slot / target-constant set (an over-approximation, sound for a proof). No proof, the oracle reports
/// the keys open.
#[test]
fn p3_c2_engine_zero_test_is_a_stop_and_never_proved() {
    let n = 2;
    let f = NodeFrame::without_header_lines_first(n, 1);
    let pp = i128::from(ENGINE_PRIMES[0]) * i128::from(ENGINE_PRIMES[1]);
    // (a) one record, exception member p1*p2*n0
    let mut r = CaseDomain::free(n, chart(n, 1));
    r.exceptions = vec![vec![Poly::parse(&format!("{pp}*n0"), n, false).unwrap()]];
    let ds = vec![r];
    let d = disp(&ds, n);
    for k in [[1, 0], [7, -2], [1000, -1000]] {
        assert!(
            matches!(d.first_match(&k).unwrap(), Dispatch::Undecided { .. }),
            "{k:?}: the dispatcher reads the engine zero test as undecided"
        );
    }
    let src = DispatcherSource::new(&d);
    let rep = gap_partition(&src, &f, &dispatch_opts(), None).unwrap();
    assert!(rep.proves_no_gap(CoverageNotion::DispatchCoverage).is_err());
    assert!(rep.uncovered(&[1, 0]).unwrap());
    assert!(rep.uncovered(&[1000, -1000]).unwrap());
    assert!(mint(&d, &f).is_err());
    assert!(probes_hit_open(&d, &f));
    // (b) an EQUATION p1*p2*n0 = 0 on an earlier record: undecided for the dispatcher at every key
    let mut e = CaseDomain::free(n, chart(n, 1));
    e.equations = vec![Poly::parse(&format!("{pp}*n0"), n, false).unwrap()];
    let ds = vec![e, CaseDomain::free(n, chart(n, 1))];
    let d = disp(&ds, n);
    assert!(open(&d, &[3, -1]));
    let rep = gap_partition(&DispatcherSource::new(&d), &f, &dispatch_opts(), None).unwrap();
    assert!(rep.uncovered(&[3, -1]).unwrap());
    assert!(mint(&d, &f).is_err(), "no proof while every key is undecided");
    // (c) an equation that IS the constant p1*p2: the normal form drops the record as never applying, but the
    // dispatcher stops at it on every key (the stop piece is registered before any drop)
    let mut c = CaseDomain::free(n, chart(n, 1));
    c.equations = vec![Poly::constant(pp)];
    let ds = vec![c, CaseDomain::free(n, chart(n, 1))];
    let d = disp(&ds, n);
    assert!(open(&d, &[2, 0]));
    let rep = gap_partition(&DispatcherSource::new(&d), &f, &dispatch_opts(), None).unwrap();
    assert!(rep.uncovered(&[2, 0]).unwrap());
    assert!(mint(&d, &f).is_err());
    // (d) exception branches in ORDER: branch 0 holds the risky member, branch 1 always vanishes (the record never
    // applies), the dispatcher stops at branch 0 first
    let mut o = CaseDomain::free(n, chart(n, 1));
    o.exceptions = vec![
        vec![Poly::parse(&format!("{pp}*n0"), n, false).unwrap()],
        vec![],
    ];
    let ds = vec![o, CaseDomain::free(n, chart(n, 1))];
    let d = disp(&ds, n);
    assert!(open(&d, &[5, -5]));
    assert!(mint(&d, &f).is_err());
    // (e) a risky record that is NEVER reached (an earlier record serves every key): the proof stands
    let mut late = CaseDomain::free(n, chart(n, 1));
    late.exceptions = vec![vec![Poly::parse(&format!("{pp}*n0"), n, false).unwrap()]];
    let ds = vec![CaseDomain::free(n, chart(n, 1)), late];
    let d = disp(&ds, n);
    assert!(!open(&d, &[5, -5]));
    mint(&d, &f).expect("the risky record is never reached: dispatch coverage is proved");
}

/// P3-M1 (MAJOR), INVERTED by partfix3: the rule point of a target pattern is computed BEFORE the equations; the
/// normal form now carries the rule-point overflow as an exact stop piece `fixed & constants & (n_j - o outside
/// i32)`, so keys where the rule point overflows and an equation fails are open for the partition too.
#[test]
fn p3_m1_rule_point_overflow_before_the_equations_is_modelled() {
    let n = 2;
    let f = NodeFrame::without_header_lines_first(n, 1);
    let target = TargetMap::Pattern(vec![
        TargetSlot::Param {
            param: 0,
            offset: -2_147_483_000,
        },
        TargetSlot::Param {
            param: 1,
            offset: 0,
        },
    ]);
    let mut r0 = CaseDomain::free(n, chart(n, 1));
    // pt0 = k0 + 2147483000: overflows i32 from k0 = 648 on
    r0.target = target.clone();
    // equation at the rule point: pt0 = 0, never true on the chart
    r0.equations = vec![Poly::parse("n0", n, false).unwrap()];
    let ds = vec![r0, CaseDomain::free(n, chart(n, 1))];
    let d = disp(&ds, n);
    assert!(!open(&d, &[647, 0]));
    assert!(matches!(
        d.first_match(&[648, 0]).unwrap(),
        Dispatch::Undecided { .. }
    ));
    let src = DispatcherSource::new(&d);
    let rep = gap_partition(&src, &f, &dispatch_opts(), None).unwrap();
    assert!(rep.uncovered(&[648, 0]).unwrap());
    assert!(!rep.uncovered(&[647, 0]).unwrap(), "the stop piece is exact");
    assert!(rep.proves_no_gap(CoverageNotion::DispatchCoverage).is_err());
    assert!(mint(&d, &f).is_err());
    assert!(probes_hit_open(&d, &f));
    // a record the normal form drops (equation = the constant 1, never selected) still overflows first
    let mut dropped = CaseDomain::free(n, chart(n, 1));
    dropped.target = target;
    dropped.equations = vec![Poly::constant(1)];
    let ds = vec![dropped, CaseDomain::free(n, chart(n, 1))];
    let d = disp(&ds, n);
    assert!(open(&d, &[700, 0]) && !open(&d, &[1, 0]));
    let rep = gap_partition(&DispatcherSource::new(&d), &f, &dispatch_opts(), None).unwrap();
    assert!(rep.uncovered(&[700, 0]).unwrap() && !rep.uncovered(&[1, 0]).unwrap());
    assert!(mint(&d, &f).is_err());
}

/// Control: list vs dispatcher source give the same digest only when the witness blocks agree (digest v3 also binds
/// the frame).
#[test]
fn p3_digest_v3_sees_the_witness_block_and_the_frame() {
    let n = 2;
    let f = NodeFrame::without_header_lines_first(n, 1);
    let g = NodeFrame::without_header_lines_first(n, 2);
    let ds = vec![CaseDomain::free(n, chart(n, 1))];
    let d = disp(&ds, n);
    assert_eq!(
        table_set_digest(&DispatcherSource::new(&d), &f),
        table_set_digest(&list(&ds, n), &f)
    );
    assert_ne!(
        table_set_digest(&list(&ds, n), &f),
        table_set_digest(&list(&ds, n), &g)
    );
    let mut el = ds.clone();
    el[0].witness_elided = true;
    let de = disp(&el, n);
    assert_ne!(
        table_set_digest(&DispatcherSource::new(&de), &f),
        table_set_digest(&DispatcherSource::new(&d), &f)
    );
}

/// P3-C3 (CRITICAL), INVERTED by partfix3: an off-floor cap that truncates the expansion of a listed class makes the
/// outcome `Partial(OffFloorUpTo)` (it used to stay Complete), and a capped run never proves (no `CompleteRun`);
/// the minting function has no cap at all.
#[test]
fn p3_c3_capped_expansion_is_partial_and_never_proves() {
    let n = 3;
    let f = NodeFrame::without_header_lines_first(n, 3);
    let c = chart(n, 3);
    let mut r0 = CaseDomain::free(n, c.clone());
    r0.fixed[2] = Some(1);
    let mut r1 = CaseDomain::free(n, c.clone());
    r1.exceptions = vec![vec![Poly::parse("n0-n1", n, false).unwrap()]];
    let mut r2 = CaseDomain::free(n, c.clone());
    r2.fixed[0] = Some(1);
    r2.fixed[1] = Some(1);
    let ds = vec![r0, r1, r2];
    let d = disp(&ds, n);
    assert!(open(&d, &[2, 2, 2]), "(2,2,2) is uncovered");
    let src = DispatcherSource::new(&d);
    let full = gap_partition(&src, &f, &dispatch_opts(), None).unwrap();
    assert!(
        full.proves_no_gap(CoverageNotion::DispatchCoverage)
            .is_err()
    );
    assert!(full.uncovered(&[2, 2, 2]).unwrap());
    let capped = PartitionOptions {
        max_off_floor: Some(2),
        ..dispatch_opts()
    };
    let rep = gap_partition(&src, &f, &capped, None).unwrap();
    assert_eq!(rep.stats().pruned_off_floor, 0);
    assert!(rep.cap_truncated() > 0, "the cap truncated an expansion");
    assert!(
        matches!(
            rep.outcome(),
            Outcome::Partial {
                explored: Explored::OffFloorUpTo(2),
                ..
            }
        ),
        "{:?}",
        rep.outcome()
    );
    assert!(rep.proves_no_gap(CoverageNotion::DispatchCoverage).is_err());
    assert!(rep.complete_run().is_err());
    assert!(mint(&d, &f).is_err(), "no FULL-node proof");
    assert!(probes_hit_open(&d, &f));
}

/// A dispatcher whose chart differs from the frame mask: keys of the frame's chart outside it are OutsideChart (never
/// served); the report carries a caveat and the minting function refuses (precondition).
#[test]
fn p3_dispatch_chart_other_than_the_frame_is_never_proved() {
    let n = 2;
    let f = NodeFrame::without_header_lines_first(n, 1);
    let ds = vec![CaseDomain::free(n, None)];
    let d = disp(&ds, n)
        .with_chart(vec![true, true])
        .unwrap();
    assert!(matches!(
        d.first_match(&[1, 0]).unwrap(),
        Dispatch::OutsideChart { .. }
    ));
    let rep = gap_partition(&DispatcherSource::new(&d), &f, &dispatch_opts(), None).unwrap();
    assert!(rep.proves_no_gap(CoverageNotion::DispatchCoverage).is_err());
    assert!(mint(&d, &f).is_err());
}
