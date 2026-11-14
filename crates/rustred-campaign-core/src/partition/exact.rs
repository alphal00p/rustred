//! Exact integer arithmetic for the partition and the loci: dense-exponent multivariate polynomials over `i128`,
//! exact integer roots of univariate polynomials of any degree (rational-root theorem + Cauchy bound, Horner with the
//! core's exact [`BigInt`] fallback), exact integer feasibility of one linear equation on a box, extended gcd,
//! linear-factor extraction.
//!
//! OVERFLOW SAFETY BY CONSTRUCTION (re-audit RA2-C1 / M1): every arithmetic operation on a coefficient, a bound or a
//! parameter value goes through a CHECKED primitive ([`neg`], [`add`], [`sub`], [`mul`], [`quot`], [`rem`],
//! [`div_floor`], [`div_ceil`], [`gcd`], or the std `checked_*` methods); a result that does not fit `i128` (or a
//! division by zero / `i128::MIN / -1`) is the typed error [`ExactError::Overflow`], never a wrap and never a panic.
//! Callers turn it into an UNDECIDED verdict (literal V, rule not covering, containment `Unknown`), never into
//! "empty" or "covered". The module-level `deny` below makes clippy refuse any new unchecked operator or truncating
//! cast here (index / counter arithmetic uses saturating or checked methods; every local `allow` states why).
//!
//! No floating point is used anywhere in this module.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use std::collections::BTreeMap;
use std::fmt;

use crate::poly::{BigInt, D_VAR, Poly};

/// Failure of an exact step. The caller treats the affected test conservatively (never as a decided verdict).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExactError {
    /// A result does not fit `i128` (or has no value: division by zero, `i128::MIN / -1`).
    Overflow,
    /// The exact procedure would exceed its work cap (named).
    TooLarge(&'static str),
}

impl fmt::Display for ExactError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExactError::Overflow => write!(f, "i128 overflow in an exact step"),
            ExactError::TooLarge(w) => write!(f, "exact step above its work cap: {w}"),
        }
    }
}

pub type XResult<T> = Result<T, ExactError>;

fn ck(o: Option<i128>) -> XResult<i128> {
    o.ok_or(ExactError::Overflow)
}

// ------------------------------------------------------------------------------------------------------------------
// Checked primitives (the ONLY way coefficients are combined in partition / loci)
// ------------------------------------------------------------------------------------------------------------------

/// `-a`, checked (`-i128::MIN` is an overflow).
#[inline]
pub fn neg(a: i128) -> XResult<i128> {
    ck(a.checked_neg())
}
/// `a + b`, checked.
#[inline]
pub fn add(a: i128, b: i128) -> XResult<i128> {
    ck(a.checked_add(b))
}
/// `a - b`, checked.
#[inline]
pub fn sub(a: i128, b: i128) -> XResult<i128> {
    ck(a.checked_sub(b))
}
/// `a * b`, checked.
#[inline]
pub fn mul(a: i128, b: i128) -> XResult<i128> {
    ck(a.checked_mul(b))
}
/// Truncating quotient `a / b`, checked (`b == 0` and `i128::MIN / -1` are errors).
#[inline]
pub fn quot(a: i128, b: i128) -> XResult<i128> {
    ck(a.checked_div(b))
}
/// Truncating remainder `a % b`, checked.
#[inline]
pub fn rem(a: i128, b: i128) -> XResult<i128> {
    ck(a.checked_rem(b))
}
/// `|a|`, checked (`|i128::MIN|` is an overflow).
#[inline]
pub fn abs(a: i128) -> XResult<i128> {
    ck(a.checked_abs())
}
/// `a + 1`, checked.
#[inline]
pub fn succ(a: i128) -> XResult<i128> {
    add(a, 1)
}
/// `a - 1`, checked.
#[inline]
pub fn pred(a: i128) -> XResult<i128> {
    sub(a, 1)
}
/// `-a - 1` (the complement offset of a bound `a >= 0` -> `-a - 1 >= 0`): equal to the bitwise NOT `!a` in two's
/// complement, defined for EVERY `i128` (never overflows; `Result` for uniformity).
#[inline]
pub fn neg_pred(a: i128) -> XResult<i128> {
    Ok(!a)
}

/// `floor(-b / a)` without negating `b` (`-i128::MIN` would overflow although the quotient may fit), checked.
pub fn floor_neg_div(b: i128, a: i128) -> XResult<i128> {
    match neg(a) {
        Ok(na) => div_floor(b, na),
        Err(_) => neg(div_ceil(b, a)?),
    }
}

/// `ceil(-b / a)` without negating `b`, checked.
pub fn ceil_neg_div(b: i128, a: i128) -> XResult<i128> {
    match neg(a) {
        Ok(na) => div_ceil(b, na),
        Err(_) => neg(div_floor(b, a)?),
    }
}

/// Every entry negated, checked (the complement of a linear form).
pub fn neg_all(v: &[i128]) -> XResult<Vec<i128>> {
    v.iter().map(|&a| neg(a)).collect()
}

/// gcd of the absolute values, in `u128` (no clamp: `gcd(i128::MIN, 0) = 2^127`).
pub fn gcd_u(a: i128, b: i128) -> u128 {
    let (mut a, mut b) = (a.unsigned_abs(), b.unsigned_abs());
    while b != 0 {
        let Some(t) = a.checked_rem(b) else { break };
        a = b;
        b = t;
    }
    a
}

/// gcd (>= 0) as `i128`; `Overflow` when it is 2^127 (re-audit RA2-m2: it is never clamped to `i128::MAX`, which
/// does not divide `i128::MIN`).
pub fn gcd(a: i128, b: i128) -> XResult<i128> {
    i128::try_from(gcd_u(a, b)).map_err(|_| ExactError::Overflow)
}

/// gcd of a list (>= 0; 0 for an empty or all-zero list), checked as [`gcd`].
pub fn gcd_all(v: impl IntoIterator<Item = i128>) -> XResult<i128> {
    let mut g: u128 = 0;
    for a in v {
        let au = a.unsigned_abs();
        let (mut x, mut y) = (g, au);
        while y != 0 {
            let Some(t) = x.checked_rem(y) else { break };
            x = y;
            y = t;
        }
        g = x;
    }
    i128::try_from(g).map_err(|_| ExactError::Overflow)
}

/// Extended gcd: (g, x, y) with a x + b y = g >= 0.
pub fn ext_gcd(a: i128, b: i128) -> XResult<(i128, i128, i128)> {
    let (mut r0, mut r1) = (a, b);
    let (mut s0, mut s1) = (1i128, 0i128);
    let (mut t0, mut t1) = (0i128, 1i128);
    while r1 != 0 {
        let q = quot(r0, r1)?;
        let r2 = sub(r0, mul(q, r1)?)?;
        let s2 = sub(s0, mul(q, s1)?)?;
        let t2 = sub(t0, mul(q, t1)?)?;
        r0 = r1;
        r1 = r2;
        s0 = s1;
        s1 = s2;
        t0 = t1;
        t1 = t2;
    }
    if r0 < 0 {
        Ok((neg(r0)?, neg(s0)?, neg(t0)?))
    } else {
        Ok((r0, s0, t0))
    }
}

