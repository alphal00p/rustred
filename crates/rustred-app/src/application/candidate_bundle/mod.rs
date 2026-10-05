//! Saved source-port candidates, deliberately separate from closed artifacts.
//!
//! Generation pays for family preparation, sector solving and bundle writing,
//! not original-source replay or coverage certification. Decoding reconstructs
//! ordinary `SectorSolution`s without granting provenance or closure authority.
//! The separate certification API goes through the existing SourcePortAudit.

mod certify;
mod checkpoint;
mod circuit_replay;
mod codec;
mod domain_overlay;
mod generate;
mod inspection;
mod load;
mod model;
mod normalization;
pub use normalization::{CandidateTerminalNormalization, CandidateTerminalNormalizationLimits};
mod order;
mod policy;
pub(super) mod preparation;
mod priority;
mod save;
mod selection;
mod session;
mod strategy;
mod view;
pub use view::{CandidateArtifact, CandidateArtifactPage};

pub use order::{
    CandidateCoordinateGroups, CandidateDegreeRow, CandidateIntegralOrder, CandidateOrderDirection,
};

pub use strategy::{
    CandidateDiscoveryStrategy, CandidateRowFeature, CandidateRowPriority, CandidateRulePortfolio,
    CandidateRulePortfolioTrigger, CandidateRuleQualityFeature, CandidateRuleQualityPriority,
    CandidateRuleQualityThreshold, CandidateRuleTrialLimits, CandidateSectorPriority,
    CandidateSourcePriority, CandidateSourceVisitPlan,
};

pub use certify::{certify_candidates, certify_candidates_with_progress};
pub use checkpoint::CandidateCheckpointOptions;
pub use circuit_replay::with_replayed_candidate_rule_circuits;
pub use domain_overlay::{
    CandidateDomainOverlayLoadLimits, encode_generated_domain_overlay,
    load_generated_domain_overlay, validate_domain_overlay_ingress,
};
pub use generate::{family_candidates, family_candidates_with_progress};
pub use inspection::*;
pub(crate) use load::load_generated_candidate_owners_with_preference_rule_subsets;
pub use load::{
    CandidateOwnerBundle, CandidateOwnerLoadLimits, inspect_generated_candidate_bundle,
    load_generated_candidate_bundle, load_generated_candidate_checkpoint,
    load_generated_candidate_owners, load_generated_candidate_owners_with_preferences,
};
pub use model::{
    CANDIDATE_BUNDLE_SCHEMA, CANDIDATE_CERTIFICATION_SCHEMA, CandidateBundleInspection,
    CandidateBundleLimits, CandidateBundleResult, CandidateCertificationRequest,
    CandidateCertificationResult, CandidateExactBackend, CaseIntersectionLimits,
    FAMILY_CANDIDATES_SCHEMA, FamilyCandidatesRequest, FiniteCaseLimits, FiniteCasePolicy,
    MAX_CANDIDATE_BUNDLE_BYTES,
};
pub use priority::{
    CheckedPriorityOwnerExport, encode_checked_priority_owner,
    encode_checked_priority_owner_with_policy,
};
pub use save::encode_generated_candidate_sector;
pub use session::{
    CandidateGenerationEvents, CandidateGenerationJob, CandidateGenerationSession,
    CandidateGenerationSnapshot, CandidateGenerationState,
};

/// Test support: split a generated multi-sector bundle into the single-sector
/// owner bundles the shared-owner loader accepts, as (mask text, bytes, family
/// fingerprint). Records are copied unchanged; nothing is re-solved.
#[cfg(test)]
pub(crate) fn split_generated_candidate_bundle(
    bytes: &[u8],
) -> Result<Vec<(String, Vec<u8>, String)>, crate::AppError> {
    let bundle = codec::read(bytes, Default::default())?;
    bundle
        .sectors
        .iter()
        .map(|sector| {
            let mut shard = bundle.clone();
            shard.sectors = vec![sector.clone()];
            let mask = sector
                .sector
                .iter()
                .map(|&active| if active { '1' } else { '0' })
                .collect();
            Ok((
                mask,
                codec::write(&shard, Default::default())?,
                bundle.family_fingerprint.clone(),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests;
