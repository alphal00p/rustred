use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::prelude::AtomView;

use crate::algebra::Coefficient;

use super::super::budget::{compiled_retained_byte_bound, retained_variable_map_arc_bytes};
use super::super::construction::{check_limit, checked_atom_shape, maximum_combined_symbol_bytes};
use super::super::error::SymbolicaAffineDenominatorError;
use super::super::evaluate::CheckedEvaluator;
use super::super::model::{CompiledSymbolicaAffineDenominator, SymbolicaAffineDenominatorCompiler};
use super::super::normalize::{
    normalized_expression_census, normalized_expression_render_byte_bound,
};
use super::super::projection::{coefficient_contains_momentum, reject_momentum_denominator};

impl SymbolicaAffineDenominatorCompiler {
    /// Compile an already parsed Atom on the authenticated combined map.
    pub fn compile(
        &self,
        source: AtomView<'_>,
    ) -> Result<CompiledSymbolicaAffineDenominator, SymbolicaAffineDenominatorError> {
        catch_unwind(AssertUnwindSafe(|| self.compile_inner(source))).map_err(|_| {
            SymbolicaAffineDenominatorError::SymbolicaPanic {
                stage: "checked expression evaluation",
            }
        })?
    }

    fn compile_inner(
        &self,
        source: AtomView<'_>,
    ) -> Result<CompiledSymbolicaAffineDenominator, SymbolicaAffineDenominatorError> {
        let input_bytes = source.get_byte_size();
        check_limit(
            "input expression bytes",
            input_bytes,
            self.limits.max_input_bytes,
        )?;
        checked_atom_shape(source, self.limits)?;
        let fixed_retained_bytes = compiled_retained_byte_bound(input_bytes, 0, 0, 0)?;
        check_limit(
            "compiled fixed retained bytes",
            fixed_retained_bytes,
            self.limits.max_compiled_retained_bytes,
        )?;

        let mut evaluator = CheckedEvaluator::new(self);
        let evaluated = evaluator.evaluate(source, true)?;
        self.combined
            .validate_with_limits(&evaluated, self.limits.exact_algebra)?;
        self.validate_retained_shape(&evaluated)?;
        reject_momentum_denominator(&evaluated, self.base_count())?;

        // Bound the Atom that rational-polynomial conversion will construct
        // before asking Symbolica to allocate it.
        let normalized_census = normalized_expression_census(&evaluated)?;
        check_limit(
            "normalized expression nodes",
            normalized_census.nodes,
            self.limits.max_normalized_expression_nodes,
        )?;
        check_limit(
            "normalized expression integer bits",
            normalized_census.integer_bits,
            self.limits.max_normalized_expression_integer_bits,
        )?;
        let maximum_symbol_bytes = maximum_combined_symbol_bytes(&self.combined)?;
        let normalized_render_byte_bound =
            normalized_expression_render_byte_bound(normalized_census, maximum_symbol_bytes)?;
        if normalized_render_byte_bound > self.limits.max_normalized_expression_bytes {
            return Err(
                SymbolicaAffineDenominatorError::NormalizedExpressionTooLarge {
                    requested: normalized_render_byte_bound,
                    limit: self.limits.max_normalized_expression_bytes,
                },
            );
        }
        let normalized_expression = evaluated.to_expression();
        let normalized_expression_bytes = normalized_expression.as_view().get_byte_size();
        if normalized_expression_bytes > self.limits.max_normalized_expression_bytes {
            return Err(
                SymbolicaAffineDenominatorError::NormalizedExpressionTooLarge {
                    requested: normalized_expression_bytes,
                    limit: self.limits.max_normalized_expression_bytes,
                },
            );
        }

        let (affine_denominator, projection_stats) = self.project_affine_denominator(
            &evaluated,
            &mut evaluator.work,
            &mut evaluator.projection_work,
        )?;
        let variable_map_arc_bytes = retained_variable_map_arc_bytes(
            std::iter::once(affine_denominator.constant())
                .chain(affine_denominator.coefficients().iter()),
        )?;
        let compiled_retained_bytes = compiled_retained_byte_bound(
            input_bytes,
            normalized_expression_bytes,
            projection_stats.projected_retained_bytes,
            variable_map_arc_bytes,
        )?;
        check_limit(
            "compiled retained bytes",
            compiled_retained_bytes,
            self.limits.max_compiled_retained_bytes,
        )?;
        Ok(CompiledSymbolicaAffineDenominator {
            source: source.to_owned(),
            normalized_expression,
            affine_denominator,
        })
    }

    /// Evaluate an Atom in the same checked parser, proving it is momentum free.
    pub fn parse_base_coefficient(
        &self,
        source: AtomView<'_>,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        catch_unwind(AssertUnwindSafe(|| {
            let input_bytes = source.get_byte_size();
            check_limit(
                "input expression bytes",
                input_bytes,
                self.limits.max_input_bytes,
            )?;
            checked_atom_shape(source, self.limits)?;
            let mut evaluator = CheckedEvaluator::new(self);
            let value = evaluator.evaluate(source, false)?;
            self.combined
                .validate_with_limits(&value, self.limits.exact_algebra)?;
            self.validate_retained_shape(&value)?;
            if coefficient_contains_momentum(&value, self.base_count())? {
                return Err(SymbolicaAffineDenominatorError::BaseCoefficientContainsMomentum);
            }
            self.project_complete_coefficient(
                &value,
                &mut evaluator.work,
                &mut evaluator.projection_work,
            )
        }))
        .map_err(|_| SymbolicaAffineDenominatorError::SymbolicaPanic {
            stage: "base-coefficient evaluation",
        })?
    }

    pub(in crate::input::affine) fn base_count(&self) -> usize {
        self.coefficients.parameter_names().len()
    }
}
