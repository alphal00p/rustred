//! Immutable, same-family candidate programs; not closure certificates.
mod domains;
mod evaluation;
pub use domains::{
    OwnerSuccessorError, OwnerSuccessorFailure, OwnerSuccessorLimits, OwnerSuccessorRegion,
    OwnerSuccessorStats, OwnerSuccessorTransition,
};
mod feedback;
mod model;
mod prepare;
pub(in crate::solver::candidate_reduction) use evaluation::OwnerStep;
pub use feedback::{
    BoundOwnerOverlay, BoundOwnerSearch, OwnerDomainAttemptLimits, OwnerDomainScope,
    OwnerFeedbackError, OwnerFeedbackPolicy, OwnerOverlayLimits, OwnerOverlayMetadata,
    OwnerOverlayUsage,
};

pub use model::{
    CandidateOwnerContext, CandidateOwnerInput, CandidateOwnerPrograms, CandidateOwnerScope,
};

#[cfg(test)]
mod tests;
