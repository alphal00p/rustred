use crate::algebra::{Coefficient, CoefficientPolynomial};

use super::super::construction::upper_triangular_index;
use super::super::error::SymbolicaAffineDenominatorError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::input::affine) enum ProjectionGroup {
    Constant,
    Coordinate(usize),
    ExternalPair(usize, usize),
}

#[allow(clippy::too_many_arguments)]
pub(in crate::input::affine) fn classify_numerator_term(
    exponents: &[u16],
    expected_variables: usize,
    base_count: usize,
    loops: usize,
    externals: usize,
    loop_loop_count: usize,
    numerator_term: usize,
) -> Result<ProjectionGroup, SymbolicaAffineDenominatorError> {
    if exponents.len() != expected_variables {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "combined numerator exponent row has the wrong length",
            },
        );
    }
    match momentum_degree(exponents, base_count)? {
        0 => Ok(ProjectionGroup::Constant),
        1 => Err(SymbolicaAffineDenominatorError::MomentumDegreeOne { numerator_term }),
        2 => classify_quadratic_group(
            &exponents[base_count..],
            loops,
            externals,
            loop_loop_count,
            numerator_term,
        ),
        degree => Err(SymbolicaAffineDenominatorError::MomentumDegreeTooHigh {
            numerator_term,
            degree,
        }),
    }
}

fn classify_quadratic_group(
    momentum_exponents: &[u16],
    loops: usize,
    externals: usize,
    loop_loop_count: usize,
    numerator_term: usize,
) -> Result<ProjectionGroup, SymbolicaAffineDenominatorError> {
    let mut first = None;
    let mut second = None;
    for (position, &exponent) in momentum_exponents.iter().enumerate() {
        if exponent == 0 {
            continue;
        }
        if first.is_none() {
            first = Some((position, exponent));
        } else if second.is_none() {
            second = Some((position, exponent));
        } else {
            return Err(
                SymbolicaAffineDenominatorError::InvalidQuadraticMomentumMonomial {
                    numerator_term,
                },
            );
        }
    }
    let (left, right) = match (first, second) {
        (Some((position, 2)), None) => (position, position),
        (Some((left, 1)), Some((right, 1))) => (left, right),
        _ => {
            return Err(
                SymbolicaAffineDenominatorError::InvalidQuadraticMomentumMonomial {
                    numerator_term,
                },
            );
        }
    };
    let momentum_count = loops.checked_add(externals).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "quadratic momentum positions",
        },
    )?;
    if right >= momentum_count {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "quadratic momentum position exceeds the combined map",
            },
        );
    }
    match (left < loops, right < loops) {
        (true, true) => Ok(ProjectionGroup::Coordinate(upper_triangular_index(
            left, right, loops,
        )?)),
        (true, false) => {
            let external = right - loops;
            let offset = left.checked_mul(externals).and_then(|value| {
                loop_loop_count
                    .checked_add(value)
                    .and_then(|value| value.checked_add(external))
            });
            Ok(ProjectionGroup::Coordinate(offset.ok_or(
                SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "loop-external coordinate index",
                },
            )?))
        }
        (false, true) => Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "canonical momentum exponents reversed a loop-external pair",
            },
        ),
        (false, false) => Ok(ProjectionGroup::ExternalPair(left - loops, right - loops)),
    }
}

pub(in crate::input::affine) fn reject_momentum_denominator(
    coefficient: &Coefficient,
    base_count: usize,
) -> Result<(), SymbolicaAffineDenominatorError> {
    if polynomial_contains_momentum(&coefficient.denominator, base_count)? {
        Err(SymbolicaAffineDenominatorError::MomentumDependentRationalDenominator)
    } else {
        Ok(())
    }
}

pub(in crate::input::affine) fn coefficient_contains_momentum(
    coefficient: &Coefficient,
    base_count: usize,
) -> Result<bool, SymbolicaAffineDenominatorError> {
    Ok(
        polynomial_contains_momentum(&coefficient.numerator, base_count)?
            || polynomial_contains_momentum(&coefficient.denominator, base_count)?,
    )
}

pub(in crate::input::affine) fn polynomial_contains_momentum(
    polynomial: &CoefficientPolynomial,
    base_count: usize,
) -> Result<bool, SymbolicaAffineDenominatorError> {
    if polynomial.variables.len() < base_count {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "polynomial variable map is shorter than the base map",
            },
        );
    }
    Ok(polynomial.exponents_iter().any(|exponents| {
        exponents[base_count..]
            .iter()
            .any(|exponent| *exponent != 0)
    }))
}

pub(in crate::input::affine) fn momentum_degree(
    exponents: &[u16],
    base_count: usize,
) -> Result<u32, SymbolicaAffineDenominatorError> {
    let suffix = exponents.get(base_count..).ok_or(
        SymbolicaAffineDenominatorError::InternalVerificationFailure {
            detail: "polynomial exponent row is shorter than the base map",
        },
    )?;
    suffix.iter().try_fold(0u32, |degree, exponent| {
        degree.checked_add(u32::from(*exponent)).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "momentum degree",
            },
        )
    })
}
