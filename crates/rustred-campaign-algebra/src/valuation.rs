//! # The exact order at d = d0 of a rational coefficient in d (lane poles, B0; interfaces/pole-diagnostics.md 1.1, 7)
//!
//! `v_{d0}(f)` = the lowest exponent of `t = d - d0` in the Laurent expansion of `f`; for `f = N / D` gcd-reduced it is
//! `mult_{d0}(N) - mult_{d0}(D)`. A coefficient that is identically zero has NO order: it is [`Valuation::Zero`], its
//! own value (never "order 0"). The pole order of `f` at `d0` is `max(0, -v)` ([`Valuation::pole_order`]).
//!
//! EVERY STEP IS SYMBOLICA (L11; the verification is recorded in WS/notes/implementer.poles.md "SYMBOLICA"):
//! - parsing: the general atom parser `try_parse!` (default namespace `rustred`) + `AtomCore::try_to_rational_polynomial
//!   (&Q, &Z, vars)` (src/atom/core.rs): numerator / denominator over Z, gcd-reduced, exactly the engine's coefficient
//!   path (crates/rustred-core/src/propagator/basis.rs `parse_coefficient`) and the algebra crate's
//!   `SymbolicaAlgebra::parse_ratio`; NOT `Token::parse_polynomial` (four defects, crate doc);
//! - the indices of a key put in: `MultivariatePolynomial::replace(var, value)` over Z (src/poly/polynomial.rs:2739),
//!   exact big integers;
//! - the multiplicity of `(d - d0)`: `map_coeff` Z -> Q (2133), `shift_var(d, d0)` (3058: `P(d) -> P(d + d0)`, exact over Q,
//!   `d0` any rational), then the lowest exponent of `d`, `degree_bounds(d).0` (2246); the coefficient of `t^m`
//!   (`to_univariate_polynomial_list`, 3759) is the polynomial in the indices whose zero set is the LOCUS where the order
//!   at d0 rises above its generic value (TRICKS item 9: poles can be index-dependent; a per-key verdict is therefore
//!   always computed AT THE KEY, with the key's indices substituted, never by searching `(d - d0)` in the text);
//! - the locus as irreducible factors: `Factorize::factor` over Q (src/poly/factor.rs:5066);
//! - printing: `MultivariatePolynomial::to_expression` + `AtomCore::printer` with every namespace hidden.
//!
//! Exact arithmetic only (Symbolica `Integer` = GMP big integers, `Rational`); no floating point, no modular shortcut.
//! Generic (L8): the arity is a parameter, the variable names are the engine's `n0 .. n{N-1}`, `d`.

pub mod relation;

use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use symbolica::atom::{NamespacedSymbol, SymbolBuilder};
use symbolica::domains::integer::{Integer, IntegerRing, Z};
use symbolica::domains::rational::{Q, Rational, RationalField};
use symbolica::poly::PolyVariable;
use symbolica::poly::factor::Factorize;
use symbolica::poly::polynomial::MultivariatePolynomial;
use symbolica::prelude::AtomCore;
use symbolica::printer::PrintOptions;
use symbolica::try_parse;

/// One line naming the method (written into every record that carries an order).
pub const VALUATION_METHOD: &str = "symbolica try_to_rational_polynomial(Q,Z) gcd-reduced N/D; replace(n_i, key) over Z; \
     map_coeff Z->Q; shift_var(d, d0); degree_bounds(d).0 = multiplicity; v = mult(N) - mult(D); exact";

type ZPoly = MultivariatePolynomial<IntegerRing, u16>;
type QPoly = MultivariatePolynomial<RationalField, u16>;

/// The order of a coefficient at `d0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Valuation {
    /// The coefficient is identically zero (at the key, when indices were substituted): it has no order.
    Zero,
    /// `v_{d0}`: > 0 a zero of that order, 0 regular and non-zero at d0, < 0 a pole of order `-v`.
    Order(i64),
}

impl Valuation {
    /// `max(0, -v)`; 0 for [`Valuation::Zero`] (a zero coefficient is regular).
    pub fn pole_order(&self) -> u64 {
        match self {
            Valuation::Order(v) if *v < 0 => v.unsigned_abs(),
            _ => 0,
        }
    }

    /// Regular at d0 (no pole; a zero coefficient is regular).
    pub fn is_regular(&self) -> bool {
        self.pole_order() == 0
    }

    pub fn is_zero(&self) -> bool {
        matches!(self, Valuation::Zero)
    }
}

