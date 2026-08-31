use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::algebra::IndexedCoefficient;

use super::super::super::condition::ParametricNonZeroCondition;
use super::super::super::relation::{IndexShift, ParametricRelation};
use super::super::super::row::RowId;
use super::super::scope::IbpSourceLayout;

/// One exact displacement in a family's ordered integral-index lattice.
///
/// Construction is fallible and bounded; clones share the retained component
/// buffer. Compatibility with a concrete family is checked when a sealed
/// source batch is translated.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IntegralShift(pub(super) IndexShift);

impl IntegralShift {
    pub fn values(&self) -> &[i64] {
        self.0.values()
    }

    pub fn len(&self) -> usize {
        self.values().len()
    }

    pub fn is_empty(&self) -> bool {
        self.values().is_empty()
    }
}

/// One requested source row at one signed integral-lattice offset.
///
/// Selected batches canonicalize requests offset-major and then by stable
/// source chronology. Exact duplicate pairs are removed before any symbolic
/// translation work.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TranslatedSourceRequest {
    source_ordinal: usize,
    offset: IntegralShift,
}

impl TranslatedSourceRequest {
    pub const fn new(source_ordinal: usize, offset: IntegralShift) -> Self {
        Self {
            source_ordinal,
            offset,
        }
    }

    pub const fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }

    pub const fn offset(&self) -> &IntegralShift {
        &self.offset
    }
}

impl Ord for TranslatedSourceRequest {
    fn cmp(&self, other: &Self) -> Ordering {
        self.offset
            .cmp(&other.offset)
            .then_with(|| self.source_ordinal.cmp(&other.source_ordinal))
    }
}

impl PartialOrd for TranslatedSourceRequest {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Stable identity of one generated source row translated by one exact
/// lattice offset.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TranslatedSourceProvenance {
    pub(super) source_ordinal: usize,
    pub(super) source_row: RowId,
    pub(super) offset: IntegralShift,
}

impl TranslatedSourceProvenance {
    pub fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }

    pub fn source_row(&self) -> &RowId {
        &self.source_row
    }

    pub fn offset(&self) -> &IntegralShift {
        &self.offset
    }

    /// Version-stable identity for diagnostics and future proof payloads.
    pub fn stable_string(&self) -> String {
        let mut output = String::new();
        self.write_stable(&mut output)
            .expect("writing translated-source provenance to String cannot fail");
        output
    }

    fn write_stable(&self, writer: &mut impl fmt::Write) -> fmt::Result {
        write!(writer, "translated-source-v1:{}:", self.source_ordinal)?;
        writer.write_str(&self.source_row.stable_string())?;
        writer.write_str(":[")?;
        for (position, value) in self.offset.values().iter().enumerate() {
            if position != 0 {
                writer.write_str(",")?;
            }
            write!(writer, "{value}")?;
        }
        writer.write_str("]")
    }
}

/// One immutable translated equation with explicit source provenance.
///
/// The underlying mutable/raw relation owner is deliberately not exposed.
/// Callers can inspect its exact sparse terms and inherited nonzero domain,
/// and a later foundry ingress can accept the sealed batch as one owner.
#[derive(Debug, PartialEq, Eq)]
pub struct TranslatedSource {
    pub(super) relation: ParametricRelation,
    pub(super) provenance: TranslatedSourceProvenance,
}

impl TranslatedSource {
    pub fn provenance(&self) -> &TranslatedSourceProvenance {
        &self.provenance
    }

    pub fn row_id(&self) -> &RowId {
        self.provenance.source_row()
    }

    pub fn terms(&self) -> &BTreeMap<IndexShift, IndexedCoefficient> {
        self.relation.terms()
    }

    pub fn nonzero_conditions(&self) -> &[ParametricNonZeroCondition] {
        self.relation.nonzero_conditions()
    }

    pub(crate) fn into_foundry_parts(self) -> (ParametricRelation, TranslatedSourceProvenance) {
        (self.relation, self.provenance)
    }
}

/// Deterministically ordered owner of a complete translated source span.
///
/// Offsets are sorted lexicographically and deduplicated. Within each offset,
/// source rows retain the sealed batch chronology.
#[derive(Debug, PartialEq, Eq)]
pub struct TranslatedSourceBatch {
    pub(super) family_fingerprint: Arc<String>,
    pub(super) context_fingerprint: Arc<String>,
    pub(super) source_layout: IbpSourceLayout,
    pub(super) source_row_count: usize,
    pub(super) offsets: Vec<IntegralShift>,
    pub(super) sources: Vec<TranslatedSource>,
}

impl TranslatedSourceBatch {
    pub fn family_fingerprint(&self) -> &str {
        self.family_fingerprint.as_str()
    }

    pub fn context_fingerprint(&self) -> &str {
        self.context_fingerprint.as_str()
    }

    /// Whether this batch was translated from the complete ordinary
    /// `L * (L + E)` source barrier rather than the deliberately smaller
    /// external-contraction-only layout.
    pub const fn is_complete_ordinary(&self) -> bool {
        matches!(self.source_layout, IbpSourceLayout::CompleteOrdinary)
    }

