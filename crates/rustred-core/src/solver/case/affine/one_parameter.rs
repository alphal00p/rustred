//! Necessary sector bounds on one free ORIGINAL integer coordinate.
//!
//! Native RREF owns all algebra. This only intersects its one-dimensional
//! integer half-lines and proposes an equality for native recanonicalization.
//! Multiple points and rays are deliberately left unresolved.

use symbolica::prelude::{Integer, IntegerRing, Matrix};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Refinement {
    Unresolved,
    Empty,
    Fix { axis: usize, value: Integer },
}

/// The matrix is the existing primitive normalization of native rational RREF,
/// including fixed-parent rows. Never treat an invented chart parameter as an
/// integer, or ignore a second free original coordinate.
pub(super) fn classify<const N: usize>(
    matrix: &Matrix<IntegerRing>,
    sector: &[bool; N],
) -> Refinement {
    if N == 0 || matrix.ncols() != N + 1 || matrix.nrows() != N - 1 {
        return Refinement::Unresolved;
    }
    let mut pivots = [false; N];
    for row in matrix.row_iter() {
        let Some(pivot) = row[..N].iter().position(|value| !value.is_zero()) else {
            return Refinement::Unresolved;
        };
        if pivots[pivot] || row[pivot].is_negative() {
            return Refinement::Unresolved;
        }
        pivots[pivot] = true;
    }
    let axis = pivots
        .iter()
        .position(|&pivot| !pivot)
        .expect("N-1 distinct pivots leave one original coordinate");
    // This follows from native RREF, but decline rather than infer bounds if
    // the private helper ever receives a different matrix representation.
    for row in matrix.row_iter() {
        let pivot = row[..N].iter().position(|value| !value.is_zero()).unwrap();
        if row[..N]
            .iter()
            .enumerate()
            .any(|(column, value)| column != pivot && column != axis && !value.is_zero())
        {
            return Refinement::Unresolved;
        }
    }
    let mut bounds = if sector[axis] {
        Bounds {
            lower: Some(Integer::one()),
            upper: None,
        }
    } else {
        Bounds {
            lower: None,
            upper: Some(Integer::zero()),
        }
    };
    for row in matrix.row_iter() {
        let pivot = row[..N].iter().position(|value| !value.is_zero()).unwrap();
        // a*n_pivot + c*t = b, a>0. Positive indices are >=1, not merely >0
        // over the rationals; inactive indices are <=0, not strictly negative.
        let rhs = if sector[pivot] {
            &row[N] - &row[pivot]
        } else {
            row[N].clone()
        };
        if !bounds.intersect(&row[axis], &rhs, !sector[pivot]) {
            return Refinement::Empty;
        }
    }
    match (bounds.lower, bounds.upper) {
        (Some(lower), Some(upper)) if lower > upper => Refinement::Empty,
        (Some(lower), Some(upper)) if lower == upper => Refinement::Fix { axis, value: lower },
        _ => Refinement::Unresolved,
    }
}

struct Bounds {
    lower: Option<Integer>,
    upper: Option<Integer>,
}

impl Bounds {
    /// Intersect c*t >= b when `at_least`, or c*t <= b otherwise.
    fn intersect(&mut self, coefficient: &Integer, rhs: &Integer, at_least: bool) -> bool {
        if coefficient.is_zero() {
            return if at_least {
                rhs <= &Integer::zero()
            } else {
                rhs >= &Integer::zero()
            };
        }
        let negative = coefficient.is_negative();
        let (numerator, denominator) = if negative {
            (-rhs, -coefficient)
        } else {
            (rhs.clone(), coefficient.clone())
        };
        let is_lower = at_least != negative;
        // Symbolica's native quot_rem is Euclidean for every integer storage
        // representation. With positive denominator it gives floor, and the
        // nonzero remainder supplies ceiling. Rational::floor/ceil instead
        // round toward/away from zero and cannot be used here for all signs.
        let (mut value, remainder) = numerator.quot_rem(&denominator);
        if is_lower && !remainder.is_zero() {
            value += Integer::one();
        }
        if is_lower {
            if self.lower.as_ref().is_none_or(|current| value > *current) {
                self.lower = Some(value);
            }
        } else if self.upper.as_ref().is_none_or(|current| value < *current) {
            self.upper = Some(value);
        }
        !matches!((&self.lower, &self.upper), (Some(lower), Some(upper)) if lower > upper)
    }
}
