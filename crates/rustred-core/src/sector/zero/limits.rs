use crate::family::symanzik::FeynmanPolynomialLimits;

use super::error::ZeroSectorError;

/// Semantics used for nonzero power shifts during sector analysis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PowerShiftPolicy {
    /// A nonzero, nonintegral shift is a formal regulator. Its support is
    /// included on the generic locus where its numerator is nonzero.
    FormalGeneric,
}

/// Checked construction and rank budgets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZeroSectorLimits {
    pub feynman: FeynmanPolynomialLimits,
    pub max_rank_rows: usize,
    pub max_rank_columns: usize,
    pub max_rank_entries: usize,
    pub max_rank_operations: usize,
    pub max_rref_integer_bits: usize,
    pub max_certificate_entries: usize,
    pub max_kernel_integer_bits: usize,
    pub max_power_shift_pair_checks: usize,
}

impl Default for ZeroSectorLimits {
    fn default() -> Self {
        Self {
            feynman: FeynmanPolynomialLimits::default(),
            max_rank_rows: 4_000_000,
            max_rank_columns: 4_097,
            max_rank_entries: 16_000_000,
            max_rank_operations: 64_000_000,
            max_rref_integer_bits: 1_000_000,
            max_certificate_entries: 4_097,
            max_kernel_integer_bits: 1_000_000,
            max_power_shift_pair_checks: 8_388_608,
        }
    }
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ZeroSectorError> {
    if requested > limit {
        Err(ZeroSectorError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(super) fn checked_add(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, ZeroSectorError> {
    left.checked_add(right)
        .ok_or(ZeroSectorError::ResourceCountOverflow { resource })
}

pub(super) fn checked_mul(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, ZeroSectorError> {
    left.checked_mul(right)
        .ok_or(ZeroSectorError::ResourceCountOverflow { resource })
}