/// Typed failures of the valuation. A failure is never an order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValuationError {
    /// The text does not parse, or is not a rational function (e.g. a function call, a float).
    NotRational { reason: String },
    /// The text holds symbols other than the allowed ones (`d`, and `n0 .. n{N-1}` for parametric coefficients).
    NotRationalInD { symbols: Vec<String> },
    /// `d0` is not the text of a rational number `a` or `a/b` (b != 0).
    D0NotRational { text: String },
    /// The denominator is identically zero.
    ZeroDenominator,
    /// The denominator vanishes identically once the key's indices are put in (the coefficient is undefined there).
    ZeroDenominatorAtKey { key: Vec<i64> },
    /// The key has another arity than the coefficient's variable map.
    KeyArity { expected: usize, got: usize },
}

impl fmt::Display for ValuationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValuationError::NotRational { reason } => {
                write!(f, "not a rational function: {reason}")
            }
            ValuationError::NotRationalInD { symbols } => {
                write!(
                    f,
                    "not a rational function of the allowed variables: extra symbols {symbols:?}"
                )
            }
            ValuationError::D0NotRational { text } => {
                write!(f, "d0 {text:?} is not a rational number a or a/b")
            }
            ValuationError::ZeroDenominator => write!(f, "the denominator is identically zero"),
            ValuationError::ZeroDenominatorAtKey { key } => {
                write!(f, "the denominator vanishes identically at the key {key:?}")
            }
            ValuationError::KeyArity { expected, got } => {
                write!(
                    f,
                    "key of arity {got}, the coefficient has {expected} indices"
                )
            }
        }
    }
}

impl std::error::Error for ValuationError {}

/// A sampling dimension d0: an exact rational with its canonical text (`a` or `a/b`, lowest terms, b > 0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct D0 {
    value: Rational,
    text: String,
}

impl D0 {
    /// Parse `a` or `a/b` (optional sign on `a`; decimal digits; b != 0); normalised to lowest terms, b > 0. Anything
    /// else (a float, an expression, whitespace inside) is [`ValuationError::D0NotRational`].
    pub fn parse(text: &str) -> Result<D0, ValuationError> {
        let bad = || ValuationError::D0NotRational {
            text: text.to_string(),
        };
        let t = text.trim();
        let (a, b) = match t.split_once('/') {
            Some((a, b)) => (a, b),
            None => (t, "1"),
        };
        let digits = |s: &str, signed: bool| {
            let s = if signed {
                s.strip_prefix('-')
                    .or_else(|| s.strip_prefix('+'))
                    .unwrap_or(s)
            } else {
                s
            };
            !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit())
        };
        if !digits(a, true) || !digits(b, true) {
            return Err(bad());
        }
        let a = Integer::from_str(a.strip_prefix('+').unwrap_or(a)).map_err(|_| bad())?;
        let b = Integer::from_str(b.strip_prefix('+').unwrap_or(b)).map_err(|_| bad())?;
        if b.is_zero() {
            return Err(bad());
        }
        // Symbolica normalises (gcd, positive denominator)
        let value: Rational = Rational::from((a, b));
        let text = if value.is_integer() {
            value.numerator_ref().to_string()
        } else {
            format!("{}/{}", value.numerator_ref(), value.denominator_ref())
        };
        Ok(D0 { value, text })
    }

    pub fn value(&self) -> &Rational {
        &self.value
    }

    /// The canonical text (`"4"`, `"17/7"`).
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The directory label of pole-diagnostics.md 12: `d4`, `d17_7`, `dm3_2` for -3/2.
    pub fn label(&self) -> String {
        format!("d{}", self.text.replace('/', "_").replace('-', "m"))
    }
}

impl fmt::Display for D0 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

/// The engine's variable map: `n0 .. n{arity-1}` then `d` (namespace `rustred`).
fn variable_map(arity: usize) -> Result<Arc<Vec<PolyVariable>>, ValuationError> {
    let mut vars = Vec::with_capacity(arity + 1);
    for name in (0..arity)
        .map(|i| format!("n{i}"))
        .chain(std::iter::once("d".to_string()))
    {
        let q = format!("rustred::{name}");
        let ns = NamespacedSymbol::try_parse(&q).ok_or_else(|| ValuationError::NotRational {
            reason: format!("symbol {q}"),
        })?;
        let s = SymbolBuilder::new(ns)
            .build()
            .map_err(|e| ValuationError::NotRational {
                reason: e.to_string(),
            })?;
        vars.push(PolyVariable::Symbol(s));
    }
    Ok(Arc::new(vars))
}

fn print_poly(p: &QPoly) -> String {
    let opts = PrintOptions {
        hide_all_namespaces: true,
        ..PrintOptions::new()
    };
    p.to_expression().printer(opts).to_string()
}

