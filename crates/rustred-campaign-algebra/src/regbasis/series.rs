//! Exact truncated Laurent series in `t = d - d0` with rational coefficients (Symbolica `MultivariatePolynomial` over Q
//! in one variable), the parser of the core's text exchange, and the exact expansion of a rational function of d.
//!
//! Precision semantics (absolute, as lane epsdepth's composition): a [`Laurent`] knows every exponent `< prec`; the
//! product of `a` and `b` knows every exponent `< min(v(a) + prec(b), v(b) + prec(a))` where `v` is the valuation, or
//! the precision for a series that is zero to its precision; a sum knows `< min(prec(a), prec(b))`.

use std::str::FromStr;
use std::sync::Arc;

use symbolica::atom::{NamespacedSymbol, SymbolBuilder};
use symbolica::domains::integer::{Integer, IntegerRing, Z};
use symbolica::domains::rational::{Q, Rational, RationalField};
use symbolica::domains::rational_polynomial::RationalPolynomial;
use symbolica::poly::PolyVariable;
use symbolica::poly::polynomial::MultivariatePolynomial;
use symbolica::prelude::AtomCore;
use symbolica::printer::PrintOptions;
use symbolica::try_parse;

use rustred_campaign_core::regbasis::SeriesText;

/// Polynomials over Q in one variable (t, or d).
pub type QP = MultivariatePolynomial<RationalField, u16>;
/// Polynomials over Z in one variable.
pub type ZP = MultivariatePolynomial<IntegerRing, u16>;
/// Rational functions over Z in one variable (d).
pub type RF = RationalPolynomial<IntegerRing, u16>;

fn one_var(name: &str) -> Arc<Vec<PolyVariable>> {
    let ns = NamespacedSymbol::try_parse(name).expect("symbol name");
    let s = SymbolBuilder::new(ns).build().expect("symbol");
    Arc::new(vec![PolyVariable::Symbol(s)])
}

/// The variable map `[t]` of the lattice arithmetic (namespace `rustred_regbasis`).
pub fn t_vars() -> Arc<Vec<PolyVariable>> {
    one_var("rustred_regbasis::t")
}

/// The variable map `[d]` (namespace `rustred`, the house coefficient syntax).
pub fn d_vars() -> Arc<Vec<PolyVariable>> {
    one_var("rustred::d")
}

/// Exact rational from `a` or `a/b` (optional sign, decimal digits).
pub fn parse_rational(text: &str) -> Result<Rational, String> {
    let t = text.trim();
    let (a, b) = t.split_once('/').unwrap_or((t, "1"));
    let a = Integer::from_str(a.trim().strip_prefix('+').unwrap_or(a.trim()))
        .map_err(|_| format!("not a rational: {text:?}"))?;
    let b = Integer::from_str(b.trim()).map_err(|_| format!("not a rational: {text:?}"))?;
    if b.is_zero() {
        return Err(format!("zero denominator: {text:?}"));
    }
    Ok(Rational::from((a, b)))
}

/// Text of an exact rational (`a` or `a/b`).
pub fn rational_text(r: &Rational) -> String {
    if r.is_integer() {
        r.numerator_ref().to_string()
    } else {
        format!("{}/{}", r.numerator_ref(), r.denominator_ref())
    }
}

/// The lowest exponent of a non-zero univariate polynomial.
pub fn low(p: &QP) -> Option<u32> {
    if p.is_zero() {
        None
    } else {
        Some(u32::from(p.exponents(0)[0]))
    }
}

/// p / t^e (caller guarantees low(p) >= e).
pub fn div_t(p: &QP, e: u32) -> QP {
    if e == 0 || p.is_zero() {
        return p.clone();
    }
    let mut q = p.clone();
    for x in q.exponents_iter_mut() {
        x[0] -= e as u16;
    }
    q
}

/// p * t^e.
pub fn mul_t(p: &QP, e: u32) -> QP {
    if e == 0 || p.is_zero() {
        p.clone()
    } else {
        p.clone().mul_exp(&[e as u16])
    }
}

/// The terms of p of degree >= e, divided by t^e (the polynomial quotient by t^e).
pub fn quot_t(p: &QP, e: u32) -> QP {
    let mut q = p.zero();
    for i in 0..p.nterms() {
        let x = u32::from(p.exponents(i)[0]);
        if x >= e {
            q.append_monomial(p.coefficients[i].clone(), &[(x - e) as u16]);
        }
    }
    q
}

/// p mod t^e.
pub fn mod_t(p: &QP, e: u32) -> QP {
    p.mod_var(0, e as u16)
}

/// Inverse of a unit (constant term non-zero) modulo t^e.
pub fn unit_inverse(u: &QP, e: u32) -> QP {
    if e == 0 {
        return u.zero();
    }
    mod_t(&u.inverse_univariate(0, e as u16), e)
}

