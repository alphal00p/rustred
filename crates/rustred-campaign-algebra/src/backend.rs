//! [`SymbolicaAlgebra`]: the identity test's [`CheckAlgebra`] on Symbolica / numerica (DEV-17 REVISED). Every routine
//! is a thin adapter onto an existing Symbolica / numerica entry point (listed per method); the only glue is the
//! conversion between the trait's data (u64 residues, i128 rationals, sparse rows) and Symbolica's types, plus the
//! weight certificate of a span test assembled from numerica's own L and U factors.
//!
//! Entry points used (vendored tree `vendor/symbolica`, `num/` = `lib/numerica/src`):
//! - field: `Zp64::new` / `to_element` / `from_element`, `Ring::{add, sub, mul, neg}`, `Field::inv`
//!   (`num/domains/finite_field.rs:1023-1130`); one `Zp64` per prime, cached per thread;
//! - primality: `Integer::is_prime` (`num/domains/integer.rs:1756`, deterministic for u64);
//! - rationals: `Rational::new`, the field `Q` (`num/domains/rational.rs:32, 982`);
//! - exact linear algebra: `Matrix::{from_nested_vec, det, inv, rank, row_reduce}` (`num/tensors/matrix.rs`), over `Z`
//!   for integer determinants, over `Q` otherwise, over `PolynomialRing<Z>` for the U / F determinants; integer gcd:
//!   `Integer::gcd` (`num/domains/integer.rs:1905`);
//! - coefficient texts, expressions, case literals: the general atom parser `try_parse!` with default namespace
//!   `rustred` and `AtomCore::try_to_rational_polynomial(&Q, &Z, vars)` / `try_to_polynomial` (`src/atom/core.rs:1428,
//!   1584`), exactly the engine's own path (crates/rustred-core/src/propagator/basis.rs `parse_coefficient`); then
//!   `MultivariatePolynomial::{map_coeff, replace_all}` (`src/poly/polynomial.rs:2133, 2966`) per prime. NOT
//!   `Token::parse_polynomial` (four defects, see the crate doc: never used for acceptance);
//! - sparse elimination: `SparseRowReducer::{new, add_row, pivots, u, l}` with `LuLMode::Full`
//!   (`num/tensors/sparse.rs:1592-1850`).

use std::cell::RefCell;
use std::sync::Arc;
use std::time::Instant;

use rustred_campaign_core::replay_base::algebra::{
    CheckAlgebra, CoeffError, CoeffStats, Ctx, Eliminator, MEMORY_GUARD, Q as RQ, QResult,
    SideStats, Span, Unknown, ZPoly,
};
use symbolica::atom::{NamespacedSymbol, SymbolBuilder};
use symbolica::domains::finite_field::{FiniteFieldCore, FiniteFieldElement, Zp64};
use symbolica::domains::integer::{Integer, Z};
use symbolica::domains::rational::{Q, Rational};
use symbolica::domains::{Field, Ring, RingOps};
use symbolica::poly::PolyVariable;
use symbolica::poly::polynomial::MultivariatePolynomial;
use symbolica::prelude::{AtomCore, Symbol};
use symbolica::tensors::matrix::Matrix;
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};
use symbolica::try_parse;

/// Budgets of the Symbolica mode (a coefficient above them is `Unsupported` in this mode, never guessed).
#[derive(Clone, Debug)]
pub struct SymbolicaBudget {
    /// largest coefficient / expression text handed to the atom parser (bytes)
    pub max_text_bytes: usize,
    /// a single parse + conversion slower than this is reported as unsupported (post hoc: Symbolica calls cannot be
    /// interrupted; the size budget is the guard that bounds them)
    pub max_seconds: f64,
}

impl Default for SymbolicaBudget {
    fn default() -> Self {
        SymbolicaBudget {
            max_text_bytes: 8_000_000,
            max_seconds: 60.0,
        }
    }
}

