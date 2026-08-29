use crate::algebra::{Coefficient, CoefficientContext};

use super::super::budget::coefficient_census;
use super::super::error::SymbolicaAffineDenominatorError;
use super::super::limits::SymbolicaAffineDenominatorLimits;
use super::check_limit;

pub(in crate::input::affine) fn validate_external_gram(
    coefficients: &CoefficientContext,
    external_momenta: &[String],
    gram: &[Vec<Coefficient>],
    limits: SymbolicaAffineDenominatorLimits,
) -> Result<(), SymbolicaAffineDenominatorError> {
    let expected = external_momenta.len();
    let expected_entries = expected.checked_mul(expected).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "external Gram entries",
        },
    )?;
    check_limit(
        "external Gram entries",
        expected_entries,
        limits.max_external_gram_entries,
    )?;
    if gram.len() != expected {
        return Err(SymbolicaAffineDenominatorError::WrongExternalGramRowCount {
            expected,
            actual: gram.len(),
        });
    }
    for (row, entries) in gram.iter().enumerate() {
        if entries.len() != expected {
            return Err(
                SymbolicaAffineDenominatorError::WrongExternalGramColumnCount {
                    row,
                    expected,
                    actual: entries.len(),
                },
            );
        }
    }

    let mut polynomial_terms = 0usize;
    let mut exponent_entries = 0usize;
    let mut integer_bits = 0usize;
    for (row, entries) in gram.iter().enumerate() {
        for (column, coefficient) in entries.iter().enumerate() {
            coefficients
                .validate_with_limits(coefficient, limits.exact_algebra)
                .map_err(|error| {
                    SymbolicaAffineDenominatorError::InvalidExternalGramCoefficient {
                        row,
                        column,
                        error,
                    }
                })?;
            let coefficient_terms = coefficient
                .numerator
                .nterms()
                .checked_add(coefficient.denominator.nterms())
                .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "external Gram polynomial terms",
                })?;
            polynomial_terms = polynomial_terms.checked_add(coefficient_terms).ok_or(
                SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "external Gram polynomial terms",
                },
            )?;
            check_limit(
                "external Gram polynomial terms",
                polynomial_terms,
                limits.max_external_gram_polynomial_terms,
            )?;
            let coefficient_exponents = coefficient_terms
                .checked_mul(coefficients.parameter_names().len())
                .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "external Gram exponent entries",
                })?;
            exponent_entries = exponent_entries.checked_add(coefficient_exponents).ok_or(
                SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "external Gram exponent entries",
                },
            )?;
            check_limit(
                "external Gram exponent entries",
                exponent_entries,
                limits.max_external_gram_exponent_entries,
            )?;
            integer_bits = integer_bits
                .checked_add(coefficient_census(coefficient)?.integer_bits)
                .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "external Gram integer bits",
                })?;
            check_limit(
                "external Gram integer bits",
                integer_bits,
                limits.max_external_gram_integer_bits,
            )?;
            if gram[column][row] != *coefficient {
                return Err(SymbolicaAffineDenominatorError::AsymmetricExternalGram {
                    row,
                    column,
                });
            }
        }
    }
    Ok(())
}
