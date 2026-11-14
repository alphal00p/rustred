//! NEW (campaign, not copied): the verification primes of the A2 identity test and the deterministic random streams.
//!
//! Plan 2.5.3: modular checks use >= 2 primes of >= 61 bits outside every engine stream; the proof-kernel range
//! [7*2^60, 7.5*2^60) is free; never 2^31-1, never 2^62-57 or 2^61-1 as the only prime. Every prime here lies in that
//! range, so the replay's P = 2^61-1 and P2 = 2^61-31, the h3 second prime 2^31-1 and the engine's 2^62-57 are
//! impossible; [`check_verify_prime`] also names them explicitly.

/// Lower end (inclusive) of the verify range, 7 * 2^60.
pub const VERIFY_LO: u64 = 7 << 60;
/// Upper end (exclusive) of the verify range, 7.5 * 2^60.
pub const VERIFY_HI: u64 = (7 << 60) + (1 << 59);

/// Primes that must never decide a verdict (named for the reader; all lie outside the range anyway).
pub const FORBIDDEN: [u64; 4] = [
    2_305_843_009_213_693_951, // 2^61 - 1 (replay P61, the search prime)
    2_305_843_009_213_693_921, // 2^61 - 31 (replay P2)
    4_611_686_018_427_387_847, // 2^62 - 57 (engine stream)
    2_147_483_647,             // 2^31 - 1 (h3's weak second prime)
];

#[inline]
fn mulmod(a: u64, b: u64, m: u64) -> u64 {
    ((a as u128 * b as u128) % m as u128) as u64
}

fn powmod(mut a: u64, mut e: u64, m: u64) -> u64 {
    let mut r = 1u64 % m;
    a %= m;
    while e > 0 {
        if e & 1 == 1 {
            r = mulmod(r, a, m);
        }
        a = mulmod(a, a, m);
        e >>= 1;
    }
    r
}

/// Deterministic Miller-Rabin for every u64 (bases 2, 325, 9375, 28178, 450775, 9780504, 1795265022: Jim Sinclair's set).
pub fn is_prime(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    for p in [2u64, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] {
        if n.is_multiple_of(p) {
            return n == p;
        }
    }
    let mut d = n - 1;
    let mut s = 0;
    while d.is_multiple_of(2) {
        d /= 2;
        s += 1;
    }
    'base: for a in [2u64, 325, 9375, 28178, 450_775, 9_780_504, 1_795_265_022] {
        let a = a % n;
        if a == 0 {
            continue;
        }
        let mut x = powmod(a, d, n);
        if x == 1 || x == n - 1 {
            continue;
        }
        for _ in 1..s {
            x = mulmod(x, x, n);
            if x == n - 1 {
                continue 'base;
            }
        }
        return false;
    }
    true
}

/// SplitMix64 (Steele, Lea, Flood 2014): the deterministic stream behind primes, d values and generic points.
#[derive(Clone, Debug)]
pub struct SplitMix64(u64);

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        SplitMix64(seed)
    }
    /// A stream derived from a seed and a list of stream ids (rule, point, prime, purpose, attempt).
    pub fn derive(seed: u64, ids: &[u64]) -> Self {
        let mut s = SplitMix64(seed ^ 0x5243_4f57_535f_4132); // "RCOWS_A2"
        let mut h = s.next_u64();
        for &x in ids {
            s = SplitMix64(h ^ x.wrapping_mul(0x9e37_79b9_7f4a_7c15));
            h = s.next_u64();
        }
        SplitMix64(h)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    /// Uniform in [lo, hi) (rejection sampling, no modulo bias).
    pub fn range(&mut self, lo: u64, hi: u64) -> u64 {
        assert!(lo < hi, "empty range");
        let n = hi - lo;
        let zone = u64::MAX - (u64::MAX % n);
        loop {
            let x = self.next_u64();
            if x < zone {
                return lo + x % n;
            }
        }
    }
}

/// Why a prime may not decide.
pub fn check_verify_prime(p: u64, exclude: &[u64]) -> Result<(), String> {
    if FORBIDDEN.contains(&p) {
        return Err(format!("prime {p} is an engine/h3 stream prime"));
    }
    if !(VERIFY_LO..VERIFY_HI).contains(&p) {
        return Err(format!(
            "prime {p} outside the verify range [7*2^60, 7.5*2^60)"
        ));
    }
    if exclude.contains(&p) {
        return Err(format!("prime {p} is a lift prime of the rule"));
    }
    if !is_prime(p) {
        return Err(format!("{p} is not prime"));
    }
    Ok(())
}

/// The next prime >= x inside the verify range, wrapping to the range start.
fn next_prime_in_range(mut x: u64, exclude: &[u64], taken: &[u64]) -> u64 {
    x |= 1;
    loop {
        if x >= VERIFY_HI {
            x = VERIFY_LO | 1;
        }
        if is_prime(x) && !exclude.contains(&x) && !taken.contains(&x) {
            return x;
        }
        x += 2;
    }
}

/// Two distinct verify-range primes derived from `seed`, none in `exclude` (the rule's lift primes).
pub fn verify_primes(seed: u64, exclude: &[u64]) -> [u64; 2] {
    let mut s = SplitMix64::derive(seed, &[0x5052_494d_4553]); // "PRIMES"
    let a = next_prime_in_range(s.range(VERIFY_LO, VERIFY_HI), exclude, &[]);
    let b = next_prime_in_range(s.range(VERIFY_LO, VERIFY_HI), exclude, &[a]);
    [a, b]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn miller_rabin_known_values() {
        assert!(is_prime(2_305_843_009_213_693_951)); // 2^61-1
        assert!(is_prime(2_305_843_009_213_693_921)); // 2^61-31
        assert!(is_prime(4_611_686_018_427_387_847)); // 2^62-57
        assert!(is_prime(18_446_744_073_709_551_557)); // largest u64 prime
        assert!(!is_prime(3_215_031_751)); // strong pseudoprime to 2,3,5,7
        assert!(!is_prime(2_305_843_009_213_693_953));
        assert!(!is_prime(1));
        // brute force on small numbers
        for n in 0u64..2000 {
            let t = n >= 2 && (2..n).take_while(|k| k * k <= n).all(|k| n % k != 0);
            assert_eq!(is_prime(n), t, "{n}");
        }
    }

    #[test]
    fn primes_in_range_distinct_deterministic() {
        for seed in 0..50u64 {
            let [a, b] = verify_primes(seed, &[]);
            assert!(a != b);
            for p in [a, b] {
                check_verify_prime(p, &[]).unwrap();
                assert!((VERIFY_LO..VERIFY_HI).contains(&p) && 64 - p.leading_zeros() >= 61);
            }
            assert_eq!(verify_primes(seed, &[]), [a, b]);
            let [c, d] = verify_primes(seed, &[a]);
            assert!(c != a && d != a);
        }
        for p in FORBIDDEN {
            assert!(check_verify_prime(p, &[]).is_err());
        }
    }
}
