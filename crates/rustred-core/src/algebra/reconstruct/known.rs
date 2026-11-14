//! Interpolation with known degrees (FireFly, arXiv:1904.00009 §2.3: after
//! the first prime the support is known, and later primes only solve for
//! the coefficients).
//!
//! With numerator degree `n`, denominator degree `m` and the denominator
//! normalised at its lowest nonzero degree `l` (coefficient one, lower
//! coefficients zero), `f = N / D` has `n + 1 + m - l` unknowns. Each point
//! gives one linear equation `N(x) - y (D(x) - x^l) = y x^l`; extra points
//! are checks.

use symbolica::domains::finite_field::Zp64;
use symbolica::domains::{Field, RingOps};
use symbolica::prelude::Ring;

use super::Zp;
use super::poly::{horner, is_zero, low_degree, normalise, trim};

/// Degrees and normalisation index of a normalised image.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Shape {
    pub numerator: usize,
    pub denominator: usize,
    /// Lowest-degree nonzero denominator coefficient (fixed to one).
    pub low: usize,
    pub zero: bool,
}

impl Shape {
    /// Shape of a normalised, trimmed image.
    pub fn of(field: &Zp64, numerator: &[Zp], denominator: &[Zp]) -> Self {
        debug_assert!(!numerator.is_empty() && !denominator.is_empty());
        Self {
            numerator: numerator.len() - 1,
            denominator: denominator.len() - 1,
            low: low_degree(field, denominator),
            zero: is_zero(field, numerator),
        }
    }

    /// Unknown coefficients (points needed without a check point).
    pub fn unknowns(&self) -> usize {
        if self.zero {
            0
        } else {
            self.numerator + 1 + self.denominator - self.low
        }
    }

    pub fn is_polynomial(&self) -> bool {
        self.zero || self.denominator == 0
    }

    /// `self` strictly dominates `other`: it differs from it, and it is what
    /// `other` can become under a lucky prime when `other` came from an
    /// unlucky one (no degree smaller, normalisation index no larger, nonzero
    /// whenever `other` is). An unlucky prime only lowers degrees, raises the
    /// index or zeroes the image.
    pub fn dominates(&self, other: &Shape) -> bool {
        self != other
            && (other.zero
                || (!self.zero
                    && self.numerator >= other.numerator
                    && self.denominator >= other.denominator
                    && self.low <= other.low))
    }
}

/// Solve for the normalised image of the known `shape` from the points;
/// at least `shape.unknowns() + 1` are needed and the points beyond the
/// unknowns are checks. `None` when the system is singular, a check fails
/// or the image does not keep `shape` (unlucky points or prime, or a
/// shape that does not hold in this prime).
pub fn fit_known_shape(
    field: &Zp64,
    shape: &Shape,
    xs: &[Zp],
    ys: &[Zp],
) -> Option<(Vec<Zp>, Vec<Zp>)> {
    if xs.len() != ys.len() {
        return None;
    }
    if shape.zero {
        return (!ys.is_empty() && ys.iter().all(|y| field.is_zero(y)))
            .then(|| (vec![field.zero()], vec![field.one()]));
    }
    let unknowns = shape.unknowns();
    if xs.len() < unknowns + 1 {
        return None;
    }
    let (n, m, low) = (shape.numerator, shape.denominator, shape.low);
    let top = n.max(m);
    let mut matrix: Vec<Vec<Zp>> = Vec::with_capacity(unknowns);
    for (x, y) in xs.iter().zip(ys).take(unknowns) {
        let mut powers = Vec::with_capacity(top + 1);
        let mut power = field.one();
        for _ in 0..=top {
            powers.push(power);
            power = field.mul(&power, x);
        }
        let mut row = Vec::with_capacity(unknowns + 1);
        row.extend_from_slice(&powers[..=n]);
        for power in &powers[low + 1..=m] {
            row.push(field.neg(&field.mul(y, power)));
        }
        row.push(field.mul(y, &powers[low]));
        matrix.push(row);
    }
    // Gauss-Jordan elimination.
    for column in 0..unknowns {
        let pivot = (column..unknowns).find(|&row| !field.is_zero(&matrix[row][column]))?;
        matrix.swap(column, pivot);
        let inverse = field.inv(&matrix[column][column]);
        for value in matrix[column].iter_mut() {
            field.mul_assign(value, &inverse);
        }
        let pivot_row = matrix[column].clone();
        for (row, values) in matrix.iter_mut().enumerate() {
            if row == column || field.is_zero(&values[column]) {
                continue;
            }
            let factor = values[column];
            for (value, pivot_value) in values.iter_mut().zip(&pivot_row) {
                field.sub_mul_assign(value, &factor, pivot_value);
            }
        }
    }
    let solution: Vec<Zp> = matrix.iter().map(|row| row[unknowns]).collect();
    let mut numerator = solution[..=n].to_vec();
    let mut denominator = vec![field.zero(); m + 1];
    denominator[low] = field.one();
    denominator[low + 1..].copy_from_slice(&solution[n + 1..]);
    for (x, y) in xs.iter().zip(ys).skip(unknowns) {
        let d = horner(field, &denominator, x);
        if field.is_zero(&d) || field.mul(y, &d) != horner(field, &numerator, x) {
            return None;
        }
    }
    trim(field, &mut numerator);
    trim(field, &mut denominator);
    let (numerator, denominator) = normalise(field, &numerator, &denominator)?;
    (Shape::of(field, &numerator, &denominator) == *shape).then_some((numerator, denominator))
}

#[cfg(test)]
mod tests {
    use super::*;
    use symbolica::domains::finite_field::FiniteFieldCore;

    #[test]
    fn known_shape_fit_needs_unknowns_plus_one_points_and_checks_them() {
        let field = Zp64::new(4611686018427387847);
        let e = |v: u64| field.to_element(v);
        // f = (2 + x) / (x (1 + 3x)): n = 1, m = 2, low = 1, 3 unknowns.
        let f = |x: &Zp| {
            let n = field.add(&e(2), x);
            let d = field.mul(x, &field.add(&e(1), &field.mul(&e(3), x)));
            field.div(&n, &d)
        };
        let shape = Shape {
            numerator: 1,
            denominator: 2,
            low: 1,
            zero: false,
        };
        assert_eq!(shape.unknowns(), 3);
        let xs: Vec<Zp> = (0..6).map(|i| e(17 + 5 * i)).collect();
        let ys: Vec<Zp> = xs.iter().map(&f).collect();
        let (n, d) = fit_known_shape(&field, &shape, &xs[..4], &ys[..4]).unwrap();
        assert_eq!(n, vec![e(2), e(1)]);
        assert_eq!(d, vec![e(0), e(1), e(3)]);
        let mut bad = ys.clone();
        bad[3] = field.add(&bad[3], &e(1));
        assert!(fit_known_shape(&field, &shape, &xs[..4], &bad[..4]).is_none());
        assert!(fit_known_shape(&field, &shape, &xs[..3], &ys[..3]).is_none());
        // Extra points are checks too.
        assert!(fit_known_shape(&field, &shape, &xs, &ys).is_some());
    }
}
