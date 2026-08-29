//! Exact lowering from normalized input to the family engine.

use std::panic::{AssertUnwindSafe, catch_unwind};

use super::affine::SymbolicaAffineDenominatorCompiler;
use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::{AffineDenominator, IntegralFamily};

use super::error::LoweringError;
use super::limits::LoweringLimits;
use super::model::{LoweredDenominator, LoweredProject, Project};

impl Project {
    /// Consume this normalized declaration and lower it to the exact integral
    /// family consumed by parametric IBP generation.
    pub fn into_lowered(self, limits: LoweringLimits) -> Result<LoweredProject, LoweringError> {
        guarded_lowering("normalized project lowering", || {
            lower_normalized_project(self, limits)
        })
    }
}

pub(super) fn lower_normalized_project(
    normalized: Project,
    limits: LoweringLimits,
) -> Result<LoweredProject, LoweringError> {
    let coefficients =
        CoefficientContext::try_new(normalized.operational_parameter_names.iter().cloned())?;
    let bootstrap_gram = coefficient_matrix(
        normalized.external_momenta.len(),
        &coefficients,
        "bootstrap external Gram matrix",
    )?;
    let bootstrap = SymbolicaAffineDenominatorCompiler::try_new(
        coefficients.clone(),
        normalized.loop_momenta.clone(),
        normalized.external_momenta.clone(),
        bootstrap_gram,
        limits.affine_denominator,
    )?;
    let dimension = bootstrap.parse_base_coefficient(normalized.dimension.as_view())?;

    let mut external_gram = Vec::<Vec<Coefficient>>::new();
    external_gram
        .try_reserve_exact(normalized.external_gram.len())
        .map_err(|_| LoweringError::AllocationFailure {
            resource: "lowered external Gram rows",
            requested: normalized.external_gram.len(),
        })?;
    for row in &normalized.external_gram {
        let mut lowered_row = Vec::<Coefficient>::new();
        lowered_row
            .try_reserve_exact(row.len())
            .map_err(|_| LoweringError::AllocationFailure {
                resource: "lowered external Gram row",
                requested: row.len(),
            })?;
        for value in row {
            lowered_row.push(bootstrap.parse_base_coefficient(value.as_view())?);
        }
        external_gram.push(lowered_row);
    }

    let mut power_shifts = Vec::<Coefficient>::new();
    power_shifts
        .try_reserve_exact(normalized.propagators.len())
        .map_err(|_| LoweringError::AllocationFailure {
            resource: "lowered power shifts",
            requested: normalized.propagators.len(),
        })?;
    for propagator in &normalized.propagators {
        power_shifts.push(bootstrap.parse_base_coefficient(propagator.power_shift.as_view())?);
    }

    // Rebuild with the authenticated physical Gram matrix before evaluating
    // any denominator containing external scalar products.
    let compiler = SymbolicaAffineDenominatorCompiler::try_new(
        coefficients.clone(),
        normalized.loop_momenta.clone(),
        normalized.external_momenta.clone(),
        external_gram.clone(),
        limits.affine_denominator,
    )?;
    let mut denominators = Vec::<LoweredDenominator>::new();
    denominators
        .try_reserve_exact(normalized.propagators.len())
        .map_err(|_| LoweringError::AllocationFailure {
            resource: "compiled Symbolica denominators",
            requested: normalized.propagators.len(),
        })?;
    let mut affine_denominators = Vec::<AffineDenominator>::new();
    affine_denominators
        .try_reserve_exact(normalized.propagators.len())
        .map_err(|_| LoweringError::AllocationFailure {
            resource: "affine denominator rows",
            requested: normalized.propagators.len(),
        })?;
    for propagator in &normalized.propagators {
        let compiled = compiler.compile(propagator.expression.as_view())?;
        let (source, normalized_expression, affine_denominator) = compiled.into_parts();
        affine_denominators.push(affine_denominator);
        denominators.push(LoweredDenominator {
            id: propagator.id.clone(),
            source,
            normalized_expression,
        });
    }

    let family = IntegralFamily::new_with_limits(
        normalized.name.clone(),
        normalized.loop_momenta.clone(),
        normalized.external_momenta.clone(),
        coefficients,
        dimension.clone(),
        affine_denominators,
        external_gram,
        power_shifts,
        limits.integral_family,
    )?;
    Ok(LoweredProject {
        normalized,
        denominators,
        family,
    })
}

pub(super) fn coefficient_matrix(
    size: usize,
    coefficients: &CoefficientContext,
    resource: &'static str,
) -> Result<Vec<Vec<Coefficient>>, LoweringError> {
    size.checked_mul(size)
        .ok_or(LoweringError::ResourceCountOverflow { resource })?;
    let mut matrix = Vec::new();
    matrix
        .try_reserve_exact(size)
        .map_err(|_| LoweringError::AllocationFailure {
            resource,
            requested: size,
        })?;
    for _ in 0..size {
        let mut row = Vec::new();
        row.try_reserve_exact(size)
            .map_err(|_| LoweringError::AllocationFailure {
                resource,
                requested: size,
            })?;
        for _ in 0..size {
            row.push(coefficients.zero());
        }
        matrix.push(row);
    }
    Ok(matrix)
}

fn guarded_lowering<T>(
    operation: &'static str,
    work: impl FnOnce() -> Result<T, LoweringError>,
) -> Result<T, LoweringError> {
    catch_unwind(AssertUnwindSafe(work)).map_err(|_| LoweringError::SymbolicaPanic { operation })?
}
