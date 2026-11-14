//! Sparse integer polynomials in the key indices `n0..n{N-1}` (and the dimension `d` where a record format has it),
//! parsed from the strings found in rule records, evaluated EXACTLY at integer keys.
//!
//! - Coefficients are `i128`; a coefficient that does not fit is a parse error ([`PolyError::CoefficientOverflow`]),
//!   never a silent wrap, for the STRICT parsers ([`Poly::parse`], [`Poly::parse_with`], [`Poly::parse_d_free`]) and
//!   for the checked arithmetic ([`Poly::add`], [`Poly::mul`], ...).
//! - EXACT literals (lane import3, 2026-11-14): the exact parsers ([`Poly::parse_exact`], [`Poly::parse_exact_with`],
//!   [`Poly::parse_d_free_exact`]) accept coefficients of ANY size. A literal whose coefficients all fit is the same
//!   `i128` polynomial the strict parser returns; otherwise it is a BIG polynomial ([`Poly::is_big`]): every term kept
//!   exactly in [`BigInt`]. Big polynomials are evaluated exactly ([`Poly::eval`], [`Poly::eval_big`],
//!   [`Poly::is_zero_at`], [`Poly::d_coefficients`]) and printed exactly; the checked arithmetic refuses them
//!   ([`PolyError::CoefficientOverflow`], as before for any overflow) and [`Poly::terms`] must not be called on them
//!   (use [`Poly::small_terms`] / [`Poly::exact_terms`]). Used for the DOMAIN literals of rule records (equations,
//!   exceptions, pole loci): dispatch needs their exact zero sets, so a large literal is evaluated, never truncated
//!   and never a reason to refuse a record. (numerica's `Integer` cannot be linked here: verification in
//!   notes/implementer.import3.md, "SYMBOLICA / OWN ALGEBRA".)
//! - Evaluation first runs in checked `i128` arithmetic; on any overflow it is repeated in the small arbitrary-precision
//!   integer [`BigInt`] (exact). [`Poly::eval_checked_i128`] exposes the overflow as an explicit error value.
//! - Grammar of the STRICT parser ([`Poly::parse`], the default for every record field): integers, `n<k>` (k < arity,
//!   no leading zero: `n01` is refused as the engine's `Expr::parse` refuses it), `d` (only if allowed),
//!   `+ - * ^ ( )`. Division is refused ([`PolyError::Division`]): domain literals are polynomials.
//! - The equation form `a=b` (read as `a-b`) is NOT accepted by the engine's parser and is refused by default; it is
//!   accepted only through the explicit lenient entry point [`Poly::parse_with`] with
//!   [`ParseOptions::allow_equation`] (used for the checker's locus files, e.g. `"n0 + n1 - n4 - 1 = 0"`).
//! - Zero tests: the crate is exact. The engine's zero test is modulo the two 61-bit primes [`ENGINE_PRIMES`]; a
//!   nonzero value divisible by both is "zero" for the engine. [`Value::engine_zero_disagrees`] detects that case so
//!   that the domain module can fail closed instead of disagreeing silently (audit A N1).

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;

/// Variable id of the dimension `d` inside a monomial (index variables use their slot number).
pub const D_VAR: u16 = u16::MAX;

/// Largest arity a polynomial can address: index variables must stay below [`D_VAR`] (audit A N4).
pub const MAX_ARITY: usize = D_VAR as usize;

/// The two 61-bit primes of the engine's exact zero test (`rustred-core/src/node/replay/arith.rs` `P`, `P2`;
/// `Expr::is_zero_at` calls a value zero iff it vanishes modulo both).
pub const ENGINE_PRIMES: [u64; 2] = [2_305_843_009_213_693_951, 2_305_843_009_213_693_921];

/// Options of [`Poly::parse_with`]. The default is the strict grammar without `d`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParseOptions {
    /// Admit the dimension `d`.
    pub allow_d: bool,
    /// LENIENT: admit one top-level `a=b`, read as `a-b`. The engine's parser refuses `=`; use this only for sources
    /// whose documented format is an equation (the checker's locus files).
    pub allow_equation: bool,
}

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
            PolyError::VariableOutOfRange { var, arity } => {
                write!(f, "variable n{var} out of range (arity {arity})")
            }
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
    /// From [`Poly::eval_checked_i128`]: the exact value does not fit `i128` arithmetic; from the domain module: the
    /// rule point of a target pattern does not fit `i32`.
    Overflow,
    /// The exact value is nonzero but vanishes modulo both [`ENGINE_PRIMES`]: the engine would read it as zero.
    EngineZeroTest,
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::KeyTooShort { needed, len } => {
                write!(f, "key of length {len}, polynomial needs {needed}")
            }
            EvalError::ContainsD => write!(f, "polynomial contains d"),
            EvalError::Overflow => write!(f, "overflow"),
            EvalError::EngineZeroTest => {
                write!(
                    f,
                    "nonzero value that the engine's modular zero test reads as zero"
                )
            }
        }
    }
}
impl std::error::Error for EvalError {}

