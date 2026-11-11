//! Runtime rule preferences are heuristics, never coverage or authority.

use std::time::Duration;

use crate::solver::{CaseSeedSchedule, SearchStats, SolverError, SourceDiscoveryStrategy};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleTrialDescriptor {
    pub source_discovery: SourceDiscoveryStrategy,
    pub seed_schedule: CaseSeedSchedule,
}

impl From<SourceDiscoveryStrategy> for RuleTrialDescriptor {
    fn from(source_discovery: SourceDiscoveryStrategy) -> Self {
        Self {
            source_discovery,
            seed_schedule: CaseSeedSchedule::Reference,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum RuleSelectionPolicy {
    #[default]
    FirstValid,
    BoundedPortfolio {
        alternatives: Vec<RuleTrialDescriptor>,
        limits: RuleTrialLimits,
        quality: Vec<RuleQualityPriority>,
        trigger: RulePortfolioTrigger,
        /// Opt-in only: reject a strictly worse known quality prefix before
        /// guard extraction. Unknown priorities and ties always proceed.
        pre_screen: bool,
    },
}

/// Optional-trial input limits; these do not preempt native algebra operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleTrialLimits {
    pub max_depth: u32,
    pub max_rows: usize,
    pub max_exact_trace_rows: usize,
    pub max_exact_trace_terms: usize,
    /// Explicit optional-trial budget: 2 (old behavior), 8, or 32.
    pub max_alternatives: usize,
    /// Cooperative case budget including the mandatory baseline. Checked at
    /// optional-trial boundaries; no individual native lift is preempted.
    pub max_case_elapsed: Option<Duration>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleQualityFeature {
    /// Maximum per-RHS-term inactive-axis negative displacement proxy.
    MaxNumeratorShiftExcursion,
    /// Sum of that proxy across RHS terms, not a rank bound.
    TotalNumeratorShiftExcursion,
    /// Maximum per-RHS-term active-axis positive displacement proxy.
    MaxPositiveShiftExcursion,
    /// Length of the admitted, unpruned original-OR-order child vector.
    ExceptionalCases,
    AffineExceptionalCases,
    GuardBranches,
    GuardPredicates,
    RhsTerms,
    CoefficientMonomials,
    SourceRows,
    SearchRows,
    /// Sum of per-RHS-term active-axis positive displacement proxies.
    /// Unlike the maximum, this distinguishes repeated active-power increases
    /// across children. It is neither net descent nor a physical power bound.
    TotalPositiveShiftExcursion,
    /// Number of DISTINCT original-support axes whose positive degree is
    /// increased in at least one RHS term. Repeated increases count once.
    DistinctRaisedSupportLines,
    /// Positive part of the maximum SIGNED net inactive-degree change of one
    /// RHS term. Negative and positive axis changes cancel before clipping.
    /// Symbolic coordinates use relative shifts; numeric coordinates use the
    /// actual max(-n,0) degree. This is a score, not a domain-wide rank bound.
    PositiveMaxNetInactiveDegreeShift,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleQualityPriority {
    pub feature: RuleQualityFeature,
    pub descending: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RulePortfolioTrigger {
    Always,
    AnyAtLeast(Vec<RuleQualityThreshold>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleQualityThreshold {
    pub feature: RuleQualityFeature,
    pub minimum: u64,
}

impl RuleSelectionPolicy {
    pub fn validate(&self, arity: usize) -> Result<(), SolverError> {
        let Self::BoundedPortfolio {
            alternatives,
            limits,
            quality,
            trigger,
            ..
        } = self
        else {
            return Ok(());
        };
        if ![2, 8, 32].contains(&limits.max_alternatives)
            || alternatives.is_empty()
            || alternatives.len() > limits.max_alternatives
        {
            return Err(invalid(
                "rule portfolio alternatives must fit an explicit 2, 8 or 32 trial budget",
            ));
        }
        for alternative in alternatives {
            alternative.source_discovery.validate(arity)?;
            alternative.seed_schedule.validate(arity)?;
        }
        if limits.max_rows == 0
            || limits.max_exact_trace_rows == 0
            || limits.max_exact_trace_terms == 0
        {
            return Err(invalid(
                "rule portfolio row and trace limits must be positive",
            ));
        }
        validate_features(quality.iter().map(|p| p.feature))?;
        if let RulePortfolioTrigger::AnyAtLeast(thresholds) = trigger {
            validate_features(thresholds.iter().map(|p| p.feature))?;
        }
        Ok(())
    }

    pub(in crate::solver) fn materialize<const N: usize>(
        &mut self,
        basis: &[crate::solver::PolynomialRow<N>],
    ) -> Result<(), SolverError> {
        if let Self::BoundedPortfolio { alternatives, .. } = self {
            for alternative in alternatives {
                if let Some(plan) = alternative.source_discovery.materialize(basis)? {
                    alternative.source_discovery = SourceDiscoveryStrategy::Materialized(plan);
                }
            }
        }
        Ok(())
    }
}

fn validate_features(
    features: impl Iterator<Item = RuleQualityFeature>,
) -> Result<(), SolverError> {
    let mut seen = [false; 14];
    let mut count = 0;
    for feature in features {
        let slot = &mut seen[feature as usize];
        if std::mem::replace(slot, true) {
            return Err(invalid("rule portfolio features must not repeat"));
        }
        count += 1;
    }
    if count == 0 || count > seen.len() {
        return Err(invalid(
            "rule portfolio requires one through fourteen features",
        ));
    }
    Ok(())
}

pub(super) fn invalid(message: &str) -> SolverError {
    SolverError::InvalidInput(message.into())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleTrialBudget {
    SourceRows,
    ExactTraceRows,
    ExactTraceTerms,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleTrialOutcome {
    Admitted,
    SearchExhausted,
    WorkLimit(RuleTrialBudget),
    UnluckySample,
    UnsupportedGeometry,
    GeometryBudget,
    NonProgress,
    Fatal,
    /// Same actual source order AND seed schedule as an earlier descriptor.
    DuplicateDescriptor,
    /// Same canonical case/target/RHS/exceptions as an earlier admitted trial.
    DuplicateCandidate,
    /// The optional trial did not have the baseline's fully recentred target.
    DifferentTarget,
    /// Cooperative case budget stopped this optional trial or its admission.
    CaseTimeBudget,
    /// Exact search, target matching and strict descent passed, but the known
    /// pre-guard quality prefix was strictly worse than the incumbent's.
    PreScreened,
}

/// Work of one trial, including rejected/fatal trials. `search.elapsed`
/// includes exact lifting; `search.exact_materialization` is a sub-duration.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuleTrialStats {
    pub search: SearchStats,
    pub exact_trace_terms: usize,
    pub exact_lifts: usize,
    pub guard_branches: usize,
    pub geometry_calls: usize,
    pub guard_extraction: Duration,
    pub geometry: Duration,
    pub elapsed: Duration,
    pub case_elapsed: Duration,
    /// All 14 literal features, indexed by RuleQualityFeature discriminant.
    pub score: Option<[u64; 14]>,
    /// Actual post-canonical RHS features, separate from a fully admitted
    /// score. Present only when pre-screening is explicitly enabled.
    pub pre_guard_rhs_terms: Option<u64>,
    pub pre_guard_numerator_rank_raise: Option<u64>,
    pub target_matches_baseline: Option<bool>,
    pub duplicate_of_trial: Option<usize>,
    /// Actual seed displacement shells visited, not just the configured cap.
    pub seed_first_depth: Option<u32>,
    pub seed_last_depth: Option<u32>,
}

#[derive(Clone, Copy, Debug)]
pub struct RuleTrialSummary {
    pub trial: usize,
    pub outcome: RuleTrialOutcome,
    pub stats: RuleTrialStats,
}

/// Invocation-local accounting, not rule authority or proof of improvement.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuleSelectionStats {
    pub attempted: usize,
    pub admitted: usize,
    pub rejected: usize,
    pub selected_alternatives: usize,
    pub trigger_skips: usize,
    pub duplicate_skips: usize,
    pub search_exhausted: usize,
    pub work_limited: usize,
    pub unlucky_samples: usize,
    pub unsupported_geometry: usize,
    pub geometry_budget: usize,
    pub non_progress: usize,
    pub fatal: usize,
    /// Every trial that completed exact rule and domain admission, including
    /// duplicate candidates and valid results discarded by the cooperative cap.
    pub completed_admissions: usize,
    pub canonical_distinct_completed: usize,
    pub candidate_duplicates: usize,
    pub different_targets: usize,
    pub case_time_skips: usize,
    pub pre_screened: usize,
    pub total: RuleTrialStats,
}

impl RuleSelectionStats {
    pub(super) fn record(&mut self, summary: RuleTrialSummary) {
        if summary.outcome == RuleTrialOutcome::DuplicateDescriptor {
            self.duplicate_skips += 1;
            return;
        }
        if summary.outcome == RuleTrialOutcome::CaseTimeBudget
            && summary.stats.search.rows == 0
            && summary.stats.exact_lifts == 0
        {
            self.case_time_skips += 1;
            return;
        }
        if summary.stats.score.is_some() {
            self.completed_admissions += 1;
            if summary.stats.duplicate_of_trial.is_none() {
                self.canonical_distinct_completed += 1;
            }
        }
        self.attempted += 1;
        match summary.outcome {
            RuleTrialOutcome::Admitted => self.admitted += 1,
            outcome => {
                self.rejected += 1;
                match outcome {
                    RuleTrialOutcome::SearchExhausted => self.search_exhausted += 1,
                    RuleTrialOutcome::WorkLimit(_) => self.work_limited += 1,
                    RuleTrialOutcome::UnluckySample => self.unlucky_samples += 1,
                    RuleTrialOutcome::UnsupportedGeometry => self.unsupported_geometry += 1,
                    RuleTrialOutcome::GeometryBudget => self.geometry_budget += 1,
                    RuleTrialOutcome::NonProgress => self.non_progress += 1,
                    RuleTrialOutcome::Fatal => self.fatal += 1,
                    RuleTrialOutcome::DuplicateCandidate => self.candidate_duplicates += 1,
                    RuleTrialOutcome::DifferentTarget => self.different_targets += 1,
                    RuleTrialOutcome::CaseTimeBudget => self.case_time_skips += 1,
                    RuleTrialOutcome::PreScreened => self.pre_screened += 1,
                    RuleTrialOutcome::DuplicateDescriptor => unreachable!(),
                    RuleTrialOutcome::Admitted => unreachable!(),
                }
            }
        }
        let from = summary.stats;
        let to = &mut self.total;
        to.search.seeds += from.search.seeds;
        to.search.rows += from.search.rows;
        to.search.independent_rows += from.search.independent_rows;
        to.search.exact_trace_rows += from.search.exact_trace_rows;
        to.search.elapsed += from.search.elapsed;
        to.search.exact_materialization += from.search.exact_materialization;
        // A reducer snapshot or a direct-hit boolean has no additive meaning.
        to.exact_trace_terms += from.exact_trace_terms;
        to.exact_lifts += from.exact_lifts;
        to.guard_branches += from.guard_branches;
        to.geometry_calls += from.geometry_calls;
        to.guard_extraction += from.guard_extraction;
        to.geometry += from.geometry;
        to.elapsed += from.elapsed;
        if let Some(first) = from.seed_first_depth {
            to.seed_first_depth = Some(to.seed_first_depth.map_or(first, |prior| prior.min(first)));
        }
        if let Some(last) = from.seed_last_depth {
            to.seed_last_depth = Some(to.seed_last_depth.map_or(last, |prior| prior.max(last)));
        }
    }
}
