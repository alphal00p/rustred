//! Re-audit part 3 (lane audit-part3): the DISPATCH-mode partition vs the crate's own first-match dispatcher
//! (`cover::Dispatcher::first_match`), and the one minting point of a FULL-node containment proof
//! (`economy::prove_coverage_preserved`). Every test here ASSERTS THE DEFECT as found on campaign ebe48378 (a minted
//! proof / `Proved` verdict next to a key the dispatcher reports open); a fix must invert the marked assertions.
//! Synthetic tables only (lane rule L8).

use rustred_campaign_core::cover::{Dispatch, Dispatcher, RuleRef, Table};
use rustred_campaign_core::domain::{CaseDomain, TargetMap, TargetSlot};
use rustred_campaign_core::economy::{prove_coverage_preserved, table_set_digest};
use rustred_campaign_core::key::NodeFrame;
use rustred_campaign_core::partition::{
    CoverageNotion, DispatchOrderStatement, DispatcherSource, DomainList, PartitionOptions,
    gap_partition,
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

/// P3-C1 (CRITICAL): `prove_coverage_preserved` forces `coverage = DispatchCoverage` but honours every other option,
/// including `rule_subset`, which the normal form uses as the list AND ORDER of dispatched records. The proof's
/// digest is over the FULL `new_tables`. A blocker left out of (or moved behind) the subset is not modelled, so a
/// proof is minted for a table set in which EVERY key is blocked by the first record.
#[test]
fn p3_c1_rule_subset_mints_a_proof_for_a_fully_blocked_table_set() {
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
    // the honest call refuses
    assert!(
        prove_coverage_preserved(&src, &src, &f, &dispatch_opts(), &DispatchOrderStatement::SameOrder)
            .is_err()
    );
    // DEFECT: a subset that omits the blocker, or puts it last, yields a proof bound to the full table set
    for subset in [vec![1], vec![1, 0]] {
        let o = PartitionOptions {
            rule_subset: Some(subset.clone()),
            ..Default::default()
        };
        let p = prove_coverage_preserved(&src, &src, &f, &o, &DispatchOrderStatement::SameOrder);
        let p = p.unwrap_or_else(|e| panic!("subset {subset:?}: expected the defect (a proof), got {e}"));
        assert_eq!(p.new_tables_digest(), table_set_digest(&src));
    }
}

/// P3-C2 (CRITICAL): the engine's modular zero test (`domain::Verdict::Error(EngineZeroTest)`, Stage::Undecided: the
/// dispatcher stops and the key is OPEN) is not modelled by the partition (interfaces/partition.md 10.1 calls it
/// "unreachable for practical key ranges"). One coefficient equal to p1*p2 (fits i128) makes it reachable at EVERY
/// key: an exception member `p1*p2*n0` is nonzero at every chart key but vanishes modulo both engine primes.
#[test]
fn p3_c2_engine_zero_test_reachable_at_every_key_and_proof_minted() {
    let n = 2;
    let f = NodeFrame::without_header_lines_first(n, 1);
    let pp = i128::from(ENGINE_PRIMES[0]) * i128::from(ENGINE_PRIMES[1]);
    // (a) one record, exception member p1*p2*n0 (exactly: never zero on n0 >= 1, so the record applies everywhere)
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
    // DEFECT: the dispatch-mode report proves no gap and the oracle calls the open keys covered
    assert!(rep.proves_no_gap(CoverageNotion::DispatchCoverage).is_ok());
    assert!(!rep.uncovered(&[1, 0]).unwrap());
    assert!(
        prove_coverage_preserved(&src, &src, &f, &dispatch_opts(), &DispatchOrderStatement::SameOrder)
            .is_ok()
    );
    // (b) an EQUATION p1*p2*n0 = 0 on an earlier record: never selected exactly, but undecided for the dispatcher at
    // every key, so the later full record never serves
    let mut e = CaseDomain::free(n, chart(n, 1));
    e.equations = vec![Poly::parse(&format!("{pp}*n0"), n, false).unwrap()];
    let ds = vec![e, CaseDomain::free(n, chart(n, 1))];
    let d = disp(&ds, n);
    assert!(open(&d, &[3, -1]));
    let src = DispatcherSource::new(&d);
    assert!(
        prove_coverage_preserved(&src, &src, &f, &dispatch_opts(), &DispatchOrderStatement::SameOrder)
            .is_ok(),
        "DEFECT: proof minted while every key is undecided"
    );
}

/// P3-M1 (MAJOR): the rule point of a target pattern is computed BEFORE the equations (`CaseDomain::rule_point`: an
/// `i32` overflow is `Error(Overflow)` = Undecided, the search stops), but the normal form puts the rule-point range
/// bound on the SELECTED set (equations, bounds, exceptions included). Keys where the rule point overflows and an
/// equation fails are open for the dispatcher and "not selected" for the partition. Offsets are `i64`, so this is
/// reached at small keys.
#[test]
fn p3_m1_rule_point_overflow_is_checked_before_the_equations() {
    let n = 2;
    let f = NodeFrame::without_header_lines_first(n, 1);
    let mut r0 = CaseDomain::free(n, chart(n, 1));
    // pt0 = k0 + 2147483000: overflows i32 from k0 = 648 on
    r0.target = TargetMap::Pattern(vec![
        TargetSlot::Param {
            param: 0,
            offset: -2_147_483_000,
        },
        TargetSlot::Param {
            param: 1,
            offset: 0,
        },
    ]);
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
    // DEFECT: the partition calls [648, 0] covered and proves no gap
    assert!(!rep.uncovered(&[648, 0]).unwrap());
    assert!(rep.proves_no_gap(CoverageNotion::DispatchCoverage).is_ok());
    assert!(
        prove_coverage_preserved(&src, &src, &f, &dispatch_opts(), &DispatchOrderStatement::SameOrder)
            .is_ok()
    );
}

/// Control: list vs dispatcher source give the same digest only when the witness blocks agree (digest v2).
#[test]
fn p3_digest_v2_sees_the_witness_block_of_a_dispatcher_source() {
    let n = 2;
    let ds = vec![CaseDomain::free(n, chart(n, 1))];
    let d = disp(&ds, n);
    assert_eq!(table_set_digest(&DispatcherSource::new(&d)), table_set_digest(&list(&ds, n)));
}
