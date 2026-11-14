//! [`Referee`]: the THIRD OPINION of the triage (rows4, re-audit RA3-M2). When the independent and the Symbolica runs
//! first differ at a quantity, the triage reruns the rule on this backend and names the side whose value the referee
//! reproduces. To be a real third opinion it shares NOTHING with either side beyond the trait definition and `std`:
//! no `independent_algebra` item, no Symbolica / numerica item. Every routine is its own, by a route chosen to differ
//! from both sides where that is cheap:
//!
//! | quantity | referee route | independent side | Symbolica side |
//! |---|---|---|---|
//! | primality | BPSW: strong probable prime to base 2 + strong Lucas test (Selfridge parameters) | Miller-Rabin, 7 fixed bases | Miller-Rabin (numerica) |
//! | field | u128 products, inverse by the extended Euclidean algorithm | u128 products, Fermat inverse | Montgomery `Zp64` |
//! | rationals, RREF, inverse, rank, unit test | own i128 rationals, one Gauss-Jordan routine | own checked i128 + linalg | `Rational`, `Matrix<Q>` |
//! | integer determinant | Bareiss (fraction-free, i128) | Laplace / own | `Matrix<Z>::det` |
//! | U, F determinants | Laplace over column bit masks on own sparse polynomials | Laplace (own) | `Matrix<PolynomialRing>::det` |
//! | coefficient texts, expressions, case literals | one recursive-descent evaluator over three value types (fractions mod p per context, exact polynomials over Q, degrees) | streaming reader + engine tree reader | atom parser + rational polynomials |
//! | elimination | sparse echelon form with a left-to-right sweep (no certificate weights: the referee never certifies) | copied engine `Elim` | `SparseRowReducer` |
//!
//! Size: the non-test line count of this file is asserted by `tests/rows4_referee.rs` (`referee_is_small`). The referee
//! never certifies anything: it only decides which side of a `Differ` is the outlier.

use std::collections::{BTreeMap, HashMap};

use rustred_campaign_core::replay_base::algebra::{
    CheckAlgebra, CoeffError, CoeffStats, Ctx, Eliminator, MEMORY_GUARD, Q, QResult, SideStats,
    Span, Stage, Unknown, ZPoly,
};

