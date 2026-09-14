//! Exact affine equality cases with integral, unit-pivot charts.
//!
//! This is a geometry primitive, not affine support in the search engine.
//! Symbolica performs all elimination, normalization and substitutions. The
//! admitted chart parametrizes the ambient integer affine lattice exactly;
//! its free coordinates still obey the original sector inequalities. General
//! integer-polyhedron feasibility and congruence charts are not implemented.

use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::prelude::{Integer, Matrix, Q, Rational};

use crate::algebra::CoefficientPolynomial;

use super::super::{GeometryError, Integral, Power, geometry};
use super::CoordinateCase;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AffineGeometryError {
    Coordinate(GeometryError),
    /// The equations have not been replaced by sampled or rectangular cases.
    UnsupportedNonlinear {
        equations: Vec<CoefficientPolynomial>,
    },
    /// The canonical rational chart is not integral. This does not assert
    /// either infeasibility or that a different integral chart cannot exist.
    UnsupportedCongruence {
        equations: Vec<CoefficientPolynomial>,
    },
    InvalidInput(&'static str),
    NativeAlgebra,
}

impl fmt::Display for AffineGeometryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Coordinate(error) => error.fmt(f),
            Self::UnsupportedNonlinear { .. } => {
                write!(f, "case retains unsupported nonlinear equalities")
            }
            Self::UnsupportedCongruence { .. } => {
                write!(
                    f,
                    "case requires an unsupported integer-affine congruence chart"
                )
            }
            Self::InvalidInput(detail) => write!(f, "invalid affine case: {detail}"),
            Self::NativeAlgebra => write!(f, "native algebra failed during affine intersection"),
        }
    }
}

impl std::error::Error for AffineGeometryError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AffineIntersection<const N: usize> {
    Empty,
    Coordinate(CoordinateCase<N>),
    Affine(AffineCase<N>),
}

/// A canonical affine equality domain within a separately owned sector.
///
/// `matrix` contains every equality, including the coordinate face, as
/// canonical rational RREF rows `[A | b]`. `equations` contains only coupled
/// equations in the original coefficient variable map, interpreted as zero.
/// Substitution RHSs contain neither fixed nor other pivot coordinates, so
/// they can be applied sequentially without altering simultaneous semantics.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AffineCase<const N: usize> {
    face: CoordinateCase<N>,
    indices: [usize; N],
    matrix: Matrix<Q>,
    equations: Vec<CoefficientPolynomial>,
    substitutions: Vec<(usize, CoefficientPolynomial)>,
}

impl<const N: usize> AffineCase<N> {
    /// Intersect a coordinate face with native index polynomials equal to zero.
    ///
    /// Coordinate-only input uses the existing fast path and allocates no
    /// matrix. Empty means proved empty, never merely unsupported. Rational
    /// consistency alone is not accepted as integer consistency: every row
    /// receives native gcd/divisibility checks, and every admitted canonical
    /// chart must have integral coefficients and constant term.
    pub fn from_coordinate(
        parent: &CoordinateCase<N>,
        equations: &[CoefficientPolynomial],
        indices: &[usize; N],
        sector: &[bool; N],
    ) -> Result<AffineIntersection<N>, AffineGeometryError> {
        // This also performs variable-map/shape/parameter admission checks.
        match geometry::intersect(parent, equations, indices, sector) {
            Ok(Some(face)) => return Ok(AffineIntersection::Coordinate(face)),
            Ok(None) => return Ok(AffineIntersection::Empty),
            Err(GeometryError::UnsupportedGeometry { .. }) => {}
            Err(error) => return Err(AffineGeometryError::Coordinate(error)),
        }
        catch_unwind(AssertUnwindSafe(|| {
            intersect_native(parent, equations, indices, sector)
        }))
        .map_err(|_| AffineGeometryError::NativeAlgebra)?
    }

    pub fn face(&self) -> &CoordinateCase<N> {
        &self.face
    }

    /// Canonical coupled equalities; fixed coordinates are in [`Self::face`].
    pub fn equations(&self) -> &[CoefficientPolynomial] {
        &self.equations
    }

    /// Canonical RREF of all equalities, in integral-axis order, `[A | b]`.
    pub fn canonical_matrix(&self) -> &Matrix<Q> {
        &self.matrix
    }

    /// `(coefficient-variable position, replacement polynomial)` pairs.
    pub fn substitutions(&self) -> &[(usize, CoefficientPolynomial)] {
        &self.substitutions
    }

    /// Restrict a polynomial to this exact chart using native substitution.
    ///
    /// Search integration MUST shift each ordinary source first and only then
    /// specialize. For example `(n0-n1)(n+s)` on `n0=n1` is `s0-s1`, not zero.
    /// This changes coefficients, never the integral columns of an IBP row.
    pub fn specialize(
        &self,
        polynomial: &CoefficientPolynomial,
    ) -> Result<CoefficientPolynomial, AffineGeometryError> {
        if polynomial.variables() != self.equations[0].variables() {
            return Err(AffineGeometryError::InvalidInput(
                "polynomial and affine case use different variable maps",
            ));
        }
        if polynomial
            .coefficients
            .len()
            .checked_mul(polynomial.nvars())
            != Some(polynomial.exponents.len())
        {
            return Err(AffineGeometryError::InvalidInput(
                "native polynomial coefficient and exponent arrays have inconsistent lengths",
            ));
        }
        let mut result = specialize_face(polynomial, &self.face, &self.indices);
        for (position, replacement) in &self.substitutions {
            result = result.replace_with_poly(*position, replacement);
        }
        Ok(result)
    }