/// An exact truncated Laurent series: `t^start * poly(t)`, every exponent `< prec` known; `poly` holds only terms with
/// `start + exponent < prec`; normalised so that a non-zero `poly` has a non-zero constant term (then `start` is the
/// valuation), and a zero series has `start = prec`.
#[derive(Clone, Debug)]
pub struct Laurent {
    pub start: i64,
    pub poly: QP,
    pub prec: i64,
}

impl Laurent {
    pub fn zero(prec: i64, vars: &Arc<Vec<PolyVariable>>) -> Laurent {
        Laurent {
            start: prec,
            poly: QP::new(&Q, None, vars.clone()),
            prec,
        }
    }

    /// Normalise: truncate to `prec`, shift the lowest term to exponent 0.
    pub fn normalised(start: i64, poly: QP, prec: i64) -> Laurent {
        let keep = prec - start;
        let poly = if keep <= 0 {
            poly.zero()
        } else {
            mod_t(&poly, keep.min(u16::MAX as i64) as u32)
        };
        match low(&poly) {
            None => Laurent {
                start: prec,
                poly,
                prec,
            },
            Some(l) => Laurent {
                start: start + i64::from(l),
                poly: div_t(&poly, l),
                prec,
            },
        }
    }

    /// The valuation (`None`: zero to its precision).
    pub fn valuation(&self) -> Option<i64> {
        if self.poly.is_zero() {
            None
        } else {
            Some(self.start)
        }
    }

    /// The pole order (0 for a regular or zero series; the caller checks that the precision decides it, see
    /// [`Laurent::decides_pole`]).
    pub fn pole_order(&self) -> u32 {
        match self.valuation() {
            Some(v) if v < 0 => (-v) as u32,
            _ => 0,
        }
    }

    /// The precision decides the pole order (every negative exponent is known): `prec >= 0` or a non-zero term seen.
    pub fn decides_pole(&self) -> bool {
        self.prec >= 0 || self.valuation().is_some()
    }

    /// Valuation, or the precision for a series zero to its precision (the "w" of the precision rule).
    fn w(&self) -> i64 {
        self.valuation().unwrap_or(self.prec)
    }

    pub fn mul(&self, o: &Laurent) -> Laurent {
        let prec = (self.w() + o.prec).min(o.w() + self.prec);
        if self.poly.is_zero() || o.poly.is_zero() {
            return Laurent {
                start: prec,
                poly: self.poly.zero(),
                prec,
            };
        }
        Laurent::normalised(self.start + o.start, &self.poly * &o.poly, prec)
    }

    pub fn add(&self, o: &Laurent) -> Laurent {
        let prec = self.prec.min(o.prec);
        if self.poly.is_zero() {
            return Laurent::normalised(o.start, o.poly.clone(), prec);
        }
        if o.poly.is_zero() {
            return Laurent::normalised(self.start, self.poly.clone(), prec);
        }
        let s = self.start.min(o.start);
        let a = mul_t(&self.poly, (self.start - s) as u32);
        let b = mul_t(&o.poly, (o.start - s) as u32);
        Laurent::normalised(s, &a + &b, prec)
    }

    /// The coefficient of t^e (exact; Err when e >= prec).
    pub fn coeff(&self, e: i64) -> Result<Rational, String> {
        if e >= self.prec {
            return Err(format!("exponent {e} beyond precision {}", self.prec));
        }
        let k = e - self.start;
        if k < 0 {
            return Ok(Rational::zero());
        }
        for i in 0..self.poly.nterms() {
            if i64::from(self.poly.exponents(i)[0]) == k {
                return Ok(self.poly.coefficients[i].clone());
            }
        }
        Ok(Rational::zero())
    }

    /// The core's text form.
    pub fn to_text(&self) -> SeriesText {
        match self.valuation() {
            None => SeriesText::zero(self.prec),
            Some(v) => {
                let hi = self.prec - v;
                let coeffs = (0..hi)
                    .map(|k| rational_text(&self.coeff(v + k).expect("below precision")))
                    .collect::<Vec<_>>();
                // drop trailing zeros (they are implied up to prec)
                let mut c = coeffs;
                while c.last().is_some_and(|x| x == "0") {
                    c.pop();
                }
                SeriesText {
                    start: v,
                    coeffs: c,
                    prec: self.prec,
                }
            }
        }
    }

    /// The pair of polynomials over Q in t: `t^shift * self` truncated below exponent `s` (requires the series known to
    /// `s - shift`): the entry of a scaled lattice vector.
    pub fn scaled_entry(&self, shift: i64, s: u32) -> Result<QP, String> {
        if self.prec < i64::from(s) - shift {
            return Err(format!(
                "precision {} below the {} needed (scaled exponent bound {s}, shift {shift})",
                self.prec,
                i64::from(s) - shift
            ));
        }
        if self.poly.is_zero() {
            return Ok(self.poly.zero());
        }
        let e = self.start + shift;
        if e < 0 {
            return Err(format!(
                "scaled entry has a negative exponent {e} (shift {shift} below the pole order)"
            ));
        }
        Ok(mod_t(&mul_t(&self.poly, e as u32), s))
    }
}

