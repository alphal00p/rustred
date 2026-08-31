//! Proposal-only bounded right-kernel breadth.
//!
//! These types deliberately cannot mint the private residual-census seal.
//! The exact primary q0 nomination remains a separate
//! [`IncidentTranslationNominations`] value and is the only input accepted by
//! sampled-dual admission.

mod cache;
mod evaluate;
mod model;
mod nominate;
mod select;

#[allow(unused_imports)] // Wired into the bounded probe-local research scheduler below.
pub(crate) use cache::{ProbeRowEvaluationCache, ProbeRowEvaluationCacheTelemetry};
#[allow(unused_imports)] // Wired into the bounded probe-local research scheduler below.
pub(crate) use evaluate::{
    ObstructionBlockProposalBatch, ObstructionBlockProposalCandidate,
    ObstructionBlockProposalScore, ObstructionBlockProposalTelemetry,
};
pub(crate) use model::{
    ObstructionBlockNominationPlan, ObstructionBlockNominationUpperBound,
    ObstructionBlockNominations, UnionObstructionSupportEntry, UnionSupportNominations,
};
#[allow(unused_imports)] // Wired into the bounded probe-local research scheduler below.
pub(crate) use select::try_select_obstruction_block_proposals;

#[cfg(test)]
mod tests;