    pub const fn source_layout_name(&self) -> &'static str {
        self.source_layout.name()
    }

    pub fn source_row_count(&self) -> usize {
        self.source_row_count
    }

    pub fn offsets(&self) -> &[IntegralShift] {
        &self.offsets
    }

    pub fn sources(&self) -> &[TranslatedSource] {
        &self.sources
    }

    pub fn len(&self) -> usize {
        self.sources.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    pub(crate) fn into_foundry_parts(self) -> (Arc<String>, Arc<String>, Vec<TranslatedSource>) {
        (
            self.family_fingerprint,
            self.context_fingerprint,
            self.sources,
        )
    }
}

/// Deterministically ordered sparse selection of translated source rows.
///
/// `requests()[i]` and `sources()[i]` describe the same exact translation.
/// The completed-row count records the source chronology against which every
/// retained source ordinal was validated.
#[derive(Debug, PartialEq, Eq)]
pub struct SelectedTranslatedSourceBatch {
    pub(super) family_fingerprint: Arc<String>,
    pub(super) context_fingerprint: Arc<String>,
    pub(super) source_layout: IbpSourceLayout,
    pub(super) completed_source_row_count: usize,
    pub(super) requests: Vec<TranslatedSourceRequest>,
    pub(super) sources: Vec<TranslatedSource>,
}

impl SelectedTranslatedSourceBatch {
    pub fn family_fingerprint(&self) -> &str {
        self.family_fingerprint.as_str()
    }

    pub fn context_fingerprint(&self) -> &str {
        self.context_fingerprint.as_str()
    }

    /// Whether the selected rows were drawn from the complete ordinary
    /// source chronology. Selection does not upgrade an external-only
    /// source barrier into a complete one.
    pub const fn is_complete_ordinary(&self) -> bool {
        matches!(self.source_layout, IbpSourceLayout::CompleteOrdinary)
    }

    pub const fn source_layout_name(&self) -> &'static str {
        self.source_layout.name()
    }

    pub const fn completed_source_row_count(&self) -> usize {
        self.completed_source_row_count
    }

    pub fn requests(&self) -> &[TranslatedSourceRequest] {
        &self.requests
    }

    pub fn sources(&self) -> &[TranslatedSource] {
        &self.sources
    }

    pub fn len(&self) -> usize {
        self.sources.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    /// Inject one authenticated same-context coefficient without its
    /// denominator gate for a crate-local projection-order mutant test.
    ///
    /// Normal source generation cannot produce this state: relation ingress
    /// always retains the denominator condition. Keeping the seam behind
    /// `cfg(test)` lets discovery tests prove that a term outside sparse
    /// obstruction support is still evaluated, without weakening production
    /// constructors or visibility.
    #[cfg(test)]
    pub(crate) fn replace_term_without_denominator_gate_for_test(
        &mut self,
        context: &crate::algebra::IndexedCoefficientContext,
        selected_source_ordinal: usize,
        term_ordinal: usize,
        coefficient: IndexedCoefficient,
    ) -> Result<(), &'static str> {
        if context.fingerprint() != self.context_fingerprint() {
            return Err("missing-gate mutant coefficient belongs to a foreign context");
        }
        context
            .bind_sealed(&coefficient)
            .map_err(|_| "missing-gate mutant coefficient is not authenticated")?;
        let source = self
            .sources
            .get_mut(selected_source_ordinal)
            .ok_or("missing-gate mutant source ordinal is out of range")?;
        if !source
            .relation
            .replace_term_without_denominator_gate_for_test(term_ordinal, coefficient)
        {
            return Err("missing-gate mutant term ordinal is out of range");
        }
        Ok(())
    }

    /// Crate-test-only provenance mutant used to prove that consumers join
    /// each selected row to the exact request which authorized it. Normal
    /// selected translation constructs both vectors together and cannot
    /// create this state.
    #[cfg(test)]
    pub(crate) fn swap_source_provenance_for_test(&mut self, left: usize, right: usize) -> bool {
        if left >= self.sources.len() || right >= self.sources.len() {
            return false;
        }
        if left == right {
            return true;
        }
        let (left_source, right_source) = if left < right {
            let (before_right, from_right) = self.sources.split_at_mut(right);
            (&mut before_right[left], &mut from_right[0])
        } else {
            let (before_left, from_left) = self.sources.split_at_mut(left);
            (&mut from_left[0], &mut before_left[right])
        };
        std::mem::swap(&mut left_source.provenance, &mut right_source.provenance);
        true
    }

    /// Crate-test-only row-identity mutant used to prove that selected
    /// residual rows retain the sealed completed-source chronology.
    #[cfg(test)]
    pub(crate) fn replace_source_row_id_for_test(
        &mut self,
        selected_source_ordinal: usize,
        row_id: RowId,
    ) -> bool {
        let Some(source) = self.sources.get_mut(selected_source_ordinal) else {
            return false;
        };
        source.provenance.source_row = row_id;
        true
    }

    #[cfg(test)]
    pub(crate) fn into_foundry_parts(
        self,
    ) -> (
        Arc<String>,
        Arc<String>,
        usize,
        Vec<TranslatedSourceRequest>,
        Vec<TranslatedSource>,
    ) {
        (
            self.family_fingerprint,
            self.context_fingerprint,
            self.completed_source_row_count,
            self.requests,
            self.sources,
        )
    }
}
