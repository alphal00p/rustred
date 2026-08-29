//! Conservative admission and output census for Symbolica field powers.

use symbolica::prelude::*;

use crate::algebra::matrix::SymbolicaCoefficientMatrixError;
use crate::algebra::matrix::admission::{check_limit, checked_add, coefficient_retained_bytes};
use crate::algebra::{
    Coefficient, CoefficientPolynomialPart, ExactAlgebraError, ExactAlgebraLimits,
    ExactAlgebraOperation,
};

use super::state::CheckedCoefficientField;
use super::unwind::{abort_checked_field, abort_checked_matrix};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PolynomialPowerAdmission {
    output_terms: usize,
    max_term_operations: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CoefficientPowerAdmission {
    numerator: PolynomialPowerAdmission,
    denominator: PolynomialPowerAdmission,
}

impl CoefficientPowerAdmission {
    fn max_term_operations(self) -> usize {
        self.numerator
            .max_term_operations
            .max(self.denominator.max_term_operations)
    }
}

fn polynomial_power_resource(part: CoefficientPolynomialPart, output: bool) -> &'static str {
    match (part, output) {
        (CoefficientPolynomialPart::Numerator, false) => {
            "exact coefficient power numerator term operations"
        }
        (CoefficientPolynomialPart::Denominator, false) => {
            "exact coefficient power denominator term operations"
        }
        (CoefficientPolynomialPart::Numerator, true) => {
            "exact coefficient power numerator output terms"
        }
        (CoefficientPolynomialPart::Denominator, true) => {
            "exact coefficient power denominator output terms"
        }
    }
}

fn polynomial_power_degree_box(
    polynomial: &MultivariatePolynomial<IntegerRing, u16>,
    exponent: u64,
    operation: ExactAlgebraOperation,
    resource: &'static str,
    limit: usize,
) -> Result<usize, ExactAlgebraError> {
    let mut terms = 1usize;
    for variable in 0..polynomial.variables.len() {
        let degree = u64::from(polynomial.degree(variable))
            .checked_mul(exponent)
            .ok_or(ExactAlgebraError::ExponentArithmeticOverflow {
                operation,
                variable,
                width: 64,
            })?;
        let width = degree
            .checked_add(1)
            .and_then(|width| usize::try_from(width).ok())
            .ok_or(ExactAlgebraError::ResourceCountOverflow { resource })?;
        terms = terms
            .checked_mul(width)
            .ok_or(ExactAlgebraError::ResourceCountOverflow { resource })?;
        if terms > limit {
            return Err(ExactAlgebraError::ResourceLimit {
                resource,
                requested: terms,
                limit,
            });
        }
    }
    if terms > limit {
        Err(ExactAlgebraError::ResourceLimit {
            resource,
            requested: terms,
            limit,
        })
    } else {
        Ok(terms)
    }
}

fn polynomial_power_admission(
    polynomial: &MultivariatePolynomial<IntegerRing, u16>,
    exponent: u64,
    part: CoefficientPolynomialPart,
    limits: ExactAlgebraLimits,
) -> Result<PolynomialPowerAdmission, ExactAlgebraError> {
    if exponent == 0 {
        return Ok(PolynomialPowerAdmission {
            output_terms: 1,
            max_term_operations: 0,
        });
    }
    if polynomial.is_zero() {
        return Ok(PolynomialPowerAdmission {
            output_terms: 0,
            max_term_operations: 0,
        });
    }

    let output_resource = polynomial_power_resource(part, true);
    let operation_resource = polynomial_power_resource(part, false);
    // Symbolica's native rational-polynomial power performs repeated
    // multiplication. Cross-GCD quotients can be denser than the sparse
    // inputs, so use the componentwise degree box rather than nterms^e.
    let output_terms = polynomial_power_degree_box(
        polynomial,
        exponent,
        ExactAlgebraOperation::Power,
        output_resource,
        limits.max_polynomial_terms,
    )?;
    let previous_terms = polynomial_power_degree_box(
        polynomial,
        exponent - 1,
        ExactAlgebraOperation::Power,
        operation_resource,
        limits.max_term_operations,
    )?;
    let base_terms = polynomial_power_degree_box(
        polynomial,
        1,
        ExactAlgebraOperation::Power,
        operation_resource,
        limits.max_term_operations,
    )?;
    let max_term_operations =
        previous_terms
            .checked_mul(base_terms)
            .ok_or(ExactAlgebraError::ResourceCountOverflow {
                resource: operation_resource,
            })?;
    if max_term_operations > limits.max_term_operations {
        return Err(ExactAlgebraError::ResourceLimit {
            resource: operation_resource,
            requested: max_term_operations,
            limit: limits.max_term_operations,
        });
    }
    Ok(PolynomialPowerAdmission {
        output_terms,
        max_term_operations,
    })
}

