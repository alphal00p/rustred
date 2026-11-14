//! Re-audit part 3 (lane audit-part3): the PROOF OBJECT (`economy::ContainmentProof`, `table_set_digest`,
//! `Retirement::check_tables`), extreme coefficients in every literal role against brute force, and the unfixed M2
//! (varying case equations keep the U semantics: over-report only). The auditor's `p3_*_defect` tests asserted the
//! defects found on campaign ebe48378; lane partfix3 INVERTED them (names without `_defect`) and added the arity-20
//! budget / liveness tests and the error-injection test of mutation survivor 47. Synthetic tables only (rule L8).

use std::cell::Cell;

use rustred_campaign_core::cover::{Dispatch, Dispatcher, RuleRef, Table};
use rustred_campaign_core::domain::{CaseDomain, PoleKind, PoleLocus};
use rustred_campaign_core::economy::{
    CoverageState, ProofWork, Retirement, RuleMeasure, SupersedeReason, prove_coverage_preserved,
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

/// P3-m1 (minor), INVERTED by partfix3: `table_set_digest` v3 binds the FRAME (arity and line mask): a proof minted
/// on one mask for chart-less domains no longer checks for the same tables on another mask.
#[test]
fn p3_m1_frame_is_bound_by_the_digest() {
    let n = 1;
    let d = CaseDomain::free(n, None); // no chart: every key of either frame
    let l = list(&[d], n);
    let as_line = NodeFrame::without_header_lines_first(n, 1);
    let as_num = NodeFrame::without_header_lines_first(n, 0);
    let p = prove_coverage_preserved(
        &l,
        &l,
        &as_line,
        &ProofWork::default(),
        &DispatchOrderStatement::SameOrder,
    )
    .expect("no gap on the line frame");
    assert_eq!(p.new_tables_digest(), table_set_digest(&l, &as_line));
    assert_ne!(p.new_tables_digest(), table_set_digest(&l, &as_num));
    let r = Retirement::new(
        "N",
        CoverageState::Full,
        &[("a".into(), measure(100))],
        ("b", &measure(10)),
        SupersedeReason::CheaperSameOrLargerDomain,
        Some(p),
    )
    .unwrap();
    assert!(r.check_tables(&l, &l, &as_line).is_ok());
    assert!(
        r.check_tables(&l, &l, &as_num).is_err(),
        "a proof of the line frame is not a proof of the numerator frame"
    );
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

/// P3-m2 (minor, hardening), INVERTED by partfix3: `prove_coverage_preserved` SNAPSHOTS both sources once and runs
/// the partition, the digest and the cross-check on the snapshot: a source whose answers change between calls gets
/// (at most) a proof bound to the digest of the domains the partition DID see, which never checks against the real
/// table set.
#[test]
fn p3_m2_source_answers_are_snapshotted() {
    let n = 1;
    let f = NodeFrame::without_header_lines_first(n, 1);
    let mut one = CaseDomain::free(n, Some(vec![true]));
    one.fixed[0] = Some(1); // covers only n0 = 1
    let real = list(&[one], n);
    let honest = prove_coverage_preserved(
        &real,
        &real,
        &f,
        &ProofWork::default(),
        &DispatchOrderStatement::SameOrder,
    );
    assert!(honest.is_err(), "n0 >= 2 is a gap");
    let fake = CaseDomain::free(n, Some(vec![true]));
    let shifty = Shifty {
        real: real.clone(),
        fake: fake.clone(),
        calls: Cell::new(0),
        lie_for: 1, // the snapshot reads each domain once
    };
    let p = prove_coverage_preserved(
        &real,
        &shifty,
        &f,
        &ProofWork::default(),
        &DispatchOrderStatement::SameOrder,
    );
    // the snapshot read the fake (free) domain once: a proof about THAT table set
    let p = p.expect("the partition saw a free domain: no gap");
    {
        // the proof is about what the partition saw (the fake free domain), never about the real set
        assert_eq!(
            p.new_tables_digest(),
            table_set_digest(&list(&[fake], n), &f)
        );
        assert_ne!(p.new_tables_digest(), table_set_digest(&real, &f));
        let r = Retirement::new(
            "N",
            CoverageState::Full,
            &[("a".into(), measure(100))],
            ("b", &measure(10)),
            SupersedeReason::CheaperSameOrLargerDomain,
            Some(p),
        )
        .unwrap();
        assert!(r.check_tables(&real, &real, &f).is_err());
    }
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
                            &ProofWork::default(),
                            &DispatchOrderStatement::SameOrder,
                        )
                        .is_ok();
                        // partfix3: the minting function adds preconditions (no bounds: role 1) and the cross-check
                        assert!(!minted || ok, "minted without a no-gap report");
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
            &ProofWork::default(),
            &DispatchOrderStatement::SameOrder
        )
        .is_err()
    );
}

