//! Checked Symbolica rank boundary and its resource accounting.

use std::fmt::{self, Write as _};

use crate::algebra::matrix::{
    SymbolicaCoefficientMatrixError, SymbolicaCoefficientMatrixLimits, rank_of_coefficient_matrix,
};
use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraError};
use crate::family::{AffineDenominator, IntegralFamilyLimits};

use super::error::IspCompletionError;
use super::model::IspCompletionLimits;

pub(super) fn checked_scalar_product_count(
    loops: usize,
    externals: usize,
) -> Result<usize, IspCompletionError> {
    // Divide the even factor before multiplying. `L*(L+1)` can overflow even
    // when the triangular scalar-product count itself is representable.
    let successor = loops
        .checked_add(1)
        .ok_or(IspCompletionError::ScalarProductCountOverflow { loops, externals })?;
    let (left, right) = if loops % 2 == 0 {
        (loops / 2, successor)
    } else {
        (loops, successor / 2)
    };
    let loop_loop = left
        .checked_mul(right)
        .ok_or(IspCompletionError::ScalarProductCountOverflow { loops, externals })?;
    loop_loop
        .checked_add(
            loops
                .checked_mul(externals)
                .ok_or(IspCompletionError::ScalarProductCountOverflow { loops, externals })?,
        )
        .ok_or(IspCompletionError::ScalarProductCountOverflow { loops, externals })
}

pub(super) fn preflight_rank_matrix(
    rows: usize,
    columns: usize,
    limit: usize,
) -> Result<(), IspCompletionError> {
    let entries = rows
        .checked_mul(columns)
        .ok_or(IspCompletionError::ResourceCountOverflow {
            resource: "automatic ISP rank matrix entries",
        })?;
    check_limit("automatic ISP rank matrix entries", entries, limit)
}

pub(super) fn authenticate_input_rows(
    context: &CoefficientContext,
    denominators: &[AffineDenominator],
    scalar_products: usize,
    family_limits: IntegralFamilyLimits,
) -> Result<(), IspCompletionError> {
    for (denominator, affine) in denominators.iter().enumerate() {
        if affine.coefficients().len() != scalar_products {
            return Err(IspCompletionError::WrongDenominatorRowSize {
                denominator,
                expected: scalar_products,
                actual: affine.coefficients().len(),
            });
        }
        context
            .validate_with_limits(affine.constant(), family_limits.exact_algebra)
            .map_err(|error| IspCompletionError::InvalidInputCoefficient {
                denominator,
                coordinate: None,
                error,
            })?;
        for (coordinate, coefficient) in affine.coefficients().iter().enumerate() {
            context
                .validate_with_limits(coefficient, family_limits.exact_algebra)
                .map_err(|error| IspCompletionError::InvalidInputCoefficient {
                    denominator,
                    coordinate: Some(coordinate),
                    error,
                })?;
        }
    }
    Ok(())
}

pub(super) struct RankBudget {
    limits: IspCompletionLimits,
    pub(super) tests: usize,
    pub(super) operations: usize,
}

impl RankBudget {
    pub(super) const fn new(limits: IspCompletionLimits) -> Self {
        Self {
            limits,
            tests: 0,
            operations: 0,
        }
    }

    fn start_test(&mut self, matrix: &[Vec<Coefficient>]) -> Result<(), IspCompletionError> {
        self.tests =
            self.tests
                .checked_add(1)
                .ok_or(IspCompletionError::ResourceCountOverflow {
                    resource: "automatic ISP rank tests",
                })?;
        check_limit(
            "automatic ISP rank tests",
            self.tests,
            self.limits.max_rank_tests,
        )?;
        let columns = matrix.first().map_or(0, Vec::len);
        preflight_rank_matrix(matrix.len(), columns, self.limits.max_rank_matrix_entries)?;
        preflight_rank_coefficients(matrix.iter().flatten(), self.limits)
    }