/// Floor division, checked (`b == 0` and `i128::MIN / -1` are `Overflow`).
pub fn div_floor(a: i128, b: i128) -> XResult<i128> {
    let q = quot(a, b)?;
    if rem(a, b)? != 0 && ((a < 0) != (b < 0)) {
        pred(q)
    } else {
        Ok(q)
    }
}

/// Ceiling division, checked.
pub fn div_ceil(a: i128, b: i128) -> XResult<i128> {
    let q = quot(a, b)?;
    if rem(a, b)? != 0 && ((a < 0) == (b < 0)) {
        succ(q)
    } else {
        Ok(q)
    }
}

/// `i128` -> `i64`, checked (re-audit RA2-m3: never an `as` truncation).
#[inline]
pub fn to_i64(a: i128) -> XResult<i64> {
    i64::try_from(a).map_err(|_| ExactError::Overflow)
}

/// Positive divisors of `n > 0` by trial division; `None` above the cap (n > 10^14).
pub fn divisors(n: u128) -> Option<Vec<u128>> {
    if n == 0 || n > 100_000_000_000_000 {
        return None;
    }
    let mut small = vec![];
    let mut large = vec![];
    let mut d: u128 = 1;
    // d <= 10^7 here, so d * d never overflows (checked anyway)
    while d.checked_mul(d).is_some_and(|dd| dd <= n) {
        if n.is_multiple_of(d) {
            small.push(d);
            let q = n.checked_div(d)?;
            if q != d {
                large.push(q);
            }
        }
        d = d.checked_add(1)?;
    }
    large.reverse();
    small.extend(large);
    Some(small)
}

// ------------------------------------------------------------------------------------------------------------------
// Univariate polynomials (coefficients ascending: c[0] + c[1] x + ...)
// ------------------------------------------------------------------------------------------------------------------

/// Sign of `p(x)` exactly (Horner in checked i128, exact BigInt fallback).
pub fn univ_sign(c: &[i128], x: i128) -> i32 {
    let mut acc: i128 = 0;
    let mut ok = true;
    for &a in c.iter().rev() {
        match acc.checked_mul(x).and_then(|v| v.checked_add(a)) {
            Some(v) => acc = v,
            None => {
                ok = false;
                break;
            }
        }
    }
    if ok {
        return match acc.signum() {
            1 => 1,
            -1 => -1,
            _ => 0,
        };
    }
    let bx = BigInt::from_i128(x);
    let mut b = BigInt::zero();
    for &a in c.iter().rev() {
        b = b.mul(&bx).add(&BigInt::from_i128(a));
    }
    b.signum()
}

fn trim(c: &[i128]) -> &[i128] {
    let mut n = c.len();
    while let Some(m) = n.checked_sub(1)
        && c[m] == 0
    {
        n = m;
    }
    &c[..n]
}

/// Cauchy bound: every real root x has |x| <= 1 + max |c_i / c_n| (integer upper bound). `c` trimmed, degree >= 1.
fn cauchy_bound(c: &[i128]) -> u128 {
    let Some((last, rest)) = c.split_last() else {
        return 0;
    };
    let lead = last.unsigned_abs().max(1);
    let mut m: u128 = 0;
    for &a in rest {
        m = m.max(a.unsigned_abs().div_ceil(lead));
    }
    m.saturating_add(1)
}

/// Smallest integer r with r^k >= q (k >= 1).
fn iroot_ceil(q: u128, k: u32) -> u128 {
    if q <= 1 || k == 1 {
        return q;
    }
    // hi^k >= 2^128 > q: ceil(128 / k) bits (audit-part-A m1: floor(128 / k) is not always an upper bound)
    let bits = 128u32.div_ceil(k).min(127);
    let (mut lo, mut hi) = (1u128, 1u128.checked_shl(bits).unwrap_or(u128::MAX));
    while lo < hi {
        let mid = lo.midpoint(hi);
        let mut p: u128 = 1;
        let mut big = false;
        for _ in 0..k {
            match p.checked_mul(mid) {
                Some(v) => p = v,
                None => {
                    big = true;
                    break;
                }
            }
        }
        if big || p >= q {
            hi = mid;
        } else {
            lo = mid.saturating_add(1); // mid < hi: no saturation
        }
    }
    lo
}

/// Fujiwara bound (integer upper bound on |root|): 2 max_i |c_{n-i}/c_n|^{1/i}, the last term with an extra 1/2.
fn fujiwara_bound(c: &[i128]) -> u128 {
    let n = c.len().saturating_sub(1);
    let lead = c[n].unsigned_abs().max(1);
    let mut m: u128 = 0;
    for i in 1..=n {
        let a = c[n.saturating_sub(i)].unsigned_abs();
        if a == 0 {
            continue;
        }
        let q = if i == n {
            a.div_ceil(lead).div_ceil(2) // = ceil(a / (2 lead)) without overflowing 2 lead
        } else {
            a.div_ceil(lead)
        };
        let k = u32::try_from(i).unwrap_or(u32::MAX);
        m = m.max(iroot_ceil(q, k));
    }
    m.saturating_mul(2).saturating_add(1)
}

/// `+x` and `-x` as `i128` where they fit (`x <= 2^127`).
fn signed_pair(x: u128) -> Vec<i128> {
    let mut out = vec![];
    if let Ok(v) = i128::try_from(x) {
        out.push(v);
        if let Ok(nv) = neg(v) {
            out.push(nv);
        }
    } else if x == i128::MIN.unsigned_abs() {
        out.push(i128::MIN);
    }
    out
}

/// ALL integer roots of a nonzero univariate polynomial, ascending, exact (no range cap: loci work in parameter
/// space, where roots can exceed `i32` and `i64`; audit-part-A m2). A LINEAR polynomial `c0 + c1 x` has the root
/// `-c0 / c1` when divisible (re-audit RA2-m7: no candidate search, any size). Otherwise roots are bounded by
/// min(Cauchy, Fujiwara); when that bound exceeds 10^6 the candidates are the divisors of the constant term (trial
/// division up to 10^14), else `ExactError::TooLarge` (the caller is conservative). A root that does not fit `i128`
/// is `ExactError::Overflow` (never dropped silently).
pub fn int_roots(c: &[i128]) -> XResult<Vec<i128>> {
    let c = trim(c);
    if c.len() <= 1 {
        return Ok(vec![]); // nonzero constant (or zero: caller's business)
    }
    let mut roots = vec![];
    let k = c.iter().position(|&x| x != 0).unwrap_or(0);
    if k > 0 {
        roots.push(0);
    }
    let c = &c[k..];
    if c.len() <= 1 {
        return Ok(roots);
    }
    if let [c0, c1] = *c {
        // c0 + c1 x = 0 -> x = -c0 / c1 (c0 != 0, c1 != 0); a root 2^127 does not fit: Overflow
        if c1.unsigned_abs() == 1 || rem(c0, c1)? == 0 {
            roots.push(floor_neg_div(c0, c1)?);
        }
        roots.sort_unstable();
        roots.dedup();
        return Ok(roots);
    }
    let bound = cauchy_bound(c).min(fujiwara_bound(c));
    let c0 = c[0].unsigned_abs();
    let cands: Vec<u128> = if bound <= 1_000_000 {
        (1..=bound).filter(|x| c0.is_multiple_of(*x)).collect()
    } else {
        match divisors(c0) {
            Some(d) => d.into_iter().filter(|&x| x <= bound).collect(),
            None => {
                return Err(ExactError::TooLarge(
                    "integer roots: constant term and root bound both large",
                ));
            }
        }
    };
    for x in cands {
        for v in signed_pair(x) {
            if univ_sign(c, v) == 0 {
                roots.push(v);
            }
        }
    }
    roots.sort_unstable();
    roots.dedup();
    Ok(roots)
}

