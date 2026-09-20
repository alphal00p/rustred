//! Exact full-column authority. No elimination or reconstruction lives here.

use symbolica::domains::SelfRing;
use symbolica::prelude::Z;
use symbolica::tensors::sparse::SparseMatrix;

use super::super::super::ExactField;
use super::{Coefficient, FrameVariables, MaterializationError, ProbeFrame, invalid};

pub(super) fn validate_product(
    frame: &ProbeFrame,
    prefix_len: usize,
    target: usize,
    weights: &[Coefficient],
    target_row: &[(u32, Coefficient)],
    variables: &FrameVariables,
) -> Result<Vec<(u32, Coefficient)>, MaterializationError> {
    if prefix_len == 0 || prefix_len > frame.rows().len() || weights.len() != prefix_len {
        return Err(invalid("weights do not match the frozen first-hit prefix"));
    }
    let width = frame.columns_with_sentinel() - 1;
    if target >= width as usize {
        return Err(MaterializationError::TargetAbsent);
    }
    let active = variables.active_variables();
    for value in weights
        .iter()
        .chain(target_row.iter().map(|(_, value)| value))
    {
        if value.numerator.variables() != &active || value.denominator.variables() != &active {
            return Err(MaterializationError::CoefficientVariableMapMismatch);
        }
        if value.denominator.is_zero() {
            return Err(invalid("zero reconstructed denominator"));
        }
    }
    if target_row
        .first()
        .is_none_or(|(column, value)| *column as usize != target || !value.is_one())
        || target_row
            .iter()
            .any(|(column, value)| *column >= width || value.is_zero())
        || target_row.windows(2).any(|pair| pair[0].0 >= pair[1].0)
    {
        return Err(invalid(
            "reconstructed row has forbidden columns, nonunit target or invalid support",
        ));
    }
    let count = u32::try_from(prefix_len).map_err(|_| MaterializationError::TooManyColumns)?;
    let field = ExactField::new(Z);
    let mut sources = SparseMatrix::new(0, width, field.clone());
    for row in &frame.rows()[..prefix_len] {
        let (ids, values) = row
            .iter()
            .filter(|(_, value)| !value.is_zero())
            .map(|(column, value)| (*column, value.clone()))
            .unzip();
        sources.add_row(values, ids);
    }
    let mut weight_matrix = SparseMatrix::new(0, count, field.clone());
    let (ids, values) = weights
        .iter()
        .enumerate()
        .filter(|(_, value)| !value.is_zero())
        .map(|(source, value)| (source as u32, value.clone()))
        .unzip();
    weight_matrix.add_row(values, ids);
    let product = &weight_matrix * &sources;
    if product.values().iter().any(|value| {
        value.numerator.variables() != &active || value.denominator.variables() != &active
    }) {
        return Err(MaterializationError::CoefficientVariableMapMismatch);
    }
    // Full sparse equality includes every harder/easier physical column. No
    // sampled support or omitted weight can authorize an exact omission.
    let mut expected = SparseMatrix::new(0, width, field);
    let (ids, values) = target_row.iter().cloned().unzip();
    expected.add_row(values, ids);
    if product != expected {
        return Err(invalid(
            "exact full-column source product differs from reconstructed target",
        ));
    }
    target_row
        .iter()
        .map(|(column, coefficient)| {
            let restored = variables.restore_coefficient(coefficient)?;
            // Remapping is injective representation transport. Check both maps on
            // the reverse trip, never identify variables through a sampled point.
            if variables.map_coefficient(&restored)? != *coefficient {
                return Err(MaterializationError::CoefficientVariableMapMismatch);
            }
            Ok((*column, restored))
        })
        .collect()
}
