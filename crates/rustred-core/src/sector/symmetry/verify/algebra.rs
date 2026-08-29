use crate::algebra::matrix::{
    SymbolicaCoefficientMatrixError, SymbolicaCoefficientMatrixLimits,
    SymbolicaCoefficientMatrixStats,
};
use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraError};

use super::super::limits::{check_limit, checked_add};
use super::super::{CoefficientMatrix, Error, Limits, Stats};

pub(super) struct ReplayAlgebra<'a> {
    pub(super) context: &'a CoefficientContext,
    pub(super) limits: Limits,
    pub(super) stats: Stats,
}

impl<'a> ReplayAlgebra<'a> {
    pub(super) fn new(context: &'a CoefficientContext, limits: Limits) -> Self {
        Self {
            context,
            limits,
            stats: Stats::default(),
        }
    }

    pub(super) fn charge_entries(&mut self, entries: usize) -> Result<(), Error> {
        self.stats.matrix_entries = checked_add(
            self.stats.matrix_entries,
            entries,
            "retained symmetry matrix entries",
        )?;
        check_limit(
            "retained matrix entries",
            self.stats.matrix_entries,
            self.limits.max_matrix_entries,
        )
    }

    fn charge_operation(&mut self) -> Result<(), Error> {
        self.stats.exact_operations =
            checked_add(self.stats.exact_operations, 1, "exact symmetry operations")?;
        check_limit(
            "exact operations",
            self.stats.exact_operations,
            self.limits.max_exact_operations,
        )
    }

    pub(super) fn remaining_symbolica_limits(
        &self,
    ) -> Result<SymbolicaCoefficientMatrixLimits, Error> {
        let max_exact_operations = self
            .limits
            .max_exact_operations
            .checked_sub(self.stats.exact_operations)
            .ok_or(Error::ResourceLimit {
                resource: "exact operations",
                requested: self.stats.exact_operations,
                limit: self.limits.max_exact_operations,
            })?;
        let max_input_retained_bytes = self
            .limits
            .max_symbolica_input_retained_bytes
            .checked_sub(self.stats.symbolica_input_retained_bytes)
            .ok_or(Error::ResourceLimit {
                resource: "Symbolica input retained bytes",
                requested: self.stats.symbolica_input_retained_bytes,
                limit: self.limits.max_symbolica_input_retained_bytes,
            })?;
        let max_output_retained_bytes = self
            .limits
            .max_symbolica_output_retained_bytes
            .checked_sub(self.stats.symbolica_output_retained_bytes)
            .ok_or(Error::ResourceLimit {
                resource: "Symbolica output retained bytes",
                requested: self.stats.symbolica_output_retained_bytes,
                limit: self.limits.max_symbolica_output_retained_bytes,
            })?;
        Ok(SymbolicaCoefficientMatrixLimits {
            exact_algebra: self.limits.exact_algebra,
            max_single_matrix_entries: self.limits.max_symbolica_single_matrix_entries,
            max_live_matrix_entries: self.limits.max_symbolica_live_matrix_entries,
            max_exact_operations,
            max_input_retained_bytes,
            max_output_retained_bytes,
        })
    }

    pub(super) fn absorb_symbolica_stats(
        &mut self,
        stats: SymbolicaCoefficientMatrixStats,
    ) -> Result<(), Error> {
        let exact_operations = checked_add(
            self.stats.exact_operations,
            stats.admitted_exact_operations(),
            "exact symmetry operations",
        )?;
        check_limit(
            "exact operations",
            exact_operations,
            self.limits.max_exact_operations,
        )?;
        let symbolica_exact_operations = checked_add(
            self.stats.symbolica_exact_operations,
            stats.exact_operations(),
            "Symbolica exact operations",
        )?;
        let symbolica_admitted_exact_operations = checked_add(
            self.stats.symbolica_admitted_exact_operations,
            stats.admitted_exact_operations(),
            "admitted Symbolica exact operations",
        )?;
        let symbolica_input_retained_bytes = checked_add(
            self.stats.symbolica_input_retained_bytes,
            stats.input_retained_bytes(),
            "Symbolica input retained bytes",
        )?;
        check_limit(
            "Symbolica input retained bytes",
            symbolica_input_retained_bytes,
            self.limits.max_symbolica_input_retained_bytes,
        )?;
        let symbolica_output_retained_bytes = checked_add(
            self.stats.symbolica_output_retained_bytes,
            stats.output_retained_bytes(),
            "Symbolica output retained bytes",
        )?;
        check_limit(
            "Symbolica output retained bytes",
            symbolica_output_retained_bytes,
            self.limits.max_symbolica_output_retained_bytes,
        )?;
        let symbolica_determinant_calls = checked_add(
            self.stats.symbolica_determinant_calls,
            stats.determinant_calls(),
            "Symbolica determinant calls",
        )?;
        let symbolica_product_calls = checked_add(
            self.stats.symbolica_product_calls,
            stats.product_calls(),
            "Symbolica product calls",
        )?;
        let symbolica_transpose_calls = checked_add(
            self.stats.symbolica_transpose_calls,
            stats.transpose_calls(),
            "Symbolica transpose calls",
        )?;

        self.stats.exact_operations = exact_operations;
        self.stats.symbolica_exact_operations = symbolica_exact_operations;
        self.stats.symbolica_admitted_exact_operations = symbolica_admitted_exact_operations;
        self.stats.symbolica_largest_matrix_entries = self
            .stats
            .symbolica_largest_matrix_entries
            .max(stats.admitted_single_matrix_entries());
        self.stats.symbolica_peak_live_matrix_entries = self
            .stats
            .symbolica_peak_live_matrix_entries
            .max(stats.admitted_peak_live_entries());
        self.stats.symbolica_input_retained_bytes = symbolica_input_retained_bytes;
        self.stats.symbolica_output_retained_bytes = symbolica_output_retained_bytes;
        self.stats.symbolica_determinant_calls = symbolica_determinant_calls;
        self.stats.symbolica_product_calls = symbolica_product_calls;
        self.stats.symbolica_transpose_calls = symbolica_transpose_calls;
        Ok(())
    }