// ------------------------------------------------------------------------------------------------------------------
// Small arbitrary-precision integer (exact fallback; add, sub, mul, decimal text).
// ------------------------------------------------------------------------------------------------------------------

/// Signed arbitrary-precision integer: sign and little-endian base-2^32 magnitude (no leading zero limbs; zero has
/// an empty magnitude and `neg == false`). Only what exact polynomial evaluation and exact literals need. The
/// representation is canonical, so the derived `Hash` agrees with equality; the order is NUMERIC.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BigInt {
    neg: bool,
    mag: Vec<u32>,
}

impl BigInt {
    pub fn zero() -> Self {
        BigInt {
            neg: false,
            mag: vec![],
        }
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
            if u <= i128::MAX as u128 + 1 {
                Some((u as i128).wrapping_neg())
            } else {
                None
            }
        } else if u <= i128::MAX as u128 {
            Some(u as i128)
        } else {
            None
        }
    }
    /// The value modulo `p` (in `0..p`).
    pub fn rem_u64(&self, p: u64) -> u64 {
        let mut r: u128 = 0;
        for &l in self.mag.iter().rev() {
            r = ((r << 32) | l as u128) % p as u128;
        }
        let r = r as u64;
        if self.neg && r != 0 { p - r } else { r }
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
        for (i, &ai) in a.iter().enumerate() {
            let mut s = ai as i64 - *b.get(i).unwrap_or(&0) as i64 - borrow;
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
            BigInt {
                neg: self.neg,
                mag: Self::add_mag(&self.mag, &o.mag),
            }
        } else {
            match Self::cmp_mag(&self.mag, &o.mag) {
                Ordering::Equal => BigInt::zero(),
                Ordering::Greater => BigInt {
                    neg: self.neg,
                    mag: Self::sub_mag(&self.mag, &o.mag),
                },
                Ordering::Less => BigInt {
                    neg: o.neg,
                    mag: Self::sub_mag(&o.mag, &self.mag),
                },
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
    /// Parse an unsigned decimal digit string (ASCII digits only, at least one). `None` otherwise.
    pub fn from_decimal(s: &str) -> Option<BigInt> {
        if s.is_empty() || !s.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let mut r = BigInt::zero();
        for chunk in s.as_bytes().chunks(9) {
            let mut m: u32 = 1;
            let mut a: u32 = 0;
            for &c in chunk {
                m *= 10;
                a = a * 10 + (c - b'0') as u32;
            }
            r.mul_small_add(m, a);
        }
        r.trim();
        Some(r)
    }
    /// `self = self * m + a` on the magnitude (sign unchanged; `self` non-negative in the callers).
    fn mul_small_add(&mut self, m: u32, a: u32) {
        let mut carry = a as u64;
        for l in self.mag.iter_mut() {
            let t = *l as u64 * m as u64 + carry;
            *l = t as u32;
            carry = t >> 32;
        }
        if carry > 0 {
            self.mag.push(carry as u32);
        }
    }
    /// Magnitude divided by a nonzero `d`: (quotient magnitude, remainder).
    fn divrem_mag_small(mag: &[u32], d: u32) -> (Vec<u32>, u32) {
        let mut q = vec![0u32; mag.len()];
        let mut r: u64 = 0;
        for i in (0..mag.len()).rev() {
            let cur = (r << 32) | mag[i] as u64;
            q[i] = (cur / d as u64) as u32;
            r = cur % d as u64;
        }
        while q.last() == Some(&0) {
            q.pop();
        }
        (q, r as u32)
    }
    /// Exact decimal text (`-` for negative values).
    pub fn to_decimal(&self) -> String {
        if self.mag.is_empty() {
            return "0".into();
        }
        let mut chunks: Vec<u32> = vec![];
        let mut m = self.mag.clone();
        while !m.is_empty() {
            let (q, r) = Self::divrem_mag_small(&m, 1_000_000_000);
            chunks.push(r);
            m = q;
        }
        let mut s = String::new();
        if self.neg {
            s.push('-');
        }
        s.push_str(&chunks.last().expect("nonzero").to_string());
        for c in chunks.iter().rev().skip(1) {
            s.push_str(&format!("{c:09}"));
        }
        s
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
        let mut r = BigInt {
            neg: self.neg != o.neg,
            mag: out,
        };
        r.trim();
        r
    }
}

impl Ord for BigInt {
    fn cmp(&self, o: &Self) -> Ordering {
        match (self.signum(), o.signum()) {
            (a, b) if a != b => a.cmp(&b),
            (0, _) => Ordering::Equal,
            (1, _) => Self::cmp_mag(&self.mag, &o.mag),
            _ => Self::cmp_mag(&o.mag, &self.mag),
        }
    }
}
impl PartialOrd for BigInt {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl fmt::Display for BigInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_decimal())
    }
}

