use std::sync::Arc;

use crate::foundry::completion::UncoveredPartition;
use crate::identity::IntegralShift;
use crate::sector::Mask;

use super::InteriorSimplexPlanError;

/// Borrowed input to one frozen interior-simplex planning epoch.
///
/// The stable key must bind every caller-side choice affecting the meaning of
/// the partition, including ordering and immutable predecessor state.  The
/// planner retains the sector separately and rejects duplicate stable keys.
#[derive(Clone, Copy, Debug)]
pub(crate) struct InteriorSimplexScopePartition<'a> {
    pub(crate) stable_scope_key: &'a str,
    pub(crate) sector: &'a Mask,
    pub(crate) uncovered: &'a UncoveredPartition,
}

impl<'a> InteriorSimplexScopePartition<'a> {
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

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct InteriorSimplexScopeKey {
    stable_scope_key: Arc<String>,
    sector: Mask,
}

impl InteriorSimplexScopeKey {
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
pub(super) struct InteriorSimplexBoxKey {
    lower: Arc<Vec<u64>>,
    upper: Arc<Vec<Option<u64>>>,
}

impl InteriorSimplexBoxKey {
    pub(super) fn new(lower: Vec<u64>, upper: Vec<Option<u64>>) -> Self {
        // Variable-sized buffers were reserved fallibly by the planner.  Arc
        // adds only its fixed-size owner allocation.
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

/// Stable, epoch-independent identity of one finite-assignment/simplex target.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct InteriorSimplexTaskKey {
    scope: InteriorSimplexScopeKey,
    box_key: InteriorSimplexBoxKey,
    interior_margin: u64,
    simplex_offset: Arc<Vec<u64>>,
    finite_assignment_ordinal: usize,
}

impl InteriorSimplexTaskKey {
    pub(super) const fn new(
        scope: InteriorSimplexScopeKey,
        box_key: InteriorSimplexBoxKey,
        interior_margin: u64,
        simplex_offset: Arc<Vec<u64>>,
        finite_assignment_ordinal: usize,
    ) -> Self {
        Self {
            scope,
            box_key,
            interior_margin,
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

    pub(crate) fn box_lower(&self) -> &[u64] {
        self.box_key.lower()
    }

    pub(crate) fn box_upper(&self) -> &[Option<u64>] {
        self.box_key.upper()
    }

    pub(crate) const fn interior_margin(&self) -> u64 {
        self.interior_margin
    }

    /// Offset coordinates are indexed by ascending unbounded-axis rank, not
    /// by ambient chart position.
    pub(crate) fn simplex_offset(&self) -> &[u64] {
        self.simplex_offset.as_slice()
    }

    /// Lexicographic mixed-radix ordinal of the finite-coordinate assignment.
    ///
    /// The complete box endpoints retained in this key make the ordinal an
    /// exact identity: ascending finite axes define lexicographic order and
    /// the last finite axis varies fastest.
    pub(crate) const fn finite_assignment_ordinal(&self) -> usize {
        self.finite_assignment_ordinal
    }
}

#[derive(Clone, Debug)]
pub(super) struct InteriorSimplexGeometryEpochIdentity(Arc<()>);

impl InteriorSimplexGeometryEpochIdentity {
    pub(super) fn fresh() -> Self {
        Self(Arc::new(()))
    }

    fn belongs_to(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// One checked target proposal from a frozen geometry epoch.
#[derive(Clone, Debug)]
pub(crate) struct InteriorSimplexTask {
    epoch_identity: InteriorSimplexGeometryEpochIdentity,
    epoch_ordinal: u64,
    canonical_ordinal: usize,
    key: InteriorSimplexTaskKey,
    lattice_target: Arc<Vec<u64>>,
    target_shift: IntegralShift,
}

impl InteriorSimplexTask {
    pub(super) fn new(
        epoch_identity: InteriorSimplexGeometryEpochIdentity,
        epoch_ordinal: u64,
        canonical_ordinal: usize,
        key: InteriorSimplexTaskKey,
        lattice_target: Vec<u64>,
        target_shift: IntegralShift,
    ) -> Self {
        Self {
            epoch_identity,
            epoch_ordinal,
            canonical_ordinal,
            key,
            // Variable-sized storage was reserved fallibly by the planner.
            lattice_target: Arc::new(lattice_target),
            target_shift,
        }
    }

    pub(crate) const fn epoch_ordinal(&self) -> u64 {
        self.epoch_ordinal
    }

    /// Canonical ordinal used to restore deterministic worker result order.
    pub(crate) const fn canonical_ordinal(&self) -> usize {
        self.canonical_ordinal
    }

    pub(crate) const fn key(&self) -> &InteriorSimplexTaskKey {
        &self.key
    }

    pub(crate) fn lattice_target(&self) -> &[u64] {
        self.lattice_target.as_slice()
    }

    pub(crate) const fn target_shift(&self) -> &IntegralShift {
        &self.target_shift
    }
}

/// Free-dimension policy frozen into one proposal plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum InteriorSimplexFreeDimensionSelection {
    /// Select every box at the largest positive free dimension present in the
    /// captured input geometry.
    Maximal,
    /// Select every box at exactly this positive free dimension.
    Exact(usize),
}

/// Immutable complete finite-assignment × simplex design for one geometry capture.
///
/// This plan contains target proposals only.  In particular, it deliberately
/// has no API for recording execution, exhaustion, cover deltas, or closure.
#[derive(Debug)]
pub(crate) struct InteriorSimplexPlan {
    pub(super) epoch_identity: InteriorSimplexGeometryEpochIdentity,
    pub(super) epoch_ordinal: u64,
    pub(super) input_scope_count: usize,
    pub(super) selected_scope_count: usize,
    pub(super) selected_box_count: usize,
    pub(super) finite_assignment_count: usize,
    pub(super) scheduler_workspace_entries: usize,
    pub(super) scheduler_visit_count: usize,
    pub(super) free_dimension_selection: InteriorSimplexFreeDimensionSelection,
    pub(super) selected_free_dimension: usize,
    pub(super) maximal_free_dimension: usize,
    pub(super) interior_margin: u64,
    pub(super) polynomial_degree_ceiling: usize,
    pub(super) simplex_sample_count: usize,
    pub(super) tasks: Vec<InteriorSimplexTask>,
}

impl InteriorSimplexPlan {
    pub(crate) const fn epoch_ordinal(&self) -> u64 {
        self.epoch_ordinal
    }

    pub(crate) const fn input_scope_count(&self) -> usize {
        self.input_scope_count
    }

    pub(crate) const fn selected_scope_count(&self) -> usize {
        self.selected_scope_count
    }

    pub(crate) const fn selected_box_count(&self) -> usize {
        self.selected_box_count
    }

    /// Sum of complete finite-axis Cartesian products over selected boxes.
    /// Each assignment is paired with every retained simplex offset.
    pub(crate) const fn finite_assignment_count(&self) -> usize {
        self.finite_assignment_count
    }

    /// Exact logical peak of index entries requested by the canonical box
    /// flattener or the ordered active-assignment frontiers.
    pub(crate) const fn scheduler_workspace_entries(&self) -> usize {
        self.scheduler_workspace_entries
    }

    /// Exact number of canonical box/round inspections performed by task
    /// scheduling. Every assignment-frontier visit emits one task; there are
    /// no rectangular scans over already exhausted boxes.
    pub(crate) const fn scheduler_visit_count(&self) -> usize {
        self.scheduler_visit_count
    }

    /// Caller selection that produced this frozen plan.
    pub(crate) const fn free_dimension_selection(&self) -> InteriorSimplexFreeDimensionSelection {
        self.free_dimension_selection
    }

    /// Common free dimension of every box selected into this plan.
    pub(crate) const fn selected_free_dimension(&self) -> usize {
        self.selected_free_dimension
    }

    /// Largest free dimension present anywhere in the captured input, which
    /// can exceed [`Self::selected_free_dimension`] for an exact
    /// lower-dimension-box plan.
    pub(crate) const fn maximal_free_dimension(&self) -> usize {
        self.maximal_free_dimension
    }

    pub(crate) const fn interior_margin(&self) -> u64 {
        self.interior_margin
    }

    pub(crate) const fn polynomial_degree_ceiling(&self) -> usize {
        self.polynomial_degree_ceiling
    }

    /// Number of simplex offsets used for every selected box.
    pub(crate) const fn simplex_sample_count(&self) -> usize {
        self.simplex_sample_count
    }

    pub(crate) fn tasks(&self) -> &[InteriorSimplexTask] {
        &self.tasks
    }

    /// Reject delayed work from any prior capture, even if its diagnostic
    /// ordinal and geometry happen to be structurally equal.
    pub(crate) fn validate_task(
        &self,
        task: &InteriorSimplexTask,
    ) -> Result<(), InteriorSimplexPlanError> {
        if self.epoch_identity.belongs_to(&task.epoch_identity) {
            Ok(())
        } else {
            Err(InteriorSimplexPlanError::StaleGeometryEpoch {
                expected_ordinal: self.epoch_ordinal,
                actual_ordinal: task.epoch_ordinal,
            })
        }
    }
}
