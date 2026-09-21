//! Ordered exceptional-case traversal for the executable SpIRed source port.

use std::cmp::Ordering;
use std::fmt;
use std::time::{Duration, Instant};

use super::geometry::{GeometryError, compare_cases};
use super::{
    AffineGeometryError, Case, CaseIntersectionError, CaseIntersectionFailure,
    CaseIntersectionLimits, CoordinateCase, ExceptionError, ExceptionalConditions, Integral,
    RuleCandidate, SearchEvent, SearchOptions, SectorSolver, SolverError, extract_exceptions,
};

mod finite;
pub use finite::{FiniteCaseLimits, FiniteCasePolicy, FiniteRetentionError};

/// A solved equation together with the exact exceptional index conditions
/// on which it must NOT be applied. Coefficient parameters remain generic.
#[derive(Debug)]
pub struct SectorRule<const N: usize> {
    pub candidate: RuleCandidate<N>,
    pub exceptions: ExceptionalConditions,
}

impl<const N: usize> SectorRule<N> {
    /// Exact intersections of the rule's case with its exceptional
    /// branches, modulo sector signs. Broader faces subsume narrower ones.
    /// Unsupported geometry is an error, never an empty set.
    pub fn exceptional_cases(
        &self,
        indices: &[usize; N],
        sector: &[bool; N],
    ) -> Result<Vec<Case<N>>, SectorSolveError<N>> {
        let (admitted, _) = self.admit_exceptional_cases(indices, sector)?;
        let mut cases = Vec::new();
        for case in admitted {
            if prune_subsumed(&mut cases, &case).map_err(|source| SectorSolveError::Geometry {
                case: case.clone(),
                source,
            })? {
                cases.push(case);
            }
        }
        cases.sort_unstable_by(Case::queue_cmp);
        Ok(cases)
    }

    // Keep the complete OR private until every AND branch has been admitted.
    // Preserve original OR-branch order for the queue: globally pruning this
    // vector could remove an earlier numeric seed before a later symbolic
    // sibling is enqueued, changing the reference's shared seed chronology.
    fn admit_exceptional_cases(
        &self,
        indices: &[usize; N],
        sector: &[bool; N],
    ) -> Result<(Vec<Case<N>>, usize), SectorSolveError<N>> {
        self.admit_exceptional_cases_in_scope(
            indices,
            sector,
            None,
            CaseIntersectionLimits::default(),
        )
    }

