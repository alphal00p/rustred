//! Shared state and checked scalar operations for the Symbolica field adapter.

use std::cell::RefCell;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use symbolica::prelude::*;

use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraError};

use super::unwind::abort_checked_field;
use crate::algebra::matrix::{SymbolicaCoefficientMatrixLimits, SymbolicaCoefficientMatrixStats};

#[derive(Clone, Copy, Debug)]
enum AtomicOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Negate,
}

#[derive(Debug, Default)]
pub(in crate::algebra::matrix) struct CheckedFieldState {
    pub(in crate::algebra::matrix) stats: SymbolicaCoefficientMatrixStats,
}

#[derive(Clone)]
pub(in crate::algebra::matrix) struct CheckedCoefficientField<'context> {
    pub(in crate::algebra::matrix) context: &'context CoefficientContext,
    pub(super) inner: RationalPolynomialField<IntegerRing, u16>,
    pub(in crate::algebra::matrix) limits: SymbolicaCoefficientMatrixLimits,
    pub(in crate::algebra::matrix) state: Rc<RefCell<CheckedFieldState>>,
}

impl fmt::Debug for CheckedCoefficientField<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedCoefficientField")
            .field("variables", &self.context.parameter_names().len())
            .finish_non_exhaustive()
    }
}

impl fmt::Display for CheckedCoefficientField<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("authenticated RustRed coefficient field")
    }
}

impl PartialEq for CheckedCoefficientField<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.context.variables() == other.context.variables()
    }
}

impl Eq for CheckedCoefficientField<'_> {}

impl Hash for CheckedCoefficientField<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.context.variables().hash(state);
    }
}

impl<'context> CheckedCoefficientField<'context> {
    pub(in crate::algebra::matrix) fn new(
        context: &'context CoefficientContext,
        limits: SymbolicaCoefficientMatrixLimits,
        admitted_single_matrix_entries: usize,
        admitted_peak_live_entries: usize,
        admitted_exact_operations: usize,
    ) -> Self {
        let stats = SymbolicaCoefficientMatrixStats {
            admitted_single_matrix_entries,
            admitted_peak_live_entries,
            admitted_exact_operations,
            ..SymbolicaCoefficientMatrixStats::default()
        };
        Self {
            context,
            inner: RationalPolynomialField::new(Z),
            limits,
            state: Rc::new(RefCell::new(CheckedFieldState { stats })),
        }
    }

    fn charge_operation(&self, operation: AtomicOperation) {
        let result = {
            let mut state = self.state.borrow_mut();
            let requested = state.stats.exact_operations.checked_add(1).ok_or(
                ExactAlgebraError::ResourceCountOverflow {
                    resource: "Symbolica coefficient matrix exact operations",
                },
            );
            match requested {
                Ok(requested) if requested <= self.limits.max_exact_operations => {
                    let operation_requested = match operation {
                        AtomicOperation::Add => state.stats.additions.checked_add(1),
                        AtomicOperation::Subtract => state.stats.subtractions.checked_add(1),
                        AtomicOperation::Multiply => state.stats.multiplications.checked_add(1),
                        AtomicOperation::Divide => state.stats.divisions.checked_add(1),
                        AtomicOperation::Negate => state.stats.negations.checked_add(1),
                    }
                    .ok_or(ExactAlgebraError::ResourceCountOverflow {
                        resource: "Symbolica coefficient matrix operation census",
                    });
                    let operation_requested =
                        operation_requested.unwrap_or_else(|error| abort_checked_field(error));
                    state.stats.exact_operations = requested;
                    match operation {
                        AtomicOperation::Add => state.stats.additions = operation_requested,
                        AtomicOperation::Subtract => state.stats.subtractions = operation_requested,
                        AtomicOperation::Multiply => {
                            state.stats.multiplications = operation_requested
                        }
                        AtomicOperation::Divide => state.stats.divisions = operation_requested,
                        AtomicOperation::Negate => state.stats.negations = operation_requested,
                    }
                    Ok(())
                }
                Ok(requested) => Err(ExactAlgebraError::ResourceLimit {
                    resource: "Symbolica coefficient matrix exact operations",
                    requested,
                    limit: self.limits.max_exact_operations,
                }),
                Err(error) => Err(error),
            }
        };
        if let Err(error) = result {
            abort_checked_field(error);
        }
    }

    pub(super) fn charge_counter(
        &self,
        select: impl FnOnce(&mut SymbolicaCoefficientMatrixStats) -> &mut usize,
    ) {
        let error = {
            let mut state = self.state.borrow_mut();
            let counter = select(&mut state.stats);
            match counter.checked_add(1) {
                Some(next) => {
                    *counter = next;
                    None
                }
                None => Some(ExactAlgebraError::ResourceCountOverflow {
                    resource: "Symbolica coefficient matrix field calls",
                }),
            }
        };
        if let Some(error) = error {
            abort_checked_field(error);
        }
    }

    fn finish(&self, result: Result<Coefficient, ExactAlgebraError>) -> Coefficient {
        match result {
            Ok(value) => value,
            Err(error) => abort_checked_field(error),
        }
    }

    pub(super) fn finish_raw(&self, value: Coefficient) -> Coefficient {
        if let Err(error) = self
            .context
            .validate_with_limits(&value, self.limits.exact_algebra)
        {
            abort_checked_field(error);
        }
        value
    }

    pub(super) fn add_checked(&self, left: &Coefficient, right: &Coefficient) -> Coefficient {
        self.charge_operation(AtomicOperation::Add);
        self.finish(self.context.try_add(left, right, self.limits.exact_algebra))
    }

    pub(super) fn sub_checked(&self, left: &Coefficient, right: &Coefficient) -> Coefficient {
        self.charge_operation(AtomicOperation::Subtract);
        self.finish(self.context.try_sub(left, right, self.limits.exact_algebra))
    }

    pub(super) fn mul_checked(&self, left: &Coefficient, right: &Coefficient) -> Coefficient {
        self.charge_operation(AtomicOperation::Multiply);
        self.finish(self.context.try_mul(left, right, self.limits.exact_algebra))
    }

    pub(super) fn div_checked(
        &self,
        numerator: &Coefficient,
        denominator: &Coefficient,
    ) -> Coefficient {
        self.charge_operation(AtomicOperation::Divide);
        self.finish(
            self.context
                .try_div(numerator, denominator, self.limits.exact_algebra),
        )
    }

    pub(super) fn neg_checked(&self, value: &Coefficient) -> Coefficient {
        self.charge_operation(AtomicOperation::Negate);
        self.finish(self.context.try_neg(value, self.limits.exact_algebra))
    }

    pub(super) fn contextual_integer(&self, value: Integer) -> Coefficient {
        self.finish_raw(self.context.template().numerator.constant(value).into())
    }
}
