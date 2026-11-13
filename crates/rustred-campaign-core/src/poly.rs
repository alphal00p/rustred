//! Sparse integer polynomials in the key indices `n0..n{N-1}` (and the dimension `d` where a record format has it),
//! parsed from the strings found in rule records, evaluated EXACTLY at integer keys.
//!
//! - Coefficients are `i128`; a coefficient that does not fit is a parse error ([`PolyError::CoefficientOverflow`]),
//!   never a silent wrap.
//! - Evaluation first runs in checked `i128` arithmetic; on any overflow it is repeated in the small arbitrary-precision
//!   integer [`BigInt`] (exact). [`Poly::eval_checked_i128`] exposes the overflow as an explicit error value.
//! - Grammar of the parser: integers, `n<k>` (k < arity), `d` (only if allowed), `+ - * ^ ( )`, and the equation form
//!   `a=b` (read as `a-b`). Division is refused ([`PolyError::Division`]): domain literals are polynomials.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;

/// Variable id of the dimension `d` inside a monomial (index variables use their slot number).
pub const D_VAR: u16 = u16::MAX;

/// A monomial: sorted `(variable, exponent)` pairs, exponents >= 1. The empty monomial is the constant 1.
pub type Mono = Vec<(u16, u32)>;

/// Errors of the parser and of polynomial arithmetic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolyError {
    /// Syntax error at a byte position.
    Syntax { pos: usize, msg: String },
    /// A variable `n<k>` with `k >= arity`.
    VariableOutOfRange { var: usize, arity: usize },
    /// `d` occurs where it is not allowed (domain literals must be d-free).
    ContainsD,
    /// A `/` (rational expression) where a polynomial is required.
    Division,
    /// A coefficient (or an intermediate coefficient of the expansion) does not fit into `i128`.
    CoefficientOverflow,
    /// An exponent above [`MAX_EXPONENT`].
    ExponentTooLarge(u64),
}

impl fmt::Display for PolyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PolyError::Syntax { pos, msg } => write!(f, "syntax error at byte {pos}: {msg}"),
            PolyError::VariableOutOfRange { var, arity } => write!(f, "variable n{var} out of range (arity {arity})"),
            PolyError::ContainsD => write!(f, "the dimension d occurs in a d-free literal"),
            PolyError::Division => write!(f, "division in a polynomial literal"),
            PolyError::CoefficientOverflow => write!(f, "coefficient does not fit into i128"),
            PolyError::ExponentTooLarge(e) => write!(f, "exponent {e} above the limit"),
        }
    }
}
impl std::error::Error for PolyError {}

/// Largest exponent the parser accepts.
pub const MAX_EXPONENT: u64 = 64;

/// Errors of evaluation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EvalError {
    /// The key is shorter than a variable index used by the polynomial.
    KeyTooShort { needed: usize, len: usize },
    /// `d` occurs in a polynomial evaluated as d-free.
    ContainsD,
    /// Only from [`Poly::eval_checked_i128`]: the exact value does not fit `i128` arithmetic.
    Overflow,
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::KeyTooShort { needed, len } => write!(f, "key of length {len}, polynomial needs {needed}"),
            EvalError::ContainsD => write!(f, "polynomial contains d"),
            EvalError::Overflow => write!(f, "i128 overflow"),
        }
    }
}
impl std::error::Error for EvalError {}

// ------------------------------------------------------------------------------------------------------------------
// Small arbitrary-precision integer (exact fallback; add, sub, mul only).
// ------------------------------------------------------------------------------------------------------------------

/// Signed arbitrary-precision integer: sign and little-endian base-2^32 magnitude (no leading zero limbs; zero has
/// an empty magnitude and `neg == false`). Only what exact polynomial evaluation needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BigInt {
    neg: bool,
    mag: Vec<u32>,
}

