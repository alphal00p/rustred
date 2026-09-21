//! Cheaply shared equality domains for symbolic sector search.

use std::cmp::Ordering;
use std::sync::Arc;

use symbolica::prelude::Integer;

use crate::algebra::CoefficientPolynomial;

use super::super::{Integral, geometry};
use super::{AffineCase, AffineGeometryError, AffineIntersection, CoordinateCase};

/// An exact equality case in original integral coordinates.
///
/// The common coordinate representation stays inline. Affine charts and their
/// native polynomials are immutable and shared, so moving cases through the
/// queue never copies expressions. A separately owned sector supplies index
/// signs; this type does not assert general integer-polyhedron feasibility.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Case<const N: usize> {
    Coordinate(CoordinateCase<N>),
    Affine(Arc<AffineCase<N>>),
}

impl<const N: usize> From<CoordinateCase<N>> for Case<N> {
    fn from(case: CoordinateCase<N>) -> Self {
        Self::Coordinate(case)
    }
}

impl<const N: usize> From<AffineCase<N>> for Case<N> {
    fn from(case: AffineCase<N>) -> Self {
        Self::Affine(Arc::new(case))
    }
}

impl<const N: usize> From<Arc<AffineCase<N>>> for Case<N> {
    fn from(case: Arc<AffineCase<N>>) -> Self {
        Self::Affine(case)
    }
}

impl<const N: usize> Case<N> {
    pub const fn generic() -> Self {
        Self::Coordinate(CoordinateCase::generic())
    }

    pub fn face(&self) -> &CoordinateCase<N> {
        match self {
            Self::Coordinate(case) => case,
            Self::Affine(case) => case.face(),
        }
    }

    pub fn fixed(&self) -> &[Option<i16>; N] {
        self.face().fixed()
    }

    pub fn coordinate(&self) -> Option<&CoordinateCase<N>> {
        match self {
            Self::Coordinate(case) => Some(case),
            Self::Affine(_) => None,
        }
    }

    pub fn affine(&self) -> Option<&AffineCase<N>> {
        match self {
            Self::Coordinate(_) => None,
            Self::Affine(case) => Some(case),
        }
    }

    /// Coupled coordinates remain separate symbolic integral-key axes.
    pub fn integral(&self) -> Integral<N> {
        self.face().integral()
    }

    pub fn is_numerical(&self) -> bool {
        self.face().is_numerical()
    }

    /// Check only fixed-coordinate signs, not coupled inequality feasibility.
    pub fn is_in_sector(&self, sector: &[bool; N]) -> bool {
        self.face().is_in_sector(sector)
    }

    /// Test target shifts tangent to the case. Ordinary source translations
    /// must not be filtered with this predicate.
    pub fn matches(&self, integral: &Integral<N>) -> bool {
        match self {
            Self::Coordinate(case) => case.matches(integral),
            Self::Affine(case) => case.matches(integral),
        }
    }

    /// Intersect an AND of exact guard equations. `None` means proved empty;
    /// unsupported geometry is retained in a typed error, never approximated.
    pub fn intersect(
        &self,
        equations: &[CoefficientPolynomial],
        indices: &[usize; N],
        sector: &[bool; N],
    ) -> Result<Option<Self>, AffineGeometryError> {
        self.intersect_in_rank(equations, indices, sector, None)
    }

    pub(crate) fn intersect_in_rank(
        &self,
        equations: &[CoefficientPolynomial],
        indices: &[usize; N],
        sector: &[bool; N],
        max_numerator_rank: Option<u32>,
    ) -> Result<Option<Self>, AffineGeometryError> {
        let result = match self {
            Self::Coordinate(case) => AffineCase::from_coordinate_in_rank(
                case,
                equations,
                indices,
                sector,
                max_numerator_rank,
            )?,
            Self::Affine(case) => {
                if case.index_variables() != indices {
                    return Err(AffineGeometryError::InvalidInput(
                        "intersection uses a different index-variable map",
                    ));
                }
                if case.is_proved_empty_in_sector(sector) {
                    return Ok(None);
                }
                if equations.is_empty() && !case.has_saturated_sector_row(sector) {
                    return Ok(Some(self.clone()));
                }
                case.intersect_in_rank(equations, sector, max_numerator_rank)?
            }
        };
        Ok(match result {
            AffineIntersection::Empty => None,
            AffineIntersection::Coordinate(case) => Some(case.into()),
            AffineIntersection::Affine(case) => Some(case.into()),
        })
    }