/// The integer points where the sign of a nonconstant polynomial changes: every integer `x` with
/// `sign p(x) != sign p(x + 1)` (|x| within the Cauchy bound), exact. Used for one-slot GE literals of degree >= 2.
pub fn sign_breaks(c: &[i128]) -> XResult<Vec<i128>> {
    let c = trim(c);
    if c.len() <= 1 {
        return Ok(vec![]);
    }
    if let [b, a] = *c {
        // a x + b: the sign changes between floor(-b/a) and ceil(-b/a)
        let f = floor_neg_div(b, a)?;
        return Ok(vec![pred(f)?, f, succ(f)?]);
    }
    let bound = cauchy_bound(c);
    if bound > 200_000 {
        return Err(ExactError::TooLarge("sign breaks: root bound above 2e5"));
    }
    let b = i128::try_from(bound).map_err(|_| ExactError::Overflow)?;
    let mut out = vec![];
    let lo = neg(b)?;
    let mut prev = univ_sign(c, pred(lo)?);
    let mut x = lo;
    while x <= succ(b)? {
        let s = univ_sign(c, x);
        if s != prev || s == 0 {
            out.push(pred(x)?);
            out.push(x);
        }
        prev = s;
        x = succ(x)?;
    }
    out.sort_unstable();
    out.dedup();
    Ok(out)
}

/// Is `p(x) >= 0` at EVERY integer `x` of `[lo, hi]` (`None` = unbounded side)? `Some(true)`: yes; `Some(false)`:
/// `p(x) < 0` at every such x; `None`: the sign varies, or the decision would exceed its work cap. Exact: beyond the
/// Cauchy bound the sign is that of the leading term; inside it every integer is evaluated (at most `cap` values).
/// Used for one-slot NONLINEAR bounds on a state (re-audit RA2-M2: their sign IS constant on every state cut at the
/// sign breaks, but it is decided here directly, whatever the state boundaries are).
pub fn univ_ge_on(c: &[i128], lo: Option<i128>, hi: Option<i128>, cap: u128) -> Option<bool> {
    let c = trim(c);
    if c.len() <= 1 {
        return Some(c.first().copied().unwrap_or(0) >= 0);
    }
    if let (Some(l), Some(h)) = (lo, hi)
        && l > h
    {
        return None; // empty range: nothing to decide (callers never ask)
    }
    let b = i128::try_from(cauchy_bound(c)).ok()?;
    let lead = *c.last()?;
    let deg_odd = c.len().is_multiple_of(2); // len = degree + 1
    let sign_pos_inf = lead > 0;
    let sign_neg_inf_ge = if deg_odd { lead < 0 } else { lead > 0 }; // p(-inf) > 0 ?
    let mut want: Option<bool> = None;
    let mut agree = |ge: bool| -> bool {
        match want {
            None => {
                want = Some(ge);
                true
            }
            Some(w) => w == ge,
        }
    };
    // tails beyond the bound: |x| > b has no real root, sign constant
    let nb = b.checked_neg()?;
    if lo.is_none_or(|l| l < nb) && !agree(sign_neg_inf_ge) {
        return None;
    }
    if hi.is_none_or(|h| h > b) && !agree(sign_pos_inf) {
        return None;
    }
    let a = lo.map_or(nb, |l| l.max(nb));
    let z = hi.map_or(b, |h| h.min(b));
    if a <= z {
        let width = z.checked_sub(a)?.unsigned_abs();
        if width >= cap {
            return None;
        }
        let mut x = a;
        loop {
            if !agree(univ_sign(c, x) >= 0) {
                return None;
            }
            if x == z {
                break;
            }
            x = x.checked_add(1)?;
        }
    }
    want
}

// ------------------------------------------------------------------------------------------------------------------
// Integer feasibility of one linear equation on a box
// ------------------------------------------------------------------------------------------------------------------

/// A closed integer range; `None` = infinite on that side.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Range {
    pub lo: Option<i64>,
    pub hi: Option<i64>,
}

impl Range {
    pub fn point(v: i64) -> Self {
        Range {
            lo: Some(v),
            hi: Some(v),
        }
    }
    pub fn contains(&self, v: i64) -> bool {
        self.lo.is_none_or(|l| v >= l) && self.hi.is_none_or(|h| v <= h)
    }
    /// [`Range::contains`] for a value of any `i128` size (an infinite side contains every value beyond `i64`).
    pub fn contains_wide(&self, v: i128) -> bool {
        self.lo.is_none_or(|l| v >= i128::from(l)) && self.hi.is_none_or(|h| v <= i128::from(h))
    }
    pub fn singleton(&self) -> Option<i64> {
        match (self.lo, self.hi) {
            (Some(a), Some(b)) if a == b => Some(a),
            _ => None,
        }
    }
    /// Number of integers in a bounded range (`None` if unbounded).
    pub fn size(&self) -> Option<u128> {
        match (self.lo, self.hi) {
            (Some(a), Some(b)) if b >= a => {
                Some(i128::from(b).abs_diff(i128::from(a)).saturating_add(1))
            }
            (Some(_), Some(_)) => Some(0),
            _ => None,
        }
    }
}

const DP_CAP: i128 = 4_000_000;

fn to_usize(v: i128) -> XResult<usize> {
    usize::try_from(v).map_err(|_| ExactError::TooLarge("index beyond usize"))
}

/// Set of values of `sum b_l y_l`, `0 <= y_l <= u_l` (all finite), as a bitset over `[smin, smax]`.
fn bounded_sums(vars: &[(i128, i128)]) -> XResult<(i128, Vec<bool>)> {
    let mut smin: i128 = 0;
    let mut smax: i128 = 0;
    for &(b, u) in vars {
        let v = mul(b, u)?;
        if v < 0 {
            smin = add(smin, v)?;
        } else {
            smax = add(smax, v)?;
        }
    }
    let width = sub(smax, smin)?;
    if width > DP_CAP {
        return Err(ExactError::TooLarge(
            "linear feasibility: bounded sum range above 4e6",
        ));
    }
    let n = to_usize(succ(width)?)?;
    let mut cur = vec![false; n];
    cur[to_usize(neg(smin)?)?] = true; // sum 0
    let mut work: i128 = 0;
    for &(b, u) in vars {
        work = add(work, mul(succ(u)?, width)?)?;
        if work > mul(40, DP_CAP)? {
            return Err(ExactError::TooLarge(
                "linear feasibility: bounded sum DP work",
            ));
        }
        let mut next = vec![false; n];
        for (i, &on) in cur.iter().enumerate() {
            if !on {
                continue;
            }
            let base = i128::try_from(i).map_err(|_| ExactError::Overflow)?;
            let mut y = 0i128;
            while y <= u {
                let j = add(base, mul(b, y)?)?;
                if let Ok(ju) = usize::try_from(j)
                    && ju < n
                {
                    next[ju] = true;
                }
                y = succ(y)?;
            }
        }
        cur = next;
    }
    Ok((smin, cur))
}

