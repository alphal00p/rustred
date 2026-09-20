//! Opt-in native factorized-denominator field for one or several exact targets.
//!
//! Integral columns, row order, zero sentinel, pivot selection and stopping
//! match the ordinary sparse backend. Symbolica owns all field operations,
//! including factorization during pivot inversion. No factorized values cross
//! this boundary, and the original source guards/provenance remain unchanged.
//! Like ordinary sparse lifting, this does not bound native CAS scratch memory
//! or coefficient growth; callers requiring hard limits need a process budget.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::domains::Ring;
use symbolica::domains::factorized_rational_polynomial::{
    FactorizedRationalPolynomial, FactorizedRationalPolynomialField,
    FromNumeratorAndFactorizedDenominator,
};
use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::prelude::{IntegerRing, PolyVariable, Z};
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use crate::algebra::Coefficient;

use super::variables::FrameVariables;
use super::{ExactRow, Integral, IntegralOrder, MaterializationError, MaterializationEvent, Term};

pub(super) type NativeCoefficient = FactorizedRationalPolynomial<IntegerRing, u16>;

pub(super) fn materialize<const N: usize>(
    rows: &[ExactRow<N>],
    columns: &[Integral<N>],
    order: &IntegralOrder<N>,
    target_column: usize,
    variables: &FrameVariables,
    mut observe: impl FnMut(MaterializationEvent<N>),
) -> Result<ExactRow<N>, MaterializationError> {
    let mut result = materialize_many(
        rows,
        columns,
        order,
        &[target_column],
        variables,
        &mut observe,
    )?;
    Ok(result.remove(0).1)
}

/// Share native elimination across requested pivots without expanding any
/// intermediate coefficient. Results retain pivot encounter order and carry
/// the caller's target ordinal, just like the ordinary finite-corner lift.
pub(super) fn materialize_many<const N: usize>(
    rows: &[ExactRow<N>],
    columns: &[Integral<N>],
    order: &IntegralOrder<N>,
    target_columns: &[usize],
    variables: &FrameVariables,
    mut observe: impl FnMut(MaterializationEvent<N>),
) -> Result<Vec<(usize, ExactRow<N>)>, MaterializationError> {
    if target_columns.is_empty() {
        return Ok(Vec::new());
    }
    let mut wanted = vec![None; columns.len()];
    for (target, &column) in target_columns.iter().enumerate() {
        let slot = wanted
            .get_mut(column)
            .ok_or(MaterializationError::InvalidTargetSelection(
                "column outside frame",
            ))?;
        if slot.replace(target).is_some() {
            return Err(MaterializationError::InvalidTargetSelection(
                "duplicate target column",
            ));
        }
    }
    let mut solutions = Vec::with_capacity(target_columns.len());
    let native_columns = columns
        .len()
        .checked_add(1)
        .and_then(|count| u32::try_from(count).ok())
        .ok_or(MaterializationError::TooManyColumns)?;
    let active = variables.active_variables();
    let field = FactorizedRationalPolynomialField::<_, u16>::new(Z, active.clone());
    let mut reducer = SparseRowReducer::new(native_columns, field, LuLMode::None);
    let mut values = Vec::new();
    let mut column_ids = Vec::new();
    for (ordinal, row) in rows.iter().enumerate() {
        values.clear();
        column_ids.clear();
        for term in row {
            if term.coefficient.denominator.is_zero() {
                return Err(MaterializationError::InvalidFactorizedCoefficient(
                    "zero input denominator",
                ));
            }
            if term.coefficient.is_zero() {
                continue;
            }
            let column = columns
                .binary_search_by(|column| order.compare(column, &term.integral))
                .expect("exact column union contains every row integral");
            let coefficient = variables.map_coefficient(&term.coefficient)?;
            values.push(factor(coefficient, &active)?);
            column_ids.push(column as u32);
        }
        observe(MaterializationEvent::RowStarted {
            row: ordinal + 1,
            input_nonzeros: values.len(),
            reducer_rows: reducer.u().nrows() as usize,
            reducer_nonzeros: reducer.u().nvalues(),
        });
        // The observer is deliberately outside the native panic boundary.
        let pivot = native("reducing an exact source row", || {
            reducer.add_row(&values, &column_ids)
        })?;
        observe(MaterializationEvent::RowFinished {
            row: ordinal + 1,
            pivot: pivot.map(|column| columns[column as usize]),
            reducer_rows: reducer.u().nrows() as usize,
            reducer_nonzeros: reducer.u().nvalues(),
        });
        if let Some(target) = pivot.and_then(|column| wanted[column as usize].take()) {
            let u = reducer.u();
            let start = u.row_ptrs()[u.nrows() as usize - 1];
            let end = u.row_ptrs()[u.nrows() as usize];
            let solution = u.col_idcs()[start..end]
                .iter()
                .zip(&u.values()[start..end])
                .map(|(&column, value)| {
                    let coefficient = ordinary(value, &active)?;
                    Ok(Term {
                        integral: columns[column as usize],
                        coefficient: variables.restore_coefficient(&coefficient)?,
                    })
                })
                .collect::<Result<_, MaterializationError>>()?;
            solutions.push((target, solution));
            if solutions.len() == target_columns.len() {
                return Ok(solutions);
            }
        }
    }
    Err(MaterializationError::TargetNotPivot)
}

