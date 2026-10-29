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
mod cached_program;
mod capture;
mod evaluator;
mod model;
mod ordered;
mod owners;
pub(crate) mod power_domain;
mod preparation;
mod reducer;
mod root_guards;
mod routed;
mod terminal_aliases;
mod terminal_normalization;
mod trace;

pub use cached_program::{CandidateGuardedCacheError, CandidateUnitMassProgram};
pub use model::{
    CandidateCacheRepresentation, CandidateDecomposition, CandidateReachabilityReport,
    CandidateReductionError, CandidateStatistics, UnresolvedPolicy,
};
pub use ordered::CandidateOrderedReduction;
pub use owners::{
    BoundOwnerOverlay, BoundOwnerSearch, CandidateIntegralIdentity, CandidateOwnerContext,
    CandidateOwnerInput, CandidateOwnerPrograms, CandidateOwnerScope,
    DeferredCandidateOwnerDescriptor, DeferredCandidateOwnerLoader, DeferredCandidateOwnerPrograms,
    DeferredCandidateOwnerStatistics, FactorCensusLimits, FactorCensusRole,
    OwnerAppliedCellRefinement, OwnerAppliedError, OwnerAppliedEvent, OwnerAppliedFailure,
    OwnerAppliedLimits, OwnerAppliedNonzero, OwnerAppliedProblem, OwnerAppliedProblemKind,
    OwnerAppliedStats, OwnerAppliedSuccessor, OwnerDomainAttemptLimits,
    OwnerDomainMatchDisposition, OwnerDomainMatchError, OwnerDomainMatchFailure,
    OwnerDomainMatchLimits, OwnerDomainMatchPiece, OwnerDomainMatchStats, OwnerDomainPredicate,
    OwnerDomainRefinementAxes, OwnerDomainScope, OwnerFactorCensus, OwnerFeedbackError,
    OwnerFeedbackPolicy, OwnerGuardedDomain, OwnerGuardedError, OwnerGuardedEvent,
    OwnerGuardedFailure, OwnerGuardedImage, OwnerGuardedLimits, OwnerGuardedResidualKind,
    OwnerGuardedStats, OwnerGuardedSuccessor, OwnerOverlayLimits, OwnerOverlayMetadata,
    OwnerOverlayUsage, OwnerSuccessorError, OwnerSuccessorFailure, OwnerSuccessorLimits,
    OwnerSuccessorRegion, OwnerSuccessorStats, OwnerSuccessorTransition,
};
pub use power_domain::{
    DomainPowerBounds, DomainPowerError, DomainPowerExtrema, DomainPowerSummary,
};
pub use reducer::CandidateReducer;
pub use routed::{
    CandidateDomainRouteCover, CandidateDomainRouteError, CandidateDomainRouteEvent,
    CandidateDomainRouteFailure, CandidateDomainRouteLimits, CandidateDomainRouteOptions,
    CandidateDomainRouteStats, CandidateEntryAdmission, CandidateOrderedPhaseKind,
    CandidateOrderedReductionProgress, CandidateOwnerRoute, CandidateRoutedActivity,
    CandidateRoutedApplicationEnvelope, CandidateRoutedApplicationEvent,
    CandidateRoutedApplicationMetrics, CandidateRoutedApplicationObserver,
    CandidateRoutedApplicationPhase, CandidateRoutedApplicationSnapshot,
    CandidateRoutedApplicationStatistics, CandidateRoutedApplicationStatus,
    CandidateRoutedCacheCensus, CandidateRoutedCacheExportStatistics, CandidateRoutedCachePolicy,
    CandidateRoutedCacheStatistics, CandidateRoutedCampaignError, CandidateRoutedCampaignFailure,
    CandidateRoutedCampaignReport, CandidateRoutedCampaignSnapshot, CandidateRoutedCompletedDelta,
    CandidateRoutedCompletedTrace, CandidateRoutedCompletedTraceError,
    CandidateRoutedCompletedTraceLimits, CandidateRoutedCompletionSource,
    CandidateRoutedComplexity, CandidateRoutedDeltaExportLimits, CandidateRoutedError,
    CandidateRoutedFrontier, CandidateRoutedFrontierReason, CandidateRoutedFrontierSample,
    CandidateRoutedGeneratedBaseline, CandidateRoutedIdentity, CandidateRoutedKeyDegrees,
    CandidateRoutedObservationOptions, CandidateRoutedObserverControl,
    CandidateRoutedObserverFault, CandidateRoutedObserverStatus,
    CandidateRoutedPersistentCacheStatistics, CandidateRoutedReduction,
    CandidateRoutedSelectionIdentity, CandidateRoutedStoredState, CandidateRoutedTerminalProgress,
    CandidateRoutedTraceCensus, CandidateRoutedTraceCensusLimits, CandidateRoutedTraceReport,
    CandidateRoutedWork, CandidateRoutedWorkBudget, CandidateRoutedWorkRef,
    CandidateRoutedWorkerPool, EntryWitnessError, EntryWitnessLimits, EntryWitnessOutcome,
    FiniteRootAdmission, RootAdmissionError, RootRegionInput, RoutedCandidateApplication,
    RoutedCandidateLimits, RoutedCandidateReducer, pick_entry_intersection_witness,
};
pub use trace::{CandidateTraceLimits, CandidateTraceReport};

#[cfg(test)]
mod owner_test_support;
#[cfg(test)]
mod tests;
