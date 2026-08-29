use crate::algebra::{Coefficient, CoefficientPolynomial};

use super::super::budget::{
    coefficient_census, exact_operation_allocation_envelope, planned_coefficient_clone_census,
    planned_polynomial_clone_census, verify_operation_result_envelope,
};
use super::super::construction::check_limit;
use super::super::error::SymbolicaAffineDenominatorError;
use super::super::model::SymbolicaAffineDenominatorCompiler;
use super::super::normalize::charge_dense_degree_box;
use super::super::work::{BinaryOperation, ExactWorkBudget, ProjectionAllocationBudget};

impl SymbolicaAffineDenominatorCompiler {
    pub(in crate::input::affine) fn project_complete_coefficient(
        &self,
        coefficient: &Coefficient,
        work: &mut ExactWorkBudget,
        projection_work: &mut ProjectionAllocationBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        projection_work.charge(
            planned_polynomial_clone_census(&coefficient.numerator, self.base_count())?,
            self.limits,
            "aggregate projected complete numerator terms",
        )?;
        projection_work.charge(
            planned_polynomial_clone_census(&coefficient.denominator, self.base_count())?,
            self.limits,
            "aggregate projected complete denominator terms",
        )?;
        let numerator = project_polynomial_prefix(
            &coefficient.numerator,
            &self.coefficients.template().numerator,
            self.base_count(),
            self.limits.max_combined_exponent_entries,
        )?;
        let denominator = project_polynomial_prefix(
            &coefficient.denominator,
            &self.coefficients.template().denominator,
            self.base_count(),
            self.limits.max_combined_exponent_entries,
        )?;
        self.projected_rational(numerator, denominator, work, projection_work)
    }

    pub(in crate::input::affine) fn projected_rational(
        &self,
        numerator: CoefficientPolynomial,
        denominator: CoefficientPolynomial,
        work: &mut ExactWorkBudget,
        projection_work: &mut ProjectionAllocationBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        if numerator.is_zero() {
            projection_work.charge(
                planned_coefficient_clone_census(self.coefficients.template(), self.base_count())?,
                self.limits,
                "aggregate projected zero coefficient terms",
            )?;
            return Ok(self.coefficients.zero());
        }
        let numerator: Coefficient = numerator.into();
        let denominator: Coefficient = denominator.into();
        self.projected_checked_div(&numerator, &denominator, work, projection_work)
    }

    fn validate_projected_coefficient(
        &self,
        coefficient: &Coefficient,
    ) -> Result<(), SymbolicaAffineDenominatorError> {
        self.coefficients
            .validate_with_limits(coefficient, self.limits.exact_algebra)?;
        let census = coefficient_census(coefficient)?;
        check_limit(
            "one projected coefficient integer bits",
            census.integer_bits,
            self.limits.max_projected_integer_bits,
        )
    }

    pub(in crate::input::affine) fn projected_checked_add(
        &self,
        left: &Coefficient,
        right: &Coefficient,
        work: &mut ExactWorkBudget,
        projection_work: &mut ProjectionAllocationBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        charge_dense_degree_box(
            left,
            right,
            BinaryOperation::Add,
            self.base_count(),
            self.limits,
            work,
        )?;
        let allocation = exact_operation_allocation_envelope(
            left,
            right,
            BinaryOperation::Add,
            self.base_count(),
        )?;
        projection_work.charge(
            allocation.census,
            self.limits,
            "aggregate projected exact-operation terms",
        )?;
        let result = self
            .coefficients
            .try_add(left, right, self.limits.exact_algebra)?;
        self.validate_projected_coefficient(&result)?;
        verify_operation_result_envelope(&result, coefficient_census(&result)?, allocation)?;
        Ok(result)
    }

    pub(in crate::input::affine) fn projected_checked_mul(
        &self,
        left: &Coefficient,
        right: &Coefficient,
        work: &mut ExactWorkBudget,
        projection_work: &mut ProjectionAllocationBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        charge_dense_degree_box(
            left,
            right,
            BinaryOperation::Multiply,
            self.base_count(),
            self.limits,
            work,
        )?;
        let allocation = exact_operation_allocation_envelope(
            left,
            right,
            BinaryOperation::Multiply,
            self.base_count(),
        )?;
        projection_work.charge(
            allocation.census,
            self.limits,
            "aggregate projected exact-operation terms",
        )?;
        let result = self
            .coefficients
            .try_mul(left, right, self.limits.exact_algebra)?;
        self.validate_projected_coefficient(&result)?;
        verify_operation_result_envelope(&result, coefficient_census(&result)?, allocation)?;
        Ok(result)
    }

