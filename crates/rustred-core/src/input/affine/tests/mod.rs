use super::*;

use symbolica::prelude::{Integer, try_parse};

use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraError};
use crate::family::ScalarProductCoordinate;

use super::budget::{
    coefficient_census, compiled_retained_byte_bound, exact_operation_allocation_envelope,
    multiply_census, planned_unit_coefficient_census, polynomial_census,
    retained_variable_map_arc_bytes, verify_operation_result_envelope,
};
use super::construction::{checked_atom_shape, maximum_combined_symbol_bytes};
use super::evaluate::CheckedEvaluator;
use super::normalize::{normalized_expression_census, normalized_expression_render_byte_bound};
use super::work::{BinaryOperation, ExactWorkBudget, ProjectionAllocationBudget};

impl SymbolicaAffineDenominatorCompiler {
    fn compile_expression(
        &self,
        expression: &str,
    ) -> Result<CompiledSymbolicaAffineDenominator, SymbolicaAffineDenominatorError> {
        let source = try_parse!(expression, default_namespace = RUSTRED_NAMESPACE)
            .map_err(SymbolicaAffineDenominatorError::Parse)?;
        self.compile(source.as_view())
    }

    fn parse_base_expression(
        &self,
        expression: &str,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        let source = try_parse!(expression, default_namespace = RUSTRED_NAMESPACE)
            .map_err(SymbolicaAffineDenominatorError::Parse)?;
        self.parse_base_coefficient(source.as_view())
    }

    fn test_with_limits(&self, limits: SymbolicaAffineDenominatorLimits) -> Self {
        Self {
            coefficients: self.coefficients.clone(),
            loop_momenta: self.loop_momenta.clone(),
            external_momenta: self.external_momenta.clone(),
            external_gram: self.external_gram.clone(),
            combined: self.combined.clone(),
            symbol_positions: self.symbol_positions.clone(),
            scalar_product: self.scalar_product,
            coordinates: self.coordinates.clone(),
            limits,
        }
    }

    fn test_clone(&self) -> Self {
        self.test_with_limits(self.limits)
    }

    const fn test_limits(&self) -> SymbolicaAffineDenominatorLimits {
        self.limits
    }

    const fn test_coefficient_context(&self) -> &CoefficientContext {
        &self.coefficients
    }
}

fn compiler(
    parameters: &[&str],
    loops: &[&str],
    externals: &[&str],
    gram: &[&[&str]],
) -> SymbolicaAffineDenominatorCompiler {
    let coefficients = CoefficientContext::new(parameters.iter().copied());
    let gram = gram
        .iter()
        .map(|row| {
            row.iter()
                .map(|entry| coefficients.coefficient_fixture(entry))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    SymbolicaAffineDenominatorCompiler::try_new(
        coefficients,
        loops.iter().map(|name| (*name).to_owned()).collect(),
        externals.iter().map(|name| (*name).to_owned()).collect(),
        gram,
        SymbolicaAffineDenominatorLimits::default(),
    )
    .unwrap()
}

fn assert_coefficients(
    compiler: &SymbolicaAffineDenominatorCompiler,
    compiled: &CompiledSymbolicaAffineDenominator,
    expected_constant: &str,
    expected_coefficients: &[&str],
) {
    let context = &compiler.coefficients;
    assert_eq!(
        compiled.affine_denominator().constant(),
        &context.coefficient_fixture(expected_constant)
    );
    assert_eq!(
        compiled.affine_denominator().coefficients().len(),
        expected_coefficients.len()
    );
    for (actual, expected) in compiled
        .affine_denominator()
        .coefficients()
        .iter()
        .zip(expected_coefficients)
    {
        assert_eq!(actual, &context.coefficient_fixture(expected));
    }
}

fn checked_test_operation(
    compiler: &SymbolicaAffineDenominatorCompiler,
    left: &Coefficient,
    right: &Coefficient,
    operation: BinaryOperation,
) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
    let mut work = ExactWorkBudget::default();
    match operation {
        BinaryOperation::Add => compiler.checked_add(left, right, &mut work),
        BinaryOperation::Multiply => compiler.checked_mul(left, right, &mut work),
        BinaryOperation::Divide => compiler.checked_div(left, right, &mut work),
    }
}

mod resources;
mod semantics;
