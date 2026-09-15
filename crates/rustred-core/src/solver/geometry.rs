//! Exact coordinate intersections for the sector case queue.
//!
//! This service never turns an affine or nonlinear equality into sampled
//! coordinate faces. A cold native exact ideal-normalization fallback can
//! expose coordinates implied jointly by nonlinear equations. Intersections
//! that still do not simplify to coordinates retain the original conjunction.

use std::cmp::Ordering;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::prelude::Integer;

use crate::algebra::CoefficientPolynomial;

use super::CoordinateCase;

mod normalization;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeometryError {
    InvalidInput(&'static str),
    /// The original conjunction, including any coordinate constraints that
    /// exposed the residual geometry. It is interpreted on the input case.
    UnsupportedGeometry {
        equations: Vec<CoefficientPolynomial>,
    },
    /// A feasible coordinate value exceeds the compact integral-key range.
    CompactOverflow {
        axis: usize,
        value: Integer,
    },
    NativeAlgebra,
}

impl fmt::Display for GeometryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(detail) => write!(f, "invalid case geometry: {detail}"),
            Self::UnsupportedGeometry { equations } => write!(
                f,
                "case intersection retains unsupported affine or nonlinear geometry ({} equations)",
                equations.len()
            ),
            Self::CompactOverflow { axis, value } => {
                write!(
                    f,
                    "coordinate n{axis}={value} exceeds the compact power range"
                )
            }
            Self::NativeAlgebra => write!(f, "native algebra failed during case intersection"),
        }
    }
}

impl std::error::Error for GeometryError {}

/// Intersect a coordinate case with an AND of exact index equations.
///
/// Native substitution is repeated until no additional coordinate is fixed.
/// Thus a coupled equation encountered before `n1 = 1` can still simplify to
/// a coordinate equation on a later pass. Contradictions, noninteger roots,
/// and sector-incompatible roots give `None`; unsupported geometry is not an
/// empty case. A genuinely nonlinear multi-equation conjunction that defeats
/// substitution receives one exact native Groebner-basis normalization and
/// one coordinate retry. The empty conjunction leaves the parent unchanged.
pub(super) fn intersect<const N: usize>(
    parent: &CoordinateCase<N>,
    conjunction: &[CoefficientPolynomial],
    indices: &[usize; N],
    sector: &[bool; N],
) -> Result<Option<CoordinateCase<N>>, GeometryError> {
    preflight(conjunction, indices)?;
    if !parent.is_in_sector(sector) {
        return Ok(None);
    }
    catch_unwind(AssertUnwindSafe(|| {
        let result = intersect_native(parent, conjunction, indices, sector);
        if let Err(GeometryError::UnsupportedGeometry { .. }) = &result
            && let Some(normalized) = normalization::normalize(parent, conjunction, indices)?
        {
            // Deliberately call the primitive, not this entry point: no
            // normalization recursion and no change to unsupported provenance.
            match intersect_native(parent, &normalized, indices, sector) {
                Err(GeometryError::UnsupportedGeometry { .. }) => result,
                retry => retry,
            }
        } else {
            result
        }
    }))
    .map_err(|_| GeometryError::NativeAlgebra)?
}

pub(super) fn preflight<const N: usize>(
    conjunction: &[CoefficientPolynomial],
    indices: &[usize; N],
) -> Result<(), GeometryError> {
    for (axis, position) in indices.iter().enumerate() {
        if indices[..axis].contains(position) {
            return Err(GeometryError::InvalidInput(
                "index-variable positions must be distinct",
            ));
        }
    }
    let Some(first) = conjunction.first() else {
        return Ok(());
    };
    let variables = first.variables();
    if indices.iter().any(|&position| position >= variables.len()) {
        return Err(GeometryError::InvalidInput(
            "index-variable position is outside the coefficient variable map",
        ));
    }
    for equation in conjunction {
        if equation.variables() != variables {
            return Err(GeometryError::InvalidInput(
                "equations must share one coefficient variable map",
            ));
        }
        if equation.coefficients.len().checked_mul(variables.len())
            != Some(equation.exponents.len())
        {
            return Err(GeometryError::InvalidInput(
                "native coefficient and exponent arrays have inconsistent lengths",
            ));
        }
        for position in 0..variables.len() {
            if !indices.contains(&position) && equation.degree(position) != 0 {
                return Err(GeometryError::InvalidInput(
                    "exceptional equations must contain only index variables",
                ));
            }
        }
    }
    Ok(())
}

