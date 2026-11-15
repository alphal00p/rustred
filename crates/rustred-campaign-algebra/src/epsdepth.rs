//! # Exact Laurent arithmetic at d0 for the composed pole profiles (lane epsdepth; interfaces/epsdepth.md 2, 4)
//!
//! Implements the core traits `rustred_campaign_core::epsdepth::{Laurent, Valuation}` on Symbolica:
//! - a coefficient at a key is read EXACTLY (the engine's coefficient path `SymbolicaAlgebra::parse_ratio` =
//!   `try_to_rational_polynomial` over Z; the key's index values put in with `MultivariatePolynomial::replace_multiple`,
//!   exact integers) and kept as [`TCoeff`] = `t^v n1(t) / d1(t)` with `t = d - d0`, `n1(0), d1(0) != 0`
//!   (`UnivariatePolynomial::shift_var(d0)` over Q, src/poly/univariate.rs:1046);
//! - its expansion to `r` terms: `d1^-1 mod t^r` from Symbolica's extended Euclid (`UnivariatePolynomial::extended_gcd`
//!   with `t^r`, univariate.rs:2255; gcd 1 because `d1(0) != 0`), times `n1`, truncated;
//! - series arithmetic and the absolute-precision bookkeeping: Symbolica `Series<Q>` (src/poly/series.rs: `Add`
//!   keeps the minimum absolute order, `Mul` the minimum relative order on normalised series, `truncate` strips
//!   cancelled leading terms and keeps the absolute order): exactly the rules of the lemma in interfaces/epsdepth.md 4.
//! - the valuation of a coefficient TEXT at a key (core trait `Valuation`): lane poles' `valuation::order_at_key`.
//!
//! SYMBOLICA VERIFICATION (L11, vendored tree 9cd57678, recorded in WS/notes/implementer.epsdepth.md): `Series<F>` has
//! `Add`, `Mul`, `Neg`, `npow`, but `Div` only for `Series<AtomField>` (series.rs:1254, via `rpow(-1)` on symbolic
//! coefficients): there is no division / inverse of a `Series<RationalField>` and no constructor from a rational
//! function; the inverse is therefore taken with the univariate extended Euclid (Symbolica), never with own code.
//! No own algebra in this file.

pub mod record_source;

use std::collections::HashMap;
use std::sync::Arc;

use rustred_campaign_core::epsdepth::{KeyStatus, Laurent, ReductionSource, Terms, Val, Valuation};
use symbolica::atom::{NamespacedSymbol, SymbolBuilder};
use symbolica::domains::Ring;
use symbolica::domains::integer::{Integer, IntegerRing, Z};
use symbolica::domains::rational::{Q, Rational, RationalField};
use symbolica::domains::rational_polynomial::{FromNumeratorAndDenominator, RationalPolynomial};
use symbolica::poly::PolyVariable;
use symbolica::poly::polynomial::MultivariatePolynomial;
use symbolica::poly::series::Series;
use symbolica::poly::univariate::UnivariatePolynomial;

use crate::SymbolicaAlgebra;
use crate::valuation::{D0, Valuation as PolesValuation, order_at_key};

type ZPoly = MultivariatePolynomial<IntegerRing, u16>;
type QUni = UnivariatePolynomial<RationalField>;
/// A truncated Laurent series in `t = d - d0` over Q with its absolute precision (Symbolica).
pub type QSeries = Series<RationalField>;

/// An exact coefficient at d0: `t^v n1(t) / d1(t)` with `n1(0) != 0`, `d1(0) != 0`, or exactly zero.
#[derive(Clone, Debug)]
pub enum TCoeff {
    Zero,
    Ratio { v: i64, n1: QUni, d1: QUni },
}

impl TCoeff {
    pub fn valuation(&self) -> Val {
        match self {
            TCoeff::Zero => Val::Zero,
            TCoeff::Ratio { v, .. } => Val::Order(*v),
        }
    }
}

/// The Symbolica backend of the composition at one d0.
#[derive(Clone, Debug)]
pub struct SymbolicaLaurent {
    d0: D0,
    tvar: Arc<PolyVariable>,
}

