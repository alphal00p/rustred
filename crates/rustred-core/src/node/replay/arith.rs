//! Own arithmetic of the replay: P61 field, a second 61-bit prime for zero tests, small exact rationals (i128,
//! checked), and Python's MT19937 `random.Random(int).randint` (deterministic sample search of p5l-replay.v1).
//! Nothing here uses RustRed or Symbolica arithmetic.

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
    if a % p == 0 {
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

/// Exact rational with i128 parts (normalized, positive denominator). Overflow is an error, never wraps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Q {
    pub n: i128,
    pub d: i128,
}

fn gcd(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

pub type QResult = Result<Q, String>;
const OVF: &str = "exact rational overflow";

impl Q {
    pub const ZERO: Q = Q { n: 0, d: 1 };
    pub const ONE: Q = Q { n: 1, d: 1 };
    pub fn int(n: i128) -> Q {
        Q { n, d: 1 }
    }
    pub fn new(n: i128, d: i128) -> QResult {
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
    pub fn is_zero(&self) -> bool {
        self.n == 0
    }
    pub fn add(self, o: Q) -> QResult {
        let n = self
            .n
            .checked_mul(o.d)
            .and_then(|a| o.n.checked_mul(self.d).and_then(|b| a.checked_add(b)))
            .ok_or(OVF)?;
        Q::new(n, self.d.checked_mul(o.d).ok_or(OVF)?)
    }
    pub fn sub(self, o: Q) -> QResult {
        self.add(Q {
            n: o.n.checked_neg().ok_or(OVF)?,
            d: o.d,
        })
    }
    pub fn mul(self, o: Q) -> QResult {
        Q::new(
            self.n.checked_mul(o.n).ok_or(OVF)?,
            self.d.checked_mul(o.d).ok_or(OVF)?,
        )
    }
    pub fn div(self, o: Q) -> QResult {
        if o.n == 0 {
            return Err("division by zero".into());
        }
        Q::new(
            self.n.checked_mul(o.d).ok_or(OVF)?,
            self.d.checked_mul(o.n).ok_or(OVF)?,
        )
    }
    pub fn neg(self) -> Q {
        Q {
            n: -self.n,
            d: self.d,
        }
    }
    pub fn modp(&self, p: u64) -> Option<u64> {
        Some(mul(p, from_i128(p, self.n), inv(p, from_i128(p, self.d))?))
    }
    /// Parse `a` or `a/b`.
    pub fn parse(s: &str) -> QResult {
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

/// CPython's MT19937 (`random.Random(seed)` for a nonnegative int seed < 2^32, `randint`).
pub struct PyRandom {
    mt: [u32; 624],
    index: usize,
}

impl PyRandom {
    pub fn new(seed: u32) -> Self {
        let mut r = PyRandom {
            mt: [0; 624],
            index: 624,
        };
        r.init_genrand(19_650_218);
        // init_by_array(key = [seed])
        let key = [seed];
        let (mut i, mut j) = (1usize, 0usize);
        let mut k = 624.max(key.len());
        while k > 0 {
            let prev = r.mt[i - 1];
            r.mt[i] = (r.mt[i] ^ ((prev ^ (prev >> 30)).wrapping_mul(1_664_525)))
                .wrapping_add(key[j])
                .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= 624 {
                r.mt[0] = r.mt[623];
                i = 1;
            }
            if j >= key.len() {
                j = 0;
            }
            k -= 1;
        }
        k = 623;
        while k > 0 {
            let prev = r.mt[i - 1];
            r.mt[i] = (r.mt[i] ^ ((prev ^ (prev >> 30)).wrapping_mul(1_566_083_941)))
                .wrapping_sub(i as u32);
            i += 1;
            if i >= 624 {
                r.mt[0] = r.mt[623];
                i = 1;
            }
            k -= 1;
        }
        r.mt[0] = 0x8000_0000;
        r.index = 624;
        r
    }

    fn init_genrand(&mut self, s: u32) {
        self.mt[0] = s;
        for i in 1..624 {
            let prev = self.mt[i - 1];
            self.mt[i] = 1_812_433_253u32
                .wrapping_mul(prev ^ (prev >> 30))
                .wrapping_add(i as u32);
        }
        self.index = 624;
    }

    pub fn genrand_u32(&mut self) -> u32 {
        if self.index >= 624 {
            for kk in 0..624 {
                let y = (self.mt[kk] & 0x8000_0000) | (self.mt[(kk + 1) % 624] & 0x7fff_ffff);
                let mut v = self.mt[(kk + 397) % 624] ^ (y >> 1);
                if y & 1 == 1 {
                    v ^= 0x9908_b0df;
                }
                self.mt[kk] = v;
            }
            self.index = 0;
        }
        let mut y = self.mt[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^= y >> 18;
        y
    }

    /// `getrandbits(k)` for `1 <= k <= 32`.
    pub fn getrandbits(&mut self, k: u32) -> u32 {
        self.genrand_u32() >> (32 - k)
    }

    /// `randint(a, b)` (inclusive), CPython's `_randbelow_with_getrandbits`.
    pub fn randint(&mut self, a: i64, b: i64) -> i64 {
        let n = (b - a + 1) as u64;
        let k = 64 - n.leading_zeros();
        let mut r = self.getrandbits(k) as u64;
        while r >= n {
            r = self.getrandbits(k) as u64;
        }
        a + r as i64
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
        assert_eq!(Q::new(6, -4).unwrap(), Q { n: -3, d: 2 });
        assert!(Q::int(i128::MAX).add(Q::ONE).is_err());
        assert_eq!(from_i128(P, -1), P - 1);
    }

    #[test]
    fn mt19937_matches_cpython() {
        // python3 -c "import random; r=random.Random(5); print([r.getrandbits(32) for _ in range(3)])"
        let mut r = PyRandom::new(5);
        assert_eq!(
            [r.genrand_u32(), r.genrand_u32(), r.genrand_u32()],
            [2_675_342_405, 1_097_127_993, 3_185_950_873]
        );
        // python3 -c "import random; r=random.Random(0x525332); print([r.randint(1,7) for _ in range(8)])"
        let mut r = PyRandom::new(0x525332);
        let v: Vec<i64> = (0..8).map(|_| r.randint(1, 7)).collect();
        assert_eq!(v, [2, 6, 6, 4, 2, 4, 4, 5]);
        let mut r = PyRandom::new(0x525332 + 88);
        let v: Vec<i64> = (0..5).map(|_| r.randint(1, 31)).collect();
        assert_eq!(v, [3, 7, 11, 18, 31]);
    }
}
