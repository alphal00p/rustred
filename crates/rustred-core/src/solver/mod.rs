//! Compact, source-directed port of SpIRed's sector solver.
//!
//! This subsystem implements the reference search algorithm over Symbolica's
//! native polynomial and sparse-linear-algebra types. Family preparation and
//! artifact publication remain outside its per-row hot path. A sector search
//! result is not, by itself, a certified family-closing artifact.

mod candidate_reduction;
mod case;
mod cuts;
mod discovery;
mod error;
mod exception;
mod execution;
mod geometry;
mod index;
mod instantiate;
mod numeric;
mod precondition;
mod row;
mod search;
mod sector;
mod seed;
mod source;

use std::time::{Duration, Instant};

use crate::family::IntegralFamily;

pub(crate) use instantiate::{
    canonicalize as canonicalize_source_port, instantiate as instantiate_source_port,
    instantiate_polynomial as instantiate_polynomial_source_port,
    translate as translate_source_port,
};

pub use candidate_reduction::{
    BoundOwnerOverlay, BoundOwnerSearch, CandidateCacheRepresentation, CandidateDecomposition,
    CandidateDomainRouteCover, CandidateDomainRouteError, CandidateDomainRouteEvent,
    CandidateDomainRouteFailure, CandidateDomainRouteLimits, CandidateDomainRouteStats,
    CandidateOwnerContext, CandidateOwnerInput, CandidateOwnerPrograms, CandidateOwnerRoute,
    CandidateOwnerScope, CandidateReachabilityReport, CandidateReducer, CandidateReductionError,
    CandidateRoutedCampaignError, CandidateRoutedCampaignFailure, CandidateRoutedCampaignReport,
    CandidateRoutedCampaignSnapshot, CandidateRoutedError, CandidateRoutedFrontier,
    CandidateRoutedFrontierReason, CandidateRoutedTraceReport, CandidateRoutedWork,
    CandidateStatistics, CandidateTraceLimits, CandidateTraceReport, OwnerAppliedError,
    OwnerAppliedEvent, OwnerAppliedFailure, OwnerAppliedLimits, OwnerAppliedNonzero,
    OwnerAppliedProblem, OwnerAppliedProblemKind, OwnerAppliedStats, OwnerAppliedSuccessor,
    OwnerDomainAttemptLimits, OwnerDomainMatchDisposition, OwnerDomainMatchError,
    OwnerDomainMatchFailure, OwnerDomainMatchLimits, OwnerDomainMatchPiece, OwnerDomainMatchStats,
    OwnerDomainPredicate, OwnerDomainScope, OwnerFeedbackError, OwnerFeedbackPolicy,
    OwnerGuardedDomain, OwnerGuardedError, OwnerGuardedEvent, OwnerGuardedFailure,
    OwnerGuardedImage, OwnerGuardedLimits, OwnerGuardedResidualKind, OwnerGuardedStats,
    OwnerGuardedSuccessor, OwnerOverlayLimits, OwnerOverlayMetadata, OwnerOverlayUsage,
    OwnerSuccessorError, OwnerSuccessorFailure, OwnerSuccessorLimits, OwnerSuccessorRegion,
    OwnerSuccessorStats, OwnerSuccessorTransition, RoutedCandidateLimits, RoutedCandidateReducer,
};
pub use case::{
    AffineCase, AffineGeometryError, AffineIntersection, Case, CaseIntersectionBudget,
    CaseIntersectionError, CaseIntersectionFailure, CaseIntersectionLimits, CaseIntersectionResult,
    CaseIntersectionStats, CoordinateCase,
};
pub(crate) use case::{AffineRestrictionChart, canonical_equalities};
pub use cuts::{LinearCutError, LinearCutPreparation, LinearCutRule, prepare_linear_cuts};
pub use discovery::{
    CoefficientVariableOrder, DiscoveryStats, MaterializationEvent, SymbolicExactBackend,
};
pub use error::SolverError;
pub use exception::{ExceptionError, ExceptionalConditions, extract_exceptions};
pub use execution::{
    SectorCompleted, SectorExecutionError, SectorExecutor, SectorExecutorBuildError,
    SectorScheduling,
};
pub use geometry::GeometryError;
pub use index::{Integral, IntegralOrder, Power, PowerError};
pub use numeric::{NumericResult, NumericStats, NumericalExactBackend};
pub(crate) use precondition::PreconditionProvenance;
pub use precondition::{precondition, precondition_with_variable_order};
pub use row::{ExactRow, PolynomialRow, Row, Term};
pub use search::{
    RuleCandidate, SearchEvent, SearchOptions, SearchStats, SectorConfig, SectorSolver, SeedSource,
};
pub use sector::{
    FiniteCaseLimits, FiniteCasePolicy, FiniteRetentionError, SectorDomainSolution, SectorEvent,
    SectorPhase, SectorRule, SectorSolution, SectorSolveError, SectorSolveOptions, SectorStats,
};
pub use seed::{Seed, Seeds};
pub use source::SourceSystem;

