//! INDEPENDENT ALGEBRA (see mod.rs for why this exists). Duplicates numerica `Integer::is_prime`
//! (lib/numerica/src/domains/integer.rs:1751-1771, deterministic for u64).
//!
//! Moved here from the campaign's `replay_base/primes.rs` (NEW campaign code, not an engine copy); the verify-range
//! policy and the random streams stay in `replay_base::primes` (domain logic).

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
}
