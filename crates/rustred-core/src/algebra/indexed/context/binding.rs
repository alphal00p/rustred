use std::sync::Arc;
#[cfg(test)]
use std::sync::atomic::Ordering;

use crate::algebra::{
    ExactAlgebraLimits, IndexedCoefficientContext, validate_coefficient_on_map,
    validate_polynomial_on_map,
};

use super::super::error::IndexedAlgebraError;
use super::super::value::{IndexedCoefficient, IndexedPolynomial};
use super::BoundIndexedCoefficient;

impl IndexedCoefficientContext {
    /// Prefer pointer identity for the hot same-context path, while retaining
    /// exact compatibility for an independently constructed equivalent
    /// context. The full string is scanned only when the owners differ.
    pub(crate) fn owns_fingerprint(&self, fingerprint: &Arc<String>) -> bool {
        Arc::ptr_eq(&self.fingerprint, fingerprint)
            || self.fingerprint.as_str() == fingerprint.as_str()
    }

    /// Fully authenticate one coefficient at an ingress boundary and return a
    /// context-bound borrow for subsequent trusted arithmetic.
    pub(crate) fn authenticate_coefficient_with_limits<'context, 'value>(
        &'context self,
        value: &'value IndexedCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<BoundIndexedCoefficient<'context, 'value>, IndexedAlgebraError> {
        let bound = self.bind_sealed(value)?;
        #[cfg(test)]
        self.authentication_counters
            .full_operand_scans
            .fetch_add(1, Ordering::Relaxed);
        validate_coefficient_on_map(&value.raw, &self.variables, limits)?;
        Ok(bound)
    }

    /// Bind an already sealed coefficient using the exact identity,
    /// without rescanning its polynomial payload.
    pub(crate) fn bind_sealed<'context, 'value>(
        &'context self,
        value: &'value IndexedCoefficient,
    ) -> Result<BoundIndexedCoefficient<'context, 'value>, IndexedAlgebraError> {
        if self.owns_fingerprint(&value.context) {
            Ok(BoundIndexedCoefficient {
                value,
                bound_context: &self.fingerprint,
            })
        } else {
            Err(IndexedAlgebraError::WrongContext)
        }
    }

    #[cfg(test)]
    pub(crate) fn authentication_scan_counts(&self) -> (usize, usize) {
        (
            self.authentication_counters
                .full_operand_scans
                .load(Ordering::Relaxed),
            self.authentication_counters
                .authenticated_native_results
                .load(Ordering::Relaxed),
        )
    }

    pub fn contains(&self, value: &IndexedCoefficient) -> bool {
        self.authenticate_coefficient_with_limits(value, ExactAlgebraLimits::default())
            .is_ok()
    }

    pub fn validate_polynomial_with_limits(
        &self,
        value: &IndexedPolynomial,
        limits: ExactAlgebraLimits,
    ) -> Result<(), IndexedAlgebraError> {
        self.validate_polynomial_context(value)?;
        validate_polynomial_on_map(
            &value.raw,
            &self.variables,
            crate::algebra::CoefficientPolynomialPart::Numerator,
            limits,
        )?;
        Ok(())
    }

    pub(crate) fn validate_polynomial_context(
        &self,
        value: &IndexedPolynomial,
    ) -> Result<(), IndexedAlgebraError> {
        if self.owns_fingerprint(&value.context) {
            Ok(())
        } else {
            Err(IndexedAlgebraError::WrongContext)
        }
    }

    pub fn validate_with_limits(
        &self,
        value: &IndexedCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<(), IndexedAlgebraError> {
        self.authenticate_coefficient_with_limits(value, limits)
            .map(|_| ())
    }

    pub(crate) fn validate_index_arity(&self, shift: &[i64]) -> Result<(), IndexedAlgebraError> {
        if shift.len() == self.index_count() {
            Ok(())
        } else {
            Err(IndexedAlgebraError::WrongIndexArity {
                expected: self.index_count(),
                actual: shift.len(),
            })
        }
    }

    pub(super) fn validate_bound(
        &self,
        value: BoundIndexedCoefficient<'_, '_>,
    ) -> Result<(), IndexedAlgebraError> {
        if Arc::ptr_eq(&self.fingerprint, value.bound_context) {
            Ok(())
        } else {
            Err(IndexedAlgebraError::WrongContext)
        }
    }

    pub(super) fn record_authenticated_native_result(&self) {
        #[cfg(test)]
        self.authentication_counters
            .authenticated_native_results
            .fetch_add(1, Ordering::Relaxed);
    }
}
