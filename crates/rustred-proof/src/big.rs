//! Sign-magnitude big integers on u64 limbs (std only).
//!
//! The add/sub/mul core is the H8 kernel's (handoff/helper-h10-kernel, reviewed with its tests); division,
//! gcd and the small helpers are added for the per-instance span mode (CRT, rational reconstruction, exact
//! polynomial checks). Hot loops reuse caller buffers (`mag_mul_into`) and never allocate per term.

use std::cmp::Ordering;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct Big {
    pub neg: bool,
    pub m: Vec<u64>,
}

pub fn trim(m: &mut Vec<u64>) {
    while let Some(&0) = m.last() {
        m.pop();
    }
}
pub fn mag_cmp(a: &[u64], b: &[u64]) -> Ordering {
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
pub fn mag_add_into(acc: &mut Vec<u64>, b: &[u64]) {
    if acc.len() < b.len() {
        acc.resize(b.len(), 0);
    }
    let mut c = 0u64;
    for i in 0..acc.len() {
        if i >= b.len() && c == 0 {
            break;
        }
        let bi = if i < b.len() { b[i] } else { 0 };
        let (s1, o1) = acc[i].overflowing_add(bi);
        let (s2, o2) = s1.overflowing_add(c);
        acc[i] = s2;
        c = (o1 as u64) + (o2 as u64);
    }
    if c != 0 {
        acc.push(c);
    }
}
/// acc = acc - b, requires acc >= b
pub fn mag_sub_into(acc: &mut Vec<u64>, b: &[u64]) {
    let mut br = 0u64;
    for i in 0..acc.len() {
        if i >= b.len() && br == 0 {
            break;
        }
        let bi = if i < b.len() { b[i] } else { 0 };
        let (s1, o1) = acc[i].overflowing_sub(bi);
        let (s2, o2) = s1.overflowing_sub(br);
        acc[i] = s2;
        br = (o1 as u64) + (o2 as u64);
    }
    trim(acc);
}
/// acc = b - acc, requires b > acc
pub fn mag_rsub_into(acc: &mut Vec<u64>, b: &[u64]) {
    let mut br = 0u64;
    acc.resize(b.len(), 0);
    for i in 0..b.len() {
        let (s1, o1) = b[i].overflowing_sub(acc[i]);
        let (s2, o2) = s1.overflowing_sub(br);
        acc[i] = s2;
        br = (o1 as u64) + (o2 as u64);
    }
    trim(acc);
}
pub fn mag_mul_into(out: &mut Vec<u64>, a: &[u64], b: &[u64]) {
    out.clear();
    if a.is_empty() || b.is_empty() {
        return;
    }
    out.resize(a.len() + b.len(), 0);
    for i in 0..a.len() {
        let ai = a[i] as u128;
        let mut c = 0u128;
        for j in 0..b.len() {
            let t = ai * (b[j] as u128) + (out[i + j] as u128) + c;
            out[i + j] = t as u64;
            c = t >> 64;
        }
        out[i + b.len()] = c as u64;
    }
    trim(out);
}
/// magnitude divided by a single limb: (quotient, remainder)
fn mag_divrem_small(a: &[u64], d: u64) -> (Vec<u64>, u64) {
    let mut q = vec![0u64; a.len()];
    let mut r = 0u128;
    for i in (0..a.len()).rev() {
        let x = (r << 64) | a[i] as u128;
        q[i] = (x / d as u128) as u64;
        r = x % d as u128;
    }
    trim(&mut q);
    (q, r as u64)
}
/// Knuth algorithm D: (quotient, remainder) of magnitudes, b nonzero.
fn mag_divrem(a: &[u64], b: &[u64]) -> (Vec<u64>, Vec<u64>) {
    assert!(!b.is_empty(), "division by zero");
    if mag_cmp(a, b) == Ordering::Less {
        return (vec![], a.to_vec());
    }
    if b.len() == 1 {
        let (q, r) = mag_divrem_small(a, b[0]);
        let mut rv = vec![r];
        trim(&mut rv);
        return (q, rv);
    }
    let s = b.last().unwrap().leading_zeros();
    let shl = |x: &[u64], extra: bool| -> Vec<u64> {
        let mut o = Vec::with_capacity(x.len() + 1);
        let mut carry = 0u64;
        for &w in x {
            if s == 0 {
                o.push(w);
            } else {
                o.push((w << s) | carry);
                carry = w >> (64 - s);
            }
        }
        if extra {
            o.push(carry);
        }
        o
    };
    let v = shl(b, false);
    let mut u = shl(a, true);
    let n = v.len();
    let m = u.len() - n - 1;
    let mut q = vec![0u64; m + 1];
    let vtop = v[n - 1] as u128;
    let vnext = v[n - 2] as u128;
    for j in (0..=m).rev() {
        let num = ((u[j + n] as u128) << 64) | u[j + n - 1] as u128;
        let mut qhat = num / vtop;
        let mut rhat = num % vtop;
        while qhat >= (1u128 << 64) || qhat * vnext > ((rhat << 64) | u[j + n - 2] as u128) {
            qhat -= 1;
            rhat += vtop;
            if rhat >= (1u128 << 64) {
                break;
            }
        }
        // u[j..j+n+1] -= qhat * v
        let mut borrow: i128 = 0;
        let mut carry: u128 = 0;
        for i in 0..n {
            let p = qhat * v[i] as u128 + carry;
            carry = p >> 64;
            let t = u[i + j] as i128 - (p as u64) as i128 + borrow;
            u[i + j] = t as u64;
            borrow = t >> 64;
        }
        let t = u[j + n] as i128 - carry as i128 + borrow;
        u[j + n] = t as u64;
        if t < 0 {
            // add back
            qhat -= 1;
            let mut c = 0u128;
            for i in 0..n {
                let s2 = u[i + j] as u128 + v[i] as u128 + c;
                u[i + j] = s2 as u64;
                c = s2 >> 64;
            }
            u[j + n] = u[j + n].wrapping_add(c as u64);
        }
        q[j] = qhat as u64;
    }
    trim(&mut q);
    // remainder = u[0..n] >> s
    let mut r = vec![0u64; n];
    for i in 0..n {
        r[i] = if s == 0 { u[i] } else { (u[i] >> s) | (u[i + 1] << (64 - s)) };
    }
    trim(&mut r);
    (q, r)
}

impl Big {
    pub fn zero() -> Self {
        Big { neg: false, m: vec![] }
    }
    pub fn one() -> Self {
        Big { neg: false, m: vec![1] }
    }
    pub fn from_i128(x: i128) -> Self {
        let neg = x < 0;
        let u = x.unsigned_abs();
        let mut m = vec![u as u64, (u >> 64) as u64];
        trim(&mut m);
        Big { neg: neg && !m.is_empty(), m }
    }
    pub fn from_u64(x: u64) -> Self {
        let mut m = vec![x];
        trim(&mut m);
        Big { neg: false, m }
    }
    pub fn is_zero(&self) -> bool {
        self.m.is_empty()
    }
    pub fn is_one(&self) -> bool {
        !self.neg && self.m == [1]
    }
    pub fn to_i128(&self) -> Option<i128> {
        if self.m.len() > 2 {
            return None;
        }
        let u = self.m.first().copied().unwrap_or(0) as u128 | ((self.m.get(1).copied().unwrap_or(0) as u128) << 64);
        if u > i128::MAX as u128 {
            return None;
        }
        Some(if self.neg { -(u as i128) } else { u as i128 })
    }
    pub fn add_signed(&mut self, neg: bool, b: &[u64]) {
        if b.is_empty() {
            return;
        }
        if self.m.is_empty() {
            self.m.clear();
            self.m.extend_from_slice(b);
            self.neg = neg;
            return;
        }
        if self.neg == neg {
            mag_add_into(&mut self.m, b);
        } else {
            match mag_cmp(&self.m, b) {
                Ordering::Greater => mag_sub_into(&mut self.m, b),
                Ordering::Equal => {
                    self.m.clear();
                    self.neg = false;
                }
                Ordering::Less => {
                    mag_rsub_into(&mut self.m, b);
                    self.neg = neg;
                }
            }
        }
        if self.m.is_empty() {
            self.neg = false;
        }
    }
    pub fn add(&mut self, b: &Big) {
        self.add_signed(b.neg, &b.m);
    }
    pub fn sub(&mut self, b: &Big) {
        self.add_signed(!b.neg, &b.m);
    }
    pub fn mul(&self, b: &Big) -> Big {
        let mut o = Vec::new();
        mag_mul_into(&mut o, &self.m, &b.m);
        let neg = (self.neg != b.neg) && !o.is_empty();
        Big { neg, m: o }
    }
    pub fn mul_i64(&self, c: i64) -> Big {
        self.mul(&Big::from_i128(c as i128))
    }
    pub fn negated(&self) -> Big {
        let mut b = self.clone();
        if !b.m.is_empty() {
            b.neg = !b.neg;
        }
        b
    }
    pub fn abs(&self) -> Big {
        Big { neg: false, m: self.m.clone() }
    }
    pub fn cmp_abs(&self, b: &Big) -> Ordering {
        mag_cmp(&self.m, &b.m)
    }
    /// truncated division (quotient rounds toward zero), b != 0
    pub fn divrem(&self, b: &Big) -> (Big, Big) {
        let (q, r) = mag_divrem(&self.m, &b.m);
        let qn = (self.neg != b.neg) && !q.is_empty();
        let rn = self.neg && !r.is_empty();
        (Big { neg: qn, m: q }, Big { neg: rn, m: r })
    }
    /// exact division (panics if not exact)
    pub fn div_exact(&self, b: &Big) -> Big {
        let (q, r) = self.divrem(b);
        assert!(r.is_zero(), "div_exact: not exact");
        q
    }
    pub fn gcd(a: &Big, b: &Big) -> Big {
        let mut x = a.abs();
        let mut y = b.abs();
        while !y.is_zero() {
            let (_, r) = x.divrem(&y);
            x = y;
            y = r.abs();
        }
        x
    }
    pub fn lcm(a: &Big, b: &Big) -> Big {
        if a.is_zero() || b.is_zero() {
            return Big::zero();
        }
        let g = Big::gcd(a, b);
        a.abs().div_exact(&g).mul(&b.abs())
    }
    pub fn mod_u64(&self, p: u64) -> u64 {
        let mut r = 0u128;
        for &l in self.m.iter().rev() {
            r = ((r << 64) | l as u128) % p as u128;
        }
        let r = r as u64;
        if self.neg && r != 0 { p - r } else { r }
    }
    pub fn from_dec(d: &[u8]) -> Big {
        let mut m: Vec<u64> = vec![];
        let mut i = 0;
        while i < d.len() {
            let t = (d.len() - i).min(19);
            let mut chunk = 0u64;
            let mut pw = 1u64;
            for k in 0..t {
                chunk = chunk * 10 + (d[i + k] - b'0') as u64;
                pw *= 10;
            }
            let mut c = chunk as u128;
            for l in m.iter_mut() {
                let x = (*l as u128) * (pw as u128) + c;
                *l = x as u64;
                c = x >> 64;
            }
            if c != 0 {
                m.push(c as u64);
            }
            i += t;
        }
        trim(&mut m);
        Big { neg: false, m }
    }
    pub fn parse(s: &str) -> Option<Big> {
        let s = s.trim();
        let (neg, digits) = match s.strip_prefix('-') {
            Some(r) => (true, r),
            None => (false, s.strip_prefix('+').unwrap_or(s)),
        };
        if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let mut b = Big::from_dec(digits.as_bytes());
        b.neg = neg && !b.is_zero();
        Some(b)
    }
    pub fn to_dec(&self) -> String {
        if self.m.is_empty() {
            return "0".into();
        }
        let mut m = self.m.clone();
        let mut parts = vec![];
        while !m.is_empty() {
            let mut r = 0u128;
            for l in m.iter_mut().rev() {
                let x = (r << 64) | *l as u128;
                *l = (x / 10_000_000_000_000_000_000u128) as u64;
                r = x % 10_000_000_000_000_000_000u128;
            }
            trim(&mut m);
            parts.push(r as u64);
        }
        let mut s = String::new();
        if self.neg {
            s.push('-');
        }
        s.push_str(&parts.last().unwrap().to_string());
        for p in parts.iter().rev().skip(1) {
            s.push_str(&format!("{:019}", p));
        }
        s
    }
    pub fn bits(&self) -> u64 {
        if self.m.is_empty() {
            0
        } else {
            64 * (self.m.len() as u64 - 1) + (64 - self.m.last().unwrap().leading_zeros() as u64)
        }
    }
}

/// Integer polynomial in one variable (dense, ascending powers, trimmed).
pub type BigPoly = Vec<Big>;
pub fn bp_trim(p: &mut BigPoly) {
    while let Some(c) = p.last() {
        if c.is_zero() {
            p.pop();
        } else {
            break;
        }
    }
}
pub fn bp_mul(a: &BigPoly, b: &BigPoly) -> BigPoly {
    if a.is_empty() || b.is_empty() {
        return vec![];
    }
    let mut o = vec![Big::zero(); a.len() + b.len() - 1];
    for (i, x) in a.iter().enumerate() {
        if x.is_zero() {
            continue;
        }
        for (j, y) in b.iter().enumerate() {
            if y.is_zero() {
                continue;
            }
            let t = x.mul(y);
            o[i + j].add(&t);
        }
    }
    bp_trim(&mut o);
    o
}
pub fn bp_add_into(acc: &mut BigPoly, b: &BigPoly) {
    if acc.len() < b.len() {
        acc.resize(b.len(), Big::zero());
    }
    for (i, y) in b.iter().enumerate() {
        acc[i].add(y);
    }
    bp_trim(acc);
}
pub fn bp_eval_i128(p: &BigPoly, x: i128) -> Big {
    let xb = Big::from_i128(x);
    let mut acc = Big::zero();
    for c in p.iter().rev() {
        acc = acc.mul(&xb);
        acc.add(c);
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn divrem_matches_u128_and_reconstructs() {
        let mut s = 0x1234_5678_9abc_def0u64;
        let mut next = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        };
        for _ in 0..2000 {
            let la = (next() % 6 + 1) as usize;
            let lb = (next() % 4 + 1) as usize;
            let a = Big { neg: next() % 2 == 0, m: { let mut v: Vec<u64> = (0..la).map(|_| next()).collect(); trim(&mut v); v } };
            let mut bm: Vec<u64> = (0..lb).map(|_| next() >> (next() % 64)).collect();
            trim(&mut bm);
            if bm.is_empty() {
                continue;
            }
            let b = Big { neg: next() % 2 == 0, m: bm };
            let (q, r) = a.divrem(&b);
            let mut back = q.mul(&b);
            back.add(&r);
            assert_eq!(back, a);
            assert_eq!(r.cmp_abs(&b), Ordering::Less);
        }
        assert_eq!(Big::parse("-123456789012345678901234567890").unwrap().to_dec(), "-123456789012345678901234567890");
        assert_eq!(Big::gcd(&Big::from_i128(84), &Big::from_i128(-36)), Big::from_i128(12));
    }
}
