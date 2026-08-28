//! Shared checked work accounting for Symanzik operations.

use super::error::FeynmanPolynomialError;
use super::model::FeynmanPolynomialLimits;

/// Aggregate counters shared by every checked algebra step in one public
/// operation. A per-primitive preflight is not sufficient for an adjugate:
/// every minor may fit while their sum is prohibitively large.
#[derive(Clone, Copy, Debug)]
pub(super) struct FeynmanWorkBudget {
    term_operations: usize,
    pub(super) determinant_ring_operations: usize,
    pub(super) limits: FeynmanPolynomialLimits,
}

impl FeynmanWorkBudget {
    pub(super) fn new(limits: FeynmanPolynomialLimits) -> Self {
        Self {
            term_operations: 0,
            determinant_ring_operations: 0,
            limits,
        }
    }

    pub(super) fn charge_term_operations(
        &mut self,
        requested: usize,
    ) -> Result<(), FeynmanPolynomialError> {
        self.term_operations = checked_add(
            self.term_operations,
            requested,
            "aggregate Feynman polynomial operations",
        )?;
        check_limit(
            "aggregate Feynman polynomial operations",
            self.term_operations,
            self.limits.max_term_operations,
        )
    }

    pub(super) fn charge_determinant_ring_operations(
        &mut self,
        requested: usize,
    ) -> Result<(), FeynmanPolynomialError> {
        self.determinant_ring_operations = checked_add(
            self.determinant_ring_operations,
            requested,
            "aggregate Symbolica determinant ring operations",
        )?;
        check_limit(
            "aggregate Symbolica determinant ring operations",
            self.determinant_ring_operations,
            self.limits.max_determinant_ring_operations,
        )
    }
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), FeynmanPolynomialError> {
    if requested > limit {
        Err(FeynmanPolynomialError::ResourceLimit {
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
) -> Result<usize, FeynmanPolynomialError> {
    left.checked_add(right)
        .ok_or(FeynmanPolynomialError::ResourceCountOverflow { resource })
}

pub(super) fn checked_mul(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, FeynmanPolynomialError> {
    left.checked_mul(right)
        .ok_or(FeynmanPolynomialError::ResourceCountOverflow { resource })
}
