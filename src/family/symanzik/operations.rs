//! Checked determinant, adjugate, and resource-accounting helpers.

use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::prelude::*;

use super::context::FeynmanPolynomialContext;
use super::error::FeynmanPolynomialError;
use super::model::{FeynmanPolynomial, FeynmanPolynomialRing};
use super::work::{FeynmanWorkBudget, check_limit, checked_add, checked_mul};

pub(super) fn checked_determinant(
    context: &FeynmanPolynomialContext,
    matrix: &[Vec<FeynmanPolynomial>],
    work: &mut FeynmanWorkBudget,
) -> Result<FeynmanPolynomial, FeynmanPolynomialError> {
    let size = matrix.len();
    if matrix.iter().any(|row| row.len() != size) {
        return Err(FeynmanPolynomialError::InternalVerificationFailure {
            detail: "determinant received a non-square matrix".to_owned(),
        });
    }
    if size == 0 {
        return context.one();
    }

    let matrix_entries = checked_mul(size, size, "Symbolica determinant matrix entries")?;
    check_limit(
        "Symbolica determinant matrix entries",
        matrix_entries,
        context.limits.max_determinant_matrix_entries,
    )?;
    work.charge_determinant_ring_operations(determinant_ring_operations(size)?)?;

    let native_size =
        u32::try_from(size).map_err(|_| FeynmanPolynomialError::ResourceCountOverflow {
            resource: "Symbolica determinant matrix dimension",
        })?;
    let native_matrix_entries = native_size.checked_mul(native_size).ok_or(
        FeynmanPolynomialError::ResourceCountOverflow {
            resource: "Symbolica determinant u32 matrix entries",
        },
    )?;
    if native_matrix_entries as usize != matrix_entries {
        return Err(FeynmanPolynomialError::InternalVerificationFailure {
            detail: "Symbolica determinant matrix dimensions failed a checked round trip"
                .to_owned(),
        });
    }
    let mut entries = Vec::new();
    entries.try_reserve_exact(matrix_entries).map_err(|_| {
        FeynmanPolynomialError::AllocationFailure {
            resource: "Symbolica determinant input entries",
            requested: matrix_entries,
        }
    })?;
    for row in matrix {
        for entry in row {
            context.authenticate(entry)?;
            entries.push(entry.raw.clone());
        }
    }
    let ring = FeynmanPolynomialRing::from_poly(&context.template);
    let native = Matrix::from_linear(entries, native_size, native_size, ring).map_err(|_| {
        FeynmanPolynomialError::InternalVerificationFailure {
            detail: "Symbolica rejected a preflighted determinant matrix".to_owned(),
        }
    })?;
    let raw = catch_unwind(AssertUnwindSafe(|| native.det()))
        .map_err(|_| FeynmanPolynomialError::SymbolicaPanic)?
        .map_err(
            |error| FeynmanPolynomialError::InternalVerificationFailure {
                detail: native_determinant_error_detail(error).to_owned(),
            },
        )?;
    context.rebind_native_result(raw)
}

pub(super) fn native_determinant_error_detail(
    error: symbolica::tensors::matrix::MatrixError<FeynmanPolynomialRing>,
) -> &'static str {
    use symbolica::tensors::matrix::MatrixError;

    match error {
        MatrixError::Underdetermined { .. } => {
            "Symbolica Matrix::det unexpectedly reported an underdetermined matrix"
        }
        MatrixError::Inconsistent => {
            "Symbolica Matrix::det unexpectedly reported an inconsistent matrix"
        }
        MatrixError::NotSquare => "Symbolica Matrix::det rejected a preflighted square K[x] matrix",
        MatrixError::Singular => {
            "Symbolica Matrix::det unexpectedly rejected a nonempty singular K[x] matrix"
        }
        MatrixError::ShapeMismatch => {
            "Symbolica Matrix::det unexpectedly reported a shape mismatch"
        }
        MatrixError::RightHandSideIsNotVector => {
            "Symbolica Matrix::det unexpectedly requested a vector right-hand side"
        }
        MatrixError::ResultNotInDomain => "Symbolica Matrix::det produced a result outside K[x]",
    }
}

