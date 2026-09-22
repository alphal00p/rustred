//! Exact integer-coordinate equality cases with rational computational charts.
//!
//! The sector search shares these charts and restricts source coefficients
//! after translation, without identifying distinct integral coordinates.
//! Symbolica performs all elimination, normalization and substitutions. The
//! case remains the original integer coordinates, exact equalities and sector
//! signs. A rational chart is a coefficient-restriction device, not a claim
//! that arbitrary integer free coordinates give integer dependent coordinates.
//! General integer-lattice and sector-inequality feasibility is not decided.

use std::borrow::Cow;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::prelude::{Integer, IntegerRing, Matrix, Q};

use crate::algebra::{Coefficient, CoefficientPolynomial};

use super::super::{GeometryError, Integral, Power, geometry};
use super::CoordinateCase;

#[path = "affine/chart.rs"]
mod chart;
use chart::Chart;
pub(crate) use chart::{Chart as AffineRestrictionChart, canonical_equalities};

#[path = "affine/bounds.rs"]
mod bounds;

#[path = "affine/one_parameter.rs"]
mod one_parameter;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AffineGeometryError {
    Coordinate(GeometryError),
    /// The equations have not been replaced by sampled or rectangular cases.
    UnsupportedNonlinear {
        equations: Vec<CoefficientPolynomial>,
    },
    /// An exact coefficient denominator vanishes identically on the case.
    UndefinedCoefficient,
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
            Self::UndefinedCoefficient => write!(f, "coefficient is undefined on its affine case"),
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
/// primitive integer equations in the original coefficient variable map,
/// interpreted as zero. `primitive_matrix` caches those whole-row integer
/// normalizations, including fixed rows, for exact tangency and queue order.
/// Substitution RHSs contain neither fixed nor other pivot coordinates, so
/// they can be applied sequentially without altering simultaneous semantics.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AffineCase<const N: usize> {
    face: CoordinateCase<N>,
    indices: [usize; N],
    matrix: Matrix<Q>,
    primitive_matrix: Matrix<IntegerRing>,
    equations: Vec<CoefficientPolynomial>,
    chart: Chart,
}

impl<const N: usize> AffineCase<N> {
    /// Intersect a coordinate face with native index polynomials equal to zero.
    ///
    /// Coordinate-only input uses the existing fast path and allocates no
    /// matrix. Empty means proved empty, never merely unsupported. Rational
    /// consistency is not asserted to prove integer feasibility: every row
    /// receives native gcd/divisibility checks, but unresolved congruences
    /// remain implicit in the original exact integer-coordinate equalities.
    /// This preserves the declared equality chart, including its symbolic
    /// target, for exact saved-rule reconstruction. Sector-implied endpoint
    /// refinements belong to [`super::Case::intersect`], not chart import.
    pub fn from_coordinate(
        parent: &CoordinateCase<N>,
        equations: &[CoefficientPolynomial],
        indices: &[usize; N],
        sector: &[bool; N],
    ) -> Result<AffineIntersection<N>, AffineGeometryError> {
        Self::from_coordinate_with_sector_refinement(
            parent, equations, indices, sector, None, false,
        )
    }

    pub(crate) fn from_coordinate_in_rank(
        parent: &CoordinateCase<N>,
        equations: &[CoefficientPolynomial],
        indices: &[usize; N],
        sector: &[bool; N],
        max_numerator_rank: Option<u32>,
    ) -> Result<AffineIntersection<N>, AffineGeometryError> {
        Self::from_coordinate_with_sector_refinement(
            parent,
            equations,
            indices,
            sector,
            max_numerator_rank,
            true,
        )
    }

    fn from_coordinate_with_sector_refinement(
        parent: &CoordinateCase<N>,
        equations: &[CoefficientPolynomial],
        indices: &[usize; N],
        sector: &[bool; N],
        max_numerator_rank: Option<u32>,
        refine_sector_endpoints: bool,
    ) -> Result<AffineIntersection<N>, AffineGeometryError> {
        // This also performs variable-map/shape/parameter admission checks.
        let coordinate = match max_numerator_rank {
            None => geometry::intersect(parent, equations, indices, sector),
            Some(_) => {
                geometry::intersect_in_rank(parent, equations, indices, sector, max_numerator_rank)
            }
        };
        match coordinate {
            Ok(Some(face)) => return Ok(AffineIntersection::Coordinate(face)),
            Ok(None) => return Ok(AffineIntersection::Empty),
            Err(GeometryError::UnsupportedGeometry { .. }) => {}
            Err(error) => return Err(AffineGeometryError::Coordinate(error)),
        }
        catch_unwind(AssertUnwindSafe(|| {
            intersect_native(
                parent,
                equations,
                indices,
                sector,
                max_numerator_rank,
                refine_sector_endpoints,
            )
        }))
        .map_err(|_| AffineGeometryError::NativeAlgebra)?
    }