/// Parse the core's text form into a [`Laurent`].
pub fn parse_series(s: &SeriesText, vars: &Arc<Vec<PolyVariable>>) -> Result<Laurent, String> {
    s.validate()?;
    let mut p = QP::new(&Q, Some(s.coeffs.len()), vars.clone());
    for (k, c) in s.coeffs.iter().enumerate() {
        let r = parse_rational(c)?;
        if !r.is_zero() {
            p.append_monomial(r, &[u16::try_from(k).map_err(|_| "series too long")?]);
        }
    }
    Ok(Laurent::normalised(s.start, p, s.prec))
}

/// Parse a coefficient text (house syntax, a rational function of `d` only) into a rational function over Z in d.
pub fn parse_rf(text: &str) -> Result<RF, String> {
    let atom = try_parse!(text, default_namespace = "rustred")
        .map_err(|e| format!("parse {text:?}: {e}"))?;
    let rf: RF = atom
        .as_view()
        .try_to_rational_polynomial::<_, _, u16>(&Q, &Z, Some(d_vars()))
        .map_err(|e| format!("not a rational function of d: {text:?}: {e}"))?;
    if rf.numerator.get_vars_ref().len() != 1 {
        return Err(format!("coefficient {text:?} has symbols other than d"));
    }
    Ok(rf)
}

fn print_zp(p: &ZP) -> String {
    let opts = PrintOptions {
        hide_all_namespaces: true,
        ..PrintOptions::new()
    };
    p.to_expression().printer(opts).to_string()
}

/// Text of a rational function of d (`num` or `(num)/(den)`; house syntax, parses back with [`parse_rf`]).
pub fn rf_text(r: &RF) -> String {
    if r.numerator.is_zero() {
        return "0".into();
    }
    if r.denominator.is_one() {
        return print_zp(&r.numerator);
    }
    format!(
        "({})/({})",
        print_zp(&r.numerator),
        print_zp(&r.denominator)
    )
}

fn shifted_q(p: &ZP, d0: &Rational, tv: &Arc<Vec<PolyVariable>>) -> QP {
    let q: QP = p
        .map_coeff(|c| Rational::from(c.clone()), Q)
        .shift_var(0, d0);
    // re-home the polynomial in the variable t
    let mut out = QP::new(&Q, Some(q.nterms()), tv.clone());
    for i in 0..q.nterms() {
        out.append_monomial(q.coefficients[i].clone(), &[q.exponents(i)[0]]);
    }
    out
}

/// The exact valuation at d0 of a rational function (`None` = identically zero).
pub fn rf_valuation(r: &RF, d0: &Rational) -> Option<i64> {
    if r.numerator.is_zero() {
        return None;
    }
    let tv = t_vars();
    let n = shifted_q(&r.numerator, d0, &tv);
    let d = shifted_q(&r.denominator, d0, &tv);
    Some(i64::from(low(&n).expect("non-zero")) - i64::from(low(&d).expect("non-zero")))
}

/// The exact expansion of a rational function of d at d0, every exponent `< abs_prec` known.
pub fn expand_rf(r: &RF, d0: &Rational, abs_prec: i64, tv: &Arc<Vec<PolyVariable>>) -> Laurent {
    if r.numerator.is_zero() {
        return Laurent::zero(abs_prec, tv);
    }
    let n = shifted_q(&r.numerator, d0, tv);
    let d = shifted_q(&r.denominator, d0, tv);
    expand_at_zero(&n, &d, abs_prec, tv)
}

/// The exact expansion at t = 0 of `num / den` given over Z in ANY one variable (read as t), every exponent
/// `< abs_prec` known (e.g. lane epsdepth's exact composed rational functions of t = d - d0).
pub fn expand_t_ratio(num: &ZP, den: &ZP, abs_prec: i64, tv: &Arc<Vec<PolyVariable>>) -> Laurent {
    if num.is_zero() {
        return Laurent::zero(abs_prec, tv);
    }
    let rehome = |p: &ZP| -> QP {
        let mut out = QP::new(&Q, Some(p.nterms()), tv.clone());
        for i in 0..p.nterms() {
            out.append_monomial(
                Rational::from(p.coefficients[i].clone()),
                &[p.exponents(i)[0]],
            );
        }
        out
    };
    expand_at_zero(&rehome(num), &rehome(den), abs_prec, tv)
}

fn expand_at_zero(n: &QP, d: &QP, abs_prec: i64, tv: &Arc<Vec<PolyVariable>>) -> Laurent {
    let (vn, vd) = (low(n).expect("non-zero"), low(d).expect("non-zero"));
    let v = i64::from(vn) - i64::from(vd);
    let terms = abs_prec - v;
    if terms <= 0 {
        // nothing below the precision: the series is zero to abs_prec (its valuation is v >= abs_prec)
        return Laurent::zero(abs_prec, tv);
    }
    let terms = terms as u32;
    let nu = div_t(n, vn);
    let du = div_t(d, vd);
    let inv = unit_inverse(&du, terms);
    Laurent::normalised(v, mod_t(&(&nu * &inv), terms), abs_prec)
}
