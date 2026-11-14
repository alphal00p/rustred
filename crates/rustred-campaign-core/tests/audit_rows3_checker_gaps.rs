//! Independent audit `audit-rows3`: tests for checker paths that SURVIVED source mutation (WS/notes/implementer.reaudit-rows3.md,
//! mutation table). Each test names the mutants it kills. They pass on the checker as delivered.

mod rows_common;

use std::cell::Cell;

use rows_common::face::{face, served};
use rows_common::*;
use rustred_campaign_core::ibp_rows::{
    CellStatus, PointSpec, ReplayLevel, ReplayPolicy, RowsOptions, RowsRule, SymUse, Unsupported,
    Verdict, identity_test, identity_test_with, replay_evidence,
};
use rustred_campaign_core::independent_algebra::{Independent, elim};
use rustred_campaign_core::replay_base::algebra::{
    CheckAlgebra, CoeffError, CoeffStats, Ctx, Eliminator, Q, QResult, Span, Unknown, ZPoly,
};
use rustred_campaign_core::replay_base::family::Family;
use rustred_campaign_core::replay_base::primes::verify_primes;
use rustred_campaign_core::replay_base::symrows::{DeriveOptions, verify_derived};
use serde_json::json;
use std::sync::Arc;

const CORNER: &str = "I(1,1,1,1,1,1,1,0,0)";

/// Kills M01: in the span at one prime and not at the other is `Undecided`, never `Pass` (a coefficient term
/// `p1 * corner` vanishes modulo p1 only; caller primes are used to place it).
#[test]
fn audit_rows3_mixed_primes_are_never_a_pass() {
    let f = face();
    let n = f.fam.n();
    let [p1, p2] = verify_primes(99, &[]);
    let mut r = f.good.clone();
    r["rhs"].as_array_mut().unwrap().push(json!([CORNER, p1.to_string()]));
    let rule = RowsRule::from_value(r, n).unwrap();
    let v = identity_test(
        &f.fam,
        &rule,
        &[PointSpec::Generic],
        &RowsOptions {
            primes: Some([p1, p2]),
            ..RowsOptions::default()
        },
    );
    println!("mixed primes: {:?}", v.verdict);
    assert!(matches!(v.verdict, Verdict::Undecided { .. }), "{:?}", v.verdict);
}

/// Kills M09: ONE non-applicable served key among applicable ones blocks the level.
#[test]
fn audit_rows3_one_non_applicable_served_key_blocks() {
    let f = face();
    let n = f.fam.n();
    let good = RowsRule::from_value(f.good.clone(), n).unwrap();
    let mut keys = served(&f);
    let mut outside = keys[0].clone();
    outside[0] = 0; // a chart line at 0: outside the chart orthant
    keys.push(outside);
    let r = replay_evidence(&f.fam, &good, &keys, &ReplayPolicy::default(), &bound(&f.fam));
    println!("one non-applicable key: {:?}", r.level);
    assert!(matches!(r.level, ReplayLevel::NotEstablished { .. }));
}

/// Kills M20: a served key off the case equations is NotApplicable (the equation is checked at integer keys).
#[test]
fn audit_rows3_case_equation_checked_at_keys() {
    let f = face();
    let n = f.fam.n();
    let mut r = f.good.clone();
    r["equations"] = json!(["n0-n1"]);
    let rule = RowsRule::from_value(r, n).unwrap();
    let key: Vec<i64> = served(&f)
        .into_iter()
        .find(|k| {
            let pt: Vec<i64> = k.iter().zip(&f.tshift).map(|(a, b)| a - b).collect();
            pt[0] != pt[1] && k[f.j] != 0
        })
        .unwrap();
    let v = identity_test(&f.fam, &rule, &[PointSpec::Key(key)], &RowsOptions::default());
    println!("key off the equation: {:?}", v.verdict);
    assert!(matches!(v.verdict, Verdict::NotApplicable { .. }), "{:?}", v.verdict);
}

/// Kills M21, M34: caller primes outside the verify range (not in the FORBIDDEN list) are refused.
#[test]
fn audit_rows3_out_of_range_caller_primes_refused() {
    let f = face();
    let n = f.fam.n();
    let good = RowsRule::from_value(f.good.clone(), n).unwrap();
    // two primes just above 2^62 (outside [7*2^60, 7.5*2^60), not named in FORBIDDEN)
    let lo = [4_611_686_018_427_388_039u64, 4_611_686_018_427_388_073u64];
    for p in lo {
        assert!(Independent.is_prime(p), "{p}");
    }
    let v = identity_test(
        &f.fam,
        &good,
        &[PointSpec::Generic],
        &RowsOptions {
            primes: Some(lo),
            ..RowsOptions::default()
        },
    );
    println!("out-of-range primes: {:?}", v.verdict);
    assert!(matches!(v.verdict, Verdict::Unsupported(Unsupported::BadPrime(_))));
}

