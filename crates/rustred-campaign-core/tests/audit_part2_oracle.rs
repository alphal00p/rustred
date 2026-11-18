//! Independent re-audit of the partition / loci / containment (lane audit-part2, 2026-11-14). Synthetic tables only
//! (lane rule L8). Two kinds of tests:
//! - `defect_*`: the reproductions of the findings, INVERTED by lane partfix2 (2026-11-14): each now asserts the
//!   CORRECT behaviour on the auditor's input (the finding's description is kept in the doc comment);
//! - `diff_*` / `exact_*`: differential property tests against brute force (they must pass; a failure is a finding).
//!
//! Report: WS/notes/implementer.reaudit-part.md.

#![allow(clippy::too_many_arguments)]

mod inputs;
use std::collections::BTreeSet;

use rustred_campaign_core::cover::{Dispatcher, RuleRef, Table};
use rustred_campaign_core::domain::{CaseDomain, PoleKind, PoleLocus, Stage, Verdict};
use rustred_campaign_core::key::{Key, NodeFrame};
use rustred_campaign_core::loci::Region;
use rustred_campaign_core::loci::region::{exhaustive_point, fm_feasible};
use rustred_campaign_core::partition::exact::{
    Affine, ExactError, Range, gcd, gcd_u, int_roots, linear_feasible,
};
use rustred_campaign_core::partition::{
    ANY_OFF, ArtefactIds, Budget, ClassHit, Containment, CoverageNotion, DomainList, ElidedPolicy,
    Emptiness, Label, PartitionOptions, PartitionReport, contained_in, face_of, gap_partition,
    is_empty, shadowed,
};
use rustred_campaign_core::poly::Poly;

// ------------------------------------------------------------------------------------------------------------------
// helpers
// ------------------------------------------------------------------------------------------------------------------

/// splitmix64
#[derive(Clone)]
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as u64) as i64
    }
    fn chance(&mut self, num: u64, den: u64) -> bool {
        self.below(den) < num
    }
}

fn p(t: &str, n: usize) -> Poly {
    Poly::parse(t, n, false).unwrap_or_else(|e| panic!("{t}: {e:?}"))
}

fn free(n: usize, e: usize) -> CaseDomain {
    CaseDomain::free(n, Some((0..n).map(|i| i < e).collect()))
}

