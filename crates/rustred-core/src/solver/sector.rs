//! Ordered exceptional-case traversal for the executable SpIRed source port.

use std::cmp::Ordering;
use std::fmt;
use std::time::{Duration, Instant};

use super::geometry::{GeometryError, compare_cases, contains, intersect};
use super::{
    CoordinateCase, ExceptionError, ExceptionalConditions, Integral, RuleCandidate, SearchOptions,
    SectorSolver, SolverError, extract_exceptions,
};

/// A solved equation together with the exact exceptional index conditions
/// on which it must NOT be applied. Coefficient parameters remain generic.
#[derive(Debug)]
pub struct SectorRule<const N: usize> {
    pub candidate: RuleCandidate<N>,
    pub exceptions: ExceptionalConditions,
}

impl<const N: usize> SectorRule<N> {
    /// Exact coordinate intersections of the rule's case with its exceptional
    /// branches, modulo sector signs. Broader faces subsume narrower ones.
    /// Unsupported coupled/nonlinear geometry is an error, never an empty set.
    pub fn exceptional_cases(
        &self,
        indices: &[usize; N],
        sector: &[bool; N],
    ) -> Result<Vec<CoordinateCase<N>>, GeometryError> {
        let mut cases = Vec::new();
        for branch in &self.exceptions.branches {
            if let Some(case) = intersect(&self.candidate.case, branch, indices, sector)? {
                if cases.iter().any(|other| contains(other, &case)) {
                    continue;
                }
                cases.retain(|other| !contains(&case, other));
                cases.push(case);
            }
        }
        cases.sort_unstable_by(compare_cases);
        Ok(cases)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SectorSolveOptions {
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
    pub elapsed: Duration,
}

/// Source-port sector output, NOT an authenticated family-closing artifact.
///
/// Every exceptional symbolic branch has been traversed if this is returned
/// successfully. `finite_residuals` are the fixed cases for which the bounded
/// numerical search found no rule; this neither proves their independence nor
/// declares them to be certified master terminals. Inherited source conditions
/// and parameter poles of RHS coefficients remain separate obligations.
#[derive(Debug)]
pub struct SectorSolution<const N: usize> {
    pub rules: Vec<SectorRule<N>>,
    pub finite_residuals: Vec<Integral<N>>,
    pub stats: SectorStats,
}

/// Borrowed progress events; no output, synchronization, or exact-row copies
/// are introduced when the default no-op observer is used.
pub enum SectorEvent<'a, const N: usize> {
    CaseStarted {
        case: CoordinateCase<N>,
        pending: usize,
    },
    RuleFound {
        rule: &'a SectorRule<N>,
        pending: usize,
    },
    NumericalStarted {
        cases: &'a [CoordinateCase<N>],
    },
}

#[derive(Debug)]
pub enum SectorSolveError<const N: usize> {
    Search {
        case: CoordinateCase<N>,
        source: SolverError,
    },
    Geometry {
        case: CoordinateCase<N>,
        source: GeometryError,
    },
    Exceptions {
        case: CoordinateCase<N>,
        source: ExceptionError,
    },
    NonProgress {
        case: CoordinateCase<N>,
    },
    CaseBudget {
        solved: usize,
        pending: usize,
    },
    Numeric(SolverError),
}

impl<const N: usize> fmt::Display for SectorSolveError<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Search { case, source } => write!(f, "search for {case:?}: {source}"),
            Self::Geometry { case, source } => write!(f, "geometry on {case:?}: {source}"),
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
    /// solve. Coupled/nonlinear geometry currently returns a typed error;
    /// it is never replaced by sampled coordinate faces.
    pub fn solve_sector_with_observer(
        &self,
        options: SectorSolveOptions,
        mut observe: impl FnMut(SectorEvent<'_, N>),
    ) -> Result<SectorSolution<N>, SectorSolveError<N>> {
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
            pending.push(initial);
        }
        let mut rules = Vec::new();
        let mut stats = SectorStats::default();
        while !pending.is_empty() {
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
                case: current,
                pending: pending.len(),
            });
            let candidate = self
                .solve_case(current, options.symbolic)
                .map_err(|source| SectorSolveError::Search {
                    case: current,
                    source,
                })?;
            stats.symbolic_cases += 1;
            stats.symbolic_rows += candidate.stats.rows;
            stats.symbolic_search += candidate.stats.elapsed;
            let rule = self.finish_rule(candidate)?;
            stats.exception_extraction += rule.1;
            let rule = rule.0;
            let geometry_start = Instant::now();
            // Insert children before storing this rule, matching solveSector.
            // A newly discovered branch is not covered by that rule itself.
            for conjunction in &rule.exceptions.branches {
                let child = intersect(
                    &current,
                    conjunction,
                    &self.system.indices,
                    self.order.sector(),
                )
                .map_err(|source| SectorSolveError::Geometry {
                    case: current,
                    source,
                })?;
                if let Some(child) = child {
                    if child == current {
                        return Err(SectorSolveError::NonProgress { case: current });
                    }
                    if !self.enqueue(child, &mut pending, &mut numerical, &rules)? {
                        stats.discarded_cases += 1;
                    }
                } else {
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
                case: candidate.case,
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
        case: CoordinateCase<N>,
        pending: &mut Vec<CoordinateCase<N>>,
        numerical: &mut Vec<CoordinateCase<N>>,
        rules: &[SectorRule<N>],
    ) -> Result<bool, SectorSolveError<N>> {
        if pending.iter().any(|queued| contains(queued, &case)) {
            return Ok(false);
        }
        pending.retain(|queued| !contains(&case, queued));
        for rule in rules {
            if self.rule_covers(rule, &case)? {
                return Ok(false);
            }
        }
        let queue = if case.is_numerical() {
            numerical
        } else {
            pending
        };
        match queue.binary_search_by(|queued| compare_cases(queued, &case)) {
            Ok(_) => Ok(false),
            Err(position) => {
                queue.insert(position, case);
                Ok(true)
            }
        }
    }

    fn rule_covers(
        &self,
        rule: &SectorRule<N>,
        case: &CoordinateCase<N>,
    ) -> Result<bool, SectorSolveError<N>> {
        if !contains(&rule.candidate.case, case) {
            return Ok(false);
        }
        for branch in &rule.exceptions.branches {
            match intersect(case, branch, &self.system.indices, self.order.sector()) {
                Ok(None) => (),
                Ok(Some(_)) => return Ok(false),
                // Failure to prove an exceptional intersection empty must
                // never suppress pending work, even if the rule might apply.
                Err(GeometryError::UnsupportedGeometry { .. })
                | Err(GeometryError::CompactOverflow { .. }) => return Ok(false),
                Err(source) => {
                    return Err(SectorSolveError::Geometry {
                        case: *case,
                        source,
                    });
                }
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests;