/// An exact evaluation result. Equality is NUMERIC (audit A N2): `Small(0) == Big(0)`, and a `Big` that fits
/// `i128` equals the `Small` of the same value.
#[derive(Clone, Debug)]
pub enum Value {
    Small(i128),
    Big(BigInt),
}

impl PartialEq for Value {
    fn eq(&self, o: &Value) -> bool {
        match (self, o) {
            (Value::Small(a), Value::Small(b)) => a == b,
            (Value::Big(a), Value::Big(b)) => a == b, // BigInt is normalised (trimmed, zero non-negative)
            (Value::Small(a), Value::Big(b)) | (Value::Big(b), Value::Small(a)) => {
                b.to_i128() == Some(*a)
            }
        }
    }
}
impl Eq for Value {}

impl Value {
    /// The value modulo `p` (in `0..p`).
    pub fn rem_u64(&self, p: u64) -> u64 {
        match self {
            Value::Small(v) => v.rem_euclid(p as i128) as u64,
            Value::Big(b) => b.rem_u64(p),
        }
    }
    /// True when the value is NONZERO but vanishes modulo both [`ENGINE_PRIMES`]: the engine's zero test would call
    /// it zero (audit A N1). Callers fail closed on it.
    pub fn engine_zero_disagrees(&self) -> bool {
        !self.is_zero() && ENGINE_PRIMES.iter().all(|&p| self.rem_u64(p) == 0)
    }
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

/// A sparse polynomial with `i128` coefficients; terms sorted by monomial, no zero coefficient. A BIG polynomial (only
/// from the exact parsers, [`Poly::is_big`]) keeps ALL its terms in `big` (and none in `terms`): it has at least one
/// coefficient beyond `i128` (a literal whose coefficients all fit is always the `i128` form: the representation is
/// canonical, so the derived equality / hash / order are structural and consistent).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Poly {
    terms: Vec<(Mono, i128)>,
    big: Option<Box<Vec<(Mono, BigInt)>>>,
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
        Poly {
            terms: vec![],
            big: None,
        }
    }
    fn small(terms: Vec<(Mono, i128)>) -> Self {
        Poly { terms, big: None }
    }
    pub fn constant(c: i128) -> Self {
        if c == 0 {
            Poly::zero()
        } else {
            Poly::small(vec![(vec![], c)])
        }
    }
    /// The variable `n<slot>`. Panics if `slot >= MAX_ARITY` (it would collide with `d`).
    pub fn var(slot: usize) -> Self {
        assert!(slot < MAX_ARITY, "variable n{slot} collides with d");
        Poly::small(vec![(vec![(slot as u16, 1)], 1)])
    }
    /// The dimension `d`.
    pub fn d() -> Self {
        Poly::small(vec![(vec![(D_VAR, 1)], 1)])
    }
    /// `n<slot> - value`.
    pub fn slot_minus(slot: usize, value: i128) -> Self {
        Poly::var(slot).sub(&Poly::constant(value)).expect("small")
    }
    fn from_map(m: BTreeMap<Mono, i128>) -> Self {
        Poly::small(m.into_iter().filter(|(_, c)| *c != 0).collect())
    }
    /// Canonical form of an exact term map: zero coefficients dropped; the `i128` form iff every coefficient fits.
    fn from_big_map(m: BTreeMap<Mono, BigInt>) -> Self {
        let t: Vec<(Mono, BigInt)> = m.into_iter().filter(|(_, c)| !c.is_zero()).collect();
        let small: Option<Vec<(Mono, i128)>> = t
            .iter()
            .map(|(m, c)| c.to_i128().map(|c| (m.clone(), c)))
            .collect();
        match small {
            Some(terms) => Poly::small(terms),
            None => Poly {
                terms: vec![],
                big: Some(Box::new(t)),
            },
        }
    }
    /// The `i128` terms. PANICS on a big polynomial ([`Poly::is_big`]; only the exact parsers make them): code that may
    /// see exact domain literals uses [`Poly::small_terms`] or [`Poly::exact_terms`].
    #[track_caller]
    pub fn terms(&self) -> &[(Mono, i128)] {
        assert!(
            self.big.is_none(),
            "Poly::terms on a polynomial with a coefficient beyond i128 (use small_terms / exact_terms)"
        );
        &self.terms
    }
    /// The `i128` terms, `None` for a big polynomial.
    pub fn small_terms(&self) -> Option<&[(Mono, i128)]> {
        self.big.is_none().then_some(self.terms.as_slice())
    }
    /// Every term with its exact coefficient (both forms).
    pub fn exact_terms(&self) -> Vec<(Mono, BigInt)> {
        match &self.big {
            Some(b) => b.as_ref().clone(),
            None => self
                .terms
                .iter()
                .map(|(m, c)| (m.clone(), BigInt::from_i128(*c)))
                .collect(),
        }
    }
    /// True iff some coefficient does not fit `i128` (only literals read by the exact parsers).
    pub fn is_big(&self) -> bool {
        self.big.is_some()
    }
    /// Number of terms (both forms).
    pub fn nterms(&self) -> usize {
        self.big.as_ref().map_or(self.terms.len(), |b| b.len())
    }
    /// Monomials of both forms.
    fn monos(&self) -> Box<dyn Iterator<Item = &Mono> + '_> {
        match &self.big {
            Some(b) => Box::new(b.iter().map(|(m, _)| m)),
            None => Box::new(self.terms.iter().map(|(m, _)| m)),
        }
    }
    pub fn is_zero(&self) -> bool {
        self.terms.is_empty() && self.big.is_none()
    }
    /// The constant value if the polynomial has no variable and its value fits `i128` (a big constant, which is
    /// nonzero, gives `None`).
    pub fn as_constant(&self) -> Option<i128> {
        if self.big.is_some() {
            return None;
        }
        match self.terms.as_slice() {
            [] => Some(0),
            [(m, c)] if m.is_empty() => Some(*c),
            _ => None,
        }
    }
    pub fn has_d(&self) -> bool {
        self.monos().any(|m| m.iter().any(|(v, _)| *v == D_VAR))
    }
    /// Index slots that occur, sorted.
    pub fn support(&self) -> Vec<usize> {
        let mut s: Vec<usize> = self
            .monos()
            .flat_map(|m| m.iter())
            .filter(|(v, _)| *v != D_VAR)
            .map(|(v, _)| *v as usize)
            .collect();
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
        self.monos()
            .map(|m| m.iter().map(|(_, e)| *e).sum::<u32>())
            .max()
            .unwrap_or(0)
    }
    /// Checked `i128` arithmetic (as every arithmetic method): a big operand is [`PolyError::CoefficientOverflow`].
    pub fn add(&self, o: &Poly) -> Result<Poly, PolyError> {
        if self.is_big() || o.is_big() {
            return Err(PolyError::CoefficientOverflow);
        }
        let mut m: BTreeMap<Mono, i128> = self.terms.iter().cloned().collect();
        for (mo, c) in &o.terms {
            let e = m.entry(mo.clone()).or_insert(0);
            *e = e.checked_add(*c).ok_or(PolyError::CoefficientOverflow)?;
        }
        Ok(Poly::from_map(m))
    }
    pub fn neg(&self) -> Result<Poly, PolyError> {
        if self.is_big() {
            return Err(PolyError::CoefficientOverflow);
        }
        let mut t = Vec::with_capacity(self.terms.len());
        for (m, c) in &self.terms {
            t.push((
                m.clone(),
                c.checked_neg().ok_or(PolyError::CoefficientOverflow)?,
            ));
        }
        Ok(Poly::small(t))
    }
    pub fn sub(&self, o: &Poly) -> Result<Poly, PolyError> {
        self.add(&o.neg()?)
    }
    pub fn mul(&self, o: &Poly) -> Result<Poly, PolyError> {
        if self.is_big() || o.is_big() {
            return Err(PolyError::CoefficientOverflow);
        }
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

    // Exact arithmetic of the exact parser (private): the checked i128 path first, the BigInt path on overflow or
    // for a big operand; results canonical.
    fn big_map(&self) -> BTreeMap<Mono, BigInt> {
        self.exact_terms().into_iter().collect()
    }
    fn add_exact(&self, o: &Poly) -> Poly {
        if let Ok(r) = self.add(o) {
            return r;
        }
        let mut m = self.big_map();
        for (mo, c) in o.exact_terms() {
            let e = m.entry(mo).or_insert_with(BigInt::zero);
            *e = e.add(&c);
        }
        Poly::from_big_map(m)
    }
    fn neg_exact(&self) -> Poly {
        if let Ok(r) = self.neg() {
            return r;
        }
        Poly::from_big_map(
            self.exact_terms()
                .into_iter()
                .map(|(m, c)| (m, c.neg()))
                .collect(),
        )
    }
    fn mul_exact(&self, o: &Poly) -> Result<Poly, PolyError> {
        match self.mul(o) {
            Ok(r) => return Ok(r),
            Err(PolyError::CoefficientOverflow) => {}
            Err(e) => return Err(e),
        }
        let (a, b) = (self.exact_terms(), o.exact_terms());
        let mut m: BTreeMap<Mono, BigInt> = BTreeMap::new();
        for (ma, ca) in &a {
            for (mb, cb) in &b {
                let e = m.entry(mono_mul(ma, mb)?).or_insert_with(BigInt::zero);
                *e = e.add(&ca.mul(cb));
            }
        }
        Ok(Poly::from_big_map(m))
    }
    fn pow_exact(&self, e: u64) -> Result<Poly, PolyError> {
        if e > MAX_EXPONENT {
            return Err(PolyError::ExponentTooLarge(e));
        }
        let mut r = Poly::constant(1);
        for _ in 0..e {
            r = r.mul_exact(self)?;
        }
        Ok(r)
    }

    /// Parse a literal with variables `n0..n{arity-1}` with the STRICT grammar (no `=`); `allow_d` admits the
    /// dimension `d`.
    pub fn parse(s: &str, arity: usize, allow_d: bool) -> Result<Poly, PolyError> {
        Poly::parse_with(
            s,
            arity,
            ParseOptions {
                allow_d,
                allow_equation: false,
            },
        )
    }
    /// Parse with explicit options; [`ParseOptions::allow_equation`] is the only lenient form.
    pub fn parse_with(s: &str, arity: usize, opts: ParseOptions) -> Result<Poly, PolyError> {
        Poly::parse_mode(s, arity, opts, false)
    }
    /// EXACT parse (coefficients of any size; module doc): the strict grammar of [`Poly::parse`]; a literal whose
    /// coefficients all fit is the `i128` polynomial the strict parser returns, else a big polynomial.
    pub fn parse_exact(s: &str, arity: usize, allow_d: bool) -> Result<Poly, PolyError> {
        Poly::parse_exact_with(
            s,
            arity,
            ParseOptions {
                allow_d,
                allow_equation: false,
            },
        )
    }
    /// [`Poly::parse_with`] with exact coefficients: the fast checked-`i128` parse first, the exact parse only when
    /// it overflows (same grammar, same errors otherwise).
    pub fn parse_exact_with(s: &str, arity: usize, opts: ParseOptions) -> Result<Poly, PolyError> {
        match Poly::parse_mode(s, arity, opts, false) {
            Err(PolyError::CoefficientOverflow) => Poly::parse_mode(s, arity, opts, true),
            r => r,
        }
    }
    /// Exact d-free literal ([`Poly::parse_exact`] without `d`): the form of every DOMAIN literal.
    pub fn parse_d_free_exact(s: &str, arity: usize) -> Result<Poly, PolyError> {
        Poly::parse_exact(s, arity, false)
    }
    fn parse_mode(
        s: &str,
        arity: usize,
        opts: ParseOptions,
        exact: bool,
    ) -> Result<Poly, PolyError> {
        let mut p = Parser {
            s: s.as_bytes(),
            pos: 0,
            arity,
            allow_d: opts.allow_d,
            depth: 0,
            exact,
        };
        let lhs = p.expr()?;
        p.ws();
        let r = if opts.allow_equation && p.peek() == Some(b'=') {
            p.pos += 1;
            let rhs = p.expr()?;
            p.sub(&lhs, &rhs)?
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
        if self.is_big() {
            // the coefficients themselves do not fit: the d / key-length errors first, then Overflow
            return self.eval_big(key).and(Err(EvalError::Overflow));
        }
        let mut acc: i128 = 0;
        for (m, c) in &self.terms {
            let mut t: i128 = *c;
            for &(v, e) in m {
                if v == D_VAR {
                    return Err(EvalError::ContainsD);
                }
                let x = *key.get(v as usize).ok_or(EvalError::KeyTooShort {
                    needed: v as usize + 1,
                    len: key.len(),
                })?;
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
        for (m, c) in self.exact_terms() {
            let mut t = c;
            for &(v, e) in &m {
                if v == D_VAR {
                    return Err(EvalError::ContainsD);
                }
                let x = *key.get(v as usize).ok_or(EvalError::KeyTooShort {
                    needed: v as usize + 1,
                    len: key.len(),
                })?;
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
        if self.is_big() {
            let mut by: BTreeMap<u32, BTreeMap<Mono, BigInt>> = BTreeMap::new();
            for (m, c) in self.exact_terms() {
                let j = m.iter().find(|(v, _)| *v == D_VAR).map_or(0, |(_, e)| *e);
                let rest: Mono = m.iter().filter(|(v, _)| *v != D_VAR).cloned().collect();
                by.entry(j).or_default().insert(rest, c); // distinct monomials: no collision
            }
            return by
                .into_iter()
                .map(|(j, m)| (j, Poly::from_big_map(m)))
                .filter(|(_, p)| !p.is_zero())
                .collect();
        }
        let mut by: BTreeMap<u32, BTreeMap<Mono, i128>> = BTreeMap::new();
        for (m, c) in &self.terms {
            let j = m.iter().find(|(v, _)| *v == D_VAR).map_or(0, |(_, e)| *e);
            let rest: Mono = m.iter().filter(|(v, _)| *v != D_VAR).cloned().collect();
            *by.entry(j).or_default().entry(rest).or_insert(0) += *c; // distinct monomials: no collision
        }
        by.into_iter()
            .map(|(j, m)| (j, Poly::from_map(m)))
            .filter(|(_, p)| !p.is_zero())
            .collect()
    }

    /// Primitive part with a canonical sign (positive coefficient on the largest monomial): same zero set.
    /// The zero polynomial stays zero; a nonzero constant becomes 1. Arithmetic is checked (audit A m3): if the
    /// normal form does not fit `i128` (a coefficient `i128::MIN` with gcd 1 to be negated) the polynomial is returned
    /// unchanged, which has the same zero set; only de-duplication can be less effective, never a verdict. A big
    /// polynomial is returned unchanged for the same reason (no content division in [`BigInt`]).
    pub fn primitive_normal(&self) -> Poly {
        if self.is_big() {
            return self.clone();
        }
        if self.terms.is_empty() {
            return Poly::zero();
        }
        let mut g: u128 = 0;
        for (_, c) in &self.terms {
            g = gcd(g, c.unsigned_abs());
        }
        let negate = self.terms.last().expect("nonzero").1 < 0;
        let mut terms = Vec::with_capacity(self.terms.len());
        for (m, c) in &self.terms {
            // g >= 1 divides |c|; g may be 2^127 only when every coefficient is i128::MIN
            let q = if g > i128::MAX as u128 {
                c.signum()
            } else {
                *c / g as i128
            };
            let q = if negate { q.checked_neg() } else { Some(q) };
            match q {
                Some(q) => terms.push((m.clone(), q)),
                None => return self.clone(),
            }
        }
        Poly::small(terms)
    }
}

fn gcd(a: u128, b: u128) -> u128 {
    if b == 0 { a } else { gcd(b, a % b) }
}

impl fmt::Display for Poly {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_zero() {
            return write!(f, "0");
        }
        for (k, (m, c)) in self.exact_terms().iter().enumerate() {
            let neg = c.signum() < 0;
            let a = if neg { c.neg() } else { c.clone() };
            let one = a.to_i128() == Some(1);
            if k == 0 {
                if neg {
                    write!(f, "-")?;
                }
            } else {
                write!(f, "{}", if neg { "-" } else { "+" })?;
            }
            let mut first = true;
            if !one || m.is_empty() {
                write!(f, "{a}")?;
                first = false;
            }
            for (v, e) in m {
                if !first {
                    write!(f, "*")?;
                }
                first = false;
                if *v == D_VAR {
                    write!(f, "d")?
                } else {
                    write!(f, "n{v}")?
                }
                if *e > 1 {
                    write!(f, "^{e}")?;
                }
            }
        }
        Ok(())
    }
}

/// Deepest nesting of parentheses / unary signs the parser accepts (audit B m3: 30,000 levels overflowed the stack).
pub const MAX_NESTING: usize = 256;

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
    arity: usize,
    allow_d: bool,
    depth: usize,
    /// Exact coefficients (BigInt on overflow) instead of checked i128.
    exact: bool,
}

impl Parser<'_> {
    fn add(&self, a: &Poly, b: &Poly) -> Result<Poly, PolyError> {
        if self.exact {
            Ok(a.add_exact(b))
        } else {
            a.add(b)
        }
    }
    fn sub(&self, a: &Poly, b: &Poly) -> Result<Poly, PolyError> {
        if self.exact {
            Ok(a.add_exact(&b.neg_exact()))
        } else {
            a.sub(b)
        }
    }
    fn mul(&self, a: &Poly, b: &Poly) -> Result<Poly, PolyError> {
        if self.exact { a.mul_exact(b) } else { a.mul(b) }
    }
    fn neg(&self, a: &Poly) -> Result<Poly, PolyError> {
        if self.exact {
            Ok(a.neg_exact())
        } else {
            a.neg()
        }
    }
    fn pow(&self, a: &Poly, e: u64) -> Result<Poly, PolyError> {
        if self.exact { a.pow_exact(e) } else { a.pow(e) }
    }
    fn err(&self, msg: &str) -> PolyError {
        PolyError::Syntax {
            pos: self.pos,
            msg: msg.to_string(),
        }
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
                    let t = self.term()?;
                    acc = self.add(&acc, &t)?;
                }
                Some(b'-') => {
                    self.pos += 1;
                    let t = self.term()?;
                    acc = self.sub(&acc, &t)?;
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
                    let u = self.unary()?;
                    acc = self.mul(&acc, &u)?;
                }
                Some(b'/') => return Err(PolyError::Division),
                _ => return Ok(acc),
            }
        }
    }
    fn unary(&mut self) -> Result<Poly, PolyError> {
        self.depth += 1;
        if self.depth > MAX_NESTING {
            return Err(self.err("nesting too deep"));
        }
        let r = self.unary_inner();
        self.depth -= 1;
        r
    }
    fn unary_inner(&mut self) -> Result<Poly, PolyError> {
        match self.peek() {
            Some(b'-') => {
                self.pos += 1;
                let u = self.unary()?;
                self.neg(&u)
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
            let e: u64 = e
                .try_into()
                .map_err(|_| PolyError::ExponentTooLarge(u64::MAX))?;
            return self.pow(&base, e);
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
            Some(c) if c.is_ascii_digit() && self.exact => {
                let start = self.pos;
                while self.pos < self.s.len() && self.s[self.pos].is_ascii_digit() {
                    self.pos += 1;
                }
                let digits = std::str::from_utf8(&self.s[start..self.pos]).expect("ascii digits");
                let v = BigInt::from_decimal(digits).expect("ascii digits");
                Ok(Poly::from_big_map(BTreeMap::from([(vec![], v)])))
            }
            Some(c) if c.is_ascii_digit() => {
                let v = self.uint()?;
                let v: i128 = v.try_into().map_err(|_| PolyError::CoefficientOverflow)?;
                Ok(Poly::constant(v))
            }
            Some(b'n') => {
                self.pos += 1;
                let start = self.pos;
                let v = self.uint().map_err(|_| PolyError::Syntax {
                    pos: start,
                    msg: "expected n<index>".into(),
                })?;
                // the engine refuses a leading zero (`n01`, `n00`); `n0` itself is fine
                if self.pos - start > 1 && self.s[start] == b'0' {
                    return Err(PolyError::Syntax {
                        pos: start,
                        msg: "leading zero in a variable index".into(),
                    });
                }
                // audit A m1: never truncate (an index of 2^64 used to wrap to n0)
                let v = usize::try_from(v).unwrap_or(usize::MAX);
                if v >= self.arity || v >= MAX_ARITY {
                    return Err(PolyError::VariableOutOfRange {
                        var: v,
                        arity: self.arity,
                    });
                }
                Ok(Poly::var(v))
            }
            Some(b'd') => {
                self.pos += 1;
                if self
                    .s
                    .get(self.pos)
                    .is_some_and(|c| c.is_ascii_alphanumeric())
                {
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
        assert_eq!(
            BigInt::from_i128(-5).add(&BigInt::from_i128(3)).to_i128(),
            Some(-2)
        );
        assert_eq!(BigInt::from_i128(i128::MIN).to_i128(), Some(i128::MIN));
        assert_eq!(
            BigInt::from_i128(-7).mul(&BigInt::from_i128(6)).to_i128(),
            Some(-42)
        );
    }

    #[test]
    fn bigint_decimal_and_order() {
        for t in [
            "0",
            "7",
            "1000000000",
            "999999999999999999",
            "170141183460469231731687303715884105728",
            "714237321763339320822816811060387425213038592",
        ] {
            assert_eq!(BigInt::from_decimal(t).unwrap().to_decimal(), t);
        }
        assert_eq!(BigInt::from_decimal("000123").unwrap().to_decimal(), "123");
        assert!(BigInt::from_decimal("").is_none() && BigInt::from_decimal("1a").is_none());
        let a = BigInt::from_decimal("170141183460469231731687303715884105728").unwrap(); // 2^127
        assert_eq!(a.to_i128(), None);
        assert_eq!(a.neg().to_i128(), Some(i128::MIN));
        let (m, z, p) = (a.neg(), BigInt::zero(), BigInt::from_i128(5));
        assert!(m < z && z < p && p < a && m.mul(&p) < m);
        assert_eq!(m.to_decimal(), "-170141183460469231731687303715884105728");
    }

    #[test]
    fn exact_literals_beyond_i128() {
        let big = "714237321763339320822816811060387425213038592"; // a T266 exception coefficient (45 digits)
        let lit = format!("{big}*n1^2 - 3*n0 + 1");
        assert_eq!(
            Poly::parse(&lit, 2, false),
            Err(PolyError::CoefficientOverflow)
        );
        let p = Poly::parse_exact(&lit, 2, false).unwrap();
        assert!(p.is_big() && p.small_terms().is_none() && p.nterms() == 3 && p.degree() == 2);
        assert_eq!(p.support(), vec![0, 1]);
        assert_eq!(p.as_constant(), None);
        // exact value at (5, 7): big*49 - 14
        let want = BigInt::from_decimal(big)
            .unwrap()
            .mul(&BigInt::from_i128(49))
            .add(&BigInt::from_i128(-14));
        assert_eq!(p.eval_big(&[5, 7]).unwrap(), want);
        assert!(matches!(p.eval(&[5, 7]).unwrap(), Value::Big(_)));
        assert_eq!(p.eval_checked_i128(&[5, 7]), Err(EvalError::Overflow));
        assert_eq!(
            p.eval_checked_i128(&[5]),
            Err(EvalError::KeyTooShort { needed: 2, len: 1 })
        );
        // the checked arithmetic refuses big operands (typed), the exact text round-trips
        assert_eq!(
            p.add(&Poly::constant(1)),
            Err(PolyError::CoefficientOverflow)
        );
        assert_eq!(
            p.mul(&Poly::constant(1)),
            Err(PolyError::CoefficientOverflow)
        );
        assert_eq!(p.neg(), Err(PolyError::CoefficientOverflow));
        assert_eq!(Poly::parse_exact(&p.to_string(), 2, false).unwrap(), p);
        assert_eq!(p.primitive_normal(), p);
        // cancellation back into i128: the canonical small form, equal to the strict parse of the result
        let c = Poly::parse_exact(&format!("{big}*n0 - {big}*n0 + 2*n1 - 1"), 2, false).unwrap();
        assert!(!c.is_big());
        assert_eq!(c, Poly::parse("2*n1 - 1", 2, false).unwrap());
        // a strict-size literal is the same polynomial under both parsers
        assert_eq!(
            Poly::parse_exact("3*n0^2 - n1 + 7", 2, false).unwrap(),
            Poly::parse("3*n0^2 - n1 + 7", 2, false).unwrap()
        );
        // zero test on the zero set of a big multiple, d-coefficients of a big pole literal
        let z = Poly::parse_exact(&format!("{big}*(n0 - 3)*(n1 + 2)"), 2, false).unwrap();
        assert!(z.is_zero_at(&[3, 9]).unwrap() && z.is_zero_at(&[1, -2]).unwrap());
        assert!(!z.is_zero_at(&[4, 9]).unwrap());
        let w = Poly::parse_exact(&format!("{big}*d*n0 + d^2 - n1"), 2, true).unwrap();
        let dc = w.d_coefficients();
        assert_eq!(dc.len(), 3);
        assert!(dc[1].1.is_big() && !dc[0].1.is_big() && !dc[2].1.is_big());
        // grammar errors are unchanged under the exact parser
        assert_eq!(
            Poly::parse_exact("n0/2", 2, false),
            Err(PolyError::Division)
        );
        assert_eq!(
            Poly::parse_exact(&format!("{big}*d"), 2, false),
            Err(PolyError::ContainsD)
        );
    }

    #[test]
    #[should_panic(expected = "coefficient beyond i128")]
    fn terms_on_a_big_literal_panics_instead_of_truncating() {
        let p = Poly::parse_exact("714237321763339320822816811060387425213038592*n0", 1, false)
            .unwrap();
        let _ = p.terms();
    }
}