impl BigInt {
    pub fn zero() -> Self {
        BigInt { neg: false, mag: vec![] }
    }
    pub fn from_i128(v: i128) -> Self {
        let neg = v < 0;
        let mut u = v.unsigned_abs();
        let mut mag = vec![];
        while u > 0 {
            mag.push(u as u32);
            u >>= 32;
        }
        BigInt { neg, mag }
    }
    pub fn is_zero(&self) -> bool {
        self.mag.is_empty()
    }
    /// -1, 0 or 1.
    pub fn signum(&self) -> i32 {
        if self.mag.is_empty() {
            0
        } else if self.neg {
            -1
        } else {
            1
        }
    }
    /// The value if it fits into `i128`.
    pub fn to_i128(&self) -> Option<i128> {
        if self.mag.len() > 4 {
            return None;
        }
        let mut u: u128 = 0;
        for (i, l) in self.mag.iter().enumerate() {
            u |= (*l as u128) << (32 * i);
        }
        if self.neg {
            if u <= i128::MAX as u128 + 1 { Some((u as i128).wrapping_neg()) } else { None }
        } else if u <= i128::MAX as u128 {
            Some(u as i128)
        } else {
            None
        }
    }
    fn trim(&mut self) {
        while self.mag.last() == Some(&0) {
            self.mag.pop();
        }
        if self.mag.is_empty() {
            self.neg = false;
        }
    }
    fn cmp_mag(a: &[u32], b: &[u32]) -> Ordering {
        if a.len() != b.len() {
            return a.len().cmp(&b.len());
        }
        for i in (0..a.len()).rev() {
            if a[i] != b[i] {
                return a[i].cmp(&b[i]);
            }
        }
        Ordering::Equal
    }
    fn add_mag(a: &[u32], b: &[u32]) -> Vec<u32> {
        let mut out = Vec::with_capacity(a.len().max(b.len()) + 1);
        let mut carry = 0u64;
        for i in 0..a.len().max(b.len()) {
            let s = *a.get(i).unwrap_or(&0) as u64 + *b.get(i).unwrap_or(&0) as u64 + carry;
            out.push(s as u32);
            carry = s >> 32;
        }
        if carry > 0 {
            out.push(carry as u32);
        }
        out
    }
    /// a - b with |a| >= |b|.
    fn sub_mag(a: &[u32], b: &[u32]) -> Vec<u32> {
        let mut out = Vec::with_capacity(a.len());
        let mut borrow = 0i64;
        for i in 0..a.len() {
            let mut s = a[i] as i64 - *b.get(i).unwrap_or(&0) as i64 - borrow;
            if s < 0 {
                s += 1 << 32;
                borrow = 1;
            } else {
                borrow = 0;
            }
            out.push(s as u32);
        }
        out
    }
    pub fn add(&self, o: &BigInt) -> BigInt {
        let mut r = if self.neg == o.neg {
            BigInt { neg: self.neg, mag: Self::add_mag(&self.mag, &o.mag) }
        } else {
            match Self::cmp_mag(&self.mag, &o.mag) {
                Ordering::Equal => BigInt::zero(),
                Ordering::Greater => BigInt { neg: self.neg, mag: Self::sub_mag(&self.mag, &o.mag) },
                Ordering::Less => BigInt { neg: o.neg, mag: Self::sub_mag(&o.mag, &self.mag) },
            }
        };
        r.trim();
        r
    }
    pub fn neg(&self) -> BigInt {
        let mut r = self.clone();
        if !r.mag.is_empty() {
            r.neg = !r.neg;
        }
        r
    }
    pub fn sub(&self, o: &BigInt) -> BigInt {
        self.add(&o.neg())
    }
    pub fn mul(&self, o: &BigInt) -> BigInt {
        if self.is_zero() || o.is_zero() {
            return BigInt::zero();
        }
        let mut out = vec![0u32; self.mag.len() + o.mag.len()];
        for (i, &a) in self.mag.iter().enumerate() {
            let mut carry = 0u64;
            for (j, &b) in o.mag.iter().enumerate() {
                let t = out[i + j] as u64 + a as u64 * b as u64 + carry;
                out[i + j] = t as u32;
                carry = t >> 32;
            }
            let mut k = i + o.mag.len();
            while carry > 0 {
                let t = out[k] as u64 + carry;
                out[k] = t as u32;
                carry = t >> 32;
                k += 1;
            }
        }
        let mut r = BigInt { neg: self.neg != o.neg, mag: out };
        r.trim();
        r
    }
}