/// Does `sum a_i x_i + c = 0` have an integer solution with every `x_i` in its range? Exact; every variable must have
/// at least one finite bound, or both infinite (treated as two half-lines). `Err` only above the work caps or on an
/// overflow (the caller is conservative).
pub fn linear_feasible(terms: &[(i128, Range)], c: i128) -> XResult<bool> {
    // y >= 0 variables: (b, Some(u)) bounded, (b, None) unbounded; equation sum b y = t
    let mut t: i128 = neg(c)?;
    let mut bnd: Vec<(i128, i128)> = vec![];
    let mut unb: Vec<i128> = vec![];
    for &(a, r) in terms {
        if a == 0 {
            continue;
        }
        match (r.lo, r.hi) {
            (Some(l), Some(h)) => {
                if h < l {
                    return Ok(false);
                }
                t = sub(t, mul(a, i128::from(l))?)?;
                if h > l {
                    bnd.push((a, sub(i128::from(h), i128::from(l))?));
                }
            }
            (Some(l), None) => {
                t = sub(t, mul(a, i128::from(l))?)?;
                unb.push(a);
            }
            (None, Some(h)) => {
                t = sub(t, mul(a, i128::from(h))?)?;
                unb.push(neg(a)?);
            }
            (None, None) => {
                unb.push(a);
                unb.push(neg(a)?);
            }
        }
    }
    let g = gcd_all(bnd.iter().map(|x| x.0).chain(unb.iter().copied()))?;
    if g == 0 {
        return Ok(t == 0);
    }
    if rem(t, g)? != 0 {
        return Ok(false);
    }
    t = quot(t, g)?;
    for x in bnd.iter_mut() {
        x.0 = quot(x.0, g)?;
    }
    for x in unb.iter_mut() {
        *x = quot(*x, g)?;
    }
    let pos = unb.iter().any(|&b| b > 0);
    let negv = unb.iter().any(|&b| b < 0);
    if pos && negv {
        // the unbounded variables generate h Z; need bounded part = t mod h
        let h = gcd_all(unb.iter().copied())?;
        if h > 1_000_000 {
            return Err(ExactError::TooLarge(
                "linear feasibility: residue modulus above 1e6",
            ));
        }
        let hm = to_usize(h)?;
        let mut reach = vec![false; hm];
        reach[0] = true;
        for &(b, u) in &bnd {
            let mut next = vec![false; hm];
            let steps = u.min(pred(h)?);
            let bm = ck(b.checked_rem_euclid(h))?;
            for (r, &on) in reach.iter().enumerate() {
                if !on {
                    continue;
                }
                let r = i128::try_from(r).map_err(|_| ExactError::Overflow)?;
                let mut y = 0i128;
                while y <= steps {
                    let v = ck(add(r, mul(bm, y)?)?.checked_rem_euclid(h))?;
                    next[to_usize(v)?] = true;
                    y = succ(y)?;
                }
            }
            reach = next;
        }
        return Ok(reach[to_usize(ck(t.checked_rem_euclid(h))?)?]);
    }
    if unb.is_empty() {
        let (smin, set) = bounded_sums(&bnd)?;
        let i = sub(t, smin)?;
        return Ok(usize::try_from(i).is_ok_and(|iu| iu < set.len() && set[iu]));
    }
    // one-signed unbounded part: make it positive
    if negv {
        t = neg(t)?;
        for x in bnd.iter_mut() {
            x.0 = neg(x.0)?;
        }
        for x in unb.iter_mut() {
            *x = neg(*x)?;
        }
    }
    let (smin, set) = bounded_sums(&bnd)?;
    let hp = gcd_all(unb.iter().copied())?;
    let bmin = unb.iter().copied().min().ok_or(ExactError::Overflow)?;
    let bmax = unb.iter().copied().max().ok_or(ExactError::Overflow)?;
    // every multiple of hp at or above lim*hp is representable
    let lim = mul(quot(bmin, hp)?, quot(bmax, hp)?)?;
    let lim_v = mul(lim, hp)?;
    if lim_v > DP_CAP {
        return Err(ExactError::TooLarge(
            "linear feasibility: Frobenius bound above 4e6",
        ));
    }
    // representable values below lim_v
    let mut rep = vec![false; to_usize(lim_v.max(1))?];
    rep[0] = true;
    for v in 1..rep.len() {
        let vi = i128::try_from(v).map_err(|_| ExactError::Overflow)?;
        let mut hit = false;
        for &b in &unb {
            if vi >= b && rep[to_usize(sub(vi, b)?)?] {
                hit = true;
                break;
            }
        }
        rep[v] = hit;
    }
    for (i, &on) in set.iter().enumerate() {
        if !on {
            continue;
        }
        let s = add(smin, i128::try_from(i).map_err(|_| ExactError::Overflow)?)?;
        let v = sub(t, s)?;
        if v < 0 || rem(v, hp)? != 0 {
            continue;
        }
        if v >= lim_v || rep[to_usize(v)?] {
            return Ok(true);
        }
    }
    Ok(false)
}

// ------------------------------------------------------------------------------------------------------------------
// Dense-exponent multivariate polynomials
// ------------------------------------------------------------------------------------------------------------------

/// A polynomial in `nvars` variables with `i128` coefficients; terms keyed by dense exponent vectors.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MPoly {
    pub nvars: usize,
    pub terms: BTreeMap<Vec<u32>, i128>,
}

/// An affine form `c + sum coef_j t_j` over `k` parameters.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Affine {
    pub coef: Vec<i128>,
    pub c: i128,
}

impl Affine {
    pub fn constant(k: usize, c: i128) -> Self {
        Affine {
            coef: vec![0; k],
            c,
        }
    }
    pub fn var(k: usize, j: usize) -> Self {
        let mut coef = vec![0; k];
        coef[j] = 1;
        Affine { coef, c: 0 }
    }
    /// `s * t_j + c` (one parameter with coefficient `s`).
    pub fn unit(k: usize, j: usize, s: i128, c: i128) -> Self {
        let mut coef = vec![0; k];
        coef[j] = s;
        Affine { coef, c }
    }
    pub fn eval(&self, t: &[i128]) -> XResult<i128> {
        let mut v = self.c;
        for (a, x) in self.coef.iter().zip(t) {
            v = add(v, mul(*a, *x)?)?;
        }
        Ok(v)
    }
    pub fn is_constant(&self) -> bool {
        self.coef.iter().all(|&a| a == 0)
    }
    /// `self >= 0` <-> `self + d >= d`; the constraint `self - shift >= 0` (checked).
    pub fn shifted(&self, shift: i128) -> XResult<Affine> {
        Ok(Affine {
            coef: self.coef.clone(),
            c: sub(self.c, shift)?,
        })
    }
    /// The complement of the constraint `self >= 0` over the integers: `-self - 1 >= 0` (checked; re-audit RA2-C1:
    /// never an unchecked negation).
    pub fn complement(&self) -> XResult<Affine> {
        Ok(Affine {
            coef: neg_all(&self.coef)?,
            c: neg_pred(self.c)?,
        })
    }
    pub fn render(&self, names: &[String]) -> String {
        let mut p = MPoly::zero(self.coef.len());
        for (j, &a) in self.coef.iter().enumerate() {
            if a != 0 {
                let mut e = vec![0; self.coef.len()];
                e[j] = 1;
                p.terms.insert(e, a);
            }
        }
        if self.c != 0 {
            p.terms.insert(vec![0; self.coef.len()], self.c);
        }
        p.render(names)
    }
}

