//! Multi-prime lifting of normalised images to Q (FireFly,
//! arXiv:1904.00009 §2.3).
//!
//! The coefficients of the normalised images of one function are combined
//! by the Chinese remainder theorem (Symbolica's `Integer::chinese_remainder`,
//! Garner). After each prime, Wang's reconstruction and Monagan's MQRR are
//! raced on the accumulated residues. A guess is accepted ("probably
//! correct") once either method returns the same function for two
//! consecutive moduli; the caller then checks it in a fresh prime (FireFly
//! Alg. 3, see [`super::driver`]). An unlucky prime can only lower
//! degrees or raise the normalisation index, so an image of a strictly larger
//! shape restarts the accumulation (an unlucky FIRST prime is recovered), and
//! an image of a smaller shape is rejected.

use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};
use symbolica::domains::integer::Integer;
use symbolica::domains::rational::Rational;

use super::Zp;
use super::known::Shape;
use super::rational::{RationalFunction, mqrr_reconstruction, wang_reconstruction};

/// Status after one more prime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiftStatus {
    /// This prime was already accumulated; nothing was changed.
    DuplicatePrime,
    /// The image has a smaller or incomparable shape than the accumulated
    /// ones: this prime is unlucky. Nothing was changed.
    ShapeMismatch,
    /// The image's shape strictly dominates the accumulated one, so the
    /// earlier primes were unlucky: the accumulation restarted from this
    /// image (see [`Lift::accumulate`]).
    Restarted,
    /// Accumulated; no guess has been stable yet.
    Pending,
    /// Wang or MQRR returned the same function at this and the previous
    /// modulus.
    Stable,
}

/// Which rational reconstruction produced an accepted guess.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiftMethod {
    Wang,
    Mqrr,
}

/// CRT accumulation and Wang/MQRR race for one function.
#[derive(Clone, Debug, Default)]
pub struct Lift {
    shape: Option<Shape>,
    residues: Vec<Integer>,
    modulus: Option<Integer>,
    primes: Vec<u64>,
    previous: [Option<RationalFunction>; 2],
    stable: Option<(RationalFunction, LiftMethod)>,
}

impl Lift {
    pub fn shape(&self) -> Option<Shape> {
        self.shape
    }

    /// Primes accumulated so far.
    pub fn primes(&self) -> &[u64] {
        &self.primes
    }

    /// The accepted guess, if the last prime made it stable.
    pub fn result(&self) -> Option<&RationalFunction> {
        self.stable.as_ref().map(|(function, _)| function)
    }

    pub fn method(&self) -> Option<LiftMethod> {
        self.stable.as_ref().map(|(_, method)| *method)
    }

    /// Product of the accumulated primes.
    pub fn modulus(&self) -> Option<&Integer> {
        self.modulus.as_ref()
    }

    /// Accumulate the normalised image `numerator / denominator` modulo
    /// `field`'s prime and race the reconstructions ([`Self::accumulate`]
    /// followed by [`Self::race`]).
    pub fn add(&mut self, field: &Zp64, numerator: &[Zp], denominator: &[Zp]) -> LiftStatus {
        match self.accumulate(field, numerator, denominator) {
            LiftStatus::Pending => self.race(),
            LiftStatus::Restarted => match self.race() {
                // A single prime cannot be stable; keep the restart visible.
                LiftStatus::Pending => LiftStatus::Restarted,
                status => status,
            },
            status => status,
        }
    }

    /// CRT step only: accumulate the image without reconstructing.
    ///
    /// - `DuplicatePrime`: this prime is already accumulated; nothing changes.
    /// - `ShapeMismatch`: the image has a smaller (or incomparable) shape than
    ///   the accumulated ones, so this prime is unlucky; nothing changes.
    /// - `Restarted`: the image's shape strictly dominates the accumulated one
    ///   ([`Shape::dominates`]): an unlucky prime can only lower degrees or
    ///   raise the normalisation index, so the EARLIER primes were unlucky.
    ///   The accumulation restarts from this image.
    /// - `Pending`: accumulated.
    ///
    /// The previous guesses and any accepted result are cleared, so the next
    /// [`Self::race`] decides stability afresh.
    pub fn accumulate(&mut self, field: &Zp64, numerator: &[Zp], denominator: &[Zp]) -> LiftStatus {
        debug_assert!(!numerator.is_empty() && !denominator.is_empty());
        debug_assert!(
            Integer::from(field.get_prime()).is_prime(0),
            "Lift needs a prime field"
        );
        let shape = Shape::of(field, numerator, denominator);
        let prime_u64 = field.get_prime();
        if self.primes.contains(&prime_u64) {
            return LiftStatus::DuplicatePrime;
        }
        let prime = Integer::from(prime_u64);
        let values = numerator
            .iter()
            .chain(denominator)
            .map(|value| Integer::from(field.from_element(value)));
        let mut status = LiftStatus::Pending;
        match (&self.shape, &self.modulus) {
            (Some(old), _) if *old != shape && !shape.dominates(old) => {
                return LiftStatus::ShapeMismatch;
            }
            (Some(old), Some(modulus)) if *old == shape => {
                let modulus = modulus.clone();
                self.residues = std::mem::take(&mut self.residues)
                    .into_iter()
                    .zip(values)
                    .map(|(old, new)| {
                        Integer::chinese_remainder(old, new, modulus.clone(), prime.clone())
                    })
                    .collect();
                self.modulus = Some(&modulus * &prime);
            }
            (old, _) => {
                if old.is_some() {
                    status = LiftStatus::Restarted;
                    self.primes.clear();
                    self.previous = [None, None];
                }
                self.shape = Some(shape);
                self.residues = values.collect();
                self.modulus = Some(prime);
            }
        }
        self.primes.push(prime_u64);
        self.stable = None;
        status
    }