fn trailing_zeros(p: &QUni) -> usize {
    p.coefficients()
        .iter()
        .position(|c| !Q.is_zero(c))
        .unwrap_or(0)
}

fn strip(p: &QUni, k: usize) -> QUni {
    QUni::from_coefficients(&Q, p.coefficients()[k..].to_vec(), p.get_vars())
}

impl SymbolicaLaurent {
    /// `d0` = `a` or `a/b` (lane poles' `D0::parse`).
    pub fn new(d0: &str) -> Result<SymbolicaLaurent, String> {
        let d0 = D0::parse(d0).map_err(|e| e.to_string())?;
        let ns = NamespacedSymbol::try_parse("rustred::epsdepth_t")
            .ok_or("symbol rustred::epsdepth_t")?;
        let t = SymbolBuilder::new(ns).build().map_err(|e| e.to_string())?;
        Ok(SymbolicaLaurent {
            d0,
            tvar: Arc::new(PolyVariable::Symbol(t)),
        })
    }

    pub fn d0(&self) -> &D0 {
        &self.d0
    }

    /// From numerator / denominator polynomials over Z whose only variable with a non-zero degree is `dv` (= d).
    pub fn coeff_from_zpolys(&self, num: &ZPoly, den: &ZPoly, dv: usize) -> Result<TCoeff, String> {
        if den.is_zero() {
            return Err("denominator vanishes identically at the key".into());
        }
        if num.is_zero() {
            return Ok(TCoeff::Zero);
        }
        for p in [num, den] {
            if (0..p.nvars()).any(|i| i != dv && p.degree(i) > 0) {
                return Err("an index variable is left after the key was put in".into());
            }
        }
        let to_q = |p: &ZPoly| -> QUni {
            p.to_univariate_from_univariate(dv)
                .map_coeff(|c: &Integer| Rational::from(c.clone()), Q)
                .shift_var(self.d0.value())
        };
        let (n, d) = (to_q(num), to_q(den));
        let (a, b) = (trailing_zeros(&n), trailing_zeros(&d));
        Ok(TCoeff::Ratio {
            v: a as i64 - b as i64,
            n1: strip(&n, a),
            d1: strip(&d, b),
        })
    }

    /// Read a coefficient text (any rational function of `d` and `n0 .. n{arity-1}`, read whole by the engine's parse
    /// path) at `key` (index values put in exactly).
    pub fn coeff_from_text(
        &self,
        alg: &SymbolicaAlgebra,
        text: &str,
        arity: usize,
        key: &[i32],
    ) -> Result<TCoeff, String> {
        let (num, den) = alg.parse_ratio(text.trim(), arity)?;
        self.coeff_at_key(&num, &den, arity, key)
    }

    /// Put the key's index values into numerator / denominator polynomials in `n0 .. n{arity-1}, d` (variable `d`
    /// at index `arity`) and build the exact coefficient.
    pub fn coeff_at_key(
        &self,
        num: &ZPoly,
        den: &ZPoly,
        arity: usize,
        key: &[i32],
    ) -> Result<TCoeff, String> {
        if key.len() != arity {
            return Err(format!(
                "key of arity {}, coefficient of arity {arity}",
                key.len()
            ));
        }
        let vals: Vec<Integer> = key.iter().map(|&x| Integer::from(x as i64)).collect();
        let reps: Vec<(usize, &Integer)> = vals.iter().enumerate().collect();
        let n = num.replace_multiple(&reps, false);
        let d = den.replace_multiple(&reps, false);
        self.coeff_from_zpolys(&n, &d, arity)
    }

