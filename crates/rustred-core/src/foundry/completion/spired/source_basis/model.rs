use std::sync::Arc;

use crate::algebra::{IndexedCoefficient, IndexedPolynomial};
use crate::identity::{CompletedIbpSourceRows, IndexShift, RowId};
use crate::sector::{Mask, OrderingPolicy};

use super::{SpiredSourceBasisError, SpiredSourceBasisLimits};

/// A denominator-free physical entry in one optimized ordinary-source row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredSourceBasisTerm {
    shift: IndexShift,
    coefficient: IndexedPolynomial,
}

impl SpiredSourceBasisTerm {
    pub(crate) const fn shift(&self) -> &IndexShift {
        &self.shift
    }

    pub(crate) const fn coefficient(&self) -> &IndexedPolynomial {
        &self.coefficient
    }

    pub(super) const fn new(shift: IndexShift, coefficient: IndexedPolynomial) -> Self {
        Self { shift, coefficient }
    }
}

/// One exact polynomial multiplier of a generated ordinary source row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredSourceBasisProvenanceTerm {
    source_ordinal: usize,
    source_row: RowId,
    coefficient: IndexedPolynomial,
}

impl SpiredSourceBasisProvenanceTerm {
    pub(crate) const fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }

    pub(crate) const fn source_row(&self) -> &RowId {
        &self.source_row
    }

    pub(crate) const fn coefficient(&self) -> &IndexedPolynomial {
        &self.coefficient
    }

    pub(super) const fn new(
        source_ordinal: usize,
        source_row: RowId,
        coefficient: IndexedPolynomial,
    ) -> Self {
        Self {
            source_ordinal,
            source_row,
            coefficient,
        }
    }
}

/// One coefficient expressing an original row in the optimized basis over
/// the generic rational-function field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredRawSourceReconstructionTerm {
    basis_row_ordinal: usize,
    coefficient: IndexedCoefficient,
}

impl SpiredRawSourceReconstructionTerm {
    pub(crate) const fn basis_row_ordinal(&self) -> usize {
        self.basis_row_ordinal
    }

    pub(crate) const fn coefficient(&self) -> &IndexedCoefficient {
        &self.coefficient
    }

    pub(super) const fn new(basis_row_ordinal: usize, coefficient: IndexedCoefficient) -> Self {
        Self {
            basis_row_ordinal,
            coefficient,
        }
    }
}

/// One denominator-cleared, deliberately non-unitarized RREF row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredSourceBasisRow {
    pivot_column: usize,
    terms: Box<[SpiredSourceBasisTerm]>,
    provenance: Box<[SpiredSourceBasisProvenanceTerm]>,
}

impl SpiredSourceBasisRow {
    pub(crate) const fn pivot_column(&self) -> usize {
        self.pivot_column
    }

    pub(crate) fn terms(&self) -> &[SpiredSourceBasisTerm] {
        &self.terms
    }

    pub(crate) fn provenance(&self) -> &[SpiredSourceBasisProvenanceTerm] {
        &self.provenance
    }

    pub(crate) fn pivot_coefficient(&self) -> &IndexedPolynomial {
        self.terms
            .first()
            .expect("an authenticated source-basis row has a physical pivot")
            .coefficient()
    }

    pub(super) fn new(
        pivot_column: usize,
        terms: Vec<SpiredSourceBasisTerm>,
        provenance: Vec<SpiredSourceBasisProvenanceTerm>,
    ) -> Self {
        Self {
            pivot_column,
            terms: terms.into_boxed_slice(),
            provenance: provenance.into_boxed_slice(),
        }
    }
}

/// Why the optimized basis cannot replace fair raw-source enumeration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpiredSourceBasisSpecializationPolicy {
    /// Generic fraction-field RREF can lose rank on exceptional index fibres.
    /// Optimized rows are an acceleration prefix; raw ordinary rows remain a
    /// fair eventual-discovery fallback.
    RawOrdinaryFallbackRequired,
}

/// Exact sector/order-local source preconditioner.
///
/// This object is proposal-only.  Its forward provenance proves every basis
/// row is an ordinary-source consequence; its reverse transform proves equal
/// generic row span.  Neither proof makes the transform specialization-safe.
#[derive(Debug)]
pub(crate) struct SpiredSourceBasis {
    source_identity: Arc<()>,
    sector: Mask,
    ordering: OrderingPolicy,
    columns: Box<[IndexShift]>,
    independent_source_ordinals: Box<[usize]>,
    rows: Box<[SpiredSourceBasisRow]>,
    raw_reconstruction: Box<[Box<[SpiredRawSourceReconstructionTerm]>]>,
    dropped_dependent_rows: usize,
    specialization_policy: SpiredSourceBasisSpecializationPolicy,
}

impl SpiredSourceBasis {
    /// Sector used only to order the complete, unpruned ordinary-source
    /// columns.  No target bound or coordinate-face constraint is compiled
    /// into this basis.
    pub(crate) const fn sector(&self) -> &Mask {
        &self.sector
    }

    pub(crate) const fn ordering(&self) -> OrderingPolicy {
        self.ordering
    }

    pub(crate) fn columns(&self) -> &[IndexShift] {
        &self.columns
    }

    pub(crate) fn independent_source_ordinals(&self) -> &[usize] {
        &self.independent_source_ordinals
    }

    pub(crate) fn rows(&self) -> &[SpiredSourceBasisRow] {
        &self.rows
    }

    pub(crate) fn raw_reconstruction(
        &self,
        source_ordinal: usize,
    ) -> Option<&[SpiredRawSourceReconstructionTerm]> {
        self.raw_reconstruction.get(source_ordinal).map(Box::as_ref)
    }

    pub(crate) const fn dropped_dependent_rows(&self) -> usize {
        self.dropped_dependent_rows
    }

    pub(crate) const fn specialization_policy(&self) -> SpiredSourceBasisSpecializationPolicy {
        self.specialization_policy
    }

    pub(crate) const fn requires_raw_ordinary_fallback(&self) -> bool {
        matches!(
            self.specialization_policy,
            SpiredSourceBasisSpecializationPolicy::RawOrdinaryFallbackRequired
        )
    }

    pub(crate) fn try_verify_translated_provenance(
        &self,
        context: &crate::algebra::IndexedCoefficientContext,
        sources: &CompletedIbpSourceRows,
        offset: &[i64],
        limits: SpiredSourceBasisLimits,
    ) -> Result<(), SpiredSourceBasisError> {
        super::replay::try_verify_translated_basis(self, context, sources, offset, limits)
    }

    pub(super) fn new(
        source_identity: Arc<()>,
        sector: Mask,
        ordering: OrderingPolicy,
        columns: Vec<IndexShift>,
        independent_source_ordinals: Vec<usize>,
        rows: Vec<SpiredSourceBasisRow>,
        raw_reconstruction: Vec<Box<[SpiredRawSourceReconstructionTerm]>>,
        dropped_dependent_rows: usize,
    ) -> Self {
        Self {
            source_identity,
            sector,
            ordering,
            columns: columns.into_boxed_slice(),
            independent_source_ordinals: independent_source_ordinals.into_boxed_slice(),
            rows: rows.into_boxed_slice(),
            raw_reconstruction: raw_reconstruction.into_boxed_slice(),
            dropped_dependent_rows,
            specialization_policy:
                SpiredSourceBasisSpecializationPolicy::RawOrdinaryFallbackRequired,
        }
    }

    pub(super) fn owns_sources(&self, sources: &CompletedIbpSourceRows) -> bool {
        sources.owns_identity(&self.source_identity)
    }
}
