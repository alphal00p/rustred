//! Exact integer arithmetic for the partition and the loci: dense-exponent multivariate polynomials over `i128`
//! (checked; overflow is the typed error [`ExactError::Overflow`], never a wrap), exact integer roots of univariate
//! polynomials of any degree (rational-root theorem + Cauchy bound, Horner with the core's exact [`BigInt`] fallback),
//! exact integer feasibility of one linear equation on a box, extended gcd, linear-factor extraction.
//!
//! No floating point is used anywhere in this module.

use std::collections::BTreeMap;
use std::fmt;

use crate::poly::{BigInt, D_VAR, Poly};

/// Failure of an exact step. The caller treats the affected test conservatively (never as a decided verdict).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExactError {
    /// An intermediate value does not fit `i128`.
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

pub fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.unsigned_abs(), b.unsigned_abs());
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a.min(i128::MAX as u128) as i128
}

/// Extended gcd: (g, x, y) with a x + b y = g >= 0.
pub fn ext_gcd(a: i128, b: i128) -> XResult<(i128, i128, i128)> {
    let (mut r0, mut r1) = (a, b);
    let (mut s0, mut s1) = (1i128, 0i128);
    let (mut t0, mut t1) = (0i128, 1i128);
    while r1 != 0 {
        let q = r0 / r1;
        let r2 = ck(r0.checked_sub(ck(q.checked_mul(r1))?))?;
        let s2 = ck(s0.checked_sub(ck(q.checked_mul(s1))?))?;
        let t2 = ck(t0.checked_sub(ck(q.checked_mul(t1))?))?;
        r0 = r1;
        r1 = r2;
        s0 = s1;
        s1 = s2;
        t0 = t1;
        t1 = t2;
    }
    if r0 < 0 {
        Ok((ck(r0.checked_neg())?, ck(s0.checked_neg())?, ck(t0.checked_neg())?))
    } else {
        Ok((r0, s0, t0))
    }
}

/// Floor division (b != 0).
pub fn div_floor(a: i128, b: i128) -> i128 {
    let q = a / b;
    if (a % b != 0) && ((a < 0) != (b < 0)) {
        q - 1
    } else {
        q
    }
}

/// Ceiling division (b != 0).
pub fn div_ceil(a: i128, b: i128) -> i128 {
    let q = a / b;
    if (a % b != 0) && ((a < 0) == (b < 0)) {
        q + 1
    } else {
        q
    }
}