/// Aggregate diagnostics returned by the topology-neutral family solver.
///
/// This is intentionally a diagnostic surface rather than an authenticated
/// closing artifact. Callers provide the family and an explicit sector
/// manifest; no topology names or built-in fixtures are consulted. A future
/// artifact publisher can consume the per-sector solutions directly while
/// retaining the same generic entry point.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FamilySolveSummary {
    pub sectors: usize,
    pub solved_sectors: usize,
    pub rules: usize,
    pub finite_residuals: usize,
    pub elapsed: Duration,
}

/// Failure while preparing or executing a generic family solve.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FamilySolveError {
    Source(String),
    Executor(String),
    Execution(String),
}

impl std::fmt::Display for FamilySolveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(error) => write!(formatter, "source preparation failed: {error}"),
            Self::Executor(error) => write!(formatter, "sector executor failed: {error}"),
            Self::Execution(error) => write!(formatter, "family sector solve failed: {error}"),
        }
    }
}

impl std::error::Error for FamilySolveError {}

/// Solve an explicitly supplied sector manifest for any authenticated family.
///
/// The source system and sector executor are shared exactly as in the
/// production lanes. This helper deliberately does not infer or dispatch on
/// topology names; callers may construct the manifest from their own input
/// schema (including four-loop and higher-loop families) and choose worker and
/// search settings independently. The returned counts are discovery
/// diagnostics only and do not certify closure or publish an artifact.
pub fn solve_family<const N: usize>(
    family: &IntegralFamily,
    sectors: &[[bool; N]],
    workers: usize,
    config: SectorConfig<N>,
    options: SectorSolveOptions,
) -> Result<FamilySolveSummary, FamilySolveError> {
    if family.denominator_count() != N {
        return Err(FamilySolveError::Source(format!(
            "family has {} denominators but the sector manifest has arity {N}",
            family.denominator_count()
        )));
    }
    if sectors.is_empty() {
        return Ok(FamilySolveSummary::default());
    }
    let started = Instant::now();
    let sources = SourceSystem::from_family(family)
        .map_err(|error| FamilySolveError::Source(error.to_string()))?;
    let executor = SectorExecutor::new(workers)
        .map_err(|error| FamilySolveError::Executor(error.to_string()))?;
    let completed = executor
        .map(&sources, sectors, &config, options, |completed| {
            Ok::<_, std::convert::Infallible>((
                completed.solution.rules.len(),
                completed.solution.finite_residuals.len(),
            ))
        })
        .map_err(|error| FamilySolveError::Execution(error.to_string()))?;
    let (rules, finite_residuals) = completed.into_iter().fold(
        (0usize, 0usize),
        |(rules, residuals), (sector_rules, sector_residuals)| {
            (
                rules.saturating_add(sector_rules),
                residuals.saturating_add(sector_residuals),
            )
        },
    );
    Ok(FamilySolveSummary {
        sectors: sectors.len(),
        solved_sectors: sectors.len(),
        rules,
        finite_residuals,
        elapsed: started.elapsed(),
    })
}

#[cfg(test)]
mod tests;
