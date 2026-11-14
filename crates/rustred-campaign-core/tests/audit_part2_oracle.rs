//! Independent re-audit of the partition / loci / containment (lane audit-part2, 2026-11-14). Synthetic tables only
//! (lane rule L8). Two kinds of tests:
//! - `defect_*`: CONFIRMING reproductions of findings (they assert the defect as it is today; a fix inverts the
//!   assertion, as the first audit round's tests were ported);
//! - `diff_*` / `exact_*`: differential property tests against brute force (they must pass; a failure is a finding).
//! Report: WS/notes/implementer.reaudit-part.md.

#![allow(clippy::too_many_arguments)]

use std::collections::BTreeSet;

use rustred_campaign_core::cover::{Dispatcher, RuleRef, Table};
use rustred_campaign_core::domain::{CaseDomain, PoleKind, PoleLocus, Stage, Verdict};
use rustred_campaign_core::key::{Key, NodeFrame};
use rustred_campaign_core::loci::Region;
use rustred_campaign_core::loci::region::{exhaustive_point, fm_feasible};
use rustred_campaign_core::partition::exact::{Affine, Range, gcd, int_roots, linear_feasible};
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
                let r = rep.states[s][v as usize];
                (r.lo, r.hi)
            }
            Some(_) => {
                if rep.nf.lines[s] {
                    (Some(2), None)
                } else {
                    (None, Some(-1))
                }
            }
            None => {
                if rep.nf.lines[s] {
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
    let fk = face_of(&rep.nf.lines, k);
    let Some(face) = rep.faces.iter().find(|f| f.key() == fk) else {
        return Err(format!("face {fk:?} not listed"));
    };
    let mut reps: Vec<&PartitionReport> = vec![rep];
    reps.extend(rep.blocked.iter().map(|(_, b)| b));
    let slots: Vec<usize> = face.sd.iter().chain(&face.sn).copied().collect();
    for r in &reps {
        let cell: Vec<u16> = slots
            .iter()
            .map(|&s| r.state_index(s, k[s] as i64).unwrap())
            .collect();
        // the face's generic cells were merged from the sub-reports with their OWN state indices; check by the
        // classes instead: a U class of r containing k
        let hit = r.class_of(k).unwrap();
        if hit == ClassHit::Uncovered && r.classes.iter().any(|c| c.label == Label::Uncovered) {
            // must be in a listed generic cell of r's own faces
            if let Some(f) = r.faces.iter().find(|f| f.key() == fk) {
                if f.generic_cells.iter().any(|c| {
                    c.iter()
                        .zip(&cell)
                        .all(|(&cs, &st)| cs == st || (cs == ANY_OFF && st != 0))
                }) {
                    return Ok(());
                }
            }
        }
        if let Some(res) = &r.resolution {
            for l in res.loci.iter().filter(|l| l.face == fk) {
                for pc in &l.pieces {
                    if let Some(t) = solve_params(&pc.region, k)
                        && pc.region.contains_t(&t).unwrap()
                    {
                        return Ok(());
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
    }
    Err(format!("key on face {fk:?} in no generic cell / locus piece"))
}

// ------------------------------------------------------------------------------------------------------------------
// targeted reproductions
// ------------------------------------------------------------------------------------------------------------------

/// FINDING (major, exactness): a multi-slot BOUND that varies on a class KILLS its rule there
/// (`classes.rs` `ge_val`: `_ => CondVal::Kill`), so a class whose every point is covered by half-space rules is
/// labelled Uncovered and listed as a GENERIC gap face with proof `exact_partition`. Here A: n0 - n1 >= 0 and
/// B: n1 - n0 - 1 >= 0 cover the whole chart, yet the partition lists the face {n0, n1}.
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
    assert!(rep.outcome.is_complete());
    // the defect: a U class and a generic face with an exact-partition proof, on a fully covered chart
    assert_eq!(rep.n_u(), 1, "U classes");
    let face = rep.faces.iter().find(|x| x.key() == (vec![0, 1], vec![])).expect("face listed");
    assert_eq!(face.kind, rustred_campaign_core::partition::FaceKind::Generic);
    assert_eq!(face.proof.tag(), "exact_partition");
    assert!(face.witness.is_none(), "no uncovered key exists");
    assert!(rep.proves_no_gap(CoverageNotion::UnionCoverage).is_err());
    // the census sees it: every key of the class is a covered key in a U class
    let c = rep.census(&window(n, e, 8)).unwrap();
    assert!(c.uncovered.is_empty());
    assert_eq!(c.covered_in_u.len(), 49);
}

/// FINDING (major, exactness): a ONE-slot NONLINEAR bound is never decided on an interval state (`ge_status`
/// only handles constants and linear forms), although the slot states are split at every sign change of the bound,
/// so its sign IS constant on each state. A: n0^2 - 4 >= 0 (true for n0 >= 2) plus B: n0 = 1 cover every key of a
/// one-line node, but the tail state n0 >= 3 is labelled Uncovered.
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
    assert!(rep.n_u() >= 1, "the tail is labelled Uncovered");
    assert!(rep.faces.iter().any(|x| x.key() == (vec![0], vec![])));
    assert!(rep.uncovered(&[7]).unwrap() == false, "the point oracle knows it is covered");
    assert_eq!(rep.class_of(&[7]).unwrap(), ClassHit::Uncovered);
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
        let mut want: BTreeSet<i64> = BTreeSet::new();
        match g.below(3) {
            0 => {
                for _ in 0..g.below(4) {
                    let r = g.range(-30, 30);
                    want.insert(r);
                    c = poly_mul(&c, &[-(r as i128), 1]);
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
        let brute: BTreeSet<i64> = (-3000i128..=3000)
            .filter(|&x| univ_eval(&c, x) == Some(0))
            .map(|x| x as i64)
            .collect();
        if !want.is_empty() {
            assert!(want.is_subset(&brute));
        }
        let got: BTreeSet<i64> = int_roots(&c)
            .unwrap_or_else(|e| panic!("case {case}: {c:?}: {e}"))
            .into_iter()
            .collect();
        assert_eq!(got, brute, "case {case}: roots of {c:?}");
    }
    // far roots: (x - 10^9)(x + 3), (x - 2^40)(x - 1), x^3 (x - 7^11)
    let far: [(Vec<i128>, Vec<i64>); 3] = [
        (poly_mul(&[-1_000_000_000, 1], &[3, 1]), vec![-3, 1_000_000_000]),
        (poly_mul(&[-(1i128 << 40), 1], &[-1, 1]), vec![1, 1 << 40]),
        (
            poly_mul(&[0, 0, 0, 1], &[-(7i128.pow(11)), 1]),
            vec![0, 7i64.pow(11)],
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
            assert!(pts.is_empty(), "case {case}: FM refuted a system with {} points: {cons:?}", pts.len());
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
            Some(None) => assert!(pts.is_empty(), "case {case}: exhaustive said empty: {cons:?}"),
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
                let gg = coef.iter().fold(0, |a, &x| gcd(a, x));
                assert!(sols.is_empty(), "case {case}: cut None but {} solutions", sols.len());
                assert!(gg != 0 && c % gg != 0 || gg == 0 && c != 0, "case {case}: None without a gcd reason");
            }
            Some(s) => {
                for t in &sols {
                    let key: Vec<i32> = t.iter().map(|&v| v as i32).collect();
                    let tt = solve_params(&s, &key)
                        .unwrap_or_else(|| panic!("case {case}: solution {t:?} not on the lattice"));
                    assert!(s.contains_t(&tt).unwrap(), "case {case}: {t:?} outside the constraints");
                }
                if let Ok(Some(t)) = s.point(8) {
                    let x = s.at(&t).unwrap();
                    assert_eq!(aff(&coef, c).eval(&x).unwrap(), 0, "case {case}");
                }
            }
        }
    }
}

/// FINDING (minor): `gcd` clamps 2^127 to i128::MAX, which does not divide i128::MIN.
#[test]
fn defect_gcd_clamps_two_to_the_127() {
    let g = gcd(i128::MIN, 0);
    assert_eq!(g, i128::MAX);
    assert_ne!(i128::MIN % g, 0, "the returned gcd does not divide its argument");
}

/// FINDING (major, exactness): a locus piece whose region is rationally feasible but has NO integer point is
/// reported (`loci::subtract_and_emit` drops a piece only when Fourier-Motzkin refutes it; FM with rounding is not
/// an integer-emptiness decision, and the result depends on the elimination order). The polygon
/// -3 n0 + 4 n1 + 12 >= 0, -4 n0 - 4 n1 + 43 >= 0, 5 n0 - 4 n1 - 24 >= 0 has no integer point; rules A, B, C cover
/// its complement and F (exception n2 = n0 + n1) covers everything off that plane. Every key of the chart is
/// covered, yet a LOCUS-ONLY gap face {n0, n1, n2} is listed with proof exact_partition and a piece without
/// witness; the crate's own `exhaustive_point` proves the piece's region integer-empty (found by
/// `explore_integer_empty_pieces`: 5 such pieces in 39 FM-robust polygons).
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
    assert!(rep.outcome.is_complete());
    let res = rep.resolution.as_ref().unwrap();
    let mut empty_pieces = 0;
    for l in &res.loci {
        for pc in &l.pieces {
            let mut budget = 1_000_000u64;
            if exhaustive_point(&pc.region.cons, pc.region.k, &mut budget).unwrap() == Some(None) {
                empty_pieces += 1;
                assert!(pc.witness.is_empty());
                assert!(!pc.covered_part_unresolved, "not even flagged");
                assert_eq!(l.proof.tag(), "exact_partition");
            }
        }
    }
    assert!(empty_pieces >= 1, "an integer-empty piece is reported");
    let face = rep.faces.iter().find(|x| x.key() == (vec![0, 1, 2], vec![])).expect("spurious face listed");
    assert_eq!(face.kind, rustred_campaign_core::partition::FaceKind::LocusOnly);
    assert_eq!(face.proof.tag(), "exact_partition");
    assert!(face.witness.is_none());
    assert!(rep.proves_no_gap(CoverageNotion::UnionCoverage).is_err(), "conservative: no proof");
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
        2 => format!("{}*n{a}+{}*n{b}+{}*n{c}-({k})", coefs(g), coefs(g), coefs(g)),
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
    let ds: Vec<CaseDomain> = (0..nr).map(|_| gen_domain(&mut g, n, e, true, true)).collect();
    let f = NodeFrame::without_header_lines_first(n, e);
    let l = list(&ds, n);
    let keys = window(n, e, w);
    t.cases += 1;
    t.keys += keys.len() as u64;
    let what = |m: &str| format!("seed {seed} (n={n}, e={e}, {nr} rules) {m}: {ds:?}");

    // ---------------- union mode (elided records absent: brute force = some record APPLIES)
    let rep = gap_partition(&l, &f, &absent(), None).unwrap_or_else(|e| panic!("{}: {e:?}", what("union")));
    match &rep.outcome {
        o if o.is_complete() => t.complete += 1,
        rustred_campaign_core::partition::Outcome::Incomplete { .. } => t.incomplete += 1,
        o => panic!("{}: unexpected outcome {o:?}", what("union")),
    }
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
    if rep.proves_no_gap(CoverageNotion::UnionCoverage).is_ok() {
        t.union_proofs += 1;
        assert!(!any_unc, "{}: FALSE union no-gap proof", what("union"));
    }
    // witnesses are uncovered; flagged pieces counted
    if let Some(res) = &rep.resolution {
        for loc in &res.loci {
            for pc in &loc.pieces {
                t.flagged_pieces += u64::from(pc.covered_part_unresolved);
                for wk in &pc.witness {
                    assert!(!union_covered(&ds, wk), "{}: covered witness {wk:?}", what("union"));
                }
            }
        }
    }
    // spurious U classes: a bounded U class inside the window whose every key is covered
    for c in rep.classes.iter().filter(|c| c.label == Label::Uncovered) {
        let b = cell_box(&rep, &c.states);
        let inside: Vec<&Key> = keys.iter().filter(|k| in_box(&b, k)).collect();
        if box_inside_window(&b, &rep.nf.lines, w) && inside.iter().all(|k| union_covered(&ds, k)) {
            t.spurious_u_bounded += 1;
        }
        t.covered_in_u += inside.iter().filter(|k| union_covered(&ds, k)).count() as u64;
    }
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
    let ca: BTreeSet<_> = rep.classes.iter().cloned().collect();
    let cb: BTreeSet<_> = r2.classes.iter().cloned().collect();
    assert_eq!(ca, cb, "{}: memo vs reference classes", what("union"));
    // determinism: the same report from a fresh thread (cold thread-local caches)
    if seed % 8 == 0 {
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

    // ---------------- dispatch mode against the core dispatcher's first match
    let mut tab = Table::new("t", n);
    for (i, d) in ds.iter().enumerate() {
        tab.push(i as i64, d.clone()).unwrap();
    }
    let disp = Dispatcher::new(vec![tab], vec![0]).unwrap();
    let drep = gap_partition(&l, &f, &dispatch(), None).unwrap_or_else(|e| panic!("{}: {e:?}", what("dispatch")));
    let mut any_open = false;
    for k in &keys {
        let fm = disp.first_match(k).unwrap();
        let open = fm.is_open();
        // the engine's modular zero test (documented UNKNOWN) never triggers on these small values
        assert_eq!(drep.uncovered(k).unwrap(), open, "{}: dispatch oracle at {k:?} ({fm:?})", what("dispatch"));
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
                panic!("{}: open {k:?} not described: {why} ({fm:?})", what("dispatch"));
            }
        }
    }
    if drep.proves_no_gap(CoverageNotion::DispatchCoverage).is_ok() {
        t.dispatch_proofs += 1;
        assert!(!any_open, "{}: FALSE dispatch no-gap proof", what("dispatch"));
    }

    if !containment {
        return;
    }
    // ---------------- emptiness / containment / shadowing
    let geo = |d: &CaseDomain, k: &[i32]| {
        let v = d.applies(k);
        v == Verdict::Applies || (d.witness_elided && v == Verdict::Elided)
    };
    for i in 0..nr {
        let d = &ds[i];
        match is_empty(d, &f) {
            Emptiness::ProvedEmpty { certificate } => {
                t.empty_ok += 1;
                for k in &keys {
                    assert!(!geo(d, k), "{}: ProvedEmpty({certificate}) but {k:?} applies (record {i})", what("empty"));
                }
            }
            Emptiness::NonEmpty { witness } => {
                assert!(geo(d, &witness), "{}: bad NonEmpty witness {witness:?} (record {i})", what("empty"));
            }
            Emptiness::Unknown { .. } => t.empty_unknown += 1,
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
                    !others.iter().any(|o| o.applies(&witness) == Verdict::Applies),
                    "{}: witness {witness:?} covered by an other",
                    what("contain")
                );
            }
            Containment::Unknown { .. } => t.contain_unknown += 1,
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
                assert!(geo(d, &witness), "{}: shadow witness {witness:?}", what("shadow"));
                assert!(
                    !(0..i).any(|j| ds[j].applies(&witness).stage() != Stage::NotSelected),
                    "{}: shadow witness {witness:?} matched by an earlier record",
                    what("shadow")
                );
            }
            Containment::Unknown { .. } => t.shadow_unknown += 1,
        }
    }
}

