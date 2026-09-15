//! Narrow adapters to native exact equation operations, not coefficient values.

use symbolica::domains::InternalOrdering;
use symbolica::poly::groebner::GroebnerBasis;
use symbolica::prelude::{Factorize, Integer, Q, Rational, Z};

use crate::algebra::CoefficientPolynomial;

use super::super::super::{GeometryError, geometry};
use super::super::{AffineGeometryError, Case};
use super::CaseIntersectionFailure;

pub(super) fn preflight<const N: usize>(
    parent: &Case<N>,
    equations: &[CoefficientPolynomial],
    indices: &[usize; N],
) -> Result<(), CaseIntersectionFailure> {
    geometry::preflight(equations, indices).map_err(|error| match error {
        GeometryError::InvalidInput(message) => CaseIntersectionFailure::InvalidInput(message),
        other => CaseIntersectionFailure::Admission(AffineGeometryError::Coordinate(other)),
    })?;
    if let Some(affine) = parent.affine()
        && affine.index_variables() != indices
    {
        return Err(CaseIntersectionFailure::InvalidInput(
            "parent and intersection use different index-variable maps",
        ));
    }
    let Some(first) = equations.first() else {
        return Ok(());
    };
    let variables = first.variables();
    if let Some(affine) = parent.affine()
        && affine.equations()[0].variables() != variables
    {
        return Err(CaseIntersectionFailure::InvalidInput(
            "parent and equations use different coefficient variable maps",
        ));
    }
    Ok(())
}

/// Discarding a nonzero scalar is valid only because these are zero equations.
pub(super) fn primitive(polynomial: CoefficientPolynomial) -> CoefficientPolynomial {
    if polynomial.is_zero() {
        return polynomial;
    }
    positive_leading(polynomial.make_primitive())
}

fn positive_leading(polynomial: CoefficientPolynomial) -> CoefficientPolynomial {
    if !polynomial.is_zero() && polynomial.lcoeff().is_negative() {
        -polynomial
    } else {
        polynomial
    }
}

pub(super) fn canonicalize(equations: &mut Vec<CoefficientPolynomial>) {
    equations.retain(|equation| !equation.is_zero());
    equations.sort_by(InternalOrdering::internal_cmp);
    equations.dedup();
}

pub(super) fn restrict<const N: usize>(
    parent: &Case<N>,
    equations: &[CoefficientPolynomial],
    indices: &[usize; N],
) -> Result<Vec<CoefficientPolynomial>, CaseIntersectionFailure> {
    let mut restricted = Vec::with_capacity(equations.len());
    for source in equations {
        let equation = if let Some(affine) = parent.affine() {
            let restricted = affine
                .restrict_equation(source)
                .map_err(CaseIntersectionFailure::Admission)?;
            if affine.has_integral_chart() {
                primitive(restricted)
            } else {
                // The rational-chart zero-locus API already removed native
                // rational content jointly; no second integer content GCD.
                positive_leading(restricted)
            }
        } else {
            let mut equation = source.clone();
            for (axis, value) in parent.fixed().iter().enumerate() {
                if let Some(value) = value {
                    equation = equation.replace(indices[axis], &Integer::from(*value));
                }
            }
            primitive(equation)
        };
        if !equation.is_zero() {
            restricted.push(equation);
        }
    }
    canonicalize(&mut restricted);
    Ok(restricted)
}

pub(super) fn is_affine(polynomial: &CoefficientPolynomial) -> bool {
    polynomial.exponents_iter().all(|exponents| {
        !exponents.iter().any(|&power| power > 1)
            && exponents
                .iter()
                .filter(|&&power| power != 0)
                .take(2)
                .count()
                <= 1
    })
}

/// Preserve the entire transformed basis, including newly exposed coupled
/// affine equations. Native Q-F4 owns every ideal operation.
pub(super) fn normalize(
    equations: &[CoefficientPolynomial],
) -> Result<Vec<CoefficientPolynomial>, CaseIntersectionFailure> {
    let ideal: Vec<_> = equations
        .iter()
        .map(|polynomial| polynomial.map_coeff(|value| Rational::from(value), Q))
        .collect();
    let basis = GroebnerBasis::new(&ideal, false);
    let mut normalized = Vec::with_capacity(basis.system.len());
    for polynomial in basis.system {
        if polynomial.is_zero() {
            continue;
        }
        let reduced = polynomial.make_primitive();
        if reduced.coefficients.iter().any(|value| !value.is_integer()) {
            return Err(CaseIntersectionFailure::NativeAlgebra);
        }
        // Q make_primitive already gives primitive integral coefficients.
        let reduced = positive_leading(reduced.map_coeff(Rational::numerator, Z));
        if reduced.variables() != equations[0].variables() {
            return Err(CaseIntersectionFailure::NativeAlgebra);
        }
        normalized.push(reduced);
    }
    canonicalize(&mut normalized);
    Ok(normalized)
}

/// Distinct nonconstant zero-locus factors. A zero input must be removed by
/// the caller, and nonzero scalar content/multiplicities create no branches.
pub(super) fn factors(
    equation: &CoefficientPolynomial,
) -> Result<Vec<CoefficientPolynomial>, CaseIntersectionFailure> {
    let mut factors: Vec<_> = equation
        .factor()
        .into_iter()
        .filter(|(factor, _)| !factor.is_constant())
        .map(|(factor, _)| primitive(factor))
        .collect();
    canonicalize(&mut factors);
    if factors.is_empty()
        || factors
            .iter()
            .any(|factor| factor.variables() != equation.variables())
    {
        return Err(CaseIntersectionFailure::NativeAlgebra);
    }
    Ok(factors)
}
