use crate::foundry::completion::source_discovery::boundary_simplex::{
    BoundarySimplexPlan, BoundarySimplexTask,
};
use crate::foundry::completion::source_discovery::interior_simplex::{
    InteriorSimplexPlan, InteriorSimplexTask,
};
use crate::identity::IntegralShift;
use crate::sector::Mask;

use super::ProbeCampaignError;

mod sealed {
    pub trait Sealed {}
}

/// Borrowed semantic view of one task authenticated by its owning planner.
///
/// The private supertrait restricts implementations to the two proposal
/// planners reviewed by this campaign boundary. Neither planner can mint
/// owner, cover, or ledger authority through this view.
pub(crate) trait ProbeCampaignPlannedTask: sealed::Sealed {
    type Plan;

    fn validate_in_plan(&self, plan: &Self::Plan) -> Result<(), ProbeCampaignError>;
    fn canonical_ordinal(&self) -> usize;
    fn parent_box_lower(&self) -> &[u64];
    fn parent_box_upper(&self) -> &[Option<u64>];
    fn sector(&self) -> &Mask;
    fn lattice_target(&self) -> &[u64];
    fn target_shift(&self) -> &IntegralShift;
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
}