    pub(super) fn map_symbolica_matrix_error(
        &self,
        error: SymbolicaCoefficientMatrixError,
    ) -> Error {
        map_symbolica_matrix_error(error, self.limits, self.stats)
    }

    pub(super) fn retain_matrix(
        &mut self,
        matrix: &CoefficientMatrix,
        name: &'static str,
    ) -> Result<(), Error> {
        self.charge_entries(matrix.entries().len())?;
        for row in 0..matrix.rows {
            for column in 0..matrix.columns {
                if let Err(error) = self
                    .context
                    .validate_with_limits(matrix.at(row, column), self.limits.exact_algebra)
                {
                    return Err(match error {
                        ExactAlgebraError::VariableMapMismatch { .. } => {
                            Error::ForeignMapCoefficient {
                                matrix: name,
                                row,
                                column,
                            }
                        }
                        other => Error::ExactAlgebra(other),
                    });
                }
            }
        }
        Ok(())
    }

    pub(super) fn add(
        &mut self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, Error> {
        self.charge_operation()?;
        Ok(self
            .context
            .try_add(left, right, self.limits.exact_algebra)?)
    }

    pub(super) fn sub(
        &mut self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, Error> {
        self.charge_operation()?;
        Ok(self
            .context
            .try_sub(left, right, self.limits.exact_algebra)?)
    }

    pub(super) fn mul(
        &mut self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, Error> {
        self.charge_operation()?;
        Ok(self
            .context
            .try_mul(left, right, self.limits.exact_algebra)?)
    }

    pub(super) fn add_product(
        &mut self,
        accumulator: Coefficient,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, Error> {
        let product = self.mul(left, right)?;
        self.add(&accumulator, &product)
    }

    pub(super) fn equal(&mut self, left: &Coefficient, right: &Coefficient) -> Result<bool, Error> {
        Ok(self.sub(left, right)?.is_zero())
    }
}

fn map_symbolica_matrix_error(
    error: SymbolicaCoefficientMatrixError,
    limits: Limits,
    stats: Stats,
) -> Error {
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
        }) => aggregate_symbolica_resource_limit(
            "exact operations",
            stats.exact_operations,
            requested,
            limits.max_exact_operations,
        ),
        SymbolicaCoefficientMatrixError::ResourceLimit {
            resource: "coefficient matrix input retained bytes",
            requested,
            ..
        } => aggregate_symbolica_resource_limit(
            "Symbolica input retained bytes",
            stats.symbolica_input_retained_bytes,
            requested,
            limits.max_symbolica_input_retained_bytes,
        ),
        SymbolicaCoefficientMatrixError::ResourceLimit {
            resource: "coefficient matrix output retained bytes",
            requested,
            ..
        } => aggregate_symbolica_resource_limit(
            "Symbolica output retained bytes",
            stats.symbolica_output_retained_bytes,
            requested,
            limits.max_symbolica_output_retained_bytes,
        ),
        SymbolicaCoefficientMatrixError::ResourceLimit {
            resource: "single Symbolica matrix entries",
            requested,
            ..
        } => Error::ResourceLimit {
            resource: "Symbolica single matrix entries",
            requested,
            limit: limits.max_symbolica_single_matrix_entries,
        },
        SymbolicaCoefficientMatrixError::ResourceLimit {
            resource: "live Symbolica matrix entries",
            requested,
            ..
        } => Error::ResourceLimit {
            resource: "Symbolica live matrix entries",
            requested,
            limit: limits.max_symbolica_live_matrix_entries,
        },
        SymbolicaCoefficientMatrixError::ResourceLimit {
            resource,
            requested,
            limit,
        } => Error::ResourceLimit {
            resource,
            requested,
            limit,
        },
        SymbolicaCoefficientMatrixError::ResourceCountOverflow { resource }
        | SymbolicaCoefficientMatrixError::ExactAlgebra(
            ExactAlgebraError::ResourceCountOverflow { resource },
        )
        | SymbolicaCoefficientMatrixError::InvalidCoefficient {
            error: ExactAlgebraError::ResourceCountOverflow { resource },
            ..
        } => Error::ResourceCountOverflow { resource },
        SymbolicaCoefficientMatrixError::AllocationFailure {
            resource,
            requested,
        } => Error::AllocationFailure {
            resource,
            requested,
        },
        SymbolicaCoefficientMatrixError::DimensionOverflow { .. } => Error::ResourceCountOverflow {
            resource: "Symbolica matrix dimensions",
        },
        SymbolicaCoefficientMatrixError::ExactAlgebra(error)
        | SymbolicaCoefficientMatrixError::InvalidCoefficient { error, .. } => {
            Error::ExactAlgebra(error)
        }
        internal => Error::InternalSymbolicaAlgebra {
            detail: internal.to_string(),
        },
    }
}

fn aggregate_symbolica_resource_limit(
    resource: &'static str,
    current: usize,
    local_requested: usize,
    limit: usize,
) -> Error {
    match current.checked_add(local_requested) {
        Some(requested) => Error::ResourceLimit {
            resource,
            requested,
            limit,
        },
        None => Error::ResourceCountOverflow { resource },
    }
}