fn list(ds: &[CaseDomain], n: usize) -> DomainList {
    let mut l = DomainList::new(n);
    for (i, d) in ds.iter().enumerate() {
        l.push(RuleRef::bare(0, i, i as i64), d.clone());
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

fn union_covered(ds: &[CaseDomain], k: &[i32]) -> bool {
    ds.iter().any(|d| d.applies(k) == Verdict::Applies)
}

fn absent() -> PartitionOptions {
    PartitionOptions {
        elided: ElidedPolicy::Absent,
        ..Default::default()
    }
}

fn dispatch() -> PartitionOptions {
    PartitionOptions {
        coverage: CoverageNotion::DispatchCoverage,
        ..Default::default()
    }
}

/// The integer box of a cell (per slot lo/hi; None = unbounded).
fn cell_box(rep: &PartitionReport, cell: &[Option<u16>]) -> Vec<(Option<i64>, Option<i64>)> {
    (0..rep.arity())
        .map(|s| match cell[s] {
            Some(v) if v != ANY_OFF => {
                let r = rep.states()[s][v as usize];
                (r.lo, r.hi)
            }
            Some(_) => {
                if rep.nf().lines[s] {
                    (Some(2), None)
                } else {
                    (None, Some(-1))
                }
            }
            None => {
                if rep.nf().lines[s] {
                    (Some(1), None)
                } else {
                    (None, Some(0))
                }
            }
        })
        .collect()
}

fn in_box(b: &[(Option<i64>, Option<i64>)], k: &[i32]) -> bool {
    b.iter()
        .zip(k)
        .all(|((lo, hi), &x)| lo.is_none_or(|l| x as i64 >= l) && hi.is_none_or(|h| x as i64 <= h))
}

fn box_inside_window(b: &[(Option<i64>, Option<i64>)], lines: &[bool], w: i32) -> bool {
    b.iter().zip(lines).all(|((lo, hi), &line)| match (lo, hi) {
        (Some(l), Some(h)) => {
            if line {
                *l >= 1 && *h <= w as i64
            } else {
                *l >= -(w as i64 - 1) && *h <= 0
            }
        }
        _ => false,
    })
}

/// Exact integer parameters t with x0 + M t = key (fraction-free Gauss-Jordan); None if none.
fn solve_params(r: &Region, key: &[i32]) -> Option<Vec<i128>> {
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
        let Some(pv) = (row..n).find(|&i| rows[i][col] != 0) else {
            continue;
        };
        rows.swap(row, pv);
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
    if rows[row..].iter().any(|r| r[k] != 0) || piv.len() < k {
        return None;
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

/// Is the uncovered key described by the report (a listed face AND a listed generic cell, a locus piece or an
/// unresolved region of that face, here or in a blocked sub-report)?
fn described(rep: &PartitionReport, k: &[i32]) -> Result<(), String> {
    let fk = face_of(&rep.nf().lines, k);
    let Some(face) = rep.faces().iter().find(|f| f.key() == fk) else {
        return Err(format!("face {fk:?} not listed"));
    };
    let mut reps: Vec<&PartitionReport> = vec![rep];
    reps.extend(rep.blocked().iter().map(|(_, b)| b));
    let slots: Vec<usize> = face.sd.iter().chain(&face.sn).copied().collect();
    for r in &reps {
        let cell: Vec<u16> = slots
            .iter()
            .map(|&s| r.state_index(s, k[s] as i64).unwrap())
            .collect();
        // the face's generic cells were merged from the sub-reports with their OWN state indices; check by the
        // classes instead: a U class of r containing k
        let hit = r.class_of(k).unwrap();
        if hit == ClassHit::Uncovered && r.classes().iter().any(|c| c.label == Label::Uncovered) {
            // must be in a listed generic cell of r's own faces
            if let Some(f) = r.faces().iter().find(|f| f.key() == fk)
                && f.generic_cells.iter().any(|c| {
                    c.iter()
                        .zip(&cell)
                        .all(|(&cs, &st)| cs == st || (cs == ANY_OFF && st != 0))
                })
            {
                return Ok(());
            }
        }
        if let Some(res) = &r.resolution() {
            // harness guard: a face with tens of thousands of open regions (capped classes) is not re-solved per key
            let parts: usize = res
                .loci
                .iter()
                .filter(|l| l.face == fk)
                .map(|l| l.pieces.len() + l.unresolved_parts.len())
                .sum();
            if parts > 3_000 {
                return Ok(());
            }
            for l in res.loci.iter().filter(|l| l.face == fk) {
                for pc in &l.pieces {
                    if let Some(t) = solve_params(&pc.region, k)
                        && pc.region.contains_t(&t).unwrap_or(true)
                    {
                        return Ok(());
                    }
                }
                for u in &l.unresolved_parts {
                    if let Some(t) = solve_params(&u.region, k)
                        && u.region.contains_t(&t).unwrap_or(true)
                    {
                        return Ok(());
                    }
                }
            }
        }
    }
    Err(format!(
        "key on face {fk:?} in no generic cell / locus piece"
    ))
}

// ------------------------------------------------------------------------------------------------------------------
// targeted reproductions
// ------------------------------------------------------------------------------------------------------------------

/// FINDING (major, exactness): a multi-slot BOUND that varies on a class KILLS its rule there
/// (`classes.rs` `ge_val`: `_ => CondVal::Kill`), so a class whose every point is covered by half-space rules is
/// labelled Uncovered and listed as a GENERIC gap face with proof `exact_partition`. Here A: n0 - n1 >= 0 and
/// B: n1 - n0 - 1 >= 0 cover the whole chart, yet the partition lists the face {n0, n1}.
/// FIXED (partfix2, RA2-M2): the varying bound soft-kills its rule, the class is Fragile, the loci split it on the
/// bound exactly: no U class, no face, a union no-gap proof (the chart IS covered).
#[test]
fn defect_varying_multislot_bound_gives_spurious_generic_face() {
    let (n, e) = (2, 2);
    let f = NodeFrame::without_header_lines_first(n, e);
    let mut a = free(n, e);
    a.bounds.push(p("n0-n1", n));
    let mut b = free(n, e);
    b.bounds.push(p("n1-n0-1", n));
    let ds = vec![a, b];
    for k in window(n, e, 12) {
        assert!(union_covered(&ds, &k), "brute force: {k:?} is covered");
    }
    let rep = gap_partition(&list(&ds, n), &f, &absent(), None).unwrap();
    assert!(rep.outcome().is_complete(), "{:?}", rep.outcome());
    assert_eq!(rep.n_u(), 0, "U classes");
    assert!(rep.stats().soft_fragile >= 1);
    assert!(rep.faces().is_empty(), "{:?}", rep.faces());
    assert!(rep.proves_no_gap(CoverageNotion::UnionCoverage).is_ok());
    let c = rep.census(&window(n, e, 8)).unwrap();
    assert!(c.uncovered.is_empty());
    assert!(c.covered_in_u.is_empty());
    // a half-plane gap is found exactly: without B the keys n1 > n0 are a gap, described by a locus piece
    let only_a = vec![ds[0].clone()];
    let ra = gap_partition(&list(&only_a, n), &f, &absent(), None).unwrap();
    assert!(ra.outcome().is_complete());
    for k in window(n, e, 9) {
        let unc = !union_covered(&only_a, &k);
        assert_eq!(ra.uncovered(&k).unwrap(), unc, "{k:?}");
        if unc {
            assert_ne!(ra.class_of(&k).unwrap(), ClassHit::Covered, "{k:?}");
            described(&ra, &k).unwrap_or_else(|w| panic!("{k:?}: {w}"));
        }
    }
}

/// FINDING (major, exactness): a ONE-slot NONLINEAR bound is never decided on an interval state (`ge_status`
/// only handles constants and linear forms), although the slot states are split at every sign change of the bound,
/// so its sign IS constant on each state. A: n0^2 - 4 >= 0 (true for n0 >= 2) plus B: n0 = 1 cover every key of a
/// one-line node, but the tail state n0 >= 3 is labelled Uncovered.
/// FIXED (partfix2, RA2-M2): a one-slot bound of any degree is decided exactly on each state: the tail is covered.
#[test]
fn defect_one_slot_nonlinear_bound_killed_on_its_own_tail() {
    let (n, e) = (1, 1);
    let f = NodeFrame::without_header_lines_first(n, e);
    let mut a = free(n, e);
    a.bounds.push(p("n0^2-4", n));
    let mut b = free(n, e);
    b.fixed[0] = Some(1);
    let ds = vec![a, b];
    for k in window(n, e, 50) {
        assert!(union_covered(&ds, &k));
    }
    let rep = gap_partition(&list(&ds, n), &f, &absent(), None).unwrap();
    assert_eq!(rep.n_u(), 0, "no U class");
    assert!(rep.faces().is_empty());
    assert!(!rep.uncovered(&[7]).unwrap());
    assert_eq!(rep.class_of(&[7]).unwrap(), ClassHit::Covered);
    assert!(rep.proves_no_gap(CoverageNotion::UnionCoverage).is_ok());
}

// ------------------------------------------------------------------------------------------------------------------
// exact integer tools against brute force
// ------------------------------------------------------------------------------------------------------------------

fn rand_range(g: &mut Rng) -> Range {
    let lo = g.range(-6, 6);
    match g.below(4) {
        0 => Range {
            lo: Some(lo),
            hi: None,
        },
        1 => Range {
            lo: None,
            hi: Some(lo),
        },
        _ => Range {
            lo: Some(lo),
            hi: Some(lo + g.range(0, 6)),
        },
    }
}

fn brute_linear(terms: &[(i128, Range)], c: i128, w: i64) -> bool {
    fn rec(terms: &[(i128, Range)], acc: i128, w: i64) -> bool {
        let Some(((a, r), rest)) = terms.split_first() else {
            return acc == 0;
        };
        let (lo, hi) = match (r.lo, r.hi) {
            (Some(l), Some(h)) => (l, h),
            (Some(l), None) => (l, l + w),
            (None, Some(h)) => (h - w, h),
            (None, None) => (-w, w),
        };
        (lo..=hi).any(|x| rec(rest, acc + a * x as i128, w))
    }
    rec(terms, c, w)
}

/// `linear_feasible` (the partition's literal test) against brute force: 200,000 random equations with 1-3
/// variables, coefficients in [-6, 6] (parity / index > 1 lattices included), bounded and half-infinite ranges.
#[test]
fn exact_linear_feasible_vs_brute_force() {
    let mut g = Rng(0xA11CE);
    let (mut n_true, mut n_false, mut n_err) = (0u64, 0u64, 0u64);
    for case in 0..200_000u64 {
        let nv = 1 + g.below(3) as usize;
        let terms: Vec<(i128, Range)> = (0..nv)
            .map(|_| {
                let mut a = g.range(-6, 6) as i128;
                if a == 0 {
                    a = 2;
                }
                if g.chance(1, 3) {
                    a *= 2; // even coefficients: parity traps
                }
                (a, rand_range(&mut g))
            })
            .collect();
        let c = g.range(-25, 25) as i128;
        match linear_feasible(&terms, c) {
            Ok(true) => {
                n_true += 1;
                assert!(
                    brute_linear(&terms, c, 80) || brute_linear(&terms, c, 600),
                    "case {case}: feasible claimed, no solution within 600: {terms:?} c={c}"
                );
            }
            Ok(false) => {
                n_false += 1;
                assert!(
                    !brute_linear(&terms, c, 80),
                    "case {case}: INFEASIBLE claimed but a solution exists: {terms:?} c={c}"
                );
            }
            Err(_) => n_err += 1,
        }
    }
    eprintln!("linear_feasible: {n_true} feasible, {n_false} infeasible, {n_err} work-cap errors");
}

fn univ_eval(c: &[i128], x: i128) -> Option<i128> {
    let mut acc: i128 = 0;
    for &a in c.iter().rev() {
        acc = acc.checked_mul(x)?.checked_add(a)?;
    }
    Some(acc)
}

fn poly_mul(a: &[i128], b: &[i128]) -> Vec<i128> {
    let mut r = vec![0i128; a.len() + b.len() - 1];
    for (i, &x) in a.iter().enumerate() {
        for (j, &y) in b.iter().enumerate() {
            r[i + j] += x * y;
        }
    }
    r
}

/// `int_roots` (critical values, one-slot literal statuses, locus cuts) against brute force on 50,000 random
/// polynomials (products of linear factors, random quadratics, random dense polynomials), plus far roots.
#[test]
fn exact_int_roots_vs_brute_force() {
    let mut g = Rng(0xB0075);
    for case in 0..50_000u64 {
        let mut c: Vec<i128> = vec![g.range(1, 4) as i128 * if g.chance(1, 2) { -1 } else { 1 }];
        let mut want: BTreeSet<i128> = BTreeSet::new();
        match g.below(3) {
            0 => {
                for _ in 0..g.below(4) {
                    let r = g.range(-30, 30) as i128;
                    want.insert(r);
                    c = poly_mul(&c, &[-r, 1]);
                }
                if g.chance(1, 2) {
                    // x^2 + a x + b without integer roots unless it factors: recompute by brute force below
                    c = poly_mul(&c, &[g.range(-9, 9) as i128, g.range(-5, 5) as i128, 1]);
                    want.clear();
                }
            }
            _ => {
                let d = 1 + g.below(4) as usize;
                c = (0..=d).map(|_| g.range(-40, 40) as i128).collect();
                if c[d] == 0 {
                    c[d] = 1;
                }
            }
        }
        if c.iter().all(|&x| x == 0) {
            continue;
        }
        let brute: BTreeSet<i128> = (-3000i128..=3000)
            .filter(|&x| univ_eval(&c, x) == Some(0))
            .collect();
        if !want.is_empty() {
            assert!(want.is_subset(&brute));
        }
        let got: BTreeSet<i128> = int_roots(&c)
            .unwrap_or_else(|e| panic!("case {case}: {c:?}: {e}"))
            .into_iter()
            .collect();
        assert_eq!(got, brute, "case {case}: roots of {c:?}");
    }
    // far roots: (x - 10^9)(x + 3), (x - 2^40)(x - 1), x^3 (x - 7^11)
    let far: [(Vec<i128>, Vec<i128>); 3] = [
        (
            poly_mul(&[-1_000_000_000, 1], &[3, 1]),
            vec![-3, 1_000_000_000],
        ),
        (poly_mul(&[-(1i128 << 40), 1], &[-1, 1]), vec![1, 1 << 40]),
        (
            poly_mul(&[0, 0, 0, 1], &[-(7i128.pow(11)), 1]),
            vec![0, 7i128.pow(11)],
        ),
    ];
    for (c, want) in far {
        assert_eq!(int_roots(&c).unwrap(), want, "{c:?}");
    }
}

fn aff(coef: &[i128], c: i128) -> Affine {
    Affine {
        coef: coef.to_vec(),
        c,
    }
}

fn brute_points(cons: &[Affine], k: usize, b: i128) -> Vec<Vec<i128>> {
    let mut out = vec![];
    let mut t = vec![-b; k];
    loop {
        if cons.iter().all(|c| c.eval(&t).unwrap() >= 0) {
            out.push(t.clone());
        }
        let mut j = 0;
        loop {
            if j == k {
                return out;
            }
            t[j] += 1;
            if t[j] <= b {
                break;
            }
            t[j] = -b;
            j += 1;
        }
    }
}

/// Fourier-Motzkin with rounding (`fm_feasible`: `false` must PROVE integer emptiness) and the exhaustive point
/// search (`Some(None)` must PROVE emptiness) against brute force on 60,000 random bounded systems in 1-3
/// parameters. Also counts the systems that are rationally feasible but integer-empty and which FM does NOT
/// refute (the real-vs-integer gap: harmless only if no caller treats `true` as "has an integer point").
#[test]
fn exact_fourier_motzkin_and_exhaustive_vs_brute_force() {
    let mut g = Rng(0xF0F0);
    let b = 7i128;
    let (mut gap, mut refuted, mut nonempty, mut undecided) = (0u64, 0u64, 0u64, 0u64);
    let mut gap_example: Option<Vec<Affine>> = None;
    for case in 0..60_000u64 {
        let k = 1 + g.below(3) as usize;
        let mut cons: Vec<Affine> = vec![];
        for j in 0..k {
            let mut lo = vec![0i128; k];
            lo[j] = 1;
            cons.push(aff(&lo, b));
            let mut hi = vec![0i128; k];
            hi[j] = -1;
            cons.push(aff(&hi, b));
        }
        for _ in 0..(1 + g.below(4)) {
            let coef: Vec<i128> = (0..k).map(|_| g.range(-5, 5) as i128).collect();
            cons.push(aff(&coef, g.range(-12, 12) as i128));
        }
        let pts = brute_points(&cons, k, b);
        let fm = fm_feasible(&cons, k).unwrap();
        if !fm {
            refuted += 1;
            assert!(
                pts.is_empty(),
                "case {case}: FM refuted a system with {} points: {cons:?}",
                pts.len()
            );
        } else if pts.is_empty() {
            gap += 1;
            if gap_example.is_none() {
                gap_example = Some(cons.clone());
            }
        } else {
            nonempty += 1;
        }
        let mut budget = 1_000_000u64;
        match exhaustive_point(&cons, k, &mut budget).unwrap() {
            Some(Some(t)) => assert!(
                cons.iter().all(|c| c.eval(&t).unwrap() >= 0),
                "case {case}: bad point"
            ),
            Some(None) => assert!(
                pts.is_empty(),
                "case {case}: exhaustive said empty: {cons:?}"
            ),
            None => undecided += 1,
        }
    }
    eprintln!(
        "FM: {refuted} refuted, {nonempty} with points, {gap} rationally-feasible integer-empty NOT refuted (e.g. {gap_example:?}); exhaustive undecided {undecided}"
    );
}

/// `Region::cut` (lattice parametrisation, Hermite column reduction): every integer solution in a box is hit
/// exactly once and every parameter point maps onto the hyperplane; `None` only without any integer solution.
#[test]
fn exact_lattice_cut_vs_brute_force() {
    let mut g = Rng(0xC07);
    for case in 0..20_000u64 {
        let k = 2 + g.below(2) as usize;
        let b = 6i128;
        let mut cons = vec![];
        for j in 0..k {
            let mut lo = vec![0i128; k];
            lo[j] = 1;
            cons.push(aff(&lo, b));
            let mut hi = vec![0i128; k];
            hi[j] = -1;
            cons.push(aff(&hi, b));
        }
        let r = Region {
            k,
            x: (0..k).map(|j| Affine::var(k, j)).collect(),
            cons,
            names: (0..k).map(|j| format!("t{j}")).collect(),
        };
        let coef: Vec<i128> = (0..k).map(|_| g.range(-6, 6) as i128).collect();
        let c = g.range(-15, 15) as i128;
        let sols: Vec<Vec<i128>> = brute_points(&r.cons, k, b)
            .into_iter()
            .filter(|t| aff(&coef, c).eval(t).unwrap() == 0)
            .collect();
        match r.cut(&coef, c).unwrap() {
            None => {
                let gg = coef.iter().fold(0, |a, &x| gcd(a, x).unwrap());
                assert!(
                    sols.is_empty(),
                    "case {case}: cut None but {} solutions",
                    sols.len()
                );
                assert!(
                    gg != 0 && c % gg != 0 || gg == 0 && c != 0,
                    "case {case}: None without a gcd reason"
                );
            }
            Some(s) => {
                for t in &sols {
                    let key: Vec<i32> = t.iter().map(|&v| v as i32).collect();
                    let tt = solve_params(&s, &key).unwrap_or_else(|| {
                        panic!("case {case}: solution {t:?} not on the lattice")
                    });
                    assert!(
                        s.contains_t(&tt).unwrap(),
                        "case {case}: {t:?} outside the constraints"
                    );
                }
                if let Ok(Some(t)) = s.point(8) {
                    let x = s.at(&t).unwrap();
                    assert_eq!(aff(&coef, c).eval(&x).unwrap(), 0, "case {case}");
                }
            }
        }
    }
}

/// FINDING (minor): `gcd` clamped 2^127 to i128::MAX, which does not divide i128::MIN. FIXED (partfix2, RA2-m2):
/// `gcd` answers `Overflow` (2^127 does not fit i128), `gcd_u` the exact u128 value.
#[test]
fn defect_gcd_clamps_two_to_the_127() {
    assert_eq!(gcd(i128::MIN, 0), Err(ExactError::Overflow));
    assert_eq!(gcd_u(i128::MIN, 0), 1u128 << 127);
    assert_eq!(gcd(i128::MIN, 12), Ok(4));
    assert_eq!(gcd(i128::MIN, i128::MAX), Ok(1));
}

/// FINDING (major, exactness): a locus piece whose region is rationally feasible but has NO integer point is
/// reported (`loci::subtract_and_emit` drops a piece only when Fourier-Motzkin refutes it; FM with rounding is not
/// an integer-emptiness decision, and the result depends on the elimination order). The polygon
/// -3 n0 + 4 n1 + 12 >= 0, -4 n0 - 4 n1 + 43 >= 0, 5 n0 - 4 n1 - 24 >= 0 has no integer point; rules A, B, C cover
/// its complement and F (exception n2 = n0 + n1) covers everything off that plane. Every key of the chart is
/// covered, yet a LOCUS-ONLY gap face {n0, n1, n2} is listed with proof exact_partition and a piece without
/// witness; the crate's own `exhaustive_point` proves the piece's region integer-empty (found by
/// `explore_integer_empty_pieces`: 5 such pieces in 39 FM-robust polygons).
/// FIXED (partfix2, RA2-M3): a piece is emitted only with a point; the integer-empty one is PROVED empty by the
/// exhaustive search and dropped: no face, complete outcome, union no-gap proof (the chart IS covered).
#[test]
fn defect_integer_empty_locus_piece_is_reported() {
    let (n, e) = (3, 3);
    let f = NodeFrame::without_header_lines_first(n, e);
    let mut a = free(n, e);
    a.bounds.push(p("3*n0-4*n1-13", n));
    let mut b = free(n, e);
    b.bounds.push(p("4*n0+4*n1-44", n));
    let mut c = free(n, e);
    c.bounds.push(p("-5*n0+4*n1+23", n));
    let mut fr = free(n, e);
    fr.exceptions.push(vec![p("n2-n0-n1", n)]);
    let ds = vec![a, b, c, fr];
    // brute force: every key with lines <= 30 is covered (the polygon is bounded: n0 + n1 <= 10)
    for k in window(n, e, 30) {
        assert!(union_covered(&ds, &k), "{k:?}");
    }
    let rep = gap_partition(&list(&ds, n), &f, &absent(), None).unwrap();
    assert!(rep.outcome().is_complete(), "{:?}", rep.outcome());
    let res = rep.resolution().unwrap();
    for l in &res.loci {
        for pc in &l.pieces {
            let mut budget = 1_000_000u64;
            assert_ne!(
                exhaustive_point(&pc.region.cons, pc.region.k, &mut budget).unwrap(),
                Some(None),
                "an integer-empty piece is reported"
            );
        }
    }
    assert!(
        res.pieces_proved_empty >= 1,
        "the empty piece was proved empty"
    );
    assert!(
        !rep.faces()
            .iter()
            .any(|x| x.key() == (vec![0, 1, 2], vec![]))
    );
    assert!(rep.proves_no_gap(CoverageNotion::UnionCoverage).is_ok());
}

// ------------------------------------------------------------------------------------------------------------------
// the differential harness: random small table sets, brute force over a window
// ------------------------------------------------------------------------------------------------------------------

fn slot(g: &mut Rng, n: usize) -> usize {
    g.below(n as u64) as usize
}

fn coefs(g: &mut Rng) -> i64 {
    [1, -1, 2, -2, 1, -1, 3][g.below(7) as usize]
}

/// A random literal (as text) of one of several shapes; `nonlinear` allows products / squares.
fn lit(g: &mut Rng, n: usize, nonlinear: bool) -> String {
    let (a, b, c) = (slot(g, n), slot(g, n), slot(g, n));
    let k = g.range(-4, 4);
    let shapes = if nonlinear { 7 } else { 4 };
    match g.below(shapes) {
        0 => format!("n{a}-({k})"),
        1 => format!("{}*n{a}+{}*n{b}-({k})", coefs(g), coefs(g)),
        2 => format!(
            "{}*n{a}+{}*n{b}+{}*n{c}-({k})",
            coefs(g),
            coefs(g),
            coefs(g)
        ),
        3 => format!("2*n{a}-2*n{b}-({})", 2 * g.range(-2, 2) + 1), // parity trap: never zero
        4 => format!("(n{a}-({}))*(n{a}-({}))", g.range(-3, 3), g.range(-3, 4)),
        5 => format!("n{a}*n{b}-({})", g.range(-6, 6)),
        _ => format!("n{a}^2-({})", g.range(0, 9)),
    }
}

fn gen_domain(g: &mut Rng, n: usize, e: usize, poles: bool, elided: bool) -> CaseDomain {
    let mut d = free(n, e);
    for s in 0..n {
        if g.chance(1, 6) {
            d.fixed[s] = Some(if s < e {
                g.range(0, 3) as i32 // 0 = outside the chart (dropped)
            } else {
                g.range(-2, 1) as i32
            });
        }
    }
    if g.chance(1, 4) {
        d.equations.push(p(&lit(g, n, true), n));
    }
    for _ in 0..g.below(3) {
        if g.chance(1, 2) {
            let nl = g.chance(1, 4);
            d.bounds.push(p(&lit(g, n, nl), n));
        }
    }
    for _ in 0..g.below(4) {
        let m = 1 + g.below(2);
        d.exceptions
            .push((0..m).map(|_| p(&lit(g, n, true), n)).collect());
    }
    if poles && g.chance(1, 5) {
        let (l1, l2) = (lit(g, n, false), lit(g, n, true));
        let t = match g.below(3) {
            0 => format!("({l1})*d+({l1})*({l2})"),
            1 => format!("({l1})*(d-2)"),
            _ => format!("({l1})*d^2+({l2})"),
        };
        d.poles.push(PoleLocus::new(
            PoleKind::Denominator,
            &Poly::parse(&t, n, true).unwrap(),
        ));
    }
    if poles && g.chance(1, 6) {
        d.recenter_bounds.push(p(&lit(g, n, false), n));
    }
    if elided && g.chance(1, 10) {
        d.witness_elided = true;
    }
    d
}

#[derive(Default, Debug)]
struct Tally {
    cases: u64,
    keys: u64,
    uncovered_keys: u64,
    complete: u64,
    incomplete: u64,
    union_proofs: u64,
    dispatch_proofs: u64,
    dispatch_open: u64,
    /// partfix3: covered keys the dispatch oracle reports open in sets with a modular-risk record (conservative)
    dispatch_overreport: u64,
    spurious_u_bounded: u64,
    covered_in_u: u64,
    flagged_pieces: u64,
    empty_ok: u64,
    empty_unknown: u64,
    contain_proved: u64,
    contain_not: u64,
    contain_unknown: u64,
    shadow_proved: u64,
    shadow_not: u64,
    shadow_unknown: u64,
    determinism_checks: u64,
    flagged_but_complete: u64,
    capped_runs: u64,
    linear_unknown: u64,
}

fn json_without_time(rep: &PartitionReport, f: &NodeFrame) -> String {
    let mut j = rep.to_json(&ArtefactIds::default(), f);
    if let Some(s) = j.get_mut("stats").and_then(|s| s.as_object_mut()) {
        s.remove("millis");
        s.remove("seconds");
    }
    j.as_object_mut().map(|o| o.remove("millis"));
    j.to_string()
}

/// Union mode, dispatch mode, memo / reference, determinism and the proof functions on one random table set.
fn check_case(seed: u64, t: &mut Tally, containment: bool) {
    let mut g = Rng(seed);
    let n = 2 + g.below(3) as usize; // arity 2..4
    let e = 1 + g.below(n as u64) as usize; // at least one line
    let w: i32 = if n <= 3 { 7 } else { 5 };
    let nr = 1 + g.below(6) as usize;
    let ds: Vec<CaseDomain> = (0..nr)
        .map(|_| gen_domain(&mut g, n, e, true, true))
        .collect();
    check_tables(seed, n, e, w, ds, t, containment);
}

fn trace(what: &str, t0: &std::time::Instant) {
    if std::env::var("AUDIT_TRACE").is_ok() {
        eprintln!("[trace] {what} at {} ms", t0.elapsed().as_millis());
    }
}

fn check_tables(
    seed: u64,
    n: usize,
    e: usize,
    w: i32,
    ds: Vec<CaseDomain>,
    t: &mut Tally,
    containment: bool,
) {
    let nr = ds.len();
    let tt = std::time::Instant::now();
    let f = NodeFrame::without_header_lines_first(n, e);
    let l = list(&ds, n);
    let keys = window(n, e, w);
    t.cases += 1;
    t.keys += keys.len() as u64;
    let what = |m: &str| format!("seed {seed} (n={n}, e={e}, {nr} rules) {m}: {ds:?}");

    // ---------------- union mode (elided records absent: brute force = some record APPLIES)
    let rep = gap_partition(&l, &f, &absent(), None)
        .unwrap_or_else(|e| panic!("{}: {e:?}", what("union")));
    match &rep.outcome() {
        o if o.is_complete() => t.complete += 1,
        rustred_campaign_core::partition::Outcome::Incomplete { .. } => t.incomplete += 1,
        o => panic!("{}: unexpected outcome {o:?}", what("union")),
    }
    trace("union partition done", &tt);
    let mut any_unc = false;
    for k in &keys {
        let cov = union_covered(&ds, k);
        let unc = rep.uncovered(k).unwrap();
        assert_eq!(unc, !cov, "{}: point oracle at {k:?}", what("union"));
        if !cov {
            any_unc = true;
            t.uncovered_keys += 1;
            assert_ne!(
                rep.class_of(k).unwrap(),
                ClassHit::Covered,
                "{}: uncovered {k:?} in a COVERED class",
                what("union")
            );
            if let Err(why) = described(&rep, k) {
                panic!("{}: uncovered {k:?} not described: {why}", what("union"));
            }
        }
    }
    trace("union window checks done", &tt);
    // far keys (unbounded directions): slots small or large (up to 10^6), oracle == brute force, never Covered
    let mut gf = Rng(seed ^ 0xFA4);
    for _ in 0..150 {
        let k: Key = (0..n)
            .map(|s| {
                let v = match gf.below(3) {
                    0 => gf.range(0, 3),
                    1 => gf.range(8, 60),
                    _ => gf.range(1_000, 1_000_000),
                } as i32;
                if s < e { v.max(1) } else { -v.abs() }
            })
            .collect();
        let cov = union_covered(&ds, &k);
        assert_eq!(
            rep.uncovered(&k).unwrap(),
            !cov,
            "{}: far key {k:?}",
            what("union")
        );
        if !cov {
            any_unc = true;
            assert_ne!(
                rep.class_of(&k).unwrap(),
                ClassHit::Covered,
                "{}: far {k:?} Covered",
                what("union")
            );
            if let Err(why) = described(&rep, &k) {
                panic!(
                    "{}: far uncovered {k:?} not described: {why}",
                    what("union")
                );
            }
        }
    }
    trace("far keys done", &tt);
    if rep.proves_no_gap(CoverageNotion::UnionCoverage).is_ok() {
        t.union_proofs += 1;
        assert!(!any_unc, "{}: FALSE union no-gap proof", what("union"));
    }
    // witnesses are uncovered; flagged pieces counted
    if let Some(res) = &rep.resolution() {
        for loc in &res.loci {
            for pc in &loc.pieces {
                t.flagged_pieces += u64::from(pc.covered_part_unresolved);
                for wk in &pc.witness {
                    assert!(
                        !union_covered(&ds, wk),
                        "{}: covered witness {wk:?}",
                        what("union")
                    );
                }
            }
        }
    }
    trace("witness checks done", &tt);
    // spurious U classes: a bounded U class inside the window whose every key is covered
    for c in rep.classes().iter().filter(|c| c.label == Label::Uncovered) {
        let b = cell_box(&rep, &c.states);
        let inside: Vec<&Key> = keys.iter().filter(|k| in_box(&b, k)).collect();
        if box_inside_window(&b, &rep.nf().lines, w) && inside.iter().all(|k| union_covered(&ds, k))
        {
            t.spurious_u_bounded += 1;
        }
        t.covered_in_u += inside.iter().filter(|k| union_covered(&ds, k)).count() as u64;
    }
    trace("spurious done", &tt);
    // memo == reference
    let r2 = gap_partition(
        &l,
        &f,
        &PartitionOptions {
            memo: false,
            ..absent()
        },
        None,
    )
    .unwrap();
    let ca: BTreeSet<_> = rep.classes().iter().cloned().collect();
    let cb: BTreeSet<_> = r2.classes().iter().cloned().collect();
    assert_eq!(ca, cb, "{}: memo vs reference classes", what("union"));
    // determinism: the same report from a fresh thread (cold thread-local caches)
    if seed.is_multiple_of(8) {
        let j1 = json_without_time(&rep, &f);
        let (l2, f2) = (l.clone(), f.clone());
        let j2 = std::thread::spawn(move || {
            let r = gap_partition(&l2, &f2, &absent(), None).unwrap();
            json_without_time(&r, &f2)
        })
        .join()
        .unwrap();
        assert_eq!(j1, j2, "{}: nondeterministic report", what("union"));
        t.determinism_checks += 1;
    }

    if rep.outcome().is_complete()
        && rep.resolution().is_some_and(|r| {
            r.loci
                .iter()
                .any(|l| l.pieces.iter().any(|p| p.covered_part_unresolved))
        })
    {
        t.flagged_but_complete += 1;
    }
    trace("memo/determinism done", &tt);
    // ---------------- capped modes (audit A M1 / M2): tiny locus caps and an off-floor cap of 1
    for (lo_nodes, cap) in [(2u64, None), (20_000, Some(1usize))] {
        let o = PartitionOptions {
            loci: rustred_campaign_core::loci::LociOptions {
                max_nodes: lo_nodes,
                ..Default::default()
            },
            max_off_floor: cap,
            ..absent()
        };
        let c =
            gap_partition(&l, &f, &o, None).unwrap_or_else(|e| panic!("{}: {e:?}", what("capped")));
        t.capped_runs += 1;
        for k in &keys {
            if !union_covered(&ds, k) {
                let hit = c.class_of(k).unwrap();
                assert_ne!(
                    hit,
                    ClassHit::Covered,
                    "{}: capped run reads {k:?} Covered",
                    what("capped")
                );
                let off = face_of(&c.nf().lines, k);
                if cap.is_none_or(|cc| off.0.len() + off.1.len() <= cc)
                    && hit != ClassHit::Unexplored
                    && let Err(why) = described(&c, k)
                {
                    panic!("{}: capped run, {k:?} not described: {why}", what("capped"));
                }
            }
        }
        if any_unc {
            assert!(c.proves_no_gap(CoverageNotion::UnionCoverage).is_err());
        }
        if cap.is_some() && c.stats().pruned_off_floor > 0 {
            assert!(
                !c.outcome().is_complete(),
                "{}: capped run says complete",
                what("capped")
            );
        }
    }

    trace("capped done", &tt);
    // ---------------- union mode, FailClosed (default policy): elided records block where SELECTED (first-match
    // semantics for elided records only, the other blockers are holes of their applies sets); Ordinary: credited
    let fc = gap_partition(&l, &f, &PartitionOptions::default(), None)
        .unwrap_or_else(|e| panic!("{}: {e:?}", what("failclosed")));
    let ord = gap_partition(
        &l,
        &f,
        &PartitionOptions {
            elided: ElidedPolicy::Ordinary,
            ..Default::default()
        },
        None,
    )
    .unwrap_or_else(|e| panic!("{}: {e:?}", what("ordinary")));
    for k in &keys {
        let first_applies = ds.iter().position(|d| d.applies(k) == Verdict::Applies);
        // partfix3 (audit-part3 M1): an elided record STOPS first match wherever its verdict is not NotSelected
        // (Blocked, or Undecided: a rule point out of i32 / the modular zero test), as the dispatcher does
        let blocked_by_elided = ds.iter().enumerate().any(|(j, d)| {
            d.witness_elided
                && d.applies(k).stage() != Stage::NotSelected
                && first_applies.is_none_or(|a| a > j)
        });
        let unc_fc = first_applies.is_none() || blocked_by_elided;
        let risky_elided = ds.iter().any(|d| {
            d.witness_elided && rustred_campaign_core::partition::nf::modular_risk(d).is_some()
        });
        if risky_elided {
            // an over-approximated stop set (partfix3): conservative only
            assert!(
                fc.uncovered(k).unwrap() || !unc_fc,
                "{}: FailClosed oracle at {k:?}",
                what("failclosed")
            );
        } else {
            assert_eq!(
                fc.uncovered(k).unwrap(),
                unc_fc,
                "{}: FailClosed oracle at {k:?}",
                what("failclosed")
            );
        }
        if unc_fc {
            assert_ne!(
                fc.class_of(k).unwrap(),
                ClassHit::Covered,
                "{}: {k:?}",
                what("failclosed")
            );
            if let Err(why) = described(&fc, k) {
                panic!("{}: {k:?} not described: {why}", what("failclosed"));
            }
        }
        let cov_ord = ds.iter().any(|d| {
            let v = d.applies(k);
            v == Verdict::Applies || (d.witness_elided && v == Verdict::Elided)
        });
        assert_eq!(
            ord.uncovered(k).unwrap(),
            !cov_ord,
            "{}: Ordinary oracle at {k:?}",
            what("ordinary")
        );
        if !cov_ord {
            assert_ne!(
                ord.class_of(k).unwrap(),
                ClassHit::Covered,
                "{}: {k:?}",
                what("ordinary")
            );
        }
    }
    if fc.proves_no_gap(CoverageNotion::UnionCoverage).is_ok() {
        assert!(keys.iter().all(|k| !fc.uncovered(k).unwrap()));
    }
    if ds.iter().any(|d| d.witness_elided) {
        assert!(
            ord.proves_no_gap(CoverageNotion::UnionCoverage).is_err(),
            "{}: Ordinary credited elided records but claims a proof",
            what("ordinary")
        );
    }

    trace("failclosed/ordinary done", &tt);
    // ---------------- dispatch mode against the core dispatcher's first match
    let mut tab = Table::new("t", n);
    for (i, d) in ds.iter().enumerate() {
        tab.push(i as i64, d.clone()).unwrap();
    }
    let disp = Dispatcher::new(vec![tab], vec![0]).unwrap();
    let drep = gap_partition(&l, &f, &dispatch(), None)
        .unwrap_or_else(|e| panic!("{}: {e:?}", what("dispatch")));
    let mut any_open = false;
    // partfix3 (audit-part3 C2): a record whose literal may meet the engine's modular zero test STOPS dispatch on a
    // possibly over-approximated set (exact points / far radius / its prefix set): the oracle is then conservative
    // (over-report only), exact otherwise
    let risky = ds
        .iter()
        .any(|d| rustred_campaign_core::partition::nf::modular_risk(d).is_some());
    for k in &keys {
        let fm = disp.first_match(k).unwrap();
        let open = fm.is_open();
        let unc = drep.uncovered(k).unwrap();
        if risky {
            assert!(
                unc || !open,
                "{}: dispatch oracle at {k:?} ({fm:?})",
                what("dispatch")
            );
            t.dispatch_overreport += u64::from(unc && !open);
        } else {
            assert_eq!(
                unc,
                open,
                "{}: dispatch oracle at {k:?} ({fm:?})",
                what("dispatch")
            );
        }
        if open {
            any_open = true;
            t.dispatch_open += 1;
            assert_ne!(
                drep.class_of(k).unwrap(),
                ClassHit::Covered,
                "{}: open {k:?} in a COVERED class ({fm:?})",
                what("dispatch")
            );
            if let Err(why) = described(&drep, k) {
                panic!(
                    "{}: open {k:?} not described: {why} ({fm:?})",
                    what("dispatch")
                );
            }
        }
    }
    if drep.proves_no_gap(CoverageNotion::DispatchCoverage).is_ok() {
        t.dispatch_proofs += 1;
        assert!(
            !any_open,
            "{}: FALSE dispatch no-gap proof",
            what("dispatch")
        );
    }

    trace("dispatch done", &tt);
    if !containment {
        return;
    }
    // ---------------- emptiness / containment / shadowing
    let geo = |d: &CaseDomain, k: &[i32]| {
        let v = d.applies(k);
        v == Verdict::Applies || (d.witness_elided && v == Verdict::Elided)
    };
    let linear_dom = |d: &CaseDomain| {
        d.target == rustred_campaign_core::domain::TargetMap::Canonical
            && d.poles.is_empty()
            && d.equations
                .iter()
                .chain(&d.bounds)
                .chain(d.exceptions.iter().flatten())
                .chain(&d.recenter_bounds)
                .all(|q| q.degree() <= 1)
    };
    let all_linear = ds.iter().all(linear_dom);
    for i in 0..nr {
        let d = &ds[i];
        match is_empty(d, &f) {
            Emptiness::ProvedEmpty { certificate } => {
                t.empty_ok += 1;
                for k in &keys {
                    assert!(
                        !geo(d, k),
                        "{}: ProvedEmpty({certificate}) but {k:?} applies (record {i})",
                        what("empty")
                    );
                }
            }
            Emptiness::NonEmpty { witness } => {
                assert!(
                    geo(d, &witness),
                    "{}: bad NonEmpty witness {witness:?} (record {i})",
                    what("empty")
                );
            }
            Emptiness::Unknown { reason } => {
                t.empty_unknown += 1;
                if all_linear {
                    t.linear_unknown += 1;
                    if t.linear_unknown <= 3 {
                        eprintln!("LINEAR Unknown (is_empty) seed {seed} record {i}: {reason}");
                    }
                }
            }
        }
        let others: Vec<&CaseDomain> = (0..nr).filter(|&j| j != i).map(|j| &ds[j]).collect();
        match contained_in(d, &others, &f) {
            Containment::Proved { certificate } => {
                t.contain_proved += 1;
                for k in &keys {
                    if geo(d, k) {
                        assert!(
                            others.iter().any(|o| o.applies(k) == Verdict::Applies),
                            "{}: contained_in Proved({certificate}) but {k:?} only in record {i}",
                            what("contain")
                        );
                    }
                }
            }
            Containment::NotContained { witness } => {
                t.contain_not += 1;
                assert!(geo(d, &witness), "{}: witness {witness:?}", what("contain"));
                assert!(
                    !others
                        .iter()
                        .any(|o| o.applies(&witness) == Verdict::Applies),
                    "{}: witness {witness:?} covered by an other",
                    what("contain")
                );
            }
            Containment::Unknown { reason } => {
                t.contain_unknown += 1;
                if all_linear {
                    t.linear_unknown += 1;
                    if t.linear_unknown <= 3 {
                        eprintln!("LINEAR Unknown (contained_in) seed {seed} record {i}: {reason}");
                    }
                }
            }
        }
        match shadowed(&l, i, &f) {
            Containment::Proved { certificate } => {
                t.shadow_proved += 1;
                for k in &keys {
                    if geo(d, k) {
                        assert!(
                            (0..i).any(|j| ds[j].applies(k).stage() != Stage::NotSelected),
                            "{}: shadowed Proved({certificate}) but no earlier record matches {k:?} (record {i})",
                            what("shadow")
                        );
                    }
                }
            }
            Containment::NotContained { witness } => {
                t.shadow_not += 1;
                assert!(
                    geo(d, &witness),
                    "{}: shadow witness {witness:?}",
                    what("shadow")
                );
                assert!(
                    !(0..i).any(|j| ds[j].applies(&witness).stage() != Stage::NotSelected),
                    "{}: shadow witness {witness:?} matched by an earlier record",
                    what("shadow")
                );
            }
            Containment::Unknown { reason } => {
                t.shadow_unknown += 1;
                if all_linear {
                    t.linear_unknown += 1;
                    if t.linear_unknown <= 3 {
                        eprintln!("LINEAR Unknown (shadowed) seed {seed} record {i}: {reason}");
                    }
                }
            }
        }
    }
}

/// The differential harness, default size (2,000 random table sets; containment on every case).
#[test]
fn diff_random_tables_against_brute_force() {
    let mut t = Tally::default();
    for seed in 0..2_000u64 {
        check_case(
            seed.wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0x51,
            &mut t,
            true,
        );
    }
    eprintln!("{t:#?}");
}

/// The differential harness, large size (run with `--ignored`): 40,000 random table sets.
#[test]
#[ignore]
fn diff_random_tables_against_brute_force_large() {
    let mut t = Tally::default();
    for seed in 0..40_000u64 {
        check_case(
            seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xA5A5,
            &mut t,
            true,
        );
    }
    eprintln!("{t:#?}");
}

/// Exploration (ignored): search small "covered complement of an integer-empty polygon" tables for a reported
/// locus piece without integer points.
#[test]
#[ignore]
fn explore_integer_empty_pieces() {
    let (n, e) = (3, 3);
    let f = NodeFrame::without_header_lines_first(n, e);
    let mut g = Rng(77);
    let (mut found, mut tried) = (0, 0);
    'case: for case in 0..400_000u64 {
        // a polygon P = {g_i >= 0} in (n0, n1), integer-empty in the chart, not refuted by FM in either order
        let m = 3;
        let mut gs: Vec<(i128, i128, i128)> = vec![];
        for _ in 0..m {
            gs.push((
                g.range(-5, 5) as i128,
                g.range(-5, 5) as i128,
                g.range(-60, 60) as i128,
            ));
        }
        let mk = |sw: bool| -> Vec<Affine> {
            let mut c = vec![];
            for &(a, b, k) in &gs {
                c.push(if sw { aff(&[b, a], k) } else { aff(&[a, b], k) });
            }
            c.push(aff(&[1, 0], -2));
            c.push(aff(&[0, 1], -2));
            c.push(aff(&[-1, 0], 40));
            c.push(aff(&[0, -1], 40));
            c
        };
        let c0 = mk(false);
        let mut budget = 1_000_000u64;
        if exhaustive_point(&c0, 2, &mut budget).unwrap() != Some(None) {
            continue;
        }
        if !fm_feasible(&c0, 2).unwrap() || !fm_feasible(&mk(true), 2).unwrap() {
            continue;
        }
        // unbounded version must also be integer-empty in the chart: require the box not to matter
        for &(a, b, k) in &gs {
            let _ = (a, b, k);
        }
        tried += 1;
        let mut ds = vec![];
        for &(a, b, k) in &gs {
            let mut d = free(n, e);
            d.bounds.push(p(&format!("-({a})*n0-({b})*n1-({k})-1"), n));
            ds.push(d);
        }
        let mut fr = free(n, e);
        fr.exceptions.push(vec![p("n2-n0-n1", n)]);
        ds.push(fr);
        let keys = window(n, e, 45);
        for k in keys.iter().filter(|k| k[2] == k[0] + k[1]) {
            if !union_covered(&ds, k) {
                continue 'case;
            }
        }
        let rep = gap_partition(&list(&ds, n), &f, &absent(), None).unwrap();
        let Some(res) = rep.resolution() else {
            continue;
        };
        for l in &res.loci {
            for pc in &l.pieces {
                let mut budget = 1_000_000u64;
                if exhaustive_point(&pc.region.cons, pc.region.k, &mut budget).unwrap()
                    == Some(None)
                {
                    found += 1;
                    if found <= 3 {
                        eprintln!(
                            "case {case}: polygon {gs:?}: empty piece x {:?} cons {:?} faces {:?} witness {:?}",
                            pc.region.render_x(),
                            pc.region.render_cons(),
                            rep.faces()
                                .iter()
                                .map(|x| (x.key(), x.kind, x.proof.tag()))
                                .collect::<Vec<_>>(),
                            pc.witness
                        );
                    }
                }
            }
        }
    }
    eprintln!("tried {tried} found {found}");
}

#[test]
#[ignore]
fn explore_triangle_debug() {
    let (n, e) = (3, 3);
    let f = NodeFrame::without_header_lines_first(n, e);
    let mut a = free(n, e);
    a.bounds.push(p("4*n1-47", n));
    let mut b = free(n, e);
    b.bounds.push(p("-3*n0-n1+38", n));
    let mut c = free(n, e);
    c.bounds.push(p("3*n0-n1-19", n));
    let mut fr = free(n, e);
    fr.exceptions.push(vec![p("n2-n0+3", n)]);
    let ds = vec![a, b, c, fr];
    let rep = gap_partition(&list(&ds, n), &f, &absent(), None).unwrap();
    eprintln!("states {:?}", rep.states());
    eprintln!("classes {:?}", rep.classes());
    let res = rep.resolution().unwrap();
    eprintln!(
        "resolved {} with gaps {} capped {} errors {}",
        res.classes_resolved, res.classes_with_gaps, res.classes_capped, res.exact_errors
    );
    // the triangle in the cut coordinates, both variable orders
    for (cons, what) in [
        (
            vec![
                aff(&[1, 0], -2),
                aff(&[0, 1], -2),
                aff(&[0, -1], 11),
                aff(&[1, 0], -5),
                aff(&[3, 1], -39),
                aff(&[-3, 1], 18),
            ],
            "n0,n1",
        ),
        (
            vec![
                aff(&[0, 1], -2),
                aff(&[1, 0], -2),
                aff(&[-1, 0], 11),
                aff(&[0, 1], -5),
                aff(&[1, 3], -39),
                aff(&[1, -3], 18),
            ],
            "n1,n0",
        ),
    ] {
        eprintln!("{what}: fm {:?}", fm_feasible(&cons, 2));
    }
}

// ------------------------------------------------------------------------------------------------------------------
// adversarial generator: shadowing copies, target patterns, contradictions, point domains, huge coefficients
// ------------------------------------------------------------------------------------------------------------------

fn huge_lit(g: &mut Rng, n: usize) -> String {
    let (a, b) = (slot(g, n), slot(g, n));
    let big = [
        "170141183460469231731687303715884105727",
        "1267650600228229401496703205376",
        "18446744073709551616",
        "9223372036854775807",
    ][g.below(4) as usize];
    match g.below(4) {
        0 => format!("{big}*n{a}-{big}*n{b}"),
        1 => format!("{big}*n{a}-({})", g.range(-3, 3)),
        2 => format!("{big}*n{a}^2-{big}*n{b}"),
        _ => format!("n{a}-{big}"),
    }
}

fn gen_domain_adv(g: &mut Rng, n: usize, e: usize, prev: &[CaseDomain]) -> CaseDomain {
    use rustred_campaign_core::domain::{TargetMap, TargetSlot};
    // a modified copy of an earlier record (overlap / shadowing / exact duplicate)
    if !prev.is_empty() && g.chance(2, 5) {
        let mut d = prev[g.below(prev.len() as u64) as usize].clone();
        match g.below(6) {
            0 => {}
            1 => {
                let s = slot(g, n);
                d.fixed[s] = Some(if s < e {
                    g.range(1, 3) as i32
                } else {
                    -(g.range(0, 2) as i32)
                });
            }
            2 => d.exceptions.push(vec![p(&lit(g, n, true), n)]),
            3 => {
                if !d.exceptions.is_empty() {
                    d.exceptions.remove(0);
                }
            }
            4 => d.bounds.push(p(&lit(g, n, false), n)),
            _ => d.witness_elided = !d.witness_elided,
        }
        return d;
    }
    let mut d = gen_domain(g, n, e, true, true);
    match g.below(8) {
        // contradictory equations
        0 => {
            let s = slot(g, n);
            d.equations.push(p(&format!("n{s}-2"), n));
            d.equations.push(p(&format!("n{s}-3"), n));
        }
        // single-point domain
        1 => {
            for s in 0..n {
                d.fixed[s] = Some(if s < e {
                    g.range(1, 4) as i32
                } else {
                    -(g.range(0, 3) as i32)
                });
            }
        }
        // a non-canonical target pattern (permutation with offsets, sometimes a constant slot)
        2 | 3 => {
            let mut perm: Vec<usize> = (0..n).collect();
            for i in (1..n).rev() {
                let j = g.below(i as u64 + 1) as usize;
                perm.swap(i, j);
            }
            let mut t = vec![];
            for (j, &a) in perm.iter().enumerate() {
                if g.chance(1, 6) {
                    t.push(TargetSlot::Const(if j < e {
                        g.range(1, 3)
                    } else {
                        -g.range(0, 2)
                    }));
                } else {
                    t.push(TargetSlot::Param {
                        param: a,
                        offset: g.range(-2, 2),
                    });
                }
            }
            d.target = TargetMap::Pattern(t);
            d.recenter_bounds.clear();
        }
        // huge coefficients (overflow paths)
        4 => {
            let h = huge_lit(g, n);
            match g.below(3) {
                0 => d.equations.push(p(&h, n)),
                1 => d.bounds.push(p(&h, n)),
                _ => d.exceptions.push(vec![p(&h, n)]),
            }
        }
        _ => {}
    }
    d
}

fn check_case_adv(seed: u64, t: &mut Tally, containment: bool) {
    let mut g = Rng(seed);
    let n = 2 + g.below(4) as usize; // arity 2..5
    let e = 1 + g.below(n as u64) as usize;
    let w: i32 = match n {
        2 | 3 => 7,
        4 => 5,
        _ => 4,
    };
    let nr = 1 + g.below(9) as usize;
    let mut ds: Vec<CaseDomain> = vec![];
    for _ in 0..nr {
        let d = gen_domain_adv(&mut g, n, e, &ds);
        if d.validate().is_ok() {
            ds.push(d);
        }
    }
    if ds.is_empty() {
        return;
    }
    check_tables(seed, n, e, w, ds, t, containment);
}

/// Adversarial differential harness, default size (2,000 table sets).
#[test]
fn diff_adversarial_tables_against_brute_force() {
    let mut t = Tally::default();
    for seed in 0..2_000u64 {
        check_case_adv(
            seed.wrapping_mul(0xD1B5_4A32_D192_ED03) ^ 0x77,
            &mut t,
            true,
        );
    }
    eprintln!("{t:#?}");
}

/// Adversarial differential harness, large size (run with `--ignored`).
#[test]
#[ignore]
fn diff_adversarial_tables_against_brute_force_large() {
    // crate panics on arithmetic overflow (finding RA2-M1) are counted, every other failure is re-raised
    let mut t = Tally::default();
    let mut crate_panics = vec![];
    let quiet = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let from: u64 = std::env::var("AUDIT_FROM")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let to: u64 = std::env::var("AUDIT_TO")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(14_000);
    let skip: Vec<u64> = std::env::var("AUDIT_SKIP")
        .ok()
        .map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect())
        .unwrap_or_default();
    for seed in (from..to).filter(|s| !skip.contains(s)) {
        let s = seed.wrapping_mul(0xA076_1D64_78BD_642F) ^ 0x1234;
        let mut tl = Tally::default();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            check_case_adv(s, &mut tl, true)
        }));
        match r {
            Ok(()) => {
                t.cases += tl.cases;
                t.keys += tl.keys;
                t.uncovered_keys += tl.uncovered_keys;
                t.union_proofs += tl.union_proofs;
                t.dispatch_proofs += tl.dispatch_proofs;
                t.spurious_u_bounded += tl.spurious_u_bounded;
                t.contain_proved += tl.contain_proved;
                t.shadow_proved += tl.shadow_proved;
                t.empty_ok += tl.empty_ok;
                t.linear_unknown += tl.linear_unknown;
                t.flagged_but_complete += tl.flagged_but_complete;
            }
            Err(p) => {
                let msg = p
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default();
                if msg.starts_with("attempt to") {
                    crate_panics.push((seed, msg));
                } else {
                    std::panic::set_hook(quiet);
                    panic!("seed {seed}: {msg}");
                }
            }
        }
    }
    std::panic::set_hook(quiet);
    eprintln!("{t:#?}");
    eprintln!(
        "crate arithmetic panics: {} (first {:?})",
        crate_panics.len(),
        crate_panics.iter().take(5).collect::<Vec<_>>()
    );
}

