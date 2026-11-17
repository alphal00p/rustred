//! Incremental univariate interpolation over `Zp64` with early termination.
//!
//! - [`Newton`]: polynomials by Newton divided differences; a new point
//!   whose divided difference vanishes is a confirmation (FireFly,
//!   arXiv:1904.00009 §2.1, early termination with `eta` extra points).
//! - [`Thiele`]: rational functions by Thiele's continued fraction
//!   `f(x) = a0 + (x - x0) / (a1 + (x - x1) / (a2 + ...))`; a new point the
//!   current fraction already predicts is a confirmation (FireFly §2.2). An
//!   accidental zero in the recursion skips the point (unlucky point,
//!   bounded by Schwartz-Zippel; FireFly §2.2).
//! - [`Interpolator`]: races both on the same points and stops with the
//!   first that terminates, so a polynomial costs `deg + 1 + eta` points
//!   instead of Thiele's `2 deg + 1 + eta` (FireFly 2, arXiv:2004.01463
//!   §2.1, races interpolation strategies on shared probes).
//!
//! Every point is used at most once: a point fed again (stored or used as a
//! confirmation before) is ignored, so a repeated point never counts as a
//! second confirmation.
//!
//! Termination is probabilistic: a wrong early termination needs the
//! function and the current interpolant to agree at `confirmations` random
//! points, which happens with probability about `((deg N + deg D + k) / p)`
//! per confirmation for a function of degrees `(deg N, deg D)` after `k`
//! stored points (Schwartz-Zippel). Results must be verified at a fresh prime
//! before use (the driver does).

use std::collections::HashSet;

use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};
use symbolica::domains::{Field, RingOps};
use symbolica::prelude::Ring;

use super::Zp;
use super::poly::{normalise, trim};

/// Consecutive correct predictions before an interpolation is complete.
pub const DEFAULT_CONFIRMATIONS: u32 = 2;
/// Accidental recursion zeros tolerated before an interpolation fails.
const MAX_SKIPS: u32 = 8;

/// Incremental Newton interpolation of a polynomial.
#[derive(Clone, Debug)]
pub struct Newton {
    xs: Vec<Zp>,
    /// Every point fed (stored or confirmation), as canonical residues.
    seen: HashSet<u64>,
    coefficients: Vec<Zp>,
    confirmations: u32,
    needed: u32,
    done: bool,
}

impl Default for Newton {
    fn default() -> Self {
        Self::new(DEFAULT_CONFIRMATIONS)
    }
}

impl Newton {
    pub fn new(confirmations: u32) -> Self {
        Self {
            xs: Vec::new(),
            seen: HashSet::new(),
            coefficients: Vec::new(),
            confirmations: 0,
            needed: confirmations.max(1),
            done: false,
        }
    }

    pub fn done(&self) -> bool {
        self.done
    }

    /// Points stored in the Newton form.
    pub fn points(&self) -> usize {
        self.xs.len()
    }

    /// Feed one point with a value; points fed before (stored or used as a
    /// confirmation) are ignored.
    pub fn feed(&mut self, field: &Zp64, x: &Zp, y: &Zp) {
        if self.done || !self.seen.insert(field.from_element(x)) {
            return;
        }
        let mut t = *y;
        for (xj, aj) in self.xs.iter().zip(&self.coefficients) {
            t = field.div(&field.sub(&t, aj), &field.sub(x, xj));
        }
        if !self.coefficients.is_empty() && field.is_zero(&t) {
            self.confirmations += 1;
            if self.confirmations >= self.needed {
                self.done = true;
            }
            return;
        }
        self.confirmations = 0;
        self.coefficients.push(t);
        self.xs.push(*x);
    }