/// The referee covers every stage (rows4: no stage delegates to either side).
pub fn referee_covers(_stage: Stage) -> bool {
    true
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Referee;

// ------------------------------------------------------------------ field --------------------------------------------

pub fn fmul(p: u64, a: u64, b: u64) -> u64 {
    (u128::from(a) * u128::from(b) % u128::from(p)) as u64
}
pub fn fadd(p: u64, a: u64, b: u64) -> u64 {
    ((u128::from(a) + u128::from(b)) % u128::from(p)) as u64
}
pub fn fsub(p: u64, a: u64, b: u64) -> u64 {
    fadd(p, a % p, p - b % p)
}
pub fn fred(p: u64, x: i128) -> u64 {
    x.rem_euclid(i128::from(p)) as u64
}
/// inverse by the extended Euclidean algorithm (`None` for 0)
pub fn finv(p: u64, a: u64) -> Option<u64> {
    let (mut r0, mut r1, mut s0, mut s1) = (i128::from(p), i128::from(a % p), 0i128, 1i128);
    while r1 != 0 {
        let q = r0 / r1;
        (r0, r1) = (r1, r0 - q * r1);
        (s0, s1) = (s1, s0 - q * s1);
    }
    (r0 == 1).then(|| fred(p, s0))
}

// ------------------------------------------------------------------ primality (BPSW) ---------------------------------

fn fpow(p: u64, mut a: u64, mut e: u64) -> u64 {
    let mut r = 1 % p;
    while e > 0 {
        if e & 1 == 1 {
            r = fmul(p, r, a);
        }
        a = fmul(p, a, a);
        e >>= 1;
    }
    r
}

fn jacobi(mut a: i128, mut n: i128) -> i32 {
    a = a.rem_euclid(n);
    let mut t = 1;
    while a != 0 {
        while a % 2 == 0 {
            a /= 2;
            if n % 8 == 3 || n % 8 == 5 {
                t = -t;
            }
        }
        (a, n) = (n, a);
        if a % 4 == 3 && n % 4 == 3 {
            t = -t;
        }
        a %= n;
    }
    if n == 1 { t } else { 0 }
}

/// Baillie-PSW: trial division, strong probable prime to base 2, strong Lucas probable prime (no known counterexample;
/// proven correct below 2^64 by exhaustive search, Feitsma / Gilchrist).
pub fn is_prime(n: u64) -> bool {
    for q in [2u64, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47] {
        if n % q == 0 {
            return n == q;
        }
    }
    if n < 2 {
        return false;
    }
    let (mut d, mut s) = (n - 1, 0);
    while d % 2 == 0 {
        d /= 2;
        s += 1;
    }
    let mut x = fpow(n, 2, d);
    if x != 1 && x != n - 1 {
        let mut ok = false;
        for _ in 1..s {
            x = fmul(n, x, x);
            if x == n - 1 {
                ok = true;
                break;
            }
        }
        if !ok {
            return false;
        }
    }
    let r = (n as f64).sqrt() as u64;
    if (r.saturating_sub(1)..=r + 1).any(|k| k.checked_mul(k) == Some(n)) {
        return false;
    }
    // Selfridge: first D in 5, -7, 9, -11, ... with (D/n) = -1; P = 1, Q = (1 - D)/4
    let mut dd: i128 = 5;
    while jacobi(dd, i128::from(n)) != -1 {
        dd = if dd > 0 { -dd - 2 } else { -dd + 2 };
    }
    let qq = fred(n, (1 - dd) / 4);
    let dm = fred(n, dd);
    let inv2 = finv(n, 2).unwrap_or(0);
    // n + 1 = d2 2^s2; U_k, V_k by the binary method (k from the top bit)
    let (mut d2, mut s2) = (u128::from(n) + 1, 0);
    while d2 % 2 == 0 {
        d2 /= 2;
        s2 += 1;
    }
    let (mut u, mut v, mut qk) = (0u64, 2 % n, 1u64);
    for bit in (0..128 - d2.leading_zeros()).rev() {
        // double: U_2k = U V, V_2k = V^2 - 2 Q^k
        (u, v) = (fmul(n, u, v), fsub(n, fmul(n, v, v), fmul(n, 2, qk)));
        qk = fmul(n, qk, qk);
        if (d2 >> bit) & 1 == 1 {
            // add one: U_{k+1} = (U + V)/2, V_{k+1} = (D U + V)/2 (P = 1)
            (u, v) = (
                fmul(n, fadd(n, u, v), inv2),
                fmul(n, fadd(n, fmul(n, dm, u), v), inv2),
            );
            qk = fmul(n, qk, qq);
        }
    }
    if u == 0 || v == 0 {
        return true;
    }
    for _ in 1..s2 {
        v = fsub(n, fmul(n, v, v), fmul(n, 2, qk));
        qk = fmul(n, qk, qk);
        if v == 0 {
            return true;
        }
    }
    false
}

// ------------------------------------------------------------------ rationals ----------------------------------------

const OVF: &str = "referee: rational overflow";

pub fn gcd(a: i128, b: i128) -> i128 {
    // binary gcd (Stein)
    let (mut a, mut b) = (a.unsigned_abs(), b.unsigned_abs());
    if a == 0 || b == 0 {
        return (a | b) as i128;
    }
    let k = (a | b).trailing_zeros();
    a >>= a.trailing_zeros();
    while b != 0 {
        b >>= b.trailing_zeros();
        if a > b {
            (a, b) = (b, a);
        }
        b -= a;
    }
    (a << k) as i128
}
pub fn qn(n: i128, d: i128) -> QResult {
    if d == 0 {
        return Err("referee: division by zero".into());
    }
    let g = gcd(n, d).max(1);
    let s = if d < 0 { -1 } else { 1 };
    Ok(Q {
        n: (n / g).checked_mul(s).ok_or(OVF)?,
        d: (d / g).checked_mul(s).ok_or(OVF)?,
    })
}
pub fn qadd(a: Q, b: Q) -> QResult {
    let g = gcd(a.d, b.d).max(1);
    let (x, y) = (b.d / g, a.d / g);
    qn(
        a.n.checked_mul(x)
            .and_then(|u| b.n.checked_mul(y).and_then(|v| u.checked_add(v)))
            .ok_or(OVF)?,
        a.d.checked_mul(x).ok_or(OVF)?,
    )
}
pub fn qmul(a: Q, b: Q) -> QResult {
    let (g1, g2) = (gcd(a.n, b.d).max(1), gcd(b.n, a.d).max(1));
    qn(
        (a.n / g1).checked_mul(b.n / g2).ok_or(OVF)?,
        (a.d / g2).checked_mul(b.d / g1).ok_or(OVF)?,
    )
}
pub fn qneg(a: Q) -> QResult {
    Ok(Q {
        n: a.n.checked_neg().ok_or(OVF)?,
        d: a.d,
    })
}
pub fn qdiv(a: Q, b: Q) -> QResult {
    if b.n == 0 {
        return Err("referee: division by zero".into());
    }
    qmul(a, qn(b.d, b.n)?)
}

/// Gauss-Jordan over Q on the first `ncols` columns (the rest carried): (RREF rows, pivot columns).
pub fn rref(rows: &[Vec<Q>], ncols: usize) -> Result<(Vec<Vec<Q>>, Vec<usize>), Unknown> {
    let mut m = rows.to_vec();
    let mut piv = Vec::new();
    for c in 0..ncols {
        let r = piv.len();
        let Some(k) = (r..m.len()).find(|&k| !m[k][c].is_zero()) else {
            continue;
        };
        m.swap(r, k);
        let inv = qdiv(Q::ONE, m[r][c])?;
        m[r] = m[r]
            .iter()
            .map(|&x| qmul(x, inv))
            .collect::<Result<_, _>>()?;
        for k in 0..m.len() {
            if k != r && !m[k][c].is_zero() {
                let f = m[k][c];
                for j in 0..m[k].len() {
                    m[k][j] = qadd(m[k][j], qneg(qmul(f, m[r][j])?)?)?;
                }
            }
        }
        piv.push(c);
    }
    Ok((m, piv))
}

/// Bareiss fraction-free determinant over i128 (`None` on overflow).
pub fn det_bareiss(m: &[Vec<i128>]) -> Option<i128> {
    let n = m.len();
    let mut a = m.to_vec();
    let (mut sign, mut prev) = (1i128, 1i128);
    for k in 0..n {
        if a[k][k] == 0 {
            let Some(s) = (k + 1..n).find(|&i| a[i][k] != 0) else {
                return Some(0);
            };
            a.swap(k, s);
            sign = -sign;
        }
        for i in k + 1..n {
            for j in k + 1..n {
                let v = a[i][j]
                    .checked_mul(a[k][k])?
                    .checked_sub(a[i][k].checked_mul(a[k][j])?)?;
                a[i][j] = v / prev;
            }
        }
        prev = a[k][k];
    }
    Some(if n == 0 { 1 } else { sign * a[n - 1][n - 1] })
}

fn zp_mul(a: &ZPoly, b: &ZPoly) -> ZPoly {
    let mut out = ZPoly::new();
    for (e1, c1) in a {
        for (e2, c2) in b {
            let e: Vec<u8> = e1.iter().zip(e2).map(|(x, y)| x + y).collect();
            *out.entry(e).or_insert(0) += c1 * c2;
        }
    }
    out.retain(|_, c| *c != 0);
    out
}

/// Laplace expansion along the rows, memoised on the bit mask of the columns left.
fn zp_det(
    m: &[Vec<ZPoly>],
    row: usize,
    cols: u64,
    dim: usize,
    t: usize,
    memo: &mut HashMap<(usize, u64), ZPoly>,
) -> ZPoly {
    if row == dim {
        return [(vec![0u8; t], 1i128)].into_iter().collect();
    }
    if let Some(v) = memo.get(&(row, cols)) {
        return v.clone();
    }
    let mut out = ZPoly::new();
    let mut sign = 1i128;
    for c in (0..dim).filter(|&c| cols >> c & 1 == 1) {
        if !m[row][c].is_empty() {
            let sub = zp_det(m, row + 1, cols & !(1 << c), dim, t, memo);
            for (e, v) in zp_mul(&m[row][c], &sub) {
                *out.entry(e).or_insert(0) += sign * v;
            }
        }
        sign = -sign;
    }
    out.retain(|_, c| *c != 0);
    memo.insert((row, cols), out.clone());
    out
}

// ------------------------------------------------------------------ expressions --------------------------------------

/// A value of the expression evaluator.
trait Val: Sized {
    fn int(digits: &[u8], cx: &Cx) -> Result<Self, Unknown>;
    fn var(i: usize, cx: &Cx) -> Result<Self, Unknown>;
    fn add(self, o: Self, neg: bool, cx: &Cx) -> Result<Self, Unknown>;
    fn mul(self, o: Self, div: bool, cx: &Cx) -> Result<Self, Unknown>;
    fn neg(self, cx: &Cx) -> Result<Self, Unknown>;
    fn pow(self, k: i64, cx: &Cx) -> Result<Self, Unknown> {
        let (mut r, mut b, mut e) = (Self::int(b"1", cx)?, self, k.unsigned_abs());
        while e > 0 {
            if e & 1 == 1 {
                r = r.mul(b.dup(), false, cx)?;
            }
            e >>= 1;
            if e > 0 {
                b = b.dup().mul(b, false, cx)?;
            }
        }
        if k < 0 {
            Self::int(b"1", cx)?.mul(r, true, cx)
        } else {
            Ok(r)
        }
    }
    fn dup(&self) -> Self;
}

/// The evaluation context: the arity and the (prime, point) contexts of a coefficient.
struct Cx<'a> {
    n: usize,
    ctxs: &'a [Ctx],
}