fn intersect_native<const N: usize>(
    parent: &CoordinateCase<N>,
    conjunction: &[CoefficientPolynomial],
    indices: &[usize; N],
    sector: &[bool; N],
) -> Result<Option<CoordinateCase<N>>, GeometryError> {
    // Keep native integers until every equation is checked. A large value
    // can prove a later contradiction even though it cannot become a key.
    let mut fixed: [Option<Integer>; N] =
        std::array::from_fn(|axis| parent.fixed()[axis].map(Integer::from));
    loop {
        let mut changed = false;
        let mut unsupported = false;
        for source in conjunction {
            let mut equation = source.clone();
            for (axis, value) in fixed.iter().enumerate() {
                if let Some(value) = value {
                    equation = equation.replace(indices[axis], value);
                }
            }
            if equation.is_zero() {
                continue;
            }
            if equation.is_constant() {
                return Ok(None);
            }
            let mut active = indices
                .iter()
                .enumerate()
                .filter(|(_, position)| equation.degree(**position) != 0);
            let (axis, &position) = active
                .next()
                .expect("preflight restricted nonconstant equations to index variables");
            if active.next().is_some() || equation.degree(position) != 1 {
                unsupported = true;
                continue;
            }
            let mut exponents = vec![0; equation.nvars()];
            exponents[position] = 1;
            let slope = equation
                .coefficient(&exponents)
                .expect("linear equation has a nonzero linear coefficient");
            let (value, remainder) = (-equation.get_constant()).quot_rem(&slope);
            if !remainder.is_zero() || ((!value.is_negative() && !value.is_zero()) != sector[axis])
            {
                return Ok(None);
            }
            debug_assert!(fixed[axis].is_none());
            fixed[axis] = Some(value);
            changed = true;
        }
        if changed {
            continue;
        }
        if unsupported {
            return Err(GeometryError::UnsupportedGeometry {
                equations: conjunction.to_vec(),
            });
        }
        let mut compact = [None; N];
        for (axis, value) in fixed.into_iter().enumerate() {
            if let Some(value) = value {
                let Some(small) = value
                    .to_i64()
                    .and_then(|value| i16::try_from(value).ok())
                    .filter(|&value| super::Power::new(false, value).is_ok())
                else {
                    return Err(GeometryError::CompactOverflow { axis, value });
                };
                compact[axis] = Some(small);
            }
        }
        return Ok(Some(
            CoordinateCase::new(compact).expect("each coordinate was checked for compactness"),
        ));
    }
}

/// Whether the second case implies every coordinate equality of the first.
pub(super) fn contains<const N: usize>(
    container: &CoordinateCase<N>,
    contained: &CoordinateCase<N>,
) -> bool {
    container
        .fixed()
        .iter()
        .zip(contained.fixed())
        .all(|(required, actual)| required.is_none() || required == actual)
}

/// SpIRed's `unsolved` order restricted to coordinate cases: constraint count,
/// then fixed-axis list, then equation constants. Its equation for `n = v` is
/// `n - v = 0`, so values are compared in reverse after axis patterns agree.
pub(super) fn compare_cases<const N: usize>(
    left: &CoordinateCase<N>,
    right: &CoordinateCase<N>,
) -> Ordering {
    let left_axes = left
        .fixed()
        .iter()
        .enumerate()
        .filter_map(|(axis, value)| value.map(|_| axis));
    let right_axes = right
        .fixed()
        .iter()
        .enumerate()
        .filter_map(|(axis, value)| value.map(|_| axis));
    left_axes
        .clone()
        .count()
        .cmp(&right_axes.clone().count())
        .then_with(|| left_axes.cmp(right_axes))
        .then_with(|| {
            // Both coordinate patterns are now identical. Option's ordering
            // does not matter; this compares only their fixed integer values.
            right.fixed().cmp(left.fixed())
        })
}

#[cfg(test)]
#[path = "geometry/tests.rs"]
mod tests;