/// The multiplicity of `(d - d0)` in `p` (variable index `dv`) and the coefficient of `t^m` after `d -> t + d0`
/// (a polynomial in the other variables; constant when `p` is univariate in d). `p` must be non-zero.
fn multiplicity(p: &ZPoly, dv: usize, d0: &Rational) -> (u64, QPoly) {
    let q: QPoly = p.map_coeff(|c| Rational::from(c), Q);
    let s = q.shift_var(dv, d0);
    let m = s.degree_bounds(dv).0;
    let rest = s
        .to_univariate_polynomial_list(dv)
        .into_iter()
        .find(|(_, e)| *e == m)
        .map(|(c, _)| c)
        .expect("the lowest exponent of a non-zero polynomial has a coefficient");
    (u64::from(m), rest)
}

/// The zero set of a polynomial in the indices, as irreducible factors over Q (Symbolica `factor`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Locus {
    /// The polynomial (printed, namespaces hidden); its zero set is where the order at d0 changes.
    pub polynomial: String,
    /// Irreducible non-constant factors with multiplicity (empty: the polynomial is a non-zero constant, no locus).
    pub factors: Vec<(String, usize)>,
}

impl Locus {
    fn of(p: &QPoly) -> Locus {
        let factors = if p.is_constant() {
            vec![]
        } else {
            p.factor()
                .into_iter()
                .filter(|(f, _)| !f.is_constant())
                .map(|(f, e)| (print_poly(&f), e))
                .collect()
        };
        Locus {
            polynomial: print_poly(p),
            factors,
        }
    }

    /// No index condition (a non-zero constant).
    pub fn is_empty(&self) -> bool {
        self.factors.is_empty()
    }
}

/// The order of a parametric coefficient for GENERIC indices, with the exact index conditions where it changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenericOrder {
    /// `mult(N) - mult(D)` as polynomials in (n, d): the order at every key OFF both loci.
    pub order: Valuation,
    pub num_multiplicity: u64,
    pub den_multiplicity: u64,
    /// Keys on this locus: the denominator has a HIGHER multiplicity there (the pole rises, or a zero falls).
    pub den_locus: Locus,
    /// Keys on this locus: the numerator has a higher multiplicity (the pole falls, or the coefficient vanishes).
    pub num_locus: Locus,
}

/// A coefficient parsed once (Symbolica), evaluated at d0 generically or at many keys.
#[derive(Debug, Clone)]
pub struct Coefficient {
    num: ZPoly,
    den: ZPoly,
    arity: usize,
}

impl Coefficient {
    /// Parse a coefficient text: a rational function of `d` (arity 0) or of `n0 .. n{arity-1}, d`.
    pub fn parse(text: &str, arity: usize) -> Result<Coefficient, ValuationError> {
        Self::parse_parts(text, None, arity)
    }

    /// Parse a numerator text and a denominator text (each a rational function; the quotient is taken exactly).
    pub fn parse_ratio(num: &str, den: &str, arity: usize) -> Result<Coefficient, ValuationError> {
        Self::parse_parts(num, Some(den), arity)
    }

    fn parse_parts(
        num: &str,
        den: Option<&str>,
        arity: usize,
    ) -> Result<Coefficient, ValuationError> {
        let vars = variable_map(arity)?;
        let one = |text: &str| -> Result<(ZPoly, ZPoly), ValuationError> {
            let atom = try_parse!(text, default_namespace = "rustred").map_err(|e| {
                ValuationError::NotRational {
                    reason: e.to_string(),
                }
            })?;
            let rp = atom
                .as_view()
                .try_to_rational_polynomial::<_, _, u16>(&Q, &Z, Some(vars.clone()))
                .map_err(|e| {
                    let r = e.to_string();
                    if r.to_lowercase().contains("zero") || r.to_lowercase().contains("division") {
                        ValuationError::ZeroDenominator
                    } else {
                        ValuationError::NotRational { reason: r }
                    }
                })?;
            let extra = &rp.numerator.get_vars_ref()[vars.len()..];
            if !extra.is_empty() {
                let opts = PrintOptions {
                    hide_all_namespaces: true,
                    ..PrintOptions::new()
                };
                let symbols = extra
                    .iter()
                    .map(|v| v.to_atom().printer(opts.clone()).to_string())
                    .collect();
                return Err(ValuationError::NotRationalInD { symbols });
            }
            if rp.denominator.is_zero() {
                return Err(ValuationError::ZeroDenominator);
            }
            Ok((rp.numerator, rp.denominator))
        };
        let (mut n, mut d) = one(num)?;
        if let Some(den) = den {
            let (n2, d2) = one(den)?;
            if n2.is_zero() {
                return Err(ValuationError::ZeroDenominator);
            }
            // (n / d) / (n2 / d2) = (n d2) / (d n2); multiplicities add, no reduction needed for the order
            n = &n * &d2;
            d = &d * &n2;
        }
        Ok(Coefficient {
            num: n,
            den: d,
            arity,
        })
    }

