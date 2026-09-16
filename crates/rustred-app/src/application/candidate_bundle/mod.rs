//! Saved source-port candidates, deliberately separate from closed artifacts.
//!
//! Generation pays for family preparation, sector solving and bundle writing,
//! not original-source replay or coverage certification. Decoding reconstructs
//! ordinary `SectorSolution`s without granting provenance or closure authority.
//! The separate certification API goes through the existing SourcePortAudit.

mod certify;
mod codec;
mod generate;
mod model;
pub(super) mod preparation;

pub use certify::certify_candidates;
pub use generate::family_candidates;
pub use model::{
    CANDIDATE_BUNDLE_SCHEMA, CANDIDATE_CERTIFICATION_SCHEMA, CandidateBundleLimits,
    CandidateBundleResult, CandidateCertificationRequest, CandidateCertificationResult,
    FAMILY_CANDIDATES_SCHEMA, FamilyCandidatesRequest,
};

#[cfg(test)]
mod tests;
