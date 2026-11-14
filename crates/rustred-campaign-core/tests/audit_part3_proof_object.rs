//! Re-audit part 3 (lane audit-part3): the PROOF OBJECT (`economy::ContainmentProof`, `table_set_digest` v2,
//! `Retirement::check_tables`), extreme coefficients in every literal role against brute force, and the unfixed M2
//! (varying case equations keep the U semantics: over-report only). Tests named `p3_*_defect` ASSERT a defect found on
//! campaign ebe48378; a fix must invert the marked assertions. Synthetic tables only (lane rule L8).

use std::cell::Cell;

use rustred_campaign_core::cover::{Dispatch, Dispatcher, RuleRef, Table};
use rustred_campaign_core::domain::{CaseDomain, PoleKind, PoleLocus};
use rustred_campaign_core::economy::{
    CoverageState, Retirement, RuleMeasure, SupersedeReason, prove_coverage_preserved,
    table_set_digest,
};
use rustred_campaign_core::key::{Key, NodeFrame};
use rustred_campaign_core::partition::{
    CoverageNotion, DispatchOrderStatement, DispatcherSource, DomainList, DomainSource,
    PartitionOptions, gap_partition,
};
use rustred_campaign_core::poly::Poly;

fn dispatch_opts() -> PartitionOptions {
    PartitionOptions {
        coverage: CoverageNotion::DispatchCoverage,
        ..Default::default()
    }
}

fn list(ds: &[CaseDomain], n: usize) -> DomainList {
    let mut l = DomainList::new(n);
    for (i, d) in ds.iter().enumerate() {
        l.push(RuleRef::bare(0, i, i as i64), d.clone());
    }
    l
}

fn disp(ds: &[CaseDomain], n: usize) -> Option<Dispatcher> {
    let mut t = Table::new("t", n);
    for (i, d) in ds.iter().enumerate() {
        t.push(i as i64, d.clone()).ok()?;
    }
    Dispatcher::new(vec![t], vec![0]).ok()
}

