use std::collections::BTreeMap;
use std::sync::Arc;

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext};

use super::super::condition::ParametricNonZeroCondition;
use super::super::row::RowId;
use super::error::ParametricRelationError;
use super::index::IndexShift;

/// A raw parametric zero equation together with every condition inherited
/// before fraction-field cancellation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParametricRelation {
    pub(super) family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    pub(super) row_id: RowId,
    arity: usize,
    pub(super) terms: BTreeMap<IndexShift, IndexedCoefficient>,
    pub(super) nonzero_conditions: Vec<ParametricNonZeroCondition>,
}

impl ParametricRelation {
    pub(in crate::identity) fn new(
        family_fingerprint: impl Into<Arc<str>>,
        row_id: RowId,
        context: &IndexedCoefficientContext,
    ) -> Self {
        Self {
            family_fingerprint: family_fingerprint.into(),
            context_fingerprint: context.fingerprint().into(),
            row_id,
            arity: context.index_count(),
            terms: BTreeMap::new(),
            nonzero_conditions: Vec::new(),
        }
    }

    pub fn row_id(&self) -> &RowId {
        &self.row_id
    }

    pub fn terms(&self) -> &BTreeMap<IndexShift, IndexedCoefficient> {
        &self.terms
    }

    pub fn nonzero_conditions(&self) -> &[ParametricNonZeroCondition] {
        &self.nonzero_conditions
    }

    pub(super) fn validate_context(
        &self,
        context: &IndexedCoefficientContext,
    ) -> Result<(), ParametricRelationError> {
        if self.context_fingerprint.as_ref() == context.fingerprint()
            && self.arity == context.index_count()
        {
            Ok(())
        } else {
            Err(ParametricRelationError::WrongContext)
        }
    }

    pub(super) fn validate_shift(&self, shift: &IndexShift) -> Result<(), ParametricRelationError> {
        if shift.arity() == self.arity {
            Ok(())
        } else {
            Err(ParametricRelationError::WrongArity {
                expected: self.arity,
                actual: shift.arity(),
            })
        }
    }

    pub(super) fn validate_compatible(
        &self,
        other: &Self,
        context: &IndexedCoefficientContext,
    ) -> Result<(), ParametricRelationError> {
        self.validate_context(context)?;
        other.validate_context(context)?;
        if self.family_fingerprint == other.family_fingerprint {
            Ok(())
        } else {
            Err(ParametricRelationError::WrongFamily)
        }
    }
}
