use std::sync::Arc;

use crate::foundry::completion::UncoveredPartition;
use crate::identity::IntegralShift;
use crate::sector::Mask;

use super::BoundarySimplexPlanError;

/// Borrowed input to one frozen boundary-simplex planning epoch.
///
/// The stable key must bind every caller-side choice affecting the partition's
/// meaning, including ordering and immutable predecessor authority. The
/// planner retains the sector separately and rejects duplicate keys.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BoundarySimplexScopePartition<'a> {
    pub(crate) stable_scope_key: &'a str,
    pub(crate) sector: &'a Mask,
    pub(crate) uncovered: &'a UncoveredPartition,
}

impl<'a> BoundarySimplexScopePartition<'a> {
    pub(crate) const fn new(
        stable_scope_key: &'a str,
        sector: &'a Mask,
        uncovered: &'a UncoveredPartition,
    ) -> Self {
        Self {
            stable_scope_key,
            sector,
            uncovered,
        }
    }
}

/// Algebraic sampling profile on the unpinned axes of a boundary face.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum BoundarySimplexSamplingProfile {
    /// Positive-margin complete simplex through one total-degree ceiling.
    Simplex {
        interior_margin: u64,
        polynomial_degree_ceiling: usize,
    },
    /// The unique all-pinned point of a zero-dimensional face.
    Vertex,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct BoundarySimplexScopeKey {
    stable_scope_key: Arc<String>,
    sector: Mask,
}

impl BoundarySimplexScopeKey {
    pub(super) const fn new(stable_scope_key: Arc<String>, sector: Mask) -> Self {
        Self {
            stable_scope_key,
            sector,
        }
    }

    pub(crate) fn stable_scope_key(&self) -> &str {
        self.stable_scope_key.as_str()
    }