impl MPoly {
    pub fn zero(nvars: usize) -> Self {
        MPoly {
            nvars,
            terms: BTreeMap::new(),
        }
    }
    pub fn constant(nvars: usize, c: i128) -> Self {
        let mut p = MPoly::zero(nvars);
        if c != 0 {
            p.terms.insert(vec![0; nvars], c);
        }
        p
    }
    pub fn var(nvars: usize, v: usize) -> Self {
        let mut p = MPoly::zero(nvars);
        let mut e = vec![0; nvars];
        e[v] = 1;
        p.terms.insert(e, 1);
        p
    }
    /// From a core polynomial: slot `i` -> variable `i`, `d` -> variable `arity` (only if `with_d`).
    pub fn from_poly(p: &Poly, arity: usize, with_d: bool) -> XResult<Self> {
        let nv = arity.saturating_add(usize::from(with_d));
        let mut out = MPoly::zero(nv);
        for (m, c) in p.terms() {
            let mut e = vec![0u32; nv];
            for &(v, x) in m {
                let idx = if v == D_VAR {
                    if !with_d {
                        return Err(ExactError::TooLarge("d in a d-free literal"));
                    }
                    arity
                } else {
                    usize::from(v)
                };
                if idx >= nv {
                    return Err(ExactError::TooLarge("variable beyond arity"));
                }
                e[idx] = x;
            }
            out.add_term(e, *c)?;
        }
        Ok(out)
    }
    /// Back to a core polynomial (variable `arity` -> d).
    pub fn to_poly(&self, arity: usize) -> XResult<Poly> {
        let core = |_| ExactError::TooLarge("core polynomial");
        let mut acc = Poly::zero();
        for (e, &c) in &self.terms {
            let mut t = Poly::constant(c);
            for (v, &x) in e.iter().enumerate() {
                if x == 0 {
                    continue;
                }
                if v != arity && v >= crate::poly::MAX_ARITY {
                    return Err(ExactError::TooLarge("variable beyond the core arity"));
                }
                let base = if v == arity { Poly::d() } else { Poly::var(v) };
                t = t
                    .mul(&base.pow(u64::from(x)).map_err(core)?)
                    .map_err(core)?;
            }
            acc = acc.add(&t).map_err(core)?;
        }
        Ok(acc)
    }
    fn add_term(&mut self, e: Vec<u32>, c: i128) -> XResult<()> {
        if c == 0 {
            return Ok(());
        }
        let ent = self.terms.entry(e).or_insert(0);
        *ent = add(*ent, c)?;
        if *ent == 0 {
            self.terms.retain(|_, v| *v != 0);
        }
        Ok(())
    }
    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }
    pub fn as_constant(&self) -> Option<i128> {
        if self.terms.is_empty() {
            return Some(0);
        }
        if self.terms.len() == 1 {
            let (e, c) = self.terms.iter().next()?;
            if e.iter().all(|&x| x == 0) {
                return Some(*c);
            }
        }
        None
    }
    pub fn support(&self) -> Vec<usize> {
        let mut s = vec![false; self.nvars];
        for e in self.terms.keys() {
            for (v, &x) in e.iter().enumerate() {
                if x > 0 {
                    s[v] = true;
                }
            }
        }
        (0..self.nvars).filter(|&v| s[v]).collect()
    }
    pub fn total_degree(&self) -> u32 {
        self.terms
            .keys()
            .map(|e| e.iter().fold(0u32, |a, &x| a.saturating_add(x)))
            .max()
            .unwrap_or(0)
    }
    pub fn degree_in(&self, v: usize) -> u32 {
        self.terms.keys().map(|e| e[v]).max().unwrap_or(0)
    }
    pub fn add(&self, o: &MPoly) -> XResult<MPoly> {
        let mut r = self.clone();
        for (e, &c) in &o.terms {
            r.add_term(e.clone(), c)?;
        }
        Ok(r)
    }
    pub fn scale(&self, k: i128) -> XResult<MPoly> {
        let mut r = MPoly::zero(self.nvars);
        if k == 0 {
            return Ok(r);
        }
        for (e, &c) in &self.terms {
            r.terms.insert(e.clone(), mul(c, k)?);
        }
        Ok(r)
    }
    pub fn sub(&self, o: &MPoly) -> XResult<MPoly> {
        self.add(&o.scale(-1)?)
    }
    pub fn mul(&self, o: &MPoly) -> XResult<MPoly> {
        let mut r = MPoly::zero(self.nvars);
        for (ea, &ca) in &self.terms {
            for (eb, &cb) in &o.terms {
                let e: Vec<u32> = ea
                    .iter()
                    .zip(eb)
                    .map(|(a, b)| a.checked_add(*b).ok_or(ExactError::Overflow))
                    .collect::<XResult<_>>()?;
                r.add_term(e, mul(ca, cb)?)?;
            }
        }
        Ok(r)
    }
    pub fn pow(&self, k: u32) -> XResult<MPoly> {
        let mut r = MPoly::constant(self.nvars, 1);
        for _ in 0..k {
            r = r.mul(self)?;
        }
        Ok(r)
    }
    /// Substitute `x_v = val`.
    pub fn subst(&self, v: usize, val: i128) -> XResult<MPoly> {
        let mut r = MPoly::zero(self.nvars);
        for (e, &c) in &self.terms {
            let mut f = c;
            for _ in 0..e[v] {
                f = mul(f, val)?;
            }
            let mut e2 = e.clone();
            e2[v] = 0;
            r.add_term(e2, f)?;
        }
        Ok(r)
    }
    /// Compose with an affine map: old variable `i` -> `map[i]` (affine in `k` new variables).
    pub fn compose(&self, map: &[Affine], k: usize) -> XResult<MPoly> {
        let lin: Vec<MPoly> = map
            .iter()
            .map(|a| {
                let mut p = MPoly::constant(k, a.c);
                for (j, &x) in a.coef.iter().enumerate() {
                    if x != 0 {
                        let mut e = vec![0; k];
                        e[j] = 1;
                        p.terms.insert(e, x);
                    }
                }
                p
            })
            .collect();
        let mut powcache: BTreeMap<(usize, u32), MPoly> = BTreeMap::new();
        let mut r = MPoly::zero(k);
        for (e, &c) in &self.terms {
            let mut t = MPoly::constant(k, c);
            for (v, &x) in e.iter().enumerate() {
                if x == 0 {
                    continue;
                }
                let pw = match powcache.entry((v, x)) {
                    std::collections::btree_map::Entry::Occupied(o) => o.into_mut(),
                    std::collections::btree_map::Entry::Vacant(slot) => slot.insert(lin[v].pow(x)?),
                };
                t = t.mul(pw)?;
            }
            r = r.add(&t)?;
        }
        Ok(r)
    }
    /// `Some((coef, c))` if the total degree is <= 1.
    pub fn linear_form(&self) -> Option<(Vec<i128>, i128)> {
        if self.total_degree() > 1 {
            return None;
        }
        let mut coef = vec![0; self.nvars];
        let mut c = 0;
        for (e, &x) in &self.terms {
            match e.iter().position(|&y| y == 1) {
                Some(v) => coef[v] = x,
                None => c = x,
            }
        }
        Some((coef, c))
    }
    /// Coefficients (ascending) in variable `v`, if `v` is the only variable.
    pub fn univariate(&self, v: usize) -> Option<Vec<i128>> {
        let deg = usize::try_from(self.degree_in(v)).ok()?;
        let mut c = vec![0i128; deg.checked_add(1)?];
        for (e, &x) in &self.terms {
            for (w, &y) in e.iter().enumerate() {
                if w != v && y > 0 {
                    return None;
                }
            }
            c[usize::try_from(e[v]).ok()?] = x;
        }
        Some(c)
    }
    /// Exact evaluation at an integer point (checked i128).
    pub fn eval(&self, x: &[i128]) -> XResult<i128> {
        let mut acc: i128 = 0;
        for (e, &c) in &self.terms {
            let mut t = c;
            for (v, &p) in e.iter().enumerate() {
                for _ in 0..p {
                    t = mul(t, x[v])?;
                }
            }
            acc = add(acc, t)?;
        }
        Ok(acc)
    }
    /// Content (gcd of the coefficients, >= 0); `Overflow` if it is 2^127.
    pub fn content(&self) -> XResult<i128> {
        gcd_all(self.terms.values().copied())
    }
    /// Primitive part with a positive coefficient on the largest exponent vector.
    pub fn primitive(&self) -> XResult<MPoly> {
        let g = self.content()?;
        if g == 0 {
            return Ok(self.clone());
        }
        let s = match self.terms.values().next_back() {
            Some(&lead) if lead < 0 => neg(g)?,
            _ => g,
        };
        let mut r = MPoly::zero(self.nvars);
        for (e, &c) in &self.terms {
            r.terms.insert(e.clone(), quot(c, s)?);
        }
        Ok(r)
    }
    /// Divided by its POSITIVE content (the sign is kept: the normal form of a GE literal).
    pub fn positive_primitive(&self) -> XResult<MPoly> {
        let g = self.content()?;
        if g == 0 {
            return Ok(self.clone());
        }
        let mut r = MPoly::zero(self.nvars);
        for (e, &c) in &self.terms {
            r.terms.insert(e.clone(), quot(c, g)?);
        }
        Ok(r)
    }
    /// Coefficients in variable `v`: `P = sum_k C_k x_v^k`.
    pub fn coeffs_in(&self, v: usize) -> Vec<MPoly> {
        let deg = usize::try_from(self.degree_in(v)).unwrap_or(0);
        let mut out = vec![MPoly::zero(self.nvars); deg.saturating_add(1)];
        for (e, &c) in &self.terms {
            let Ok(k) = usize::try_from(e[v]) else {
                continue;
            };
            let mut e2 = e.clone();
            e2[v] = 0;
            out[k].terms.insert(e2, c);
        }
        out
    }
    /// Exact division by `a x_v + m` (`m` free of `x_v`, `a != 0`): `Some(quotient)` iff it divides exactly over Z.
    pub fn div_linear(&self, v: usize, a: i128, m: &MPoly) -> XResult<Option<MPoly>> {
        let cs = self.coeffs_in(v);
        let n = cs.len().saturating_sub(1);
        if n == 0 || a == 0 {
            return Ok(None);
        }
        // P = (a x + m) Q, Q = sum_{k<n} q_k x^k: c_n = a q_{n-1}; c_k = a q_{k-1} + m q_k; c_0 = m q_0
        let mut q: Vec<MPoly> = vec![MPoly::zero(self.nvars); n];
        let mut rest = cs[n].clone();
        for k in (1..=n).rev() {
            // rest = a q_{k-1}
            let km1 = k.saturating_sub(1); // k >= 1
            let mut qk = MPoly::zero(self.nvars);
            for (e, &c) in &rest.terms {
                if rem(c, a)? != 0 {
                    return Ok(None);
                }
                qk.terms.insert(e.clone(), quot(c, a)?);
            }
            q[km1] = qk;
            rest = cs[km1].sub(&m.mul(&q[km1])?)?;
        }
        if !rest.is_zero() {
            return Ok(None);
        }
        let mut out = MPoly::zero(self.nvars);
        for (k, qk) in q.iter().enumerate() {
            let k = u32::try_from(k).map_err(|_| ExactError::Overflow)?;
            for (e, &c) in &qk.terms {
                let mut e2 = e.clone();
                e2[v] = e2[v].checked_add(k).ok_or(ExactError::Overflow)?;
                out.add_term(e2, c)?;
            }
        }
        Ok(Some(out))
    }
    /// Render with variable names.
    pub fn render(&self, names: &[String]) -> String {
        if self.terms.is_empty() {
            return "0".into();
        }
        let mut s = String::new();
        for (i, (e, &c)) in self.terms.iter().rev().enumerate() {
            let mono: Vec<String> = e
                .iter()
                .enumerate()
                .filter(|(_, x)| **x > 0)
                .map(|(v, &x)| {
                    if x == 1 {
                        names[v].clone()
                    } else {
                        format!("{}^{x}", names[v])
                    }
                })
                .collect();
            let (negv, a) = (c < 0, c.unsigned_abs());
            if i == 0 {
                if negv {
                    s.push('-');
                }
            } else {
                s.push_str(if negv { "-" } else { "+" });
            }
            if mono.is_empty() {
                s.push_str(&a.to_string());
            } else {
                if a != 1 {
                    s.push_str(&format!("{a}*"));
                }
                s.push_str(&mono.join("*"));
            }
        }
        s
    }
}

