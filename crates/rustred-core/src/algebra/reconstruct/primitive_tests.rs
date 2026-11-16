//! Property tests of the primitives: random univariate rational functions
//! over Q, up to degree 170/170, are interpolated modulo word-size primes
//! ([`Interpolator`]) and lifted to Q ([`Lift`]); an unlucky prime (one
//! that lowers a degree) is rejected as a shape mismatch.

use rand::{Rng, SeedableRng, rngs::StdRng};
use symbolica::domains::Field;
use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};
use symbolica::domains::integer::Integer;
use symbolica::domains::rational::Rational;

use super::interpolate::{DEFAULT_CONFIRMATIONS, Interpolator};
use super::lift::{Lift, LiftStatus};
use super::poly::{horner, normalise};
use super::rational::RationalFunction;
use super::{Zp, known::Shape};

/// Odd primes below `start`, descending.
fn primes_below(start: u64, count: usize) -> Vec<u64> {
    let mut primes = Vec::with_capacity(count);
    let mut candidate = if start.is_multiple_of(2) {
        start - 1
    } else {
        start - 2
    };
    while primes.len() < count {
        if Integer::from(candidate).is_prime(0) {
            primes.push(candidate);
        }
        candidate -= 2;
    }
    primes
}

fn q(value: i64) -> Rational {
    Rational::from(Integer::from(value))
}

/// Integer coefficients in `[-bound, bound]`; the top coefficient and the
/// constant denominator coefficient are nonzero.
fn random_function(rng: &mut StdRng, degrees: (usize, usize), bound: i64) -> RationalFunction {
    let mut draw = |len: usize, nonzero: &[usize]| -> Vec<Rational> {
        (0..len)
            .map(|index| {
                loop {
                    let value = rng.random_range(-bound..=bound);
                    if value != 0 || !nonzero.contains(&index) {
                        break q(value);
                    }
                }
            })
            .collect()
    };
    let numerator = draw(degrees.0 + 1, &[degrees.0]);
    let denominator = draw(degrees.1 + 1, &[0, degrees.1]);
    RationalFunction {
        numerator,
        denominator,
    }
}

/// The interpolated, normalised image of `f` modulo `prime`.
fn interpolate(f: &RationalFunction, prime: u64, rng: &mut StdRng) -> (Vec<Zp>, Vec<Zp>) {
    let field = Zp64::new(prime);
    let mut racer = Interpolator::new(DEFAULT_CONFIRMATIONS);
    while !racer.done() {
        let x = field.to_element(rng.random_range(1..prime));
        // A pole of this prime's image is an unlucky point: skip it.
        let Some(y) = f.evaluate(&field, &x) else {
            continue;
        };
        racer.feed(&field, &x, &y);
        assert!(!racer.failed(), "interpolation failed at prime {prime}");
    }
    racer.rational(&field).expect("terminated interpolation")
}

fn same_function(left: &RationalFunction, right: &RationalFunction, rng: &mut StdRng) {
    for _ in 0..4 {
        let x = &q(rng.random_range(-1000..1000)) / &q(rng.random_range(1..1000));
        assert_eq!(left.evaluate_rational(&x), right.evaluate_rational(&x));
    }
}

#[test]
fn random_functions_are_interpolated_and_lifted_exactly() {
    let mut rng = StdRng::seed_from_u64(0x5072_696d_6974_6976);
    let primes = primes_below(1 << 62, 12);
    for (case, degrees) in [(0, 0), (4, 0), (0, 4), (7, 5), (40, 39), (170, 170)]
        .into_iter()
        .enumerate()
    {
        let f = random_function(&mut rng, degrees, 1000);
        let mut lift = Lift::default();
        let mut status = LiftStatus::Pending;
        for &prime in &primes {
            let field = Zp64::new(prime);
            let (numerator, denominator) = interpolate(&f, prime, &mut rng);
            // The image is the normalised image of the exact function.
            let (n, d) = f.image(&field).unwrap();
            let expected = normalise(&field, &n, &d).unwrap();
            assert_eq!(
                (&numerator, &denominator),
                (&expected.0, &expected.1),
                "case {case}"
            );
            status = lift.add(&field, &numerator, &denominator);
            if status == LiftStatus::Stable {
                break;
            }
        }
        assert_eq!(status, LiftStatus::Stable, "case {case} {degrees:?}");
        let lifted = lift.result().unwrap();
        same_function(lifted, &f, &mut rng);
        assert!(lifted.degrees().0 <= degrees.0 && lifted.degrees().1 <= degrees.1);
    }
}

/// A prime dividing the leading numerator coefficient lowers the degree of
/// its image: [`Lift::add`] reports a shape mismatch and keeps the earlier
/// primes, and lifting completes with the others.
#[test]
fn an_unlucky_prime_is_a_shape_mismatch() {
    let mut rng = StdRng::seed_from_u64(7);
    let primes = primes_below(1 << 31, 8);
    let unlucky = primes[1];
    let mut f = random_function(&mut rng, (6, 3), 50);
    f.numerator[6] = q(unlucky as i64);
    let mut lift = Lift::default();
    let mut statuses = Vec::new();
    for &prime in &primes {
        let field = Zp64::new(prime);
        let (numerator, denominator) = interpolate(&f, prime, &mut rng);
        let status = lift.add(&field, &numerator, &denominator);
        statuses.push(status);
        if prime == unlucky {
            assert_eq!(status, LiftStatus::ShapeMismatch);
            assert!(Shape::of(&field, &numerator, &denominator).numerator < 6);
        }
        if status == LiftStatus::Stable {
            break;
        }
    }
    assert_eq!(statuses.last(), Some(&LiftStatus::Stable), "{statuses:?}");
    assert!(!lift.primes().contains(&unlucky));
    same_function(lift.result().unwrap(), &f, &mut rng);
    // Horner on the exact image agrees with evaluation.
    let field = Zp64::new(primes[0]);
    let (n, d) = f.image(&field).unwrap();
    let x = field.to_element(12345);
    let y = field.div(&horner(&field, &n, &x), &horner(&field, &d, &x));
    assert_eq!(f.evaluate(&field, &x), Some(y));
}