    pub fn face(&self) -> &CoordinateCase<N> {
        &self.face
    }

    pub fn index_variables(&self) -> &[usize; N] {
        &self.indices
    }

    /// Canonical coupled equalities; fixed coordinates are in [`Self::face`].
    pub fn equations(&self) -> &[CoefficientPolynomial] {
        &self.equations
    }

    /// Canonical RREF of all equalities, in integral-axis order, `[A | b]`.
    pub fn canonical_matrix(&self) -> &Matrix<Q> {
        &self.matrix
    }

    /// Primitive integer canonical rows `[A | b]`, with positive leading entry.
    pub fn primitive_matrix(&self) -> &Matrix<IntegerRing> {
        &self.primitive_matrix
    }

    /// Whether coefficient restriction can use only integer polynomials.
    /// This does not assert feasibility of the case's sector inequalities.
    pub fn has_integral_chart(&self) -> bool {
        self.chart.is_integral()
    }

    pub(crate) fn restriction_term_bound(
        &self,
        polynomial: &CoefficientPolynomial,
    ) -> Result<(usize, usize), AffineGeometryError> {
        self.validate_polynomial(polynomial)?;
        self.chart
            .restriction_term_bound(polynomial, self.face.fixed(), &self.indices)
    }

    /// Retained native payload census, without cloning or native arithmetic.
    /// Shared variable maps, allocator overhead and hidden Matrix spare capacity
    /// are excluded. Matrix occupied elements and all exposed limb/polynomial
    /// capacities are counted. This is not a transient allocation/RSS bound.
    pub(crate) fn native_payload_bytes(&self) -> Option<usize> {
        use crate::algebra::{
            integer_clone_owned_heap_byte_bound as integer_bytes,
            polynomial_clone_owned_heap_byte_bound as polynomial_bytes,
        };
        use symbolica::prelude::Rational;
        let mut bytes = size_of::<Self>().checked_add(
            self.equations
                .capacity()
                .checked_mul(size_of::<CoefficientPolynomial>())?,
        )?;
        for equation in &self.equations {
            bytes = bytes.checked_add(polynomial_bytes(equation)?)?;
        }
        bytes = bytes.checked_add(
            self.matrix
                .iter()
                .len()
                .checked_mul(size_of::<Rational>())?,
        )?;
        for value in self.matrix.iter() {
            bytes = bytes
                .checked_add(integer_bytes(value.numerator_ref())?)?
                .checked_add(integer_bytes(value.denominator_ref())?)?;
        }
        bytes = bytes.checked_add(
            self.primitive_matrix
                .iter()
                .len()
                .checked_mul(size_of::<Integer>())?,
        )?;
        for value in self.primitive_matrix.iter() {
            bytes = bytes.checked_add(integer_bytes(value)?)?;
        }
        bytes.checked_add(self.chart.native_payload_bytes()?)
    }

    /// A necessary row-bound test, not a complete integer feasibility solver.
    /// Retained cases may still be empty for joint inequality/congruence reasons.
    /// Prove that this exact affine locus has no point in the declared
    /// sector using the primitive integer rows and coordinate bounds.
    ///
    /// This is deliberately only an emptiness fast path.  A `false` result
    /// says nothing about feasibility (joint congruences and inequalities may
    /// still be unresolved), and must never be used as a coverage claim.
    pub(crate) fn is_proved_empty_in_sector(&self, sector: &[bool; N]) -> bool {
        !self.face.is_in_sector(sector)
            || self
                .primitive_matrix
                .row_iter()
                .any(|row| bounds::excludes_rhs(row, &self.face, sector))
    }

    /// A stored affine case may acquire integer endpoint equalities in this
    /// sector. Such a case must be recanonicalized even without new guards.
    pub(crate) fn has_sector_endpoint_refinement(&self, sector: &[bool; N]) -> bool {
        self.primitive_matrix.row_iter().any(|row| {
            matches!(
                bounds::classify(row, &self.face, sector),
                bounds::RowBounds::Endpoints(_)
            )
        })
    }

    /// Search-only strengthening; saved declared charts remain immutable.
    pub(crate) fn has_sector_refinement(&self, sector: &[bool; N]) -> bool {
        self.has_sector_endpoint_refinement(sector)
            || !matches!(
                one_parameter::classify(&self.primitive_matrix, sector),
                one_parameter::Refinement::Unresolved
            )
    }