/// The Symbolica-backed algebra. Construct with [`SymbolicaAlgebra::new`] (refuses without the licence).
#[derive(Clone, Debug)]
pub struct SymbolicaAlgebra {
    pub budget: SymbolicaBudget,
}

/// Is the Symbolica licence available to this process? (the environment variable first, so that an unlicensed
/// process never reaches Symbolica's per-user lock; then Symbolica's own key check, which takes no lock)
pub fn licensed() -> bool {
    std::env::var_os("SYMBOLICA_LICENSE").is_some_and(|v| !v.is_empty())
        && symbolica::license::LicenseManager::is_licensed()
}

thread_local! {
    static FIELD: RefCell<Option<(u64, Zp64)>> = const { RefCell::new(None) };
}

fn with_field<T>(p: u64, f: impl FnOnce(&Zp64) -> T) -> T {
    FIELD.with(|c| {
        let mut c = c.borrow_mut();
        if c.as_ref().is_none_or(|(q, _)| *q != p) {
            *c = Some((p, Zp64::new(p)));
        }
        f(&c.as_ref().expect("field").1)
    })
}

fn to_rational(a: RQ) -> Rational {
    Rational::new(Integer::from(a.n), Integer::from(a.d))
}

fn from_rational(r: &Rational) -> QResult {
    let ovf = || "exact rational overflow (i128)".to_string();
    let n = i128::try_from(r.numerator()).map_err(|_| ovf())?;
    let d = i128::try_from(r.denominator()).map_err(|_| ovf())?;
    match d {
        0 => Err("division by zero".into()),
        d if d < 0 => Ok(RQ {
            n: n.checked_neg().ok_or_else(ovf)?,
            d: d.checked_neg().ok_or_else(ovf)?,
        }),
        d => Ok(RQ { n, d }),
    }
}

fn int_mod(f: &Zp64, i: &Integer) -> FiniteFieldElement<u64> {
    let p = Integer::from(f.get_prime());
    let mut r = i % &p;
    if r < 0 {
        r += &p;
    }
    f.to_element(u64::try_from(r).expect("residue fits u64"))
}

/// The `rustred::` symbols of the variable map `n0 .. n{arity-1}` (+ `d` when `with_d`).
fn variables(arity: usize, with_d: bool) -> Result<(Arc<Vec<PolyVariable>>, Vec<Symbol>), Unknown> {
    let mut vars = Vec::with_capacity(arity + 1);
    let mut syms = Vec::with_capacity(arity + 1);
    let names: Vec<String> = (0..arity)
        .map(|i| format!("n{i}"))
        .chain(with_d.then(|| "d".to_string()))
        .collect();
    for name in names {
        let q = format!("rustred::{name}");
        let ns = NamespacedSymbol::try_parse(&q).ok_or_else(|| format!("symbol {q}"))?;
        let s = SymbolBuilder::new(ns).build().map_err(|e| e.to_string())?;
        vars.push(PolyVariable::Symbol(s));
        syms.push(s);
    }
    Ok((Arc::new(vars), syms))
}

/// Miller-Rabin rounds requested from numerica (ignored below 2^64: deterministic there).
const MR_ROUNDS: usize = 32;

type ZPolyS = MultivariatePolynomial<symbolica::domains::integer::IntegerRing, u16>;

impl SymbolicaAlgebra {
    /// The backend, or the reason it may not run (no licence: two unlicensed Symbolica processes of this Unix user
    /// abort each other host-wide).
    pub fn new() -> Result<SymbolicaAlgebra, String> {
        if !licensed() {
            return Err(
                "Symbolica licence absent (source WS/ops/env.sh): the Symbolica backend refuses to run".into(),
            );
        }
        Ok(SymbolicaAlgebra {
            budget: SymbolicaBudget::default(),
        })
    }

    fn budget_check(&self, text: &str) -> Result<(), String> {
        if text.len() > self.budget.max_text_bytes {
            return Err(format!(
                "Unsupported in the Symbolica mode: text of {} bytes above the budget of {} bytes",
                text.len(),
                self.budget.max_text_bytes
            ));
        }
        Ok(())
    }