    fn remaining_operations(&self) -> Result<usize, IspCompletionError> {
        self.limits
            .max_rank_operations
            .checked_sub(self.operations)
            .ok_or(IspCompletionError::ResourceLimit {
                resource: "automatic ISP rank operations",
                requested: self.operations,
                limit: self.limits.max_rank_operations,
            })
    }

    fn record_native_operations(&mut self, operations: usize) -> Result<(), IspCompletionError> {
        let requested = self.operations.checked_add(operations).ok_or(
            IspCompletionError::ResourceCountOverflow {
                resource: "automatic ISP rank operations",
            },
        )?;
        check_limit(
            "automatic ISP rank operations",
            requested,
            self.limits.max_rank_operations,
        )?;
        self.operations = requested;
        Ok(())
    }
}

pub(super) fn checked_row_rank(
    context: &CoefficientContext,
    matrix: &[Vec<Coefficient>],
    budget: &mut RankBudget,
) -> Result<usize, IspCompletionError> {
    let columns = matrix.first().map_or(0, Vec::len);
    if matrix.iter().any(|row| row.len() != columns) {
        return Err(IspCompletionError::InternalVerificationFailure {
            detail: "rank matrix is not rectangular".to_owned(),
        });
    }
    // Census the borrowed input before the authenticated matrix boundary
    // duplicates every coefficient for Symbolica's destructive rank pass.
    budget.start_test(matrix)?;
    let operations_before = budget.operations;
    let native_limits = SymbolicaCoefficientMatrixLimits {
        exact_algebra: budget.limits.family.exact_algebra,
        max_single_matrix_entries: budget.limits.max_rank_matrix_entries,
        max_live_matrix_entries: budget.limits.max_rank_matrix_entries,
        max_exact_operations: budget.remaining_operations()?,
        max_input_retained_bytes: budget.limits.max_rank_input_retained_bytes,
        max_output_retained_bytes: budget.limits.max_rank_output_retained_bytes,
    };
    let (rank, stats) =
        rank_of_coefficient_matrix(context, matrix, native_limits).map_err(|error| {
            map_symbolica_rank_error(error, operations_before, budget.limits.max_rank_operations)
        })?;
    if stats.rank_calls() != 1 {
        return Err(IspCompletionError::InternalVerificationFailure {
            detail: format!(
                "Symbolica rank boundary recorded {} native rank calls instead of one",
                stats.rank_calls()
            ),
        });
    }
    budget.record_native_operations(stats.exact_operations())?;
    Ok(rank)
}

pub(super) fn map_symbolica_rank_error(
    error: SymbolicaCoefficientMatrixError,
    operations_before: usize,
    operation_limit: usize,
) -> IspCompletionError {
    match error {
        SymbolicaCoefficientMatrixError::ResourceLimit {
            resource: "Symbolica coefficient matrix exact operations",
            requested,
            ..
        }
        | SymbolicaCoefficientMatrixError::ExactAlgebra(ExactAlgebraError::ResourceLimit {
            resource: "Symbolica coefficient matrix exact operations",
            requested,
            ..
        }) => match operations_before.checked_add(requested) {
            Some(requested) => IspCompletionError::ResourceLimit {
                resource: "automatic ISP rank operations",
                requested,
                limit: operation_limit,
            },
            None => IspCompletionError::ResourceCountOverflow {
                resource: "automatic ISP rank operations",
            },
        },
        SymbolicaCoefficientMatrixError::ExactAlgebra(
            ExactAlgebraError::ResourceCountOverflow {
                resource: "Symbolica coefficient matrix exact operations",
            },
        ) => IspCompletionError::ResourceCountOverflow {
            resource: "automatic ISP rank operations",
        },
        SymbolicaCoefficientMatrixError::ResourceLimit {
            resource: "single Symbolica matrix entries" | "live Symbolica matrix entries",
            requested,
            limit,
        } => IspCompletionError::ResourceLimit {
            resource: "automatic ISP rank matrix entries",
            requested,
            limit,
        },
        SymbolicaCoefficientMatrixError::ResourceLimit {
            resource: "coefficient matrix input retained bytes",
            requested,
            limit,
        } => IspCompletionError::ResourceLimit {
            resource: "automatic ISP rank native input retained bytes",
            requested,
            limit,
        },
        SymbolicaCoefficientMatrixError::ResourceLimit {
            resource: "coefficient matrix output retained bytes",
            requested,
            limit,
        } => IspCompletionError::ResourceLimit {
            resource: "automatic ISP rank native output retained bytes",
            requested,
            limit,
        },
        SymbolicaCoefficientMatrixError::ResourceLimit {
            resource,
            requested,
            limit,
        } => IspCompletionError::ResourceLimit {
            resource,
            requested,
            limit,
        },
        SymbolicaCoefficientMatrixError::ResourceCountOverflow { resource } => {
            IspCompletionError::ResourceCountOverflow { resource }
        }
        SymbolicaCoefficientMatrixError::DimensionOverflow { .. } => {
            IspCompletionError::ResourceCountOverflow {
                resource: "automatic ISP native rank matrix dimensions",
            }
        }
        SymbolicaCoefficientMatrixError::AllocationFailure {
            resource,
            requested,
        } => IspCompletionError::AllocationFailure {
            resource,
            requested,
        },
        SymbolicaCoefficientMatrixError::InvalidCoefficient { error, .. }
        | SymbolicaCoefficientMatrixError::ExactAlgebra(error) => {
            IspCompletionError::ExactAlgebra(error)
        }
        internal => IspCompletionError::InternalVerificationFailure {
            detail: format!("native Symbolica rank boundary failed: {internal}"),
        },
    }
}

