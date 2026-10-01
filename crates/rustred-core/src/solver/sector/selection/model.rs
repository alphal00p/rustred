//! Runtime rule preferences are heuristics, never coverage or authority.

use std::time::Duration;

use crate::solver::{SearchStats, SolverError, SourceDiscoveryStrategy};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum RuleSelectionPolicy {
    #[default]
    FirstValid,
    BoundedPortfolio {
        alternatives: Vec<SourceDiscoveryStrategy>,
        limits: RuleTrialLimits,
        quality: Vec<RuleQualityPriority>,
        trigger: RulePortfolioTrigger,
    },
}

/// Optional-trial input limits; these do not preempt native algebra operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleTrialLimits {
    pub max_depth: u32,
    pub max_rows: usize,
    pub max_exact_trace_rows: usize,
    pub max_exact_trace_terms: usize,
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
        } = self
        else {
            return Ok(());
        };
        if !(1..=2).contains(&alternatives.len()) {
            return Err(invalid("rule portfolio requires one or two alternatives"));
        }
        for alternative in alternatives {
            alternative.validate(arity)?;
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
                if let Some(plan) = alternative.materialize(basis)? {
                    *alternative = SourceDiscoveryStrategy::Materialized(plan);
                }
            }
        }
        Ok(())
    }
}

fn validate_features(
    features: impl Iterator<Item = RuleQualityFeature>,
) -> Result<(), SolverError> {
    let mut seen = [false; 12];
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
            "rule portfolio requires one through twelve features",
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
    pub total: RuleTrialStats,
}

impl RuleSelectionStats {
    pub(super) fn record(&mut self, summary: RuleTrialSummary) {
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
    }
}
