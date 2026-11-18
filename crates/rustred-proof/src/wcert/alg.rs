//! Symbolica layer of the weight-certificate checker (L11 SYMBOLICA FIRST): parsing of coefficient texts into exact
//! rational functions over Z, shifts / substitutions of index variables, factorisation, d-coefficients, sign test on a
//! box, Groebner bases and normal forms over Q. Nothing here knows a family, an arity or a node: the variable set is
//! `d, n0 .. n(arity-1)` for the arity read from the stored rule.
use std::collections::BTreeMap;
use std::sync::Arc;

use symbolica::prelude::*;

/// Polynomial over Z in the checker's variables (Lex order, `d` first).
pub type Poly = MultivariatePolynomial<IntegerRing, u16>;
/// Reduced rational function over Z (numerator and denominator coprime, denominator with positive leading coefficient).
pub type Rf = RationalPolynomial<IntegerRing, u16>;
/// Polynomial over Q in grevlex order (Groebner bases).
pub type QPoly =
    MultivariatePolynomial<symbolica::domains::rational::RationalField, u16, GrevLexOrder>;

/// The variables `d, n0, .., n(arity-1)`; index of `d` is 0, of slot `i` is `i + 1`.
pub struct Vars {
    pub arity: usize,
    pub map: Arc<Vec<PolyVariable>>,
    names: Vec<String>,
}

impl Vars {
    pub fn new(arity: usize) -> Self {
        let mut names = vec!["d".to_string()];
        names.extend((0..arity).map(|i| format!("n{i}")));
        let map = Arc::new(
            names
                .iter()
                .map(|n| PolyVariable::Symbol(symbol!(n.as_str())))
                .collect(),
        );
        Vars { arity, map, names }
    }
    pub const D: usize = 0;
    pub fn slot(i: usize) -> usize {
        i + 1
    }
    pub fn name(&self, var: usize) -> &str {
        &self.names[var]
    }

    /// Parse a coefficient text (any Symbolica expression that is a rational function of the variables; `**` is read
    /// as `^`). Errors are texts (unknown variable, not rational, syntax).
    pub fn rf(&self, text: &str) -> Result<Rf, String> {
        let t = if text.contains("**") {
            std::borrow::Cow::Owned(text.replace("**", "^"))
        } else {
            std::borrow::Cow::Borrowed(text)
        };
        let tok = Token::parse(&t, ParseSettings::default()).map_err(|e| format!("parse: {e}"))?;
        let names: Vec<_> = self.names.iter().map(|s| s.as_str().into()).collect();
        tok.to_rational_polynomial::<IntegerRing, IntegerRing, u16>(
            &Z,
            &Z,
            &self.map,
            names.as_slice(),
        )
        .map_err(|e| format!("not a rational function of the index variables and d: {e}"))
    }

    /// Parse a polynomial text (refuses a non-constant denominator).
    pub fn poly(&self, text: &str) -> Result<Poly, String> {
        let r = self.rf(text)?;
        if !r.denominator.is_one() {
            if r.denominator.is_constant() {
                return Err("polynomial text with a rational coefficient".into());
            }
            return Err("polynomial text with a denominator".into());
        }
        Ok(r.numerator)
    }

    pub fn zero_rf(&self) -> Rf {
        RationalPolynomial::new(&Z, self.map.clone())
    }
    pub fn const_rf(&self, c: i64) -> Rf {
        let z = self.zero_rf();
        let num = z.numerator.constant(Integer::from(c));
        Rf::from(num)
    }
    /// The polynomial `c0 + sum c_v x_v` (var indices of this map).
    pub fn affine(&self, c0: i64, lin: &[(usize, i64)]) -> Poly {
        let z = self.zero_rf().numerator;
        let mut p = z.constant(Integer::from(c0));
        for &(v, c) in lin {
            if c != 0 {
                let x = z.variable(&self.map[v]).expect("variable of the map");
                p = &p + &x.mul_coeff(Integer::from(c));
            }
        }
        p
    }
}

