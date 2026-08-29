use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::algebra::{Coefficient, CoefficientContext};

use super::super::budget::planned_coefficient_clone_census;
use super::super::error::SymbolicaAffineDenominatorError;
use super::super::limits::SymbolicaAffineDenominatorLimits;
use super::super::model::SymbolicaAffineDenominatorCompiler;
use super::check_limit;
use super::coordinates::{scalar_product_coordinate_count, scalar_product_coordinates};
use super::gram::validate_external_gram;
use super::symbols::{
    authenticate_combined_symbols, reserved_scalar_product, validate_declared_labels,
};

impl SymbolicaAffineDenominatorCompiler {
    /// Authenticate one already-normalized ordered declaration.
    ///
    /// The base parameter list may have been explicit or inferred by a caller;
    /// this layer deliberately does not distinguish those provenance paths.
    pub fn try_new(
        coefficients: CoefficientContext,
        loop_momenta: Vec<String>,
        external_momenta: Vec<String>,
        external_gram: Vec<Vec<Coefficient>>,
        limits: SymbolicaAffineDenominatorLimits,
    ) -> Result<Self, SymbolicaAffineDenominatorError> {
        catch_unwind(AssertUnwindSafe(|| {
            Self::try_new_inner(
                coefficients,
                loop_momenta,
                external_momenta,
                external_gram,
                limits,
            )
        }))
        .map_err(|_| SymbolicaAffineDenominatorError::SymbolicaPanic {
            stage: "compiler construction",
        })?
    }

    fn try_new_inner(
        coefficients: CoefficientContext,
        loop_momenta: Vec<String>,
        external_momenta: Vec<String>,
        external_gram: Vec<Vec<Coefficient>>,
        limits: SymbolicaAffineDenominatorLimits,
    ) -> Result<Self, SymbolicaAffineDenominatorError> {
        check_limit(
            "base parameters",
            coefficients.parameter_names().len(),
            limits.max_base_parameters,
        )?;
        // Authenticate the already-retained template without constructing an
        // additional zero coefficient before its storage policy is known.
        coefficients.validate_with_limits(coefficients.template(), limits.exact_algebra)?;
        if loop_momenta.is_empty() {
            return Err(SymbolicaAffineDenominatorError::NoLoopMomenta);
        }

        let momentum_count = loop_momenta
            .len()
            .checked_add(external_momenta.len())
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "declared momenta",
            })?;
        check_limit("declared momenta", momentum_count, limits.max_momenta)?;
        let coordinate_count =
            scalar_product_coordinate_count(loop_momenta.len(), external_momenta.len())?;
        check_limit(
            "scalar-product coordinates",
            coordinate_count,
            limits.max_scalar_product_coordinates,
        )?;
        validate_declared_labels(&coefficients, &loop_momenta, &external_momenta, limits)?;
        validate_external_gram(&coefficients, &external_momenta, &external_gram, limits)?;

        let combined_count = coefficients
            .parameter_names()
            .len()
            .checked_add(momentum_count)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "combined Symbolica variables",
            })?;
        check_limit(
            "combined Symbolica variables",
            combined_count,
            limits.max_combined_variables,
        )?;
        check_limit(
            "combined variable-map exponent width",
            combined_count,
            limits.max_combined_exponent_entries,
        )?;
        let mut combined_names = Vec::new();
        combined_names
            .try_reserve_exact(combined_count)
            .map_err(|_| SymbolicaAffineDenominatorError::AllocationFailure {
                resource: "combined Symbolica variable names",
                requested: combined_count,
            })?;
        combined_names.extend(coefficients.parameter_names().iter().cloned());
        combined_names.extend(loop_momenta.iter().cloned());
        combined_names.extend(external_momenta.iter().cloned());
        let combined = CoefficientContext::try_new(combined_names.clone())?;
        let combined_template_census = planned_coefficient_clone_census(
            combined.template(),
            combined.parameter_names().len(),
        )?;
        check_limit(
            "combined template retained bytes",
            combined_template_census.retained_bytes,
            limits.max_combined_retained_bytes,
        )?;

        for (position, label) in coefficients.parameter_names().iter().enumerate() {
            if combined.variables()[position] != coefficients.variables()[position] {
                return Err(
                    SymbolicaAffineDenominatorError::CombinedVariableMapMismatch {
                        position,
                        label: label.clone(),
                    },
                );
            }
        }
        let symbol_positions = authenticate_combined_symbols(&combined, &combined_names)?;
        let scalar_product = reserved_scalar_product(&symbol_positions)?;
        let coordinates = scalar_product_coordinates(
            loop_momenta.len(),
            external_momenta.len(),
            coordinate_count,
        )?;
        Ok(Self {
            coefficients,
            loop_momenta,
            external_momenta,
            external_gram,
            combined,
            symbol_positions,
            scalar_product,
            coordinates,
            limits,
        })
    }
}
