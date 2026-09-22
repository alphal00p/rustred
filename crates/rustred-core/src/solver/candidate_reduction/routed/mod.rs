//! Trace-only reuse of immutable sector owners through verified momentum maps.
//! No coefficient back-substitution or family-closure authority is provided.
mod campaign;
mod domain_overcover;
mod model;
mod prepare;
mod trace;

pub use campaign::{
    CandidateRoutedCampaignError, CandidateRoutedCampaignFailure, CandidateRoutedCampaignReport,
    CandidateRoutedCampaignSnapshot, CandidateRoutedWork,
};
pub use domain_overcover::{
    CandidateDomainRouteCover, CandidateDomainRouteError, CandidateDomainRouteEvent,
    CandidateDomainRouteFailure, CandidateDomainRouteLimits, CandidateDomainRouteStats,
};

pub use model::{
    CandidateOwnerRoute, CandidateRoutedError, CandidateRoutedFrontier,
    CandidateRoutedFrontierReason, CandidateRoutedTraceReport, RoutedCandidateLimits,
    RoutedCandidateReducer,
};

#[cfg(test)]
mod tests;