    pub(crate) const fn sector(&self) -> &Mask {
        &self.sector
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct BoundarySimplexParentBoxKey {
    lower: Arc<Vec<u64>>,
    upper: Arc<Vec<Option<u64>>>,
}

impl BoundarySimplexParentBoxKey {
    pub(super) fn new(lower: Vec<u64>, upper: Vec<Option<u64>>) -> Self {
        Self {
            lower: Arc::new(lower),
            upper: Arc::new(upper),
        }
    }

    pub(crate) fn lower(&self) -> &[u64] {
        self.lower.as_slice()
    }

    pub(crate) fn upper(&self) -> &[Option<u64>] {
        self.upper.as_slice()
    }

    pub(crate) fn arity(&self) -> usize {
        self.lower.len()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct BoundarySimplexFaceKey {
    parent: BoundarySimplexParentBoxKey,
    pinned_axes: Arc<Vec<usize>>,
    remaining_axes: Arc<Vec<usize>>,
}

impl BoundarySimplexFaceKey {
    pub(super) fn new(
        parent: BoundarySimplexParentBoxKey,
        pinned_axes: Vec<usize>,
        remaining_axes: Vec<usize>,
    ) -> Self {
        Self {
            parent,
            pinned_axes: Arc::new(pinned_axes),
            remaining_axes: Arc::new(remaining_axes),
        }
    }

    pub(crate) const fn parent(&self) -> &BoundarySimplexParentBoxKey {
        &self.parent
    }

    pub(crate) fn pinned_axes(&self) -> &[usize] {
        self.pinned_axes.as_slice()
    }

    pub(crate) fn remaining_axes(&self) -> &[usize] {
        self.remaining_axes.as_slice()
    }
}

/// Stable, epoch-independent identity of one face/assignment/simplex target.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct BoundarySimplexTaskKey {
    scope: BoundarySimplexScopeKey,
    face: BoundarySimplexFaceKey,
    parent_free_dimension: usize,
    boundary_codimension: usize,
    profile: BoundarySimplexSamplingProfile,
    simplex_offset: Arc<Vec<u64>>,
    finite_assignment_ordinal: usize,
}

impl BoundarySimplexTaskKey {
    #[allow(clippy::too_many_arguments)]
    pub(super) const fn new(
        scope: BoundarySimplexScopeKey,
        face: BoundarySimplexFaceKey,
        parent_free_dimension: usize,
        boundary_codimension: usize,
        profile: BoundarySimplexSamplingProfile,
        simplex_offset: Arc<Vec<u64>>,
        finite_assignment_ordinal: usize,
    ) -> Self {
        Self {
            scope,
            face,
            parent_free_dimension,
            boundary_codimension,
            profile,
            simplex_offset,
            finite_assignment_ordinal,
        }
    }

    pub(crate) fn stable_scope_key(&self) -> &str {
        self.scope.stable_scope_key()
    }

    pub(crate) const fn sector(&self) -> &Mask {
        self.scope.sector()
    }

    /// Exact parent partition box, not a synthetic cover mutation.
    pub(crate) fn parent_box_lower(&self) -> &[u64] {
        self.face.parent().lower()
    }

    /// Exact parent partition box, not a synthetic cover mutation.
    pub(crate) fn parent_box_upper(&self) -> &[Option<u64>] {
        self.face.parent().upper()
    }

    pub(crate) const fn parent_free_dimension(&self) -> usize {
        self.parent_free_dimension
    }

    pub(crate) const fn boundary_codimension(&self) -> usize {
        self.boundary_codimension
    }

    pub(crate) const fn face_dimension(&self) -> usize {
        self.parent_free_dimension - self.boundary_codimension
    }

    pub(crate) fn pinned_axes(&self) -> &[usize] {
        self.face.pinned_axes()
    }

    pub(crate) fn remaining_axes(&self) -> &[usize] {
        self.face.remaining_axes()
    }

    pub(crate) const fn profile(&self) -> BoundarySimplexSamplingProfile {
        self.profile
    }

    /// Offset coordinates are indexed by ascending remaining-axis rank.
    /// Vertex tasks retain the unique empty offset.
    pub(crate) fn simplex_offset(&self) -> &[u64] {
        self.simplex_offset.as_slice()
    }

    /// Lexicographic mixed-radix ordinal over the parent's original finite
    /// axes. The last finite ambient axis varies fastest.
    pub(crate) const fn finite_assignment_ordinal(&self) -> usize {
        self.finite_assignment_ordinal
    }
}

#[derive(Clone, Debug)]
pub(super) struct BoundarySimplexGeometryEpochIdentity(Arc<()>);

impl BoundarySimplexGeometryEpochIdentity {
    pub(super) fn fresh() -> Self {
        Self(Arc::new(()))
    }

    fn belongs_to(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// One checked target proposal from a frozen face-geometry epoch.
#[derive(Clone, Debug)]
pub(crate) struct BoundarySimplexTask {
    epoch_identity: BoundarySimplexGeometryEpochIdentity,
    epoch_ordinal: u64,
    canonical_ordinal: usize,
    key: BoundarySimplexTaskKey,
    lattice_target: Arc<Vec<u64>>,
    target_shift: IntegralShift,
}

impl BoundarySimplexTask {
    pub(super) fn new(
        epoch_identity: BoundarySimplexGeometryEpochIdentity,
        epoch_ordinal: u64,
        canonical_ordinal: usize,
        key: BoundarySimplexTaskKey,
        lattice_target: Vec<u64>,
        target_shift: IntegralShift,
    ) -> Self {
        Self {
            epoch_identity,
            epoch_ordinal,
            canonical_ordinal,
            key,
            lattice_target: Arc::new(lattice_target),
            target_shift,
        }
    }

    pub(crate) const fn epoch_ordinal(&self) -> u64 {
        self.epoch_ordinal
    }

    pub(crate) const fn canonical_ordinal(&self) -> usize {
        self.canonical_ordinal
    }

    pub(crate) const fn key(&self) -> &BoundarySimplexTaskKey {
        &self.key
    }

    pub(crate) fn lattice_target(&self) -> &[u64] {
        self.lattice_target.as_slice()
    }

    /// Canonical coefficient-evaluation anchor for this face proposal.
    ///
    /// The lattice target parameterizes the pivot shift, not the coefficient
    /// sample. Complementary face axes evaluate the base index at the sector
    /// corner (chart zero); each remaining symbolic axis uses the first
    /// interior chart point. Task-relative probe offsets may move only those
    /// symbolic coordinates. This canonical origin reproduces the established
    /// K=6 mixed-dot-ray sample without coupling the sample to the pivot size.
    pub(crate) fn base_probe_chart_origin(&self) -> impl ExactSizeIterator<Item = u64> + '_ {
        self.lattice_target.iter().enumerate().map(|(position, _)| {
            if self.key.remaining_axes().binary_search(&position).is_ok() {
                1
            } else {
                0
            }
        })
    }

    pub(crate) const fn target_shift(&self) -> &IntegralShift {
        &self.target_shift
    }
}

/// Immutable complete face × finite-assignment × simplex proposal design.
#[derive(Debug)]
pub(crate) struct BoundarySimplexPlan {
    pub(super) epoch_identity: BoundarySimplexGeometryEpochIdentity,
    pub(super) epoch_ordinal: u64,
    pub(super) input_scope_count: usize,
    pub(super) selected_scope_count: usize,
    pub(super) selected_parent_box_count: usize,
    pub(super) boundary_face_count: usize,
    pub(super) parent_finite_assignment_count: usize,
    pub(super) face_finite_assignment_count: usize,
    pub(super) scheduler_workspace_entries: usize,
    pub(super) scheduler_visit_count: usize,
    pub(super) subset_unrank_work_upper_bound: usize,
    pub(super) parent_free_dimension: usize,
    pub(super) boundary_codimension: usize,
    pub(super) face_dimension: usize,
    pub(super) maximal_available_free_dimension: usize,
    pub(super) profile: BoundarySimplexSamplingProfile,
    pub(super) simplex_sample_count: usize,
    pub(super) tasks: Vec<BoundarySimplexTask>,
}

impl BoundarySimplexPlan {
    pub(crate) const fn epoch_ordinal(&self) -> u64 {
        self.epoch_ordinal
    }

    pub(crate) const fn input_scope_count(&self) -> usize {
        self.input_scope_count
    }

    pub(crate) const fn selected_scope_count(&self) -> usize {
        self.selected_scope_count
    }

    pub(crate) const fn selected_parent_box_count(&self) -> usize {
        self.selected_parent_box_count
    }

    pub(crate) const fn boundary_face_count(&self) -> usize {
        self.boundary_face_count
    }

    pub(crate) const fn parent_finite_assignment_count(&self) -> usize {
        self.parent_finite_assignment_count
    }

    pub(crate) const fn face_finite_assignment_count(&self) -> usize {
        self.face_finite_assignment_count
    }

    pub(crate) const fn scheduler_workspace_entries(&self) -> usize {
        self.scheduler_workspace_entries
    }

    pub(crate) const fn scheduler_visit_count(&self) -> usize {
        self.scheduler_visit_count
    }

    pub(crate) const fn subset_unrank_work_upper_bound(&self) -> usize {
        self.subset_unrank_work_upper_bound
    }

    pub(crate) const fn parent_free_dimension(&self) -> usize {
        self.parent_free_dimension
    }

    pub(crate) const fn boundary_codimension(&self) -> usize {
        self.boundary_codimension
    }

    pub(crate) const fn face_dimension(&self) -> usize {
        self.face_dimension
    }

    pub(crate) const fn maximal_available_free_dimension(&self) -> usize {
        self.maximal_available_free_dimension
    }

    pub(crate) const fn profile(&self) -> BoundarySimplexSamplingProfile {
        self.profile
    }

    pub(crate) const fn simplex_sample_count(&self) -> usize {
        self.simplex_sample_count
    }

    pub(crate) fn tasks(&self) -> &[BoundarySimplexTask] {
        &self.tasks
    }

    /// Reject delayed work from every independently rebuilt geometry capture.
    pub(crate) fn validate_task(
        &self,
        task: &BoundarySimplexTask,
    ) -> Result<(), BoundarySimplexPlanError> {
        if self.epoch_identity.belongs_to(&task.epoch_identity) {
            Ok(())
        } else {
            Err(BoundarySimplexPlanError::StaleGeometryEpoch {
                expected_ordinal: self.epoch_ordinal,
                actual_ordinal: task.epoch_ordinal,
            })
        }
    }
}
