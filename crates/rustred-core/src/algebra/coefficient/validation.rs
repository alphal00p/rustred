use std::{cmp::Ordering, mem::size_of, sync::Arc};

use symbolica::prelude::{Integer, IntegerRing, MultivariatePolynomial, PolyVariable};

use super::{
    Coefficient, CoefficientPolynomialPart, ExactAlgebraError, ExactAlgebraLimits,
    ExactAlgebraOperation,
};

pub(crate) fn validate_coefficient_on_map(
    coefficient: &Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    validate_polynomial_on_map(
        &coefficient.numerator,
        variables,
        CoefficientPolynomialPart::Numerator,
        limits,
    )?;
    validate_polynomial_on_map(
        &coefficient.denominator,
        variables,
        CoefficientPolynomialPart::Denominator,
        limits,
    )?;
    if coefficient.denominator.coefficients.is_empty() {
        return Err(ExactAlgebraError::ZeroDenominator);
    }
    Ok(())
}

pub(crate) fn validate_polynomial_on_map(
    polynomial: &MultivariatePolynomial<IntegerRing, u16>,
    variables: &Arc<Vec<PolyVariable>>,
    part: CoefficientPolynomialPart,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    if polynomial.variables.as_ref() != variables.as_ref() {
        return Err(ExactAlgebraError::VariableMapMismatch { part });
    }
    let expected = polynomial
        .coefficients
        .len()
        .checked_mul(variables.len())
        .ok_or(ExactAlgebraError::ResourceCountOverflow {
            resource: "polynomial exponent layout",
        })?;
    if polynomial.exponents.len() != expected {
        return Err(ExactAlgebraError::MalformedExponentLayout {
            part,
            coefficients: polynomial.coefficients.len(),
            exponents: polynomial.exponents.len(),
            variables: variables.len(),
        });
    }
    check_exact_resource_limit(
        "authenticated polynomial terms",
        polynomial.coefficients.len(),
        limits.max_polynomial_terms,
    )?;
    for (term, coefficient) in polynomial.coefficients.iter().enumerate() {
        // Symbolica's public `Integer` representation can retain a numeric zero
        // in a noncanonical backend variant, whereas `IntegerRing::is_zero`
        // recognizes only the canonical small zero. Authentication is a
        // numeric boundary, so reject every representation of exact zero.
        if coefficient.cmp(&Integer::Single(0)) == Ordering::Equal {
            return Err(ExactAlgebraError::ZeroCoefficient { part, term });
        }
    }
    if variables.is_empty() {
        if polynomial.coefficients.len() > 1 {
            return Err(ExactAlgebraError::NonCanonicalMonomialOrder { part, term: 1 });
        }
        return Ok(());
    }
    for (term, exponents) in polynomial
        .exponents
        .chunks_exact(variables.len())
        .enumerate()
    {
        for (variable, &exponent) in exponents.iter().enumerate() {
            if exponent > limits.max_exponent {
                return Err(ExactAlgebraError::ExponentLimit {
                    operation: ExactAlgebraOperation::Authenticate,
                    variable,
                    requested: u64::from(exponent),
                    limit: limits.max_exponent,
                });
            }
        }
        if term > 0 {
            let previous_start = (term - 1) * variables.len();
            let previous = &polynomial.exponents[previous_start..previous_start + variables.len()];
            if previous.cmp(exponents) != Ordering::Less {
                return Err(ExactAlgebraError::NonCanonicalMonomialOrder { part, term });
            }
        }
    }
    Ok(())
}

pub(super) fn check_exact_resource_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ExactAlgebraError> {
    if requested > limit {
        Err(ExactAlgebraError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(crate) fn coefficient_clone_owned_retained_byte_bound(
    coefficient: &Coefficient,
) -> Option<usize> {
    let polynomial_bytes = |polynomial: &MultivariatePolynomial<IntegerRing, u16>| {
        let mut bytes = polynomial
            .coefficients
            .capacity()
            .checked_mul(size_of::<Integer>())?
            .checked_add(
                polynomial
                    .exponents
                    .capacity()
                    .checked_mul(size_of::<u16>())?,
            )?;
        for coefficient in &polynomial.coefficients {
            if let Integer::Large(value) = coefficient {
                let capacity_bits = usize::try_from(value.capacity()).ok()?;
                bytes = bytes.checked_add(capacity_bits.checked_add(7)?.checked_div(8)?)?;
            }
        }
        Some(bytes)
    };
    size_of::<Coefficient>()
        .checked_add(polynomial_bytes(&coefficient.numerator)?)?
        .checked_add(polynomial_bytes(&coefficient.denominator)?)
}