/// An exact evaluation result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Small(i128),
    Big(BigInt),
}

impl Value {
    pub fn signum(&self) -> i32 {
        match self {
            Value::Small(v) => v.signum() as i32,
            Value::Big(b) => b.signum(),
        }
    }
    pub fn is_zero(&self) -> bool {
        self.signum() == 0
    }
    /// True when the value needed the arbitrary-precision path.
    pub fn is_big(&self) -> bool {
        matches!(self, Value::Big(_))
    }
}

// ------------------------------------------------------------------------------------------------------------------
// Polynomials
// ------------------------------------------------------------------------------------------------------------------

/// A sparse polynomial with `i128` coefficients; terms sorted by monomial, no zero coefficient.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Poly {
    terms: Vec<(Mono, i128)>,
}

fn mono_mul(a: &Mono, b: &Mono) -> Result<Mono, PolyError> {
    let mut out = Vec::with_capacity(a.len() + b.len());
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        if j == b.len() || (i < a.len() && a[i].0 < b[j].0) {
            out.push(a[i]);
            i += 1;
        } else if i == a.len() || b[j].0 < a[i].0 {
            out.push(b[j]);
            j += 1;
        } else {
            let e = a[i].1 as u64 + b[j].1 as u64;
            if e > MAX_EXPONENT * 4 {
                return Err(PolyError::ExponentTooLarge(e));
            }
            out.push((a[i].0, e as u32));
            i += 1;
            j += 1;
        }
    }
    Ok(out)
}

