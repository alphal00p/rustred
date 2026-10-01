//! Optional exact-rule portfolio. No trial mutates the exceptional-case queue.

use std::cmp::Ordering;
use std::time::Instant;

use super::*;
use crate::solver::search::TrialSearchError;
use crate::solver::{SourceDiscoveryStrategy, SourceVisitOrder};

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
        let baseline_order = self.source_visit_order().map(SourceVisitOrder::ordinals);
        let mut best =
            self.rule_trial(current, options, baseline_order, None, 0, stats, observe)?;
        let mut best_features = best.features;
        let aggregate = stats
            .rule_selection
            .as_mut()
            .expect("baseline records portfolio stats");
        if !best_features.triggered(trigger) {
            aggregate.trigger_skips += alternatives.len();
            return Ok(best);
        }
        let mut winner = 0;
        for (index, alternative) in alternatives.iter().enumerate() {
            let order = source_order(alternative);
            let duplicate = same_order(order, baseline_order, self.basis.len())
                || alternatives[..index]
                    .iter()
                    .any(|prior| same_order(order, source_order(prior), self.basis.len()));
            if duplicate {
                stats.rule_selection.as_mut().unwrap().duplicate_skips += 1;
                continue;
            }
            let candidate = match self.rule_trial(
                current,
                options,
                order,
                Some(*limits),
                index + 1,
                stats,
                observe,
            ) {
                Ok(candidate) => candidate,
                Err(error) if optional_refusal(&error).is_some() => continue,
                Err(error) => return Err(error),
            };
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

    fn rule_trial(
        &self,
        current: &Case<N>,
        options: SectorSolveOptions,
        order: Option<&[usize]>,
        limits: Option<RuleTrialLimits>,
        trial: usize,
        stats: &mut SectorStats,
        observe: &mut impl FnMut(SectorEvent<'_, N>),
    ) -> Result<Admitted<N>, SectorSolveError<N>> {
        let mut work = RuleTrialStats::default();
        let mut budget = None;
        let result = (|| {
            self.validate_search(current, options.symbolic)
                .map_err(|source| SectorSolveError::Search {
                    case: current.clone(),
                    source,
                })?;
            let (candidate, searched) = self.search_attempt(
                current.clone(),
                options.symbolic,
                order,
                limits,
                true,
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
                        // Private transport; callers distinguish the typed budget
                        // before public propagation. It is never a rule/coverage.
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
            Ok(admitted)
        })();
        let outcome = match &result {
            Ok(_) => RuleTrialOutcome::Admitted,
            Err(_) if budget.is_some() => RuleTrialOutcome::WorkLimit(budget.unwrap()),
            Err(error) if trial != 0 => optional_refusal(error).unwrap_or(RuleTrialOutcome::Fatal),
            Err(_) => RuleTrialOutcome::Fatal,
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
        // Work includes losers/refusals; logical case/discard counts belong only
        // to the eventual winner and are updated by the queue itself.
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
