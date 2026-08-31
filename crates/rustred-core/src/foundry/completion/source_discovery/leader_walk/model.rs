use std::sync::Arc;

use crate::foundry::completion::UncoveredPartition;
use crate::identity::IntegralShift;
use crate::sector::Mask;

use super::LeaderWalkPlanError;

/// Borrowed, topology-neutral input to one frozen planning epoch.
///
/// `stable_scope_key` must bind every caller-side choice that changes the
/// meaning of the partition (sector, stratum, ordering, and immutable
/// predecessor snapshot).  The planner additionally retains the sector and
/// rejects duplicate keys, but does not interpret or authenticate the key.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LeaderWalkScopePartition<'a> {
    pub(crate) stable_scope_key: &'a str,
    pub(crate) sector: &'a Mask,
    pub(crate) uncovered: &'a UncoveredPartition,
}

impl<'a> LeaderWalkScopePartition<'a> {
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
pub(super) struct LeaderWalkScopeKey {
    stable_scope_key: Arc<String>,
    sector: Mask,
}

impl LeaderWalkScopeKey {
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
pub(super) struct LeaderWalkBoxKey {
    lower: Arc<Vec<u64>>,
    upper: Arc<Vec<Option<u64>>>,
}

impl LeaderWalkBoxKey {
    pub(super) fn new(lower: Vec<u64>, upper: Vec<Option<u64>>) -> Self {
        Self {
            // The variable-sized buffers were reserved fallibly by the
            // planner. Arc adds only fixed-size owner allocations.
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

/// The only two bounded proposal depths in this planning seam.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum LeaderWalkDepth {
    LowerCorner,
    DepthOne,
}

/// Stable, epoch-independent identity of one proposed task.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct LeaderWalkTaskKey {
    scope: LeaderWalkScopeKey,
    box_key: LeaderWalkBoxKey,
    depth: LeaderWalkDepth,
    depth_one_axis: Option<usize>,
}

impl LeaderWalkTaskKey {
    pub(super) const fn new(
        scope: LeaderWalkScopeKey,
        box_key: LeaderWalkBoxKey,
        depth: LeaderWalkDepth,
        depth_one_axis: Option<usize>,
    ) -> Self {
        Self {
            scope,
            box_key,
            depth,
            depth_one_axis,
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

    pub(crate) const fn depth(&self) -> LeaderWalkDepth {
        self.depth
    }

    /// The single unbounded chart axis raised by a depth-one seed.
    ///
    /// Lower-corner seeds retain `None`. Construction is private to the
    /// planner, which guarantees that a depth-one axis is present and is
    /// unbounded in `box_upper`.
    pub(crate) const fn depth_one_axis(&self) -> Option<usize> {
        self.depth_one_axis
    }
}

#[derive(Clone, Debug)]
pub(super) struct LeaderWalkGeometryEpochIdentity(Arc<()>);

impl LeaderWalkGeometryEpochIdentity {
    pub(super) fn fresh() -> Self {
        Self(Arc::new(()))
    }

    fn belongs_to(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// One fully owned proposal from a frozen geometry epoch.
///
/// The checked target shift is relative to the declared sector corner.  The
/// retained epoch identity prevents delayed worker results from being applied
/// to a rebuilt cover merely because an external ordinal was reused.
#[derive(Clone, Debug)]
pub(crate) struct LeaderWalkTask {
    epoch_identity: LeaderWalkGeometryEpochIdentity,
    epoch_ordinal: u64,
    canonical_ordinal: usize,
    key: LeaderWalkTaskKey,
    leader: Arc<Vec<u64>>,
    target_shift: IntegralShift,
}

impl LeaderWalkTask {
    pub(super) fn new(
        epoch_identity: LeaderWalkGeometryEpochIdentity,
        epoch_ordinal: u64,
        canonical_ordinal: usize,
        key: LeaderWalkTaskKey,
        leader: Vec<u64>,
        target_shift: IntegralShift,
    ) -> Self {
        Self {
            epoch_identity,
            epoch_ordinal,
            canonical_ordinal,
            key,
            // The coordinate buffer was reserved fallibly by the planner.
            leader: Arc::new(leader),
            target_shift,
        }
    }

    pub(crate) const fn epoch_ordinal(&self) -> u64 {
        self.epoch_ordinal
    }

    /// Canonical within its wave.  Worker results must be restored to this
    /// order before deterministic admission or telemetry.
    pub(crate) const fn canonical_ordinal(&self) -> usize {
        self.canonical_ordinal
    }

    pub(crate) const fn key(&self) -> &LeaderWalkTaskKey {
        &self.key
    }

    pub(crate) fn leader(&self) -> &[u64] {
        self.leader.as_slice()
    }

    pub(crate) const fn target_shift(&self) -> &IntegralShift {
        &self.target_shift
    }
}

/// One complete, canonically ordered proposal wave.
#[derive(Debug)]
pub(crate) struct LeaderWalkWave {
    depth: LeaderWalkDepth,
    tasks: Vec<LeaderWalkTask>,
}

impl LeaderWalkWave {
    pub(super) fn new(depth: LeaderWalkDepth, tasks: Vec<LeaderWalkTask>) -> Self {
        Self { depth, tasks }
    }

    pub(crate) const fn depth(&self) -> LeaderWalkDepth {
        self.depth
    }

    pub(crate) fn tasks(&self) -> &[LeaderWalkTask] {
        &self.tasks
    }
}

/// Immutable two-wave proposal plan tied to one in-memory geometry capture.
#[derive(Debug)]
pub(crate) struct LeaderWalkPlan {
    pub(super) epoch_identity: LeaderWalkGeometryEpochIdentity,
    pub(super) epoch_ordinal: u64,
    pub(super) input_scope_count: usize,
    pub(super) selected_scope_count: usize,
    pub(super) selected_box_count: usize,
    pub(super) planned_task_count: usize,
    pub(super) maximal_free_dimension: usize,
    pub(super) lower_corner: LeaderWalkWave,
    pub(super) depth_one: LeaderWalkWave,
}

impl LeaderWalkPlan {
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

    pub(crate) const fn planned_task_count(&self) -> usize {
        self.planned_task_count
    }

    pub(crate) const fn maximal_free_dimension(&self) -> usize {
        self.maximal_free_dimension
    }

    /// Execution order is lower-corner first and depth-one only after the
    /// first wave produces no admitted cover change.
    pub(crate) fn waves(&self) -> [&LeaderWalkWave; 2] {
        [&self.lower_corner, &self.depth_one]
    }

    /// Reject a task from any previous capture, including a structurally equal
    /// capture whose caller happened to reuse the same diagnostic ordinal.
    pub(crate) fn validate_task(&self, task: &LeaderWalkTask) -> Result<(), LeaderWalkPlanError> {
        if self.epoch_identity.belongs_to(&task.epoch_identity) {
            Ok(())
        } else {
            Err(LeaderWalkPlanError::StaleGeometryEpoch {
                expected_ordinal: self.epoch_ordinal,
                actual_ordinal: task.epoch_ordinal,
            })
        }
    }

    /// Describe the finite planning envelope retained by this plan.
    ///
    /// This census does not assert that tasks ran, that no relation exists, or
    /// that any region is covered. It deliberately carries no outcome, owner,
    /// terminal, rule, or cover capability.
    pub(crate) fn planning_envelope_census(&self) -> PlanningEnvelopeCensus {
        PlanningEnvelopeCensus {
            epoch_identity: self.epoch_identity.clone(),
            epoch_ordinal: self.epoch_ordinal,
            selected_scope_count: self.selected_scope_count,
            selected_box_count: self.selected_box_count,
            planned_task_count: self.planned_task_count,
        }
    }
}

/// Neutral structural census of one bounded seed-planning envelope.
///
/// This type contains no task results and no stop disposition. It cannot state
/// that the envelope was executed or exhausted, much less establish a master,
/// terminal, finite complement, or infinite-domain closure.
#[derive(Clone, Debug)]
pub(crate) struct PlanningEnvelopeCensus {
    epoch_identity: LeaderWalkGeometryEpochIdentity,
    epoch_ordinal: u64,
    selected_scope_count: usize,
    selected_box_count: usize,
    planned_task_count: usize,
}

impl PlanningEnvelopeCensus {
    pub(crate) const fn epoch_ordinal(&self) -> u64 {
        self.epoch_ordinal
    }

    pub(crate) const fn selected_scope_count(&self) -> usize {
        self.selected_scope_count
    }

    pub(crate) const fn selected_box_count(&self) -> usize {
        self.selected_box_count
    }

    pub(crate) const fn planned_task_count(&self) -> usize {
        self.planned_task_count
    }

    pub(crate) fn belongs_to(&self, plan: &LeaderWalkPlan) -> bool {
        self.epoch_identity.belongs_to(&plan.epoch_identity)
    }
}