    pub fn arity(&self) -> usize {
        self.arity
    }

    /// Identically zero (as a rational function of all its variables).
    pub fn is_zero(&self) -> bool {
        self.num.is_zero()
    }

    /// Depends on an index variable (a parametric coefficient).
    pub fn depends_on_indices(&self) -> bool {
        (0..self.arity).any(|i| self.num.degree(i) > 0 || self.den.degree(i) > 0)
    }

    /// The order at d0 for generic indices, and the exact loci where it changes (for a coefficient of d alone the loci
    /// are empty and the order is THE order).
    pub fn generic(&self, d0: &D0) -> GenericOrder {
        let dv = self.arity;
        let (dm, drest) = multiplicity(&self.den, dv, d0.value());
        if self.num.is_zero() {
            return GenericOrder {
                order: Valuation::Zero,
                num_multiplicity: 0,
                den_multiplicity: dm,
                den_locus: Locus::of(&drest),
                num_locus: Locus {
                    polynomial: "0".into(),
                    factors: vec![],
                },
            };
        }
        let (nm, nrest) = multiplicity(&self.num, dv, d0.value());
        GenericOrder {
            order: Valuation::Order(nm as i64 - dm as i64),
            num_multiplicity: nm,
            den_multiplicity: dm,
            den_locus: Locus::of(&drest),
            num_locus: Locus::of(&nrest),
        }
    }

    /// The order at d0 AT THE KEY: the key's indices put in exactly (Symbolica `replace` over Z), then the multiplicity of
    /// `(d - d0)` in what remains. The verdict of record for a served key (TRICKS item 9).
    pub fn at_key(&self, key: &[i64], d0: &D0) -> Result<Valuation, ValuationError> {
        if key.len() != self.arity {
            return Err(ValuationError::KeyArity {
                expected: self.arity,
                got: key.len(),
            });
        }
        let put = |p: &ZPoly| -> ZPoly {
            let mut p = p.clone();
            for (i, &k) in key.iter().enumerate() {
                if p.degree(i) > 0 {
                    p = p.replace(i, &Integer::from(k));
                }
            }
            p
        };
        let (n, d) = (put(&self.num), put(&self.den));
        if d.is_zero() {
            return Err(ValuationError::ZeroDenominatorAtKey { key: key.to_vec() });
        }
        if n.is_zero() {
            return Ok(Valuation::Zero);
        }
        let dv = self.arity;
        let (nm, _) = multiplicity(&n, dv, d0.value());
        let (dm, _) = multiplicity(&d, dv, d0.value());
        Ok(Valuation::Order(nm as i64 - dm as i64))
    }

    /// The order of a coefficient of d alone (arity 0, or every index absent). Parametric coefficients must be
    /// evaluated with [`Coefficient::at_key`]: their generic order is not a verdict at a key.
    pub fn order(&self, d0: &D0) -> Result<Valuation, ValuationError> {
        if self.depends_on_indices() {
            let mut symbols: Vec<String> = (0..self.arity)
                .filter(|&i| self.num.degree(i) > 0 || self.den.degree(i) > 0)
                .map(|i| format!("n{i}"))
                .collect();
            symbols.dedup();
            return Err(ValuationError::NotRationalInD { symbols });
        }
        Ok(self.generic(d0).order)
    }
}

/// `v_{d0}` of one coefficient text in d alone (the relation coefficients of a solved key: indices already in).
pub fn order_of_text(text: &str, d0: &D0) -> Result<Valuation, ValuationError> {
    Coefficient::parse(text, 0)?.order(d0)
}

/// `v_{d0}` of numerator text / denominator text, both in d alone.
pub fn order_of_ratio(num: &str, den: &str, d0: &D0) -> Result<Valuation, ValuationError> {
    Coefficient::parse_ratio(num, den, 0)?.order(d0)
}

/// `v_{d0}` of a parametric coefficient text (indices `n0 .. n{arity-1}`) at one key.
pub fn order_at_key(
    text: &str,
    arity: usize,
    key: &[i64],
    d0: &D0,
) -> Result<Valuation, ValuationError> {
    Coefficient::parse(text, arity)?.at_key(key, d0)
}
