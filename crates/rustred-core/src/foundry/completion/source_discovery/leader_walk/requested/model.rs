use std::sync::Arc;

use crate::foundry::completion::{LatticePoint, UncoveredPartition};
use crate::identity::IntegralShift;
use crate::sector::Mask;

use super::super::LeaderWalkPlanError;
use super::super::model::{LeaderWalkBoxKey, LeaderWalkGeometryEpochIdentity, LeaderWalkScopeKey};

/// One exact partition and its explicitly ordered diagnostic target sequence.
///
/// Request order is semantic proposal chronology. Scope order is not: the
/// planner canonicalizes scopes and then interleaves equal request ordinals so
/// a long sequence cannot starve another scope.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RequestedDomainScopePartition<'a> {
    pub(super) stable_scope_key: &'a str,
    pub(super) sector: &'a Mask,
    pub(super) uncovered: &'a UncoveredPartition,
    pub(super) requested: &'a [RequestedDomain],
}

impl<'a> RequestedDomainScopePartition<'a> {
    pub(crate) const fn new(
        stable_scope_key: &'a str,
        sector: &'a Mask,
        uncovered: &'a UncoveredPartition,
        requested: &'a [RequestedDomain],
    ) -> Self {
        Self {
            stable_scope_key,
            sector,
            uncovered,
            requested,
        }
    }
}

/// One explicit rectangular pivot domain.
///
/// `point` is the lower threshold on symbolic axes and the exact literal on
/// complementary axes. Thus symbolic axes are unbounded above while fixed
/// axes have `lower == upper == point`. A planner intersects this entire box,
/// not merely its minimal point, with fresh uncovered geometry.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RequestedDomain {
    point: LatticePoint,
    symbolic_axes: Box<[usize]>,
}

impl RequestedDomain {
    pub(crate) fn new(point: LatticePoint, symbolic_axes: impl IntoIterator<Item = usize>) -> Self {
        Self {
            point,
            symbolic_axes: symbolic_axes.into_iter().collect(),
        }
    }

    pub(crate) const fn point(&self) -> &LatticePoint {
        &self.point
    }

    pub(crate) fn symbolic_axes(&self) -> &[usize] {
        &self.symbolic_axes
    }
}

/// Stable value identity of one requested-domain residual attached to one
/// exact uncovered box.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct RequestedDomainTaskKey {
    scope: LeaderWalkScopeKey,
    box_key: LeaderWalkBoxKey,
    requested_ordinal: usize,
    leader: Arc<Vec<u64>>,
    symbolic_axes: Arc<Vec<usize>>,
    fixed_indices: Arc<Vec<i64>>,
    requested_domain_lower: Arc<Vec<u64>>,
    requested_domain_upper: Arc<Vec<Option<u64>>>,
    residual_domain_upper: Arc<Vec<Option<u64>>>,
}

