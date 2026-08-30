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
    pub(super) point: Box<[FiniteFieldElement<u64>]>,
    pub(super) matrix: SparseMatrix<Zp64>,
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
        &self.point
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
    ) -> Result<ModularTargetQuery, ModularKernelError> {
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

/// Positive modular discovery evidence.  Exact lift and replay are still
/// required before this may become a closing relation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ModularHit {
    pub(crate) diagnostics: ModularRankDiagnostics,
}

/// A target-local modular no-hit.  This is explicitly inconclusive: another
/// sample, prime, source frame, or exact computation may still find a relation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ModularNoHit {
    pub(crate) diagnostics: ModularRankDiagnostics,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ModularTargetQuery {
    Hit(ModularHit),
    ModularNoHit(ModularNoHit),
}

impl ModularTargetQuery {
    pub(crate) const fn diagnostics(&self) -> &ModularRankDiagnostics {
        match self {
            Self::Hit(hit) => &hit.diagnostics,
            Self::ModularNoHit(no_hit) => &no_hit.diagnostics,
        }
    }
}