    /// Intersect further guard-zero equations after applying the parent chart.
    /// This allows a seemingly nonlinear guard to become linear on the parent.
    pub fn intersect(
        &self,
        equations: &[CoefficientPolynomial],
        sector: &[bool; N],
    ) -> Result<AffineIntersection<N>, AffineGeometryError> {
        let mut combined = self.equations.clone();
        for equation in equations {
            combined.push(self.specialize(equation)?);
        }
        Self::from_coordinate(&self.face, &combined, &self.indices, sector)
    }

    /// Whether a displacement preserves every equality, including fixed axes.
    /// Source seeds are NOT restricted by this test; recentered targets are.
    pub fn is_tangent(&self, displacement: &[i16; N]) -> bool {
        self.matrix.row_iter().all(|row| {
            row[..N]
                .iter()
                .zip(displacement)
                .fold(Integer::zero(), |sum, (coefficient, shift)| {
                    sum + coefficient.numerator_ref() * Integer::from(*shift)
                })
                .is_zero()
        })
    }

    /// Fixed-coordinate matching followed by the homogeneous `A * shift = 0`
    /// test used by the reference's `matchWithShift`.
    pub fn matches(&self, integral: &Integral<N>) -> bool {
        self.face.matches(integral)
            && self.is_tangent(&std::array::from_fn(|axis| {
                if integral.powers()[axis].is_symbolic() {
                    integral.powers()[axis].value()
                } else {
                    0
                }
            }))
    }

