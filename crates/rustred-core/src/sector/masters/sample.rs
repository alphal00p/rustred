//! Seeded modular sample points and the specialization of `G` at them.

use std::collections::{BTreeSet, HashMap};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use symbolica::domains::finite_field::{FiniteFieldCore, PrimeIteratorU64, ToFiniteField, Zp};
use symbolica::prelude::{Field, Ring};

use crate::algebra::{Coefficient, CoefficientPolynomial};
use crate::sector::Mask;

use super::error::MasterCountError;
use super::ideal::ModularElement;

/// Primes lie above `2^31` so that unlucky reductions are rare.
const PRIME_LOWER: u64 = 1 << 31;
/// Symbolica's 32-bit `F4` row reduction keeps products below `p^2` in an
/// `i64`, so the largest admissible modulus is `floor(sqrt(i64::MAX))`.
const PRIME_UPPER: u64 = 3_037_000_499;
/// Seeded starts stay this far below [`PRIME_UPPER`]; prime gaps in this range
/// are below a thousand.
const PRIME_WINDOW: u64 = 1 << 20;

/// Exact family data a modular point must keep generic.
#[derive(Debug)]
pub(super) struct Kinematics {
    pub(super) parameters: usize,
    pub(super) denominators: usize,
    /// Terms of `G = U + F` over all Feynman parameters.
    pub(super) g: Vec<(Coefficient, Box<[u16]>)>,
    /// Family-domain and zero-analysis conditions that must stay nonzero.
    pub(super) conditions: Vec<CoefficientPolynomial>,
    /// Exact external Gram determinant when it is required to stay nonzero:
    /// `None` for vacuum families and identically singular Gram matrices.
    pub(super) gram_determinant: Option<Coefficient>,
}

/// One specialization of the kinematics and generic Euler exponents.
#[derive(Debug)]
pub(super) struct SamplePoint {
    /// Terms of `G` with nonzero modular coefficients; the support of the
    /// exact `G` is preserved.
    pub(super) g: Vec<(ModularElement, Box<[u16]>)>,
    /// Euler exponent `u_i` for every Feynman parameter.
    pub(super) exponents: Vec<ModularElement>,
    /// Memoized torus Euler counts `c(T)` at this point.
    pub(super) euler: HashMap<Mask, usize>,
}

/// The deterministic point stream of one sample.
#[derive(Debug)]
pub(super) struct SampleStream {
    index: usize,
    field: Zp,
    rng: StdRng,
    attempts: usize,
    point: Option<SamplePoint>,
}

impl SampleStream {
    /// Streams with pairwise distinct primes in
    /// `(PRIME_LOWER, PRIME_UPPER]` and independent point seeds, all derived
    /// from `seed`.
    pub(super) fn seeded(seed: u64, samples: usize) -> Vec<Self> {
        let mut master = StdRng::seed_from_u64(seed);
        let mut used = BTreeSet::new();
        (0..samples)
            .map(|index| {
                let start = master.random_range(PRIME_LOWER..PRIME_UPPER - PRIME_WINDOW);
                let prime = distinct_prime(start, &used);
                used.insert(prime);
                Self {
                    index,
                    field: Zp::new(prime),
                    rng: StdRng::seed_from_u64(master.random()),
                    attempts: 0,
                    point: None,
                }
            })
            .collect()
    }

    /// The current admissible point, drawing new ones as needed.
    pub(super) fn point(
        &mut self,
        kinematics: &Kinematics,
        max_attempts: usize,
    ) -> Result<(&Zp, &mut SamplePoint), MasterCountError> {
        while self.point.is_none() {
            if self.attempts >= max_attempts {
                return Err(self.exhausted());
            }
            self.attempts += 1;
            self.point = kinematics.specialize(&self.field, &mut self.rng)?;
        }
        let exhausted = self.exhausted();
        self.point
            .as_mut()
            .map(|point| (&self.field, point))
            .ok_or(exhausted)
    }

    /// Give one sector's count a fresh attempt budget.
    pub(super) fn reset_attempts(&mut self) {
        self.attempts = 0;
    }

    /// Discard an unlucky point; the next request draws a fresh one.
    pub(super) fn discard(&mut self) {
        self.point = None;
    }

    fn exhausted(&self) -> MasterCountError {
        MasterCountError::AttemptsExhausted {
            sample: self.index,
            attempts: self.attempts,
        }
    }
}

/// The first prime after `start` not in `used`, wrapping to [`PRIME_LOWER`]
/// past [`PRIME_UPPER`]. The range holds tens of millions of primes, far
/// more than any sample count, so the search terminates.
fn distinct_prime(start: u64, used: &BTreeSet<u32>) -> u32 {
    let mut primes = PrimeIteratorU64::new(start);
    loop {
        match primes.next() {
            Some(prime) if prime > PRIME_UPPER => primes = PrimeIteratorU64::new(PRIME_LOWER),
            Some(prime) => {
                // PRIME_UPPER < 2^32, so the conversion is exact.
                if let Ok(prime) = u32::try_from(prime)
                    && !used.contains(&prime)
                {
                    return prime;
                }
            }
            None => primes = PrimeIteratorU64::new(PRIME_LOWER),
        }
    }
}

impl Kinematics {
    /// Specialize at the next random point of `rng`, or `None` when the point
    /// is inadmissible: a domain condition, the Gram determinant, or a
    /// numerator or denominator of a `G` coefficient vanishes.
    fn specialize(
        &self,
        field: &Zp,
        rng: &mut StdRng,
    ) -> Result<Option<SamplePoint>, MasterCountError> {
        // Draw everything up front so that every attempt advances the stream
        // by the same amount.
        let prime = field.get_prime();
        let mut draw = |count: usize| -> Vec<ModularElement> {
            (0..count)
                .map(|_| field.to_element(rng.random_range(1..prime)))
                .collect()
        };
        let point = draw(self.parameters);
        let exponents = draw(self.denominators);

        let vanishes = |polynomial: &CoefficientPolynomial| {
            evaluate(polynomial, &point, field).map(|value| field.is_zero(&value))
        };
        for condition in &self.conditions {
            if vanishes(condition)? {
                return Ok(None);
            }
        }
        if let Some(determinant) = &self.gram_determinant
            && (vanishes(&determinant.numerator)? || vanishes(&determinant.denominator)?)
        {
            return Ok(None);
        }
        let mut g = Vec::with_capacity(self.g.len());
        for (coefficient, monomial) in &self.g {
            let numerator = evaluate(&coefficient.numerator, &point, field)?;
            let denominator = evaluate(&coefficient.denominator, &point, field)?;
            if field.is_zero(&numerator) || field.is_zero(&denominator) {
                return Ok(None);
            }
            g.push((field.div(&numerator, &denominator), monomial.clone()));
        }
        Ok(Some(SamplePoint {
            g,
            exponents,
            euler: HashMap::new(),
        }))
    }
}

fn evaluate(
    polynomial: &CoefficientPolynomial,
    point: &[ModularElement],
    field: &Zp,
) -> Result<ModularElement, MasterCountError> {
    if polynomial.nvars() != point.len() {
        return Err(MasterCountError::ForeignCoefficient {
            expected: point.len(),
            actual: polynomial.nvars(),
        });
    }
    Ok(polynomial.evaluate_with_coeff_map(
        |coefficient| coefficient.to_finite_field(field),
        point,
        field,
    ))
}