/// Fractions (num, den) mod p per context, plus syntactic degrees (num total, num in d, den total, den in d).
struct Frac(Vec<(u64, u64)>, [u32; 4]);
impl Val for Frac {
    fn int(ds: &[u8], cx: &Cx) -> Result<Self, Unknown> {
        let v = cx.ctxs.iter().map(|c| {
            let x = ds.iter().fold(0u64, |a, &b| {
                fadd(c.p, fmul(c.p, a, 10), u64::from(b - b'0'))
            });
            (x, 1 % c.p)
        });
        Ok(Frac(v.collect(), [0; 4]))
    }
    fn var(i: usize, cx: &Cx) -> Result<Self, Unknown> {
        let dg = if i == cx.n {
            [1, 1, 0, 0]
        } else {
            [1, 0, 0, 0]
        };
        Ok(Frac(
            cx.ctxs.iter().map(|c| (c.vals[i] % c.p, 1 % c.p)).collect(),
            dg,
        ))
    }
    fn add(self, o: Self, neg: bool, cx: &Cx) -> Result<Self, Unknown> {
        let (a, b) = (self.1, o.1);
        let dg = [
            (a[0] + b[2]).max(b[0] + a[2]),
            (a[1] + b[3]).max(b[1] + a[3]),
            a[2] + b[2],
            a[3] + b[3],
        ];
        let v = self
            .0
            .iter()
            .zip(&o.0)
            .zip(cx.ctxs)
            .map(|((&(x, y), &(u, w)), c)| {
                let p = c.p;
                let (l, r) = (fmul(p, x, w), fmul(p, u, y));
                (
                    if neg { fsub(p, l, r) } else { fadd(p, l, r) },
                    fmul(p, y, w),
                )
            });
        Ok(Frac(v.collect(), dg))
    }
    fn mul(self, o: Self, div: bool, cx: &Cx) -> Result<Self, Unknown> {
        let (a, b) = (self.1, o.1);
        let dg = if div {
            [a[0] + b[2], a[1] + b[3], a[2] + b[0], a[3] + b[1]]
        } else {
            [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
        };
        let v = self
            .0
            .iter()
            .zip(&o.0)
            .zip(cx.ctxs)
            .map(|((&(x, y), &(u, w)), c)| {
                let p = c.p;
                if div {
                    (fmul(p, x, w), fmul(p, y, u))
                } else {
                    (fmul(p, x, u), fmul(p, y, w))
                }
            });
        Ok(Frac(v.collect(), dg))
    }
    fn neg(self, cx: &Cx) -> Result<Self, Unknown> {
        Ok(Frac(
            self.0
                .iter()
                .zip(cx.ctxs)
                .map(|(&(x, y), c)| (fsub(c.p, 0, x), y))
                .collect(),
            self.1,
        ))
    }
    fn dup(&self) -> Self {
        Frac(self.0.clone(), self.1)
    }
}

/// Exact polynomials over Q in n0..n{N-1}, d (exponent vector -> coefficient).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolyQ(BTreeMap<Vec<u32>, Q>);
impl PolyQ {
    fn constant(c: Q, n: usize) -> PolyQ {
        PolyQ(if c.is_zero() {
            BTreeMap::new()
        } else {
            [(vec![0; n + 1], c)].into()
        })
    }
    fn as_const(&self) -> Option<Q> {
        match self.0.iter().next() {
            None => Some(Q::ZERO),
            Some((e, c)) if self.0.len() == 1 && e.iter().all(|&x| x == 0) => Some(*c),
            _ => None,
        }
    }
}
impl Val for PolyQ {
    fn int(ds: &[u8], cx: &Cx) -> Result<Self, Unknown> {
        let s = std::str::from_utf8(ds).map_err(|e| e.to_string())?;
        Ok(PolyQ::constant(
            Q::int(s.parse::<i128>().map_err(|_| OVF)?),
            cx.n,
        ))
    }
    fn var(i: usize, cx: &Cx) -> Result<Self, Unknown> {
        let mut e = vec![0; cx.n + 1];
        e[i] = 1;
        Ok(PolyQ([(e, Q::ONE)].into()))
    }
    fn add(mut self, o: Self, neg: bool, _: &Cx) -> Result<Self, Unknown> {
        for (e, c) in o.0 {
            let c = if neg { qneg(c)? } else { c };
            let v = qadd(self.0.get(&e).copied().unwrap_or(Q::ZERO), c)?;
            self.0.insert(e, v);
        }
        self.0.retain(|_, c| !c.is_zero());
        Ok(self)
    }
    fn mul(self, o: Self, div: bool, _: &Cx) -> Result<Self, Unknown> {
        if div {
            let c = o
                .as_const()
                .filter(|c| !c.is_zero())
                .ok_or("referee: division by a non-constant")?;
            let inv = qdiv(Q::ONE, c)?;
            return Ok(PolyQ(
                self.0
                    .into_iter()
                    .map(|(e, x)| Ok((e, qmul(x, inv)?)))
                    .collect::<Result<_, Unknown>>()?,
            ));
        }
        let mut out = PolyQ(BTreeMap::new());
        for (e1, c1) in &self.0 {
            for (e2, c2) in &o.0 {
                let e: Vec<u32> = e1.iter().zip(e2).map(|(a, b)| a + b).collect();
                let v = qadd(out.0.get(&e).copied().unwrap_or(Q::ZERO), qmul(*c1, *c2)?)?;
                out.0.insert(e, v);
            }
        }
        out.0.retain(|_, c| !c.is_zero());
        if out.0.len() > 100_000 {
            return Err("referee: polynomial too large".into());
        }
        Ok(out)
    }
    fn neg(self, _: &Cx) -> Result<Self, Unknown> {
        Ok(PolyQ(
            self.0
                .into_iter()
                .map(|(e, c)| Ok((e, qneg(c)?)))
                .collect::<Result<_, Unknown>>()?,
        ))
    }
    fn dup(&self) -> Self {
        self.clone()
    }
}

/// Recursive descent: sum := prod (('+'|'-') prod)*, prod := unary (('*'|'/') unary)*, unary := '-' unary | '+' unary |
/// pow, pow := atom ('^' unary)? (right associative, `-a^2 = -(a^2)`), atom := digits | n<i> | d | '(' sum ')'.
struct Parser<'s> {
    s: &'s [u8],
    i: usize,
    saw_d: bool,
}
impl Parser<'_> {
    fn peek(&mut self) -> Option<u8> {
        while self.s.get(self.i).is_some_and(u8::is_ascii_whitespace) {
            self.i += 1;
        }
        self.s.get(self.i).copied()
    }
    fn sum<V: Val>(&mut self, cx: &Cx) -> Result<V, Unknown> {
        let mut v: V = self.prod(cx)?;
        while let Some(c @ (b'+' | b'-')) = self.peek() {
            self.i += 1;
            v = v.add(self.prod(cx)?, c == b'-', cx)?;
        }
        Ok(v)
    }
    fn prod<V: Val>(&mut self, cx: &Cx) -> Result<V, Unknown> {
        let mut v: V = self.unary(cx)?;
        while let Some(c @ (b'*' | b'/')) = self.peek() {
            self.i += 1;
            v = v.mul(self.unary(cx)?, c == b'/', cx)?;
        }
        Ok(v)
    }
    fn unary<V: Val>(&mut self, cx: &Cx) -> Result<V, Unknown> {
        match self.peek() {
            Some(b'-') => {
                self.i += 1;
                self.unary::<V>(cx)?.neg(cx)
            }
            Some(b'+') => {
                self.i += 1;
                self.unary(cx)
            }
            _ => {
                let base: V = self.atom(cx)?;
                if self.peek() != Some(b'^') {
                    return Ok(base);
                }
                self.i += 1;
                let k = self
                    .unary::<PolyQ>(&Cx { n: cx.n, ctxs: &[] })?
                    .as_const()
                    .filter(|q| q.d == 1);
                let k = k
                    .and_then(|q| i64::try_from(q.n).ok())
                    .filter(|k| k.abs() <= 100_000)
                    .ok_or("referee: exponent")?;
                base.pow(k, cx)
            }
        }
    }
    fn atom<V: Val>(&mut self, cx: &Cx) -> Result<V, Unknown> {
        let start = self.i;
        match self.peek() {
            Some(b'(') => {
                self.i += 1;
                let v = self.sum(cx)?;
                if self.peek() != Some(b')') {
                    return Err(format!("referee: ')' expected at byte {}", self.i));
                }
                self.i += 1;
                Ok(v)
            }
            Some(c) if c.is_ascii_digit() => {
                let s = self.i;
                while self.s.get(self.i).is_some_and(u8::is_ascii_digit) {
                    self.i += 1;
                }
                V::int(&self.s[s..self.i], cx)
            }
            Some(b'd') => {
                self.i += 1;
                self.saw_d = true;
                V::var(cx.n, cx)
            }
            Some(b'n') => {
                self.i += 1;
                let s = self.i;
                while self.s.get(self.i).is_some_and(u8::is_ascii_digit) {
                    self.i += 1;
                }
                let k: usize = std::str::from_utf8(&self.s[s..self.i])
                    .ok()
                    .and_then(|x| x.parse().ok())
                    .ok_or("referee: variable")?;
                if k >= cx.n {
                    return Err(format!("referee: n{k} outside the arity"));
                }
                V::var(k, cx)
            }
            _ => Err(format!("referee: unexpected input at byte {start}")),
        }
    }
}