    fn timed<T>(&self, what: &str, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        let t0 = Instant::now();
        let r = f()?;
        let s = t0.elapsed().as_secs_f64();
        if s > self.budget.max_seconds {
            return Err(format!(
                "Unsupported in the Symbolica mode: {what} took {s:.1} s > {} s",
                self.budget.max_seconds
            ));
        }
        Ok(r)
    }

    /// Parse a coefficient text with the engine's path: (numerator, denominator) over Z in n0..n{N-1}, d.
    pub fn parse_ratio(&self, text: &str, arity: usize) -> Result<(ZPolyS, ZPolyS), String> {
        self.budget_check(text)?;
        self.timed("coefficient parse", || {
            let (vars, _) = variables(arity, true)?;
            let atom =
                try_parse!(text, default_namespace = "rustred").map_err(|e| e.to_string())?;
            let rp = atom
                .as_view()
                .try_to_rational_polynomial::<_, _, u16>(&Q, &Z, Some(vars.clone()))
                .map_err(|e| format!("not a rational function: {e}"))?;
            if rp.numerator.get_vars_ref().len() != vars.len() {
                return Err("symbols outside n0..n{N-1}, d".into());
            }
            Ok((rp.numerator, rp.denominator))
        })
    }

    /// Parse an expression as a polynomial over `ring` in n0..n{N-1} (+ d); extra symbols are refused.
    fn parse_poly<R>(
        &self,
        text: &str,
        n: usize,
        with_d: bool,
        ring: &R,
    ) -> Result<MultivariatePolynomial<R, u16>, String>
    where
        R: symbolica::domains::EuclideanDomain + symbolica::prelude::ConvertToRing,
    {
        self.budget_check(text)?;
        self.timed("expression parse", || {
            let (vars, _) = variables(n, with_d)?;
            let atom =
                try_parse!(text, default_namespace = "rustred").map_err(|e| e.to_string())?;
            let p = atom
                .as_view()
                .try_to_polynomial::<R, u16>(ring, Some(vars.clone()))
                .map_err(|e| format!("not a polynomial: {e}"))?;
            if p.get_vars_ref().len() != vars.len() {
                return Err("symbols outside the variable map".into());
            }
            Ok(p)
        })
    }
}

fn side_stats(p: &ZPolyS, arity: usize) -> SideStats {
    let mut st = SideStats {
        terms: p.nterms(),
        ..SideStats::default()
    };
    for (c, e) in p.coefficients.iter().zip(p.exponents_iter()) {
        let degn: u32 = e[..arity].iter().map(|&x| u32::from(x)).sum();
        let degd = u32::from(e[arity]);
        st.deg = st.deg.max(degn + degd);
        st.degd = st.degd.max(degd);
        st.degn = st.degn.max(degn);
        let unit = c == &Integer::from(1) || c == &Integer::from(-1);
        let factors = e.iter().filter(|&&x| x != 0).count() as u32 + u32::from(!unit);
        st.factors = st.factors.max(factors.max(1));
    }
    st
}

/// Eliminated rows on numerica's `SparseRowReducer` (L recorded for the weight certificate).
pub struct SymElim<'r> {
    rows: &'r [Vec<(usize, u64)>],
    p: u64,
    red: SparseRowReducer<Zp64>,
    /// source row of every U row
    u_source: Vec<usize>,
    /// the L row of the input that created every U row (numerica records no L row for an empty row)
    u_lrow: Vec<usize>,
    entries: usize,
}

