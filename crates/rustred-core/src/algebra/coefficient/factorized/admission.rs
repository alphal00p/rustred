//! Resource envelopes and variable-map admission, never algebraic rewrites.

use symbolica::prelude::Integer;

use std::sync::Arc;

use super::{FactorizedCoefficient, Native};
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

struct OperandResources {
    numerator: Shape,
    denominator: Shape,
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
) -> Result<usize, ExactAlgebraError> {
    let pairs = left.terms.checked_mul(right.terms).ok_or_else(overflow)?;
    check_exact_resource_limit(
        "factorized native product term-pair envelope",
        pairs,
        limits.max_term_operations,
    )?;
    for (variable, (&a, &b)) in left.degrees.iter().zip(&right.degrees).enumerate() {
        let requested = a.checked_add(b).ok_or_else(overflow)?;
        if requested > u64::from(limits.max_exponent) {
            return Err(ExactAlgebraError::ExponentLimit {
                operation,
                variable,
                requested,
                limit: limits.max_exponent,
            });
        }
    }
    Ok(pairs)
}

/// Recheck current policy, without rescanning a sealed operand's immutable
/// monomial layout or coefficient canonicality. Full validation remains at
/// every native-result boundary. No resource metadata is retained in the cache.
fn operand_resources(
    sealed: &FactorizedCoefficient,
    context: &CoefficientContext,
    limits: ExactAlgebraLimits,
) -> Result<OperandResources, ExactAlgebraError> {
    let value = &sealed.value;
    let variables = value.numerator.variables();
    if !Arc::ptr_eq(variables, context.variables())
        && variables.as_ref() != context.variables().as_ref()
    {
        return Err(ExactAlgebraError::VariableMapMismatch {
            part: CoefficientPolynomialPart::Numerator,
        });
    }
    // Admission bound every factor to the numerator's exact ordered map.
    // Its positive multiplicity makes each factor degree no larger than the
    // corresponding expanded-denominator degree checked below.
    for polynomial in
        std::iter::once(&value.numerator).chain(value.denominators.iter().map(|(factor, _)| factor))
    {
        check_exact_resource_limit(
            "authenticated polynomial terms",
            polynomial.nterms(),
            limits.max_polynomial_terms,
        )?;
    }
    let numerator = polynomial_shape(&value.numerator);
    admit_degrees(&numerator, ExactAlgebraOperation::Authenticate, limits)?;
    let denominator = denominator_shape(value)?;
    admit_degrees(&denominator, ExactAlgebraOperation::Authenticate, limits)?;
    check_exact_resource_limit(
        "factorized expanded denominator term envelope",
        denominator.terms,
        limits.max_polynomial_terms,
    )?;
    Ok(OperandResources {
        numerator,
        denominator,
    })
}

pub(super) fn preflight(
    left: &FactorizedCoefficient,
    right: &FactorizedCoefficient,
    context: &CoefficientContext,
    limits: ExactAlgebraLimits,
    add: bool,
) -> Result<(), ExactAlgebraError> {
    let left_resources = operand_resources(left, context, limits)?;
    let right_resources = operand_resources(right, context, limits)?;
    let left = &left.value;
    let right = &right.value;
    if left.is_zero() || right.is_zero() {
        return Ok(());
    }
    let operation = if add {
        ExactAlgebraOperation::Add
    } else {
        ExactAlgebraOperation::Multiply
    };
    let left_num = &left_resources.numerator;
    let right_num = &right_resources.numerator;
    let left_den = &left_resources.denominator;
    let right_den = &right_resources.denominator;
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
            let a = product(left_num, right_den, operation, limits)?;
            let b = product(right_num, left_den, operation, limits)?;
            check_exact_resource_limit(
                "factorized sum term envelope",
                a.checked_add(b).ok_or_else(overflow)?,
                limits.max_term_operations,
            )?;
            product(left_den, right_den, operation, limits)?;
        }
    } else {
        product(left_num, right_num, operation, limits)?;
        product(left_den, right_den, operation, limits)?;
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
    value: &FactorizedCoefficient,
    context: &CoefficientContext,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    let shape = operand_resources(value, context, limits)?.denominator;
    // Every intermediate denominator lies within the final degree box. Its
    // pair envelope is deliberately conservative; CAS scratch is not bounded.
    check_exact_resource_limit(
        "factorized native product term-pair envelope",
        shape.terms.checked_mul(shape.terms).ok_or_else(overflow)?,
        limits.max_term_operations,
    )
}
