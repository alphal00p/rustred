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

/// P3-M2 (second shape): one record coupling an early and a late slot (exception {n0 - n1 = 0, n10 - 2 = 0}) makes the
/// class recursion split every intermediate slot (no rule completes before slot 11); the memo keeps the recursion
/// small, but `collect_classes` lists one class per PATH of the memo DAG (2^8 paths through slots 2..9, per leaf
/// kind), each Fragile class is expanded into cells and resolved. None of this is bounded by the node budget. The
/// auditor's arity-20 fuzz set 0x38ecb4765e420c4c (same shape, coupled slots 0, 1, 16) grew past 0.5 GB at ~13 MB/s.
#[test]
fn p3_m2_classes_are_memo_dag_paths_not_charged_to_the_budget_defect() {
    let n = 14;
    let f = NodeFrame::without_header_lines_first(n, n);
    let mut d = CaseDomain::free(n, Some(vec![true; n]));
    d.exceptions = vec![vec![
        Poly::parse("n0-n1", n, false).unwrap(),
        Poly::parse("n10-2", n, false).unwrap(),
    ]];
    let l = list(&[d], n);
    let o = PartitionOptions {
        resolve_loci: false,
        budget: rustred_campaign_core::partition::Budget {
            max_nodes: Some(300),
            max_faces: Some(1),
            ..Default::default()
        },
        ..dispatch_opts()
    };
    let rep = gap_partition(&l, &f, &o, None).unwrap();
    assert!(
        rep.budget_exhausted().is_none(),
        "the recursion fits 300 units"
    );
    assert!(rep.stats().nodes <= 300);
    // DEFECT (robustness): the listed classes grow as 2^(intermediate slots), outside the budget
    assert!(
        rep.classes().len() >= 1 << 8,
        "{} classes",
        rep.classes().len()
    );
    assert!(rep.proves_no_gap(CoverageNotion::DispatchCoverage).is_err());
}

/// P3-M3 (major, liveness of `shadowed` / `contained_in` / `is_empty`): when the sign split ends on a leaf with only
/// NONLINEAR literals, its candidate point is verified against the domains; if that fails, `verified_witness`
/// (loci/contain.rs) walks EVERY point of the L1 ball of radius 1..=12 around a base point in all k parameters with no
/// node, work or wall check: about D(k, 12) points per radius (k = 6: ~3.7e5, k = 20: ~6e10). Shape: subject free;
/// earlier records A (free except on n0*n1 = 4) and C (exactly n0*n1 = 4) match every key, so the true answer is
/// "shadowed", and the walk never finds a witness. The auditor's arity-20 fuzz set 0x73e0f4246dfa99d0 (4 records, one
/// quadratic member) spent > 10 min in this walk (gdb backtrace) inside `shadowed_with` with a 1 s wall budget;
/// acceptance calls `partition::shadowed` on every FULL-node candidate. Here: arity 8, a 10 ms wall budget (arity 6 took 56 ms, arity 8 ~10x more).
#[test]
fn p3_m3_verified_witness_walk_ignores_the_proof_budget_defect() {
    use rustred_campaign_core::loci::contain::ProofBudget;
    use rustred_campaign_core::partition::{Containment, shadowed_with};
    let n = 8;
    let f = NodeFrame::without_header_lines_first(n, n);
    let c = Some(vec![true; n]);
    let mut a = CaseDomain::free(n, c.clone());
    a.exceptions = vec![vec![Poly::parse("n0*n1-4", n, false).unwrap()]];
    let mut on = CaseDomain::free(n, c.clone());
    on.equations = vec![Poly::parse("n0*n1-4", n, false).unwrap()];
    let subject = CaseDomain::free(n, c);
    let l = list(&[a, on, subject], n);
    let budget = ProofBudget {
        max_nodes: 10,
        wall: Some(std::time::Duration::from_millis(10)),
        ..Default::default()
    };
    let t0 = std::time::Instant::now();
    let v = shadowed_with(&l, 2, &f, &budget);
    let ms = t0.elapsed().as_millis();
    eprintln!("shadowed_with on arity {n}: {v:?} after {ms} ms (wall budget 10 ms)");
    assert!(
        !matches!(v, Containment::NotContained { .. }),
        "every key is matched"
    );
    // DEFECT: the walk runs far beyond the 10 ms wall budget (grows as D(k, 12): a hang at arity 20)
    assert!(ms >= 150, "{ms} ms");
}

/// Mutation survivor 36 (region::combine with swapped multipliers, correct only for equal pivot coefficients) is
/// killed by NEITHER the crate's permanent suite NOR the fuzzers: Fourier-Motzkin with NON-UNIT pivots is not checked
/// against brute force anywhere by default. This test does it: random systems in 2-3 variables, coefficients in
/// [-5, 5], inside the box [-6, 6]^k (box constraints included); `fm_feasible == Ok(false)` must mean no integer point.
#[test]
fn p3_fm_infeasible_with_non_unit_pivots_matches_brute_force() {
    use rustred_campaign_core::loci::region::fm_feasible;
    use rustred_campaign_core::partition::exact::Affine;
    let mut s: u64 = 0x00F0_0D5E_ED00;
    let mut next = |m: i64| -> i64 {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((s >> 33) as i64).rem_euclid(2 * m + 1) - m
    };
    let (mut refuted, mut checked) = (0u64, 0u64);
    for _ in 0..20_000 {
        let k = 2 + (next(1).unsigned_abs() as usize % 2);
        let mut cons: Vec<Affine> = vec![];
        for j in 0..k {
            cons.push(Affine::unit(k, j, 1, 6)); // t_j >= -6
            cons.push(Affine::unit(k, j, -1, 6)); // t_j <= 6
        }
        for _ in 0..(2 + next(2).unsigned_abs() as usize) {
            let coef: Vec<i128> = (0..k).map(|_| i128::from(next(5))).collect();
            cons.push(Affine {
                coef,
                c: i128::from(next(12)),
            });
        }
        checked += 1;
        if fm_feasible(&cons, k) != Ok(false) {
            continue;
        }
        refuted += 1;
        // brute force over the box
        let mut t = vec![-6i128; k];
        loop {
            let ok = cons
                .iter()
                .all(|c| c.c + c.coef.iter().zip(&t).map(|(a, x)| a * x).sum::<i128>() >= 0);
            assert!(
                !ok,
                "fm_feasible refuted a system with the integer point {t:?}: {cons:?}"
            );
            let mut j = 0;
            while j < k {
                t[j] += 1;
                if t[j] <= 6 {
                    break;
                }
                t[j] = -6;
                j += 1;
            }
            if j == k {
                break;
            }
        }
    }
    eprintln!("fm brute force: {checked} systems, {refuted} refuted, all integer-empty");
    assert!(refuted > 1_000);
}