    /// Exact equality-domain containment; sectors must be compared by callers.
    /// The second case implies this case iff every required equality vanishes
    /// after its native chart substitution. No sampled points are involved.
    pub fn contains_affine(&self, other: &Self) -> Result<bool, AffineGeometryError> {
        self.compatible(other)?;
        if !geometry::contains(&self.face, &other.face) {
            return Ok(false);
        }
        for equation in &self.equations {
            if !other.specialize(equation)?.is_zero() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub fn contains_coordinate(&self, other: &CoordinateCase<N>) -> bool {
        geometry::contains(&self.face, other)
            && self
                .equations
                .iter()
                .all(|equation| specialize_face(equation, other, &self.indices).is_zero())
    }

    pub fn is_contained_in_coordinate(&self, container: &CoordinateCase<N>) -> bool {
        geometry::contains(container, &self.face)
    }

    fn compatible(&self, other: &Self) -> Result<(), AffineGeometryError> {
        if self.indices != other.indices
            || self.equations[0].variables() != other.equations[0].variables()
        {
            return Err(AffineGeometryError::InvalidInput(
                "affine cases use different index-variable maps",
            ));
        }
        Ok(())
    }
}

fn specialize_face<const N: usize>(
    polynomial: &CoefficientPolynomial,
    face: &CoordinateCase<N>,
    indices: &[usize; N],
) -> CoefficientPolynomial {
    let mut result = polynomial.clone();
    for (axis, value) in face.fixed().iter().enumerate() {
        if let Some(value) = value {
            result = result.replace(indices[axis], &Integer::from(*value));
        }
    }
    result
}

fn intersect_native<const N: usize>(
    parent: &CoordinateCase<N>,
    equations: &[CoefficientPolynomial],
    indices: &[usize; N],
    sector: &[bool; N],
) -> Result<AffineIntersection<N>, AffineGeometryError> {
    let Some(template) = equations.first() else {
        return Ok(AffineIntersection::Coordinate(*parent));
    };
    let columns = N.checked_add(1).and_then(|n| u32::try_from(n).ok()).ok_or(
        AffineGeometryError::InvalidInput("affine matrix dimensions exceed native limits"),
    )?;
    let mut rows: Vec<Vec<Rational>> = Vec::new();
    for (axis, value) in parent.fixed().iter().enumerate() {
        if let Some(value) = value {
            let mut row = vec![Rational::zero(); N + 1];
            row[axis] = Rational::one();
            row[N] = Rational::from(*value);
            rows.push(row);
        }
    }
    for source in equations {
        let equation = specialize_face(source, parent, indices);
        if equation.is_zero() {
            continue;
        }
        if equation.is_constant() {
            return Ok(AffineIntersection::Empty);
        }
        if (0..equation.nterms()).any(|term| {
            equation
                .exponents(term)
                .iter()
                .map(|&e| usize::from(e))
                .sum::<usize>()
                > 1
        }) {
            return Err(AffineGeometryError::UnsupportedNonlinear {
                equations: equations.to_vec(),
            });
        }
        let mut row = vec![Integer::zero(); N + 1];
        let mut exponents = vec![0; equation.nvars()];
        for (axis, &position) in indices.iter().enumerate() {
            exponents[position] = 1;
            row[axis] = equation
                .coefficient(&exponents)
                .unwrap_or_else(Integer::zero);
            exponents[position] = 0;
        }
        row[N] = -equation.get_constant();
        if !integer_row_possible(&row) {
            return Ok(AffineIntersection::Empty);
        }
        rows.push(row.into_iter().map(Rational::from).collect());
    }
    // Numerica's reducer requires at least one row and one coefficient column.
    if rows.is_empty() {
        return Ok(AffineIntersection::Coordinate(*parent));
    }
    if N == 0 {
        return Err(AffineGeometryError::InvalidInput(
            "nonconstant zero-dimensional case",
        ));
    }
    let row_count = u32::try_from(rows.len()).map_err(|_| {
        AffineGeometryError::InvalidInput("affine matrix dimensions exceed native limits")
    })?;
    row_count
        .checked_mul(columns)
        .ok_or(AffineGeometryError::InvalidInput(
            "affine matrix dimensions exceed native limits",
        ))?;
    let mut matrix =
        Matrix::from_linear(rows.into_iter().flatten().collect(), row_count, columns, Q)
            .map_err(|_| AffineGeometryError::NativeAlgebra)?;
    let rank = matrix.row_reduce(N as u32);
    if matrix.row_iter().skip(rank).any(|row| !row[N].is_zero()) {
        return Ok(AffineIntersection::Empty);
    }
    // Check integer divisibility again on equations exposed by elimination.
    // Native rational primitive_part clears all denominators exactly.
    for row in matrix.row_iter().take(rank) {
        let primitive = Matrix::new_vec(row.to_vec(), Q).primitive_part();
        let integers: Vec<_> = primitive.iter().map(|entry| entry.numerator()).collect();
        if primitive.iter().any(|entry| !entry.is_integer()) {
            return Err(AffineGeometryError::NativeAlgebra);
        }
        if !integer_row_possible(&integers) {
            return Ok(AffineIntersection::Empty);
        }
    }
    if matrix
        .row_iter()
        .take(rank)
        .flatten()
        .any(|entry| !entry.is_integer())
    {
        return Err(AffineGeometryError::UnsupportedCongruence {
            equations: equations.to_vec(),
        });
    }
    let matrix = Matrix::from_linear(
        matrix.row_iter().take(rank).flatten().cloned().collect(),
        rank as u32,
        columns,
        Q,
    )
    .map_err(|_| AffineGeometryError::NativeAlgebra)?;
    let mut fixed = [None; N];
    let mut coupled = Vec::new();
    let mut substitutions = Vec::new();
    for row in matrix.row_iter() {
        let pivot = row[..N]
            .iter()
            .position(|entry| !entry.is_zero())
            .ok_or(AffineGeometryError::NativeAlgebra)?;
        let value = row[N].numerator();
        if row[..N].iter().filter(|entry| !entry.is_zero()).count() == 1 {
            if (!value.is_negative() && !value.is_zero()) != sector[pivot] {
                return Ok(AffineIntersection::Empty);
            }
            let compact = value
                .to_i64()
                .and_then(|value| i16::try_from(value).ok())
                .filter(|&value| Power::new(false, value).is_ok())
                .ok_or_else(|| {
                    AffineGeometryError::Coordinate(GeometryError::CompactOverflow {
                        axis: pivot,
                        value: value.clone(),
                    })
                })?;
            fixed[pivot] = Some(compact);
            continue;
        }
        let mut replacement = template.constant(value);
        let mut equation = template.constant(-row[N].numerator());
        let mut exponents = vec![0; template.nvars()];
        for (axis, coefficient) in row[..N].iter().enumerate() {
            if coefficient.is_zero() {
                continue;
            }
            exponents[indices[axis]] = 1;
            equation.append_monomial(coefficient.numerator(), &exponents);
            if axis != pivot {
                replacement.append_monomial(-coefficient.numerator(), &exponents);
            }
            exponents[indices[axis]] = 0;
        }
        coupled.push(equation);
        substitutions.push((indices[pivot], replacement));
    }
    let face = CoordinateCase::new(fixed).map_err(|_| AffineGeometryError::NativeAlgebra)?;
    if coupled.is_empty() {
        return Ok(AffineIntersection::Coordinate(face));
    }
    Ok(AffineIntersection::Affine(AffineCase {
        face,
        indices: *indices,
        matrix,
        equations: coupled,
        substitutions,
    }))
}

/// Necessary integer consistency of one exact integer equation `[A | b]`.
/// This is not used as a sufficient test for a system of several equations.
fn integer_row_possible(row: &[Integer]) -> bool {
    let (rhs, coefficients) = row.split_last().expect("augmented row has a constant");
    let divisor = coefficients
        .iter()
        .fold(Integer::zero(), |gcd, value| gcd.gcd(value));
    if divisor.is_zero() {
        rhs.is_zero()
    } else {
        rhs.quot_rem(&divisor).1.is_zero()
    }
}

#[cfg(test)]
#[path = "affine/tests.rs"]
mod tests;