// ------------------------------------------------------------------------------------------------------------------
// levels checkpoint, budgets, T266
// ------------------------------------------------------------------------------------------------------------------

/// FINDING (major, latent: no production caller yet): the per-level checkpoint of `gap_partition_levels` records
/// no digest of its inputs (domains, frame, coverage notion, elided policy, options). A checkpoint written for one
/// table set is resumed for ANOTHER: its levels come back `proved: true` with the OLD face lists. Here table A
/// covers the whole chart (no face at levels 0..2); table B leaves the face {n0} open; resuming B from A's
/// checkpoint reports levels 0..2 proved with no face.
#[test]
fn defect_levels_checkpoint_resumed_for_other_inputs() {
    use rustred_campaign_core::partition::gap_partition_levels;
    let (n, e) = (3, 2);
    let f = NodeFrame::without_header_lines_first(n, e);
    let a = vec![free(n, e)];
    let mut b0 = free(n, e);
    b0.fixed[0] = Some(1);
    let b = vec![b0];
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let cp = dir.join("audit_part2_levels.checkpoint.jsonl");
    let _ = std::fs::remove_file(&cp);
    let la = gap_partition_levels(&list(&a, n), &f, &absent(), 2, Some(&cp), None).unwrap();
    assert!(la.iter().all(|r| r.proved && r.faces.is_empty()));
    // fresh run of B: the face {n0} is open at level 1
    let fresh = gap_partition_levels(&list(&b, n), &f, &absent(), 2, None, None).unwrap();
    assert!(fresh[1].faces.contains(&(vec![0], vec![])));
    // FIXED (partfix2, RA2-M4): resuming B (or A in dispatch mode, or with another frame) from A's checkpoint is
    // REFUSED; resuming A itself skips the recorded levels
    use rustred_campaign_core::partition::PartitionError;
    let stale =
        |r: Result<Vec<_>, PartitionError>| matches!(r, Err(PartitionError::StaleCheckpoint(_)));
    assert!(stale(gap_partition_levels(
        &list(&b, n),
        &f,
        &absent(),
        2,
        Some(&cp),
        None
    )));
    assert!(stale(gap_partition_levels(
        &list(&a, n),
        &f,
        &dispatch(),
        2,
        Some(&cp),
        None
    )));
    let f2 = NodeFrame::without_header_lines_first(n, e + 1);
    assert!(stale(gap_partition_levels(
        &list(&a, n),
        &f2,
        &absent(),
        2,
        Some(&cp),
        None
    )));
    let again = gap_partition_levels(&list(&a, n), &f, &absent(), 2, Some(&cp), None).unwrap();
    assert_eq!(again.len(), 3);
    assert!(again.iter().all(|r| r.proved && r.faces.is_empty()));
    // a checkpoint line of the old format (no input digest) is refused too
    std::fs::write(&cp, "{\"level\":0,\"faces\":[],\"generic\":[],\"nodes\":1,\"millis\":0,\"proved\":true,\"incomplete\":[]}\n").unwrap();
    assert!(stale(gap_partition_levels(
        &list(&a, n),
        &f,
        &absent(),
        2,
        Some(&cp),
        None
    )));
    let _ = std::fs::remove_file(&cp);
}