impl RequestedDomainTaskKey {
    pub(super) const fn new(
        scope: LeaderWalkScopeKey,
        box_key: LeaderWalkBoxKey,
        requested_ordinal: usize,
        leader: Arc<Vec<u64>>,
        symbolic_axes: Arc<Vec<usize>>,
        fixed_indices: Arc<Vec<i64>>,
        requested_domain_lower: Arc<Vec<u64>>,
        requested_domain_upper: Arc<Vec<Option<u64>>>,
        residual_domain_upper: Arc<Vec<Option<u64>>>,
    ) -> Self {
        Self {
            scope,
            box_key,
            requested_ordinal,
            leader,
            symbolic_axes,
            fixed_indices,
            requested_domain_lower,
            requested_domain_upper,
            residual_domain_upper,
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

    pub(crate) const fn requested_ordinal(&self) -> usize {
        self.requested_ordinal
    }

    pub(crate) fn leader(&self) -> &[u64] {
        self.leader.as_slice()
    }

    pub(crate) fn symbolic_axes(&self) -> &[usize] {
        self.symbolic_axes.as_slice()
    }

    pub(crate) fn fixed_indices(&self) -> &[i64] {
        self.fixed_indices.as_slice()
    }

    pub(crate) fn requested_domain_lower(&self) -> &[u64] {
        self.requested_domain_lower.as_slice()
    }

    pub(crate) fn requested_domain_upper(&self) -> &[Option<u64>] {
        self.requested_domain_upper.as_slice()
    }

    /// Upper endpoints of the nonempty intersection between the requested
    /// domain and this task's exact current parent box. Its lower endpoints
    /// are [`Self::leader`].
    pub(crate) fn residual_domain_upper(&self) -> &[Option<u64>] {
        self.residual_domain_upper.as_slice()
    }
}

/// One requested target tied to an opaque in-memory geometry capture.
#[derive(Clone, Debug)]
pub(crate) struct RequestedDomainTask {
    epoch_identity: LeaderWalkGeometryEpochIdentity,
    epoch_ordinal: u64,
    canonical_ordinal: usize,
    key: RequestedDomainTaskKey,
    target_shift: IntegralShift,
}

impl RequestedDomainTask {
    pub(super) const fn new(
        epoch_identity: LeaderWalkGeometryEpochIdentity,
        epoch_ordinal: u64,
        canonical_ordinal: usize,
        key: RequestedDomainTaskKey,
        target_shift: IntegralShift,
    ) -> Self {
        Self {
            epoch_identity,
            epoch_ordinal,
            canonical_ordinal,
            key,
            target_shift,
        }
    }

    pub(crate) const fn epoch_ordinal(&self) -> u64 {
        self.epoch_ordinal
    }

    pub(crate) const fn canonical_ordinal(&self) -> usize {
        self.canonical_ordinal
    }

    pub(crate) const fn key(&self) -> &RequestedDomainTaskKey {
        &self.key
    }

    pub(crate) fn leader(&self) -> &[u64] {
        self.key.leader()
    }

    /// Canonical base-domain sample: the first positive offset on a symbolic
    /// axis only when this residual has room for it, otherwise the residual
    /// lower point. Fixed axes always remain at the sector corner.
    ///
    /// The residual leader is geometric replanning state and must not replace
    /// the original requested-domain recurrence shift. Conversely, it is not
    /// itself a coefficient-evaluation point: this sample remains in the
    /// recurrence's base domain.
    pub(crate) fn base_probe_chart_origin(&self) -> impl ExactSizeIterator<Item = u64> + '_ {
        (0..self.key.sector().arity()).map(|position| {
            let has_positive_extent = self.key.residual_domain_upper()[position]
                .is_none_or(|upper| self.key.leader()[position] < upper);
            if self.key.symbolic_axes().binary_search(&position).is_ok() && has_positive_extent {
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

/// Complete result of one bounded explicit-target attachment pass.
#[derive(Debug)]
pub(crate) struct RequestedDomainPlan {
    pub(super) epoch_identity: LeaderWalkGeometryEpochIdentity,
    pub(super) epoch_ordinal: u64,
    pub(super) declared_scopes: Box<[LeaderWalkScopeKey]>,
    pub(super) input_scope_count: usize,
    pub(super) requested_domain_count: usize,
    pub(super) fully_covered_domain_count: usize,
    pub(super) tasks: Vec<RequestedDomainTask>,
}

impl RequestedDomainPlan {
    pub(crate) const fn epoch_ordinal(&self) -> u64 {
        self.epoch_ordinal
    }

    pub(crate) const fn input_scope_count(&self) -> usize {
        self.input_scope_count
    }

    /// Whether this plan was constructed from the exact stable scope and
    /// sector. This remains available when every request in that scope was
    /// already covered and therefore produced no residual task.
    pub(crate) fn declares_scope(&self, stable_scope_key: &str, sector: &Mask) -> bool {
        self.declared_scopes
            .iter()
            .any(|scope| scope.stable_scope_key() == stable_scope_key && scope.sector() == sector)
    }

    pub(crate) const fn requested_domain_count(&self) -> usize {
        self.requested_domain_count
    }

    pub(crate) const fn fully_covered_domain_count(&self) -> usize {
        self.fully_covered_domain_count
    }

    pub(crate) fn tasks(&self) -> &[RequestedDomainTask] {
        &self.tasks
    }

    pub(crate) fn validate_task(
        &self,
        task: &RequestedDomainTask,
    ) -> Result<(), LeaderWalkPlanError> {
        if self.epoch_identity.belongs_to(&task.epoch_identity) {
            Ok(())
        } else {
            Err(LeaderWalkPlanError::StaleGeometryEpoch {
                expected_ordinal: self.epoch_ordinal,
                actual_ordinal: task.epoch_ordinal,
            })
        }
    }
}
