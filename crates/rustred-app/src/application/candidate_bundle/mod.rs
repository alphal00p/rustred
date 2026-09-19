//! Saved source-port candidates, deliberately separate from closed artifacts.
//!
//! Generation pays for family preparation, sector solving and bundle writing,
//! not original-source replay or coverage certification. Decoding reconstructs
//! ordinary `SectorSolution`s without granting provenance or closure authority.
//! The separate certification API goes through the existing SourcePortAudit.

mod certify;
mod codec;
mod generate;
mod load;
mod model;
pub(super) mod preparation;

pub use certify::certify_candidates;
pub use generate::{family_candidates, family_candidates_with_progress};
pub use load::{inspect_generated_candidate_bundle, load_generated_candidate_bundle};
pub use model::{
    CANDIDATE_BUNDLE_SCHEMA, CANDIDATE_CERTIFICATION_SCHEMA, CandidateBundleInspection,
    CandidateBundleLimits, CandidateBundleResult, CandidateCertificationRequest,
    CandidateCertificationResult, CandidateExactBackend, FAMILY_CANDIDATES_SCHEMA,
    FamilyCandidatesRequest, MAX_CANDIDATE_BUNDLE_BYTES,
};

#[cfg(test)]
mod tests;