    /// The series of `c` known for every exponent `< abs_prec` (at least one term); `None` for zero.
    pub fn expand_coeff(&self, c: &TCoeff, abs_prec: i64) -> Result<Option<QSeries>, String> {
        let (v, n1, d1) = match c {
            TCoeff::Zero => return Ok(None),
            TCoeff::Ratio { v, n1, d1 } => (*v, n1, d1),
        };
        let r = (abs_prec - v).max(1);
        let r = usize::try_from(r).map_err(|_| "expansion depth out of range".to_string())?;
        // d1^-1 mod t^r (Symbolica extended Euclid; gcd(d1, t^r) = 1 since d1(0) != 0)
        let tr = d1.monomial(Q.one(), r);
        let (g, s, _) = d1.extended_gcd(&tr);
        if !g.is_one() {
            return Err("internal: gcd(d1, t^r) != 1".into());
        }
        let q = n1 * &s;
        let coeffs = q.coefficients();
        let base = QSeries::new(
            &Q,
            Some(r),
            self.tvar.clone(),
            Q.zero(),
            Rational::from(r as i64),
        );
        let mut terms = Vec::with_capacity(r);
        for (i, c) in coeffs.iter().take(r).enumerate() {
            if !Q.is_zero(c) {
                terms.push(base.monomial(c.clone(), Rational::from(v + i as i64)));
            }
        }
        if terms.is_empty() || Q.is_zero(&coeffs[0]) {
            return Err("internal: the leading Laurent coefficient vanished".into());
        }
        Ok(Some(QSeries::sum(terms)))
    }

    /// (lowest known non-zero exponent or None, absolute precision) of a series.
    pub fn series_order(s: &QSeries) -> (Option<i64>, i64) {
        let abs = s.absolute_order();
        let p = abs.numerator().to_i64().unwrap_or(i64::MAX);
        if s.is_zero() {
            (None, p)
        } else {
            (s.get_trailing_exponent().numerator().to_i64(), p)
        }
    }

    /// The coefficient of `t^e` of a series (exact; `None` when `e` is beyond its precision).
    pub fn series_coefficient(s: &QSeries, e: i64) -> Option<Rational> {
        let (_, p) = Self::series_order(s);
        if e >= p {
            return None;
        }
        Some(s.coefficient(Rational::from(e)).unwrap_or_else(|| Q.zero()))
    }
}

impl Laurent for SymbolicaLaurent {
    type Coeff = TCoeff;
    type Series = QSeries;

    fn valuation(&mut self, c: &TCoeff) -> Result<Val, String> {
        Ok(c.valuation())
    }
    fn expand(&mut self, c: &TCoeff, abs_prec: i64) -> Result<Option<QSeries>, String> {
        self.expand_coeff(c, abs_prec)
    }
    fn add(&self, a: &QSeries, b: &QSeries) -> QSeries {
        a + b
    }
    fn mul(&self, a: &QSeries, b: &QSeries) -> QSeries {
        a * b
    }
    fn order(&self, s: &QSeries) -> (Option<i64>, i64) {
        Self::series_order(s)
    }
    fn leading_text(&self, s: &QSeries) -> Option<String> {
        (!s.is_zero()).then(|| s.get_trailing_coefficient().to_string())
    }
    fn one(&mut self, abs_prec: i64) -> QSeries {
        let r = abs_prec.max(1);
        QSeries::new(&Q, Some(1), self.tvar.clone(), Q.zero(), Rational::from(r))
            .monomial(Q.one(), Rational::from(0))
    }
}

/// An EXACT rational function of `t = d - d0` over Z (Symbolica `RationalPolynomial`, gcd-normalised): the "series"
/// of [`SymbolicaExact`], of infinite precision.
pub type TRational = RationalPolynomial<IntegerRing, u16>;

/// The EXACT backend of the composition (POLES_OBJECTIVE 2A: "compose exactly in d down to the terminals before
/// anything is expanded"): every coefficient and every composed vector component is an exact rational function of
/// `t = d - d0` (Symbolica `RationalPolynomial` over Z: add / mul with gcd normalisation), so no precision is ever
/// lost or needed: the order at d0 is the multiplicity of `t` in numerator minus denominator. Same `Coeff` as
/// [`SymbolicaLaurent`] (any [`ReductionSource`] of `TCoeff` works with both).
#[derive(Clone, Debug)]
pub struct SymbolicaExact {
    vars: Arc<Vec<PolyVariable>>,
}

impl SymbolicaExact {
    pub fn new() -> Result<SymbolicaExact, String> {
        let ns = NamespacedSymbol::try_parse("rustred::epsdepth_t")
            .ok_or("symbol rustred::epsdepth_t")?;
        let t = SymbolBuilder::new(ns).build().map_err(|e| e.to_string())?;
        Ok(SymbolicaExact {
            vars: Arc::new(vec![PolyVariable::Symbol(t)]),
        })
    }