fn parse_all<V: Val>(text: &str, cx: &Cx) -> Result<(V, bool), Unknown> {
    let mut p = Parser {
        s: text.as_bytes(),
        i: 0,
        saw_d: false,
    };
    let v = p.sum(cx)?;
    if p.peek().is_some() {
        return Err(format!("referee: trailing input at byte {}", p.i));
    }
    Ok((v, p.saw_d))
}

/// A case literal of the referee (exact polynomial over Q, d-free).
pub fn literal(text: &str, n: usize) -> Result<PolyQ, Unknown> {
    match parse_all::<PolyQ>(text, &Cx { n, ctxs: &[] })? {
        (_, true) => Err("referee: case literal contains d".into()),
        (p, false) => Ok(p),
    }
}

/// Exact value of a polynomial at an integer point (d = 0).
pub fn poly_at(p: &PolyQ, point: &[i64]) -> QResult {
    let mut acc = Q::ZERO;
    for (e, c) in &p.0 {
        let mut t = *c;
        for (&x, &k) in point.iter().zip(e) {
            for _ in 0..k {
                t = qmul(t, Q::int(i128::from(x)))?;
            }
        }
        acc = qadd(acc, t)?;
    }
    Ok(acc)
}

/// Evaluate a coefficient text at every context (`num`, `den` mod p; the degrees as syntactic stats).
pub fn eval_text(
    text: &str,
    arity: usize,
    ctxs: &[Ctx],
    num: &mut [u64],
    den: &mut [u64],
) -> Result<CoeffStats, Unknown> {
    let (f, _) = parse_all::<Frac>(text, &Cx { n: arity, ctxs })?;
    for (k, &(x, y)) in f.0.iter().enumerate() {
        (num[k], den[k]) = (x, y);
    }
    let side = |deg: u32, degd: u32| SideStats {
        deg,
        degd,
        degn: deg - degd,
        ..SideStats::default()
    };
    Ok(CoeffStats {
        num: side(f.1[0], f.1[1]),
        den: side(f.1[2], f.1[3]),
        has_den: f.1[2] > 0,
    })
}