pub(super) fn preflight_rank_coefficients<'coefficient>(
    coefficients: impl IntoIterator<Item = &'coefficient Coefficient>,
    limits: IspCompletionLimits,
) -> Result<(), IspCompletionError> {
    let mut coefficient_terms = 0usize;
    let mut coefficient_bytes = 0usize;
    for coefficient in coefficients {
        let terms = coefficient
            .numerator
            .nterms()
            .checked_add(coefficient.denominator.nterms())
            .ok_or(IspCompletionError::ResourceCountOverflow {
                resource: "automatic ISP rank coefficient terms",
            })?;
        coefficient_terms = coefficient_terms.checked_add(terms).ok_or(
            IspCompletionError::ResourceCountOverflow {
                resource: "automatic ISP rank coefficient terms",
            },
        )?;
        check_limit(
            "automatic ISP rank coefficient terms",
            coefficient_terms,
            limits.max_rank_coefficient_terms,
        )?;
        coefficient_bytes = checked_coefficient_display_bytes(
            coefficient_bytes,
            coefficient,
            limits.max_rank_coefficient_bytes,
        )?;
    }
    Ok(())
}

pub(super) fn checked_coefficient_display_bytes(
    retained: usize,
    coefficient: &Coefficient,
    limit: usize,
) -> Result<usize, IspCompletionError> {
    let remaining = limit.saturating_sub(retained);
    let mut writer = BoundedByteCounter {
        bytes: 0,
        limit: remaining,
    };
    if write!(&mut writer, "{coefficient}").is_err() {
        return Err(IspCompletionError::ResourceLimit {
            resource: "automatic ISP rank coefficient bytes",
            requested: limit.saturating_add(1),
            limit,
        });
    }
    let requested =
        retained
            .checked_add(writer.bytes)
            .ok_or(IspCompletionError::ResourceCountOverflow {
                resource: "automatic ISP rank coefficient bytes",
            })?;
    check_limit("automatic ISP rank coefficient bytes", requested, limit)?;
    Ok(requested)
}

struct BoundedByteCounter {
    bytes: usize,
    limit: usize,
}

impl fmt::Write for BoundedByteCounter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.bytes = self.bytes.checked_add(value.len()).ok_or(fmt::Error)?;
        if self.bytes > self.limit {
            return Err(fmt::Error);
        }
        Ok(())
    }
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), IspCompletionError> {
    if requested > limit {
        Err(IspCompletionError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
