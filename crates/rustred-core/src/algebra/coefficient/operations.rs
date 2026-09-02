use std::{
    borrow::Cow,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

use symbolica::prelude::{IntegerRing, MultivariatePolynomial, PolyVariable};

use super::{
    Coefficient, CoefficientPolynomialPart, ExactAlgebraError, ExactAlgebraLimits,
    ExactAlgebraOperation,
    validation::{
        check_exact_resource_limit, validate_coefficient_on_map, validate_polynomial_on_map,
    },
};

const SUM_DENOMINATOR_GCD_TERM_PAIRS: &str = "exact sum denominator GCD term pairs";

pub(crate) fn checked_coefficient_add_on_map(
    left: &Coefficient,
    right: &Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
    limits: ExactAlgebraLimits,
) -> Result<Coefficient, ExactAlgebraError> {
    validate_binary_inputs(left, right, variables, limits)?;
    trusted_coefficient_sum_on_map(
        left,
        right,
        variables,
        limits,
        ExactAlgebraOperation::Add,
        false,
    )
}

pub(crate) fn checked_coefficient_sub_on_map(
    left: &Coefficient,
    right: &Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
    limits: ExactAlgebraLimits,
) -> Result<Coefficient, ExactAlgebraError> {
    validate_binary_inputs(left, right, variables, limits)?;
    trusted_coefficient_sum_on_map(
        left,
        right,
        variables,
        limits,
        ExactAlgebraOperation::Subtract,
        true,
    )
}

pub(crate) fn checked_coefficient_mul_on_map(
    left: &Coefficient,
    right: &Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
    limits: ExactAlgebraLimits,
) -> Result<Coefficient, ExactAlgebraError> {
    validate_binary_inputs(left, right, variables, limits)?;
    trusted_coefficient_mul_on_map(left, right, variables, limits)
}

/// Multiply coefficients that have already crossed this exact variable-map
/// boundary.
///
/// This is deliberately visible only inside the algebra owner. It retains
/// prospective work/degree admission and authenticates the native Symbolica
/// result, but does not rescan either sealed operand.
pub(in crate::algebra) fn trusted_coefficient_mul_on_map(
    left: &Coefficient,
    right: &Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
    limits: ExactAlgebraLimits,
) -> Result<Coefficient, ExactAlgebraError> {
    preflight_trusted_input_limits(left, limits)?;
    preflight_trusted_input_limits(right, limits)?;
    preflight_product_degrees(
        &left.numerator,
        &right.numerator,
        ExactAlgebraOperation::Multiply,
        limits,
    )?;
    preflight_product_degrees(
        &left.denominator,
        &right.denominator,
        ExactAlgebraOperation::Multiply,
        limits,
    )?;
    preflight_product_terms(
        left.numerator.nterms(),
        right.numerator.nterms(),
        "exact multiplication numerator terms",
        limits,
    )?;
    preflight_product_terms(
        left.denominator.nterms(),
        right.denominator.nterms(),
        "exact multiplication denominator terms",
        limits,
    )?;
    let result = left * right;
    validate_coefficient_on_map(&result, variables, limits)?;
    Ok(result)
}

pub(crate) fn checked_coefficient_div_on_map(
    numerator: &Coefficient,
    denominator: &Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
    limits: ExactAlgebraLimits,
) -> Result<Coefficient, ExactAlgebraError> {
    validate_binary_inputs(numerator, denominator, variables, limits)?;
    trusted_coefficient_div_on_map(numerator, denominator, variables, limits)
}

/// Divide coefficients that have already crossed this exact variable-map
/// boundary.
///
/// Like the other trusted arithmetic seams, this retains prospective
/// operation limits and authenticates the native Symbolica result without
/// rescanning either sealed operand's map and sparse layout.
pub(in crate::algebra) fn trusted_coefficient_div_on_map(
    numerator: &Coefficient,
    denominator: &Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
    limits: ExactAlgebraLimits,
) -> Result<Coefficient, ExactAlgebraError> {
    preflight_trusted_input_limits(numerator, limits)?;
    preflight_trusted_input_limits(denominator, limits)?;
    if denominator.numerator.coefficients.is_empty() {
        return Err(ExactAlgebraError::DivisionByZero);
    }
    preflight_product_degrees(
        &numerator.numerator,
        &denominator.denominator,
        ExactAlgebraOperation::Divide,
        limits,
    )?;
    preflight_product_degrees(
        &numerator.denominator,
        &denominator.numerator,
        ExactAlgebraOperation::Divide,
        limits,
    )?;
    preflight_product_terms(
        numerator.numerator.nterms(),
        denominator.denominator.nterms(),
        "exact division numerator terms",
        limits,
    )?;
    preflight_product_terms(
        numerator.denominator.nterms(),
        denominator.numerator.nterms(),
        "exact division denominator terms",
        limits,
    )?;
    let result = numerator / denominator;
    validate_coefficient_on_map(&result, variables, limits)?;
    Ok(result)
}

pub(crate) fn checked_coefficient_neg_on_map(
    value: &Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
    limits: ExactAlgebraLimits,
) -> Result<Coefficient, ExactAlgebraError> {
    validate_coefficient_on_map(value, variables, limits)?;
    trusted_coefficient_neg_on_map(value, variables, limits)
}

/// Negate a coefficient already sealed to `variables`, authenticating the
/// native result exactly once.
pub(in crate::algebra) fn trusted_coefficient_neg_on_map(
    value: &Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
    limits: ExactAlgebraLimits,
) -> Result<Coefficient, ExactAlgebraError> {
    preflight_trusted_input_limits(value, limits)?;
    let result = -value.clone();
    validate_coefficient_on_map(&result, variables, limits)?;
    Ok(result)
}

/// Add coefficients already sealed to `variables`, retaining all prospective
/// admission and one exact result authentication without rescanning inputs.
pub(in crate::algebra) fn trusted_coefficient_add_on_map(
    left: &Coefficient,
    right: &Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
    limits: ExactAlgebraLimits,
) -> Result<Coefficient, ExactAlgebraError> {
    trusted_coefficient_sum_on_map(
        left,
        right,
        variables,
        limits,
        ExactAlgebraOperation::Add,
        false,
    )
}

/// Subtract coefficients already sealed to `variables`, retaining all
/// prospective admission and one exact result authentication without
/// rescanning inputs.
pub(in crate::algebra) fn trusted_coefficient_sub_on_map(
    left: &Coefficient,
    right: &Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
    limits: ExactAlgebraLimits,
) -> Result<Coefficient, ExactAlgebraError> {
    trusted_coefficient_sum_on_map(
        left,
        right,
        variables,
        limits,
        ExactAlgebraOperation::Subtract,
        true,
    )
}

fn trusted_coefficient_sum_on_map(
    left: &Coefficient,
    right: &Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
    limits: ExactAlgebraLimits,
    operation: ExactAlgebraOperation,
    subtract: bool,
) -> Result<Coefficient, ExactAlgebraError> {
    preflight_trusted_input_limits(left, limits)?;
    preflight_trusted_input_limits(right, limits)?;
    if left.denominator == right.denominator {
        preflight_sum_terms(
            left.numerator.nterms(),
            right.numerator.nterms(),
            "exact equal-denominator numerator terms",
            limits,
        )?;
    } else {
        preflight_unequal_denominator_sum(left, right, variables, operation, limits)?;
    }
    let native_operation = if subtract {
        "performing exact rational-polynomial subtraction"
    } else {
        "performing exact rational-polynomial addition"
    };
    let result = catch_unwind(AssertUnwindSafe(|| {
        if subtract { left - right } else { left + right }
    }))
    .map_err(|_| ExactAlgebraError::NativePanic {
        operation: native_operation,
    })?;
    validate_coefficient_on_map(&result, variables, limits)?;
    Ok(result)
}

/// Preserve the allocation-free legacy preflight for the overwhelmingly
/// common case where its conservative projection is already admitted. Only a
/// projection failure that a shared denominator factor can lower pays for a
/// native denominator GCD and exact quotients.
fn preflight_unequal_denominator_sum(
    left: &Coefficient,
    right: &Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
    operation: ExactAlgebraOperation,
    limits: ExactAlgebraLimits,
) -> Result<bool, ExactAlgebraError> {
    match preflight_unreduced_denominator_sum(left, right, operation, limits) {
        Ok(()) => Ok(false),
        Err(error) if sum_projection_may_shrink_after_denominator_gcd(&error, operation) => {
            preflight_reduced_denominator_sum(left, right, variables, operation, limits)?;
            Ok(true)
        }
        Err(error) => Err(error),
    }
}

fn preflight_unreduced_denominator_sum(
    left: &Coefficient,
    right: &Coefficient,
    operation: ExactAlgebraOperation,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    preflight_cross_sum_degrees(left, right, operation, limits)?;
    let left_terms = checked_term_product(
        left.numerator.nterms(),
        right.denominator.nterms(),
        "exact addition numerator terms",
    )?;
    let right_terms = checked_term_product(
        right.numerator.nterms(),
        left.denominator.nterms(),
        "exact addition numerator terms",
    )?;
    preflight_sum_terms(
        left_terms,
        right_terms,
        "exact addition numerator terms",
        limits,
    )?;
    preflight_product_terms(
        left.denominator.nterms(),
        right.denominator.nterms(),
        "exact addition denominator terms",
        limits,
    )
}

fn sum_projection_may_shrink_after_denominator_gcd(
    error: &ExactAlgebraError,
    operation: ExactAlgebraOperation,
) -> bool {
    match error {
        ExactAlgebraError::ExponentLimit {
            operation: failed_operation,
            ..
        }
        | ExactAlgebraError::ExponentArithmeticOverflow {
            operation: failed_operation,
            ..
        } => *failed_operation == operation,
        ExactAlgebraError::ResourceLimit { resource, .. }
        | ExactAlgebraError::ResourceCountOverflow { resource } => matches!(
            *resource,
            "exact addition numerator terms" | "exact addition denominator terms"
        ),
        _ => false,
    }
}

#[cfg(test)]
pub(super) fn sum_uses_denominator_gcd_fallback_for_test(
    left: &Coefficient,
    right: &Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
    operation: ExactAlgebraOperation,
    limits: ExactAlgebraLimits,
) -> Result<bool, ExactAlgebraError> {
    validate_binary_inputs(left, right, variables, limits)?;
    if left.denominator == right.denominator {
        return Ok(false);
    }
    preflight_unequal_denominator_sum(left, right, variables, operation, limits)
}

/// Match Symbolica's public rational-polynomial addition algorithm closely
/// enough to admit the same denominator-GCD reduction before projecting
/// numerator and denominator product support.
///
/// The GCD itself remains a native Symbolica operation. The input Cartesian
/// term-pair count is a simple admission gate before native entry; it neither
/// estimates nor bounds Symbolica's GCD work or scratch memory. Exact quotient
/// outputs and the final rational result are authenticated independently.
fn preflight_reduced_denominator_sum(
    left: &Coefficient,
    right: &Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
    operation: ExactAlgebraOperation,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    let gcd_term_pairs = checked_term_product(
        left.denominator.nterms(),
        right.denominator.nterms(),
        SUM_DENOMINATOR_GCD_TERM_PAIRS,
    )?;
    check_exact_resource_limit(
        SUM_DENOMINATOR_GCD_TERM_PAIRS,
        gcd_term_pairs,
        limits.max_term_operations,
    )?;

    let denominator_gcd = catch_unwind(AssertUnwindSafe(|| {
        left.denominator.gcd(&right.denominator)
    }))
    .map_err(|_| ExactAlgebraError::NativePanic {
        operation: "computing an exact sum denominator GCD",
    })?;
    validate_polynomial_on_map(
        &denominator_gcd,
        variables,
        CoefficientPolynomialPart::Denominator,
        limits,
    )?;

    let (left_reduced, right_reduced) = if denominator_gcd.is_one() {
        (
            Cow::Borrowed(&left.denominator),
            Cow::Borrowed(&right.denominator),
        )
    } else {
        (
            Cow::Owned(exact_denominator_quotient(
                &left.denominator,
                &denominator_gcd,
                variables,
                limits,
                "dividing the left exact sum denominator by its GCD",
            )?),
            Cow::Owned(exact_denominator_quotient(
                &right.denominator,
                &denominator_gcd,
                variables,
                limits,
                "dividing the right exact sum denominator by its GCD",
            )?),
        )
    };

    // Symbolica forms N_left * (D_right / gcd) and
    // N_right * (D_left / gcd). Addition cannot increase the degree beyond
    // the maximum degree of those two products.
    preflight_product_degrees(&left.numerator, &right_reduced, operation, limits)?;
    preflight_product_degrees(&right.numerator, &left_reduced, operation, limits)?;
    let left_terms = checked_term_product(
        left.numerator.nterms(),
        right_reduced.nterms(),
        "exact addition numerator terms",
    )?;
    let right_terms = checked_term_product(
        right.numerator.nterms(),
        left_reduced.nterms(),
        "exact addition numerator terms",
    )?;
    preflight_sum_terms(
        left_terms,
        right_terms,
        "exact addition numerator terms",
        limits,
    )?;

    // Use the same algebraically equivalent denominator product selected by
    // Symbolica: prefer small * large over medium * medium when that choice
    // is visible from retained support sizes.
    let (denominator_left, denominator_right) = if left.denominator.nterms()
        > right.denominator.nterms()
        && left.denominator.nterms() > left_reduced.nterms()
    {
        (right_reduced.as_ref(), &left.denominator)
    } else {
        (left_reduced.as_ref(), &right.denominator)
    };
    preflight_product_degrees(denominator_left, denominator_right, operation, limits)?;
    preflight_product_terms(
        denominator_left.nterms(),
        denominator_right.nterms(),
        "exact addition denominator terms",
        limits,
    )
}

fn exact_denominator_quotient(
    denominator: &MultivariatePolynomial<IntegerRing, u16>,
    gcd: &MultivariatePolynomial<IntegerRing, u16>,
    variables: &Arc<Vec<PolyVariable>>,
    limits: ExactAlgebraLimits,
    operation: &'static str,
) -> Result<MultivariatePolynomial<IntegerRing, u16>, ExactAlgebraError> {
    let quotient = catch_unwind(AssertUnwindSafe(|| denominator.try_div(gcd)))
        .map_err(|_| ExactAlgebraError::NativePanic { operation })?
        .ok_or(ExactAlgebraError::NonExactPolynomialDivision { operation })?;
    validate_polynomial_on_map(
        &quotient,
        variables,
        CoefficientPolynomialPart::Denominator,
        limits,
    )?;
    Ok(quotient)
}

fn validate_binary_inputs(
    left: &Coefficient,
    right: &Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    validate_coefficient_on_map(left, variables, limits)?;
    validate_coefficient_on_map(right, variables, limits)
}

/// Re-admit the observable size of a sealed operand under the current policy
/// without repeating map, layout, coefficient-zero, or monomial-order
/// authentication. The common `u16::MAX` exponent policy is O(1); a caller
/// choosing a narrower ceiling pays only the necessary dense-exponent scan.
fn preflight_trusted_input_limits(
    value: &Coefficient,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    for polynomial in [&value.numerator, &value.denominator] {
        check_exact_resource_limit(
            "sealed input polynomial terms",
            polynomial.nterms(),
            limits.max_polynomial_terms,
        )?;
        if limits.max_exponent != u16::MAX {
            if let Some((position, &exponent)) = polynomial
                .exponents
                .iter()
                .enumerate()
                .find(|(_, exponent)| **exponent > limits.max_exponent)
            {
                let variable = if polynomial.variables.is_empty() {
                    0
                } else {
                    position % polynomial.variables.len()
                };
                return Err(ExactAlgebraError::ExponentLimit {
                    operation: ExactAlgebraOperation::Authenticate,
                    variable,
                    requested: u64::from(exponent),
                    limit: limits.max_exponent,
                });
            }
        }
    }
    Ok(())
}

fn preflight_cross_sum_degrees(
    left: &Coefficient,
    right: &Coefficient,
    operation: ExactAlgebraOperation,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    for variable in 0..left.numerator.variables.len() {
        let left_numerator = left.numerator.degree(variable);
        let left_denominator = left.denominator.degree(variable);
        let right_numerator = right.numerator.degree(variable);
        let right_denominator = right.denominator.degree(variable);
        let requested =
            checked_pairwise_exponent_sum(left_numerator, right_denominator, operation, variable)?
                .max(checked_pairwise_exponent_sum(
                    right_numerator,
                    left_denominator,
                    operation,
                    variable,
                )?)
                .max(checked_pairwise_exponent_sum(
                    left_denominator,
                    right_denominator,
                    operation,
                    variable,
                )?);
        check_exact_exponent(operation, variable, u64::from(requested), limits)?;
    }
    Ok(())
}

fn preflight_product_degrees(
    left: &MultivariatePolynomial<IntegerRing, u16>,
    right: &MultivariatePolynomial<IntegerRing, u16>,
    operation: ExactAlgebraOperation,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    for variable in 0..left.variables.len() {
        let requested = checked_pairwise_exponent_sum(
            left.degree(variable),
            right.degree(variable),
            operation,
            variable,
        )?;
        check_exact_exponent(operation, variable, u64::from(requested), limits)?;
    }
    Ok(())
}

fn checked_pairwise_exponent_sum(
    left: u16,
    right: u16,
    operation: ExactAlgebraOperation,
    variable: usize,
) -> Result<u32, ExactAlgebraError> {
    u32::from(left).checked_add(u32::from(right)).ok_or(
        ExactAlgebraError::ExponentArithmeticOverflow {
            operation,
            variable,
            width: 32,
        },
    )
}

pub(super) fn preflight_power_degrees(
    polynomial: &MultivariatePolynomial<IntegerRing, u16>,
    exponent: u64,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    for variable in 0..polynomial.variables.len() {
        let requested = u64::from(polynomial.degree(variable))
            .checked_mul(exponent)
            .ok_or(ExactAlgebraError::ExponentArithmeticOverflow {
                operation: ExactAlgebraOperation::Power,
                variable,
                width: 64,
            })?;
        check_exact_exponent(ExactAlgebraOperation::Power, variable, requested, limits)?;
    }
    Ok(())
}

fn check_exact_exponent(
    operation: ExactAlgebraOperation,
    variable: usize,
    requested: u64,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    if requested > u64::from(limits.max_exponent) {
        Err(ExactAlgebraError::ExponentLimit {
            operation,
            variable,
            requested,
            limit: limits.max_exponent,
        })
    } else {
        Ok(())
    }
}

fn checked_term_product(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, ExactAlgebraError> {
    left.checked_mul(right)
        .ok_or(ExactAlgebraError::ResourceCountOverflow { resource })
}

fn preflight_product_terms(
    left: usize,
    right: usize,
    resource: &'static str,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    let requested = checked_term_product(left, right, resource)?;
    check_exact_resource_limit(resource, requested, limits.max_term_operations)?;
    check_exact_resource_limit(resource, requested, limits.max_polynomial_terms)
}

fn preflight_sum_terms(
    left: usize,
    right: usize,
    resource: &'static str,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    let requested = left
        .checked_add(right)
        .ok_or(ExactAlgebraError::ResourceCountOverflow { resource })?;
    check_exact_resource_limit(resource, requested, limits.max_term_operations)?;
    check_exact_resource_limit(resource, requested, limits.max_polynomial_terms)
}