/// The node budget is deterministic and an aborted run is never a proof; dispatch mode too;
/// `economy::prove_coverage_preserved` refuses it.
#[test]
fn budget_abort_is_deterministic_and_never_a_proof() {
    use rustred_campaign_core::economy::prove_coverage_preserved;
    use rustred_campaign_core::partition::DispatchOrderStatement;
    let (n, e) = (8, 4);
    let f = NodeFrame::without_header_lines_first(n, e);
    let mut checked = 0;
    for seed in 0..8u64 {
        let mut g = Rng(4242 + seed);
        let ds: Vec<CaseDomain> = (0..24)
            .map(|_| gen_domain(&mut g, n, e, true, false))
            .collect();
        let l = list(&ds, n);
        let o = |cov| PartitionOptions {
            coverage: cov,
            budget: Budget {
                max_nodes: Some(5_000),
                ..Default::default()
            },
            ..absent()
        };
        for cov in [
            CoverageNotion::UnionCoverage,
            CoverageNotion::DispatchCoverage,
        ] {
            let r1 = gap_partition(&l, &f, &o(cov), None).unwrap();
            if !r1.outcome().is_aborted() {
                continue;
            }
            checked += 1;
            let r2 = gap_partition(&l, &f, &o(cov), None).unwrap();
            assert_eq!(
                r1.stats().nodes,
                r2.stats().nodes,
                "deterministic node count"
            );
            assert_eq!(json_without_time(&r1, &f), json_without_time(&r2, &f));
            assert!(r1.proves_no_gap(cov).is_err());
            // partfix3 (audit-part3 C1): the minting function takes a work budget only (the same 5,000 units)
            let w = rustred_campaign_core::economy::ProofWork {
                max_units: 5_000,
                wall: None,
                max_memo: None,
            };
            assert!(
                prove_coverage_preserved(&l, &l, &f, &w, &DispatchOrderStatement::SameOrder)
                    .is_err()
            );
            for k in window(n, e, 2) {
                if r1.uncovered(&k).unwrap() {
                    assert_ne!(r1.class_of(&k).unwrap(), ClassHit::Covered, "{k:?}");
                }
            }
        }
    }
    eprintln!("aborted runs checked: {checked}");
    assert!(checked >= 2);
}