impl Poly {
    pub fn zero() -> Self {
        Poly { terms: vec![] }
    }
    pub fn constant(c: i128) -> Self {
        if c == 0 { Poly::zero() } else { Poly { terms: vec![(vec![], c)] } }
    }
    /// The variable `n<slot>`.
    pub fn var(slot: usize) -> Self {
        Poly { terms: vec![(vec![(slot as u16, 1)], 1)] }
    }
    /// The dimension `d`.
    pub fn d() -> Self {
        Poly { terms: vec![(vec![(D_VAR, 1)], 1)] }
    }
    /// `n<slot> - value`.
    pub fn slot_minus(slot: usize, value: i128) -> Self {
        Poly::var(slot).sub(&Poly::constant(value)).expect("small")
    }
    fn from_map(m: BTreeMap<Mono, i128>) -> Self {
        Poly { terms: m.into_iter().filter(|(_, c)| *c != 0).collect() }
    }
    pub fn terms(&self) -> &[(Mono, i128)] {
        &self.terms
    }
    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }
    /// The constant value if the polynomial has no variable.
    pub fn as_constant(&self) -> Option<i128> {
        match self.terms.as_slice() {
            [] => Some(0),
            [(m, c)] if m.is_empty() => Some(*c),
            _ => None,
        }
    }
    pub fn has_d(&self) -> bool {
        self.terms.iter().any(|(m, _)| m.iter().any(|(v, _)| *v == D_VAR))
    }
    /// Index slots that occur, sorted.
    pub fn support(&self) -> Vec<usize> {
        let mut s: Vec<usize> =
            self.terms.iter().flat_map(|(m, _)| m.iter()).filter(|(v, _)| *v != D_VAR).map(|(v, _)| *v as usize).collect();
        s.sort_unstable();
        s.dedup();
        s
    }
    /// Smallest key length the polynomial can be evaluated at.
    pub fn min_arity(&self) -> usize {
        self.support().last().map_or(0, |s| s + 1)
    }
    /// Total degree (the zero polynomial has degree 0).
    pub fn degree(&self) -> u32 {
        self.terms.iter().map(|(m, _)| m.iter().map(|(_, e)| *e).sum::<u32>()).max().unwrap_or(0)
    }
    pub fn add(&self, o: &Poly) -> Result<Poly, PolyError> {
        let mut m: BTreeMap<Mono, i128> = self.terms.iter().cloned().collect();
        for (mo, c) in &o.terms {
            let e = m.entry(mo.clone()).or_insert(0);
            *e = e.checked_add(*c).ok_or(PolyError::CoefficientOverflow)?;
        }
        Ok(Poly::from_map(m))
    }
    pub fn neg(&self) -> Result<Poly, PolyError> {
        let mut t = Vec::with_capacity(self.terms.len());
        for (m, c) in &self.terms {
            t.push((m.clone(), c.checked_neg().ok_or(PolyError::CoefficientOverflow)?));
        }
        Ok(Poly { terms: t })
    }
    pub fn sub(&self, o: &Poly) -> Result<Poly, PolyError> {
        self.add(&o.neg()?)
    }
    pub fn mul(&self, o: &Poly) -> Result<Poly, PolyError> {
        let mut m: BTreeMap<Mono, i128> = BTreeMap::new();
        for (a, ca) in &self.terms {
            for (b, cb) in &o.terms {
                let c = ca.checked_mul(*cb).ok_or(PolyError::CoefficientOverflow)?;
                let e = m.entry(mono_mul(a, b)?).or_insert(0);
                *e = e.checked_add(c).ok_or(PolyError::CoefficientOverflow)?;
            }
        }
        Ok(Poly::from_map(m))
    }
    pub fn pow(&self, e: u64) -> Result<Poly, PolyError> {
        if e > MAX_EXPONENT {
            return Err(PolyError::ExponentTooLarge(e));
        }
        let mut r = Poly::constant(1);
        for _ in 0..e {
            r = r.mul(self)?;
        }
        Ok(r)
    }

    /// Parse a literal with variables `n0..n{arity-1}`; `allow_d` admits the dimension `d`.
    pub fn parse(s: &str, arity: usize, allow_d: bool) -> Result<Poly, PolyError> {
        let mut p = Parser { s: s.as_bytes(), pos: 0, arity, allow_d };
        let lhs = p.expr()?;
        p.ws();
        let r = if p.peek() == Some(b'=') {
            p.pos += 1;
            let rhs = p.expr()?;
            lhs.sub(&rhs)?
        } else {
            lhs
        };
        p.ws();
        if p.pos != p.s.len() {
            return Err(p.err("trailing input"));
        }
        Ok(r)
    }
    /// Parse a d-free literal (asserts that `d` does not occur: [`PolyError::ContainsD`]).
    pub fn parse_d_free(s: &str, arity: usize) -> Result<Poly, PolyError> {
        Poly::parse(s, arity, false)
    }

    /// Exact value at an integer key (`d`-free polynomials only).
    pub fn eval(&self, key: &[i32]) -> Result<Value, EvalError> {
        match self.eval_checked_i128(key) {
            Ok(v) => Ok(Value::Small(v)),
            Err(EvalError::Overflow) => self.eval_big(key).map(Value::Big),
            Err(e) => Err(e),
        }
    }
    /// Exact zero test at an integer key.
    pub fn is_zero_at(&self, key: &[i32]) -> Result<bool, EvalError> {
        Ok(self.eval(key)?.is_zero())
    }
    /// Value in checked `i128` arithmetic; [`EvalError::Overflow`] when any intermediate does not fit.
    pub fn eval_checked_i128(&self, key: &[i32]) -> Result<i128, EvalError> {
        let mut acc: i128 = 0;
        for (m, c) in &self.terms {
            let mut t: i128 = *c;
            for &(v, e) in m {
                if v == D_VAR {
                    return Err(EvalError::ContainsD);
                }
                let x = *key.get(v as usize).ok_or(EvalError::KeyTooShort { needed: v as usize + 1, len: key.len() })?;
                let p = (x as i128).checked_pow(e).ok_or(EvalError::Overflow)?;
                t = t.checked_mul(p).ok_or(EvalError::Overflow)?;
            }
            acc = acc.checked_add(t).ok_or(EvalError::Overflow)?;
        }
        Ok(acc)
    }
    /// Value in arbitrary precision (always exact).
    pub fn eval_big(&self, key: &[i32]) -> Result<BigInt, EvalError> {
        let mut acc = BigInt::zero();
        for (m, c) in &self.terms {
            let mut t = BigInt::from_i128(*c);
            for &(v, e) in m {
                if v == D_VAR {
                    return Err(EvalError::ContainsD);
                }
                let x = *key.get(v as usize).ok_or(EvalError::KeyTooShort { needed: v as usize + 1, len: key.len() })?;
                let bx = BigInt::from_i128(x as i128);
                for _ in 0..e {
                    t = t.mul(&bx);
                }
            }
            acc = acc.add(&t);
        }
        Ok(acc)
    }

    /// Split by powers of `d`: `P = sum_j c_j(n) d^j`, returned as `(j, c_j)` with every `c_j` d-free and nonzero.
    pub fn d_coefficients(&self) -> Vec<(u32, Poly)> {
        let mut by: BTreeMap<u32, BTreeMap<Mono, i128>> = BTreeMap::new();
        for (m, c) in &self.terms {
            let j = m.iter().find(|(v, _)| *v == D_VAR).map_or(0, |(_, e)| *e);
            let rest: Mono = m.iter().filter(|(v, _)| *v != D_VAR).cloned().collect();
            *by.entry(j).or_default().entry(rest).or_insert(0) += *c; // distinct monomials: no collision
        }
        by.into_iter().map(|(j, m)| (j, Poly::from_map(m))).filter(|(_, p)| !p.is_zero()).collect()
    }

    /// Primitive part with a canonical sign (positive coefficient on the largest monomial): same zero set.
    /// The zero polynomial stays zero; a nonzero constant becomes 1.
    pub fn primitive_normal(&self) -> Poly {
        if self.terms.is_empty() {
            return Poly::zero();
        }
        let mut g: u128 = 0;
        for (_, c) in &self.terms {
            g = gcd(g, c.unsigned_abs());
        }
        let sign: i128 = if self.terms.last().expect("nonzero").1 < 0 { -1 } else { 1 };
        Poly { terms: self.terms.iter().map(|(m, c)| (m.clone(), sign * (*c / g as i128))).collect() }
    }
}