    fn admit_exceptional_cases_in_scope(
        &self,
        indices: &[usize; N],
        sector: &[bool; N],
        max_numerator_rank: Option<u32>,
        limits: CaseIntersectionLimits,
    ) -> Result<(Vec<Case<N>>, usize), SectorSolveError<N>> {
        let mut cases = Vec::new();
        let mut discarded = 0;
        for branch in &self.exceptions.branches {
            let intersection = match max_numerator_rank {
                None => self
                    .candidate
                    .case
                    .intersect_many(branch, indices, sector, limits),
                Some(maximum) => self.candidate.case.intersect_many_with_max_numerator_rank(
                    branch, indices, sector, limits, maximum,
                ),
            }
            .map_err(|source| SectorSolveError::Intersection(Box::new(source)))?;
            if intersection.cases.is_empty() {
                discarded += 1;
            }
            cases.extend(intersection.cases);
        }
        Ok((cases, discarded))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SectorSolveOptions {
    /// Optional input negative-index degree `sum(max(-n_i,0))`. Positive
    /// powers remain symbolic; source seeds and RHS successors are NOT cut.
    pub max_numerator_rank: Option<u32>,
    /// Explicit nonminimal finite-leaf retention, separate from search depth.
    pub finite_case_policy: FiniteCasePolicy,
    /// Aggregate work/storage limits for finite retention, not coverage.
    pub finite_case_limits: FiniteCaseLimits,
    /// Shared work budget for each exceptional AND conjunction and its exact
    /// refinement branches, including conservative queued-case coverage tests.
    /// Not a sector-wide counter, coverage bound, or hard native memory limit.
    pub case_intersection_limits: CaseIntersectionLimits,
    /// Symbolic search is unbounded by default, as in the reference.
    pub symbolic: SearchOptions,
    /// Search radius for fully fixed cases, not a master-independence test.
    pub numerical_depth: u32,
    /// Optional diagnostic limit. Reaching it is an error, not completion.
    pub max_symbolic_cases: Option<usize>,
}

impl Default for SectorSolveOptions {
    fn default() -> Self {
        Self {
            max_numerator_rank: None,
            finite_case_policy: FiniteCasePolicy::default(),
            finite_case_limits: FiniteCaseLimits::default(),
            case_intersection_limits: CaseIntersectionLimits::default(),
            symbolic: SearchOptions::default(),
            numerical_depth: 2,
            max_symbolic_cases: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SectorStats {
    pub symbolic_cases: usize,
    pub numerical_cases: usize,
    pub discarded_cases: usize,
    pub symbolic_rows: usize,
    pub symbolic_search: Duration,
    pub exception_extraction: Duration,
    pub geometry: Duration,
    pub numerical_search: Duration,
    pub finite_points_visited: usize,
    pub retained_finite_terminals: usize,
    pub finite_enumeration: Duration,
    pub elapsed: Duration,
}

/// Source-port sector output, NOT an authenticated family-closing artifact.
///
/// Every exceptional symbolic branch within `max_numerator_rank` (unrestricted
/// when absent) has been traversed if this is returned successfully. This does
/// not prove coverage of recursive successors above the input rank.
/// `finite_residuals` are fixed cases left by bounded numerical search or
/// deliberately retained without search under `finite_case_policy`;
/// this neither proves their independence nor
/// declares them to be certified master terminals. Inherited source conditions
/// and parameter poles of RHS coefficients remain separate obligations.
#[derive(Debug)]
pub struct SectorSolution<const N: usize> {
    pub max_numerator_rank: Option<u32>,
    pub finite_case_policy: FiniteCasePolicy,
    pub rules: Vec<SectorRule<N>>,
    pub finite_residuals: Vec<Integral<N>>,
    pub stats: SectorStats,
}

/// Borrowed progress events; no output, synchronization, or exact-row copies
/// are introduced when the default no-op observer is used.
pub enum SectorEvent<'a, const N: usize> {
    CaseStarted {
        case: Case<N>,
        pending: usize,
    },
    Search {
        case: &'a Case<N>,
        event: SearchEvent<N>,
    },
    PhaseStarted {
        case: &'a Case<N>,
        phase: SectorPhase,
    },
    RuleFound {
        rule: &'a SectorRule<N>,
        pending: usize,
    },
    NumericalStarted {
        cases: &'a [CoordinateCase<N>],
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectorPhase {
    GuardExtraction,
    ExceptionalGeometry,
    FiniteRetention,
}

#[derive(Debug)]
pub enum SectorSolveError<const N: usize> {
    Search {
        case: Case<N>,
        source: SolverError,
    },
    Geometry {
        case: Case<N>,
        source: AffineGeometryError,
    },
    /// A complete exceptional union could not be admitted. Provenance retains
    /// the original parent/conjunction and unresolved branch; no partial cover
    /// or partially published parent rule is returned.
    Intersection(Box<CaseIntersectionError<N>>),
    Exceptions {
        case: Case<N>,
        source: ExceptionError,
    },
    NonProgress {
        case: Case<N>,
    },
    CaseBudget {
        solved: usize,
        pending: usize,
    },
    Numeric(SolverError),
    FiniteRetention {
        case: Case<N>,
        source: FiniteRetentionError,
    },
}

impl<const N: usize> fmt::Display for SectorSolveError<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Search { case, source } => write!(f, "search for {case:?}: {source}"),
            Self::Geometry { case, source } => write!(f, "geometry on {case:?}: {source}"),
            Self::Intersection(source) => source.fmt(f),
            Self::Exceptions { case, source } => write!(f, "exceptions for {case:?}: {source}"),
            Self::NonProgress { case } => {
                write!(f, "a rule excludes its entire current case {case:?}")
            }
            Self::CaseBudget { solved, pending } => {
                write!(
                    f,
                    "symbolic-case budget exhausted: {solved} solved, {pending} pending"
                )
            }
            Self::Numeric(source) => write!(f, "numerical case search: {source}"),
            Self::FiniteRetention { case, source } => {
                write!(f, "finite candidate retention on {case:?}: {source}")
            }
        }
    }
}

impl<const N: usize> std::error::Error for SectorSolveError<N> {}

impl<const N: usize> SectorSolver<'_, N> {
    pub fn solve_sector(
        &self,
        options: SectorSolveOptions,
    ) -> Result<SectorSolution<N>, SectorSolveError<N>> {
        self.solve_sector_with_observer(options, |_| {})
    }

    /// Port the reference's equality-case queue: solve the least constrained
    /// case, intersect each exact exceptional conjunction, remove subsumed
    /// work, then store the rule. Fully fixed cases share the later numerical
    /// solve. Admitted affine cases retain their exact charts. Unsupported
    /// geometry is never replaced by sampled coordinate faces. An explicit
    /// numerator rank permits exhaustive in-scope negative-coordinate splits.
    pub fn solve_sector_with_observer(
        &self,
        options: SectorSolveOptions,
        mut observe: impl FnMut(SectorEvent<'_, N>),
    ) -> Result<SectorSolution<N>, SectorSolveError<N>> {
        if options.finite_case_policy == FiniteCasePolicy::RetainRankFinite
            && options.max_numerator_rank.is_none()
        {
            return Err(SectorSolveError::FiniteRetention {
                case: Case::generic(),
                source: FiniteRetentionError::MissingNumeratorRank,
            });
        }
        let start = Instant::now();
        let initial = CoordinateCase::new(std::array::from_fn(|i| {
            self.config.removed_deltas[i].then_some(1)
        }))
        .expect("one is a representable compact fixed power");
        let mut pending = Vec::new();
        let mut numerical = Vec::new();
        if initial.is_numerical() {
            numerical.push(initial);
        } else {
            pending.push(Case::from(initial));
        }
        let mut rules = Vec::new();
        let mut stats = SectorStats::default();
        let mut retained = finite::Retention::default();
        while !pending.is_empty() {
            if options.finite_case_policy == FiniteCasePolicy::RetainRankFinite
                && self
                    .order
                    .sector()
                    .iter()
                    .zip(pending[0].fixed())
                    .all(|(&active, value)| !active || value.is_some())
            {
                let current = pending.remove(0);
                observe(SectorEvent::CaseStarted {
                    case: current.clone(),
                    pending: pending.len(),
                });
                observe(SectorEvent::PhaseStarted {
                    case: &current,
                    phase: SectorPhase::FiniteRetention,
                });
                let finite_start = Instant::now();
                let is_finite = retained
                    .retain_case(
                        &current,
                        &self.system.indices,
                        self.order.sector(),
                        options.max_numerator_rank.expect("checked retention rank"),
                        options.finite_case_limits,
                    )
                    .map_err(|source| SectorSolveError::FiniteRetention {
                        case: current.clone(),
                        source,
                    })?;
                stats.finite_enumeration += finite_start.elapsed();
                debug_assert!(is_finite, "checked all active coordinates are fixed");
                continue;
            }
            if options
                .max_symbolic_cases
                .is_some_and(|limit| stats.symbolic_cases >= limit)
            {
                return Err(SectorSolveError::CaseBudget {
                    solved: stats.symbolic_cases,
                    pending: pending.len(),
                });
            }
            let current = pending.remove(0);
            observe(SectorEvent::CaseStarted {
                case: current.clone(),
                pending: pending.len(),
            });
            let candidate = self
                .solve_case_with_observer(current.clone(), options.symbolic, |event| {
                    observe(SectorEvent::Search {
                        case: &current,
                        event,
                    });
                })
                .map_err(|source| SectorSolveError::Search {
                    case: current.clone(),
                    source,
                })?;
            stats.symbolic_cases += 1;
            stats.symbolic_rows += candidate.stats.rows;
            stats.symbolic_search += candidate.stats.elapsed;
            observe(SectorEvent::PhaseStarted {
                case: &current,
                phase: SectorPhase::GuardExtraction,
            });
            let rule = self.finish_rule(candidate)?;
            stats.exception_extraction += rule.1;
            let rule = rule.0;
            observe(SectorEvent::PhaseStarted {
                case: &current,
                phase: SectorPhase::ExceptionalGeometry,
            });
            let geometry_start = Instant::now();
            // Admit every exceptional sibling before mutating the queue or
            // publishing the parent. A newly discovered child is not covered
            // by this rule itself, so only earlier rules may suppress it.
            let (children, discarded) = rule.admit_exceptional_cases_in_scope(
                &self.system.indices,
                self.order.sector(),
                options.max_numerator_rank,
                options.case_intersection_limits,
            )?;
            stats.discarded_cases += discarded;
            if children.iter().any(|child| child == &current) {
                return Err(SectorSolveError::NonProgress { case: current });
            }
            for child in children {
                if !self.enqueue(
                    child,
                    &mut pending,
                    &mut numerical,
                    &rules,
                    options.case_intersection_limits,
                )? {
                    stats.discarded_cases += 1;
                }
            }
            stats.geometry += geometry_start.elapsed();
            observe(SectorEvent::RuleFound {
                rule: &rule,
                pending: pending.len(),
            });
            rules.push(rule);
        }
        if options.finite_case_policy == FiniteCasePolicy::RetainRankFinite {
            let finite_start = Instant::now();
            for point in numerical {
                let case = Case::from(point);
                observe(SectorEvent::CaseStarted {
                    case: case.clone(),
                    pending: 0,
                });
                observe(SectorEvent::PhaseStarted {
                    case: &case,
                    phase: SectorPhase::FiniteRetention,
                });
                let is_finite = retained
                    .retain_case(
                        &case,
                        &self.system.indices,
                        self.order.sector(),
                        options.max_numerator_rank.expect("checked retention rank"),
                        options.finite_case_limits,
                    )
                    .map_err(|source| SectorSolveError::FiniteRetention { case, source })?;
                debug_assert!(is_finite, "fully fixed cases are finite at bounded rank");
            }
            stats.finite_enumeration += finite_start.elapsed();
            stats.finite_points_visited = retained.visited();
            stats.retained_finite_terminals = retained.len();
            stats.elapsed = start.elapsed();
            return Ok(SectorSolution {
                max_numerator_rank: options.max_numerator_rank,
                finite_case_policy: options.finite_case_policy,
                rules,
                finite_residuals: retained.into_points(),
                stats,
            });
        }
        stats.numerical_cases = numerical.len();
        observe(SectorEvent::NumericalStarted { cases: &numerical });
        let numeric_start = Instant::now();
        let result = self
            .solve_numeric_cases(
                numerical,
                SearchOptions {
                    max_depth: Some(options.numerical_depth),
                    ..options.symbolic
                },
            )
            .map_err(SectorSolveError::Numeric)?;
        stats.numerical_search = numeric_start.elapsed();
        for candidate in result.rules {
            let (rule, duration) = self.finish_rule(candidate)?;
            stats.exception_extraction += duration;
            // A normalized fully fixed rule cannot have index exceptions.
            if !rule.exceptions.branches.is_empty() {
                return Err(SectorSolveError::NonProgress {
                    case: rule.candidate.case,
                });
            }
            observe(SectorEvent::RuleFound {
                rule: &rule,
                pending: 0,
            });
            rules.push(rule);
        }
        stats.elapsed = start.elapsed();
        Ok(SectorSolution {
            max_numerator_rank: options.max_numerator_rank,
            finite_case_policy: options.finite_case_policy,
            rules,
            finite_residuals: result
                .residuals
                .iter()
                .map(CoordinateCase::integral)
                .collect(),
            stats,
        })
    }

    fn finish_rule(
        &self,
        candidate: RuleCandidate<N>,
    ) -> Result<(SectorRule<N>, Duration), SectorSolveError<N>> {
        if candidate
            .rhs
            .iter()
            .any(|term| self.order.compare(&candidate.target, &term.integral) != Ordering::Less)
        {
            return Err(SectorSolveError::Search {
                case: candidate.case,
                source: SolverError::ExactReplay("winning rule is not strictly descending".into()),
            });
        }
        let start = Instant::now();
        let exceptions = extract_exceptions(&candidate, &self.system.indices, self.order.sector())
            .map_err(|source| SectorSolveError::Exceptions {
                case: candidate.case.clone(),
                source,
            })?;
        Ok((
            SectorRule {
                candidate,
                exceptions,
            },
            start.elapsed(),
        ))
    }

    fn enqueue(
        &self,
        case: Case<N>,
        pending: &mut Vec<Case<N>>,
        numerical: &mut Vec<CoordinateCase<N>>,
        rules: &[SectorRule<N>],
        limits: CaseIntersectionLimits,
    ) -> Result<bool, SectorSolveError<N>> {
        if !prune_subsumed(pending, &case).map_err(|source| SectorSolveError::Geometry {
            case: case.clone(),
            source,
        })? {
            return Ok(false);
        }
        for rule in rules {
            if self.rule_covers(rule, &case, limits)? {
                return Ok(false);
            }
        }
        if case.is_numerical() {
            // A full-rank affine system canonicalizes to a coordinate case.
            // Preserve the existing shared numerical search input contract.
            let fixed = *case.face();
            return match numerical.binary_search_by(|queued| compare_cases(queued, &fixed)) {
                Ok(_) => Ok(false),
                Err(position) => {
                    numerical.insert(position, fixed);
                    Ok(true)
                }
            };
        }
        match pending.binary_search_by(|queued| queued.queue_cmp(&case)) {
            Ok(_) => Ok(false),
            Err(position) => {
                pending.insert(position, case);
                Ok(true)
            }
        }
    }

    fn rule_covers(
        &self,
        rule: &SectorRule<N>,
        case: &Case<N>,
        limits: CaseIntersectionLimits,
    ) -> Result<bool, SectorSolveError<N>> {
        if !rule
            .candidate
            .case
            .contains(case)
            .map_err(|source| SectorSolveError::Geometry {
                case: case.clone(),
                source,
            })?
        {
            return Ok(false);
        }
        for branch in &rule.exceptions.branches {
            match case.intersect_many(branch, &self.system.indices, self.order.sector(), limits) {
                Ok(result) if result.cases.is_empty() => (),
                Ok(_) => return Ok(false),
                // Failure to prove an exceptional intersection empty must
                // never suppress pending work, even if the rule might apply.
                Err(source)
                    if matches!(
                        &source.failure,
                        CaseIntersectionFailure::UnsupportedGeometry
                            | CaseIntersectionFailure::Budget { .. }
                            | CaseIntersectionFailure::RepeatedState
                            | CaseIntersectionFailure::Admission(
                                AffineGeometryError::UnsupportedNonlinear { .. }
                                    | AffineGeometryError::Coordinate(
                                        GeometryError::UnsupportedGeometry { .. }
                                            | GeometryError::CompactOverflow { .. }
                                    )
                            )
                    ) =>
                {
                    return Ok(false);
                }
                Err(source) => {
                    return Err(SectorSolveError::Intersection(Box::new(source)));
                }
            }
        }
        Ok(true)
    }
}

/// Drop narrower queued domains only when exact implication proves it safe.
fn prune_subsumed<const N: usize>(
    cases: &mut Vec<Case<N>>,
    candidate: &Case<N>,
) -> Result<bool, AffineGeometryError> {
    for queued in cases.iter() {
        if queued.contains(candidate)? {
            return Ok(false);
        }
    }
    let mut position = 0;
    while position < cases.len() {
        if candidate.contains(&cases[position])? {
            cases.remove(position);
        } else {
            position += 1;
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "sector/intersection_tests.rs"]
mod intersection_tests;
