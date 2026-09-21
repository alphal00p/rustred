//! Explicit prospective source searches and immutable partial-domain overlays.
use std::fmt;
use std::sync::Arc;

use super::super::CandidateReductionError;
use super::super::preparation::shared::prepare_batch;
use super::model::*;
use crate::sector::OrderingPolicy;
use crate::solver::{
    Case, CaseIntersectionLimits, CoefficientVariableOrder, FiniteCaseLimits, FiniteCasePolicy,
    NumericalExactBackend, SearchOptions, SectorConfig, SectorDomainSolution, SectorEvent,
    SectorSolveError, SectorSolveOptions, SectorSolver, SectorStats, SolverError,
    SymbolicExactBackend,
};

mod limits;
pub(super) use limits::OwnerOverlayUsage as OverlayUsage;
pub use limits::{OwnerOverlayLimits, OwnerOverlayUsage};

/// Caller-selected policy for NEW searches, not historical artifact authority.
#[derive(Clone, Copy, Debug)]
pub struct OwnerFeedbackPolicy {
    pub numerical_depth: u32,
    pub symbolic: SearchOptions,
    pub symbolic_exact_backend: SymbolicExactBackend,
    pub numerical_exact_backend: NumericalExactBackend,
    pub coefficient_variable_order: CoefficientVariableOrder,
}
impl Default for OwnerFeedbackPolicy {
    fn default() -> Self {
        Self {
            numerical_depth: SectorSolveOptions::default().numerical_depth,
            symbolic: Default::default(),
            symbolic_exact_backend: Default::default(),
            numerical_exact_backend: Default::default(),
            coefficient_variable_order: Default::default(),
        }
    }
}

/// Actual domain scope; independent of the public campaign's entry-rank limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnerDomainScope {
    pub max_numerator_rank: Option<u32>,
    pub finite_case_policy: FiniteCasePolicy,
}

#[derive(Clone, Copy, Debug)]
pub struct OwnerDomainAttemptLimits {
    pub max_requested_cases: usize,
    pub finite_case_limits: FiniteCaseLimits,
    pub case_intersection_limits: CaseIntersectionLimits,
    pub max_symbolic_cases: Option<usize>,
}
impl Default for OwnerDomainAttemptLimits {
    fn default() -> Self {
        Self {
            max_requested_cases: 10_000,
            finite_case_limits: Default::default(),
            case_intersection_limits: Default::default(),
            max_symbolic_cases: None,
        }
    }
}

#[derive(Debug)]
pub enum OwnerFeedbackError<const N: usize> {
    Candidate(CandidateReductionError),
    Source(SolverError),
    Search(Box<SectorSolveError<N>>),
    InvalidInput(String),
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
}
impl<const N: usize> From<CandidateReductionError> for OwnerFeedbackError<N> {
    fn from(error: CandidateReductionError) -> Self {
        Self::Candidate(error)
    }
}
impl<const N: usize> fmt::Display for OwnerFeedbackError<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Candidate(error) => error.fmt(f),
            Self::Source(error) => error.fmt(f),
            Self::Search(error) => error.fmt(f),
            Self::InvalidInput(error) => write!(f, "invalid owner feedback: {error}"),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                f,
                "owner feedback {resource} requires {requested}; limit is {limit}"
            ),
        }
    }
}
impl<const N: usize> std::error::Error for OwnerFeedbackError<N> {}

/// Immutable partial-domain metadata; no whole-sector or recursive closure claim.
#[derive(Debug)]
pub struct OwnerOverlayMetadata<const N: usize> {
    pub requested_cases: Vec<Case<N>>,
    pub scope: OwnerDomainScope,
    pub policy: OwnerFeedbackPolicy,
    pub attempt_limits: OwnerDomainAttemptLimits,
    pub stats: SectorStats,
}

/// Shares the existing ordinary+LI sources; preconditioning stays sector-local.
#[derive(Debug)]
pub struct BoundOwnerSearch<const N: usize> {
    lineage: Arc<()>,
    context: Arc<CandidateOwnerContext<N>>,
    owner: Arc<PreparedOwner<N>>,
    sector: [bool; N],
    policy: OwnerFeedbackPolicy,
    permutation: Option<[usize; N]>,
}