    /// Race Wang and MQRR on the accumulated residues: `Stable` when either
    /// returns the same function as at the previous call (two consecutive
    /// moduli), `Pending` otherwise.
    ///
    /// `Stable` is a probabilistic acceptance: a wrong guess survives one
    /// more prime with probability of order `1/p` per wrong coefficient. A
    /// result must be checked at a fresh prime before it is used (the
    /// black-box driver does).
    pub fn race(&mut self) -> LiftStatus {
        let (Some(shape), Some(modulus)) = (self.shape, self.modulus.clone()) else {
            return LiftStatus::Pending;
        };
        let split = shape.numerator + 1;
        let attempt = |reconstruct: fn(&Integer, &Integer) -> Option<Rational>| {
            let values: Option<Vec<Rational>> = self
                .residues
                .iter()
                .map(|value| reconstruct(value, &modulus))
                .collect();
            values.map(|mut values| {
                let denominator = values.split_off(split);
                RationalFunction {
                    numerator: values,
                    denominator,
                }
            })
        };
        let guesses = [attempt(wang_reconstruction), attempt(mqrr_reconstruction)];
        let mut stable = None;
        for (index, guess) in guesses.into_iter().enumerate() {
            if let Some(guess) = &guess
                && stable.is_none()
                && self.previous[index].as_ref() == Some(guess)
            {
                let method = if index == 0 {
                    LiftMethod::Wang
                } else {
                    LiftMethod::Mqrr
                };
                stable = Some((guess.clone(), method));
            }
            self.previous[index] = guess;
        }
        let status = if stable.is_some() {
            LiftStatus::Stable
        } else {
            LiftStatus::Pending
        };
        self.stable = stable;
        status
    }

    /// Whether `candidate` reduces to the given normalised image.
    pub fn matches(
        candidate: &RationalFunction,
        field: &Zp64,
        numerator: &[Zp],
        denominator: &[Zp],
    ) -> bool {
        match candidate.image(field) {
            Some((n, d)) => n == numerator && d == denominator,
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::reconstruct::interpolate::Thiele;
    use crate::algebra::reconstruct::known::fit_known_shape;
    use symbolica::domains::{Field, RingOps};
    use symbolica::prelude::Ring;

    #[test]
    fn thiele_then_known_shapes_lift_to_the_exact_function() {
        // f(x) = (3x^2 - 1/2) / (x (x + 7)), normalised at degree 1:
        // (-1/14 + 3/7 x^2) / (x + x^2/7).
        let primes = [
            4611686018427387847u64,
            4611686018427387817,
            4611686018427387787,
        ];
        let mut lift = Lift::default();
        let mut shape = None;
        let mut statuses = Vec::new();
        for (round, &prime) in primes.iter().enumerate() {
            let field = Zp64::new(prime);
            let e = |v: u64| field.to_element(v);
            let f = |x: &Zp| {
                let half = field.inv(&e(2));
                let n = field.sub(&field.mul(&e(3), &field.mul(x, x)), &half);
                field.div(&n, &field.mul(x, &field.add(x, &e(7))))
            };
            let xs: Vec<Zp> = (0..12).map(|i| e(1000 + 17 * i + round as u64)).collect();
            let ys: Vec<Zp> = xs.iter().map(&f).collect();
            let image = match shape {
                None => {
                    let mut thiele = Thiele::default();
                    for (x, y) in xs.iter().zip(&ys) {
                        thiele.feed(&field, x, y);
                    }
                    assert!(thiele.done() && !thiele.failed());
                    let image = thiele.rational(&field).unwrap();
                    shape = Some(Shape::of(&field, &image.0, &image.1));
                    image
                }
                Some(shape) => {
                    let points = shape.unknowns() + 1;
                    fit_known_shape(&field, &shape, &xs[..points], &ys[..points]).unwrap()
                }
            };
            statuses.push(lift.add(&field, &image.0, &image.1));
            let _ = field.zero();
        }
        assert_eq!(
            shape.map(|s| (s.numerator, s.denominator, s.low)),
            Some((2, 2, 1))
        );
        assert_eq!(statuses.last(), Some(&LiftStatus::Stable));
        let q = |n: i64, d: i64| Rational::from((Integer::from(n), Integer::from(d)));
        let result = lift.result().unwrap();
        assert_eq!(result.numerator, vec![q(-1, 14), q(0, 1), q(3, 7)]);
        assert_eq!(result.denominator, vec![q(0, 1), q(1, 1), q(1, 7)]);
    }
}