fn gcd(a: u128, b: u128) -> u128 {
    if b == 0 { a } else { gcd(b, a % b) }
}

impl fmt::Display for Poly {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.terms.is_empty() {
            return write!(f, "0");
        }
        for (k, (m, c)) in self.terms.iter().enumerate() {
            let neg = *c < 0;
            let a = c.unsigned_abs();
            if k == 0 {
                if neg {
                    write!(f, "-")?;
                }
            } else {
                write!(f, "{}", if neg { "-" } else { "+" })?;
            }
            let mut first = true;
            if a != 1 || m.is_empty() {
                write!(f, "{a}")?;
                first = false;
            }
            for (v, e) in m {
                if !first {
                    write!(f, "*")?;
                }
                first = false;
                if *v == D_VAR { write!(f, "d")? } else { write!(f, "n{v}")? }
                if *e > 1 {
                    write!(f, "^{e}")?;
                }
            }
        }
        Ok(())
    }
}

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
    arity: usize,
    allow_d: bool,
}

impl Parser<'_> {
    fn err(&self, msg: &str) -> PolyError {
        PolyError::Syntax { pos: self.pos, msg: msg.to_string() }
    }
    fn ws(&mut self) {
        while self.pos < self.s.len() && self.s[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.pos).copied()
    }
    fn expr(&mut self) -> Result<Poly, PolyError> {
        let mut acc = self.term()?;
        loop {
            match self.peek() {
                Some(b'+') => {
                    self.pos += 1;
                    acc = acc.add(&self.term()?)?;
                }
                Some(b'-') => {
                    self.pos += 1;
                    acc = acc.sub(&self.term()?)?;
                }
                _ => return Ok(acc),
            }
        }
    }
    fn term(&mut self) -> Result<Poly, PolyError> {
        let mut acc = self.unary()?;
        loop {
            match self.peek() {
                Some(b'*') => {
                    self.pos += 1;
                    acc = acc.mul(&self.unary()?)?;
                }
                Some(b'/') => return Err(PolyError::Division),
                _ => return Ok(acc),
            }
        }
    }
    fn unary(&mut self) -> Result<Poly, PolyError> {
        match self.peek() {
            Some(b'-') => {
                self.pos += 1;
                self.unary()?.neg()
            }
            Some(b'+') => {
                self.pos += 1;
                self.unary()
            }
            _ => self.power(),
        }
    }
    fn power(&mut self) -> Result<Poly, PolyError> {
        let base = self.atom()?;
        if self.peek() == Some(b'^') {
            self.pos += 1;
            self.ws();
            let e = self.uint()?;
            let e: u64 = e.try_into().map_err(|_| PolyError::ExponentTooLarge(u64::MAX))?;
            return base.pow(e);
        }
        Ok(base)
    }
    fn uint(&mut self) -> Result<u128, PolyError> {
        let start = self.pos;
        let mut v: u128 = 0;
        while self.pos < self.s.len() && self.s[self.pos].is_ascii_digit() {
            v = v
                .checked_mul(10)
                .and_then(|v| v.checked_add((self.s[self.pos] - b'0') as u128))
                .ok_or(PolyError::CoefficientOverflow)?;
            self.pos += 1;
        }
        if self.pos == start {
            return Err(self.err("expected an integer"));
        }
        Ok(v)
    }
    fn atom(&mut self) -> Result<Poly, PolyError> {
        match self.peek() {
            Some(b'(') => {
                self.pos += 1;
                let e = self.expr()?;
                if self.peek() != Some(b')') {
                    return Err(self.err("expected ')'"));
                }
                self.pos += 1;
                Ok(e)
            }
            Some(c) if c.is_ascii_digit() => {
                let v = self.uint()?;
                let v: i128 = v.try_into().map_err(|_| PolyError::CoefficientOverflow)?;
                Ok(Poly::constant(v))
            }
            Some(b'n') => {
                self.pos += 1;
                let start = self.pos;
                let v = self.uint().map_err(|_| PolyError::Syntax { pos: start, msg: "expected n<index>".into() })?;
                let v = v as usize;
                if v >= self.arity {
                    return Err(PolyError::VariableOutOfRange { var: v, arity: self.arity });
                }
                Ok(Poly::var(v))
            }
            Some(b'd') => {
                self.pos += 1;
                if self.s.get(self.pos).is_some_and(|c| c.is_ascii_alphanumeric()) {
                    return Err(self.err("unknown identifier"));
                }
                if !self.allow_d {
                    return Err(PolyError::ContainsD);
                }
                Ok(Poly::d())
            }
            Some(b'/') => Err(PolyError::Division),
            _ => Err(self.err("expected a number, a variable or '('")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bigint_basics() {
        let a = BigInt::from_i128(i128::MAX);
        let b = a.mul(&a);
        assert_eq!(b.signum(), 1);
        assert_eq!(b.to_i128(), None);
        let c = b.sub(&b);
        assert!(c.is_zero());
        assert_eq!(BigInt::from_i128(-5).add(&BigInt::from_i128(3)).to_i128(), Some(-2));
        assert_eq!(BigInt::from_i128(i128::MIN).to_i128(), Some(i128::MIN));
        assert_eq!(BigInt::from_i128(-7).mul(&BigInt::from_i128(6)).to_i128(), Some(-42));
    }
}