impl SymElim<'_> {
    /// Express `c` (coefficients on the U rows) as weights on the original rows: U_j = (row_{k_j} - sum_{j'} L[k_j][j']
    /// U_{j'}) / L[k_j][j], processed from the last U row down.
    fn weights(
        &self,
        f: &Zp64,
        mut c: Vec<FiniteFieldElement<u64>>,
    ) -> Vec<FiniteFieldElement<u64>> {
        let l = self.red.l();
        let (lv, li, lp) = (l.values(), l.col_idcs(), l.row_ptrs());
        let mut w = vec![f.zero(); self.rows.len()];
        for j in (0..self.u_source.len()).rev() {
            if f.is_zero(&c[j]) {
                continue;
            }
            let k = self.u_source[j];
            let lk = self.u_lrow[j];
            let (s, e) = (lp[lk], lp[lk + 1]);
            let lead = (s..e)
                .find(|&t| li[t] as usize == j)
                .map(|t| lv[t])
                .expect("U row has its diagonal L entry");
            let cj = f.div(&c[j], &lead);
            w[k] = f.add(&w[k], &cj);
            for t in s..e {
                let jj = li[t] as usize;
                if jj != j {
                    c[jj] = f.sub(&c[jj], &f.mul(&cj, &lv[t]));
                }
            }
        }
        w
    }
}

impl Eliminator for SymElim<'_> {
    fn rank(&self) -> usize {
        self.u_source.len()
    }
    fn entries(&self) -> usize {
        self.entries
    }
    fn pivot_columns(&self) -> Vec<usize> {
        let mut by_row: Vec<(u32, usize)> = self
            .red
            .pivots()
            .iter()
            .enumerate()
            .filter_map(|(c, r)| r.map(|r| (r, c)))
            .collect();
        by_row.sort_unstable();
        by_row.into_iter().map(|(_, c)| c).collect()
    }
    fn solve(&self, desired: &[(usize, u64)]) -> Result<Span, Unknown> {
        with_field(self.p, |f| {
            let mut probe = self.red.clone();
            let vals: Vec<FiniteFieldElement<u64>> =
                desired.iter().map(|&(_, v)| f.to_element(v)).collect();
            let cols: Vec<u32> = desired.iter().map(|&(c, _)| c as u32).collect();
            let rank = self.u_source.len();
            let l_before = probe.l().nrows();
            let new_pivot = if desired.is_empty() {
                None
            } else {
                probe.add_row(&vals, &cols)
            };
            if !desired.is_empty() && probe.l().nrows() == l_before {
                // numerica's full-rank shortcut returns before recording L: no coefficients, no certificate
                return Err("Symbolica backend: full-rank shortcut, no span certificate".into());
            }
            // the L row of `desired`: its coefficients on the U rows (and on the new U row when independent)
            let l = probe.l();
            let (lv, li, lp) = (l.values(), l.col_idcs(), l.row_ptrs());
            let mut c = vec![f.zero(); rank];
            let mut remainder: Vec<(usize, u64)> = Vec::new();
            if !desired.is_empty() {
                let k = l.nrows() as usize - 1;
                for t in lp[k]..lp[k + 1] {
                    let j = li[t] as usize;
                    if j < rank {
                        c[j] = f.add(&c[j], &lv[t]);
                    } else {
                        // desired = sum c_j U_j + lead U_new: the remainder is lead * U_new
                        let u = probe.u();
                        let (uv, ui, up) = (u.values(), u.col_idcs(), u.row_ptrs());
                        for s in up[j]..up[j + 1] {
                            let v = f.mul(&lv[t], &uv[s]);
                            remainder.push((ui[s] as usize, f.from_element(&v)));
                        }
                    }
                }
            }
            let w = self.weights(f, c);
            // certificate: desired - sum w_k rows_k == remainder, recomputed on the ORIGINAL rows
            let mut acc: std::collections::BTreeMap<usize, FiniteFieldElement<u64>> = desired
                .iter()
                .map(|&(col, v)| (col, f.to_element(v)))
                .collect();
            for (k, wk) in w.iter().enumerate() {
                if f.is_zero(wk) {
                    continue;
                }
                for &(col, v) in &self.rows[k] {
                    let e = acc.entry(col).or_insert_with(|| f.zero());
                    *e = f.sub(&*e, &f.mul(wk, &f.to_element(v)));
                }
            }
            let left: Vec<(usize, u64)> = acc
                .into_iter()
                .filter(|(_, v)| !f.is_zero(v))
                .map(|(c, v)| (c, f.from_element(&v)))
                .collect();
            if left != remainder || (new_pivot.is_some() == remainder.is_empty()) {
                return Err(
                    "Symbolica backend: span certificate does not reproduce on the original rows"
                        .into(),
                );
            }
            Ok(Span {
                in_span: remainder.is_empty(),
                rank,
                weights: w
                    .iter()
                    .enumerate()
                    .filter(|(_, x)| !f.is_zero(x))
                    .map(|(k, x)| (k, f.from_element(x)))
                    .collect(),
                remainder,
                pivot_rows: self.u_source.clone(),
            })
        })
    }
}