impl CheckedCoefficientField<'_> {
    pub(super) fn preflight_power_admission(
        &self,
        base: &Coefficient,
        exponent: u64,
    ) -> CoefficientPowerAdmission {
        if let Err(error) =
            self.context
                .preflight_power_with_limits(base, exponent, self.limits.exact_algebra)
        {
            abort_checked_field(error);
        }
        if exponent > u64::from(u32::MAX) {
            abort_checked_matrix(SymbolicaCoefficientMatrixError::NativePowerExponentLimit {
                requested: exponent,
                limit: u32::MAX,
            });
        }
        let numerator = polynomial_power_admission(
            &base.numerator,
            exponent,
            CoefficientPolynomialPart::Numerator,
            self.limits.exact_algebra,
        )
        .unwrap_or_else(|error| abort_checked_field(error));
        let denominator = polynomial_power_admission(
            &base.denominator,
            exponent,
            CoefficientPolynomialPart::Denominator,
            self.limits.exact_algebra,
        )
        .unwrap_or_else(|error| abort_checked_field(error));
        CoefficientPowerAdmission {
            numerator,
            denominator,
        }
    }

    pub(super) fn charge_power_operations(
        &self,
        exponent: u64,
        admission: CoefficientPowerAdmission,
    ) {
        if exponent > u64::from(u32::MAX) {
            abort_checked_matrix(SymbolicaCoefficientMatrixError::NativePowerExponentLimit {
                requested: exponent,
                limit: u32::MAX,
            });
        }
        let operations = usize::try_from(exponent).unwrap_or_else(|_| {
            abort_checked_field(ExactAlgebraError::ResourceCountOverflow {
                resource: "Symbolica coefficient power operations",
            })
        });

        let result = {
            let mut state = self.state.borrow_mut();
            let exact_operations = state.stats.exact_operations.checked_add(operations).ok_or(
                ExactAlgebraError::ResourceCountOverflow {
                    resource: "Symbolica coefficient matrix exact operations",
                },
            );
            let multiplications = state.stats.multiplications.checked_add(operations).ok_or(
                ExactAlgebraError::ResourceCountOverflow {
                    resource: "Symbolica coefficient matrix operation census",
                },
            );
            match (exact_operations, multiplications) {
                (Ok(exact_operations), Ok(multiplications))
                    if exact_operations <= self.limits.max_exact_operations =>
                {
                    state.stats.exact_operations = exact_operations;
                    state.stats.multiplications = multiplications;
                    state.stats.admitted_power_exponent =
                        state.stats.admitted_power_exponent.max(exponent);
                    state.stats.admitted_power_term_operations = state
                        .stats
                        .admitted_power_term_operations
                        .max(admission.max_term_operations());
                    state.stats.admitted_power_numerator_terms = state
                        .stats
                        .admitted_power_numerator_terms
                        .max(admission.numerator.output_terms);
                    state.stats.admitted_power_denominator_terms = state
                        .stats
                        .admitted_power_denominator_terms
                        .max(admission.denominator.output_terms);
                    Ok(())
                }
                (Ok(exact_operations), Ok(_)) => Err(ExactAlgebraError::ResourceLimit {
                    resource: "Symbolica coefficient matrix exact operations",
                    requested: exact_operations,
                    limit: self.limits.max_exact_operations,
                }),
                (Err(error), _) | (_, Err(error)) => Err(error),
            }
        };
        if let Err(error) = result {
            abort_checked_field(error);
        }
    }

    pub(super) fn finish_power_raw(
        &self,
        value: Coefficient,
        admission: CoefficientPowerAdmission,
    ) -> Coefficient {
        let result = (|| {
            self.context
                .validate_with_limits(&value, self.limits.exact_algebra)
                .map_err(SymbolicaCoefficientMatrixError::ExactAlgebra)?;
            let numerator_terms = value.numerator.nterms();
            let denominator_terms = value.denominator.nterms();
            check_limit(
                polynomial_power_resource(CoefficientPolynomialPart::Numerator, true),
                numerator_terms,
                admission.numerator.output_terms,
            )?;
            check_limit(
                polynomial_power_resource(CoefficientPolynomialPart::Denominator, true),
                denominator_terms,
                admission.denominator.output_terms,
            )?;
            let retained_bytes = coefficient_retained_bytes(&value)?;
            let mut state = self.state.borrow_mut();
            let output_retained_bytes = checked_add(
                "coefficient matrix output retained bytes",
                state.stats.output_retained_bytes,
                retained_bytes,
            )?;
            check_limit(
                "coefficient matrix output retained bytes",
                output_retained_bytes,
                self.limits.max_output_retained_bytes,
            )?;
            let authenticated_entries = checked_add(
                "authenticated Symbolica matrix entries",
                state.stats.authenticated_entries,
                1,
            )?;
            let output_entries = checked_add(
                "coefficient matrix output entries",
                state.stats.output_entries,
                1,
            )?;
            state.stats.output_retained_bytes = output_retained_bytes;
            state.stats.authenticated_entries = authenticated_entries;
            state.stats.output_entries = output_entries;
            state.stats.output_power_numerator_terms = state
                .stats
                .output_power_numerator_terms
                .max(numerator_terms);
            state.stats.output_power_denominator_terms = state
                .stats
                .output_power_denominator_terms
                .max(denominator_terms);
            Ok::<(), SymbolicaCoefficientMatrixError>(())
        })();
        if let Err(error) = result {
            abort_checked_matrix(error);
        }
        value
    }
}