    /// Prove that `other` implies this case's equalities. Callers own sectors.
    /// Rational-affine implication is sufficient but conservative when an
    /// integer/sector domain is empty for reasons not decided by this service.
    pub fn contains(&self, other: &Self) -> Result<bool, AffineGeometryError> {
        match (self, other) {
            (Self::Coordinate(container), Self::Coordinate(contained)) => {
                Ok(geometry::contains(container, contained))
            }
            (Self::Coordinate(container), Self::Affine(contained)) => {
                Ok(contained.is_contained_in_coordinate(container))
            }
            (Self::Affine(container), Self::Coordinate(contained)) => {
                Ok(container.contains_coordinate(contained))
            }
            (Self::Affine(container), Self::Affine(contained)) => {
                if Arc::ptr_eq(container, contained) {
                    Ok(true)
                } else {
                    container.contains_affine(contained)
                }
            }
        }
    }

    /// Reference `unsolved` ordering within one family and sector: total
    /// constraints, coordinate constraints, leading axes, coupled equations,
    /// then coordinate equation constants. Not a cross-family ordering.
    pub fn queue_cmp(&self, other: &Self) -> Ordering {
        if let (Self::Coordinate(left), Self::Coordinate(right)) = (self, other) {
            return geometry::compare_cases(left, right);
        }
        self.constraint_count()
            .cmp(&other.constraint_count())
            .then_with(|| self.simple_count().cmp(&other.simple_count()))
            .then_with(|| self.leading_axes().cmp(other.leading_axes()))
            .then_with(|| {
                for (left, right) in self.coupled_rows().zip(other.coupled_rows()) {
                    for (a, b) in left[..N].iter().zip(&right[..N]) {
                        let order = reference_coefficient_cmp(a, b);
                        if order != Ordering::Equal {
                            return order;
                        }
                    }
                    // The reference stores A*n+c=0; matrix RHS is b=-c.
                    let constant_order = match (left[N].is_zero(), right[N].is_zero()) {
                        (true, false) => Ordering::Greater,
                        (false, true) => Ordering::Less,
                        _ => right[N].cmp(&left[N]),
                    };
                    if constant_order != Ordering::Equal {
                        return constant_order;
                    }
                }
                Ordering::Equal
            })
            .then_with(|| other.fixed().cmp(self.fixed()))
    }

    fn simple_count(&self) -> usize {
        self.fixed().iter().filter(|value| value.is_some()).count()
    }

    fn constraint_count(&self) -> usize {
        match self {
            Self::Coordinate(_) => self.simple_count(),
            Self::Affine(case) => case.canonical_matrix().nrows(),
        }
    }

    fn leading_axes(&self) -> impl Iterator<Item = usize> {
        let mut leading = self.fixed().map(|value| value.is_some());
        for row in self.coupled_rows() {
            let pivot = row[..N]
                .iter()
                .position(|coefficient| !coefficient.is_zero())
                .expect("admitted coupled equation has a pivot");
            leading[pivot] = true;
        }
        leading
            .into_iter()
            .enumerate()
            .filter_map(|(axis, is_leading)| is_leading.then_some(axis))
    }

    fn coupled_rows(&self) -> impl Iterator<Item = &[Integer]> {
        self.affine()
            .into_iter()
            .flat_map(|case| case.primitive_matrix().row_iter())
            .filter(|row| row[..N].iter().filter(|entry| !entry.is_zero()).count() > 1)
    }
}

fn reference_coefficient_cmp(left: &Integer, right: &Integer) -> Ordering {
    match (left.is_zero(), right.is_zero()) {
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        _ => left.cmp(right),
    }
}

#[cfg(test)]
mod tests;
