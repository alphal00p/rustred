//! Resource limits and allocation-envelope helpers for indexed algebra.

use symbolica::prelude::*;

use crate::algebra::{CoefficientPolynomial, ExactAlgebraLimits};

use super::error::IndexedAlgebraError;

/// Resource policy for constructing one authenticated indexed context.
///
/// The family-backed generator path uses the same default index bound as
/// `IntegralFamilyLimits::max_scalar_products`. Standalone callers that raise
/// the arity bound must also choose explicit fingerprint and native-name work
/// budgets appropriate for their input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexedContextLimits {
    pub max_index_variables: usize,
    pub max_fingerprint_bytes: usize,
    pub max_native_symbol_name_bytes: usize,
}

impl Default for IndexedContextLimits {
    fn default() -> Self {
        Self {
            max_index_variables: 4_096,
            max_fingerprint_bytes: 1024 * 1024 * 1024,
            max_native_symbol_name_bytes: 1_000_000,
        }
    }
}

/// Explicit upper bounds around Symbolica operations whose output can expand
/// under an affine index translation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexedAlgebraLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub max_specialization_power_operations: usize,
    /// Maximum conservative magnitude bit length of an integer coefficient
    /// produced while specializing or affinely translating index variables.
    pub max_specialization_integer_bits: usize,
}

impl Default for IndexedAlgebraLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_specialization_power_operations: 16_000_000,
            max_specialization_integer_bits: 16_000_000,
        }
    }
}

pub(super) fn verify_polynomial_execution_envelope(
    polynomial: &CoefficientPolynomial,
    term_bound: usize,
    exponent_entry_bound: usize,
    integer_bit_bound: usize,
    operation: &'static str,
) -> Result<(), IndexedAlgebraError> {
    let integer_bit_bound = u64::try_from(integer_bit_bound).unwrap_or(u64::MAX);
    if polynomial.nterms() > term_bound
        || polynomial.exponents.len() > exponent_entry_bound
        || polynomial
            .coefficients
            .iter()
            .any(|coefficient| integer_magnitude_bits(coefficient) > integer_bit_bound)
    {
        return Err(IndexedAlgebraError::Symbolica(format!(
            "{operation} escaped its allocation-free preflight envelope"
        )));
    }
    Ok(())
}

pub(super) fn checked_indexed_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, IndexedAlgebraError> {
    left.checked_add(right)
        .ok_or(IndexedAlgebraError::ResourceCountOverflow { resource })
}

pub(super) fn checked_indexed_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, IndexedAlgebraError> {
    left.checked_mul(right)
        .ok_or(IndexedAlgebraError::ResourceCountOverflow { resource })
}

pub(super) fn ceil_log2(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        usize::BITS as usize - (value - 1).leading_zeros() as usize
    }
}

pub(super) fn integer_magnitude_bits(value: &Integer) -> u64 {
    match value {
        Integer::Single(value) => {
            let magnitude = value.unsigned_abs();
            u64::from(i64::BITS - magnitude.leading_zeros())
        }
        Integer::Double(value) => {
            let magnitude = value.unsigned_abs();
            u64::from(i128::BITS - magnitude.leading_zeros())
        }
        Integer::Large(value) => u64::from(value.significant_bits()),
    }
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), IndexedAlgebraError> {
    if requested > limit {
        Err(IndexedAlgebraError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::integer_magnitude_bits;
    use symbolica::prelude::Integer;

    #[test]
    fn integer_magnitude_bits_handles_double_and_large_boundaries() {
        assert_eq!(integer_magnitude_bits(&Integer::Double(i128::MIN)), 128);

        let large = Integer::from(1) << 200_u32;
        assert!(matches!(large, Integer::Large(_)));
        assert_eq!(integer_magnitude_bits(&large), 201);
    }
}