    /// Monomial coefficients, trimmed.
    pub fn polynomial(&self, field: &Zp64) -> Vec<Zp> {
        let mut result = vec![field.zero()];
        for k in (0..self.coefficients.len()).rev() {
            // result = result * (x - x_k) + a_k
            let mut next = vec![field.zero(); result.len() + 1];
            let shift = field.neg(&self.xs[k]);
            for (i, c) in result.iter().enumerate() {
                field.add_assign(&mut next[i + 1], c);
                field.add_mul_assign(&mut next[i], &shift, c);
            }
            field.add_assign(&mut next[0], &self.coefficients[k]);
            result = next;
        }
        trim(field, &mut result);
        result
    }
}

/// Incremental Thiele interpolation of a rational function.
#[derive(Clone, Debug)]
pub struct Thiele {
    xs: Vec<Zp>,
    /// Every point fed (stored, confirmation or skipped), as canonical
    /// residues.
    seen: HashSet<u64>,
    coefficients: Vec<Zp>,
    confirmations: u32,
    needed: u32,
    done: bool,
    skipped: u32,
    failed: bool,
}

impl Default for Thiele {
    fn default() -> Self {
        Self::new(DEFAULT_CONFIRMATIONS)
    }
}

impl Thiele {
    pub fn new(confirmations: u32) -> Self {
        Self {
            xs: Vec::new(),
            seen: HashSet::new(),
            coefficients: Vec::new(),
            confirmations: 0,
            needed: confirmations.max(1),
            done: false,
            skipped: 0,
            failed: false,
        }
    }

    /// Terminated, successfully or not.
    pub fn done(&self) -> bool {
        self.done || self.failed
    }

    /// Too many accidental zeros: the points (or the prime) are unlucky.
    pub fn failed(&self) -> bool {
        self.failed
    }

    pub fn points(&self) -> usize {
        self.xs.len()
    }

    fn predict(&self, field: &Zp64, x: &Zp) -> Option<Zp> {
        let mut value = *self.coefficients.last()?;
        for j in (0..self.coefficients.len() - 1).rev() {
            let inverse = field.try_inv(&value)?;
            value = field.add(
                &self.coefficients[j],
                &field.mul(&field.sub(x, &self.xs[j]), &inverse),
            );
        }
        Some(value)
    }

    /// Feed one point with a value; points fed before are ignored.
    pub fn feed(&mut self, field: &Zp64, x: &Zp, y: &Zp) {
        if self.done() || !self.seen.insert(field.from_element(x)) {
            return;
        }
        if self.predict(field, x).as_ref() == Some(y) {
            self.confirmations += 1;
            if self.confirmations >= self.needed {
                self.done = true;
            }
            return;
        }
        self.confirmations = 0;
        let mut t = *y;
        for j in 0..self.coefficients.len() {
            let difference = field.sub(&t, &self.coefficients[j]);
            let Some(inverse) = field.try_inv(&difference) else {
                self.skipped += 1;
                if self.skipped > MAX_SKIPS {
                    self.failed = true;
                }
                return;
            };
            t = field.mul(&field.sub(x, &self.xs[j]), &inverse);
        }
        self.coefficients.push(t);
        self.xs.push(*x);
    }

    /// Normalised `(numerator, denominator)` of the continued fraction;
    /// `None` when it is degenerate.
    pub fn rational(&self, field: &Zp64) -> Option<(Vec<Zp>, Vec<Zp>)> {
        let k = self.coefficients.len().checked_sub(1)?;
        let mut numerator = vec![self.coefficients[k]];
        let mut denominator = vec![field.one()];
        for j in (0..k).rev() {
            // a_j + (x - x_j) D / N = (a_j N + (x - x_j) D) / N
            let width = numerator.len().max(denominator.len() + 1);
            let mut next = vec![field.zero(); width];
            for (i, c) in numerator.iter().enumerate() {
                field.add_mul_assign(&mut next[i], &self.coefficients[j], c);
            }
            let shift = field.neg(&self.xs[j]);
            for (i, c) in denominator.iter().enumerate() {
                field.add_assign(&mut next[i + 1], c);
                field.add_mul_assign(&mut next[i], &shift, c);
            }
            denominator = std::mem::replace(&mut numerator, next);
        }
        normalise(field, &numerator, &denominator)
    }
}

