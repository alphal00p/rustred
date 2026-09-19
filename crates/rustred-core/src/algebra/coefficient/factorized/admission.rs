//! Resource envelopes and variable-map admission, never algebraic rewrites.

use symbolica::prelude::Integer;

use super::Native;
use crate::algebra::coefficient::validation::{
    check_exact_resource_limit, validate_polynomial_on_map,
};
use crate::algebra::{
    CoefficientContext, CoefficientPolynomial, CoefficientPolynomialPart, ExactAlgebraError,
    ExactAlgebraLimits, ExactAlgebraOperation,
};

pub(super) struct Shape {
    pub terms: usize,
    degrees: Vec<u64>,
}

pub(super) fn overflow() -> ExactAlgebraError {
    ExactAlgebraError::ResourceCountOverflow {
        resource: "factorized coefficient resource envelope",
    }
}

fn polynomial_shape(value: &CoefficientPolynomial) -> Shape {
    Shape {
        terms: value.nterms(),
        degrees: (0..value.variables().len())
            .map(|axis| u64::from(value.degree(axis)))
            .collect(),
    }
}

fn checked_count_power(mut base: usize, mut power: usize) -> Option<usize> {
    let mut result = 1_usize;
    while power > 0 {
        if power & 1 == 1 {
            result = result.checked_mul(base)?;
        }
        power >>= 1;
        if power > 0 {
            base = base.checked_mul(base)?;
        }
    }
    Some(result)
}

pub(super) fn denominator_shape(value: &Native) -> Result<Shape, ExactAlgebraError> {
    let mut degrees = vec![0_u64; value.numerator.variables().len()];
    let mut sparse = Some(1_usize);
    for (factor, power) in &value.denominators {
        let multiplicity = u64::try_from(*power).map_err(|_| overflow())?;
        for (axis, total) in degrees.iter_mut().enumerate() {
            *total = total
                .checked_add(
                    u64::from(factor.degree(axis))
                        .checked_mul(multiplicity)
                        .ok_or_else(overflow)?,
                )
                .ok_or_else(overflow)?;
        }
        sparse = sparse.and_then(|n| n.checked_mul(checked_count_power(factor.nterms(), *power)?));
    }
    // Either bound may overflow while the other still provides a valid bound.
    let degree_box = degrees.iter().try_fold(1_usize, |n, &degree| {
        n.checked_mul(usize::try_from(degree).ok()?.checked_add(1)?)
    });
    let terms = match (sparse, degree_box) {
        (Some(a), Some(b)) => a.min(b),
        (Some(a), None) | (None, Some(a)) => a,
        (None, None) => return Err(overflow()),
    };
    Ok(Shape { terms, degrees })
}

fn admit_degrees(
    shape: &Shape,
    operation: ExactAlgebraOperation,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    for (variable, &requested) in shape.degrees.iter().enumerate() {
        if requested > u64::from(limits.max_exponent) {
            return Err(ExactAlgebraError::ExponentLimit {
                operation,
                variable,
                requested,
                limit: limits.max_exponent,
            });
        }
    }
    Ok(())
}

pub(super) fn validate(
    context: &CoefficientContext,
    value: &Native,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    validate_polynomial_on_map(
        &value.numerator,
        context.variables(),
        CoefficientPolynomialPart::Numerator,
        limits,
    )?;
    if value.denom_coeff.cmp(&Integer::Single(0)).is_eq() {
        return Err(ExactAlgebraError::ZeroDenominator);
    }
    if !value.numerator.is_zero() && value.numer_coeff.cmp(&Integer::Single(0)).is_eq() {
        return Err(ExactAlgebraError::InvalidFactorizedRepresentation {
            detail: "zero scalar with a nonzero numerator polynomial",
        });
    }
    for (factor, power) in &value.denominators {
        validate_polynomial_on_map(
            factor,
            context.variables(),
            CoefficientPolynomialPart::Denominator,
            limits,
        )?;
        if factor.is_zero() {
            return Err(ExactAlgebraError::ZeroDenominator);
        }
        if *power == 0 || factor.is_constant() {
            return Err(ExactAlgebraError::InvalidFactorizedRepresentation {
                detail: "denominator factors must be nonconstant with positive multiplicity",
            });
        }
    }
    let denominator = denominator_shape(value)?;
    admit_degrees(&denominator, ExactAlgebraOperation::Authenticate, limits)?;
    check_exact_resource_limit(
        "factorized expanded denominator term envelope",
        denominator.terms,
        limits.max_polynomial_terms,
    )
}