/// Kills M23: a group verified on ANOTHER family text (same arity and chart) is refused.
#[test]
fn audit_rows3_group_of_another_family_refused() {
    let src = data("LA3_handwritten.sym");
    let fa = Family::parse(&src).unwrap();
    let fb = Family::parse(&src.replace("name(LA3)", "name(LA3b)")).unwrap();
    let chart: Vec<bool> = (0..9).map(|i| i < 7).collect();
    let g = verify_derived(&fa, &chart, &DeriveOptions::default()).unwrap();
    let f = face();
    let good = RowsRule::from_value(f.good.clone(), 9).unwrap();
    let v = identity_test(
        &fb,
        &good,
        &[PointSpec::Generic],
        &RowsOptions {
            symmetry: Some(Arc::new(g)),
            sym_use: SymUse::Always,
            ..RowsOptions::default()
        },
    );
    println!("group of another family: {:?}", v.verdict);
    assert!(matches!(v.verdict, Verdict::Unsupported(Unsupported::SymGroupMismatch(_))));
}

/// Kills M27: the generic point values are random (differ between the two generic points and between seeds).
#[test]
fn audit_rows3_generic_values_are_drawn() {
    let f = face();
    let good = RowsRule::from_value(f.good.clone(), 9).unwrap();
    let v = identity_test(&f.fam, &good, &[PointSpec::Generic, PointSpec::Generic], &RowsOptions::default());
    let g0 = v.points[0].cells[0].generic_values.clone().unwrap();
    let g1 = v.points[1].cells[0].generic_values.clone().unwrap();
    println!("generic values differ: {}", g0 != g1);
    assert_ne!(g0, g1);
    assert!(g0.iter().filter(|&&x| x == g0[0]).count() < g0.len());
}

// ------------------------------------------------------------------ a control that fails ---------------------------

/// A backend whose `nth` span test of every elimination answers "in the span" (1 = the +1 mutant, >= 2 = corners).
struct ControlLie(usize);

struct LieElim<'r> {
    inner: elim::Elim<'r>,
    nth: usize,
    calls: Cell<usize>,
}

impl Eliminator for LieElim<'_> {
    fn rank(&self) -> usize {
        self.inner.rank()
    }
    fn entries(&self) -> usize {
        self.inner.entries()
    }
    fn pivot_columns(&self) -> Vec<usize> {
        self.inner.pivot_columns()
    }
    fn solve(&self, desired: &[(usize, u64)]) -> Result<Span, Unknown> {
        let k = self.calls.get();
        self.calls.set(k + 1);
        let mut s = self.inner.solve(desired)?;
        if k == self.nth {
            s.in_span = true;
            s.remainder.clear();
        }
        Ok(s)
    }
}

macro_rules! delegate {
    ($($f:ident($($a:ident: $t:ty),*) -> $r:ty;)*) => {
        $(fn $f(&self, $($a: $t),*) -> $r { Independent.$f($($a),*) })*
    };
}

