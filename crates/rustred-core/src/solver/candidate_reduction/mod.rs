//! Experimental concrete application of source-port candidate formulas.
//!
//! This owner is deliberately NOT a `ClosedArtifact`. It neither replays
//! original-source provenance nor proves a family cover. Each requested
//! integer point is checked against the actual case, whole exceptional
//! conjunctions, inherited source conditions, coefficient denominators, and
//! exact descent order. A missing rule is an error, never an inferred master.
//! Oracle comparisons can test this lane before durable publication succeeds.

mod application;
mod cache;
mod evaluator;
mod model;
mod owners;
pub(crate) mod power_domain;
mod preparation;
mod reducer;
mod routed;
mod terminal_aliases;
mod terminal_normalization;
mod trace;

pub use model::{
    CandidateCacheRepresentation, CandidateDecomposition, CandidateReachabilityReport,
    CandidateReductionError, CandidateStatistics,
};
pub use owners::{
    BoundOwnerOverlay, BoundOwnerSearch, CandidateOwnerContext, CandidateOwnerInput,
    CandidateOwnerPrograms, CandidateOwnerScope, OwnerAppliedError, OwnerAppliedEvent,
    OwnerAppliedFailure, OwnerAppliedLimits, OwnerAppliedNonzero, OwnerAppliedProblem,
    OwnerAppliedProblemKind, OwnerAppliedStats, OwnerAppliedSuccessor, OwnerDomainAttemptLimits,
    OwnerDomainMatchDisposition, OwnerDomainMatchError, OwnerDomainMatchFailure,
    OwnerDomainMatchLimits, OwnerDomainMatchPiece, OwnerDomainMatchStats, OwnerDomainPredicate,
    OwnerDomainRefinementAxes, OwnerDomainScope, OwnerFeedbackError, OwnerFeedbackPolicy,
    OwnerGuardedDomain, OwnerGuardedError, OwnerGuardedEvent, OwnerGuardedFailure,
    OwnerGuardedImage, OwnerGuardedLimits, OwnerGuardedResidualKind, OwnerGuardedStats,
    OwnerGuardedSuccessor, OwnerOverlayLimits, OwnerOverlayMetadata, OwnerOverlayUsage,
    OwnerSuccessorError, OwnerSuccessorFailure, OwnerSuccessorLimits, OwnerSuccessorRegion,
    OwnerSuccessorStats, OwnerSuccessorTransition,
};
pub use power_domain::{
    DomainPowerBounds, DomainPowerError, DomainPowerExtrema, DomainPowerSummary,
};
pub use reducer::CandidateReducer;
pub use routed::{
    CandidateDomainRouteCover, CandidateDomainRouteError, CandidateDomainRouteEvent,
    CandidateDomainRouteFailure, CandidateDomainRouteLimits, CandidateDomainRouteStats,
    CandidateEntryAdmission, CandidateOwnerRoute, CandidateRoutedCampaignError,
    CandidateRoutedCampaignFailure, CandidateRoutedCampaignReport, CandidateRoutedCampaignSnapshot,
    CandidateRoutedError, CandidateRoutedFrontier, CandidateRoutedFrontierReason,
    CandidateRoutedTraceReport, CandidateRoutedWork, EntryWitnessError, EntryWitnessLimits,
    EntryWitnessOutcome, FiniteRootAdmission, RootAdmissionError, RootRegionInput,
    RoutedCandidateLimits, RoutedCandidateReducer, pick_entry_intersection_witness,
};
pub use trace::{CandidateTraceLimits, CandidateTraceReport};

#[cfg(test)]
mod owner_test_support;
#[cfg(test)]
mod tests;
