//! Family-owned adaptation of checked exact-algebra operations.

#[cfg(test)]
use crate::algebra::matrix::verify_coefficient_matrix_inverse;
use crate::algebra::matrix::{
    SymbolicaCoefficientMatrixError, SymbolicaCoefficientMatrixLimits,
    congruence_of_coefficient_matrix, invert_and_verify_coefficient_matrix,
};
use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};

use super::error::IntegralFamilyError;
use super::model::IntegralFamilyLimits;

pub(super) fn coefficients_are_equal(
    context: &CoefficientContext,
    left: &Coefficient,
    right: &Coefficient,
    limits: ExactAlgebraLimits,
) -> Result<bool, IntegralFamilyError> {
    if left == right {
        return Ok(true);
    }
    Ok(context.try_sub(left, right, limits)?.is_zero())
}

pub(crate) fn congruence_symbolic_matrix(
    context: &CoefficientContext,
    transform: &[Vec<Coefficient>],
    middle: &[Vec<Coefficient>],
    limits: IntegralFamilyLimits,
) -> Result<Vec<Vec<Coefficient>>, IntegralFamilyError> {
    let size = transform.len().max(middle.len());
    let (product, _stats) = congruence_of_coefficient_matrix(
        context,
        transform,
        middle,
        symbolica_matrix_limits(limits),
    )
    .map_err(|error| map_symbolica_matrix_error(error, size))?;
    Ok(product)
}

pub(crate) fn invert_symbolic_matrix<Row>(
    context: &CoefficientContext,
    matrix: &[Row],
    limits: IntegralFamilyLimits,
) -> Result<(Vec<Vec<Coefficient>>, Coefficient), IntegralFamilyError>
where
    Row: AsRef<[Coefficient]>,
{
    let size = matrix.len();
    let verified =
        invert_and_verify_coefficient_matrix(context, matrix, symbolica_matrix_limits(limits))
            .map_err(|error| map_symbolica_matrix_error(error, size))?;
    let (inverse, determinant, _stats) = verified.into_parts();
    Ok((inverse, determinant))
}

#[cfg(test)]
pub(super) fn verify_inverse<Row, InverseRow>(
    context: &CoefficientContext,
    matrix: &[Row],
    inverse: &[InverseRow],
    limits: IntegralFamilyLimits,
) -> Result<(), IntegralFamilyError>
where
    Row: AsRef<[Coefficient]>,
    InverseRow: AsRef<[Coefficient]>,
{
    verify_coefficient_matrix_inverse(context, matrix, inverse, symbolica_matrix_limits(limits))
        .map_err(|error| map_symbolica_matrix_error(error, matrix.len()))?;
    Ok(())
}

pub(crate) fn symbolica_matrix_limits(
    limits: IntegralFamilyLimits,
) -> SymbolicaCoefficientMatrixLimits {
    SymbolicaCoefficientMatrixLimits::for_family(
        limits.exact_algebra,
        limits.max_matrix_entries,
        limits.max_matrix_exact_operations,
        limits.max_matrix_input_retained_bytes,
        limits.max_matrix_output_retained_bytes,
    )
}

pub(super) fn map_symbolica_matrix_error(
    error: SymbolicaCoefficientMatrixError,
    size: usize,
) -> IntegralFamilyError {
    match error {
        SymbolicaCoefficientMatrixError::EmptyMatrix
        | SymbolicaCoefficientMatrixError::RaggedMatrix { .. }
        | SymbolicaCoefficientMatrixError::NotSquare { .. }
        | SymbolicaCoefficientMatrixError::Singular => {
            IntegralFamilyError::SingularDenominatorBasis
        }
        SymbolicaCoefficientMatrixError::DimensionOverflow { .. } => {
            IntegralFamilyError::MatrixDimensionOverflow { size }
        }
        SymbolicaCoefficientMatrixError::ResourceCountOverflow { resource } => {
            IntegralFamilyError::ResourceCountOverflow { resource }
        }
        SymbolicaCoefficientMatrixError::ResourceLimit {
            resource,
            requested,
            limit,
        } => IntegralFamilyError::ResourceLimit {
            resource,
            requested,
            limit,
        },
        SymbolicaCoefficientMatrixError::AllocationFailure {
            resource,
            requested,
        } => IntegralFamilyError::AllocationFailure {
            resource,
            requested,
        },
        SymbolicaCoefficientMatrixError::InvalidCoefficient { error, .. }
        | SymbolicaCoefficientMatrixError::ExactAlgebra(error) => {
            IntegralFamilyError::ExactAlgebra(error)
        }
        internal @ (SymbolicaCoefficientMatrixError::ShapeMismatch { .. }
        | SymbolicaCoefficientMatrixError::NativePowerExponentLimit { .. }
        | SymbolicaCoefficientMatrixError::NativeError { .. }
        | SymbolicaCoefficientMatrixError::NativePanic { .. }
        | SymbolicaCoefficientMatrixError::InverseVerificationFailure { .. }
        | SymbolicaCoefficientMatrixError::InternalShapeFailure { .. }) => {
            IntegralFamilyError::InternalVerificationFailure {
                detail: internal.to_string(),
            }
        }
    }
}
