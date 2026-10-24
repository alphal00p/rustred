//! Resource limits and allocation-envelope helpers for indexed algebra.

use symbolica::prelude::*;

use crate::algebra::{CoefficientPolynomial, ExactAlgebraError, ExactAlgebraLimits};

use super::error::IndexedAlgebraError;

/// Resource policy for constructing one authenticated indexed context.
///
/// The family-backed generator path uses the same default index bound as
/// `IntegralFamilyLimits::max_scalar_products`. Standalone callers that raise
/// the arity bound must also choose explicit fingerprint and native-name work
/// budgets appropriate for their input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexedContextLimits {
    /// Enforce count/work thresholds only for an explicitly requested diagnostic.
    pub diagnostics_enabled: bool,
    pub max_index_variables: usize,
    pub max_fingerprint_bytes: usize,
    pub max_native_symbol_name_bytes: usize,
}

impl Default for IndexedContextLimits {
    fn default() -> Self {
        Self {
            diagnostics_enabled: false,
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
    /// Enforce count/work thresholds only for an explicitly requested diagnostic.
    pub diagnostics_enabled: bool,
    pub exact_algebra: ExactAlgebraLimits,
    pub max_specialization_power_operations: usize,
    /// Maximum conservative magnitude bit length of an integer coefficient
    /// produced while specializing or affinely translating index variables.
    pub max_specialization_integer_bits: usize,
}

impl IndexedAlgebraLimits {
    /// Explicitly lift aggregate specialization/term quotas for trusted work;
    /// native exponent representation and checked counter arithmetic remain.
    pub const fn unbounded_resources() -> Self {
        Self {
            diagnostics_enabled: false,
            exact_algebra: ExactAlgebraLimits::unbounded_resources(),
            max_specialization_power_operations: usize::MAX,
            max_specialization_integer_bits: usize::MAX,
        }
    }
}

impl Default for IndexedAlgebraLimits {
    fn default() -> Self {
        Self {
            diagnostics_enabled: false,
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

pub(crate) fn ceil_log2(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        usize::BITS as usize - (value - 1).leading_zeros() as usize
    }
}

pub(crate) fn integer_magnitude_bits(value: &Integer) -> u64 {
    match value {
        Integer::Single(value) => {
            let magnitude = value.unsigned_abs();
            u64::from(i64::BITS - magnitude.leading_zeros())
        }
        Integer::Double(value) => {
            let magnitude = value.get().unsigned_abs();
            u64::from(i128::BITS - magnitude.leading_zeros())
        }
        Integer::Large(value) => u64::from(value.significant_bits()),
    }
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
    diagnostics_enabled: bool,
) -> Result<(), IndexedAlgebraError> {
    symbolica::utils::resources::checkpoint().map_err(ExactAlgebraError::Resource)?;
    if diagnostics_enabled && requested > limit {
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
        assert_eq!(
            integer_magnitude_bits(&Integer::Double(i128::MIN.into())),
            128
        );

        let large = Integer::from(1) << 200_u32;
        assert!(matches!(large, Integer::Large(_)));
        assert_eq!(integer_magnitude_bits(&large), 201);
    }
}

impl IndexedContextLimits {
    /// Explicitly enable diagnostic count/work thresholds.
    pub fn with_diagnostics(mut self) -> Self {
        self.diagnostics_enabled = true;
        self
    }
}

impl IndexedAlgebraLimits {
    /// Explicitly enable diagnostic count/work thresholds.
    pub fn with_diagnostics(mut self) -> Self {
        self.diagnostics_enabled = true;
        self.exact_algebra = self.exact_algebra.with_diagnostics();
        self
    }
}

impl IndexedAlgebraLimits {
    pub(crate) fn bounded_by(self, outer: Self) -> Self {
        let selected = match (self.diagnostics_enabled, outer.diagnostics_enabled) {
            (true, false) | (false, false) => self,
            (false, true) => outer,
            (true, true) => Self {
                diagnostics_enabled: true,
                exact_algebra: self.exact_algebra,
                max_specialization_power_operations: self
                    .max_specialization_power_operations
                    .min(outer.max_specialization_power_operations),
                max_specialization_integer_bits: self
                    .max_specialization_integer_bits
                    .min(outer.max_specialization_integer_bits),
            },
        };
        Self {
            exact_algebra: self.exact_algebra.bounded_by(outer.exact_algebra),
            ..selected
        }
    }
}

/// Conservative work/support censuses are diagnostic data, not allocation sizes.
pub(super) fn estimated_indexed_add(
    resource: &'static str,
    left: usize,
    right: usize,
    diagnostics_enabled: bool,
) -> Result<usize, IndexedAlgebraError> {
    if diagnostics_enabled {
        checked_indexed_add(resource, left, right)
    } else {
        Ok(left.saturating_add(right))
    }
}
pub(super) fn estimated_indexed_mul(
    resource: &'static str,
    left: usize,
    right: usize,
    diagnostics_enabled: bool,
) -> Result<usize, IndexedAlgebraError> {
    if diagnostics_enabled {
        checked_indexed_mul(resource, left, right)
    } else {
        Ok(left.saturating_mul(right))
    }
}
