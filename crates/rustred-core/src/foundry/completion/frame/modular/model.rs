use std::sync::Arc;

use symbolica::domains::finite_field::{FiniteFieldElement, Zp64};
use symbolica::tensors::sparse::SparseMatrix;

use super::super::{PhysicalFramePlan, SourceInstanceId};
use super::{ModularKernelError, ModularKernelLimits};

/// One admitted modular image of an exact physical frame.
///
/// Rows retain the exact frame chronology, including rows which specialize to
/// zero.  The borrowed plan therefore remains the authority for every raw
/// [`SourceInstanceId`] needed by a later exact lift.
#[derive(Debug)]
pub(crate) struct ModularPhysicalFrame<'frame> {
    pub(super) plan: &'frame PhysicalFramePlan,
    pub(super) field: Zp64,
    pub(super) sample: Arc<ModularSampleFingerprint>,
    pub(super) matrix: SparseMatrix<Zp64>,
}

/// Immutable identity of one admitted modular evaluation point.
///
/// The point stores the ordered base-parameter residues followed by the
/// mapped integral-index residues. Sharing this owner between the sampled
/// frame and every query result prevents a hit from losing the exact sample
/// at which its independent-row chronology was discovered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ModularSampleFingerprint {
    modulus: u64,
    point: Arc<[FiniteFieldElement<u64>]>,
}

impl ModularSampleFingerprint {
    pub(crate) const fn modulus(&self) -> u64 {
        self.modulus
    }

    pub(crate) fn point(&self) -> &[FiniteFieldElement<u64>] {
        &self.point
    }

    pub(super) fn new(modulus: u64, point: Box<[FiniteFieldElement<u64>]>) -> Self {
        Self {
            modulus,
            point: Arc::from(point),
        }
    }
}

impl<'frame> ModularPhysicalFrame<'frame> {
    pub(crate) const fn plan(&self) -> &'frame PhysicalFramePlan {
        self.plan
    }

    pub(crate) const fn field(&self) -> &Zp64 {
        &self.field
    }

    /// Ordered base-parameter values followed by mapped integral indices.
    pub(crate) fn point(&self) -> &[FiniteFieldElement<u64>] {
        self.sample.point()
    }

    pub(crate) fn sample_fingerprint(&self) -> &Arc<ModularSampleFingerprint> {
        &self.sample
    }

    pub(crate) const fn matrix(&self) -> &SparseMatrix<Zp64> {
        &self.matrix
    }

    pub(crate) fn source_instances(&self) -> &[SourceInstanceId] {
        self.plan.source_instances()
    }

    pub(crate) fn query_target(
        &self,
        target_column: usize,
        forbidden_columns: &[usize],
        limits: ModularKernelLimits,
    ) -> Result<ModularTargetQuery<'frame>, ModularKernelError> {
        super::rank::query_target(self, target_column, forbidden_columns, limits)
    }
}

/// Deterministic diagnostics from the two target-local physical rank probes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ModularRankDiagnostics {
    pub(crate) target_column: usize,
    pub(crate) forbidden_columns: Box<[usize]>,
    pub(crate) forbidden_rank: usize,
    pub(crate) augmented_rank: usize,
    pub(crate) forbidden_pivot_columns: Box<[usize]>,
    pub(crate) augmented_pivot_columns: Box<[usize]>,
    /// Original exact-frame row ordinals accepted by the forbidden reducer.
    pub(crate) forbidden_independent_source_rows: Box<[usize]>,
    /// Original exact-frame row ordinals accepted by the augmented reducer.
    /// These map directly into [`ModularPhysicalFrame::source_instances`].
    pub(crate) augmented_independent_source_rows: Box<[usize]>,
    pub(crate) forbidden_input_nonzeros: usize,
    pub(crate) augmented_input_nonzeros: usize,
    pub(crate) forbidden_lower_pattern_nonzeros: usize,
    pub(crate) augmented_lower_pattern_nonzeros: usize,
    pub(crate) forbidden_upper_nonzeros: usize,
    pub(crate) augmented_upper_nonzeros: usize,
    pub(crate) forbidden_total_fill_nonzeros: usize,
    pub(crate) augmented_total_fill_nonzeros: usize,
}

/// One nonzero entry of a target-normalized modular right obstruction.
///
/// Columns use the query-local logical order: canonical forbidden columns
/// first and the target last.  This is discovery evidence only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ModularObstructionEntry {
    pub(super) logical_column: usize,
    pub(super) coefficient: FiniteFieldElement<u64>,
}

impl ModularObstructionEntry {
    pub(crate) const fn logical_column(&self) -> usize {
        self.logical_column
    }

    pub(crate) const fn coefficient(&self) -> &FiniteFieldElement<u64> {
        &self.coefficient
    }

