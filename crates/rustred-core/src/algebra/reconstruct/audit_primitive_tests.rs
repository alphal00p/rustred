//! Regression tests from the adversarial audit of the primitives (review of
//! #23, findings 2-4 and the robustness checks): repeated points, a Thiele
//! failure, an unlucky first prime, large coefficients, Wang/MQRR consistency
//! and primes above 2^63. Adapted from the auditor's scratch tests, with the
//! assertions turned into the contract the fixes establish.

use rand::{Rng, SeedableRng, rngs::StdRng};
use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};
use symbolica::domains::integer::Integer;
use symbolica::domains::rational::Rational;
use symbolica::domains::{Field, RingOps};
use symbolica::prelude::Ring;

use super::Zp;
use super::interpolate::{Interpolator, Newton, Thiele};
use super::lift::{Lift, LiftStatus};
use super::poly::normalise;
use super::rational::{RationalFunction, mqrr_reconstruction, wang_reconstruction};

fn q(v: i64) -> Rational {
    Rational::from(Integer::from(v))
}

fn qi(v: Integer) -> Rational {
    Rational::from(v)
}

fn prev_prime(mut c: u64) -> u64 {
    if c.is_multiple_of(2) {
        c -= 1;
    }
    while !Integer::from(c).is_prime(0) {
        c -= 2;
    }
    c
}

/// Descending primes below 2^62, skipping 2^61 - 1.
fn primes(count: usize) -> Vec<u64> {
    let mut out = Vec::with_capacity(count);
    let mut c = (1u64 << 62) - 1;
    while out.len() < count {
        c = prev_prime(c);
        if c != (1 << 61) - 1 {
            out.push(c);
        }
        c -= 2;
    }
    out
}

/// Review #23 finding 2: for `f = x^2`, `f(1) = f(-1)`, so feeding 1, -1, -1
/// used to end with the constant 1. A repeated point is ignored now.
#[test]
fn a1_a_repeated_point_is_not_a_second_confirmation() {
    let field = Zp64::new((1 << 61) - 1);
    let e = |v: i64| {
        let m = field.to_element(v.unsigned_abs());
        if v < 0 { field.neg(&m) } else { m }
    };
    let f = |x: &Zp| field.mul(x, x);
    let mut newton = Newton::default();
    let mut thiele = Thiele::default();
    let mut racer = Interpolator::default();
    for x in [e(1), e(-1), e(-1), e(-1), e(1)] {
        newton.feed(&field, &x, &f(&x));
        thiele.feed(&field, &x, &f(&x));
        racer.feed(&field, &x, &f(&x));
    }
    assert!(!newton.done() && !thiele.done() && !racer.done());
    // Fresh points finish with the true degree.
    for v in 2..20 {
        let x = e(v);
        racer.feed(&field, &x, &f(&x));
        if racer.done() {
            break;
        }
    }
    let (n, d) = racer.rational(&field).unwrap();
    assert_eq!((n.len() - 1, d.len() - 1), (2, 0));
}

/// Review #23 finding 3: a Thiele failure must not stop Newton.
///
/// `P(x) = c * prod_{i != 1} (x - x_i)` over `x_0 = 1, x_2..x_10 = 3..11`,
/// normalised so that `P(x_1 = 2) = 1`. Thiele stores `x_0` (`a_0 = 0`) and
/// `x_1`; at `x_2..x_10` the value equals `a_0` while the prediction
/// `(x - x_0) / a_1` does not, so every one of these points is an accidental
/// zero of the recursion and Thiele fails after its skip budget. Newton must
/// keep going and finish with the degree-10 polynomial.
#[test]
fn a2_newton_keeps_running_after_a_thiele_failure() {
    let field = Zp64::new(1_000_003);
    let e = |v: u64| field.to_element(v);
    let roots: Vec<u64> = std::iter::once(1).chain(3..=11).collect();
    let product = |x: &Zp| {
        roots
            .iter()
            .fold(field.one(), |acc, &r| field.mul(&acc, &field.sub(x, &e(r))))
    };
    let c = field.inv(&product(&e(2)));
    let p = |x: &Zp| field.mul(&c, &product(x));
    let mut racer = Interpolator::default();
    let mut thiele = Thiele::default();
    for x in 1..=11u64 {
        racer.feed(&field, &e(x), &p(&e(x)));
        thiele.feed(&field, &e(x), &p(&e(x)));
    }
    assert!(
        thiele.failed(),
        "Thiele must have failed on the accidental zeros"
    );
    assert!(racer.failed() && !racer.done());
    for x in 12..40u64 {
        racer.feed(&field, &e(x), &p(&e(x)));
        if racer.done() {
            break;
        }
    }
    assert!(racer.done() && !racer.failed());
    let (n, d) = racer.rational(&field).unwrap();
    assert_eq!((n.len() - 1, d.len() - 1), (10, 0));
}