impl CheckAlgebra for SymbolicaAlgebra {
    type Literal = MultivariatePolynomial<symbolica::domains::integer::IntegerRing, u16>;
    type Elim<'r> = SymElim<'r>;

    fn name(&self) -> &'static str {
        "symbolica"
    }
    fn is_prime(&self, n: u64) -> bool {
        // SYMBOLICA PR CANDIDATE (rows3): Integer::is_prime panics (`unreachable!`, num/domains/finite_field.rs:3270)
        // for n = u64::MAX (the Sinclair-basis branch tests `n < u64::MAX`); u64::MAX = 3 * 5 * 17 * ... is composite
        if n == u64::MAX {
            return false;
        }
        // the Miller-Rabin round count is ignored for 64-bit inputs (deterministic witness set)
        Integer::from(n).is_prime(MR_ROUNDS)
    }
    fn add(&self, p: u64, a: u64, b: u64) -> u64 {
        with_field(p, |f| {
            f.from_element(&f.add(&f.to_element(a), &f.to_element(b)))
        })
    }
    fn sub(&self, p: u64, a: u64, b: u64) -> u64 {
        with_field(p, |f| {
            f.from_element(&f.sub(&f.to_element(a), &f.to_element(b)))
        })
    }
    fn mul(&self, p: u64, a: u64, b: u64) -> u64 {
        with_field(p, |f| {
            f.from_element(&f.mul(&f.to_element(a), &f.to_element(b)))
        })
    }
    fn neg(&self, p: u64, a: u64) -> u64 {
        with_field(p, |f| f.from_element(&f.neg(&f.to_element(a))))
    }
    fn inv(&self, p: u64, a: u64) -> Option<u64> {
        with_field(p, |f| {
            let e = f.to_element(a % p);
            (!f.is_zero(&e)).then(|| f.from_element(&f.inv(&e)))
        })
    }
    fn from_i128(&self, p: u64, x: i128) -> u64 {
        with_field(p, |f| f.from_element(&int_mod(f, &Integer::from(x))))
    }
    fn q_new(&self, n: i128, d: i128) -> QResult {
        if d == 0 {
            return Err("division by zero".into());
        }
        from_rational(&Rational::new(Integer::from(n), Integer::from(d)))
    }
    fn q_add(&self, a: RQ, b: RQ) -> QResult {
        from_rational(&Q.add(&to_rational(a), &to_rational(b)))
    }
    fn q_sub(&self, a: RQ, b: RQ) -> QResult {
        from_rational(&Q.sub(&to_rational(a), &to_rational(b)))
    }
    fn q_mul(&self, a: RQ, b: RQ) -> QResult {
        from_rational(&Q.mul(&to_rational(a), &to_rational(b)))
    }
    fn q_div(&self, a: RQ, b: RQ) -> QResult {
        if b.n == 0 {
            return Err("division by zero".into());
        }
        from_rational(&Q.div(&to_rational(a), &to_rational(b)))
    }
    fn q_neg(&self, a: RQ) -> QResult {
        from_rational(&Q.neg(&to_rational(a)))
    }
    fn q_parse(&self, s: &str) -> QResult {
        let bad = || format!("rational {s}");
        let r = match s.split_once('/') {
            Some((a, b)) => {
                let a: Integer = a.trim().parse().map_err(|_| bad())?;
                let b: Integer = b.trim().parse().map_err(|_| bad())?;
                if b == 0 {
                    return Err("division by zero".into());
                }
                Rational::new(a, b)
            }
            None => Rational::new(
                s.trim().parse::<Integer>().map_err(|_| bad())?,
                Integer::from(1),
            ),
        };
        from_rational(&r)
    }
    fn q_modp(&self, a: RQ, p: u64) -> Option<u64> {
        with_field(p, |f| {
            let d = int_mod(f, &Integer::from(a.d));
            (!f.is_zero(&d)).then(|| f.from_element(&f.div(&int_mod(f, &Integer::from(a.n)), &d)))
        })
    }
    fn q_inverse(&self, m: &[Vec<RQ>]) -> Result<Vec<Vec<RQ>>, Unknown> {
        let n = m.len();
        let mat = Matrix::from_nested_vec(
            m.iter()
                .map(|r| r.iter().map(|&x| to_rational(x)).collect())
                .collect(),
            Q,
        )?;
        let inv = mat.inv().map_err(|e| format!("singular matrix ({e:?})"))?;
        (0..n)
            .map(|i| {
                (0..n)
                    .map(|j| from_rational(&inv[(i as u32, j as u32)]))
                    .collect()
            })
            .collect()
    }
    fn q_rank(&self, rows: &[Vec<RQ>]) -> Result<usize, Unknown> {
        if rows.is_empty() || rows[0].is_empty() {
            return Ok(0);
        }
        let mat = Matrix::from_nested_vec(
            rows.iter()
                .map(|r| r.iter().map(|&x| to_rational(x)).collect())
                .collect(),
            Q,
        )?;
        Ok(mat.rank())
    }
    fn det_i128(&self, m: &[Vec<i128>]) -> Option<i128> {
        if m.is_empty() {
            return Some(1);
        }
        let mat = Matrix::from_nested_vec(
            m.iter()
                .map(|r| r.iter().map(|&x| Integer::from(x)).collect())
                .collect(),
            Z,
        )
        .ok()?;
        i128::try_from(mat.det().ok()?).ok()
    }
    fn q_rref(
        &self,
        rows: &[Vec<RQ>],
        ncols: usize,
    ) -> Result<(Vec<Vec<RQ>>, Vec<usize>), Unknown> {
        if rows.is_empty() {
            return Ok((Vec::new(), Vec::new()));
        }
        let mut mat = Matrix::from_nested_vec(
            rows.iter()
                .map(|r| r.iter().map(|&x| to_rational(x)).collect())
                .collect(),
            Q,
        )?;
        let rank = mat.row_reduce(ncols as u32);
        let w = rows[0].len();
        let mut out: Vec<Vec<RQ>> = Vec::with_capacity(rows.len());
        let mut pivots = Vec::with_capacity(rank);
        for i in 0..rows.len() {
            let mut r: Vec<Rational> = (0..w).map(|j| mat[(i as u32, j as u32)].clone()).collect();
            if i < rank {
                let pc = (0..ncols)
                    .find(|&j| !Q.is_zero(&r[j]))
                    .ok_or("rank row without pivot")?;
                if !Q.is_one(&r[pc]) {
                    let inv = Q.inv(&r[pc]);
                    r = r.iter().map(|x| Q.mul(x, &inv)).collect();
                }
                pivots.push(pc);
            }
            out.push(r.iter().map(from_rational).collect::<Result<_, _>>()?);
        }
        Ok((out, pivots))
    }
    fn q_unit_in_row_space(&self, rows: &[Vec<RQ>], col: usize) -> Result<bool, Unknown> {
        if rows.is_empty() {
            return Ok(false);
        }
        let w = rows[0].len();
        let base = self.q_rank(rows)?;
        let mut with: Vec<Vec<RQ>> = rows.to_vec();
        with.push(
            (0..w)
                .map(|j| if j == col { RQ::ONE } else { RQ::ZERO })
                .collect(),
        );
        Ok(self.q_rank(&with)? == base)
    }
    fn gcd_i128(&self, a: i128, b: i128) -> i128 {
        i128::try_from(Integer::from(a).gcd(&Integer::from(b))).unwrap_or(0)
    }
    fn zpoly_det(&self, m: &[Vec<ZPoly>], dim: usize, t: usize) -> ZPoly {
        // Laplace expansion along the first row with Symbolica polynomial arithmetic (MultivariatePolynomial<Z, u8>
        // add / mul). Matrix<PolynomialRing>::det is NOT used: its ring's zero() has no variables and
        // `exponents_iter` panics on it (`chunks(0)`, src/poly/polynomial.rs:1222): SYMBOLICA PR CANDIDATE (rows3).
        type P = MultivariatePolynomial<symbolica::domains::integer::IntegerRing, u8>;
        let Ok((vars, _)) = variables(t, false) else {
            return ZPoly::new();
        };
        let to_poly = |z: &ZPoly| -> P {
            let mut p: P = MultivariatePolynomial::new(&Z, Some(z.len()), vars.clone());
            for (e, c) in z {
                p.append_monomial(Integer::from(*c), e);
            }
            p
        };
        let mm: Vec<Vec<P>> = (0..dim)
            .map(|i| (0..dim).map(|j| to_poly(&m[i][j])).collect())
            .collect();
        fn det(
            mm: &[Vec<P>],
            rows: &[usize],
            cols: &[usize],
            one: &P,
            memo: &mut std::collections::HashMap<(Vec<usize>, Vec<usize>), P>,
        ) -> P {
            if rows.is_empty() {
                return one.clone();
            }
            let key = (rows.to_vec(), cols.to_vec());
            if let Some(r) = memo.get(&key) {
                return r.clone();
            }
            let mut out = one.zero();
            for (j, &c) in cols.iter().enumerate() {
                if mm[rows[0]][c].is_zero() {
                    continue;
                }
                let sub_cols: Vec<usize> = cols.iter().copied().filter(|&x| x != c).collect();
                let sub = det(mm, &rows[1..], &sub_cols, one, memo);
                let term = &mm[rows[0]][c] * &sub;
                out = if j % 2 == 0 {
                    &out + &term
                } else {
                    &out - &term
                };
            }
            memo.insert(key, out.clone());
            out
        }
        let one = to_poly(&std::iter::once((vec![0u8; t], 1i128)).collect());
        let idx: Vec<usize> = (0..dim).collect();
        let d = det(&mm, &idx, &idx, &one, &mut std::collections::HashMap::new());
        let mut out = ZPoly::new();
        for (c, e) in d.coefficients.iter().zip(d.exponents_iter()) {
            if let Ok(v) = i128::try_from(c.clone()) {
                out.insert(e.to_vec(), v);
            }
        }
        out
    }
    fn eval_ratio(
        &self,
        text: &str,
        arity: usize,
        ctxs: &[Ctx],
        num: &mut [u64],
        den: &mut [u64],
    ) -> Result<CoeffStats, CoeffError> {
        let (pn, pd) = self
            .parse_ratio(text, arity)
            .map_err(|msg| CoeffError { pos: 0, msg })?;
        for (k, c) in ctxs.iter().enumerate() {
            with_field(c.p, |f| {
                let vals: Vec<FiniteFieldElement<u64>> =
                    c.vals.iter().map(|&v| f.to_element(v % c.p)).collect();
                let en = pn
                    .map_coeff(|x| int_mod(f, x), f.clone())
                    .replace_all(&vals);
                let ed = pd
                    .map_coeff(|x| int_mod(f, x), f.clone())
                    .replace_all(&vals);
                num[k] = f.from_element(&en);
                den[k] = f.from_element(&ed);
            });
        }
        Ok(CoeffStats {
            num: side_stats(&pn, arity),
            den: side_stats(&pd, arity),
            has_den: !pd.is_one(),
        })
    }
    fn cross_check_coefficient(
        &self,
        _text: &str,
        _arity: usize,
        _ctx: &Ctx,
        _num: u64,
        _den: u64,
    ) -> Result<(), Unknown> {
        // one reader only (Symbolica's atom parser); the triage compares it with the independent readers
        Ok(())
    }
    fn expr_linear(&self, text: &str, n: usize) -> Result<Vec<RQ>, Unknown> {
        let p = self.parse_poly(text, n, true, &Q)?;
        let mut out = vec![RQ::ZERO; n + 2];
        for (c, e) in p.coefficients.iter().zip(p.exponents_iter()) {
            let deg: u32 = e.iter().map(|&x| u32::from(x)).sum();
            match deg {
                0 => out[0] = from_rational(c)?,
                1 => {
                    let v = e.iter().position(|&x| x == 1).expect("degree one");
                    out[v + 1] = from_rational(c)?;
                }
                _ => return Err(format!("{text}: not affine")),
            }
        }
        Ok(out)
    }
    fn expr_has_d(&self, text: &str, n: usize) -> Result<bool, Unknown> {
        self.budget_check(text)?;
        let (_, syms) = variables(n, true)?;
        let atom = try_parse!(text, default_namespace = "rustred").map_err(|e| e.to_string())?;
        Ok(atom.contains_symbol(syms[n]))
    }
    fn literal_parse(&self, text: &str, n: usize) -> Result<Self::Literal, Unknown> {
        self.parse_poly(text, n, false, &Z)
    }
    fn literal_is_zero_at(&self, lit: &Self::Literal, point: &[i32]) -> Result<bool, Unknown> {
        if point.len() != lit.get_vars_ref().len() {
            return Err("point length".into());
        }
        let vals: Vec<Integer> = point.iter().map(|&x| Integer::from(x as i64)).collect();
        Ok(lit.replace_all(&vals) == 0)
    }
    fn literal_same_up_to_constant(&self, a: &str, b: &str, n: usize) -> bool {
        match (
            self.parse_poly(a, n, false, &Z),
            self.parse_poly(b, n, false, &Z),
        ) {
            (Ok(x), Ok(y)) if !x.is_zero() && !y.is_zero() => {
                x.clone().mul_coeff(y.lcoeff()) == y.clone().mul_coeff(x.lcoeff())
            }
            (Ok(x), Ok(y)) => x.is_zero() && y.is_zero(),
            _ => false,
        }
    }
    fn eliminate<'r>(
        &self,
        rows: &'r [Vec<(usize, u64)>],
        ncols: usize,
        p: u64,
        max_entries: usize,
    ) -> Result<SymElim<'r>, Unknown> {
        with_field(p, |f| {
            let mut red = SparseRowReducer::new(ncols as u32, f.clone(), LuLMode::Full);
            let mut u_source = Vec::new();
            let mut u_lrow = Vec::new();
            for (k, r) in rows.iter().enumerate() {
                if r.is_empty() {
                    continue;
                }
                let vals: Vec<FiniteFieldElement<u64>> =
                    r.iter().map(|&(_, v)| f.to_element(v)).collect();
                let cols: Vec<u32> = r.iter().map(|&(c, _)| c as u32).collect();
                let l_before = red.l().nrows() as usize;
                if red.add_row(&vals, &cols).is_some() {
                    u_source.push(k);
                    u_lrow.push(l_before);
                }
                let entries = red.u().nvalues() + red.l().nvalues();
                if entries > max_entries {
                    return Err(format!("{MEMORY_GUARD}: {entries} entries > {max_entries}"));
                }
            }
            let entries = red.u().nvalues() + red.l().nvalues();
            Ok(SymElim {
                rows,
                p,
                red,
                u_source,
                u_lrow,
                entries,
            })
        })
    }
}