/// Names `n0..n{arity-1}` (and `d` at index `arity` when `with_d`).
pub fn slot_names(arity: usize, with_d: bool) -> Vec<String> {
    let mut v: Vec<String> = (0..arity).map(|i| format!("n{i}")).collect();
    if with_d {
        v.push("d".into());
    }
    v
}

fn signed_divisors(n: i128) -> Option<Vec<i128>> {
    let d = divisors(n.unsigned_abs())?;
    Some(d.into_iter().flat_map(signed_pair).collect())
}

/// Extract the linear (total degree 1) factors of `p` over Z exactly: `p = c * prod(factors) * cofactor`, every factor
/// primitive. Candidates come from the rational-root theorem applied recursively (the constant part in one variable
/// must be divisible by the factor's remainder); every candidate is verified by exact division, so an incomplete
/// search only leaves a reducible cofactor, never a wrong factor.
pub fn linear_factors(p: &MPoly) -> XResult<(Vec<MPoly>, MPoly)> {
    thread_local! {
        static MEMO: std::cell::RefCell<std::collections::HashMap<MPoly, (Vec<MPoly>, MPoly)>> =
            std::cell::RefCell::new(std::collections::HashMap::new());
    }
    if let Some(r) = MEMO.with(|m| m.borrow().get(p).cloned()) {
        return Ok(r);
    }
    let r = linear_factors_uncached(p)?;
    MEMO.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() > 200_000 {
            m.clear();
        }
        m.insert(p.clone(), r.clone());
    });
    Ok(r)
}

