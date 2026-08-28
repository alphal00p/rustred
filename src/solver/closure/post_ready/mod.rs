//! Exact post-Ready derivation and compact publication pipeline.
//!
//! Private children retain the linear ownership chain from one exact-session
//! Ready token through condition planning, WhenBad materialization, relative
//! partitioning, and compact publication.  The narrow solver-visible facade
//! contains only the types needed by the exact-session transaction owner.

mod analysis;
mod condition_plan;
mod materialization;
mod partition;
mod publication;

pub(in crate::solver) use analysis::{
    GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler,
    GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome,
};
pub(in crate::solver) use condition_plan::GeneratedAffineResidualGroupExactConditionPlanCompiler;
pub(in crate::solver) use materialization::GeneratedAffineResidualGroupExactWhenBadMaterializationCompiler;
pub(in crate::solver) use partition::{
    GeneratedAffineResidualGroupExactWhenBadPartitionCompilation,
    GeneratedAffineResidualGroupExactWhenBadPartitionCompiler,
    GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBadReason,
    GeneratedAffineResidualGroupExactWhenBadRejectedCandidate,
    GeneratedAffineResidualGroupExactWhenBadRejectedCandidateReplayRecipe,
};
pub(in crate::solver) use publication::{
    PreparedPublication, PublicationLeaf, PublicationLeafDisposition, PublicationPayload,
    PublicationStats,
};

#[cfg(test)]
pub(in crate::solver) use analysis::GeneratedAffineResidualGroupReadyPublicationAnalysisLimits;
#[cfg(test)]
pub(in crate::solver) use condition_plan::GeneratedAffineResidualGroupExactConditionPlanLimits;
#[cfg(test)]
pub(in crate::solver) use materialization::GeneratedAffineResidualGroupExactWhenBadMaterializationLimits;
#[cfg(test)]
pub(in crate::solver) use partition::GeneratedAffineResidualGroupExactWhenBadPartitionLimits;
#[cfg(test)]
pub(in crate::solver) use publication::PublicationLimits;
