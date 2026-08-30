use crate::identity::{
    IntegralShift, TranslatedSource, TranslatedSourceBatch, TranslatedSourceProvenance,
};
use crate::sector::Mask;

/// Stable identity and exact provenance for one row in physical-frame order.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct SourceInstanceId {
    total_translation_degree: usize,
    provenance: TranslatedSourceProvenance,
}

impl SourceInstanceId {
    pub(crate) const fn total_translation_degree(&self) -> usize {
        self.total_translation_degree
    }

    pub(crate) const fn provenance(&self) -> &TranslatedSourceProvenance {
        &self.provenance
    }

    pub(crate) fn stable_string(&self) -> String {
        format!(
            "physical-frame-source-v1:{}:{}",
            self.total_translation_degree,
            self.provenance.stable_string()
        )
    }

    pub(super) const fn new(
        total_translation_degree: usize,
        provenance: TranslatedSourceProvenance,
    ) -> Self {
        Self {
            total_translation_degree,
            provenance,
        }
    }
}

/// Immutable raw physical CSR plan for a one-sided sector-chart frame.
///
/// `columns` contains only exact integral shifts. Source provenance and the
/// permutation back into `translated_sources` are separate row sidecars, so
/// no identity-augmentation column can enter the physical pattern.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PhysicalFramePlan {
    sector: Mask,
    degree: usize,
    offsets: Box<[IntegralShift]>,
    columns: Box<[IntegralShift]>,
    row_offsets: Box<[u32]>,
    column_indices: Box<[u32]>,
    source_instances: Box<[SourceInstanceId]>,
    translated_source_indices: Box<[u32]>,
    translated_sources: TranslatedSourceBatch,
}

impl PhysicalFramePlan {
    pub(crate) fn family_fingerprint(&self) -> &str {
        self.translated_sources.family_fingerprint()
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        self.translated_sources.context_fingerprint()
    }

    pub(crate) const fn sector(&self) -> &Mask {
        &self.sector
    }

    pub(crate) const fn degree(&self) -> usize {
        self.degree
    }

    /// One-sided offsets in total-degree, chart-lexicographic order.
    pub(crate) fn offsets(&self) -> &[IntegralShift] {
        &self.offsets
    }

    /// Sorted unique raw physical integral-shift columns.
    pub(crate) fn columns(&self) -> &[IntegralShift] {
        &self.columns
    }

    pub(crate) fn row_offsets(&self) -> &[u32] {
        &self.row_offsets
    }

    pub(crate) fn column_indices(&self) -> &[u32] {
        &self.column_indices
    }

    pub(crate) fn source_instances(&self) -> &[SourceInstanceId] {
        &self.source_instances
    }

    pub(crate) fn row_count(&self) -> usize {
        self.source_instances.len()
    }

    pub(crate) fn entry_count(&self) -> usize {
        self.column_indices.len()
    }

    pub(crate) fn source_for_row(&self, row: usize) -> Option<&TranslatedSource> {
        let source = usize::try_from(*self.translated_source_indices.get(row)?).ok()?;
        self.translated_sources.sources().get(source)
    }

    pub(crate) fn column_indices_for_row(&self, row: usize) -> Option<&[u32]> {
        let lower = usize::try_from(*self.row_offsets.get(row)?).ok()?;
        let upper = usize::try_from(*self.row_offsets.get(row.checked_add(1)?)?).ok()?;
        self.column_indices.get(lower..upper)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_parts(
        sector: Mask,
        degree: usize,
        offsets: Vec<IntegralShift>,
        columns: Vec<IntegralShift>,
        row_offsets: Vec<u32>,
        column_indices: Vec<u32>,
        source_instances: Vec<SourceInstanceId>,
        translated_source_indices: Vec<u32>,
        translated_sources: TranslatedSourceBatch,
    ) -> Self {
        Self {
            sector,
            degree,
            offsets: offsets.into_boxed_slice(),
            columns: columns.into_boxed_slice(),
            row_offsets: row_offsets.into_boxed_slice(),
            column_indices: column_indices.into_boxed_slice(),
            source_instances: source_instances.into_boxed_slice(),
            translated_source_indices: translated_source_indices.into_boxed_slice(),
            translated_sources,
        }
    }
}