    fn qpoly(
        &self,
        coeffs: &[Rational],
        shift: usize,
    ) -> MultivariatePolynomial<RationalField, u16> {
        let mut p = MultivariatePolynomial::new(&Q, Some(coeffs.len()), self.vars.clone());
        for (i, c) in coeffs.iter().enumerate() {
            if !Q.is_zero(c) {
                p.append_monomial(c.clone(), &[(i + shift) as u16]);
            }
        }
        p
    }

    /// The exact rational function of a coefficient (`None` = zero).
    pub fn rational(&self, c: &TCoeff) -> Option<TRational> {
        let TCoeff::Ratio { v, n1, d1 } = c else {
            return None;
        };
        let num = self.qpoly(n1.coefficients(), (*v).max(0) as usize);
        let den = self.qpoly(d1.coefficients(), (-*v).max(0) as usize);
        Some(<TRational as FromNumeratorAndDenominator<
            RationalField,
            IntegerRing,
            u16,
        >>::from_num_den(num, den, &Z, true))
    }

    /// Order at d0 (multiplicity of t) of a non-zero rational function; `None` for zero.
    pub fn order_of(r: &TRational) -> Option<i64> {
        if r.numerator.is_zero() {
            return None;
        }
        Some(
            i64::from(r.numerator.degree_bounds(0).0) - i64::from(r.denominator.degree_bounds(0).0),
        )
    }

    /// The exact coefficient of `t^e` of the Laurent expansion is not needed here; the LEADING coefficient (of t^order).
    pub fn leading(r: &TRational) -> Option<Rational> {
        if r.numerator.is_zero() {
            return None;
        }
        let low = |p: &MultivariatePolynomial<IntegerRing, u16>| -> Integer {
            let m = p.degree_bounds(0).0;
            p.into_iter()
                .find(|t| t.exponents[0] == m)
                .map(|t| t.coefficient.clone())
                .unwrap_or_else(|| Integer::from(0i64))
        };
        Some(Rational::from((low(&r.numerator), low(&r.denominator))))
    }
}

impl Laurent for SymbolicaExact {
    type Coeff = TCoeff;
    type Series = TRational;

    fn valuation(&mut self, c: &TCoeff) -> Result<Val, String> {
        Ok(c.valuation())
    }
    fn expand(&mut self, c: &TCoeff, _abs_prec: i64) -> Result<Option<TRational>, String> {
        Ok(self.rational(c))
    }
    fn add(&self, a: &TRational, b: &TRational) -> TRational {
        a + b
    }
    fn mul(&self, a: &TRational, b: &TRational) -> TRational {
        a * b
    }
    fn order(&self, s: &TRational) -> (Option<i64>, i64) {
        (Self::order_of(s), i64::MAX)
    }
    fn leading_text(&self, s: &TRational) -> Option<String> {
        Self::leading(s).map(|q| q.to_string())
    }
    fn one(&mut self, _abs_prec: i64) -> TRational {
        let one = MultivariatePolynomial::new(&Z, Some(1), self.vars.clone()).one();
        TRational::from(one)
    }
}

/// The core trait `Valuation` on lane poles' exact valuation (`valuation::order_at_key`).
#[derive(Clone, Copy, Debug, Default)]
pub struct ExactValuation;

impl Valuation for ExactValuation {
    fn pole_order(
        &mut self,
        coeff: &str,
        arity: usize,
        key: &[i32],
        d0: &str,
    ) -> Result<Val, String> {
        let d0 = D0::parse(d0).map_err(|e| e.to_string())?;
        let key: Vec<i64> = key.iter().map(|&x| i64::from(x)).collect();
        match order_at_key(coeff, arity, &key, &d0).map_err(|e| e.to_string())? {
            PolesValuation::Zero => Ok(Val::Zero),
            PolesValuation::Order(v) => Ok(Val::Order(v)),
        }
    }
}

/// (server label, [(term key, coefficient text)]) of one served key of a [`TextReduction`].
type TextRelation = (String, Vec<(Vec<i32>, String)>);