fn product(
    left: &Shape,
    right: &Shape,
    operation: ExactAlgebraOperation,
    limits: ExactAlgebraLimits,
) -> Result<Shape, ExactAlgebraError> {
    let degrees = left
        .degrees
        .iter()
        .zip(&right.degrees)
        .map(|(&a, &b)| a.checked_add(b).ok_or_else(overflow))
        .collect::<Result<Vec<_>, _>>()?;
    let pairs = left.terms.checked_mul(right.terms).ok_or_else(overflow)?;
    check_exact_resource_limit(
        "factorized native product term-pair envelope",
        pairs,
        limits.max_term_operations,
    )?;
    let shape = Shape {
        degrees,
        terms: pairs,
    };
    admit_degrees(&shape, operation, limits)?;
    Ok(shape)
}

pub(super) fn preflight(
    left: &Native,
    right: &Native,
    context: &CoefficientContext,
    limits: ExactAlgebraLimits,
    add: bool,
) -> Result<(), ExactAlgebraError> {
    validate(context, left, limits)?;
    validate(context, right, limits)?;
    if left.is_zero() || right.is_zero() {
        return Ok(());
    }
    let operation = if add {
        ExactAlgebraOperation::Add
    } else {
        ExactAlgebraOperation::Multiply
    };
    let left_num = polynomial_shape(&left.numerator);
    let right_num = polynomial_shape(&right.numerator);
    let left_den = denominator_shape(left)?;
    let right_den = denominator_shape(right)?;
    if add {
        // Native equal factor lists need no polynomial denominator expansion.
        // Equality is used only to tighten a resource bound, never as proof of
        // rational-function equality. Both variable maps were authenticated.
        if left.denominators == right.denominators {
            let terms = left_num
                .terms
                .checked_add(right_num.terms)
                .ok_or_else(overflow)?;
            check_exact_resource_limit(
                "factorized equal-denominator sum terms",
                terms,
                limits.max_term_operations,
            )?;
        } else {
            let a = product(&left_num, &right_den, operation, limits)?;
            let b = product(&right_num, &left_den, operation, limits)?;
            check_exact_resource_limit(
                "factorized sum term envelope",
                a.terms.checked_add(b.terms).ok_or_else(overflow)?,
                limits.max_term_operations,
            )?;
            product(&left_den, &right_den, operation, limits)?;
        }
    } else {
        product(&left_num, &right_num, operation, limits)?;
        product(&left_den, &right_den, operation, limits)?;
        // Native Mul adds powers of equal factors before any later operation.
        for (factor, power) in &left.denominators {
            if let Some((_, other)) = right
                .denominators
                .iter()
                .find(|(candidate, _)| candidate == factor)
            {
                power.checked_add(*other).ok_or_else(overflow)?;
            }
        }
    }
    Ok(())
}

pub(super) fn preflight_materialization(
    value: &Native,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    let shape = denominator_shape(value)?;
    // Every intermediate denominator lies within the final degree box. Its
    // pair envelope is deliberately conservative; CAS scratch is not bounded.
    product(
        &shape,
        &Shape {
            terms: shape.terms,
            degrees: vec![0; shape.degrees.len()],
        },
        ExactAlgebraOperation::Power,
        limits,
    )?;
    Ok(())
}