/// Produced only by a bound source search, never from an arbitrary partial result.
#[derive(Debug)]
pub struct BoundOwnerOverlay<const N: usize> {
    lineage: Arc<()>,
    sector: [bool; N],
    root: [bool; N],
    ordering: OrderingPolicy,
    policy: OwnerFeedbackPolicy,
    attempt_limits: OwnerDomainAttemptLimits,
    solution: SectorDomainSolution<N>,
}
impl<const N: usize> BoundOwnerOverlay<N> {
    pub fn owner_sector(&self) -> &[bool; N] {
        &self.sector
    }
    pub fn requested_cases(&self) -> &[Case<N>] {
        &self.solution.requested_cases
    }
    pub fn scope(&self) -> OwnerDomainScope {
        OwnerDomainScope {
            max_numerator_rank: self.solution.max_numerator_rank,
            finite_case_policy: self.solution.finite_case_policy,
        }
    }
    pub fn policy(&self) -> OwnerFeedbackPolicy {
        self.policy
    }
    pub fn stats(&self) -> SectorStats {
        self.solution.stats
    }
    pub fn rule_count(&self) -> usize {
        self.solution.rules.len()
    }
    pub fn terminal_count(&self) -> usize {
        self.solution.finite_residuals.len()
    }
    /// Borrow the distinct partial-domain native payload, without install or
    /// complete-sector authority. Intended for reporting or future native IO.
    pub fn partial_solution(&self) -> &SectorDomainSolution<N> {
        &self.solution
    }
    /// Measure a borrowed raw result for bounded staging, without expression
    /// clones. Prepared output has additional denominator storage and is
    /// independently measured during append; this is not its byte bound.
    pub fn raw_payload_usage(
        &self,
        limits: OwnerOverlayLimits,
    ) -> Result<OwnerOverlayUsage, OwnerFeedbackError<N>> {
        let mut usage = OwnerOverlayUsage::default();
        usage.admit_solution(&self.solution, limits)?;
        Ok(usage)
    }
}

impl<const N: usize> CandidateOwnerPrograms<N> {
    /// Bind future work to this immutable lineage and exact owner order.
    /// The policy is explicit and prospective: no missing historical settings
    /// are inferred. Duplicate job scheduling belongs to the caller.
    pub fn bind_owner_search(
        self: &Arc<Self>,
        sector: [bool; N],
        policy: OwnerFeedbackPolicy,
    ) -> Result<BoundOwnerSearch<N>, OwnerFeedbackError<N>> {
        let owner = self.owners.get(&sector).ok_or_else(|| {
            OwnerFeedbackError::InvalidInput("source search requires an installed owner".into())
        })?;
        if !owner.ordering.is_spired() {
            return Err(OwnerFeedbackError::InvalidInput(
                "source feedback currently requires an uncut SpIReD owner ordering".into(),
            ));
        }
        let permutation = owner
            .ordering
            .try_coordinate_priority()
            .map_err(|error| OwnerFeedbackError::InvalidInput(error.to_string()))?
            .map(|priority| {
                let mut slots = [0; N];
                // Native ordering metadata is rank-by-slot; solver input is
                // slot-by-priority. Admission already checked its arity.
                for (slot, &rank) in priority.rank_by_slot().iter().enumerate() {
                    slots[rank] = slot;
                }
                slots
            });
        Ok(BoundOwnerSearch {
            lineage: self.lineage.clone(),
            context: self.context.clone(),
            owner: owner.clone(),
            sector,
            policy,
            permutation,
        })
    }

    /// Atomically append partial results in caller-supplied deterministic order.
    /// Existing native expressions are Arc-shared, never cloned on publication.
    /// Limits cover cumulative overlays in this snapshot, not only this call.
    /// Old snapshots remain valid; graph memo reuse and persistence are absent.
    pub fn append_domain_overlays(
        self: &Arc<Self>,
        overlays: Vec<BoundOwnerOverlay<N>>,
        limits: OwnerOverlayLimits,
    ) -> Result<Arc<Self>, OwnerFeedbackError<N>> {
        let mut usage = self.overlay_usage;
        usage.validate(limits)?;
        // Validate every binding and the raw payload before any conversion.
        for overlay in &overlays {
            let owner = self.owners.get(&overlay.sector).ok_or_else(|| {
                OwnerFeedbackError::InvalidInput("overlay owner is not installed".into())
            })?;
            if !Arc::ptr_eq(&self.lineage, &overlay.lineage)
                || owner.root != overlay.root
                || owner.ordering != overlay.ordering
            {
                return Err(OwnerFeedbackError::InvalidInput(
                    "overlay belongs to another program lineage or owner order".into(),
                ));
            }
            usage.admit_solution(&overlay.solution, limits)?;
        }
        let mut owners = self.owners.clone(); // Arc handles only.
        let mut actual_usage = self.overlay_usage;
        for overlay in overlays {
            let owner = &owners[&overlay.sector];
            let mut ordinal = owner
                .batches
                .iter()
                .try_fold(0_usize, |sum, batch| sum.checked_add(batch.rules.len()))
                .ok_or_else(|| {
                    OwnerFeedbackError::InvalidInput("owner rule ordinal overflow".into())
                })?;
            let scope = overlay.scope();
            let SectorDomainSolution {
                requested_cases,
                rules,
                finite_residuals,
                stats,
                ..
            } = overlay.solution;
            let (rules, terminals) = prepare_batch(
                &self.context.shared,
                overlay.sector,
                rules,
                finite_residuals,
                self.context.limits,
                &mut ordinal,
            )?;
            let metadata = OwnerOverlayMetadata {
                requested_cases,
                scope,
                policy: overlay.policy,
                attempt_limits: overlay.attempt_limits,
                stats,
            };
            let batch = Arc::new(PreparedOwnerBatch::new(rules, terminals, Some(metadata)));
            // Native conversion may retain additional original denominators.
            // Check their actual owned payload before publishing any snapshot.
            actual_usage.admit_batch(&batch, limits)?;
            let mut batches = owner.batches.clone();
            batches.push(batch);
            owners.insert(
                overlay.sector,
                Arc::new(PreparedOwner {
                    root: owner.root,
                    ordering: owner.ordering,
                    batches,
                }),
            );
        }
        Ok(Arc::new(Self {
            context: self.context.clone(),
            lineage: self.lineage.clone(),
            owners,
            overlay_usage: actual_usage,
        }))
    }

