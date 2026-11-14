//! INDEPENDENT ALGEBRA (see mod.rs for why this exists). COPY of rustred-core/src/node/replay/arith.rs at engine commit
//! b9f06de7 (campaign copy; changes: the MT19937 sampler was dropped (unused here); the rational type [`Q`] is the
//! DATA type of `replay_base::algebra` and its checked arithmetic is the module-private extension trait [`QArith`]).
//!
//! Own arithmetic of the replay: the prime field on u64 residues (u128 products), the replay's P61 / P2 constants
//! (kept for the copied tests only; the identity test never decides at them), small exact rationals (i128, checked).
//! Duplicates: numerica `Zp64` (lib/numerica/src/domains/finite_field.rs:41) and `Rational` (domains/rational.rs).

pub use crate::replay_base::algebra::{Q, QResult};

/// 2^61 - 1, the replay prime of p5l-replay.v1.
pub const P: u64 = 2_305_843_009_213_693_951;
/// A second 61-bit prime, only for exact-zero tests of exception conjunctions (zero over Q iff zero mod P and P2,
/// up to an error probability below 1e-30 per test).
pub const P2: u64 = 2_305_843_009_213_693_921;

#[inline]
pub fn add(p: u64, a: u64, b: u64) -> u64 {
    let s = a as u128 + b as u128;
    (s % p as u128) as u64
}
#[inline]
pub fn sub(p: u64, a: u64, b: u64) -> u64 {
    if a >= b { a - b } else { p - (b - a) }
}
#[inline]
pub fn mul(p: u64, a: u64, b: u64) -> u64 {
    ((a as u128 * b as u128) % p as u128) as u64
}
pub fn pow(p: u64, mut a: u64, mut e: u64) -> u64 {
    let mut r = 1u64;
    a %= p;
    while e > 0 {
        if e & 1 == 1 {
            r = mul(p, r, a);
        }
        a = mul(p, a, a);
        e >>= 1;
    }
    r
}
pub fn inv(p: u64, a: u64) -> Option<u64> {
    if a.is_multiple_of(p) {
        None
    } else {
        Some(pow(p, a, p - 2))
    }
}
pub fn from_i128(p: u64, x: i128) -> u64 {
    x.rem_euclid(p as i128) as u64
}
pub fn neg(p: u64, a: u64) -> u64 {
    if a == 0 { 0 } else { p - a }
}

fn gcd(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

const OVF: &str = "exact rational overflow";

/// Checked exact rational arithmetic on [`Q`] (normal form: lowest terms, positive denominator). Visible ONLY inside
/// `independent_algebra`: the domain logic reaches it through `CheckAlgebra::q_*`.
pub(in crate::independent_algebra) trait QArith: Sized {
    #[allow(
        clippy::new_ret_no_self,
        reason = "checked constructor returning QResult"
    )]
    fn new(n: i128, d: i128) -> QResult;
    fn add(self, o: Q) -> QResult;
    fn sub(self, o: Q) -> QResult;
    fn mul(self, o: Q) -> QResult;
    fn div(self, o: Q) -> QResult;
    /// unchecked negation (as the engine copy; `CheckAlgebra::q_neg` is the checked one)
    fn neg(self) -> Q;
    fn modp(&self, p: u64) -> Option<u64>;
    /// Parse `a` or `a/b`.
    fn parse(s: &str) -> QResult;
}

impl QArith for Q {
    fn new(n: i128, d: i128) -> QResult {
        if d == 0 {
            return Err("division by zero".into());
        }
        let g = gcd(n, d).max(1);
        let (mut n, mut d) = (n / g, d / g);
        if d < 0 {
            n = n.checked_neg().ok_or(OVF)?;
            d = d.checked_neg().ok_or(OVF)?;
        }
        Ok(Q { n, d })
    }
    fn add(self, o: Q) -> QResult {
        let n = self
            .n
            .checked_mul(o.d)
            .and_then(|a| o.n.checked_mul(self.d).and_then(|b| a.checked_add(b)))
            .ok_or(OVF)?;
        Q::new(n, self.d.checked_mul(o.d).ok_or(OVF)?)
    }
    fn sub(self, o: Q) -> QResult {
        self.add(Q {
            n: o.n.checked_neg().ok_or(OVF)?,
            d: o.d,
        })
    }
    fn mul(self, o: Q) -> QResult {
        Q::new(
            self.n.checked_mul(o.n).ok_or(OVF)?,
            self.d.checked_mul(o.d).ok_or(OVF)?,
        )
    }
    fn div(self, o: Q) -> QResult {
        if o.n == 0 {
            return Err("division by zero".into());
        }
        Q::new(
            self.n.checked_mul(o.d).ok_or(OVF)?,
            self.d.checked_mul(o.n).ok_or(OVF)?,
        )
    }
    fn neg(self) -> Q {
        Q {
            n: -self.n,
            d: self.d,
        }
    }
    fn modp(&self, p: u64) -> Option<u64> {
        Some(mul(p, from_i128(p, self.n), inv(p, from_i128(p, self.d))?))
    }
    fn parse(s: &str) -> QResult {
        match s.split_once('/') {
            Some((a, b)) => Q::new(
                a.trim().parse().map_err(|_| format!("rational {s}"))?,
                b.trim().parse().map_err(|_| format!("rational {s}"))?,
            ),
            None => Ok(Q::int(
                s.trim().parse().map_err(|_| format!("rational {s}"))?,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_and_rationals() {
        assert_eq!(mul(P, inv(P, 7).unwrap(), 7), 1);
        assert_eq!(
            Q::parse("17/7").unwrap().modp(P).map(|x| mul(P, x, 7)),
            Some(17)
        );
        assert_eq!(<Q as QArith>::new(6, -4).unwrap(), Q { n: -3, d: 2 });
        assert!(Q::int(i128::MAX).add(Q::ONE).is_err());
        assert_eq!(from_i128(P, -1), P - 1);
    }
}