/// `p(x + s)` for the given (variable, shift) pairs (zero shifts skipped).
pub fn shift_poly(p: &Poly, shifts: &[(usize, i64)]) -> Poly {
    let s: Vec<(usize, Integer)> = shifts
        .iter()
        .filter(|x| x.1 != 0)
        .map(|&(v, c)| (v, Integer::from(c)))
        .collect();
    if s.is_empty() || p.is_constant() {
        return p.clone();
    }
    let refs: Vec<(usize, &Integer)> = s.iter().map(|(v, c)| (*v, c)).collect();
    p.shift_vars(&refs)
}

/// Rebuild a rational function from a numerator and a denominator that are coprime (shifts and sign flips of the
/// variables are ring automorphisms: they keep coprimality); the denominator sign is normalised.
pub fn rebuild(num: Poly, den: Poly, gcd: bool) -> Rf {
    <Rf as symbolica::domains::rational_polynomial::FromNumeratorAndDenominator<
        IntegerRing,
        IntegerRing,
        u16,
    >>::from_num_den(num, den, &Z, gcd)
}

/// `r(x + s)`.
pub fn shift_rf(r: &Rf, shifts: &[(usize, i64)]) -> Rf {
    if shifts.iter().all(|x| x.1 == 0) {
        return r.clone();
    }
    rebuild(
        shift_poly(&r.numerator, shifts),
        shift_poly(&r.denominator, shifts),
        false,
    )
}

/// Substitute integer values for variables (fixed slots): `r(x_v = c)`; cancels common factors created by it.
/// Err when the denominator vanishes identically under the substitution.
pub fn subst_rf(r: &Rf, values: &[(usize, i64)]) -> Result<Rf, String> {
    if values.is_empty() {
        return Ok(r.clone());
    }
    let num = subst_poly(&r.numerator, values);
    let den = subst_poly(&r.denominator, values);
    if den.is_zero() {
        return Err("denominator vanishes identically under the fixed-slot substitution".into());
    }
    Ok(rebuild(num, den, true))
}

pub fn subst_poly(p: &Poly, values: &[(usize, i64)]) -> Poly {
    let mut q = p.clone();
    for &(v, c) in values {
        if q.degree(v) > 0 {
            q = q.replace(v, &Integer::from(c));
        }
    }
    q
}

/// Substitute polynomials for variables (affine case: dependent index := form), in the given order.
pub fn subst_poly_forms(p: &Poly, forms: &[(usize, Poly)]) -> Poly {
    let mut q = p.clone();
    for (v, f) in forms {
        if q.degree(*v) > 0 {
            q = q.replace_with_poly(*v, f);
        }
    }
    q
}

/// Primitive part with a positive leading coefficient (canonical representative of a factor up to units of Z).
pub fn normalise_factor(p: &Poly) -> Poly {
    let c = p.content();
    let mut q = if c.is_one() {
        p.clone()
    } else {
        p.clone().div_coeff(&c)
    };
    if q.lcoeff().is_negative() {
        q = -q;
    }
    q
}

/// The irreducible non-constant factors of `p` over Z, normalised (multiplicities dropped).
pub fn irreducible_factors(p: &Poly) -> Vec<Poly> {
    if p.is_constant() {
        return vec![];
    }
    let mut out: Vec<Poly> = vec![];
    for (f, _) in p.factor() {
        if f.is_constant() {
            continue;
        }
        let g = normalise_factor(&f);
        if !out.contains(&g) {
            out.push(g);
        }
    }
    out
}

/// Whether `p` contains a variable other than `d`.
pub fn has_index(p: &Poly) -> bool {
    (1..p.nvars()).any(|v| p.degree(v) > 0)
}