/// Count the native determinant's structural ring operations without doing
/// any algebra in RustRed.  Symbolica uses direct formulas for sizes at most
/// three and fraction-free Bareiss elimination above that threshold.  Four
/// operations per trailing entry conservatively includes the first Bareiss
/// step, where Symbolica omits the exact division.
pub(super) fn determinant_ring_operations(size: usize) -> Result<usize, FeynmanPolynomialError> {
    match size {
        0 | 1 => Ok(0),
        2 => Ok(3),
        3 => Ok(14),
        _ => {
            let mut sum_of_squares = 0_usize;
            for trailing_size in 1..size {
                let square = checked_mul(
                    trailing_size,
                    trailing_size,
                    "Symbolica Bareiss determinant ring operations",
                )?;
                sum_of_squares = checked_add(
                    sum_of_squares,
                    square,
                    "Symbolica Bareiss determinant ring operations",
                )?;
            }
            checked_mul(
                4,
                sum_of_squares,
                "Symbolica Bareiss determinant ring operations",
            )
        }
    }
}

pub(super) fn checked_symbolica_neg(
    context: &FeynmanPolynomialContext,
    polynomial: &FeynmanPolynomial,
    work: &mut FeynmanWorkBudget,
) -> Result<FeynmanPolynomial, FeynmanPolynomialError> {
    context.authenticate(polynomial)?;
    work.charge_term_operations(polynomial.raw.nterms())?;
    let ring = FeynmanPolynomialRing::from_poly(&context.template);
    let raw = catch_unwind(AssertUnwindSafe(|| ring.neg(polynomial.raw())))
        .map_err(|_| FeynmanPolynomialError::SymbolicaPanic)?;
    context.rebind_native_result(raw)
}

pub(super) fn checked_adjugate(
    context: &FeynmanPolynomialContext,
    matrix: &[Vec<FeynmanPolynomial>],
    work: &mut FeynmanWorkBudget,
) -> Result<Vec<Vec<FeynmanPolynomial>>, FeynmanPolynomialError> {
    let size = matrix.len();
    let minors = size
        .checked_mul(size)
        .ok_or(FeynmanPolynomialError::ResourceCountOverflow {
            resource: "adjugate minors",
        })?;
    check_limit(
        "adjugate minors",
        minors,
        context.limits.max_adjugate_minors,
    )?;
    let mut adjugate = vec![vec![context.zero(); size]; size];
    for row in 0..size {
        for column in 0..size {
            // adj(A)[row,column] is the cofactor with row `column` and
            // column `row` deleted.
            let minor = matrix
                .iter()
                .enumerate()
                .filter(|(candidate, _)| *candidate != column)
                .map(|(_, values)| {
                    values
                        .iter()
                        .enumerate()
                        .filter(|(candidate, _)| *candidate != row)
                        .map(|(_, value)| value.clone())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let mut cofactor = checked_determinant(context, &minor, work)?;
            if (row + column) % 2 == 1 {
                cofactor = checked_symbolica_neg(context, &cofactor, work)?;
            }
            adjugate[row][column] = cofactor;
        }
    }
    Ok(adjugate)
}

pub(super) fn verify_homogeneous(
    polynomial: &FeynmanPolynomial,
    expected: usize,
    name: &'static str,
) -> Result<(), FeynmanPolynomialError> {
    for (_, exponents) in polynomial.terms() {
        let degree = exponents.iter().try_fold(0_usize, |total, &exponent| {
            total.checked_add(usize::from(exponent))
        });
        if degree != Some(expected) {
            return Err(FeynmanPolynomialError::InternalVerificationFailure {
                detail: format!("{name} has a monomial of degree {degree:?}, expected {expected}"),
            });
        }
    }
    Ok(())
}