/// Two fixed test points per variable count (arbitrary small values; only a necessary-condition filter: exact
/// division decides). Wrapping arithmetic on values < 64 * 15 + 4: never wraps.
fn probe_points(nvars: usize) -> Vec<Vec<i128>> {
    (0..2i128)
        .map(|r| {
            (0..nvars)
                .map(|i| {
                    let i = i128::try_from(i).unwrap_or(0);
                    i.wrapping_mul(r.wrapping_mul(4).wrapping_add(7))
                        .wrapping_add(3)
                        .wrapping_add(r)
                        .rem_euclid(29)
                        .wrapping_add(2)
                })
                .collect()
        })
        .collect()
}

fn linear_factors_uncached(p: &MPoly) -> XResult<(Vec<MPoly>, MPoly)> {
    let mut rest = p.clone();
    let mut out = vec![];
    if rest.is_zero() || rest.total_degree() <= 1 {
        return Ok((out, rest));
    }
    let mut depth_guard = 0u32;
    'outer: loop {
        depth_guard = depth_guard.saturating_add(1);
        if depth_guard > 64 || rest.total_degree() <= 1 {
            break;
        }
        for v in rest.support() {
            // x_v itself
            let c0 = rest.subst(v, 0)?;
            if c0.is_zero() {
                let m = MPoly::zero(rest.nvars);
                if let Some(q) = rest.div_linear(v, 1, &m)? {
                    out.push(MPoly::var(rest.nvars, v));
                    rest = q;
                    continue 'outer;
                }
            }
            let cs = rest.coeffs_in(v);
            let Some(top) = cs.last() else { continue };
            let a_cands = match divisors(top.content()?.unsigned_abs()) {
                Some(d) => d,
                None => continue,
            };
            // candidate remainders m: +-mu, +-mu*F for F a linear factor of c0
            let mut m_cands: Vec<MPoly> = vec![];
            let cont0 = c0.content()?;
            let mus = match signed_divisors(cont0) {
                Some(d) => d,
                None => continue,
            };
            if mus.len() > 400 || a_cands.len() > 200 {
                continue;
            }
            for &mu in &mus {
                m_cands.push(MPoly::constant(rest.nvars, mu));
            }
            if c0.total_degree() >= 1 {
                let (fs, _) = linear_factors(&c0.primitive()?)?;
                let mut seen = std::collections::BTreeSet::new();
                for f in fs {
                    if !seen.insert(f.clone()) {
                        continue;
                    }
                    for &mu in &mus {
                        m_cands.push(f.scale(mu)?);
                    }
                }
            }
            // cheap necessary test at two integer points y: a^n P(-m(y)/a, y) = 0
            let pts = probe_points(rest.nvars);
            let evs: Vec<Option<Vec<i128>>> = pts
                .iter()
                .map(|y| cs.iter().map(|c| c.eval(y).ok()).collect())
                .collect();
            let passes = |a: i128, m: &MPoly| -> bool {
                for (y, ev) in pts.iter().zip(&evs) {
                    let (Some(ev), Ok(mv)) = (ev, m.eval(y)) else {
                        continue;
                    };
                    let n = ev.len().saturating_sub(1);
                    let Some(neg_mv) = mv.checked_neg() else {
                        continue;
                    };
                    // sum_k e_k (-mv)^k a^(n-k)
                    let mut acc: Option<i128> = Some(0);
                    let mut pw_m: Option<i128> = Some(1);
                    for (k, &e) in ev.iter().enumerate() {
                        let pw_a =
                            (0..n.saturating_sub(k)).try_fold(1i128, |x, _| x.checked_mul(a));
                        let t = pw_m
                            .and_then(|pm| pw_a.and_then(|pa| e.checked_mul(pm)?.checked_mul(pa)));
                        acc = match (acc, t) {
                            (Some(x), Some(t)) => x.checked_add(t),
                            _ => None,
                        };
                        pw_m = pw_m.and_then(|pm| pm.checked_mul(neg_mv));
                    }
                    if acc.is_some_and(|x| x != 0) {
                        return false;
                    }
                }
                true
            };
            for a in &a_cands {
                let Ok(a) = i128::try_from(*a) else { continue };
                for m in &m_cands {
                    if !passes(a, m) {
                        continue;
                    }
                    if let Some(q) = rest.div_linear(v, a, m)? {
                        let mut f = m.clone();
                        let mut e = vec![0; rest.nvars];
                        e[v] = 1;
                        f.add_term(e, a)?;
                        let g = f.content()?;
                        let f = f.primitive()?;
                        out.push(f);
                        // keep the scale in the cofactor
                        rest = q.scale(g)?;
                        continue 'outer;
                    }
                }
            }
        }
        break;
    }
    if rest.total_degree() == 1 {
        let g = rest.content()?;
        let s = match rest.terms.values().next_back() {
            Some(&lead) if lead < 0 => neg(g)?,
            _ => g,
        };
        out.push(rest.primitive()?);
        rest = MPoly::constant(rest.nvars, s);
    }
    Ok((out, rest))
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)] // test arithmetic on small literals
mod tests {
    use super::*;

    #[test]
    fn roots_and_breaks() {
        // (x-2)(x+3)(x-7) = x^3 - 6x^2 - 13x + 42
        assert_eq!(int_roots(&[42, -13, -6, 1]).unwrap(), vec![-3, 2, 7]);
        assert_eq!(int_roots(&[0, 0, 5]).unwrap(), vec![0]);
        assert_eq!(int_roots(&[-263, 159, 42]).unwrap(), Vec::<i128>::new());
        // degree 15: (x-1)^15 has root 1
        let mut c = vec![1i128];
        for _ in 0..15 {
            let mut n = vec![0i128; c.len() + 1];
            for (i, &a) in c.iter().enumerate() {
                n[i + 1] += a;
                n[i] -= a;
            }
            c = n;
        }
        assert_eq!(int_roots(&c).unwrap(), vec![1]);
        assert!(sign_breaks(&[-4, 0, 1]).unwrap().contains(&2));
    }

