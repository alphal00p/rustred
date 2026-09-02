use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::identity::{CompletedIbpSourceRows, RowId};

use super::super::error::try_vec;
use super::super::{ForwardShift, InvolutiveLimits, OreConsequence, OreOrderingAdapter, OreRow};
use super::lift::{build_lifted_source, preflight_relation};
use super::{OrdinaryChartLiftError, OrdinaryChartLiftLimits};

/// One exact source row after its minimal common left shift has moved every
/// physical integral displacement into the sector-forward monoid.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LiftedOrdinarySource {
    pub(super) source_ordinal: usize,
    pub(super) source_row: RowId,
    pub(super) left_shift: ForwardShift,
    pub(super) consequence: OreConsequence,
}

impl LiftedOrdinarySource {
    pub(crate) fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }

    pub(crate) fn source_row(&self) -> &RowId {
        &self.source_row
    }

    pub(crate) fn left_shift(&self) -> &ForwardShift {
        &self.left_shift
    }

    pub(crate) fn row(&self) -> &OreRow {
        self.consequence.row()
    }

    pub(crate) fn consequence(&self) -> &OreConsequence {
        &self.consequence
    }

    /// Enter Janet/Ore completion with the exact source-module witness
    /// `E^left_shift P_source`.  This remains proposal-only; it grants no
    /// rule-owner, guard, descent, or publication authority.
    pub(crate) fn into_consequence(self) -> OreConsequence {
        self.consequence
    }
}

/// One source-owner-bound, deterministic chart lift of the complete ordinary
/// module.  The opaque owner seal prevents sparse source ordinals from being
/// replayed against an equivalent-looking but different execution transcript.
#[derive(Debug)]
pub(crate) struct LiftedOrdinarySourceBatch {
    pub(super) completed_owner: Arc<()>,
    pub(super) sources: Box<[LiftedOrdinarySource]>,
}

impl PartialEq for LiftedOrdinarySourceBatch {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.completed_owner, &other.completed_owner) && self.sources == other.sources
    }
}

impl Eq for LiftedOrdinarySourceBatch {}

impl LiftedOrdinarySourceBatch {
    pub(crate) fn sources(&self) -> &[LiftedOrdinarySource] {
        &self.sources
    }

    pub(crate) fn len(&self) -> usize {
        self.sources.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    pub(crate) fn try_replay_source(
        &self,
        source_ordinal: usize,
        completed: &CompletedIbpSourceRows,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: OrdinaryChartLiftLimits,
    ) -> Result<LiftedOrdinarySource, OrdinaryChartLiftError> {
        if !completed.owns_identity(&self.completed_owner) {
            return Err(OrdinaryChartLiftError::ForeignSourceOwner);
        }
        if !ordering.owns_completed_source_module(completed) {
            return Err(OrdinaryChartLiftError::ForeignSourceOwner);
        }
        let retained = self.sources.get(source_ordinal).ok_or(
            OrdinaryChartLiftError::SourceOrdinalOutOfRange {
                source_ordinal,
                source_rows: self.sources.len(),
            },
        )?;
        retained
            .consequence
            .try_validate(ordering, context, limits.involutive)?;
        let relation = completed.source_relation(source_ordinal).ok_or(
            OrdinaryChartLiftError::SourceOrdinalOutOfRange {
                source_ordinal,
                source_rows: completed.source_row_count(),
            },
        )?;
        if relation.row_id() != retained.source_row() {
            return Err(OrdinaryChartLiftError::SourceRowMismatch { source_ordinal });
        }
        preflight_relation(relation, source_ordinal, ordering, context, limits)?;
        let replayed = build_lifted_source(
            relation,
            source_ordinal,
            ordering,
            context,
            limits.involutive,
        )?;
        if replayed.left_shift != retained.left_shift || replayed.source_row != retained.source_row
        {
            return Err(OrdinaryChartLiftError::SourceRowMismatch { source_ordinal });
        }
        Ok(replayed)
    }

    pub(crate) fn try_into_consequences(
        self,
        completed: &CompletedIbpSourceRows,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
    ) -> Result<Box<[OreConsequence]>, OrdinaryChartLiftError> {
        if !completed.owns_identity(&self.completed_owner)
            || !ordering.owns_completed_source_module(completed)
        {
            return Err(OrdinaryChartLiftError::ForeignSourceOwner);
        }
        for source in self.sources.iter() {
            source.consequence.try_validate(ordering, context, limits)?;
        }
        let mut consequences = try_vec("lifted ordinary Ore consequences", self.sources.len())?;
        for source in self.sources.into_vec() {
            consequences.push(source.into_consequence());
        }
        Ok(consequences.into_boxed_slice())
    }
}