fn t266_load() -> Option<(DomainList, NodeFrame)> {
    let dir = std::path::PathBuf::from(
        std::env::var("RUSTRED_T266_SOURCES").unwrap_or("/common/ben/sym-investigation".into()),
    );
    let path = dir.join("h11b/out/rules266_nf.json");
    if !path.is_file() {
        inputs::missing(format_args!("SKIP: T266 sources not found"));
        return None;
    }
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let e = v["E"].as_u64().unwrap() as usize;
    let n = 20;
    let frame = NodeFrame::without_header_lines_first(n, e);
    let mut l = DomainList::new(n);
    for r in v["rules"].as_array().unwrap() {
        let mut d = free(n, e);
        let pp = |s: &serde_json::Value| {
            Poly::parse(
                &s.as_str().unwrap().replace("**", "^").replace(' ', ""),
                n,
                false,
            )
            .ok()
        };
        let eq: Option<Vec<Poly>> = r["EQ"].as_array().unwrap().iter().map(pp).collect();
        let bad: Option<Vec<Vec<Poly>>> = r["BAD"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b.as_array().unwrap().iter().map(pp).collect())
            .collect();
        let (Some(eq), Some(bad)) = (eq, bad) else {
            continue;
        };
        d.equations = eq;
        d.exceptions = bad;
        let k = r["k"].as_u64().unwrap() as usize;
        l.push(RuleRef::bare(0, k, k as i64), d);
    }
    Some((l, frame))
}

/// T266 (test data, read-only checker sources): the exact partition under a DETERMINISTIC node budget returns an
/// honest `budget_exceeded` (Partial / Aborted), identical node counts on two runs, no proof under either notion,
/// no FULL-node containment proof, and the closure-gap keys are never read Covered.
#[test]
#[ignore = "needs the read-only T266 sources; release; ~1-2 min"]
fn t266_node_budget_is_honest_and_deterministic() {
    use rustred_campaign_core::economy::prove_coverage_preserved;
    use rustred_campaign_core::partition::DispatchOrderStatement;
    let Some((l, frame)) = t266_load() else {
        return;
    };
    let o = PartitionOptions {
        elided: ElidedPolicy::Ordinary,
        resolve_loci: false,
        budget: Budget {
            max_nodes: Some(1_000_000),
            max_memo: Some(25_000),
            ..Default::default()
        },
        ..Default::default()
    };
    let t0 = std::time::Instant::now();
    let r1 = gap_partition(&l, &frame, &o, None).unwrap();
    let s1 = t0.elapsed().as_millis();
    let r2 = gap_partition(&l, &frame, &o, None).unwrap();
    println!(
        "T266: {} domains, outcome {} ({:?}), nodes {} / {}, U {} F {} unexplored {}, faces {}, {s1} ms",
        l.items.len(),
        r1.outcome().tag(),
        r1.outcome().reasons(),
        r1.stats().nodes,
        r2.stats().nodes,
        r1.n_u(),
        r1.n_f(),
        r1.n_unexplored(),
        r1.faces().len()
    );
    assert!(r1.outcome().is_aborted());
    assert_eq!(r1.stats().nodes, r2.stats().nodes);
    assert_eq!(r1.classes(), r2.classes());
    for cov in [
        CoverageNotion::UnionCoverage,
        CoverageNotion::DispatchCoverage,
    ] {
        assert!(r1.proves_no_gap(cov).is_err());
    }
    let w = rustred_campaign_core::economy::ProofWork {
        max_units: 1_000_000,
        wall: None,
        max_memo: Some(25_000),
    };
    assert!(
        prove_coverage_preserved(&l, &l, &frame, &w, &DispatchOrderStatement::SameOrder).is_err()
    );
    // the 12,501 closure gaps: uncovered by the oracle, never in a Covered class
    let dir = std::path::PathBuf::from(
        std::env::var("RUSTRED_T266_SOURCES").unwrap_or("/common/ben/sym-investigation".into()),
    );
    let text = std::fs::read_to_string(dir.join("k2/out/close/uncovered.txt")).unwrap();
    let (mut n_keys, mut cov_class) = (0, 0);
    for line in text.lines() {
        let k: Vec<i32> = serde_json::from_str(line.split_whitespace().next().unwrap()).unwrap();
        n_keys += 1;
        assert!(r1.uncovered(&k).unwrap());
        if r1.class_of(&k).unwrap() == ClassHit::Covered {
            cov_class += 1;
        }
    }
    println!("T266: {n_keys} closure-gap keys, {cov_class} in a Covered class");
    assert_eq!(cov_class, 0, "a gap key in a class labelled Covered");
}