    #[test]
    fn linear_roots_of_any_size() {
        // re-audit RA2-m7: x - 2^100 has the root 2^100 (no candidate search)
        let big = 1i128 << 100;
        assert_eq!(int_roots(&[-big, 1]).unwrap(), vec![big]);
        assert_eq!(int_roots(&[big, 3]).unwrap(), Vec::<i128>::new());
        assert_eq!(int_roots(&[3 * big, -3]).unwrap(), vec![big]);
        // x * (x + 2^100)
        assert_eq!(int_roots(&[0, big, 1]).unwrap(), vec![-big, 0]);
        // the root 2^127 does not fit: Overflow, never a dropped root
        assert_eq!(int_roots(&[i128::MIN, 1]), Err(ExactError::Overflow));
        // the root -2^127 fits: found
        assert_eq!(int_roots(&[i128::MIN, -1]).unwrap(), vec![i128::MIN]);
        assert_eq!(int_roots(&[i128::MIN, i128::MIN]).unwrap(), vec![-1]);
    }

    #[test]
    fn checked_primitives_never_wrap() {
        assert_eq!(neg(i128::MIN), Err(ExactError::Overflow));
        assert_eq!(div_floor(i128::MIN, -1), Err(ExactError::Overflow));
        assert_eq!(div_ceil(i128::MIN, -1), Err(ExactError::Overflow));
        assert_eq!(div_floor(-7, 2), Ok(-4));
        assert_eq!(div_ceil(-7, 2), Ok(-3));
        assert_eq!(div_floor(7, 0), Err(ExactError::Overflow));
        assert_eq!(gcd(i128::MIN, 0), Err(ExactError::Overflow));
        assert_eq!(gcd_u(i128::MIN, 0), 1u128 << 127);
        assert_eq!(gcd(i128::MIN, 6), Ok(2));
        assert_eq!(gcd_all([i128::MIN, i128::MIN]), Err(ExactError::Overflow));
        assert_eq!(gcd_all([12, -18, 0]), Ok(6));
        let a = Affine {
            coef: vec![i128::MIN, 1],
            c: 0,
        };
        assert_eq!(a.complement(), Err(ExactError::Overflow));
        let b = Affine {
            coef: vec![3, 1],
            c: i128::MIN,
        };
        assert_eq!(
            b.complement().unwrap(),
            Affine {
                coef: vec![-3, -1],
                c: i128::MAX,
            }
        );
        assert_eq!(neg_pred(i128::MIN), Ok(i128::MAX));
        assert_eq!(neg_pred(i128::MAX), Ok(i128::MIN));
        assert_eq!(floor_neg_div(i128::MIN, -1), Ok(i128::MIN));
        assert_eq!(floor_neg_div(i128::MIN, 1), Err(ExactError::Overflow));
        assert_eq!(ceil_neg_div(5, i128::MIN), Ok(1));
        assert_eq!(floor_neg_div(5, i128::MIN), Ok(0));
        assert_eq!(sign_breaks(&[i128::MIN, 1]), Err(ExactError::Overflow));
    }

    #[test]
    fn univ_ge_on_intervals() {
        // x^2 - 4 >= 0 on [3, inf) yes, on [2, inf) yes, on [1, inf) varies, on [-1, 1] no
        let c = [-4, 0, 1];
        assert_eq!(univ_ge_on(&c, Some(3), None, 1000), Some(true));
        assert_eq!(univ_ge_on(&c, Some(2), None, 1000), Some(true));
        assert_eq!(univ_ge_on(&c, Some(1), None, 1000), None);
        assert_eq!(univ_ge_on(&c, Some(-1), Some(1), 1000), Some(false));
        assert_eq!(univ_ge_on(&c, None, Some(-2), 1000), Some(true));
        // -x^3 on (-inf, 0] >= 0
        assert_eq!(univ_ge_on(&[0, 0, 0, -1], None, Some(0), 1000), Some(true));
        assert_eq!(univ_ge_on(&[0, 0, 0, -1], None, Some(1), 1000), None);
    }

    #[test]
    fn linear_feasibility() {
        let up = |l| Range {
            lo: Some(l),
            hi: None,
        };
        let dn = |h| Range {
            lo: None,
            hi: Some(h),
        };
        // n8 - n12 - 2 = 0 with n8 >= 2, n12 <= -1: n8 - n12 >= 3 -> infeasible
        assert!(!linear_feasible(&[(1, up(2)), (-1, dn(-1))], -2).unwrap());
        assert!(linear_feasible(&[(1, up(2)), (-1, dn(0))], -2).unwrap());
        // 2a - 2b + 1 = 0: parity
        assert!(!linear_feasible(&[(2, up(1)), (-2, up(1))], 1).unwrap());
        // mixed sign unbounded: a - b = 5
        assert!(linear_feasible(&[(1, up(2)), (-1, up(2))], -5).unwrap());
        // one-signed: 3a + 5b = 7 with a, b >= 0: infeasible (7 not representable)
        assert!(!linear_feasible(&[(3, up(0)), (5, up(0))], -7).unwrap());
        assert!(linear_feasible(&[(3, up(0)), (5, up(0))], -8).unwrap());
        // bounded: x in [2,3], y in [2,2]: x + y = 6 no, 5 yes
        let b = |l, h| Range {
            lo: Some(l),
            hi: Some(h),
        };
        assert!(!linear_feasible(&[(1, b(2, 3)), (1, b(2, 2))], -6).unwrap());
        assert!(linear_feasible(&[(1, b(2, 3)), (1, b(2, 2))], -5).unwrap());
        // extreme coefficients: an overflow is an error, never a verdict
        assert!(linear_feasible(&[(i128::MIN, up(1)), (1, up(1))], 0).is_err());
        assert!(linear_feasible(&[(1, up(1))], i128::MIN).is_err());
    }

    #[test]
    fn factors() {
        // (n2 - 1)(n3 - 1) = n2 n3 - n2 - n3 + 1
        let p = Poly::parse("1-n3-n2+n2*n3", 20, false).unwrap();
        let m = MPoly::from_poly(&p, 20, true).unwrap();
        let (fs, rest) = linear_factors(&m).unwrap();
        assert_eq!(fs.len(), 2, "{fs:?}");
        assert_eq!(rest.as_constant(), Some(1));
        // -2 n6 + 2 n6^2 + 2 n1 n6 - 2 n1 n6^2 + d - d n6 - d n1 + d n1 n6 = (n1 - 1)(n6 - 1)(d - 2 n6) (up to sign)
        let p = Poly::parse(
            "-2*n6+2*n6^2+2*n1*n6-2*n1*n6^2+d-d*n6-d*n1+d*n1*n6",
            20,
            true,
        )
        .unwrap();
        let m = MPoly::from_poly(&p, 20, true).unwrap();
        let (fs, rest) = linear_factors(&m).unwrap();
        assert_eq!(fs.len(), 3, "{fs:?} rest {rest:?}");
        assert!(rest.as_constant().is_some());
        // a non-unit multi-slot factor times a quadric
        let p = Poly::parse("(n0+2*n12-1)*(n1^2+n2^2+3)", 20, false).unwrap();
        let m = MPoly::from_poly(&p, 20, true).unwrap();
        let (fs, rest) = linear_factors(&m).unwrap();
        assert_eq!(fs.len(), 1);
        assert_eq!(rest.total_degree(), 2);
    }
}
