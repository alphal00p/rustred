use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::prelude::Z;

use crate::algebra::Coefficient;

use super::super::budget::{
    coefficient_census, exact_operation_allocation_envelope, planned_polynomial_clone_census,
    verify_operation_result_envelope,
};
use super::super::construction::check_limit;
use super::super::error::SymbolicaAffineDenominatorError;
use super::super::model::SymbolicaAffineDenominatorCompiler;
use super::super::normalize::charge_dense_degree_box;
use super::super::projection::lift_polynomial_prefix;
use super::super::work::{
    BinaryOperation, CoefficientCensus, ExactOperationAllocationEnvelope, ExactWorkBudget,
    ProjectionAllocationBudget,
};

impl SymbolicaAffineDenominatorCompiler {
    pub(in crate::input::affine) fn validate_retained_shape(
        &self,
        coefficient: &Coefficient,
    ) -> Result<CoefficientCensus, SymbolicaAffineDenominatorError> {
        let numerator_terms = coefficient.numerator.nterms();
        let denominator_terms = coefficient.denominator.nterms();
        check_limit(
            "combined numerator terms",
            numerator_terms,
            self.limits.max_combined_polynomial_terms,
        )?;
        check_limit(
            "combined denominator terms",
            denominator_terms,
            self.limits.max_combined_polynomial_terms,
        )?;
        let all_terms = numerator_terms.checked_add(denominator_terms).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "combined polynomial terms",
            },
        )?;
        let exponent_entries = all_terms
            .checked_mul(self.combined.parameter_names().len())
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "combined exponent entries",
            })?;
        check_limit(
            "combined exponent entries",
            exponent_entries,
            self.limits.max_combined_exponent_entries,
        )?;
        let census = coefficient_census(coefficient)?;
        check_limit(
            "combined coefficient integer bits",
            census.integer_bits,
            self.limits.max_coefficient_integer_bits,
        )?;
        check_limit(
            "combined retained bytes",
            census.retained_bytes,
            self.limits.max_combined_retained_bytes,
        )?;
        Ok(census)
    }

    fn preflight_binary_shape(
        &self,
        left: &Coefficient,
        right: &Coefficient,
        operation: BinaryOperation,
        work: &mut ExactWorkBudget,
    ) -> Result<ExactOperationAllocationEnvelope, SymbolicaAffineDenominatorError> {
        charge_dense_degree_box(
            left,
            right,
            operation,
            self.combined.parameter_names().len(),
            self.limits,
            work,
        )?;
        let allocation = exact_operation_allocation_envelope(
            left,
            right,
            operation,
            self.combined.parameter_names().len(),
        )?;
        check_limit(
            "combined exact-operation numerator term envelope",
            allocation.numerator_terms,
            self.limits.max_combined_polynomial_terms,
        )?;
        check_limit(
            "combined exact-operation denominator term envelope",
            allocation.denominator_terms,
            self.limits.max_combined_polynomial_terms,
        )?;
        check_limit(
            "combined exact-operation exponent-entry envelope",
            allocation.census.exponent_entries,
            self.limits.max_combined_exponent_entries,
        )?;
        check_limit(
            "combined exact-operation integer bits",
            allocation.census.integer_bits,
            self.limits.max_coefficient_integer_bits,
        )?;
        check_limit(
            "combined exact-operation retained bytes",
            allocation.census.retained_bytes,
            self.limits.max_combined_retained_bytes,
        )?;
        Ok(allocation)
    }

    pub(in crate::input::affine) fn checked_add(
        &self,
        left: &Coefficient,
        right: &Coefficient,
        work: &mut ExactWorkBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        let allocation = self.preflight_binary_shape(left, right, BinaryOperation::Add, work)?;
        let result = self
            .combined
            .try_add(left, right, self.limits.exact_algebra)?;
        let actual = self.validate_retained_shape(&result)?;
        verify_operation_result_envelope(&result, actual, allocation)?;
        Ok(result)
    }

    pub(in crate::input::affine) fn checked_mul(
        &self,
        left: &Coefficient,
        right: &Coefficient,
        work: &mut ExactWorkBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        let allocation =
            self.preflight_binary_shape(left, right, BinaryOperation::Multiply, work)?;
        let result = self
            .combined
            .try_mul(left, right, self.limits.exact_algebra)?;
        let actual = self.validate_retained_shape(&result)?;
        verify_operation_result_envelope(&result, actual, allocation)?;
        Ok(result)
    }

    pub(in crate::input::affine) fn checked_div(
        &self,
        numerator: &Coefficient,
        denominator: &Coefficient,
        work: &mut ExactWorkBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        let allocation =
            self.preflight_binary_shape(numerator, denominator, BinaryOperation::Divide, work)?;
        let result = self
            .combined
            .try_div(numerator, denominator, self.limits.exact_algebra)?;
        let actual = self.validate_retained_shape(&result)?;
        verify_operation_result_envelope(&result, actual, allocation)?;
        Ok(result)
    }

    pub(in crate::input::affine) fn lift_base_coefficient(
        &self,
        coefficient: &Coefficient,
        projection_work: &mut ProjectionAllocationBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        let combined_variables = self.combined.parameter_names().len();
        projection_work.charge(
            planned_polynomial_clone_census(&coefficient.numerator, combined_variables)?,
            self.limits,
            "aggregate lifted numerator terms",
        )?;
        projection_work.charge(
            planned_polynomial_clone_census(&coefficient.denominator, combined_variables)?,
            self.limits,
            "aggregate lifted denominator terms",
        )?;
        let numerator = lift_polynomial_prefix(
            &coefficient.numerator,
            &self.combined.template().numerator,
            self.base_count(),
            self.limits.max_combined_exponent_entries,
        )?;
        let denominator = lift_polynomial_prefix(
            &coefficient.denominator,
            &self.combined.template().denominator,
            self.base_count(),
            self.limits.max_combined_exponent_entries,
        )?;
        let lifted = Coefficient::from_num_den(numerator, denominator, &Z, false);
        self.combined
            .validate_with_limits(&lifted, self.limits.exact_algebra)?;
        self.validate_retained_shape(&lifted)?;
        Ok(lifted)
    }
}
