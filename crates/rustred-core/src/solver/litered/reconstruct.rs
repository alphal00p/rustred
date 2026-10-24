//! Univariate rational reconstruction over Zp (Thiele continued fractions with
//! early termination), multi-prime CRT and maximal-quotient reconstruction of
//! the normalized polynomial coefficients.
//!
//! Normalization: the denominator's lowest-order nonzero coefficient is one,
//! so every prime yields the same exact representative.

use symbolica::domains::integer::Integer;
use symbolica::domains::rational::Rational;

use super::zp::PrimeField;

/// `numerator(x) / denominator(x)`, dense coefficients from degree zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RationalFunction {
    pub numerator: Vec<Rational>,
    pub denominator: Vec<Rational>,
}

impl RationalFunction {
    pub fn is_zero(&self) -> bool {
        self.numerator.iter().all(Rational::is_zero)
    }

    /// Value at a Zp point; `None` at a pole or if a coefficient denominator
    /// vanishes mod p.
    pub fn evaluate(&self, x: u64, field: &PrimeField) -> Option<u64> {
        let horner = |coefficients: &[Rational]| -> Option<u64> {
            let mut value = 0u64;
            for coefficient in coefficients.iter().rev() {
                value = field.add(field.mul(value, x), field.from_rational(coefficient)?);
            }
            Some(value)
        };
        let numerator = horner(&self.numerator)?;
        let denominator = horner(&self.denominator)?;
        field
            .inv(denominator)
            .map(|inverse| field.mul(numerator, inverse))
    }
}

/// Incremental Thiele interpolation `f(x) = a0 + (x-x0)/(a1 + (x-x1)/(...))`.
#[derive(Clone, Debug, Default)]
pub(crate) struct Thiele {
    xs: Vec<u64>,
    coefficients: Vec<u64>,
    confirmations: u32,
    done: bool,
    failed: bool,
}

/// Consecutive correct predictions required before an entry is complete.
const CONFIRMATIONS: u32 = 2;

impl Thiele {
    pub fn done(&self) -> bool {
        self.done || self.failed
    }
    pub fn failed(&self) -> bool {
        self.failed
    }
    pub fn points(&self) -> usize {
        self.xs.len()
    }

    fn evaluate(&self, x: u64, field: &PrimeField) -> Option<u64> {
        let mut value = *self.coefficients.last()?;
        for j in (0..self.coefficients.len() - 1).rev() {
            let inverse = field.inv(value)?;
            value = field.add(
                self.coefficients[j],
                field.mul(field.sub(x, self.xs[j]), inverse),
            );
        }
        Some(value)
    }

    pub fn feed(&mut self, x: u64, y: u64, field: &PrimeField) {
        if self.done() {
            return;
        }
        if self.evaluate(x, field) == Some(y) {
            self.confirmations += 1;
            if self.confirmations >= CONFIRMATIONS {
                self.done = true;
            }
            return;
        }
        self.confirmations = 0;
        let mut t = y;
        for j in 0..self.coefficients.len() {
            let difference = field.sub(t, self.coefficients[j]);
            let Some(inverse) = field.inv(difference) else {
                self.failed = true;
                return;
            };
            t = field.mul(field.sub(x, self.xs[j]), inverse);
        }
        self.coefficients.push(t);
        self.xs.push(x);
    }

    /// Normalized `(numerator, denominator)`; `None` if degenerate.
    pub fn polynomials(&self, field: &PrimeField) -> Option<(Vec<u64>, Vec<u64>)> {
        let k = self.coefficients.len().checked_sub(1)?;
        let mut numerator = vec![self.coefficients[k]];
        let mut denominator = vec![1u64];
        for j in (0..k).rev() {
            // (a_j N + (x - x_j) D, N)
            let width = numerator.len().max(denominator.len() + 1);
            let mut next = vec![0u64; width];
            for (i, &c) in numerator.iter().enumerate() {
                next[i] = field.mul_add(next[i], self.coefficients[j], c);
            }
            let shift = field.neg(self.xs[j]);
            for (i, &c) in denominator.iter().enumerate() {
                next[i + 1] = field.add(next[i + 1], c);
                next[i] = field.mul_add(next[i], shift, c);
            }
            denominator = std::mem::replace(&mut numerator, next);
        }
        trim(&mut numerator);
        trim(&mut denominator);
        let lowest = *denominator.iter().find(|&&c| c != 0)?;
        let inverse = field.inv(lowest)?;
        for c in numerator.iter_mut().chain(denominator.iter_mut()) {
            *c = field.mul(*c, inverse);
        }
        if numerator.is_empty() {
            numerator.push(0);
            denominator = vec![1];
        }
        Some((numerator, denominator))
    }
}

fn trim(coefficients: &mut Vec<u64>) {
    while coefficients.len() > 1 && *coefficients.last().unwrap() == 0 {
        coefficients.pop();
    }
}

/// CRT accumulation of normalized modular polynomials of fixed shape.
#[derive(Clone, Debug, Default)]
pub(crate) struct CrtAccumulator {
    shape: Option<(usize, usize)>,
    residues: Vec<Integer>,
}

impl CrtAccumulator {
    /// Add one prime's image; `false` (and no change) on a shape mismatch.
    pub fn add(
        &mut self,
        numerator: &[u64],
        denominator: &[u64],
        prime: u64,
        modulus: &Integer,
    ) -> bool {
        let shape = (numerator.len(), denominator.len());
        let values = numerator.iter().chain(denominator);
        match self.shape {
            None => {
                self.shape = Some(shape);
                self.residues = values.map(|&v| Integer::from(v)).collect();
                true
            }
            Some(old) if old != shape => false,
            Some(_) => {
                let prime = Integer::from(prime);
                self.residues = std::mem::take(&mut self.residues)
                    .into_iter()
                    .zip(values)
                    .map(|(old, &new)| {
                        Integer::chinese_remainder(
                            old,
                            Integer::from(new),
                            modulus.clone(),
                            prime.clone(),
                        )
                    })
                    .collect();
                true
            }
        }
    }

    pub fn shape(&self) -> Option<(usize, usize)> {
        self.shape
    }

    pub fn reconstruct(&self, modulus: &Integer) -> Option<RationalFunction> {
        let (n, _) = self.shape?;
        let values: Option<Vec<Rational>> = self
            .residues
            .iter()
            .map(|value| Rational::maximal_quotient_reconstruction(value, modulus, None).ok())
            .collect();
        let mut values = values?;
        let denominator = values.split_off(n);
        Some(RationalFunction {
            numerator: values,
            denominator,
        })
    }

    /// Whether `candidate` reduces to the given normalized image mod `prime`.
    pub fn matches(
        candidate: &RationalFunction,
        numerator: &[u64],
        denominator: &[u64],
        field: &PrimeField,
    ) -> bool {
        candidate.numerator.len() == numerator.len()
            && candidate.denominator.len() == denominator.len()
            && candidate
                .numerator
                .iter()
                .chain(&candidate.denominator)
                .zip(numerator.iter().chain(denominator))
                .all(|(value, &image)| field.from_rational(value) == Some(image))
    }
}
