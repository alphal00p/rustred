//! Family-owned adaptation of checked exact-algebra operations.

#[cfg(test)]
use crate::algebra::matrix::verify_coefficient_matrix_inverse;
use crate::algebra::matrix::{
    SymbolicaCoefficientMatrixError, SymbolicaCoefficientMatrixLimits,
    congruence_of_coefficient_matrix, invert_and_verify_coefficient_matrix,
};
use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use symbolica::domains::rational::Rational;

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
    if let Some(inverse) = invert_rational_constant_matrix(context, matrix) {
        return inverse;
    }
    let verified =
        invert_and_verify_coefficient_matrix(context, matrix, symbolica_matrix_limits(limits))
            .map_err(|error| map_symbolica_matrix_error(error, size))?;
    let (inverse, determinant, _stats) = verified.into_parts();
    Ok((inverse, determinant))
}

/// The rational value of a coefficient that is a constant of its context.
fn rational_constant(value: &Coefficient) -> Option<Rational> {
    (value.numerator.is_constant() && value.denominator.is_constant()).then(|| {
        Rational::new(
            value.numerator.get_constant(),
            value.denominator.get_constant(),
        )
    })
}

/// A constant coefficient of `context` with the value `value`.
fn constant_coefficient(context: &CoefficientContext, value: &Rational) -> Coefficient {
    let polynomial = &context.template().numerator;
    Coefficient {
        numerator: polynomial.constant(value.numerator()),
        denominator: polynomial.constant(value.denominator()),
    }
}

/// Fast exact path of [`invert_symbolic_matrix`] for a matrix of rational
/// constants (every p-integral family: denominators with integer
/// coefficients): Gauss-Jordan over `Q` with exact rationals, then BOTH
/// identity products checked exactly, as the Symbolica path does. `None` when
/// an entry depends on a parameter or the shape is not square: the caller
/// then takes the general Symbolica path, which also reports every error
/// (a singular constant matrix is reported as such here). M5 1L-3L: the
/// general path over `Q(d)` was a third of a cold 1L-3L vakint request with
/// the 1-4 library (444 families decoded at load).
fn invert_rational_constant_matrix<Row>(
    context: &CoefficientContext,
    matrix: &[Row],
) -> Option<Result<(Vec<Vec<Coefficient>>, Coefficient), IntegralFamilyError>>
where
    Row: AsRef<[Coefficient]>,
{
    let size = matrix.len();
    if size == 0 || size > 64 {
        return None;
    }
    let mut original = Vec::with_capacity(size);
    for row in matrix {
        let row = row.as_ref();
        if row.len() != size {
            return None;
        }
        let values: Option<Vec<Rational>> = row.iter().map(rational_constant).collect();
        original.push(values?);
    }
    let mut left = original.clone();
    let mut right: Vec<Vec<Rational>> = (0..size)
        .map(|r| {
            (0..size)
                .map(|c| {
                    if r == c {
                        Rational::one()
                    } else {
                        Rational::zero()
                    }
                })
                .collect()
        })
        .collect();
    let mut determinant = Rational::one();
    for column in 0..size {
        let Some(pivot) = (column..size).find(|&row| !left[row][column].is_zero()) else {
            return Some(Err(IntegralFamilyError::SingularDenominatorBasis));
        };
        if pivot != column {
            left.swap(pivot, column);
            right.swap(pivot, column);
            determinant = determinant.neg();
        }
        let value = left[column][column].clone();
        determinant = &determinant * &value;
        let inverse = value.inv();
        for entry in left[column].iter_mut().chain(right[column].iter_mut()) {
            *entry = &*entry * &inverse;
        }
        for row in 0..size {
            if row == column || left[row][column].is_zero() {
                continue;
            }
            let factor = left[row][column].clone();
            for k in 0..size {
                let l = &left[column][k] * &factor;
                left[row][k] = &left[row][k] - &l;
                let r = &right[column][k] * &factor;
                right[row][k] = &right[row][k] - &r;
            }
        }
    }
    // Both identity products, exactly.
    let product_is_identity = |a: &[Vec<Rational>], b: &[Vec<Rational>]| {
        (0..size).all(|r| {
            (0..size).all(|c| {
                let mut sum = Rational::zero();
                for k in 0..size {
                    sum = &sum + &(&a[r][k] * &b[k][c]);
                }
                if r == c { sum.is_one() } else { sum.is_zero() }
            })
        })
    };
    if !product_is_identity(&original, &right) || !product_is_identity(&right, &original) {
        return None;
    }
    let inverse = right
        .iter()
        .map(|row| {
            row.iter()
                .map(|v| constant_coefficient(context, v))
                .collect()
        })
        .collect();
    Some(Ok((inverse, constant_coefficient(context, &determinant))))
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
    let native = SymbolicaCoefficientMatrixLimits::for_family(
        limits.exact_algebra,
        limits.max_matrix_entries,
        limits.max_matrix_exact_operations,
        limits.max_matrix_input_retained_bytes,
        limits.max_matrix_output_retained_bytes,
    );
    if limits.diagnostics_enabled {
        native.with_diagnostics()
    } else {
        native
    }
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

#[cfg(test)]
mod rational_constant_tests {
    use super::*;

    /// The rational fast path returns exactly the Symbolica path's inverse
    /// and determinant (same orientation, same canonical coefficients).
    #[test]
    fn the_rational_fast_path_matches_the_symbolica_inverse() {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let limits = IntegralFamilyLimits::default();
        let mut seed = 0x9e37_79b9_u64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % 7) as i64 - 3
        };
        let mut checked = 0;
        for size in [1_usize, 2, 3, 5, 9, 14] {
            for _ in 0..6 {
                let matrix: Vec<Vec<Coefficient>> = (0..size)
                    .map(|_| (0..size).map(|_| context.integer(next())).collect())
                    .collect();
                let general = invert_and_verify_coefficient_matrix(
                    &context,
                    &matrix,
                    symbolica_matrix_limits(limits),
                )
                .map(|verified| {
                    let (inverse, determinant, _) = verified.into_parts();
                    (inverse, determinant)
                });
                let fast = invert_rational_constant_matrix(&context, &matrix)
                    .expect("a constant square matrix takes the fast path");
                match (general, fast) {
                    (Ok(general), Ok(fast)) => {
                        assert_eq!(general, fast);
                        checked += 1;
                    }
                    (Err(_), Err(IntegralFamilyError::SingularDenominatorBasis)) => {}
                    (general, fast) => panic!("paths disagree: {general:?} vs {fast:?}"),
                }
            }
        }
        assert!(checked >= 10, "{checked} invertible samples");
        let d = context.parameter("d").unwrap();
        let symbolic = vec![vec![d, context.one()], vec![context.one(), context.one()]];
        assert!(invert_rational_constant_matrix(&context, &symbolic).is_none());
    }
}