    /// Restrict an equation interpreted as zero. Nonzero rational scalar
    /// content may be removed; this must not be used for coefficient values.
    pub fn restrict_equation(
        &self,
        polynomial: &CoefficientPolynomial,
    ) -> Result<CoefficientPolynomial, AffineGeometryError> {
        self.validate_polynomial(polynomial)?;
        Ok(self
            .chart
            .restrict_equation(polynomial, &self.face, &self.indices))
    }

    /// Restrict a polynomial value exactly, including every rational factor.
    ///
    /// Search integration MUST shift each ordinary source first and only then
    /// specialize. For example `(n0-n1)(n+s)` on `n0=n1` is `s0-s1`, not zero.
    /// This changes coefficients, never the integral columns of an IBP row.
    pub fn restrict_polynomial_value(
        &self,
        polynomial: &CoefficientPolynomial,
    ) -> Result<Coefficient, AffineGeometryError> {
        self.validate_polynomial(polynomial)?;
        Ok(self.restrict_polynomial_value_validated(polynomial))
    }

    /// Restrict numerator and denominator jointly, preserving their relative
    /// scale. An identically zero restricted denominator is an explicit error.
    pub fn restrict_coefficient(
        &self,
        coefficient: &Coefficient,
    ) -> Result<Coefficient, AffineGeometryError> {
        self.validate_polynomial(&coefficient.numerator)?;
        self.validate_polynomial(&coefficient.denominator)?;
        self.chart
            .restrict_coefficient(coefficient, &self.face, &self.indices)
    }

    fn validate_polynomial(
        &self,
        polynomial: &CoefficientPolynomial,
    ) -> Result<(), AffineGeometryError> {
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
        Ok(())
    }

    /// Internal seeded-row path: the solver checks the immutable source and
    /// chart maps once at case entry; no repeated boundary validation per term.
    pub(crate) fn restrict_polynomial_value_validated(
        &self,
        polynomial: &CoefficientPolynomial,
    ) -> Coefficient {
        self.chart
            .restrict_polynomial_value(polynomial, &self.face, &self.indices)
    }

    /// Intersect further guard-zero equations after applying the parent chart.
    /// This allows a seemingly nonlinear guard to become linear on the parent.
    pub fn intersect(
        &self,
        equations: &[CoefficientPolynomial],
        sector: &[bool; N],
    ) -> Result<AffineIntersection<N>, AffineGeometryError> {
        self.intersect_in_rank(equations, sector, None)
    }

    pub(crate) fn intersect_in_rank(
        &self,
        equations: &[CoefficientPolynomial],
        sector: &[bool; N],
        max_numerator_rank: Option<u32>,
    ) -> Result<AffineIntersection<N>, AffineGeometryError> {
        let mut combined = self.equations.clone();
        for equation in equations {
            combined.push(self.restrict_equation(equation)?);
        }
        Self::from_coordinate_in_rank(
            &self.face,
            &combined,
            &self.indices,
            sector,
            max_numerator_rank,
        )
    }