/// The differential harness, default size (2,000 random table sets; containment on every case).
#[test]
fn diff_random_tables_against_brute_force() {
    let mut t = Tally::default();
    for seed in 0..2_000u64 {
        check_case(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0x51, &mut t, true);
    }
    eprintln!("{t:#?}");
}

/// The differential harness, large size (run with `--ignored`): 40,000 random table sets.
#[test]
#[ignore]
fn diff_random_tables_against_brute_force_large() {
    let mut t = Tally::default();
    for seed in 0..40_000u64 {
        check_case(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xA5A5, &mut t, true);
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
            gs.push((g.range(-5, 5) as i128, g.range(-5, 5) as i128, g.range(-60, 60) as i128));
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
        let Some(res) = rep.resolution.as_ref() else { continue };
        for l in &res.loci {
            for pc in &l.pieces {
                let mut budget = 1_000_000u64;
                if exhaustive_point(&pc.region.cons, pc.region.k, &mut budget).unwrap() == Some(None) {
                    found += 1;
                    if found <= 3 {
                        eprintln!("case {case}: polygon {gs:?}: empty piece x {:?} cons {:?} faces {:?} witness {:?}", pc.region.render_x(), pc.region.render_cons(), rep.faces.iter().map(|x| (x.key(), x.kind, x.proof.tag())).collect::<Vec<_>>(), pc.witness);
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
    eprintln!("states {:?}", rep.states);
    eprintln!("classes {:?}", rep.classes);
    let res = rep.resolution.as_ref().unwrap();
    eprintln!("resolved {} with gaps {} capped {} errors {}", res.classes_resolved, res.classes_with_gaps, res.classes_capped, res.exact_errors);
    // the triangle in the cut coordinates, both variable orders
    for (cons, what) in [
        (vec![aff(&[1, 0], -2), aff(&[0, 1], -2), aff(&[0, -1], 11), aff(&[1, 0], -5), aff(&[3, 1], -39), aff(&[-3, 1], 18)], "n0,n1"),
        (vec![aff(&[0, 1], -2), aff(&[1, 0], -2), aff(&[-1, 0], 11), aff(&[0, 1], -5), aff(&[1, 3], -39), aff(&[1, -3], 18)], "n1,n0"),
    ] {
        eprintln!("{what}: fm {:?}", fm_feasible(&cons, 2));
    }
}
