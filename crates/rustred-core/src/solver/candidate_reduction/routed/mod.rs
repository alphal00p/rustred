//! Reuse of immutable sector owners through verified momentum maps.
//! Tracing and exact application retain candidate-only authority.
mod application;
mod campaign;
mod domain_overcover;
mod entry;
mod identities;
mod model;
mod prepare;
mod trace;

pub use application::{
    CandidateRoutedApplicationPhase, CandidateRoutedApplicationSnapshot,
    CandidateRoutedApplicationStatistics, CandidateRoutedApplicationStatus,
    CandidateRoutedCacheCensus, CandidateRoutedCachePolicy, CandidateRoutedCacheStatistics,
    CandidateRoutedReduction, RoutedCandidateApplication,
};

pub use campaign::{
    CandidateRoutedCampaignError, CandidateRoutedCampaignFailure, CandidateRoutedCampaignReport,
    CandidateRoutedCampaignSnapshot, CandidateRoutedWork, CandidateRoutedWorkBudget,
};
pub use domain_overcover::{
    CandidateDomainRouteCover, CandidateDomainRouteError, CandidateDomainRouteEvent,
    CandidateDomainRouteFailure, CandidateDomainRouteLimits, CandidateDomainRouteOptions,
    CandidateDomainRouteStats,
};
pub use entry::{
    CandidateEntryAdmission, EntryWitnessError, EntryWitnessLimits, EntryWitnessOutcome,
    FiniteRootAdmission, RootAdmissionError, RootRegionInput, pick_entry_intersection_witness,
};

pub use model::{
    CandidateOwnerRoute, CandidateRoutedError, CandidateRoutedFrontier,
    CandidateRoutedFrontierReason, CandidateRoutedIdentity, CandidateRoutedTraceReport,
    RoutedCandidateLimits, RoutedCandidateReducer,
};

#[cfg(test)]
mod tests;