/// FINDING (minor, description): in dispatch / FailClosed mode the faces of the blocked sub-partitions are merged
/// into the main face list (`merge_blocked_faces`) with their generic cells written in the SUB-report's state
/// indices, while `PartitionReport::states` (and the artefact's "states") are the MAIN report's. R0 (bound
/// n0 >= 5, pole on n1 = 3) blocks on {n0 >= 5, n1 = 3}; R1 covers everything else. The main states of n0 are
/// {1},[2,4],{5},[6,inf); the sub-report's {1},[2,3],{4},{5},[6,inf): the merged cells read with the main states
/// miss the open key (5,3) and name a state index the main slot does not have.
#[test]
fn defect_blocked_face_cells_use_sub_report_state_indices() {
    let (n, e) = (2, 2);
    let f = NodeFrame::without_header_lines_first(n, e);
    let mut r0 = free(n, e);
    r0.bounds.push(p("n0-5", n));
    r0.poles.push(PoleLocus::new(
        PoleKind::Denominator,
        &Poly::parse("(n1-3)*d+(n1-3)", n, true).unwrap(),
    ));
    let r1 = free(n, e);
    let ds = vec![r0, r1];
    let mut tab = Table::new("t", n);
    for (i, d) in ds.iter().enumerate() {
        tab.push(i as i64, d.clone()).unwrap();
    }
    let disp = Dispatcher::new(vec![tab], vec![0]).unwrap();
    let rep = gap_partition(&list(&ds, n), &f, &dispatch(), None).unwrap();
    assert!(disp.first_match(&[5, 3]).unwrap().is_open());
    assert!(rep.uncovered(&[5, 3]).unwrap());
    let face = rep
        .faces()
        .iter()
        .find(|x| x.key() == (vec![0, 1], vec![]))
        .expect("face");
    let main_n0 = &rep.states()[0];
    eprintln!(
        "main states n0 {main_n0:?}; sub states n0 {:?}; generic cells {:?}; blocked cells {:?}",
        rep.blocked()[0].1.states()[0],
        face.generic_cells,
        face.blocked_cells
    );
    assert_ne!(rep.blocked()[0].1.states()[0], rep.states()[0]);
    // FIXED (partfix2, RA2-m1): `generic_cells` are in THIS report's states only (in range); the sub-report's cells
    // are `blocked_cells`, tagged with their blocker and read with THAT sub-report's states
    for c in &face.generic_cells {
        for (j, &s) in [0usize, 1].iter().enumerate() {
            assert!(c[j] == ANY_OFF || (c[j] as usize) < rep.states()[s].len());
        }
    }
    let (e, sub) = (&rep.blocked()[0].0, &rep.blocked()[0].1);
    let sub5 = sub.state_index(0, 5).unwrap();
    let sub3 = sub.state_index(1, 3).unwrap();
    assert!(
        face.blocked_cells
            .iter()
            .any(|(x, c)| x == e && c[0] == sub5 && c[1] == sub3),
        "the open key (5,3) is described by a blocked cell in the sub-report's states"
    );
    let j = rep.to_json(&ArtefactIds::default(), &f);
    let face_j = j["faces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["Sd"] == serde_json::json!([0, 1]))
        .unwrap();
    assert!(
        face_j["blocked_cells"].to_string().contains("\"5\""),
        "{face_j}"
    );
}

/// Locator (ignored): the first seeds of the large adversarial run whose check panics, with the domains.
#[test]
#[ignore]
fn locate_adversarial_panics() {
    let mut found = 0;
    for seed in 0..14_000u64 {
        let s = seed.wrapping_mul(0xA076_1D64_78BD_642F) ^ 0x1234;
        let r = std::panic::catch_unwind(|| {
            let mut t = Tally::default();
            check_case_adv(s, &mut t, true);
        });
        if r.is_err() {
            found += 1;
            let mut g = Rng(s);
            let n = 2 + g.below(4) as usize;
            let e = 1 + g.below(n as u64) as usize;
            let _ = (n, e);
            eprintln!("PANIC at loop seed {seed} (rng seed {s:#x})");
            if found >= 3 {
                break;
            }
        }
    }
    eprintln!("panicking seeds found: {found}");
}

/// Locator (ignored): one adversarial seed (env AUDIT_SEED = loop seed), printing the domains first.
#[test]
#[ignore]
fn locate_one_adversarial_seed() {
    let seed: u64 = std::env::var("AUDIT_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2100);
    let s = seed.wrapping_mul(0xA076_1D64_78BD_642F) ^ 0x1234;
    let mut g = Rng(s);
    let n = 2 + g.below(4) as usize;
    let e = 1 + g.below(n as u64) as usize;
    let _w = g.clone();
    let nr = 1 + g.below(9) as usize;
    let mut ds: Vec<CaseDomain> = vec![];
    for _ in 0..nr {
        let d = gen_domain_adv(&mut g, n, e, &ds);
        if d.validate().is_ok() {
            ds.push(d);
        }
    }
    for (i, d) in ds.iter().enumerate() {
        eprintln!(
            "n={n} e={e} record {i}: fixed {:?} target {:?} eqs {:?} bounds {:?} exc {:?} recenter {:?} poles {:?} elided {}",
            d.fixed,
            d.target,
            d.equations
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>(),
            d.bounds.iter().map(|x| x.to_string()).collect::<Vec<_>>(),
            d.exceptions
                .iter()
                .map(|b| b.iter().map(|x| x.to_string()).collect::<Vec<_>>())
                .collect::<Vec<_>>(),
            d.recenter_bounds
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>(),
            d.poles,
            d.witness_elided
        );
    }
    let mut t = Tally::default();
    let t0 = std::time::Instant::now();
    check_case_adv(s, &mut t, true);
    eprintln!("ok in {} ms", t0.elapsed().as_millis());
}

/// Locator (ignored): per-seed wall time of the adversarial checks from AUDIT_FROM (env), printing slow seeds.
#[test]
#[ignore]
fn locate_slow_adversarial_seeds() {
    let from: u64 = std::env::var("AUDIT_FROM")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2101);
    let to: u64 = std::env::var("AUDIT_TO")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(14_000);
    for seed in from..to {
        let s = seed.wrapping_mul(0xA076_1D64_78BD_642F) ^ 0x1234;
        eprintln!("start {seed}");
        let t0 = std::time::Instant::now();
        let r = std::panic::catch_unwind(|| {
            let mut t = Tally::default();
            check_case_adv(s, &mut t, true);
        });
        let ms = t0.elapsed().as_millis();
        if ms > 3000 || r.is_err() {
            eprintln!("seed {seed}: {ms} ms panic={}", r.is_err());
        }
    }
}

// ------------------------------------------------------------------------------------------------------------------
// integer overflow: unchecked negations reach a division by -1 (panic in release AND debug)
// ------------------------------------------------------------------------------------------------------------------

/// FINDING (major, robustness): `region::bounds0` computes `div_floor(-c.c, a)` with an UNCHECKED negation; for a
/// constant c = i128::MIN the negation wraps (release) and `i128::MIN / -1` panics. Reached through the public
/// `Region::point` / `find_point` / `exhaustive_point` (used by `is_empty`, `contained_in`, `shadowed`, the loci).
/// FIXED (partfix2, RA2-M1): no panic. -t + i128::MIN >= 0 has the bound t <= i128::MIN (computed exactly); fixing
/// t = i128::MIN in the constraint needs (-1) * i128::MIN = 2^127, which does not fit: the search answers the typed
/// `Overflow` (undecided), never a point that violates the constraint and never "empty".
#[test]
fn defect_find_point_panics_on_i128_min_constant() {
    use rustred_campaign_core::loci::region::find_point;
    let cons = vec![aff(&[-1], i128::MIN)];
    match find_point(&cons, 1, 4, &[0]) {
        Ok(Some(t)) => assert!(cons.iter().all(|a| a.eval(&t).is_ok_and(|v| v >= 0))),
        Ok(None) => {
            panic!("not a proof, but the search must not miss the only feasible side silently")
        }
        Err(e) => assert_eq!(e, ExactError::Overflow),
    }
    let mut b = 10u64;
    assert_ne!(
        exhaustive_point(&cons, 1, &mut b),
        Ok(Some(None)),
        "never 'empty'"
    );
    // the same half-line with a representable point: found
    let cons2 = vec![aff(&[-1], i128::MIN + 1), aff(&[1], i128::MAX)];
    let mut b = 10u64;
    assert_eq!(
        exhaustive_point(&cons2, 1, &mut b),
        Ok(Some(Some(vec![i128::MIN + 1])))
    );
    assert_eq!(
        find_point(&cons2, 1, 4, &[0]),
        Ok(Some(vec![i128::MIN + 1]))
    );
    // a constraint whose bound does not fit i128 is an error, never a panic, never "empty"
    let cons3 = vec![aff(&[1], i128::MIN)];
    assert!(find_point(&cons3, 1, 4, &[0]).is_err());
}

/// FINDING (major, robustness): in dispatch mode the COMPLEMENT of a blocker's selection bound
/// `n0 + (2^127 - 1) >= 0` is the GE literal `-n0 + i128::MIN >= 0` (exact, checked); `slot_states` /
/// `sign_breaks` then negate its constant unchecked and divide by -1: the whole partition panics.
/// FIXED (partfix2, RA2-M1): no panic; the dispatch-mode partition is honest (the open key 3 is found, no proof).
#[test]
fn defect_dispatch_partition_panics_on_bound_complement() {
    let (n, e) = (1, 1);
    let f = NodeFrame::without_header_lines_first(n, e);
    let mut r0 = free(n, e);
    r0.bounds
        .push(p("n0+170141183460469231731687303715884105727", n));
    r0.poles.push(PoleLocus::new(
        PoleKind::Denominator,
        &Poly::parse("(n0-3)*d+(n0-3)", n, true).unwrap(),
    ));
    let ds = vec![r0, free(n, e)];
    let rep = gap_partition(&list(&ds, n), &f, &dispatch(), None).unwrap();
    let mut tab = Table::new("t", n);
    for (i, d) in ds.iter().enumerate() {
        tab.push(i as i64, d.clone()).unwrap();
    }
    let disp = Dispatcher::new(vec![tab], vec![0]).unwrap();
    for k in window(n, e, 40) {
        let open = disp.first_match(&k).unwrap().is_open();
        assert_eq!(rep.uncovered(&k).unwrap(), open, "{k:?}");
        if open {
            assert_ne!(rep.class_of(&k).unwrap(), ClassHit::Covered);
        }
    }
    assert!(rep.uncovered(&[3]).unwrap());
    assert!(rep.proves_no_gap(CoverageNotion::DispatchCoverage).is_err());
}

/// FINDING (major, robustness), found by the large adversarial harness (seed 2100): `shadowed` (which acceptance
/// calls on every FULL-node candidate) panics on a table whose record pins a slot by the case equation
/// n0 = 2^127 - 1, instead of answering `Unknown`.
/// FIXED (partfix2, RA2-M1): no panic; every verdict is sound (checked against brute force on a window).
#[test]
fn defect_shadowed_panics_on_huge_case_equation() {
    let (n, e) = (3, 3);
    let f = NodeFrame::without_header_lines_first(n, e);
    let mut r0 = free(n, e);
    r0.equations
        .push(p("-170141183460469231731687303715884105727+n0", n));
    r0.bounds.push(p("1-n0+n1-n2", n));
    r0.poles.push(PoleLocus::new(
        PoleKind::Denominator,
        &Poly::parse("6-5*n0+n0^2+(n1-4)*d", n, true).unwrap(),
    ));
    r0.witness_elided = true;
    let mut r2 = free(n, e);
    r2.fixed[1] = Some(1);
    let mut r3 = free(n, e);
    r3.equations.push(p("-3-2*n2+n2^2", n));
    r3.bounds.push(p("1+3*n1+n2", n));
    r3.exceptions
        .push(vec![p("-2+n0^2", n), p("2+2*n1-3*n2", n)]);
    r3.exceptions.push(vec![p("-3+2*n1", n), p("3+n0+n1", n)]);
    r3.exceptions.push(vec![p("4+n0+2*n2", n)]);
    r3.poles.push(PoleLocus::new(
        PoleKind::Denominator,
        &Poly::parse("-5+2*n0-2*n2+(2+n0)*d", n, true).unwrap(),
    ));
    let ds = vec![r0, r2, r3];
    let l = list(&ds, n);
    let keys = window(n, e, 7);
    let geo = |d: &CaseDomain, k: &[i32]| {
        let v = d.applies(k);
        v == Verdict::Applies || (d.witness_elided && v == Verdict::Elided)
    };
    for i in 0..ds.len() {
        if let Containment::Proved { .. } = shadowed(&l, i, &f) {
            for k in &keys {
                if geo(&ds[i], k) {
                    assert!((0..i).any(|j| ds[j].applies(k).stage() != Stage::NotSelected));
                }
            }
        }
        let others: Vec<&CaseDomain> = ds
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, d)| d)
            .collect();
        if let Containment::Proved { .. } = contained_in(&ds[i], &others, &f) {
            for k in &keys {
                if geo(&ds[i], k) {
                    assert!(others.iter().any(|o| o.applies(k) == Verdict::Applies));
                }
            }
        }
        match is_empty(&ds[i], &f) {
            Emptiness::ProvedEmpty { .. } => assert!(keys.iter().all(|k| !geo(&ds[i], k))),
            Emptiness::NonEmpty { witness } => assert!(geo(&ds[i], &witness)),
            Emptiness::Unknown { .. } => {}
        }
    }
}