fn window(n: usize, e: usize, w: i32) -> Vec<Key> {
    let mut out: Vec<Key> = vec![vec![]];
    for s in 0..n {
        let vals: Vec<i32> = if s < e {
            (1..=w).collect()
        } else {
            (-(w - 1)..=0).collect()
        };
        out = out
            .into_iter()
            .flat_map(|k| {
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

fn measure(t: u64) -> RuleMeasure {
    RuleMeasure {
        cases: 1,
        rhs_terms: t,
        monomials: t,
        monomials_estimated: false,
        max_degree: None,
        served_keys: None,
        bytes: 1,
    }
}

/// P3-m1 (minor): `table_set_digest` v2 binds the domains and witness blocks but NOT the frame (only the arity).
/// For domains without a chart (`chart: None`, which the dispatcher accepts) a proof minted on one line mask passes
/// `Retirement::check_tables` for the same tables used on another mask, where every key is uncovered. (Assembly's
/// tables carry the header chart, so `gap_partition` would refuse the wrong frame there: `ChartNotFrame`.)
#[test]
fn p3_m1_frame_not_bound_by_the_digest_defect() {
    let n = 1;
    let mut d = CaseDomain::free(n, None);
    d.bounds = vec![Poly::parse("n0-1", n, false).unwrap()]; // n0 >= 1: every LINE key, no numerator key
    let l = list(&[d], n);
    let as_line = NodeFrame::without_header_lines_first(n, 1);
    let as_num = NodeFrame::without_header_lines_first(n, 0);
    let p = prove_coverage_preserved(
        &l,
        &l,
        &as_line,
        &dispatch_opts(),
        &DispatchOrderStatement::SameOrder,
    )
    .expect("no gap on the line frame");
    let r = Retirement::new(
        "N",
        CoverageState::Full,
        &[("a".into(), measure(100))],
        ("b", &measure(10)),
        SupersedeReason::CheaperSameOrLargerDomain,
        Some(p),
    )
    .unwrap();
    // DEFECT: the same tables on the numerator frame have no covered key at all, and the proof still checks
    let rep = gap_partition(&l, &as_num, &dispatch_opts(), None).unwrap();
    assert!(rep.uncovered(&[0]).unwrap() && rep.uncovered(&[-5]).unwrap());
    assert!(rep.proves_no_gap(CoverageNotion::DispatchCoverage).is_err());
    assert!(r.check_tables(&l, &l).is_ok());
}

/// A `DomainSource` whose `domain(i)` answers differ between calls (any caller can implement the public trait).
struct Shifty {
    real: DomainList,
    fake: CaseDomain,
    calls: Cell<usize>,
    lie_for: usize,
}

impl DomainSource for Shifty {
    fn arity(&self) -> usize {
        self.real.arity
    }
    fn len(&self) -> usize {
        self.real.items.len()
    }
    fn domain(&self, i: usize) -> &CaseDomain {
        let c = self.calls.get();
        self.calls.set(c + 1);
        if c < self.lie_for {
            &self.fake
        } else {
            &self.real.items[i].1
        }
    }
    fn rule_ref(&self, i: usize) -> RuleRef {
        self.real.items[i].0
    }
}

/// P3-m2 (minor, hardening): `prove_coverage_preserved` runs the partition on `new_tables` FIRST and digests it
/// AFTERWARDS, calling `domain(i)` again; a source whose answers change between the calls gets a proof bound to the
/// digest of domains the partition never saw (here: the real record covers one key, the partition saw a free
/// domain). Smallest fix: snapshot both sources once into a `DomainList` (domains + witness blocks), then digest and
/// partition the snapshot.
#[test]
fn p3_m2_source_answers_are_not_snapshotted_defect() {
    let n = 1;
    let f = NodeFrame::without_header_lines_first(n, 1);
    let mut one = CaseDomain::free(n, Some(vec![true]));
    one.fixed[0] = Some(1); // covers only n0 = 1
    let real = list(&[one], n);
    let honest = prove_coverage_preserved(
        &real,
        &real,
        &f,
        &dispatch_opts(),
        &DispatchOrderStatement::SameOrder,
    );
    assert!(honest.is_err(), "n0 >= 2 is a gap");
    let shifty = Shifty {
        real: real.clone(),
        fake: CaseDomain::free(n, Some(vec![true])),
        calls: Cell::new(0),
        lie_for: 1, // nf::build reads each domain once
    };
    let p = prove_coverage_preserved(
        &real,
        &shifty,
        &f,
        &dispatch_opts(),
        &DispatchOrderStatement::SameOrder,
    );
    // DEFECT: a proof whose new-table digest is the REAL table set's
    let p = p.expect("the defect: a proof minted");
    assert_eq!(p.new_tables_digest(), table_set_digest(&real));
    let r = Retirement::new(
        "N",
        CoverageState::Full,
        &[("a".into(), measure(100))],
        ("b", &measure(10)),
        SupersedeReason::CheaperSameOrLargerDomain,
        Some(p),
    )
    .unwrap();
    assert!(r.check_tables(&real, &real).is_ok());
}

/// Extreme coefficients in EVERY literal role (equation, bound, exception member, pole coefficient, recenter bound) of
/// a near-cover chain, arities 1-3: whenever the dispatch-mode partition proves no gap, no window / far key is open
/// for the dispatcher (except the known engine-zero class, asserted absent here because no value is a multiple of
/// p1*p2 by construction: the coefficients below are not divisible by either prime).
#[test]
fn p3_extreme_coefficients_in_every_role_vs_brute_force() {
    let ext: [i128; 10] = [
        i128::MIN,
        i128::MIN + 1,
        i128::MAX,
        -(1 << 126),
        1 << 126,
        1 << 63,
        -(1 << 63),
        (1 << 62) + 1,
        -(1 << 100),
        3 << 120,
    ];
    let mut proved = 0u64;
    let mut sets = 0u64;
    let mut bad: Vec<String> = vec![];
    for n in 1..=3usize {
        for e in 0..=n {
            let f = NodeFrame::without_header_lines_first(n, e);
            let chart: Vec<bool> = (0..n).map(|i| i < e).collect();
            let mut keys = window(n, e, if n == 1 { 12 } else { 6 });
            for b in [1_000i32, 1_000_000, i32::MAX - 1] {
                let mut k: Key = (0..n).map(|i| if i < e { 1 } else { 0 }).collect();
                k[0] = if e > 0 { b } else { -b };
                keys.push(k);
            }
            for &c in &ext {
                // literal shapes in slot variables
                let shapes: Vec<Poly> = [
                    Poly::constant(c).mul(&Poly::var(0)),
                    Poly::constant(c)
                        .mul(&Poly::var(0))
                        .and_then(|p| p.add(&Poly::constant(c))),
                    Poly::constant(c)
                        .mul(&Poly::var(0))
                        .and_then(|p| p.sub(&Poly::constant(c))),
                    Poly::var(0).sub(&Poly::constant(c)),
                    Poly::constant(c)
                        .mul(&Poly::var(0))
                        .and_then(|p| p.add(&Poly::var(n - 1)))
                        .and_then(|p| p.sub(&Poly::constant(1))),
                ]
                .into_iter()
                .filter_map(Result::ok)
                .collect();
                for q in &shapes {
                    for role in 0..5 {
                        // chain: [cov(q-role), catch-all with exception {q}] and the reverse role-free variants
                        let mut cov = CaseDomain::free(n, Some(chart.clone()));
                        let mut last = CaseDomain::free(n, Some(chart.clone()));
                        match role {
                            0 => {
                                cov.equations = vec![q.clone()];
                                last.exceptions = vec![vec![q.clone()]];
                            }
                            1 => {
                                cov.bounds = vec![q.clone()];
                                last.bounds = vec![q.neg().unwrap_or(q.clone())];
                            }
                            2 => {
                                cov.exceptions = vec![vec![q.clone()]];
                                last.equations = vec![q.clone()];
                            }
                            3 => {
                                let p = q.mul(&Poly::d()).unwrap();
                                cov.poles = vec![PoleLocus::new(PoleKind::Denominator, &p)];
                            }
                            _ => cov.recenter_bounds = vec![q.clone()],
                        }
                        let ds = vec![cov, last];
                        let Some(d) = disp(&ds, n) else { continue };
                        sets += 1;
                        let src = DispatcherSource::new(&d);
                        let rep = gap_partition(&src, &f, &dispatch_opts(), None).unwrap();
                        let ok = rep.proves_no_gap(CoverageNotion::DispatchCoverage).is_ok();
                        let minted = prove_coverage_preserved(
                            &src,
                            &src,
                            &f,
                            &dispatch_opts(),
                            &DispatchOrderStatement::SameOrder,
                        )
                        .is_ok();
                        assert_eq!(ok, minted);
                        proved += u64::from(ok);
                        for k in &keys {
                            let open = !matches!(d.first_match(k).unwrap(), Dispatch::Covered(_));
                            let m = rep.uncovered(k).unwrap();
                            if open && (ok || !m) {
                                bad.push(format!(
                                    "n={n} e={e} c={c} q={q} role={role} key={k:?} proved={ok} oracle_uncovered={m}: {:?}",
                                    d.first_match(k).unwrap()
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    eprintln!("extreme sweep: {sets} sets, {proved} proved");
    assert!(
        bad.is_empty(),
        "{} unsound: {:#?}",
        bad.len(),
        &bad[..bad.len().min(10)]
    );
    assert!(sets > 500 && proved > 50);
}

/// M2 (unfixed by partfix2, documented): a case equation that varies on a class kills its rule there (class U,
/// "uncovered except on covered_on loci"). Confirm on a diagonal it only OVER-reports (covered keys in U classes,
/// never an uncovered key in a C class) and that the exact loci still prove a full cover made of a diagonal and the
/// two half-planes.
#[test]
fn p3_m2_varying_case_equation_only_over_reports() {
    let n = 2;
    let f = NodeFrame::without_header_lines_first(n, 2);
    let c = Some(vec![true, true]);
    let mut diag = CaseDomain::free(n, c.clone());
    diag.equations = vec![Poly::parse("n0-n1", n, false).unwrap()];
    let mut above = CaseDomain::free(n, c.clone());
    above.bounds = vec![Poly::parse("n0-n1-1", n, false).unwrap()];
    let mut below = CaseDomain::free(n, c.clone());
    below.bounds = vec![Poly::parse("n1-n0-1", n, false).unwrap()];
    let keys = window(n, 2, 30);
    // the diagonal alone: U semantics, over-report only
    let only = list(&[diag.clone()], n);
    let rep = gap_partition(&only, &f, &dispatch_opts(), None).unwrap();
    let cen = rep.census(&keys).unwrap();
    assert!(cen.uncovered_in_c.is_empty());
    assert!(
        !cen.covered_in_u.is_empty(),
        "M2: the diagonal's covered keys sit in a U class (over-report)"
    );
    // the full cover: proved without a gap
    let full = list(&[diag, above, below], n);
    let rep = gap_partition(&full, &f, &dispatch_opts(), None).unwrap();
    assert!(rep.proves_no_gap(CoverageNotion::DispatchCoverage).is_ok());
    // with the diagonal shifted by a parity trap (2 n0 - 2 n1 - 1 = 0 has no integer point) the diagonal is a gap
    let mut trap = CaseDomain::free(n, c.clone());
    trap.equations = vec![Poly::parse("2*n0-2*n1-1", n, false).unwrap()];
    let mut above = CaseDomain::free(n, c.clone());
    above.bounds = vec![Poly::parse("n0-n1-1", n, false).unwrap()];
    let mut below = CaseDomain::free(n, c);
    below.bounds = vec![Poly::parse("n1-n0-1", n, false).unwrap()];
    let gap = list(&[trap, above, below], n);
    let rep = gap_partition(&gap, &f, &dispatch_opts(), None).unwrap();
    assert!(rep.proves_no_gap(CoverageNotion::DispatchCoverage).is_err());
    assert!(rep.uncovered(&[5, 5]).unwrap());
    assert!(rep.census(&keys).unwrap().uncovered_in_c.is_empty());
}

/// Arity above the engine's 64 slots: a typed refusal, never a proof.
#[test]
fn p3_arity_above_64_is_refused() {
    let n = 70;
    let f = NodeFrame::without_header_lines_first(n, 35);
    let l = list(
        &[CaseDomain::free(n, Some((0..n).map(|i| i < 35).collect()))],
        n,
    );
    assert!(gap_partition(&l, &f, &dispatch_opts(), None).is_err());
    assert!(
        prove_coverage_preserved(
            &l,
            &l,
            &f,
            &dispatch_opts(),
            &DispatchOrderStatement::SameOrder
        )
        .is_err()
    );
}

/// P3-M2 (major, budget / memory): the face expansion of a class with k `any` slots is 2^k cells (up to
/// `max_any_slots` = 24 by default: 16.7 M cells) and is NOT charged to the call's work budget (`Budget::max_nodes`
/// counts recursion / locus / split nodes only; `Budget::max_faces` defaults to unlimited). A one-record table on
/// arity 16 that leaves `n0 >= 2` open: 2 recursion nodes, 2^15 faces, outcome Complete under a budget of 10 units.
/// At arity 20 (the five-loop arity) this is 2^19 faces per such class; the auditor's fuzz set 0x7e8eed9fd121babb
/// (arity 20, three records) grew past 1.4 GB inside `gap_partition` and was OOM-killed at the 4 GB scope, also
/// through `economy::prove_coverage_preserved`, which needs no face list at all.
#[test]
fn p3_m2_face_expansion_is_not_charged_to_the_work_budget_defect() {
    let n = 16;
    let f = NodeFrame::without_header_lines_first(n, n);
    let mut d = CaseDomain::free(n, Some(vec![true; n]));
    d.fixed[0] = Some(1); // covers the n0 = 1 hyperplane only
    let l = list(&[d], n);
    let o = PartitionOptions {
        budget: rustred_campaign_core::partition::Budget {
            max_nodes: Some(10),
            ..Default::default()
        },
        ..dispatch_opts()
    };
    let rep = gap_partition(&l, &f, &o, None).unwrap();
    assert!(rep.stats().nodes <= 10);
    assert!(rep.budget_exhausted().is_none());
    // DEFECT: 2^15 faces materialised within a 10-unit budget
    assert_eq!(rep.faces().len(), 1 << 15);
    assert!(rep.outcome().is_complete());
    assert!(rep.proves_no_gap(CoverageNotion::DispatchCoverage).is_err());
}