/// P3-M2 (major, budget / memory), INVERTED by partfix3: the face expansion (2^k cells for k `any` slots), the
/// thresholds and the face witnesses are charged to the call's ONE work budget: a one-record table on arity 16 that
/// leaves `n0 >= 2` open stops at a budget of 10 units (aborted, never Complete, never a proof) instead of listing
/// 2^15 faces.
#[test]
fn p3_m2_face_expansion_is_charged_to_the_work_budget() {
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
    assert!(
        rep.budget_exhausted().is_some(),
        "the expansion exhausts the budget"
    );
    assert!(rep.faces().len() < 1 << 15);
    assert!(rep.outcome().is_aborted(), "{:?}", rep.outcome());
    assert!(rep.proves_no_gap(CoverageNotion::DispatchCoverage).is_err());
}

/// P3-M2 (second shape), INVERTED by partfix3: listing the classes walks every PATH of the memo DAG (2^8 paths
/// here); it is charged to the budget, so a 300-unit budget stops it (aborted, no proof) instead of listing every
/// path outside the budget.
#[test]
fn p3_m2_class_paths_are_charged_to_the_budget() {
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
    assert!(rep.budget_exhausted().is_some());
    assert!(rep.outcome().is_aborted());
    assert!(
        rep.classes().len() < 1 << 8,
        "{} classes",
        rep.classes().len()
    );
    assert!(rep.proves_no_gap(CoverageNotion::DispatchCoverage).is_err());
}

/// P3-M2 at the REAL arity (20, lane rule L8: arity is a parameter): the auditor's two OOM shapes (a record leaving
/// `n0 >= 2` open: 2^19 faces per class; a record coupling slots 0, 1 and 16: one class per memo-DAG path) and the
/// minting function on both stop at the work budget, quickly (this test binary runs under the 2 GB scope of
/// light_build), never a proof.
#[test]
fn p3_m2_arity_20_shapes_stop_at_the_budget() {
    let n = 20;
    let f = NodeFrame::without_header_lines_first(n, 10);
    let c: Vec<bool> = (0..n).map(|i| i < 10).collect();
    let mut a = CaseDomain::free(n, Some(c.clone()));
    a.fixed[0] = Some(1);
    let mut b = CaseDomain::free(n, Some(c.clone()));
    b.exceptions = vec![vec![
        Poly::parse("3-2*n0+3*n1", n, false).unwrap(),
        Poly::parse("n16^2-4", n, false).unwrap(),
    ]];
    for (name, ds) in [("face expansion", vec![a]), ("class paths", vec![b])] {
        let l = list(&ds, n);
        let o = PartitionOptions {
            budget: rustred_campaign_core::partition::Budget {
                max_nodes: Some(200_000),
                ..Default::default()
            },
            ..dispatch_opts()
        };
        let t0 = std::time::Instant::now();
        let rep = gap_partition(&l, &f, &o, None).unwrap();
        let ms_rep = t0.elapsed().as_millis();
        let t1 = std::time::Instant::now();
        let minted = prove_coverage_preserved(
            &l,
            &l,
            &f,
            &ProofWork {
                max_units: 200_000,
                wall: None,
                max_memo: Some(10_000),
            },
            &DispatchOrderStatement::SameOrder,
        );
        let ms_mint = t1.elapsed().as_millis();
        eprintln!(
            "arity 20 {name}: outcome {} ({} classes, {} faces) in {ms_rep} ms; minting {:?} in {ms_mint} ms",
            rep.outcome().tag(),
            rep.classes().len(),
            rep.faces().len(),
            minted
                .as_ref()
                .map(|_| ())
                .map_err(|e| e.chars().take(120).collect::<String>())
        );
        assert!(rep.proves_no_gap(CoverageNotion::DispatchCoverage).is_err());
        assert!(minted.is_err(), "{name}: the set has a gap");
        assert!(
            ms_rep < 60_000 && ms_mint < 60_000,
            "{name}: {ms_rep} / {ms_mint} ms"
        );
    }
}