    pub(super) const fn new(logical_column: usize, coefficient: FiniteFieldElement<u64>) -> Self {
        Self {
            logical_column,
            coefficient,
        }
    }
}

/// Checked target-local right-nullspace evidence for one modular no-hit.
///
/// `logical_physical_columns` is exactly the canonical forbidden physical
/// columns followed by the target physical column.  Construction is private
/// to the checked obstruction service, which verifies a sparse `q` with
/// `q_target = 1` and `A q = 0` against the sampled projected matrix.  The
/// result cannot authorize an exact rule, owner, terminal, or closure claim.
#[derive(Clone, Debug)]
pub(crate) struct ModularRightObstruction<'frame> {
    plan: &'frame PhysicalFramePlan,
    sample: Arc<ModularSampleFingerprint>,
    diagnostics: ModularRankDiagnostics,
    logical_physical_columns: Box<[usize]>,
    entries: Box<[ModularObstructionEntry]>,
}

impl<'frame> ModularRightObstruction<'frame> {
    pub(crate) const fn plan(&self) -> &'frame PhysicalFramePlan {
        self.plan
    }

    pub(crate) const fn diagnostics(&self) -> &ModularRankDiagnostics {
        &self.diagnostics
    }

    pub(crate) const fn sample_fingerprint(&self) -> &Arc<ModularSampleFingerprint> {
        &self.sample
    }

    /// Canonical forbidden physical columns followed by the physical target.
    pub(crate) fn logical_physical_columns(&self) -> &[usize] {
        &self.logical_physical_columns
    }

    pub(crate) fn logical_forbidden_columns(&self) -> &[usize] {
        &self.logical_physical_columns[..self.logical_physical_columns.len() - 1]
    }

    pub(crate) const fn target_logical_column(&self) -> usize {
        self.logical_physical_columns.len() - 1
    }

    pub(crate) fn target_physical_column(&self) -> usize {
        self.logical_physical_columns[self.target_logical_column()]
    }

    pub(crate) fn entries(&self) -> &[ModularObstructionEntry] {
        &self.entries
    }

    pub(super) fn from_checked_parts(
        plan: &'frame PhysicalFramePlan,
        sample: Arc<ModularSampleFingerprint>,
        diagnostics: ModularRankDiagnostics,
        logical_physical_columns: Vec<usize>,
        entries: Vec<ModularObstructionEntry>,
    ) -> Self {
        Self {
            plan,
            sample,
            diagnostics,
            logical_physical_columns: logical_physical_columns.into_boxed_slice(),
            entries: entries.into_boxed_slice(),
        }
    }
}

impl PartialEq for ModularRightObstruction<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.plan, other.plan)
            && self.sample == other.sample
            && self.diagnostics == other.diagnostics
            && self.logical_physical_columns == other.logical_physical_columns
            && self.entries == other.entries
    }
}

impl Eq for ModularRightObstruction<'_> {}

/// Positive modular discovery evidence.  Exact lift and replay are still
/// required before this may become a closing relation.
#[derive(Clone, Debug)]
pub(crate) struct ModularHit<'frame> {
    plan: &'frame PhysicalFramePlan,
    sample: Arc<ModularSampleFingerprint>,
    pub(crate) diagnostics: ModularRankDiagnostics,
}

impl<'frame> ModularHit<'frame> {
    pub(crate) const fn plan(&self) -> &'frame PhysicalFramePlan {
        self.plan
    }

    pub(crate) fn sample_fingerprint(&self) -> &Arc<ModularSampleFingerprint> {
        &self.sample
    }

    pub(crate) const fn diagnostics(&self) -> &ModularRankDiagnostics {
        &self.diagnostics
    }

    pub(super) fn new(
        plan: &'frame PhysicalFramePlan,
        sample: Arc<ModularSampleFingerprint>,
        diagnostics: ModularRankDiagnostics,
    ) -> Self {
        Self {
            plan,
            sample,
            diagnostics,
        }
    }
}

impl PartialEq for ModularHit<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.plan, other.plan)
            && self.sample == other.sample
            && self.diagnostics == other.diagnostics
    }
}

impl Eq for ModularHit<'_> {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ModularTargetQuery<'frame> {
    Hit(ModularHit<'frame>),
    NoHitWithObstruction(ModularRightObstruction<'frame>),
}

impl<'frame> ModularTargetQuery<'frame> {
    pub(crate) const fn diagnostics(&self) -> &ModularRankDiagnostics {
        match self {
            Self::Hit(hit) => &hit.diagnostics,
            Self::NoHitWithObstruction(obstruction) => obstruction.diagnostics(),
        }
    }

    pub(crate) const fn obstruction(&self) -> Option<&ModularRightObstruction<'frame>> {
        match self {
            Self::Hit(_) => None,
            Self::NoHitWithObstruction(obstruction) => Some(obstruction),
        }
    }
}
