use crate::foundry::completion::source_discovery::boundary_simplex::{
    BoundarySimplexPlan, BoundarySimplexTask,
};
use crate::foundry::completion::source_discovery::interior_simplex::{
    InteriorSimplexPlan, InteriorSimplexTask,
};
use crate::foundry::completion::source_discovery::leader_walk::{
    LeaderWalkPlan, LeaderWalkTask, RequestedDomainPlan, RequestedDomainTask,
};
use crate::identity::IntegralShift;
use crate::sector::Mask;

use super::ProbeCampaignError;

mod sealed {
    pub trait Sealed {}
}

/// Borrowed semantic view of one task authenticated by its owning planner.
///
/// The private supertrait restricts implementations to the reviewed interior,
/// boundary, and leader proposal planners. No planner can mint owner, cover,
/// or ledger authority through this view.
pub(crate) trait ProbeCampaignPlannedTask: sealed::Sealed {
    type Plan;

    fn validate_in_plan(&self, plan: &Self::Plan) -> Result<(), ProbeCampaignError>;
    fn canonical_ordinal(&self) -> usize;
    fn parent_box_lower(&self) -> &[u64];
    fn parent_box_upper(&self) -> &[Option<u64>];
    fn sector(&self) -> &Mask;
    fn lattice_target(&self) -> &[u64];
    fn target_shift(&self) -> &IntegralShift;

    /// Axes left symbolic by an exact boundary-face task. `None` selects the
    /// ordinary maximal-stratum lane; `Some` fixes every complementary base
    /// index at the sector corner. The task lattice target identifies the
    /// recurrence pivot shift, not a coefficient-evaluation anchor.
    fn restricted_symbolic_axes(&self) -> Option<&[usize]>;

    /// Optional fixed base indices for complementary axes. When absent, the
    /// complement is fixed at the sector corner. Requested-face tasks retain
    /// those base coordinates explicitly; their target shift, not these base
    /// values, carries the requested pivot literal.
    fn restricted_fixed_indices(&self) -> Option<&[i64]> {
        None
    }
}

impl sealed::Sealed for InteriorSimplexTask {}

impl ProbeCampaignPlannedTask for InteriorSimplexTask {
    type Plan = InteriorSimplexPlan;

    fn validate_in_plan(&self, plan: &Self::Plan) -> Result<(), ProbeCampaignError> {
        plan.validate_task(self)
            .map_err(ProbeCampaignError::InteriorPlan)
    }

    fn canonical_ordinal(&self) -> usize {
        self.canonical_ordinal()
    }

    fn parent_box_lower(&self) -> &[u64] {
        self.key().box_lower()
    }

    fn parent_box_upper(&self) -> &[Option<u64>] {
        self.key().box_upper()
    }

    fn sector(&self) -> &Mask {
        self.key().sector()
    }

    fn lattice_target(&self) -> &[u64] {
        self.lattice_target()
    }

    fn target_shift(&self) -> &IntegralShift {
        self.target_shift()
    }

    fn restricted_symbolic_axes(&self) -> Option<&[usize]> {
        None
    }
}

impl sealed::Sealed for BoundarySimplexTask {}

impl ProbeCampaignPlannedTask for BoundarySimplexTask {
    type Plan = BoundarySimplexPlan;

    fn validate_in_plan(&self, plan: &Self::Plan) -> Result<(), ProbeCampaignError> {
        plan.validate_task(self)
            .map_err(ProbeCampaignError::BoundaryPlan)
    }

    fn canonical_ordinal(&self) -> usize {
        self.canonical_ordinal()
    }

    fn parent_box_lower(&self) -> &[u64] {
        self.key().parent_box_lower()
    }

    fn parent_box_upper(&self) -> &[Option<u64>] {
        self.key().parent_box_upper()
    }

    fn sector(&self) -> &Mask {
        self.key().sector()
    }

    fn lattice_target(&self) -> &[u64] {
        self.lattice_target()
    }

    fn target_shift(&self) -> &IntegralShift {
        self.target_shift()
    }

    fn restricted_symbolic_axes(&self) -> Option<&[usize]> {
        Some(self.key().remaining_axes())
    }
}

impl sealed::Sealed for LeaderWalkTask {}

impl ProbeCampaignPlannedTask for LeaderWalkTask {
    type Plan = LeaderWalkPlan;

    fn validate_in_plan(&self, plan: &Self::Plan) -> Result<(), ProbeCampaignError> {
        plan.validate_task(self)
            .map_err(ProbeCampaignError::LeaderPlan)
    }

    fn canonical_ordinal(&self) -> usize {
        self.canonical_ordinal()
    }

    fn parent_box_lower(&self) -> &[u64] {
        self.key().box_lower()
    }

    fn parent_box_upper(&self) -> &[Option<u64>] {
        self.key().box_upper()
    }

    fn sector(&self) -> &Mask {
        self.key().sector()
    }

    fn lattice_target(&self) -> &[u64] {
        self.leader()
    }

    fn target_shift(&self) -> &IntegralShift {
        self.target_shift()
    }

    fn restricted_symbolic_axes(&self) -> Option<&[usize]> {
        None
    }
}

impl sealed::Sealed for RequestedDomainTask {}

impl ProbeCampaignPlannedTask for RequestedDomainTask {
    type Plan = RequestedDomainPlan;

    fn validate_in_plan(&self, plan: &Self::Plan) -> Result<(), ProbeCampaignError> {
        plan.validate_task(self)
            .map_err(ProbeCampaignError::LeaderPlan)
    }

    fn canonical_ordinal(&self) -> usize {
        self.canonical_ordinal()
    }

    fn parent_box_lower(&self) -> &[u64] {
        self.key().box_lower()
    }

    fn parent_box_upper(&self) -> &[Option<u64>] {
        self.key().box_upper()
    }

    fn sector(&self) -> &Mask {
        self.key().sector()
    }

    fn lattice_target(&self) -> &[u64] {
        self.leader()
    }

    fn target_shift(&self) -> &IntegralShift {
        self.target_shift()
    }

    fn restricted_symbolic_axes(&self) -> Option<&[usize]> {
        Some(self.key().symbolic_axes())
    }

    fn restricted_fixed_indices(&self) -> Option<&[i64]> {
        Some(self.key().fixed_indices())
    }
}