/// P3-M3 (major, liveness), INVERTED by partfix3: `verified_witness` (loci/contain.rs) is charged to the proof
/// budget (one work unit per point, the wall read every 256 points, at most 20,000 points): arity 8 with a 10 ms wall
/// returns promptly, and the answer is never `NotContained` (every key is matched by an earlier record).
#[test]
fn p3_m3_verified_witness_walk_honours_the_proof_budget() {
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
    assert!(ms < 2_000, "{ms} ms");
}

/// P3-M3 at the REAL arity 20 (the auditor's hang: > 10 min in `verified_witness` despite a 1 s wall): the same
/// shape on 20 slots, and the default proof budget with a 1 s wall: `shadowed_with` returns within a few seconds
/// (Unknown or Proved, never NotContained).
#[test]
fn p3_m3_shadowed_at_arity_20_returns_within_the_budget() {
    use rustred_campaign_core::loci::contain::ProofBudget;
    use rustred_campaign_core::partition::{Containment, shadowed_with};
    let n = 20;
    let f = NodeFrame::without_header_lines_first(n, 12);
    let c: Vec<bool> = (0..n).map(|i| i < 12).collect();
    let mut a = CaseDomain::free(n, Some(c.clone()));
    a.exceptions = vec![vec![Poly::parse("n0*n2-4", n, false).unwrap()]];
    let mut on = CaseDomain::free(n, Some(c.clone()));
    on.equations = vec![Poly::parse("n0*n2-4", n, false).unwrap()];
    let mut mid = CaseDomain::free(n, Some(c.clone()));
    mid.fixed[5] = Some(2);
    let subject = CaseDomain::free(n, Some(c));
    let l = list(&[a, on, mid, subject], n);
    let budget = ProofBudget {
        wall: Some(std::time::Duration::from_secs(1)),
        ..Default::default()
    };
    let t0 = std::time::Instant::now();
    let v = shadowed_with(&l, 3, &f, &budget);
    let ms = t0.elapsed().as_millis();
    eprintln!("shadowed_with on arity {n}: {v:?} after {ms} ms (wall budget 1 s)");
    assert!(!matches!(v, Containment::NotContained { .. }));
    assert!(ms < 10_000, "{ms} ms");
}

/// Mutation survivor 47 (loci::lit_status: a Fourier-Motzkin ERROR on the cut region read as "never zero") is now
/// killed: error injection. The region's constraints have coprime coefficients near 2^100, so eliminating a variable
/// overflows i128 (`fm_feasible` = Err): a linear member's status must stay V (varies) with the error flag, never NZ
/// (which would let a branch member read "never zero" and credit coverage).
#[test]
fn p3_mutant47_lit_status_fm_error_stays_varies() {
    use rustred_campaign_core::loci::{Region, lit_status, region::fm_feasible};
    use rustred_campaign_core::partition::classes::Status;
    use rustred_campaign_core::partition::exact::{Affine, MPoly};
    let big = 1i128 << 100;
    let cons = vec![
        Affine {
            coef: vec![big + 1, big + 3, 0],
            c: 0,
        },
        Affine {
            coef: vec![-(big + 5), -(big + 7), 0],
            c: 10,
        },
    ];
    let r = Region {
        k: 3,
        x: (0..3).map(|j| Affine::var(3, j)).collect(),
        cons: cons.clone(),
        names: vec!["a".into(), "b".into(), "c".into()],
    };
    assert!(fm_feasible(&cons, 3).is_err(), "the injected error");
    // the literal t2 (linear): its cut keeps the overflowing constraints in (t0, t1)
    let q = MPoly::var(3, 2);
    let (st, err) = lit_status(&r, &q);
    assert_eq!(st, Status::V, "an FM error is undecided, never NZ");
    assert!(err);
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
