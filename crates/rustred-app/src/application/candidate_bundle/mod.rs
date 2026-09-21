//! Saved source-port candidates, deliberately separate from closed artifacts.
//!
//! Generation pays for family preparation, sector solving and bundle writing,
//! not original-source replay or coverage certification. Decoding reconstructs
//! ordinary `SectorSolution`s without granting provenance or closure authority.
//! The separate certification API goes through the existing SourcePortAudit.

mod certify;
mod checkpoint;
mod codec;
mod generate;
mod load;
mod model;
mod policy;
pub(super) mod preparation;
mod save;

pub use certify::{certify_candidates, certify_candidates_with_progress};
pub use checkpoint::CandidateCheckpointOptions;
pub use generate::{family_candidates, family_candidates_with_progress};
pub use load::{
    inspect_generated_candidate_bundle, load_generated_candidate_bundle,
    load_generated_candidate_checkpoint,
};
pub use model::{
    CANDIDATE_BUNDLE_SCHEMA, CANDIDATE_CERTIFICATION_SCHEMA, CandidateBundleInspection,
    CandidateBundleLimits, CandidateBundleResult, CandidateCertificationRequest,
    CandidateCertificationResult, CandidateExactBackend, CaseIntersectionLimits,
    FAMILY_CANDIDATES_SCHEMA, FamilyCandidatesRequest, FiniteCaseLimits, FiniteCasePolicy,
    MAX_CANDIDATE_BUNDLE_BYTES,
};
pub use save::encode_generated_candidate_sector;

#[cfg(test)]
mod tests;