/// FINDING (major, budgets; audit-part-B M3 not fully closed): in dispatch / FailClosed mode every blocked
/// sub-partition gets the FULL budget again (`partition_nf(sub, opts, None, Instant::now())`: fresh wall clock, fresh
/// node count), and the sub-partitions are still computed after the MAIN run aborted. A budgeted dispatch-mode run
/// therefore does up to (blockers + 1) x the stated work (T178 raw38: 5,395 blockers). The verdict stays honest
/// (Partial / Aborted, no proof), but the budget is not a bound on the run.
/// FIXED (partfix2, RA2-M5): ONE budget for the whole call. After the main run aborts no sub-partition runs (they
/// are listed as skipped, their keys read Unexplored, the point oracle stays exact); with a budget that the main run
/// survives, the main + sub-partition + loci work never exceeds it by more than one unit per consumer.
#[test]
fn defect_dispatch_budget_is_per_subpartition() {
    let (n, e) = (8, 4);
    let f = NodeFrame::without_header_lines_first(n, e);
    let (mut aborted_main, mut aborted_later) = (0, 0);
    for seed in 0..60u64 {
        let mut g = Rng(99 + seed);
        let ds: Vec<CaseDomain> = (0..24)
            .map(|_| gen_domain(&mut g, n, e, true, false))
            .collect();
        let l = list(&ds, n);
        let full = gap_partition(&l, &f, &dispatch(), None).unwrap();
        for max in [1u64, 2_000, 20_000] {
            let o = PartitionOptions {
                budget: Budget {
                    max_nodes: Some(max),
                    ..Default::default()
                },
                ..dispatch()
            };
            let rep = gap_partition(&l, &f, &o, None).unwrap();
            let sub_nodes: u64 = rep.blocked().iter().map(|(_, b)| b.stats().nodes).sum();
            let total = rep.stats().nodes + sub_nodes;
            if rep.stats().aborted {
                aborted_main += 1;
                assert!(
                    rep.blocked().is_empty(),
                    "seed {seed}: a sub-partition ran after the abort"
                );
                assert_eq!(rep.blocked_skipped().len(), full.nf().blockers.len());
            }
            if rep.outcome().is_aborted() {
                aborted_later += u64::from(!rep.stats().aborted);
                assert!(rep.proves_no_gap(CoverageNotion::DispatchCoverage).is_err());
                assert!(
                    total <= max + 1,
                    "seed {seed}: {total} recursion nodes > budget {max}"
                );
                for k in window(n, e, 2) {
                    assert_eq!(
                        rep.uncovered(&k).unwrap(),
                        full.uncovered(&k).unwrap(),
                        "{k:?}"
                    );
                    if rep.uncovered(&k).unwrap() {
                        assert_ne!(rep.class_of(&k).unwrap(), ClassHit::Covered, "{k:?}");
                    }
                }
            } else {
                assert_eq!(
                    rep.outcome(),
                    full.outcome(),
                    "seed {seed}: a run within budget"
                );
            }
        }
    }
    eprintln!(
        "aborted in the main run: {aborted_main}; aborted later (sub-partitions / loci): {aborted_later}"
    );
    assert!(aborted_main >= 10);
}

fn adv_tables(seed: u64) -> (usize, usize, Vec<CaseDomain>) {
    let s = seed.wrapping_mul(0xA076_1D64_78BD_642F) ^ 0x1234;
    let mut g = Rng(s);
    let n = 2 + g.below(4) as usize;
    let e = 1 + g.below(n as u64) as usize;
    let nr = 1 + g.below(9) as usize;
    let mut ds: Vec<CaseDomain> = vec![];
    for _ in 0..nr {
        let d = gen_domain_adv(&mut g, n, e, &ds);
        if d.validate().is_ok() {
            ds.push(d);
        }
    }
    (n, e, ds)
}

/// Locator (ignored): phase timings of one adversarial seed (env AUDIT_SEED).
#[test]
#[ignore]
fn locate_phases_of_one_seed() {
    let seed: u64 = std::env::var("AUDIT_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(6209);
    let (n, e, ds) = adv_tables(seed);
    for (i, d) in ds.iter().enumerate() {
        eprintln!(
            "n={n} e={e} record {i}: fixed {:?} target {:?} eqs {:?} bounds {:?} exc {:?} recenter {:?} poles {:?} elided {}",
            d.fixed,
            d.target,
            d.equations
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>(),
            d.bounds.iter().map(|x| x.to_string()).collect::<Vec<_>>(),
            d.exceptions
                .iter()
                .map(|b| b.iter().map(|x| x.to_string()).collect::<Vec<_>>())
                .collect::<Vec<_>>(),
            d.recenter_bounds
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>(),
            d.poles,
            d.witness_elided
        );
    }
    let f = NodeFrame::without_header_lines_first(n, e);
    let l = list(&ds, n);
    let t0 = std::time::Instant::now();
    let ms = |t0: &std::time::Instant| t0.elapsed().as_millis();
    let r = gap_partition(&l, &f, &absent(), None).unwrap();
    eprintln!("union {} ms ({:?})", ms(&t0), r.outcome().tag());
    let t0 = std::time::Instant::now();
    let _ = gap_partition(
        &l,
        &f,
        &PartitionOptions {
            memo: false,
            ..absent()
        },
        None,
    )
    .unwrap();
    eprintln!("union no-memo {} ms", ms(&t0));
    let t0 = std::time::Instant::now();
    let _ = gap_partition(&l, &f, &PartitionOptions::default(), None).unwrap();
    eprintln!("failclosed {} ms", ms(&t0));
    let t0 = std::time::Instant::now();
    let d = gap_partition(&l, &f, &dispatch(), None).unwrap();
    eprintln!(
        "dispatch {} ms, blockers kept {} empty {}",
        ms(&t0),
        d.blocked().len(),
        d.blocked_empty()
    );
    for i in 0..ds.len() {
        let t0 = std::time::Instant::now();
        let a = is_empty(&ds[i], &f);
        eprintln!(
            "is_empty({i}) {} ms {:?}",
            ms(&t0),
            std::mem::discriminant(&a)
        );
        let others: Vec<&CaseDomain> = ds
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, d)| d)
            .collect();
        let t0 = std::time::Instant::now();
        let b = contained_in(&ds[i], &others, &f);
        eprintln!(
            "contained_in({i}) {} ms {}",
            ms(&t0),
            match &b {
                Containment::Unknown { reason } => reason.clone(),
                x => format!("{:?}", std::mem::discriminant(x)),
            }
        );
        let t0 = std::time::Instant::now();
        let c = shadowed(&l, i, &f);
        eprintln!(
            "shadowed({i}) {} ms {}",
            ms(&t0),
            match &c {
                Containment::Unknown { reason } => reason.clone(),
                x => format!("{:?}", std::mem::discriminant(x)),
            }
        );
    }
}

/// Locator (ignored): the Ordinary-policy partition of one adversarial seed, with a wall budget (env AUDIT_WALL_S).
#[test]
#[ignore]
fn locate_ordinary_of_one_seed() {
    let seed: u64 = std::env::var("AUDIT_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(6209);
    let wall: Option<u64> = std::env::var("AUDIT_WALL_S")
        .ok()
        .and_then(|s| s.parse().ok());
    let (n, e, ds) = adv_tables(seed);
    let f = NodeFrame::without_header_lines_first(n, e);
    let l = list(&ds, n);
    let t0 = std::time::Instant::now();
    let env = |k: &str| std::env::var(k).ok().and_then(|s| s.parse::<u64>().ok());
    let mut lo = rustred_campaign_core::loci::LociOptions::default();
    if let Some(x) = env("AUDIT_LOCI_NODES") {
        lo.max_nodes = x;
    }
    if let Some(x) = env("AUDIT_SPLIT") {
        lo.max_split_nodes = x;
    }
    if let Some(x) = env("AUDIT_TRIES") {
        lo.tries = x as usize;
    }
    if let Some(x) = env("AUDIT_WIT") {
        lo.witnesses = x as usize;
    }
    let o = PartitionOptions {
        elided: ElidedPolicy::Ordinary,
        resolve_loci: env("AUDIT_NOLOCI").is_none(),
        loci: lo,
        budget: Budget {
            wall: wall.map(std::time::Duration::from_secs),
            ..Default::default()
        },
        ..Default::default()
    };
    let cb = |p: &rustred_campaign_core::partition::Progress| {
        eprintln!("progress nodes {} at {} ms", p.nodes, p.seconds_x1000)
    };
    let r = gap_partition(&l, &f, &o, Some(&cb)).unwrap();
    eprintln!(
        "ordinary {} ms outcome {} nodes {} U {} F {} loci {:?}",
        t0.elapsed().as_millis(),
        r.outcome().tag(),
        r.stats().nodes,
        r.n_u(),
        r.n_f(),
        r.resolution().map(|x| (
            x.loci.len(),
            x.classes_resolved,
            x.classes_capped,
            x.exact_errors
        ))
    );
}

/// Locator (ignored): print the adversarial tables of an RNG seed (env AUDIT_RSEED, as printed by the harness).
#[test]
#[ignore]
fn locate_tables_of_rng_seed() {
    let s: u64 = std::env::var("AUDIT_RSEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let mut g = Rng(s);
    let n = 2 + g.below(4) as usize;
    let e = 1 + g.below(n as u64) as usize;
    let nr = 1 + g.below(9) as usize;
    let mut ds: Vec<CaseDomain> = vec![];
    for _ in 0..nr {
        let d = gen_domain_adv(&mut g, n, e, &ds);
        if d.validate().is_ok() {
            ds.push(d);
        }
    }
    for (i, d) in ds.iter().enumerate() {
        eprintln!(
            "n={n} e={e} record {i}: fixed {:?} target {:?} eqs {:?} bounds {:?} exc {:?} recenter {:?}",
            d.fixed,
            d.target,
            d.equations
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>(),
            d.bounds.iter().map(|x| x.to_string()).collect::<Vec<_>>(),
            d.exceptions
                .iter()
                .map(|b| b.iter().map(|x| x.to_string()).collect::<Vec<_>>())
                .collect::<Vec<_>>(),
            d.recenter_bounds
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
        );
    }
}

fn vm_hwm_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1).and_then(|x| x.parse().ok()))
        })
        .unwrap_or(0)
}

/// FINDING (major, resources; audit-part-B M3 not closed for memory): a capped locus resolution turns EVERY region
/// still pending after the cap into its own open `Unresolved` region (the cap stops the work per call, not the
/// number of calls), so the report grows without a memory bound. Adversarial seed 6209 (arity 4, 9 records):
/// Ordinary policy, 5 fragile classes, every one capped: tens of thousands of loci / open regions and a peak RSS
/// measured here; the harness process holding a few such reports was killed at the 2 GB scope.
#[test]
fn defect_capped_loci_emit_unbounded_open_regions() {
    let (n, e, ds) = adv_tables(6209);
    let f = NodeFrame::without_header_lines_first(n, e);
    let l = list(&ds, n);
    let before = vm_hwm_kb();
    let t0 = std::time::Instant::now();
    let r = gap_partition(
        &l,
        &f,
        &PartitionOptions {
            elided: ElidedPolicy::Ordinary,
            ..Default::default()
        },
        None,
    )
    .unwrap();
    let res = r.resolution().unwrap();
    let parts: usize = res.loci.iter().map(|x| x.unresolved_parts.len()).sum();
    let constraints: usize = res
        .loci
        .iter()
        .flat_map(|x| x.unresolved_parts.iter().map(|u| u.region.cons.len()))
        .sum();
    eprintln!(
        "n={n}, {} records: {} F classes ({} capped), {} loci, {parts} open regions ({constraints} constraints), peak RSS {} -> {} MB, {} ms, outcome {}",
        ds.len(),
        r.n_f(),
        res.classes_capped,
        res.loci.len(),
        before / 1024,
        vm_hwm_kb() / 1024,
        t0.elapsed().as_millis(),
        r.outcome().tag()
    );
    assert!(!r.outcome().is_complete());
    assert!(r.proves_no_gap(CoverageNotion::UnionCoverage).is_err());
    // FIXED (partfix2, RA2-M5): at most `max_open_regions` open regions per resolved cell (+ one whole-cell summary)
    let lo = rustred_campaign_core::loci::LociOptions::default();
    assert!(
        parts <= res.classes_resolved * (lo.max_open_regions + 1),
        "open regions {parts} for {} cells",
        res.classes_resolved
    );
    assert!(vm_hwm_kb() / 1024 < 1024, "peak RSS below 1 GB");
    // every uncovered window key is still described (by a piece or an open whole-cell region)
    for k in window(n, e, 4) {
        let cov = ds.iter().any(|d| {
            let v = d.applies(&k);
            v == Verdict::Applies || (d.witness_elided && v == Verdict::Elided)
        });
        if !cov {
            assert_ne!(r.class_of(&k).unwrap(), ClassHit::Covered, "{k:?}");
            described(&r, &k).unwrap_or_else(|w| panic!("{k:?}: {w}"));
        }
    }
}

/// FINDING (CRITICAL if reproduced, contrived input): `loci::Resolver::ge` negates the bound's coefficients
/// UNCHECKED (`coef.iter().map(|a| -a)`); for a coefficient of exactly -2^127 the "q <= -1" half keeps the WRONG
/// sign, and `gcd`'s 2^127 -> i128::MAX clamp makes both halves look empty to Fourier-Motzkin: the fragile region is
/// split into two empty halves and its uncovered points vanish. A: bound -2^127 n0 + 5 >= 0 (never true on the
/// chart), B: exception n0 = n1, C: the point (1,1). The diagonal n0 = n1 >= 2 is uncovered, e.g. (2,2).
#[test]
fn defect_i128_min_bound_coefficient_drops_a_fragile_gap() {
    use rustred_campaign_core::economy::prove_coverage_preserved;
    use rustred_campaign_core::partition::DispatchOrderStatement;
    let (n, e) = (2, 2);
    let f = NodeFrame::without_header_lines_first(n, e);
    let mut a = free(n, e);
    a.bounds
        .push(p("-170141183460469231731687303715884105727*n0-n0+5", n));
    let mut b = free(n, e);
    b.exceptions.push(vec![p("n0-n1", n)]);
    let mut c = free(n, e);
    c.fixed = vec![Some(1), Some(1)];
    let ds = vec![a, b, c];
    assert!(
        !union_covered(&ds, &[2, 2]),
        "brute force: (2,2) is uncovered"
    );
    let rep = gap_partition(&list(&ds, n), &f, &absent(), None).unwrap();
    assert!(rep.uncovered(&[2, 2]).unwrap(), "the point oracle agrees");
    let res = rep.resolution().unwrap();
    eprintln!(
        "outcome {} U {} F {} loci {} capped {} proves_no_gap(union) {:?} class_of(2,2) {:?}",
        rep.outcome().tag(),
        rep.n_u(),
        rep.n_f(),
        res.loci.len(),
        res.classes_capped,
        rep.proves_no_gap(CoverageNotion::UnionCoverage),
        rep.class_of(&[2, 2]).unwrap()
    );
    let d = gap_partition(&list(&ds, n), &f, &dispatch(), None).unwrap();
    let proof = prove_coverage_preserved(
        &list(&ds, n),
        &list(&ds, n),
        &f,
        &Default::default(),
        &DispatchOrderStatement::SameOrder,
    )
    .ok();
    eprintln!(
        "dispatch: proves_no_gap {:?}; FULL-node containment proof issued: {}",
        d.proves_no_gap(CoverageNotion::DispatchCoverage),
        proof.is_some()
    );
    // FIXED (partfix2, RA2-C1): no proof, the gap is described, no FULL-node containment proof
    assert!(rep.proves_no_gap(CoverageNotion::UnionCoverage).is_err());
    assert!(d.proves_no_gap(CoverageNotion::DispatchCoverage).is_err());
    assert!(proof.is_none());
    for k in window(n, e, 9) {
        if !union_covered(&ds, &k) {
            assert!(rep.uncovered(&k).unwrap());
            assert_ne!(rep.class_of(&k).unwrap(), ClassHit::Covered, "{k:?}");
            described(&rep, &k).unwrap_or_else(|w| panic!("{k:?}: {w}"));
            assert_ne!(d.class_of(&k).unwrap(), ClassHit::Covered, "{k:?}");
        }
    }
}