/// Review #23 finding 4: an unlucky FIRST prime (it divides the leading
/// numerator coefficient) used to make every later prime a ShapeMismatch.
/// A strictly larger shape now restarts the accumulation.
#[test]
fn a3_lift_recovers_from_an_unlucky_first_prime() {
    let primes = primes(8);
    let p0 = primes[0];
    let f = RationalFunction {
        numerator: vec![q(1), q(1), qi(Integer::from(p0))],
        denominator: vec![q(1), q(2)],
    };
    let mut lift = Lift::default();
    let mut statuses = Vec::new();
    for &p in &primes {
        let field = Zp64::new(p);
        let (n, d) = f.image(&field).unwrap();
        let (n, d) = normalise(&field, &n, &d).unwrap();
        let status = lift.add(&field, &n, &d);
        statuses.push(status);
        if status == LiftStatus::Stable {
            break;
        }
    }
    assert_eq!(statuses[1], LiftStatus::Restarted, "{statuses:?}");
    assert_eq!(statuses.last(), Some(&LiftStatus::Stable), "{statuses:?}");
    assert_eq!(lift.result(), Some(&f));
    assert!(!lift.primes().contains(&p0));
}

/// Large coefficients (200 to 960 bits): never a wrong `Stable`.
#[test]
fn a4_large_coefficients_are_never_stable_wrong() {
    let primes = primes(40);
    let mut rng = StdRng::seed_from_u64(5);
    for trial in 0..20 {
        let bits_n = 200 + 40 * trial;
        let mut num = Integer::one();
        for _ in 0..bits_n / 60 {
            num = &(&num * &Integer::from(1u64 << 60))
                + &Integer::from(rng.random_range(1u64..(1 << 60)));
        }
        let mut den = Integer::one();
        for _ in 0..2 {
            den = &(&den * &Integer::from(1u64 << 50))
                + &Integer::from(rng.random_range(1u64..(1 << 50)));
        }
        let c = &qi(num) / &qi(den);
        let f = RationalFunction {
            numerator: vec![c.clone(), q(1)],
            denominator: vec![q(1)],
        };
        let mut lift = Lift::default();
        let mut stable = false;
        for &p in &primes {
            let field = Zp64::new(p);
            let (n, d) = f.image(&field).unwrap();
            if lift.add(&field, &n, &d) == LiftStatus::Stable {
                assert_eq!(lift.result(), Some(&f), "trial {trial}");
                stable = true;
                break;
            }
        }
        assert!(stable, "trial {trial}");
    }
}

/// Wang and MQRR never return a value inconsistent with its residue.
#[test]
fn a4b_wang_and_mqrr_outputs_are_consistent_with_the_residue() {
    let m = Integer::from(prev_prime(1 << 62)) * Integer::from(prev_prime((1 << 62) - 100));
    let mut rng = StdRng::seed_from_u64(1);
    for _ in 0..2000 {
        let v = (&Integer::from(rng.random::<u64>()) * &Integer::from(rng.random::<u64>())) % &m;
        for r in [wang_reconstruction(&v, &m), mqrr_reconstruction(&v, &m)]
            .into_iter()
            .flatten()
        {
            let back = (&r.numerator() * &r.denominator().mod_inverse(&m)) % &m;
            let back = if back < Integer::zero() {
                back + m.clone()
            } else {
                back
            };
            assert_eq!(back, v);
        }
    }
}

/// Primes above 2^63 (up to 2^64 - 59) in interpolation and lifting.
#[test]
fn a6_primes_above_2_63() {
    let mut primes = Vec::new();
    let mut c = u64::MAX;
    while primes.len() < 6 {
        c = prev_prime(c - 1);
        primes.push(c);
    }
    let f = RationalFunction {
        numerator: vec![q(-3), q(0), q(7), q(-1000000007)],
        denominator: vec![q(1), q(-5), q(11)],
    };
    let mut lift = Lift::default();
    let mut rng = StdRng::seed_from_u64(3);
    for &p in &primes {
        let field = Zp64::new(p);
        let mut racer = Interpolator::default();
        while !racer.done() {
            let x = field.to_element(rng.random_range(1..p));
            if let Some(y) = f.evaluate(&field, &x) {
                racer.feed(&field, &x, &y);
            }
        }
        let (n, d) = racer.rational(&field).unwrap();
        let (en, ed) = f.image(&field).unwrap();
        assert_eq!(normalise(&field, &en, &ed).unwrap(), (n.clone(), d.clone()));
        if lift.add(&field, &n, &d) == LiftStatus::Stable {
            break;
        }
    }
    assert_eq!(lift.result(), Some(&f));
}