// ------------------------------------------------------------------ elimination --------------------------------------

/// Sparse echelon form mod p: pivot rows (pivot entry 1, entries only at columns >= the pivot), in elimination order.
pub struct RefElim {
    p: u64,
    piv: BTreeMap<usize, Vec<(usize, u64)>>,
    order: Vec<usize>,
    entries: usize,
}

impl RefElim {
    pub fn new(rows: &[Vec<(usize, u64)>], p: u64, max_entries: usize) -> Result<RefElim, Unknown> {
        let mut e = RefElim {
            p,
            piv: BTreeMap::new(),
            order: Vec::new(),
            entries: 0,
        };
        for r in rows {
            let rem = e.reduce(r);
            if let Some((&c, &lead)) = rem.iter().next() {
                let inv = finv(p, lead).ok_or("referee: zero pivot")?;
                let row: Vec<(usize, u64)> =
                    rem.iter().map(|(&j, &v)| (j, fmul(p, v, inv))).collect();
                e.entries += row.len();
                if e.entries > max_entries {
                    return Err(format!(
                        "{MEMORY_GUARD}: referee {} entries > {max_entries}",
                        e.entries
                    ));
                }
                e.piv.insert(c, row);
                e.order.push(c);
            }
        }
        Ok(e)
    }
    /// left-to-right sweep: subtract the pivot row at every pivot column met (a pivot row touches only columns to its
    /// right, so one sweep reduces fully)
    fn reduce(&self, v: &[(usize, u64)]) -> BTreeMap<usize, u64> {
        let p = self.p;
        let mut m: BTreeMap<usize, u64> = v
            .iter()
            .filter(|x| x.1 % p != 0)
            .map(|&(c, x)| (c, x % p))
            .collect();
        let mut cur = 0usize;
        while let Some((&c, &x)) = m.range(cur..).next() {
            if x != 0
                && let Some(row) = self.piv.get(&c)
            {
                for &(j, y) in row {
                    let e = m.entry(j).or_insert(0);
                    *e = fsub(p, *e, fmul(p, x, y));
                }
            }
            cur = c + 1;
        }
        m.retain(|_, y| *y != 0);
        m
    }
}