    /// Whether a displacement preserves every equality, including fixed axes.
    /// Source seeds are NOT restricted by this test; recentered targets are.
    pub fn is_tangent(&self, displacement: &[i16; N]) -> bool {
        self.primitive_matrix.row_iter().all(|row| {
            row[..N]
                .iter()
                .zip(displacement)
                .fold(Integer::zero(), |sum, (coefficient, shift)| {
                    sum + coefficient * Integer::from(*shift)
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

    /// Prove containment by exact rational-affine equality implication;
    /// sectors must be compared by callers. This is sufficient but conservative
    /// for integer/sector domains when their feasibility remains unresolved.
    /// No sampled points or assumed free-integer chart parameters are involved.
    pub fn contains_affine(&self, other: &Self) -> Result<bool, AffineGeometryError> {
        self.compatible(other)?;
        if !geometry::contains(&self.face, &other.face) {
            return Ok(false);
        }
        for equation in &self.equations {
            if !other.restrict_equation(equation)?.is_zero() {
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
    max_numerator_rank: Option<u32>,
    refine_sector_endpoints: bool,
) -> Result<AffineIntersection<N>, AffineGeometryError> {
    let Some(template) = equations.first() else {
        return Ok(AffineIntersection::Coordinate(*parent));
    };
    let mut refined = *parent;
    let mut conjunction = Cow::Borrowed(equations);
    let (matrix, primitive_matrix) = loop {
        let Some((matrix, primitive_matrix)) =
            canonical_equalities(refined.fixed(), &conjunction, indices)?
        else {
            return Ok(AffineIntersection::Empty);
        };
        let mut fixed = *refined.fixed();
        for row in primitive_matrix.row_iter() {
            match bounds::classify(row, &refined, sector) {
                bounds::RowBounds::Excluded => return Ok(AffineIntersection::Empty),
                bounds::RowBounds::Endpoints(endpoints) if refine_sector_endpoints => {
                    for (axis, &forced) in endpoints.iter().enumerate() {
                        if fixed[axis].is_none() && forced {
                            fixed[axis] = Some(i16::from(sector[axis]));
                        }
                    }
                }
                bounds::RowBounds::Unresolved | bounds::RowBounds::Endpoints(_) => {}
            }
        }
        if &fixed != refined.fixed() {
            // Each endpoint round fixes at least one previously free ORIGINAL
            // coordinate. Retain the whole conjunction on every elimination.
            refined = CoordinateCase::new(fixed).map_err(|_| AffineGeometryError::NativeAlgebra)?;
            continue;
        }
        if refine_sector_endpoints {
            match one_parameter::classify(&primitive_matrix, sector) {
                one_parameter::Refinement::Empty => return Ok(AffineIntersection::Empty),
                one_parameter::Refinement::Fix { axis, value } => {
                    // Do not compact-convert a possibly huge integer yet.
                    // The full native conjunction still owns divisibility,
                    // consistency and rank-before-compact admission. Adding
                    // this equality removes the sole free original axis, so
                    // at most one such extra canonicalization can occur.
                    let mut equality = template.constant(-value);
                    let mut exponents = vec![0; template.nvars()];
                    exponents[indices[axis]] = 1;
                    equality.append_monomial(Integer::one(), &exponents);
                    conjunction.to_mut().push(equality);
                    continue;
                }
                one_parameter::Refinement::Unresolved => {}
            }
        }
        break (matrix, primitive_matrix);
    };
    // Native RREF includes the parent fixed rows and has distinct original
    // coordinate pivots. Only singleton rows fix an original index; coupled
    // chart RHSs are not coordinate values. Check all such values before
    // converting ANY row, so an unrelated positive overflow cannot obscure
    // an exact proof that this entire AND branch lies outside the rank scope.
    if geometry::fixed_rank_exceeds(
        matrix.row_iter().filter_map(|row| {
            (row[..N].iter().filter(|entry| !entry.is_zero()).count() == 1 && row[N].is_integer())
                .then(|| row[N].numerator_ref())
        }),
        max_numerator_rank,
    ) {
        return Ok(AffineIntersection::Empty);
    }
    let mut fixed = [None; N];
    let mut coupled = Vec::new();
    for (row, primitive) in matrix.row_iter().zip(primitive_matrix.row_iter()) {
        let pivot = row[..N]
            .iter()
            .position(|entry| !entry.is_zero())
            .ok_or(AffineGeometryError::NativeAlgebra)?;
        if row[..N].iter().filter(|entry| !entry.is_zero()).count() == 1 {
            // Divisibility above proves that this unit-pivot RHS is integral.
            if !row[N].is_integer() {
                return Err(AffineGeometryError::NativeAlgebra);
            }
            let value = row[N].numerator();
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
        let mut equation = template.constant(-&primitive[N]);
        let mut exponents = vec![0; template.nvars()];
        for (axis, coefficient) in primitive[..N].iter().enumerate() {
            if coefficient.is_zero() {
                continue;
            }
            exponents[indices[axis]] = 1;
            equation.append_monomial(coefficient.clone(), &exponents);
            exponents[indices[axis]] = 0;
        }
        coupled.push(equation);
    }
    let face = CoordinateCase::new(fixed).map_err(|_| AffineGeometryError::NativeAlgebra)?;
    if coupled.is_empty() {
        return Ok(AffineIntersection::Coordinate(face));
    }
    let chart = Chart::new(template, &matrix, indices);
    Ok(AffineIntersection::Affine(AffineCase {
        face,
        indices: *indices,
        matrix,
        primitive_matrix,
        equations: coupled,
        chart,
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

#[cfg(test)]
#[path = "affine/rational_audit.rs"]
mod rational_audit;

#[cfg(test)]
#[path = "affine/saturation_tests.rs"]
mod saturation_tests;

#[cfg(test)]
#[path = "affine/positive_slack_tests.rs"]
mod positive_slack_tests;

#[cfg(test)]
#[path = "affine/one_parameter_tests.rs"]
mod one_parameter_tests;
