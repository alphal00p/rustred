//! Optional exact-rule portfolio. No trial mutates the exceptional-case queue.
use super::*;
use crate::solver::search::TrialSearchError;
use crate::solver::{CaseSeedSchedule, SourceDiscoveryStrategy, SourceVisitOrder};
use std::cmp::Ordering;
use std::time::Instant;
mod model;
mod quality;
pub use model::*;
use quality::Features;

pub(super) struct Admitted<const N: usize> {
    pub rule: SectorRule<N>,
    pub children: Vec<Case<N>>,
    pub discarded: usize,
    features: Features,
}

// Exact canonical relation identity; sources are witnesses, not its identity.
struct CandidateIdentity<const N: usize> {
    case: Case<N>,
    target: Integral<N>,
    rhs: crate::solver::ExactRow<N>,
    exceptions: Vec<Vec<crate::algebra::CoefficientPolynomial>>,
}
impl<const N: usize> CandidateIdentity<N> {
    fn read(candidate: &Admitted<N>) -> Self {
        let rule = &candidate.rule;
        Self {
            case: rule.candidate.case.clone(),
            target: rule.candidate.target,
            rhs: rule.candidate.rhs.clone(),
            exceptions: rule.exceptions.branches.clone(),
        }
    }
    fn matches(&self, candidate: &Admitted<N>) -> bool {
        let rule = &candidate.rule;
        self.case == rule.candidate.case
            && self.target == rule.candidate.target
            && self.rhs == rule.candidate.rhs
            && self.exceptions == rule.exceptions.branches
    }
}