impl Eliminator for RefElim {
    fn rank(&self) -> usize {
        self.order.len()
    }
    fn entries(&self) -> usize {
        self.entries
    }
    fn pivot_columns(&self) -> Vec<usize> {
        self.order.clone()
    }
    fn solve(&self, desired: &[(usize, u64)]) -> Result<Span, Unknown> {
        let remainder: Vec<(usize, u64)> = self.reduce(desired).into_iter().collect();
        Ok(Span {
            in_span: remainder.is_empty(),
            rank: self.order.len(),
            weights: BTreeMap::new(),
            remainder,
            pivot_rows: Vec::new(),
        })
    }
}

// ------------------------------------------------------------------ the trait ----------------------------------------

fn rank_q(rows: &[Vec<Q>]) -> Result<usize, Unknown> {
    let w = rows.first().map_or(0, Vec::len);
    Ok(rref(rows, w)?.1.len())
}

impl CheckAlgebra for Referee {
    type Literal = PolyQ;
    type Elim<'r> = RefElim;

    fn name(&self) -> &'static str {
        "referee"
    }
    fn is_prime(&self, n: u64) -> bool {
        is_prime(n)
    }
    fn add(&self, p: u64, a: u64, b: u64) -> u64 {
        fadd(p, a, b)
    }
    fn sub(&self, p: u64, a: u64, b: u64) -> u64 {
        fsub(p, a, b)
    }
    fn mul(&self, p: u64, a: u64, b: u64) -> u64 {
        fmul(p, a, b)
    }
    fn neg(&self, p: u64, a: u64) -> u64 {
        fsub(p, 0, a)
    }
    fn inv(&self, p: u64, a: u64) -> Option<u64> {
        finv(p, a)
    }
    fn from_i128(&self, p: u64, x: i128) -> u64 {
        fred(p, x)
    }
    fn q_new(&self, n: i128, d: i128) -> QResult {
        qn(n, d)
    }
    fn q_add(&self, a: Q, b: Q) -> QResult {
        qadd(a, b)
    }
    fn q_sub(&self, a: Q, b: Q) -> QResult {
        qadd(a, qneg(b)?)
    }
    fn q_mul(&self, a: Q, b: Q) -> QResult {
        qmul(a, b)
    }
    fn q_div(&self, a: Q, b: Q) -> QResult {
        qdiv(a, b)
    }
    fn q_neg(&self, a: Q) -> QResult {
        qneg(a)
    }
    fn q_parse(&self, s: &str) -> QResult {
        let (a, b) = s.split_once('/').unwrap_or((s, "1"));
        let num = |x: &str| {
            x.trim()
                .parse::<i128>()
                .map_err(|_| format!("referee: rational {s}"))
        };
        qn(num(a)?, num(b)?)
    }
    fn q_modp(&self, a: Q, p: u64) -> Option<u64> {
        finv(p, fred(p, a.d)).map(|i| fmul(p, fred(p, a.n), i))
    }
    fn q_inverse(&self, m: &[Vec<Q>]) -> Result<Vec<Vec<Q>>, Unknown> {
        let n = m.len();
        let aug: Vec<Vec<Q>> = m
            .iter()
            .enumerate()
            .map(|(i, r)| {
                r.iter()
                    .copied()
                    .chain((0..n).map(|j| if i == j { Q::ONE } else { Q::ZERO }))
                    .collect()
            })
            .collect();
        let (r, piv) = rref(&aug, n)?;
        if piv.len() < n {
            return Err("referee: singular matrix".into());
        }
        Ok(r.into_iter().map(|row| row[n..].to_vec()).collect())
    }
    fn q_rank(&self, rows: &[Vec<Q>]) -> Result<usize, Unknown> {
        rank_q(rows)
    }
    fn det_i128(&self, m: &[Vec<i128>]) -> Option<i128> {
        det_bareiss(m)
    }
    fn q_rref(&self, rows: &[Vec<Q>], ncols: usize) -> Result<(Vec<Vec<Q>>, Vec<usize>), Unknown> {
        rref(rows, ncols)
    }
    fn q_unit_in_row_space(&self, rows: &[Vec<Q>], col: usize) -> Result<bool, Unknown> {
        let Some(w) = rows.first().map(Vec::len).filter(|&w| col < w) else {
            return Ok(false);
        };
        let mut with = rows.to_vec();
        with.push(
            (0..w)
                .map(|j| if j == col { Q::ONE } else { Q::ZERO })
                .collect(),
        );
        Ok(rank_q(&with)? == rank_q(rows)?)
    }
    fn gcd_i128(&self, a: i128, b: i128) -> i128 {
        gcd(a, b)
    }
    fn int_mat_mul(
        &self,
        a: &[i128],
        b: &[i128],
        r: usize,
        k: usize,
        c: usize,
    ) -> Option<Vec<i128>> {
        if a.len() != r * k || b.len() != k * c {
            return None;
        }
        (0..r * c)
            .map(|x| {
                (0..k).try_fold(0i128, |s, m| {
                    s.checked_add(a[x / c * k + m].checked_mul(b[m * c + x % c])?)
                })
            })
            .collect()
    }
    fn int_div_exact(&self, a: i128, b: i128) -> Option<i128> {
        (b != 0 && a.checked_rem(b)? == 0).then(|| a / b)
    }
    fn zpoly_det(&self, m: &[Vec<ZPoly>], dim: usize, t: usize) -> ZPoly {
        zp_det(m, 0, (1u64 << dim) - 1, dim, t, &mut HashMap::new())
    }
    fn eval_ratio(
        &self,
        text: &str,
        arity: usize,
        ctxs: &[Ctx],
        num: &mut [u64],
        den: &mut [u64],
    ) -> Result<CoeffStats, CoeffError> {
        eval_text(text, arity, ctxs, num, den).map_err(|msg| CoeffError { pos: 0, msg })
    }
    fn cross_check_coefficient(
        &self,
        _: &str,
        _: usize,
        _: &Ctx,
        _: u64,
        _: u64,
    ) -> Result<(), Unknown> {
        Ok(())
    }
    fn expr_linear(&self, text: &str, n: usize) -> Result<Vec<Q>, Unknown> {
        let (p, _) = parse_all::<PolyQ>(text, &Cx { n, ctxs: &[] })?;
        let mut out = vec![Q::ZERO; n + 2];
        for (e, c) in &p.0 {
            match e.iter().sum::<u32>() {
                0 => out[0] = *c,
                1 => out[1 + e.iter().position(|&x| x == 1).unwrap_or(0)] = *c,
                _ => return Err(format!("referee: {text} is not affine")),
            }
        }
        Ok(out)
    }
    fn expr_has_d(&self, text: &str, n: usize) -> Result<bool, Unknown> {
        Ok(parse_all::<PolyQ>(text, &Cx { n, ctxs: &[] })?.1)
    }
    fn literal_parse(&self, text: &str, n: usize) -> Result<PolyQ, Unknown> {
        literal(text, n)
    }
    fn literal_is_zero_at(&self, lit: &PolyQ, point: &[i32]) -> Result<bool, Unknown> {
        let pt: Vec<i64> = point.iter().map(|&x| i64::from(x)).collect();
        Ok(poly_at(lit, &pt)?.is_zero())
    }
    fn literal_same_up_to_constant(&self, a: &str, b: &str, n: usize) -> bool {
        let (Ok(x), Ok(y)) = (literal(a, n), literal(b, n)) else {
            return false;
        };
        let (Some(ca), Some(cb)) = (x.0.values().next(), y.0.values().next()) else {
            return x.0.is_empty() && y.0.is_empty();
        };
        x.0.len() == y.0.len()
            && x.0
                .iter()
                .zip(&y.0)
                .all(|((e1, c1), (e2, c2))| e1 == e2 && qmul(*c1, *cb).ok() == qmul(*c2, *ca).ok())
    }
    fn eliminate<'r>(
        &self,
        rows: &'r [Vec<(usize, u64)>],
        _ncols: usize,
        p: u64,
        max_entries: usize,
    ) -> Result<RefElim, Unknown> {
        RefElim::new(rows, p, max_entries)
    }
}