    pub(in crate::input::affine) fn projected_checked_div(
        &self,
        numerator: &Coefficient,
        denominator: &Coefficient,
        work: &mut ExactWorkBudget,
        projection_work: &mut ProjectionAllocationBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        charge_dense_degree_box(
            numerator,
            denominator,
            BinaryOperation::Divide,
            self.base_count(),
            self.limits,
            work,
        )?;
        let allocation = exact_operation_allocation_envelope(
            numerator,
            denominator,
            BinaryOperation::Divide,
            self.base_count(),
        )?;
        projection_work.charge(
            allocation.census,
            self.limits,
            "aggregate projected exact-operation terms",
        )?;
        let result =
            self.coefficients
                .try_div(numerator, denominator, self.limits.exact_algebra)?;
        self.validate_projected_coefficient(&result)?;
        verify_operation_result_envelope(&result, coefficient_census(&result)?, allocation)?;
        Ok(result)
    }
}

pub(in crate::input::affine) fn project_polynomial_prefix(
    source: &CoefficientPolynomial,
    target_template: &CoefficientPolynomial,
    retained: usize,
    max_exponent_entries: usize,
) -> Result<CoefficientPolynomial, SymbolicaAffineDenominatorError> {
    if target_template.variables.len() != retained {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "base projection target has the wrong variable count",
            },
        );
    }
    let expected_source_entries = source.nterms().checked_mul(source.variables.len()).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "source projection exponent entries",
        },
    )?;
    if source.exponents.len() != expected_source_entries {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "source projection polynomial has a malformed exponent layout",
            },
        );
    }
    let target_entries = source.nterms().checked_mul(retained).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "target projection exponent entries",
        },
    )?;
    check_limit(
        "target projection exponent entries",
        target_entries,
        max_exponent_entries,
    )?;
    let mut target = target_template.zero_with_capacity(source.nterms());
    for (integer, exponents) in source.coefficients.iter().zip(source.exponents_iter()) {
        if exponents.len() != source.variables.len() || exponents.len() < retained {
            return Err(
                SymbolicaAffineDenominatorError::InternalVerificationFailure {
                    detail: "combined polynomial exponent row is too short",
                },
            );
        }
        if exponents[retained..].iter().any(|exponent| *exponent != 0) {
            return Err(SymbolicaAffineDenominatorError::BaseCoefficientContainsMomentum);
        }
        target.append_monomial(integer.clone(), &exponents[..retained]);
    }
    Ok(target)
}

pub(in crate::input::affine) fn lift_polynomial_prefix(
    source: &CoefficientPolynomial,
    target_template: &CoefficientPolynomial,
    retained: usize,
    max_exponent_entries: usize,
) -> Result<CoefficientPolynomial, SymbolicaAffineDenominatorError> {
    if source.variables.len() != retained || target_template.variables.len() < retained {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "base lift uses incompatible variable maps",
            },
        );
    }
    let target_variables = target_template.variables.len();
    let target_entries = source.nterms().checked_mul(target_variables).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "base lift exponent entries",
        },
    )?;
    check_limit(
        "base lift exponent entries",
        target_entries,
        max_exponent_entries,
    )?;
    let mut exponent_row = Vec::new();
    exponent_row
        .try_reserve_exact(target_variables)
        .map_err(|_| SymbolicaAffineDenominatorError::AllocationFailure {
            resource: "base lift exponent row",
            requested: target_variables,
        })?;
    exponent_row.resize(target_variables, 0u16);
    let mut target = target_template.zero_with_capacity(source.nterms());
    for (integer, exponents) in source.coefficients.iter().zip(source.exponents_iter()) {
        if exponents.len() != retained {
            return Err(
                SymbolicaAffineDenominatorError::InternalVerificationFailure {
                    detail: "base lift source exponent row has the wrong width",
                },
            );
        }
        exponent_row[..retained].copy_from_slice(exponents);
        target.append_monomial(integer.clone(), &exponent_row);
    }
    Ok(target)
}