/// A reduction given as coefficient TEXTS (tests, small hand-made nodes, other lanes' fixtures): every served key has
/// a relation `I(key) = sum_j c_j I(k_j)` over top-sector keys, `c_j` a rational function of `d` and the key's
/// indices `n0 .. n{arity-1}` (put in at the key: index-dependent poles, TRICKS 9).
pub struct TextReduction {
    pub keys: Vec<Vec<i32>>,
    index: HashMap<Vec<i32>, usize>,
    terminals: Vec<usize>,
    relations: HashMap<usize, TextRelation>,
    open: HashMap<usize, String>,
    alg: SymbolicaAlgebra,
    lau: SymbolicaLaurent,
    arity: usize,
}

impl TextReduction {
    pub fn new(alg: SymbolicaAlgebra, lau: SymbolicaLaurent, arity: usize) -> TextReduction {
        TextReduction {
            keys: vec![],
            index: HashMap::new(),
            terminals: vec![],
            relations: HashMap::new(),
            open: HashMap::new(),
            alg,
            lau,
            arity,
        }
    }
    fn intern(&mut self, k: &[i32]) -> usize {
        if let Some(&i) = self.index.get(k) {
            return i;
        }
        let i = self.keys.len();
        self.keys.push(k.to_vec());
        self.index.insert(k.to_vec(), i);
        i
    }
    /// Declare a terminal (component order = declaration order).
    pub fn terminal(&mut self, k: &[i32]) -> usize {
        let i = self.intern(k);
        self.terminals.push(i);
        i
    }
    /// Declare the relation serving `k` (server label for blame lists).
    pub fn relation(&mut self, k: &[i32], server: &str, rhs: &[(&[i32], &str)]) -> usize {
        let i = self.intern(k);
        for (t, _) in rhs {
            self.intern(t);
        }
        self.relations.insert(
            i,
            (
                server.to_string(),
                rhs.iter()
                    .map(|(t, c)| (t.to_vec(), c.to_string()))
                    .collect(),
            ),
        );
        i
    }
    /// Declare an unserved key.
    pub fn open(&mut self, k: &[i32], why: &str) -> usize {
        let i = self.intern(k);
        self.open.insert(i, why.to_string());
        i
    }
    pub fn index_of(&self, k: &[i32]) -> Option<usize> {
        self.index.get(k).copied()
    }
    pub fn n_terminals(&self) -> usize {
        self.terminals.len()
    }
    pub fn terminal_keys(&self) -> Vec<Vec<i32>> {
        self.terminals
            .iter()
            .map(|&i| self.keys[i].clone())
            .collect()
    }
}

impl ReductionSource for TextReduction {
    type Coeff = TCoeff;
    fn n_keys(&self) -> usize {
        self.keys.len()
    }
    fn key(&self, i: usize) -> Vec<i32> {
        self.keys[i].clone()
    }
    fn status(&self, i: usize) -> KeyStatus {
        if let Some(j) = self.terminals.iter().position(|&t| t == i) {
            return KeyStatus::Terminal(j);
        }
        if let Some(r) = self.open.get(&i) {
            return KeyStatus::Open(r.clone());
        }
        if self.relations.contains_key(&i) {
            KeyStatus::Served
        } else {
            KeyStatus::Open("no relation".into())
        }
    }
    fn server(&self, i: usize) -> String {
        self.relations
            .get(&i)
            .map(|r| r.0.clone())
            .unwrap_or_default()
    }
    fn relations(
        &mut self,
        keys: &[usize],
        visit: &mut dyn FnMut(usize, Result<Terms<TCoeff>, String>),
    ) -> Result<(), String> {
        for &k in keys {
            let Some((_, rhs)) = self.relations.get(&k) else {
                visit(k, Err("no relation".into()));
                continue;
            };
            let key = self.keys[k].clone();
            let res: Result<Terms<TCoeff>, String> = rhs
                .iter()
                .map(|(t, c)| {
                    let ti = self.index[t];
                    self.lau
                        .coeff_from_text(&self.alg, c, self.arity, &key)
                        .map(|c| (ti, c))
                })
                .collect();
            visit(k, res);
        }
        Ok(())
    }
}
