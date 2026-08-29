use crate::algebra::matrix::RightKernelLimits;
use crate::family::symanzik::FeynmanPolynomialLimits;

use super::error::Error;

/// Checked construction and exact-rank budgets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub feynman: FeynmanPolynomialLimits,
    pub max_rank_rows: usize,
    pub max_rank_columns: usize,
    pub max_rank_entries: usize,
    pub max_rank_operations: usize,
    pub max_rref_integer_bits: usize,
    pub max_kernel_entries: usize,
    pub max_kernel_integer_bits: usize,
    pub max_power_shift_pair_checks: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            feynman: FeynmanPolynomialLimits::default(),
            max_rank_rows: 4_000_000,
            max_rank_columns: 4_097,
            max_rank_entries: 16_000_000,
            max_rank_operations: 64_000_000,
            max_rref_integer_bits: 1_000_000,
            max_kernel_entries: 4_097,
            max_kernel_integer_bits: 1_000_000,
            max_power_shift_pair_checks: 8_388_608,
        }
    }
}

impl Limits {
    pub(super) fn right_kernel(self) -> RightKernelLimits {
        RightKernelLimits {
            max_rows: self.max_rank_rows,
            max_columns: self.max_rank_columns,
            max_entries: self.max_rank_entries,
            max_rank_operations: self.max_rank_operations,
            max_rref_integer_bits: self.max_rref_integer_bits,
            max_kernel_entries: self.max_kernel_entries,
            max_kernel_integer_bits: self.max_kernel_integer_bits,
        }
    }
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), Error> {
    if requested > limit {
        Err(Error::ResourceLimit {
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
) -> Result<usize, Error> {
    left.checked_add(right)
        .ok_or(Error::ResourceCountOverflow { resource })
}

pub(super) fn checked_mul(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, Error> {
    left.checked_mul(right)
        .ok_or(Error::ResourceCountOverflow { resource })
}
