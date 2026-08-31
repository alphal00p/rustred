use std::sync::Arc;

use crate::identity::{IntegralShift, TranslatedSource, TranslatedSourceProvenance};
use crate::sector::Mask;

/// Stable identity and exact provenance for one row in physical-frame order.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct SourceInstanceId {
    provenance: TranslatedSourceProvenance,
}

impl SourceInstanceId {
    pub(crate) const fn provenance(&self) -> &TranslatedSourceProvenance {
        &self.provenance
    }

    pub(crate) fn stable_string(&self) -> String {
        format!(
            "physical-frame-source-v2:{}",
            self.provenance.stable_string()
        )
    }

    pub(super) const fn new(provenance: TranslatedSourceProvenance) -> Self {
        Self { provenance }
    }
}

/// Immutable construction-neutral raw physical CSR plan.
///
/// `columns` contains only exact integral shifts. Source provenance and the
/// permutation back into `translated_sources` are separate row sidecars. No
/// chart metadata, target, symmetry representative, or identity-augmentation
/// column can enter the physical pattern.
#[derive(Clone, Debug)]
pub(crate) struct PhysicalFramePlanIdentity(Arc<()>);

impl PhysicalFramePlanIdentity {
    pub(super) fn fresh() -> Self {
        Self(Arc::new(()))
    }

    pub(crate) fn belongs_to(&self, plan: &PhysicalFramePlan) -> bool {
        Arc::ptr_eq(&self.0, &plan.identity.0)
    }
}

#[derive(Debug)]
pub(crate) struct PhysicalFramePlan {
    /// Unforgeable in-memory identity for proof objects compiled against this
    /// exact plan. Structural equality deliberately ignores this token.
    identity: PhysicalFramePlanIdentity,
    family_fingerprint: Arc<String>,
    context_fingerprint: Arc<String>,
    sector: Mask,
    columns: Box<[IntegralShift]>,
    row_offsets: Box<[u32]>,
    column_indices: Box<[u32]>,
    source_instances: Box<[SourceInstanceId]>,
    translated_source_indices: Box<[u32]>,
    translated_sources: Box<[TranslatedSource]>,
}

impl PhysicalFramePlan {
    pub(crate) fn identity_owner(&self) -> PhysicalFramePlanIdentity {
        self.identity.clone()
    }

    pub(crate) fn family_fingerprint(&self) -> &str {
        self.family_fingerprint.as_str()
    }

    pub(crate) fn family_fingerprint_owner(&self) -> Arc<String> {
        self.family_fingerprint.clone()
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        self.context_fingerprint.as_str()
    }

    pub(crate) fn context_fingerprint_owner(&self) -> Arc<String> {
        self.context_fingerprint.clone()
    }

    pub(crate) const fn sector(&self) -> &Mask {
        &self.sector
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
        self.translated_sources.get(source)
    }

    pub(crate) fn column_indices_for_row(&self, row: usize) -> Option<&[u32]> {
        let lower = usize::try_from(*self.row_offsets.get(row)?).ok()?;
        let upper = usize::try_from(*self.row_offsets.get(row.checked_add(1)?)?).ok()?;
        self.column_indices.get(lower..upper)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_parts(
        family_fingerprint: Arc<String>,
        context_fingerprint: Arc<String>,
        sector: Mask,
        columns: Vec<IntegralShift>,
        row_offsets: Vec<u32>,
        column_indices: Vec<u32>,
        source_instances: Vec<SourceInstanceId>,
        translated_source_indices: Vec<u32>,
        translated_sources: Vec<TranslatedSource>,
    ) -> Self {
        Self {
            identity: PhysicalFramePlanIdentity::fresh(),
            family_fingerprint,
            context_fingerprint,
            sector,
            columns: columns.into_boxed_slice(),
            row_offsets: row_offsets.into_boxed_slice(),
            column_indices: column_indices.into_boxed_slice(),
            source_instances: source_instances.into_boxed_slice(),
            translated_source_indices: translated_source_indices.into_boxed_slice(),
            translated_sources: translated_sources.into_boxed_slice(),
        }
    }
}

impl PartialEq for PhysicalFramePlan {
    fn eq(&self, other: &Self) -> bool {
        self.family_fingerprint == other.family_fingerprint
            && self.context_fingerprint == other.context_fingerprint
            && self.sector == other.sector
            && self.columns == other.columns
            && self.row_offsets == other.row_offsets
            && self.column_indices == other.column_indices
            && self.source_instances == other.source_instances
            && self.translated_source_indices == other.translated_source_indices
            && self.translated_sources == other.translated_sources
    }
}

impl Eq for PhysicalFramePlan {}

/// Construction shell for the rectangular one-sided chart
/// `M_degree(sector)`.
///
/// The chart schedule stays outside [`PhysicalFramePlan`], so exact and
/// modular consumers can accept plans built by other bounded source
/// selections without learning chart-specific state.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct OneSidedChartFrame {
    plan: PhysicalFramePlan,
    degree: usize,
    offsets: Box<[IntegralShift]>,
}

impl OneSidedChartFrame {
    pub(crate) const fn plan(&self) -> &PhysicalFramePlan {
        &self.plan
    }

    pub(crate) fn into_plan(self) -> PhysicalFramePlan {
        self.plan
    }

    pub(crate) const fn degree(&self) -> usize {
        self.degree
    }

    /// One-sided offsets in total-degree, chart-lexicographic order.
    pub(crate) fn offsets(&self) -> &[IntegralShift] {
        &self.offsets
    }

    pub(super) fn from_parts(
        plan: PhysicalFramePlan,
        degree: usize,
        offsets: Vec<IntegralShift>,
    ) -> Self {
        Self {
            plan,
            degree,
            offsets: offsets.into_boxed_slice(),
        }
    }
}

/// Construction shell for one sparse, explicitly selected source batch.
///
/// The completed-row count is retained only as source-chronology metadata;
/// the physical plan owns exactly the selected translated rows and no
/// Cartesian offset-by-source completion.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SelectedSourceFrame {
    plan: PhysicalFramePlan,
    completed_source_row_count: usize,
}

impl SelectedSourceFrame {
    pub(crate) const fn plan(&self) -> &PhysicalFramePlan {
        &self.plan
    }

    pub(crate) fn into_plan(self) -> PhysicalFramePlan {
        self.plan
    }

    pub(crate) const fn completed_source_row_count(&self) -> usize {
        self.completed_source_row_count
    }

    pub(super) const fn from_parts(
        plan: PhysicalFramePlan,
        completed_source_row_count: usize,
    ) -> Self {
        Self {
            plan,
            completed_source_row_count,
        }
    }
}