/// Convert an already frame-mapped ordinary coefficient using the native field.
pub(super) fn factor(
    coefficient: Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
) -> Result<NativeCoefficient, MaterializationError> {
    if coefficient.numerator.variables() != variables
        || coefficient.denominator.variables() != variables
    {
        return Err(MaterializationError::CoefficientVariableMapMismatch);
    }
    if coefficient.denominator.is_zero() {
        return Err(MaterializationError::InvalidFactorizedCoefficient(
            "zero input denominator",
        ));
    }
    let value = native("converting an input denominator", || {
        NativeCoefficient::from_num_den(
            coefficient.numerator,
            vec![(coefficient.denominator, 1)],
            &Z,
            true,
        )
    })?;
    validate_map(&value, variables)?;
    Ok(value)
}

pub(super) fn validate_map(
    value: &NativeCoefficient,
    variables: &Arc<Vec<PolyVariable>>,
) -> Result<(), MaterializationError> {
    if value.numerator.variables() != variables
        || value
            .denominators
            .iter()
            .any(|(factor, _)| factor.variables() != variables)
    {
        return Err(MaterializationError::CoefficientVariableMapMismatch);
    }
    if Z.is_zero(&value.denom_coeff)
        || value
            .denominators
            .iter()
            .any(|(factor, power)| factor.is_zero() || *power == 0)
    {
        return Err(MaterializationError::InvalidFactorizedCoefficient(
            "zero denominator or invalid factor multiplicity",
        ));
    }
    Ok(())
}

pub(super) fn ordinary(
    value: &NativeCoefficient,
    variables: &Arc<Vec<PolyVariable>>,
) -> Result<Coefficient, MaterializationError> {
    validate_map(value, variables)?;
    // Same composition of native calls as Symbolica's constructor tests.
    // This is representation materialization, not a polynomial/CAS kernel.
    native("materializing the exact target row", || {
        let numerator = value.numerator.clone().mul_coeff(value.numer_coeff.clone());
        let denominator = value.denominators.iter().fold(
            numerator.constant(value.denom_coeff.clone()),
            |product, (factor, power)| product * &factor.pow(*power),
        );
        Coefficient::from_num_den(numerator, denominator, &Z, true)
    })
}

pub(super) fn native<T>(
    operation: &'static str,
    function: impl FnOnce() -> T,
) -> Result<T, MaterializationError> {
    catch_unwind(AssertUnwindSafe(function))
        .map_err(|_| MaterializationError::FactorizedNativePanic { operation })
}

#[cfg(test)]
mod tests;