/// The coefficients c_i of `p = sum_i c_i d^i` (nonzero ones), as polynomials without `d`.
pub fn d_coefficients(p: &Poly) -> Vec<(u16, Poly)> {
    if p.degree(Vars::D) == 0 {
        return if p.is_zero() {
            vec![]
        } else {
            vec![(0, p.clone())]
        };
    }
    p.to_univariate_polynomial_list(Vars::D)
        .into_iter()
        .filter(|(c, _)| !c.is_zero())
        .map(|(c, e)| (e, c))
        .collect()
}

/// Bounds of one variable on the domain box (inclusive).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bound {
    pub lo: Option<i64>,
    pub hi: Option<i64>,
}

/// (c2) Sign argument: substitute `x_v = lo_v + u_v` (or `hi_v - u_v`), `u_v >= 0`; if the result has a nonzero constant
/// term and every coefficient has the constant's sign, `|p| >= |constant| > 0` on the box. `prefer_upper` chooses the
/// upper bound for two-sided variables. Returns the substituted polynomial's constant on success.
pub fn sign_on_box(
    p: &Poly,
    bounds: &BTreeMap<usize, Bound>,
    prefer_upper: bool,
) -> Option<(Integer, String)> {
    let mut q = p.clone();
    let mut how = vec![];
    for v in 0..p.nvars() {
        if q.degree(v) == 0 {
            continue;
        }
        let b = bounds.get(&v).copied().unwrap_or_default();
        let use_upper = match (b.lo, b.hi) {
            (None, None) => return None,
            (Some(_), None) => false,
            (None, Some(_)) => true,
            (Some(_), Some(_)) => prefer_upper,
        };
        if use_upper {
            // x = hi - u: flip the variable's sign, then shift by -hi: q1(u) = q(-u), q1(u - hi) = q(hi - u)
            let hi = b.hi.unwrap();
            let n = q.nvars();
            for (i, c) in q.coefficients.iter_mut().enumerate() {
                if q.exponents[i * n + v] % 2 == 1 {
                    *c = -c.clone();
                }
            }
            q = shift_poly(&q, &[(v, -hi)]);
            how.push(format!("{}=hi-u", v));
        } else {
            q = shift_poly(&q, &[(v, b.lo.unwrap())]);
            how.push(format!("{}=lo+u", v));
        }
    }
    let n = q.nvars();
    let mut constant: Option<Integer> = None;
    let mut pos = 0usize;
    let mut neg = 0usize;
    for (i, c) in q.coefficients.iter().enumerate() {
        if q.exponents[i * n..(i + 1) * n].iter().all(|&e| e == 0) {
            constant = Some(c.clone());
        }
        if c.is_negative() {
            neg += 1;
        } else {
            pos += 1;
        }
    }
    let c0 = constant?;
    if c0.is_zero() || (c0.is_negative() && pos > 0) || (!c0.is_negative() && neg > 0) {
        return None;
    }
    Some((c0, how.join(",")))
}

/// Convert to Q, grevlex.
pub fn to_q(p: &Poly) -> QPoly {
    p.map_coeff(|c| Rational::from(c), Q)
        .reorder::<GrevLexOrder>()
}

/// Reduced Groebner basis over Q (grevlex) of the given generators.
pub fn groebner(gens: &[Poly]) -> Vec<QPoly> {
    let ideal: Vec<QPoly> = gens.iter().map(to_q).collect();
    GroebnerBasis::new(&ideal, false).system
}

/// Whether the basis is {1} (the generators have no common complex zero).
pub fn is_unit_ideal(g: &[QPoly]) -> bool {
    g.iter().any(|p| p.is_constant() && !p.is_zero())
}

/// Whether `p` lies in the ideal with Groebner basis `g` (normal form zero).
pub fn in_ideal(p: &Poly, g: &[QPoly]) -> bool {
    if p.is_zero() {
        return true;
    }
    if g.is_empty() {
        return false;
    }
    to_q(p).reduce(g).is_zero()
}

/// Truncated text of a printable value (records stay small).
pub fn short(s: String, max: usize) -> String {
    if s.len() <= max {
        s
    } else {
        let mut cut = max;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        format!("{}...[{} chars]", &s[..cut], s.len())
    }
}
