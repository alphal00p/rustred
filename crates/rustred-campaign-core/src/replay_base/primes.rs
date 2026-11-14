//! DOMAIN LOGIC (no algebra: primality is `CheckAlgebra::is_prime`). NEW (campaign, not copied): the verification primes of the A2 identity test and the deterministic random streams.
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

use super::algebra::CheckAlgebra;

/// Why a prime may not decide.
pub fn check_verify_prime<A: CheckAlgebra>(alg: &A, p: u64, exclude: &[u64]) -> Result<(), String> {
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
    if !alg.is_prime(p) {
        return Err(format!("{p} is not prime"));
    }
    Ok(())
}

/// The next prime >= x inside the verify range, wrapping to the range start.
fn next_prime_in_range<A: CheckAlgebra>(
    alg: &A,
    mut x: u64,
    exclude: &[u64],
    taken: &[u64],
) -> u64 {
    x |= 1;
    loop {
        if x >= VERIFY_HI {
            x = VERIFY_LO | 1;
        }
        if alg.is_prime(x) && !exclude.contains(&x) && !taken.contains(&x) {
            return x;
        }
        x += 2;
    }
}

/// Two distinct verify-range primes derived from `seed`, none in `exclude` (the rule's lift primes).
pub fn verify_primes_with<A: CheckAlgebra>(alg: &A, seed: u64, exclude: &[u64]) -> [u64; 2] {
    let mut s = SplitMix64::derive(seed, &[0x5052_494d_4553]); // "PRIMES"
    let a = next_prime_in_range(alg, s.range(VERIFY_LO, VERIFY_HI), exclude, &[]);
    let b = next_prime_in_range(alg, s.range(VERIFY_LO, VERIFY_HI), exclude, &[a]);
    [a, b]
}

/// [`verify_primes_with`] on the certificate backend (`Independent`).
pub fn verify_primes(seed: u64, exclude: &[u64]) -> [u64; 2] {
    verify_primes_with(&crate::independent_algebra::Independent, seed, exclude)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primes_in_range_distinct_deterministic() {
        for seed in 0..50u64 {
            let [a, b] = verify_primes(seed, &[]);
            assert!(a != b);
            for p in [a, b] {
                check_verify_prime(&crate::independent_algebra::Independent, p, &[]).unwrap();
                assert!((VERIFY_LO..VERIFY_HI).contains(&p) && 64 - p.leading_zeros() >= 61);
            }
            assert_eq!(verify_primes(seed, &[]), [a, b]);
            let [c, d] = verify_primes(seed, &[a]);
            assert!(c != a && d != a);
        }
        for p in FORBIDDEN {
            assert!(check_verify_prime(&crate::independent_algebra::Independent, p, &[]).is_err());
        }
    }
}
