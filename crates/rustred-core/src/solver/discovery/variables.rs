//! Native polynomial-context compaction for one selected exact frame.
//!
//! Removing globally absent variables is an injective change of representation,
//! not a substitution. An optional reversal changes only the native coefficient
//! representation, never physical integral keys. Restore the complete original
//! map before canonicalization.

use std::sync::Arc;

use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::poly::PolyVariable;
use symbolica::prelude::Z;

use crate::algebra::Coefficient;

use super::{CoefficientVariableOrder, ExactRow, MaterializationError};

#[derive(Debug)]
pub(super) struct FrameVariables {
    original: Arc<Vec<PolyVariable>>,
    active: Arc<Vec<PolyVariable>>,
}

impl FrameVariables {
    pub(super) fn try_new<const N: usize>(
        rows: &[ExactRow<N>],
        order: CoefficientVariableOrder,
    ) -> Result<Self, MaterializationError> {
        let Some(first) = rows.iter().flatten().next() else {
            let empty = Arc::new(Vec::new());
            return Ok(Self {
                original: empty.clone(),
                active: empty,
            });
        };
        let original = first.coefficient.get_variables().clone();
        let mut used = vec![false; original.len()];
        for term in rows.iter().flatten() {
            Self::validate_map(&term.coefficient, &original)?;
            for (axis, active) in used.iter_mut().enumerate() {
                if !*active {
                    *active = term.coefficient.numerator.contains(axis)
                        || term.coefficient.denominator.contains(axis);
                }
            }
        }
        let active = if order == CoefficientVariableOrder::Original && used.iter().all(|used| *used)
        {
            original.clone()
        } else {
            let mut active: Vec<_> = original
                .iter()
                .zip(used)
                .filter_map(|(variable, used)| used.then(|| variable.clone()))
                .collect();
            if order == CoefficientVariableOrder::Reverse {
                active.reverse();
            }
            Arc::new(active)
        };
        Ok(Self { original, active })
    }

    pub(super) fn original_len(&self) -> usize {
        self.original.len()
    }

    pub(super) fn active_len(&self) -> usize {
        self.active.len()
    }

    pub(super) fn map_coefficient(
        &self,
        value: &Coefficient,
    ) -> Result<Coefficient, MaterializationError> {
        Self::remap(value, &self.original, &self.active)
    }

    pub(super) fn restore_coefficient(
        &self,
        value: &Coefficient,
    ) -> Result<Coefficient, MaterializationError> {
        Self::remap(value, &self.active, &self.original)
    }

    fn validate_map(
        value: &Coefficient,
        expected: &Arc<Vec<PolyVariable>>,
    ) -> Result<(), MaterializationError> {
        if value.numerator.variables() != expected || value.denominator.variables() != expected {
            return Err(MaterializationError::CoefficientVariableMapMismatch);
        }
        Ok(())
    }

    fn remap(
        value: &Coefficient,
        from: &Arc<Vec<PolyVariable>>,
        to: &Arc<Vec<PolyVariable>>,
    ) -> Result<Coefficient, MaterializationError> {
        Self::validate_map(value, from)?;
        if from == to {
            return Ok(value.clone());
        }
        let numerator = value
            .numerator
            .rearrange_with_growth(to)
            .map_err(MaterializationError::CoefficientVariableRemap)?;
        let denominator = value
            .denominator
            .rearrange_with_growth(to)
            .map_err(MaterializationError::CoefficientVariableRemap)?;
        // Both directions preserve every occurring variable. The normalized
        // fraction remains coprime, so a new native GCD would be redundant.
        Ok(Coefficient::from_num_den(numerator, denominator, &Z, false))
    }
}

#[cfg(test)]
mod tests;