/// Newton and Thiele raced on the same points.
#[derive(Clone, Debug)]
pub struct Interpolator {
    newton: Newton,
    thiele: Thiele,
}

impl Default for Interpolator {
    fn default() -> Self {
        Self::new(DEFAULT_CONFIRMATIONS)
    }
}

impl Interpolator {
    pub fn new(confirmations: u32) -> Self {
        Self {
            newton: Newton::new(confirmations),
            thiele: Thiele::new(confirmations),
        }
    }

    /// One of the two terminated successfully.
    pub fn done(&self) -> bool {
        self.newton.done() || (self.thiele.done() && !self.thiele.failed())
    }

    /// Thiele failed (too many accidental zeros) and Newton has not
    /// terminated. Newton keeps being fed, so a polynomial can still finish;
    /// a rational function cannot, and the caller's point budget ends it.
    pub fn failed(&self) -> bool {
        !self.newton.done() && self.thiele.failed()
    }

    pub fn feed(&mut self, field: &Zp64, x: &Zp, y: &Zp) {
        if self.done() {
            return;
        }
        self.newton.feed(field, x, y);
        self.thiele.feed(field, x, y);
    }

    /// Normalised image, once [`Self::done`] and not [`Self::failed`].
    pub fn rational(&self, field: &Zp64) -> Option<(Vec<Zp>, Vec<Zp>)> {
        if self.newton.done() {
            return normalise(field, &self.newton.polynomial(field), &[field.one()]);
        }
        if self.thiele.done() && !self.thiele.failed() {
            return self.thiele.rational(field);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use symbolica::domains::finite_field::FiniteFieldCore;

    fn e(field: &Zp64, v: u64) -> Zp {
        field.to_element(v)
    }

    #[test]
    fn newton_terminates_after_degree_plus_confirmations() {
        let field = Zp64::new((1 << 61) - 1);
        // 3 x^3 - x + 5
        let f = |x: &Zp| {
            let x3 = field.mul(&field.mul(x, x), x);
            field.add(&field.sub(&field.mul(&e(&field, 3), &x3), x), &e(&field, 5))
        };
        let mut newton = Newton::default();
        let mut fed = 0;
        for i in 0..20 {
            let x = e(&field, 101 + 7 * i);
            newton.feed(&field, &x, &f(&x));
            fed += 1;
            if newton.done() {
                break;
            }
        }
        assert!(newton.done());
        assert_eq!(fed, 4 + 2);
        let poly = newton.polynomial(&field);
        assert_eq!(
            poly,
            vec![
                e(&field, 5),
                field.neg(&e(&field, 1)),
                field.zero(),
                e(&field, 3)
            ]
        );
    }

    #[test]
    fn interpolator_reconstructs_a_rational_function() {
        let field = Zp64::new((1 << 61) - 1);
        // (x^2 + 3) / (x - 4)^2: normalised denominator 1 - x/2 + x^2/16.
        let f = |x: &Zp| {
            let n = field.add(&field.mul(x, x), &e(&field, 3));
            let t = field.sub(x, &e(&field, 4));
            field.div(&n, &field.mul(&t, &t))
        };
        let mut racer = Interpolator::default();
        for i in 0..40 {
            let x = e(&field, 1000 + 13 * i);
            racer.feed(&field, &x, &f(&x));
            if racer.done() {
                break;
            }
        }
        assert!(racer.done() && !racer.failed());
        let (n, d) = racer.rational(&field).unwrap();
        let sixteenth = field.inv(&e(&field, 16));
        assert_eq!(
            n,
            vec![
                field.mul(&e(&field, 3), &sixteenth),
                field.zero(),
                sixteenth
            ]
        );
        assert_eq!(
            d,
            vec![field.one(), field.neg(&field.inv(&e(&field, 2))), sixteenth]
        );
    }
}