impl CheckAlgebra for ControlLie {
    type Literal = <Independent as CheckAlgebra>::Literal;
    type Elim<'r> = LieElim<'r>;
    fn name(&self) -> &'static str {
        "control-lie"
    }
    delegate! {
        is_prime(n: u64) -> bool;
        add(p: u64, a: u64, b: u64) -> u64;
        sub(p: u64, a: u64, b: u64) -> u64;
        mul(p: u64, a: u64, b: u64) -> u64;
        neg(p: u64, a: u64) -> u64;
        inv(p: u64, a: u64) -> Option<u64>;
        from_i128(p: u64, x: i128) -> u64;
        q_new(n: i128, d: i128) -> QResult;
        q_add(a: Q, b: Q) -> QResult;
        q_sub(a: Q, b: Q) -> QResult;
        q_mul(a: Q, b: Q) -> QResult;
        q_div(a: Q, b: Q) -> QResult;
        q_neg(a: Q) -> QResult;
        q_parse(s: &str) -> QResult;
        q_modp(a: Q, p: u64) -> Option<u64>;
        q_inverse(m: &[Vec<Q>]) -> Result<Vec<Vec<Q>>, Unknown>;
        q_rank(rows: &[Vec<Q>]) -> Result<usize, Unknown>;
        det_i128(m: &[Vec<i128>]) -> Option<i128>;
        q_rref(rows: &[Vec<Q>], ncols: usize) -> Result<(Vec<Vec<Q>>, Vec<usize>), Unknown>;
        q_unit_in_row_space(rows: &[Vec<Q>], col: usize) -> Result<bool, Unknown>;
        gcd_i128(a: i128, b: i128) -> i128;
        zpoly_det(m: &[Vec<ZPoly>], dim: usize, t: usize) -> ZPoly;
        cross_check_coefficient(text: &str, arity: usize, ctx: &Ctx, num: u64, den: u64) -> Result<(), Unknown>;
        expr_linear(text: &str, n: usize) -> Result<Vec<Q>, Unknown>;
        expr_has_d(text: &str, n: usize) -> Result<bool, Unknown>;
        literal_parse(text: &str, n: usize) -> Result<Self::Literal, Unknown>;
        literal_is_zero_at(lit: &Self::Literal, point: &[i32]) -> Result<bool, Unknown>;
        literal_same_up_to_constant(a: &str, b: &str, n: usize) -> bool;
    }
    fn eval_ratio(
        &self,
        text: &str,
        arity: usize,
        ctxs: &[Ctx],
        num: &mut [u64],
        den: &mut [u64],
    ) -> Result<CoeffStats, CoeffError> {
        Independent.eval_ratio(text, arity, ctxs, num, den)
    }
    fn eliminate<'r>(
        &self,
        rows: &'r [Vec<(usize, u64)>],
        ncols: usize,
        p: u64,
        max_entries: usize,
    ) -> Result<LieElim<'r>, Unknown> {
        Ok(LieElim {
            inner: Independent.eliminate(rows, ncols, p, max_entries)?,
            nth: self.0,
            calls: Cell::new(0),
        })
    }
}

/// Kills M03, M04, M32: a failed control (the +1 mutant, or a corner, in the span) makes the cell Undecided and the rule
/// never `Pass` (the controls are the safety net against row-generation defects; no test exercised a failing control).
#[test]
fn audit_rows3_failed_controls_are_never_a_pass() {
    let f = face();
    let fam = &f.fam;
    let good = RowsRule::from_value(f.good.clone(), 9).unwrap();
    // an integer key off the face: its rows reach numerator-free (corner) columns, so the corner control runs
    // (numerators at -1, so the rows raise them to 0; the A-m7 note: the corner control is vacuous elsewhere)
    let mut found = None;
    'search: for (a, b, c7, c8) in (1..=4i64).flat_map(|a| (1..=4i64).flat_map(move |b| [(0i64, 0i64), (0, -1), (-1, 0), (-1, -1)].map(|(x, y)| (a, b, x, y)))) {
        {
            // rule point: lines 1..=4, numerators 0 / -1 (the seed sits at the rule point; rows raise a numerator to 0)
            let pt: Vec<i64> = (0..9).map(|i| match i { 7 => c7, 8 => c8, _ => 1 + (a * i as i64 + b) % 4 }).collect();
            let key: Vec<i64> = pt.iter().zip(&f.tshift).map(|(x, t)| x + t).collect();
            if (0..9).any(|i| (i < 7 && key[i] < 1) || (i >= 7 && key[i] > 0)) {
                continue;
            }
            let clean = identity_test(fam, &good, &[PointSpec::Key(key.clone())], &RowsOptions::default());
            let corners = clean.points[0].cells.first().map_or(0, |c| c.corners_tested);
            if clean.verdict == Verdict::Pass && corners >= 1 {
                found = Some((key, corners));
                break 'search;
            }
        }
    }
    let (key, corners) = found.unwrap_or_else(|| panic!("no key whose rows reach a corner column (tshift {:?})", f.tshift));
    for (what, nth) in [("+1 mutant in span", 1usize), ("first corner in span", 2)] {
        let v = identity_test_with(&ControlLie(nth), fam, &good, &[PointSpec::Key(key.clone())], &RowsOptions::default());
        let st: Vec<&CellStatus> = v.points.iter().flat_map(|p| &p.cells).map(|c| &c.status).collect();
        println!("{what} ({corners} corners probed): {:?} {:?}", v.verdict, st);
        assert!(matches!(v.verdict, Verdict::Undecided { .. }), "{what}: {:?}", v.verdict);
    }
}