    pub fn overlays(&self, sector: &[bool; N]) -> impl Iterator<Item = &OwnerOverlayMetadata<N>> {
        self.owners
            .get(sector)
            .into_iter()
            .flat_map(|owner| &owner.batches)
            .filter_map(|batch| batch.overlay.as_ref())
    }
    pub fn overlay_usage(&self) -> OwnerOverlayUsage {
        self.overlay_usage
    }
}

impl<const N: usize> BoundOwnerSearch<N> {
    pub fn owner_sector(&self) -> &[bool; N] {
        &self.sector
    }
    pub fn policy(&self) -> OwnerFeedbackPolicy {
        self.policy
    }

    /// Search exactly the nominated domains with the ordinary exceptional queue.
    /// A successful local queue does not assert recursive successor coverage.
    /// No cancellation/preemption inside a native call is promised by this API.
    pub fn solve_domains_with_observer(
        &self,
        cases: Vec<Case<N>>,
        scope: OwnerDomainScope,
        limits: OwnerDomainAttemptLimits,
        observe: impl FnMut(SectorEvent<'_, N>),
    ) -> Result<BoundOwnerOverlay<N>, OwnerFeedbackError<N>> {
        if cases.is_empty() {
            return Err(OwnerFeedbackError::InvalidInput(
                "missing-domain request is empty".into(),
            ));
        }
        limits::check(cases.len(), limits.max_requested_cases, "requested domains")?;
        if scope.finite_case_policy == FiniteCasePolicy::RetainRankFinite
            && scope.max_numerator_rank.is_none()
        {
            return Err(OwnerFeedbackError::InvalidInput(
                "finite retention requires an explicit domain rank".into(),
            ));
        }
        let solver = SectorSolver::new(
            &self.context.shared.sources,
            self.sector,
            SectorConfig {
                permutation: self.permutation,
                zero_sectors: self
                    .context
                    .shared
                    .zero_sectors
                    .iter()
                    .copied()
                    .collect::<Vec<_>>()
                    .into(),
                symbolic_exact_backend: self.policy.symbolic_exact_backend,
                numerical_exact_backend: self.policy.numerical_exact_backend,
                coefficient_variable_order: self.policy.coefficient_variable_order,
                ..Default::default()
            },
        )
        .map_err(OwnerFeedbackError::Source)?;
        let solution = solver
            .solve_domains_with_observer(
                cases,
                SectorSolveOptions {
                    max_numerator_rank: scope.max_numerator_rank,
                    finite_case_policy: scope.finite_case_policy,
                    finite_case_limits: limits.finite_case_limits,
                    case_intersection_limits: limits.case_intersection_limits,
                    symbolic: self.policy.symbolic,
                    numerical_depth: self.policy.numerical_depth,
                    max_symbolic_cases: limits.max_symbolic_cases,
                },
                observe,
            )
            .map_err(|error| OwnerFeedbackError::Search(Box::new(error)))?;
        Ok(BoundOwnerOverlay {
            lineage: self.lineage.clone(),
            sector: self.sector,
            root: self.owner.root,
            ordering: self.owner.ordering,
            policy: self.policy,
            attempt_limits: limits,
            solution,
        })
    }
}

#[cfg(test)]
mod tests;