// ------------------------------------------------------------------------------------------------------------------
// partfix2: adversarial generator of EXTREME coefficients (around +-2^63 and +-2^127) against brute force
// ------------------------------------------------------------------------------------------------------------------

/// Extreme values (decimal text; -2^127 is written as a sum because its absolute value does not parse).
const EXTREME: [&str; 12] = [
    "9223372036854775807",                          // 2^63 - 1
    "9223372036854775808",                          // 2^63
    "(-9223372036854775808)",                       // -2^63
    "(-9223372036854775809)",                       // -2^63 - 1
    "18446744073709551616",                         // 2^64
    "4611686018427387904",                          // 2^62
    "170141183460469231731687303715884105727",      // 2^127 - 1
    "(-170141183460469231731687303715884105727)",   // -(2^127 - 1)
    "(-170141183460469231731687303715884105727-1)", // -2^127
    "85070591730234615865843651857942052864",       // 2^126
    "(-85070591730234615865843651857942052864)",    // -2^126
    "170141183460469231731687303715884105726",      // 2^127 - 2
];

/// Exact brute force with extreme values (the core's BigInt: no i128 overflow in the reference).
fn big_affine(c: &Affine, t: &[i128]) -> rustred_campaign_core::poly::BigInt {
    use rustred_campaign_core::poly::BigInt;
    let mut acc = BigInt::from_i128(c.c);
    for (a, x) in c.coef.iter().zip(t) {
        acc = acc.add(&BigInt::from_i128(*a).mul(&BigInt::from_i128(*x)));
    }
    acc
}

fn brute_points_big(cons: &[Affine], k: usize, b: i128) -> Vec<Vec<i128>> {
    let mut out = vec![];
    let mut t = vec![-b; k];
    loop {
        if cons.iter().all(|c| big_affine(c, &t).signum() >= 0) {
            out.push(t.clone());
        }
        let mut j = 0;
        loop {
            if j == k {
                return out;
            }
            t[j] += 1;
            if t[j] <= b {
                break;
            }
            t[j] = -b;
            j += 1;
        }
    }
}

fn brute_linear_big(terms: &[(i128, Range)], c: i128, w: i64) -> bool {
    use rustred_campaign_core::poly::BigInt;
    fn rec(terms: &[(i128, Range)], acc: BigInt, w: i64) -> bool {
        let Some(((a, r), rest)) = terms.split_first() else {
            return acc.is_zero();
        };
        let (lo, hi) = match (r.lo, r.hi) {
            (Some(l), Some(h)) => (l, h),
            (Some(l), None) => (l, l + w),
            (None, Some(h)) => (h - w, h),
            (None, None) => (-w, w),
        };
        (lo..=hi).any(|x| {
            rec(
                rest,
                acc.add(&BigInt::from_i128(*a).mul(&BigInt::from_i128(x as i128))),
                w,
            )
        })
    }
    rec(terms, BigInt::from_i128(c), w)
}

fn extreme(g: &mut Rng) -> &'static str {
    EXTREME[g.below(EXTREME.len() as u64) as usize]
}

fn try_p(t: &str, n: usize) -> Option<Poly> {
    Poly::parse(t, n, false).ok()
}

/// A literal with at least one extreme coefficient or constant (`None` if it does not parse: overflow while
/// expanding is a parse error of the core, not a partition matter).
fn extreme_lit(g: &mut Rng, n: usize) -> Option<Poly> {
    let (a, b) = (slot(g, n), slot(g, n));
    let k = g.range(-4, 4);
    let (x, y, z) = (extreme(g), extreme(g), extreme(g));
    let t = match g.below(9) {
        0 => format!("{x}*n{a}-{x}*n{b}+({k})"),
        1 => format!("{x}*n{a}+({k})"),
        2 => format!("n{a}-{x}"),
        3 => format!("{x}*n{a}+{y}*n{b}+{z}"),
        4 => format!("{x}*n{a}-{x}*({k})"),
        5 => format!("{x}*n{a}^2-{y}"),
        6 => format!("({k})*n{a}+({})*n{b}+{x}", g.range(-3, 3)),
        7 => format!("{x}*n{a}*n{b}+({k})"),
        _ => format!("{x}*n{a}+{y}"),
    };
    try_p(&t, n)
}

fn gen_domain_extreme(g: &mut Rng, n: usize, e: usize) -> CaseDomain {
    let mut d = gen_domain(g, n, e, true, true);
    for _ in 0..1 + g.below(2) {
        let Some(q) = extreme_lit(g, n) else { continue };
        match g.below(6) {
            0 => d.equations.push(q),
            1 | 2 => d.bounds.push(q),
            3 => d.exceptions.push(vec![q]),
            4 => d.recenter_bounds.push(q),
            _ => {
                // a pole locus with an extreme d-coefficient: (q) * d + (small literal)
                let small = lit(g, n, false);
                let text = format!("({})*d+({small})", q);
                if let Ok(pp) = Poly::parse(&text, n, true) {
                    d.poles.push(PoleLocus::new(PoleKind::Denominator, &pp));
                }
            }
        }
    }
    d
}

fn check_case_extreme(seed: u64, t: &mut Tally, containment: bool) {
    let mut g = Rng(seed);
    let n = 2 + g.below(3) as usize; // arity 2..4
    let e = 1 + g.below(n as u64) as usize;
    let w: i32 = if n <= 3 { 6 } else { 4 };
    let nr = 1 + g.below(6) as usize;
    let mut ds: Vec<CaseDomain> = vec![];
    for _ in 0..nr {
        let d = if g.chance(2, 3) {
            gen_domain_extreme(&mut g, n, e)
        } else {
            gen_domain(&mut g, n, e, true, true)
        };
        if d.validate().is_ok() {
            ds.push(d);
        }
    }
    if ds.is_empty() {
        return;
    }
    check_tables(seed, n, e, w, ds, t, containment);
}

/// Exact tools with extreme coefficients: every answer is sound against brute force on a box (an overflow is an
/// `Err`, never a verdict, never a panic).
fn extreme_tools_case(g: &mut Rng, case: u64) {
    let pick = |g: &mut Rng| -> i128 {
        match g.below(6) {
            0 => i128::MIN,
            1 => i128::MAX,
            2 => (1i128 << 63) * if g.chance(1, 2) { 1 } else { -1 } + g.range(-2, 2) as i128,
            3 => (1i128 << 126) * if g.chance(1, 2) { 1 } else { -1 },
            4 => i128::MIN + g.range(0, 3) as i128,
            _ => g.range(-5, 5) as i128,
        }
    };
    // linear feasibility on a box
    let nv = 1 + g.below(3) as usize;
    let terms: Vec<(i128, Range)> = (0..nv).map(|_| (pick(g), rand_range(g))).collect();
    let c = pick(g);
    if let Ok(false) = linear_feasible(&terms, c) {
        assert!(
            !brute_linear_big(&terms, c, 40),
            "case {case}: infeasible claimed: {terms:?} c={c}"
        );
    }
    // integer roots: every claimed root is a root, and every root in a box is claimed
    let cs: Vec<i128> = (0..2 + g.below(2)).map(|_| pick(g)).collect();
    if cs.iter().any(|&x| x != 0)
        && let Ok(rs) = int_roots(&cs)
    {
        for &r in &rs {
            assert_eq!(
                rustred_campaign_core::partition::exact::univ_sign(&cs, r),
                0,
                "case {case}: {cs:?} root {r}"
            );
        }
        for x in -50i128..=50 {
            if rustred_campaign_core::partition::exact::univ_sign(&cs, x) == 0 {
                assert!(rs.contains(&x), "case {case}: {cs:?} misses the root {x}");
            }
        }
    }
    // Fourier-Motzkin / exhaustive search / lattice cut on a box with extreme constraints
    let k = 1 + g.below(2) as usize;
    let b = 4i128;
    let mut cons = vec![];
    for j in 0..k {
        let mut lo = vec![0i128; k];
        lo[j] = 1;
        cons.push(aff(&lo, b));
        let mut hi = vec![0i128; k];
        hi[j] = -1;
        cons.push(aff(&hi, b));
    }
    let coef: Vec<i128> = (0..k).map(|_| pick(g)).collect();
    cons.push(aff(&coef, pick(g)));
    let pts = brute_points_big(&cons, k, b);
    if fm_feasible(&cons, k) == Ok(false) {
        assert!(
            pts.is_empty(),
            "case {case}: FM refuted a system with {} points: {cons:?}",
            pts.len()
        );
    }
    let mut budget = 100_000u64;
    match exhaustive_point(&cons, k, &mut budget) {
        Ok(Some(None)) => assert!(
            pts.is_empty(),
            "case {case}: exhaustive: empty claimed: {cons:?}"
        ),
        Ok(Some(Some(t))) => assert!(
            cons.iter()
                .all(|a| a.eval(&t).map(|v| v >= 0).unwrap_or(false))
        ),
        _ => {}
    }
    if let Ok(Some(t)) = rustred_campaign_core::loci::region::find_point(&cons, k, 8, &vec![0; k]) {
        assert!(
            cons.iter()
                .all(|a| a.eval(&t).map(|v| v >= 0).unwrap_or(false)),
            "case {case}"
        );
    }
    let r = Region {
        k,
        x: (0..k).map(|j| Affine::var(k, j)).collect(),
        cons: cons[..2 * k].to_vec(),
        names: (0..k).map(|j| format!("t{j}")).collect(),
    };
    let (cc, c0) = ((0..k).map(|_| pick(g)).collect::<Vec<_>>(), pick(g));
    let sols: Vec<Vec<i128>> = brute_points_big(&r.cons, k, b)
        .into_iter()
        .filter(|t| big_affine(&aff(&cc, c0), t).is_zero())
        .collect();
    match r.cut(&cc, c0) {
        Ok(None) => assert!(
            sols.is_empty(),
            "case {case}: cut None but solutions {sols:?}"
        ),
        Ok(Some(s)) => {
            for t in &sols {
                let key: Vec<i32> = t.iter().map(|&v| v as i32).collect();
                assert!(
                    solve_params(&s, &key).is_some(),
                    "case {case}: {t:?} not on the lattice"
                );
            }
        }
        Err(_) => {}
    }
}

/// partfix2: extreme-coefficient adversarial campaign, 20,000 table sets through the full brute-force harness
/// (union / memo / capped / FailClosed / Ordinary / dispatch vs `first_match`; emptiness / containment / shadowing on
/// every 4th set) plus 20,000 exact-tool cases.
#[test]
fn diff_extreme_coefficients_against_brute_force() {
    let mut t = Tally::default();
    let n_sets: u64 = std::env::var("AUDIT_EXTREME_SETS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20_000);
    for seed in 0..n_sets {
        check_case_extreme(
            seed.wrapping_mul(0x9FB2_1C65_1E98_DF25) ^ 0xE7,
            &mut t,
            seed % 4 == 0,
        );
    }
    let mut g = Rng(0xE77E);
    for case in 0..20_000u64 {
        extreme_tools_case(&mut g, case);
    }
    eprintln!("{t:#?}");
    assert!(t.cases >= n_sets * 9 / 10, "cases {}", t.cases);
}

/// Locator (ignored): one extreme-coefficient seed (env AUDIT_RSEED = the RNG seed printed by the harness) and a key
/// (env AUDIT_KEY = "1,1,-5"): normal-form notes, rules and per-record verdicts at the key.
#[test]
#[ignore]
fn locate_extreme_seed() {
    let s: u64 = std::env::var("AUDIT_RSEED")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(0);
    let key: Vec<i32> = std::env::var("AUDIT_KEY")
        .unwrap_or_default()
        .split(',')
        .filter_map(|x| x.trim().parse().ok())
        .collect();
    let mut g = Rng(s);
    let n = 2 + g.below(3) as usize;
    let e = 1 + g.below(n as u64) as usize;
    let nr = 1 + g.below(6) as usize;
    let mut ds: Vec<CaseDomain> = vec![];
    for _ in 0..nr {
        let d = if g.chance(2, 3) {
            gen_domain_extreme(&mut g, n, e)
        } else {
            gen_domain(&mut g, n, e, true, true)
        };
        if d.validate().is_ok() {
            ds.push(d);
        }
    }
    let f = NodeFrame::without_header_lines_first(n, e);
    let rep = gap_partition(&list(&ds, n), &f, &absent(), None).unwrap();
    eprintln!("notes {:?}", rep.nf().notes);
    eprintln!("sources {:?}", rep.nf().sources);
    for (i, r) in rep.nf().rules.iter().enumerate() {
        let show = |ls: &[u32]| {
            ls.iter()
                .map(|&l| rep.nf().lits[l as usize].text.clone())
                .collect::<Vec<_>>()
        };
        eprintln!(
            "rule {i}: eq {:?} ge {:?} bad {:?} covers {:?}",
            show(&r.eq),
            show(&r.ge),
            r.bad.iter().map(|b| show(b)).collect::<Vec<_>>(),
            rep.nf().rule_covers(i, &key)
        );
    }
    for (i, d) in ds.iter().enumerate() {
        eprintln!("record {i}: applies {:?}", d.applies(&key));
    }
}