/// Positive divisors of `n > 0` by trial division; `None` above the cap (n > 10^14).
pub fn divisors(n: u128) -> Option<Vec<u128>> {
    if n == 0 || n > 100_000_000_000_000 {
        return None;
    }
    let mut small = vec![];
    let mut large = vec![];
    let mut d: u128 = 1;
    while d * d <= n {
        if n % d == 0 {
            small.push(d);
            if d * d != n {
                large.push(n / d);
            }
        }
        d += 1;
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
        return acc.signum() as i32;
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
    while n > 0 && c[n - 1] == 0 {
        n -= 1;
    }
    &c[..n]
}

/// Cauchy bound: every real root x has |x| <= 1 + max |c_i / c_n| (integer upper bound).
fn cauchy_bound(c: &[i128]) -> u128 {
    let c = trim(c);
    let lead = c[c.len() - 1].unsigned_abs();
    let mut m: u128 = 0;
    for &a in &c[..c.len() - 1] {
        m = m.max(a.unsigned_abs().div_ceil(lead));
    }
    m.saturating_add(1)
}

/// All integer roots of a nonzero univariate polynomial with |x| <= i32::MAX, ascending, exact.
pub fn int_roots(c: &[i128]) -> XResult<Vec<i64>> {
    let c = trim(c);
    if c.len() <= 1 {
        return Ok(vec![]); // nonzero constant (or zero: caller's business)
    }
    let mut roots = vec![];
    let mut k = 0;
    while c[k] == 0 {
        k += 1;
    }
    if k > 0 {
        roots.push(0);
    }
    let c = &c[k..];
    if c.len() <= 1 {
        return Ok(roots);
    }
    let bound = cauchy_bound(c).min(i32::MAX as u128);
    let c0 = c[0].unsigned_abs();
    let cands: Vec<u128> = match divisors(c0) {
        Some(d) => d.into_iter().filter(|&x| x <= bound).collect(),
        None => {
            if bound > 2_000_000 {
                return Err(ExactError::TooLarge("integer roots: constant term and root bound both large"));
            }
            (1..=bound).filter(|x| c0 % x == 0).collect()
        }
    };
    for x in cands {
        for s in [-1i128, 1] {
            let v = s * x as i128;
            if univ_sign(c, v) == 0 {
                roots.push(v as i64);
            }
        }
    }
    roots.sort_unstable();
    roots.dedup();
    Ok(roots)
}

/// The integer points where the sign of a nonconstant polynomial changes: every integer `x` with
/// `sign p(x) != sign p(x + 1)` (|x| within the Cauchy bound), exact. Used for one-slot GE literals of degree >= 2.
pub fn sign_breaks(c: &[i128]) -> XResult<Vec<i64>> {
    let c = trim(c);
    if c.len() <= 1 {
        return Ok(vec![]);
    }
    if c.len() == 2 {
        // a x + b: the sign changes between floor(-b/a) and ceil(-b/a)
        let (b, a) = (c[0], c[1]);
        let f = div_floor(-b, a);
        return Ok(vec![(f - 1) as i64, f as i64, (f + 1) as i64]);
    }
    let bound = cauchy_bound(c);
    if bound > 200_000 {
        return Err(ExactError::TooLarge("sign breaks: root bound above 2e5"));
    }
    let b = bound as i128;
    let mut out = vec![];
    let mut prev = univ_sign(c, -b - 1);
    for x in -b..=b + 1 {
        let s = univ_sign(c, x);
        if s != prev || s == 0 {
            out.push((x - 1) as i64);
            out.push(x as i64);
        }
        prev = s;
    }
    out.sort_unstable();
    out.dedup();
    Ok(out)
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
    pub fn singleton(&self) -> Option<i64> {
        match (self.lo, self.hi) {
            (Some(a), Some(b)) if a == b => Some(a),
            _ => None,
        }
    }
}

const DP_CAP: i128 = 4_000_000;

/// Set of values of `sum b_l y_l`, `0 <= y_l <= u_l` (all finite), as a bitset over `[smin, smax]`.
fn bounded_sums(vars: &[(i128, i128)]) -> XResult<(i128, Vec<bool>)> {
    let mut smin: i128 = 0;
    let mut smax: i128 = 0;
    for &(b, u) in vars {
        let v = ck(b.checked_mul(u))?;
        if v < 0 {
            smin = ck(smin.checked_add(v))?;
        } else {
            smax = ck(smax.checked_add(v))?;
        }
    }
    let width = ck(smax.checked_sub(smin))?;
    if width > DP_CAP {
        return Err(ExactError::TooLarge("linear feasibility: bounded sum range above 4e6"));
    }
    let n = (width + 1) as usize;
    let mut cur = vec![false; n];
    cur[(-smin) as usize] = true; // sum 0
    let mut work: i128 = 0;
    for &(b, u) in vars {
        work += (u + 1) * width;
        if work > 40 * DP_CAP {
            return Err(ExactError::TooLarge("linear feasibility: bounded sum DP work"));
        }
        let mut next = vec![false; n];
        for (i, &on) in cur.iter().enumerate() {
            if !on {
                continue;
            }
            for y in 0..=u {
                let j = i as i128 + b * y;
                if j >= 0 && (j as usize) < n {
                    next[j as usize] = true;
                }
            }
        }
        cur = next;
    }
    Ok((smin, cur))
}

/// Does `sum a_i x_i + c = 0` have an integer solution with every `x_i` in its range? Exact; every variable must have
/// at least one finite bound, or both infinite (treated as two half-lines). `Err` only above the work caps.
pub fn linear_feasible(terms: &[(i128, Range)], c: i128) -> XResult<bool> {
    // y >= 0 variables: (b, Some(u)) bounded, (b, None) unbounded; equation sum b y = t
    let mut t: i128 = ck(c.checked_neg())?;
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
                t = ck(t.checked_sub(ck(a.checked_mul(l as i128))?))?;
                if h > l {
                    bnd.push((a, (h - l) as i128));
                }
            }
            (Some(l), None) => {
                t = ck(t.checked_sub(ck(a.checked_mul(l as i128))?))?;
                unb.push(a);
            }
            (None, Some(h)) => {
                t = ck(t.checked_sub(ck(a.checked_mul(h as i128))?))?;
                unb.push(ck(a.checked_neg())?);
            }
            (None, None) => {
                unb.push(a);
                unb.push(ck(a.checked_neg())?);
            }
        }
    }
    let mut g = 0;
    for &(b, _) in &bnd {
        g = gcd(g, b);
    }
    for &b in &unb {
        g = gcd(g, b);
    }
    if g == 0 {
        return Ok(t == 0);
    }
    if t % g != 0 {
        return Ok(false);
    }
    t /= g;
    for x in bnd.iter_mut() {
        x.0 /= g;
    }
    for x in unb.iter_mut() {
        *x /= g;
    }
    let pos = unb.iter().any(|&b| b > 0);
    let neg = unb.iter().any(|&b| b < 0);
    if pos && neg {
        // the unbounded variables generate h Z; need bounded part = t mod h
        let mut h = 0;
        for &b in &unb {
            h = gcd(h, b);
        }
        if h > 1_000_000 {
            return Err(ExactError::TooLarge("linear feasibility: residue modulus above 1e6"));
        }
        let hm = h as usize;
        let mut reach = vec![false; hm];
        reach[0] = true;
        for &(b, u) in &bnd {
            let mut next = vec![false; hm];
            let steps = u.min(h - 1);
            for (r, &on) in reach.iter().enumerate() {
                if !on {
                    continue;
                }
                for y in 0..=steps {
                    let v = (r as i128 + (b.rem_euclid(h)) * y).rem_euclid(h);
                    next[v as usize] = true;
                }
            }
            reach = next;
        }
        return Ok(reach[t.rem_euclid(h) as usize]);
    }
    if unb.is_empty() {
        let (smin, set) = bounded_sums(&bnd)?;
        let i = t - smin;
        return Ok(i >= 0 && (i as usize) < set.len() && set[i as usize]);
    }
    // one-signed unbounded part: make it positive
    if neg {
        t = -t;
        for x in bnd.iter_mut() {
            x.0 = -x.0;
        }
        for x in unb.iter_mut() {
            *x = -*x;
        }
    }
    let (smin, set) = bounded_sums(&bnd)?;
    let mut hp = 0;
    for &b in &unb {
        hp = gcd(hp, b);
    }
    let bmin = *unb.iter().min().expect("nonempty");
    let bmax = *unb.iter().max().expect("nonempty");
    let lim = ck((bmin / hp).checked_mul(bmax / hp))?; // every multiple of hp at or above lim*hp is representable
    let lim_v = ck(lim.checked_mul(hp))?;
    if lim_v > DP_CAP {
        return Err(ExactError::TooLarge("linear feasibility: Frobenius bound above 4e6"));
    }
    // representable values below lim_v
    let mut rep = vec![false; lim_v.max(1) as usize];
    rep[0] = true;
    for v in 1..rep.len() {
        rep[v] = unb.iter().any(|&b| (v as i128) >= b && rep[v - b as usize]);
    }
    for (i, &on) in set.iter().enumerate() {
        if !on {
            continue;
        }
        let s = smin + i as i128;
        let v = t - s;
        if v < 0 || v % hp != 0 {
            continue;
        }
        if v >= lim_v || rep[v as usize] {
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
    pub fn eval(&self, t: &[i128]) -> XResult<i128> {
        let mut v = self.c;
        for (a, x) in self.coef.iter().zip(t) {
            v = ck(v.checked_add(ck(a.checked_mul(*x))?))?;
        }
        Ok(v)
    }
    pub fn is_constant(&self) -> bool {
        self.coef.iter().all(|&a| a == 0)
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
        let nv = arity + usize::from(with_d);
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
                    v as usize
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
    pub fn to_poly(&self, arity: usize) -> Poly {
        let mut acc = Poly::zero();
        for (e, &c) in &self.terms {
            let mut t = Poly::constant(c);
            for (v, &x) in e.iter().enumerate() {
                if x == 0 {
                    continue;
                }
                let base = if v == arity { Poly::d() } else { Poly::var(v) };
                t = t.mul(&base.pow(x as u64).expect("small")).expect("checked by construction");
            }
            acc = acc.add(&t).expect("checked by construction");
        }
        acc
    }
    fn add_term(&mut self, e: Vec<u32>, c: i128) -> XResult<()> {
        if c == 0 {
            return Ok(());
        }
        let ent = self.terms.entry(e).or_insert(0);
        *ent = ck(ent.checked_add(c))?;
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
            let (e, c) = self.terms.iter().next().expect("one");
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
        self.terms.keys().map(|e| e.iter().sum::<u32>()).max().unwrap_or(0)
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
            r.terms.insert(e.clone(), ck(c.checked_mul(k))?);
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
                let e: Vec<u32> = ea.iter().zip(eb).map(|(a, b)| a + b).collect();
                r.add_term(e, ck(ca.checked_mul(cb))?)?;
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
                f = ck(f.checked_mul(val))?;
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
                if !powcache.contains_key(&(v, x)) {
                    let pw = lin[v].pow(x)?;
                    powcache.insert((v, x), pw);
                }
                t = t.mul(&powcache[&(v, x)])?;
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
        let mut c = vec![0i128; self.degree_in(v) as usize + 1];
        for (e, &x) in &self.terms {
            for (w, &y) in e.iter().enumerate() {
                if w != v && y > 0 {
                    return None;
                }
            }
            c[e[v] as usize] = x;
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
                    t = ck(t.checked_mul(x[v]))?;
                }
            }
            acc = ck(acc.checked_add(t))?;
        }
        Ok(acc)
    }
    /// Content (gcd of the coefficients, >= 0).
    pub fn content(&self) -> i128 {
        self.terms.values().fold(0, |g, &c| gcd(g, c))
    }
    /// Primitive part with a positive coefficient on the largest exponent vector.
    pub fn primitive(&self) -> XResult<MPoly> {
        let g = self.content();
        if g == 0 {
            return Ok(self.clone());
        }
        let s = if *self.terms.values().next_back().expect("nonzero") < 0 { -g } else { g };
        let mut r = MPoly::zero(self.nvars);
        for (e, &c) in &self.terms {
            r.terms.insert(e.clone(), c / s);
        }
        Ok(r)
    }
    /// Coefficients in variable `v`: `P = sum_k C_k x_v^k`.
    pub fn coeffs_in(&self, v: usize) -> Vec<MPoly> {
        let mut out = vec![MPoly::zero(self.nvars); self.degree_in(v) as usize + 1];
        for (e, &c) in &self.terms {
            let k = e[v] as usize;
            let mut e2 = e.clone();
            e2[v] = 0;
            out[k].terms.insert(e2, c);
        }
        out
    }
    /// Exact division by `a x_v + m` (`m` free of `x_v`, `a != 0`): `Some(quotient)` iff it divides exactly over Z.
    pub fn div_linear(&self, v: usize, a: i128, m: &MPoly) -> XResult<Option<MPoly>> {
        let cs = self.coeffs_in(v);
        let n = cs.len() - 1;
        if n == 0 {
            return Ok(None);
        }
        // P = (a x + m) Q, Q = sum_{k<n} q_k x^k: c_n = a q_{n-1}; c_k = a q_{k-1} + m q_k; c_0 = m q_0
        let mut q: Vec<MPoly> = vec![MPoly::zero(self.nvars); n];
        let mut rem = cs[n].clone();
        for k in (1..=n).rev() {
            // rem = a q_{k-1}
            let mut qk = MPoly::zero(self.nvars);
            for (e, &c) in &rem.terms {
                if c % a != 0 {
                    return Ok(None);
                }
                qk.terms.insert(e.clone(), c / a);
            }
            q[k - 1] = qk;
            rem = cs[k - 1].sub(&m.mul(&q[k - 1])?)?;
        }
        if !rem.is_zero() {
            return Ok(None);
        }
        let mut out = MPoly::zero(self.nvars);
        for (k, qk) in q.iter().enumerate() {
            for (e, &c) in &qk.terms {
                let mut e2 = e.clone();
                e2[v] += k as u32;
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
                .map(|(v, &x)| if x == 1 { names[v].clone() } else { format!("{}^{x}", names[v]) })
                .collect();
            let (neg, a) = (c < 0, c.unsigned_abs());
            if i == 0 {
                if neg {
                    s.push('-');
                }
            } else {
                s.push_str(if neg { "-" } else { "+" });
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
    Some(d.into_iter().flat_map(|x| [x as i128, -(x as i128)]).collect())
}

/// Extract the linear (total degree 1) factors of `p` over Z exactly: `p = c * prod(factors) * cofactor`, every factor
/// primitive. Candidates come from the rational-root theorem applied recursively (the constant part in one variable
/// must be divisible by the factor's remainder); every candidate is verified by exact division, so an incomplete
/// search only leaves a reducible cofactor, never a wrong factor.
pub fn linear_factors(p: &MPoly) -> XResult<(Vec<MPoly>, MPoly)> {
    let mut rest = p.clone();
    let mut out = vec![];
    if rest.is_zero() || rest.total_degree() <= 1 {
        return Ok((out, rest));
    }
    let mut depth_guard = 0;
    'outer: loop {
        depth_guard += 1;
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
            let top = &cs[cs.len() - 1];
            let a_cands = match divisors(top.content().unsigned_abs()) {
                Some(d) => d,
                None => continue,
            };
            // candidate remainders m: +-mu, +-mu*F for F a linear factor of c0
            let mut m_cands: Vec<MPoly> = vec![];
            let cont0 = c0.content();
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
            for a in &a_cands {
                for m in &m_cands {
                    if let Some(q) = rest.div_linear(v, *a as i128, m)? {
                        let mut f = m.clone();
                        let mut e = vec![0; rest.nvars];
                        e[v] = 1;
                        f.add_term(e, *a as i128)?;
                        let g = f.content();
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
        let g = rest.content();
        let s = if *rest.terms.values().next_back().expect("nonzero") < 0 { -g } else { g };
        out.push(rest.primitive()?);
        rest = MPoly::constant(rest.nvars, s);
    }
    Ok((out, rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roots_and_breaks() {
        // (x-2)(x+3)(x-7) = x^3 - 6x^2 - 13x + 42
        assert_eq!(int_roots(&[42, -13, -6, 1]).unwrap(), vec![-3, 2, 7]);
        assert_eq!(int_roots(&[0, 0, 5]).unwrap(), vec![0]);
        assert_eq!(int_roots(&[-263, 159, 42]).unwrap(), Vec::<i64>::new());
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
    fn linear_feasibility() {
        let up = |l| Range { lo: Some(l), hi: None };
        let dn = |h| Range { lo: None, hi: Some(h) };
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
        let b = |l, h| Range { lo: Some(l), hi: Some(h) };
        assert!(!linear_feasible(&[(1, b(2, 3)), (1, b(2, 2))], -6).unwrap());
        assert!(linear_feasible(&[(1, b(2, 3)), (1, b(2, 2))], -5).unwrap());
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