impl<const N: usize> SectorSolver<'_, N> {
    pub(super) fn select_rule(
        &self,
        current: &Case<N>,
        options: SectorSolveOptions,
        stats: &mut SectorStats,
        observe: &mut impl FnMut(SectorEvent<'_, N>),
    ) -> Result<Admitted<N>, SectorSolveError<N>> {
        let RuleSelectionPolicy::BoundedPortfolio {
            alternatives,
            limits,
            quality,
            trigger,
        } = &self.config.rule_selection
        else {
            unreachable!("first-valid uses the original queue branch")
        };
        let case_started = Instant::now();
        let baseline_order = self.source_visit_order().map(SourceVisitOrder::ordinals);
        let reference = CaseSeedSchedule::Reference;
        let mut best = self.rule_trial(
            current,
            options,
            baseline_order,
            &reference,
            None,
            0,
            None,
            &[],
            case_started,
            stats,
            observe,
        )?;
        let baseline_target = best.rule.candidate.target;
        let mut identities = vec![(0, CandidateIdentity::read(&best))];
        let mut best_features = best.features;
        if !best_features.triggered(trigger) {
            stats.rule_selection.as_mut().unwrap().trigger_skips += alternatives.len();
            return Ok(best);
        }
        let mut winner = 0;
        for (index, alternative) in alternatives.iter().enumerate() {
            if case_budget_expired(*limits, case_started) {
                self.skipped_trial(
                    current,
                    index + 1,
                    RuleTrialOutcome::CaseTimeBudget,
                    None,
                    case_started,
                    stats,
                    observe,
                );
                continue;
            }
            let order = source_order(&alternative.source_discovery);
            let duplicate_of = if alternative.seed_schedule == reference
                && same_order(order, baseline_order, self.basis.len())
            {
                Some(0)
            } else {
                alternatives[..index]
                    .iter()
                    .position(|prior| {
                        prior.seed_schedule == alternative.seed_schedule
                            && same_order(
                                order,
                                source_order(&prior.source_discovery),
                                self.basis.len(),
                            )
                    })
                    .map(|prior| prior + 1)
            };
            if let Some(prior) = duplicate_of {
                self.skipped_trial(
                    current,
                    index + 1,
                    RuleTrialOutcome::DuplicateDescriptor,
                    Some(prior),
                    case_started,
                    stats,
                    observe,
                );
                continue;
            }
            let candidate = match self.rule_trial(
                current,
                options,
                order,
                &alternative.seed_schedule,
                Some(*limits),
                index + 1,
                Some(&baseline_target),
                &identities,
                case_started,
                stats,
                observe,
            ) {
                Ok(candidate) => candidate,
                Err(error) if optional_refusal(&error).is_some() => continue,
                Err(error) => return Err(error),
            };
            identities.push((index + 1, CandidateIdentity::read(&candidate)));
            let features = candidate.features;
            if features.compare(&best_features, quality) == Ordering::Less {
                best = candidate;
                best_features = features;
                winner = index + 1;
            }
        }
        if winner != 0 {
            stats.rule_selection.as_mut().unwrap().selected_alternatives += 1;
        }
        Ok(best)
    }

    fn skipped_trial(
        &self,
        current: &Case<N>,
        trial: usize,
        outcome: RuleTrialOutcome,
        duplicate_of_trial: Option<usize>,
        case_started: Instant,
        stats: &mut SectorStats,
        observe: &mut impl FnMut(SectorEvent<'_, N>),
    ) {
        let summary = RuleTrialSummary {
            trial,
            outcome,
            stats: RuleTrialStats {
                case_elapsed: case_started.elapsed(),
                duplicate_of_trial,
                ..Default::default()
            },
        };
        stats.rule_selection.as_mut().unwrap().record(summary);
        observe(SectorEvent::RuleTrialFinished {
            case: current,
            summary,
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn rule_trial(
        &self,
        current: &Case<N>,
        options: SectorSolveOptions,
        order: Option<&[usize]>,
        schedule: &CaseSeedSchedule,
        limits: Option<RuleTrialLimits>,
        trial: usize,
        baseline_target: Option<&Integral<N>>,
        identities: &[(usize, CandidateIdentity<N>)],
        case_started: Instant,
        stats: &mut SectorStats,
        observe: &mut impl FnMut(SectorEvent<'_, N>),
    ) -> Result<Admitted<N>, SectorSolveError<N>> {
        let started_trial = Instant::now();
        let mut work = RuleTrialStats::default();
        let mut budget = None;
        let mut selection_refusal = None;
        let result = (|| {
            self.validate_search(current, options.symbolic)
                .map_err(|source| SectorSolveError::Search {
                    case: current.clone(),
                    source,
                })?;
            let (candidate, searched) = self.search_attempt_with_schedule(
                current.clone(),
                options.symbolic,
                order,
                limits,
                true,
                schedule,
                |event| {
                    observe(SectorEvent::Search {
                        case: current,
                        event,
                    })
                },
            );
            work = searched;
            let candidate = candidate.map_err(|error| {
                let source = match error {
                    TrialSearchError::Solver(source) => source,
                    TrialSearchError::Limit(kind) => {
                        budget = Some(kind);
                        SolverError::SearchExhausted {
                            depth: limits.unwrap().max_depth,
                            rows: work.search.rows,
                        }
                    }
                };
                SectorSolveError::Search {
                    case: current.clone(),
                    source,
                }
            })?;
            // The returned candidate already includes the search's full
            // canonicalization/recentering of coefficients and source seeds.
            if let Some(target) = baseline_target {
                work.target_matches_baseline = Some(candidate.target == *target);
                if candidate.target != *target {
                    selection_refusal = Some(RuleTrialOutcome::DifferentTarget);
                    return Err(SectorSolveError::NonProgress {
                        case: current.clone(),
                    });
                }
            }
            observe(SectorEvent::PhaseStarted {
                case: current,
                phase: SectorPhase::GuardExtraction,
            });
            let started = Instant::now();
            let finished = self.finish_rule(candidate);
            work.guard_extraction = started.elapsed();
            let (rule, _) = finished?;
            work.guard_branches = rule.exceptions.branches.len();
            observe(SectorEvent::PhaseStarted {
                case: current,
                phase: SectorPhase::ExceptionalGeometry,
            });
            let started = Instant::now();
            let admitted = rule.admit_exceptional_cases_counted(
                &self.system.indices,
                self.order.sector(),
                options.max_numerator_rank,
                options.case_intersection_limits,
                || work.geometry_calls += 1,
            );
            work.geometry = started.elapsed();
            let (children, discarded) = admitted?;
            if children.iter().any(|child| child == current) {
                return Err(SectorSolveError::NonProgress {
                    case: current.clone(),
                });
            }
            let mut admitted = Admitted {
                rule,
                children,
                discarded,
                features: Features::default(),
            };
            admitted.features =
                Features::read(&admitted, self.order.sector()).map_err(|source| {
                    SectorSolveError::Search {
                        case: current.clone(),
                        source,
                    }
                })?;
            work.score = Some(admitted.features.snapshot());
            if let Some((prior, _)) = identities
                .iter()
                .find(|(_, identity)| identity.matches(&admitted))
            {
                work.duplicate_of_trial = Some(*prior);
                selection_refusal = Some(RuleTrialOutcome::DuplicateCandidate);
                return Err(SectorSolveError::NonProgress {
                    case: current.clone(),
                });
            }
            // Cooperative, not preemptive: an indivisible native lift may
            // overrun. An over-budget optional result keeps the incumbent.
            if limits.is_some_and(|limits| case_budget_expired(limits, case_started)) {
                selection_refusal = Some(RuleTrialOutcome::CaseTimeBudget);
                return Err(SectorSolveError::NonProgress {
                    case: current.clone(),
                });
            }
            Ok(admitted)
        })();
        work.elapsed = started_trial.elapsed();
        work.case_elapsed = case_started.elapsed();
        let outcome = if let Some(refusal) = selection_refusal {
            refusal
        } else {
            match &result {
                Ok(_) => RuleTrialOutcome::Admitted,
                Err(_) if budget.is_some() => RuleTrialOutcome::WorkLimit(budget.unwrap()),
                Err(error) if trial != 0 => {
                    optional_refusal(error).unwrap_or(RuleTrialOutcome::Fatal)
                }
                Err(_) => RuleTrialOutcome::Fatal,
            }
        };
        let summary = RuleTrialSummary {
            trial,
            outcome,
            stats: work,
        };
        stats
            .rule_selection
            .get_or_insert_with(Default::default)
            .record(summary);
        stats.symbolic_rows += work.search.rows;
        stats.symbolic_search += work.search.elapsed;
        stats.exception_extraction += work.guard_extraction;
        stats.geometry += work.geometry;
        observe(SectorEvent::RuleTrialFinished {
            case: current,
            summary,
        });
        result
    }
}

fn case_budget_expired(limits: RuleTrialLimits, started: Instant) -> bool {
    limits
        .max_case_elapsed
        .is_some_and(|budget| started.elapsed() >= budget)
}

fn source_order(strategy: &SourceDiscoveryStrategy) -> Option<&[usize]> {
    match strategy {
        SourceDiscoveryStrategy::InputOrder => None,
        SourceDiscoveryStrategy::Materialized(plan) => Some(plan.ordinals()),
        SourceDiscoveryStrategy::Features(_) => {
            unreachable!("prepared portfolio has materialized features")
        }
    }
}

fn same_order(left: Option<&[usize]>, right: Option<&[usize]>, count: usize) -> bool {
    (0..count).all(|position| {
        left.map_or(position, |p| p[position]) == right.map_or(position, |p| p[position])
    })
}

fn optional_refusal<const N: usize>(error: &SectorSolveError<N>) -> Option<RuleTrialOutcome> {
    match error {
        SectorSolveError::Search {
            source: SolverError::SearchExhausted { .. },
            ..
        } => Some(RuleTrialOutcome::SearchExhausted),
        SectorSolveError::Search {
            source: SolverError::UnluckySample,
            ..
        } => Some(RuleTrialOutcome::UnluckySample),
        SectorSolveError::NonProgress { .. } => Some(RuleTrialOutcome::NonProgress),
        SectorSolveError::Geometry {
            source: AffineGeometryError::UnsupportedNonlinear { .. },
            ..
        } => Some(RuleTrialOutcome::UnsupportedGeometry),
        SectorSolveError::Geometry {
            source: AffineGeometryError::Coordinate(GeometryError::UnsupportedGeometry { .. }),
            ..
        } => Some(RuleTrialOutcome::UnsupportedGeometry),
        SectorSolveError::Intersection(error) => match error.failure {
            CaseIntersectionFailure::UnsupportedGeometry
            | CaseIntersectionFailure::RepeatedState
            | CaseIntersectionFailure::Admission(AffineGeometryError::UnsupportedNonlinear {
                ..
            })
            | CaseIntersectionFailure::Admission(AffineGeometryError::Coordinate(
                GeometryError::UnsupportedGeometry { .. },
            )) => Some(RuleTrialOutcome::UnsupportedGeometry),
            CaseIntersectionFailure::Budget { .. } => Some(RuleTrialOutcome::GeometryBudget),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests;
