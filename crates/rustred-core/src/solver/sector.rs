//! Ordered exceptional-case traversal for the executable SpIRed source port.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

use symbolica::prelude::Integer;

use crate::algebra::CoefficientPolynomial;

use super::geometry::{GeometryError, compare_cases};
use super::{
    AffineGeometryError, Case, CaseIntersectionError, CaseIntersectionFailure,
    CaseIntersectionLimits, CaseRefinement, CoordinateCase, DeferredRegion, ExceptionError,
    ExceptionalConditions, GuardReport, GuardRoute, GuardRouteTaken, Integral, NativeGuardTry,
    RuleCandidate, SearchEvent, SearchOptions, SectorSolver, SolverError, UnsupportedBranchPolicy,
};

mod domains;
mod finite;
mod selection;
pub use domains::SectorDomainSolution;
pub use finite::{FiniteCaseLimits, FiniteCasePolicy, FiniteRetentionError};
pub use selection::{
    RulePortfolioTrigger, RuleQualityFeature, RuleQualityPriority, RuleQualityThreshold,
    RuleSelectionPolicy, RuleSelectionStats, RuleTrialBudget, RuleTrialDescriptor, RuleTrialLimits,
    RuleTrialOutcome, RuleTrialStats, RuleTrialSummary,
};

/// Saved dispatch semantics, independent of a rule's algebraic identity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RuleDispatchPolicy {
    /// Ordinary ordered dispatch, including exact case/guard partitioning.
    #[default]
    Partition,
    /// An optional alternative only after an ordinary rule has selected a
    /// complete piece in the same prepared batch. It must apply uniformly to
    /// that unchanged piece; it never partitions it or fills a baseline gap.
    AfterBaselinePartitionWholePiece,
}

/// A solved equation and its independent runtime dispatch policy.
#[derive(Debug)]
pub struct SectorRule<const N: usize> {
    pub candidate: RuleCandidate<N>,
    pub exceptions: ExceptionalConditions,
    pub dispatch_policy: RuleDispatchPolicy,
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
        let (admitted, _, _) = self.admit_exceptional_cases(indices, sector)?;
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
    ) -> Result<(Vec<Case<N>>, usize, ResolutionCounts), SectorSolveError<N>> {
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
    ) -> Result<(Vec<Case<N>>, usize, ResolutionCounts), SectorSolveError<N>> {
        self.admit_exceptional_cases_counted(indices, sector, max_numerator_rank, limits, || {})
    }

    fn admit_exceptional_cases_counted(
        &self,
        indices: &[usize; N],
        sector: &[bool; N],
        max_numerator_rank: Option<u32>,
        limits: CaseIntersectionLimits,
        before_branch: impl FnMut(),
    ) -> Result<(Vec<Case<N>>, usize, ResolutionCounts), SectorSolveError<N>> {
        let mut deferred = Vec::new();
        let admitted = self.admit_exceptional_cases_with_policy(
            indices,
            sector,
            max_numerator_rank,
            limits,
            UnsupportedBranchPolicy::Refuse,
            CaseRefinement::default(),
            &mut deferred,
            &mut Vec::new(),
            before_branch,
        );
        debug_assert!(deferred.is_empty(), "Refuse never defers");
        admitted
    }

    /// The complete OR of exceptional cases. Under `Defer` (S4) every
    /// unresolved branch is appended to `deferred` with the index of its
    /// exception branch instead of failing; such a branch is NOT among the
    /// returned cases and is never counted as discarded (proved empty).
    /// `case_branches` receives the exception branch of every returned case.
    #[allow(clippy::too_many_arguments)]
    fn admit_exceptional_cases_with_policy(
        &self,
        indices: &[usize; N],
        sector: &[bool; N],
        max_numerator_rank: Option<u32>,
        limits: CaseIntersectionLimits,
        unsupported: UnsupportedBranchPolicy,
        refinement: CaseRefinement,
        deferred: &mut Vec<(usize, DeferredRegion<N>)>,
        case_branches: &mut Vec<usize>,
        mut before_branch: impl FnMut(),
    ) -> Result<(Vec<Case<N>>, usize, ResolutionCounts), SectorSolveError<N>> {
        let mut cases = Vec::new();
        let mut discarded = 0;
        let mut resolutions = ResolutionCounts::default();
        for (position, branch) in self.exceptions.branches.iter().enumerate() {
            before_branch();
            let intersection = self
                .candidate
                .case
                .intersect_many_refined(
                    branch,
                    indices,
                    sector,
                    limits,
                    max_numerator_rank,
                    unsupported,
                    refinement,
                )
                .map_err(|source| SectorSolveError::Intersection(Box::new(source)))?;
            if intersection.cases.is_empty() && intersection.deferred.is_empty() {
                discarded += 1;
            }
            resolutions.lifted += intersection.stats.lifted_branches;
            resolutions.modular_empty += intersection.stats.modular_empty_branches;
            case_branches.extend(std::iter::repeat_n(position, intersection.cases.len()));
            cases.extend(intersection.cases);
            deferred.extend(
                intersection
                    .deferred
                    .into_iter()
                    .map(|region| (position, region)),
            );
        }
        Ok((cases, discarded, resolutions))
    }
}

/// One exceptional branch region that the opt-in S4 solve
/// ([`SectorSolver::solve_sector_deferring`]) left unresolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectorDeferredRegion<const N: usize> {
    /// Index in [`SectorSolution::rules`] of the rule whose exceptional
    /// geometry produced the region (its case is `region.parent`).
    pub rule: usize,
    /// Index of the AND branch in that rule's `exceptions.branches` (the
    /// original conjunction of the intersection).
    pub branch: usize,
    pub region: DeferredRegion<N>,
}

/// The rule and exception branch whose exceptional cases first enqueued a
/// case (S4 provenance): the case lies in that branch's locus on that rule's
/// case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SectorRuleParent {
    pub rule: usize,
    pub branch: usize,
}

/// Output of the opt-in S4 solve: a sector solution that is complete except
/// for `deferred`.
///
/// Contract (exact and conservative): an integer point of the sector that
/// lies in any deferred region (see [`DeferredRegion::predicates`]) is
/// UNCOVERED. No rule of `solution` may be applied there, whether or not a
/// rule's case would contain it; such points must be supplied separately
/// (e.g. an exact finite table) or refused. Every other point is covered by
/// the unchanged queue logic (children, coverage pruning, numerical leaves),
/// exactly as in an ordinary [`SectorSolution`]. With no deferred region this
/// is the ordinary solution of [`SectorSolver::solve_sector`].
#[derive(Debug)]
pub struct DeferredSectorSolution<const N: usize> {
    pub solution: SectorSolution<N>,
    pub deferred: Vec<SectorDeferredRegion<N>>,
    /// Per rule of `solution.rules` (same index): the rule and exception
    /// branch whose exceptional cases first enqueued this rule's case; `None`
    /// for an initial case. Following it from a region's rule up to `None`
    /// gives the region's complete parent chain from the chart root.
    pub rule_parents: Vec<Option<SectorRuleParent>>,
}

impl<const N: usize> DeferredSectorSolution<N> {
    /// The parent chain of `self.deferred[region]`, chart root first; the last
    /// link is the region's own rule and branch. Every link's rule case lies in
    /// the previous link's branch locus on the previous rule's case, and the
    /// region lies in all of them. The flag is false if the provenance is
    /// missing or inconsistent (then the chain stops early).
    pub fn parent_chain(&self, region: usize) -> (Vec<SectorRuleParent>, bool) {
        let Some(deferred) = self.deferred.get(region) else {
            return (Vec::new(), false);
        };
        let mut chain = vec![SectorRuleParent {
            rule: deferred.rule,
            branch: deferred.branch,
        }];
        let mut complete = self.rule_parents.len() == self.solution.rules.len();
        while let Some(parent) = self
            .rule_parents
            .get(chain[chain.len() - 1].rule)
            .copied()
            .flatten()
        {
            if parent.rule >= chain[chain.len() - 1].rule {
                complete = false;
                break;
            }
            chain.push(parent);
        }
        chain.reverse();
        (chain, complete)
    }
}

/// Which (rule, RHS term) pairs of `rules` may send a point of a deferred
/// region (`predicates`, see [`DeferredRegion::predicates`]) into the region:
/// the term's key at a point n of the rule's case satisfies every predicate.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeferredExitScan {
    /// Rule-term pairs examined.
    pub pairs: usize,
    /// Pairs proved never to land in the region.
    pub excluded: usize,
    /// The other pairs, `(rule, term)`: possible exits into the region.
    pub candidates: Vec<(usize, usize)>,
}

/// Conservative exit scan for the S4 hybrid table: a pair is excluded only by
/// proof, either a numeric key coordinate of the other sign than `sector`, or
/// a predicate pulled back by the key (`n_i -> n_i + offset` on symbolic axes,
/// `n_i -> value` on numeric ones) that is a nonzero constant on the rule's
/// case. A listed pair's landing set is {n in the rule's domain : every
/// predicate vanishes at key(n)}.
pub fn deferred_exit_scan<const N: usize>(
    predicates: &[CoefficientPolynomial],
    rules: &[SectorRule<N>],
    indices: &[usize; N],
    sector: &[bool; N],
) -> DeferredExitScan {
    let mut scan = DeferredExitScan::default();
    for (r, rule) in rules.iter().enumerate() {
        for (t, term) in rule.candidate.rhs.iter().enumerate() {
            scan.pairs += 1;
            let key = &term.integral;
            let other_sign = key
                .powers()
                .iter()
                .zip(sector)
                .any(|(power, &active)| !power.is_symbolic() && (power.value() >= 1) != active);
            let misses = other_sign
                || predicates.iter().any(|predicate| {
                    let pulled = pull_back(predicate, key, indices);
                    let restricted = match rule.candidate.case.affine() {
                        Some(affine) => affine.restrict_equation(&pulled).unwrap_or(pulled),
                        None => pulled,
                    };
                    restricted.is_constant() && !restricted.is_zero()
                });
            if misses {
                scan.excluded += 1;
            } else {
                scan.candidates.push((r, t));
            }
        }
    }
    scan
}

/// `polynomial(key(n))`: symbolic axes shifted by their offset, numeric set.
fn pull_back<const N: usize>(
    polynomial: &CoefficientPolynomial,
    key: &Integral<N>,
    indices: &[usize; N],
) -> CoefficientPolynomial {
    let mut pulled = polynomial.clone();
    for (axis, power) in key.powers().iter().enumerate() {
        let value = Integer::from(i64::from(power.value()));
        if !power.is_symbolic() {
            pulled = pulled.replace(indices[axis], &value);
        } else if power.value() != 0 {
            pulled = pulled.shift_var(indices[axis], &value);
        }
    }
    pulled
}

/// S4 bookkeeping of one deferring solve.
#[derive(Default)]
struct DeferralLog<const N: usize> {
    /// Opt-in exact refinement of the deferring admission (default: off).
    refinement: CaseRefinement,
    regions: Vec<SectorDeferredRegion<N>>,
    producers: HashMap<Case<N>, SectorRuleParent>,
    rule_parents: Vec<Option<SectorRuleParent>>,
}

/// Stuck exceptional branches resolved exactly while admitting one rule's
/// cases (case-intersection S1 lift and S2 modular emptiness).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ResolutionCounts {
    pub lifted: usize,
    pub modular_empty: usize,
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
    /// Present only for opt-in exact portfolios; includes refused/losing work.
    pub rule_selection: Option<RuleSelectionStats>,
    pub symbolic_cases: usize,
    pub numerical_cases: usize,
    pub discarded_cases: usize,
    /// Exceptional branches the case intersection resolved by lifting their
    /// chart into the locus (S1), summed over the rules found.
    pub case_lifts: usize,
    /// Exceptional branches the case intersection proved integer-empty modulo
    /// a small prime (S2), summed over the rules found.
    pub case_modular_empty: usize,
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
    /// Actual mathematical order used by preconditioning and all searches.
    /// A permutation alone is no longer sufficient replay metadata.
    pub order: super::IntegralOrder<N>,
    pub max_numerator_rank: Option<u32>,
    pub finite_case_policy: FiniteCasePolicy,
    pub rules: Vec<SectorRule<N>>,
    pub finite_residuals: Vec<Integral<N>>,
    pub stats: SectorStats,
}

/// Borrowed progress events; no output, synchronization, or exact-row copies
/// are introduced when the default no-op observer is used.
pub enum SectorEvent<'a, const N: usize> {
    /// A trial is admitted only after exact guards and complete geometry pass;
    /// admission is not publication or a family-closure claim.
    RuleTrialFinished {
        case: &'a Case<N>,
        summary: RuleTrialSummary,
    },
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

impl<const N: usize> std::error::Error for SectorSolveError<N> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Search { source, .. } | Self::Numeric(source) => Some(source),
            Self::Geometry { source, .. } => Some(source),
            Self::Intersection(source) => Some(source.as_ref()),
            Self::Exceptions { source, .. } => Some(source),
            Self::FiniteRetention { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// An earlier rule used only to prune queued cases (FirstValid order): its
/// case and exceptional conditions, as a table record stores them. A case it
/// covers (the rule's case contains it and no exceptional branch meets it) is
/// never solved, exactly as if the rule had been found earlier in the same
/// queue. Sound only when every cover rule precedes the solved rules in the
/// assembled table (FirstValid dispatch then applies the cover rule there).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverRule<const N: usize> {
    pub case: Case<N>,
    pub exceptions: ExceptionalConditions,
}

impl<const N: usize> From<&SectorRule<N>> for CoverRule<N> {
    fn from(rule: &SectorRule<N>) -> Self {
        Self {
            case: rule.candidate.case.clone(),
            exceptions: rule.exceptions.clone(),
        }
    }
}

/// Inputs of [`SectorSolver::solve_case_queue_deferring`]: the engine's own
/// (FirstValid, deferring) case queue, re-entered at case level.
#[derive(Clone, Debug, Default)]
pub struct CaseQueue<const N: usize> {
    /// Initial cases of the queue; `None` = the chart's initial case (the
    /// whole sector, as [`SectorSolver::solve_sector_deferring`]). The cases
    /// are admitted through the ordinary queue (subsumption, own and cover
    /// rules); a case outside the sector is refused.
    pub initial: Option<Vec<Case<N>>>,
    /// Earlier rules (FirstValid order) that prune queued cases.
    pub cover: Vec<CoverRule<N>>,
    /// How each symbolic rule's exceptions are extracted.
    pub guard_route: GuardRoute,
    /// Exact refinement of the deferring admission (as
    /// [`SectorSolver::solve_sector_deferring_refined`]).
    pub refinement: CaseRefinement,
    /// How the fully fixed (numerical) endpoint cases are solved.
    pub numeric_endpoints: NumericEndpoints,
}

/// How a queue solves its fully fixed (numerical) endpoint cases.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NumericEndpoints {
    /// The shared numerical search of all endpoints at depth
    /// `SectorSolveOptions::numerical_depth`, with
    /// `SectorConfig::numerical_exact_backend` (the established engine).
    #[default]
    Shared,
    /// Each endpoint through the ordinary case search at the same depth,
    /// so it honours the solver's source visit order and
    /// `SectorConfig::symbolic_exact_backend` (e.g. target-only factorized);
    /// an endpoint whose search is exhausted at that depth is a residual. A
    /// shared search may find a rule for an endpoint through another
    /// endpoint's seeds, so this route can leave MORE residuals, never a
    /// rule the shared route would refuse.
    PerCase,
}

/// Progress of [`SectorSolver::solve_case_queue_deferring`], next to the
/// ordinary [`SectorEvent`]s.
pub enum CaseQueueEvent<'a, const N: usize> {
    /// Before each symbolic case: `pending[0]` is the case about to be solved,
    /// the rest the queue behind it; `numerical` the fully fixed cases kept
    /// for the final numerical solve; `rules` the rules found so far. Enough
    /// to re-enter the queue later (initial cases = `pending`, cover = the
    /// rules found so far plus the original cover).
    QueueState {
        pending: &'a [Case<N>],
        numerical: &'a [CoordinateCase<N>],
        rules: usize,
    },
    /// The guard extraction of the rule about to be found on `case`.
    Guard {
        case: &'a Case<N>,
        report: GuardReport,
    },
}

/// Counters of [`SectorSolver::solve_case_queue_deferring`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CaseQueueStats {
    /// Initial cases admitted to the queue.
    pub initial_queued: usize,
    /// Initial cases dropped (subsumed by another initial case or covered).
    pub initial_dropped: usize,
    /// Queued cases (initial or children) pruned by a cover rule.
    pub cover_pruned: usize,
    /// Rules whose exceptions came from the content route.
    pub content_routes: usize,
    /// Timed native tries that timed out or failed.
    pub native_timeouts: usize,
}

/// Output of [`SectorSolver::solve_case_queue_deferring`].
#[derive(Debug)]
pub struct CaseQueueSolution<const N: usize> {
    pub solved: DeferredSectorSolution<N>,
    pub queue: CaseQueueStats,
}

/// Queue options beyond [`SectorSolveOptions`]; the default is the
/// established queue (no cover, native guards, no queue observer).
struct QueueExtras<'a, const N: usize> {
    cover: &'a [CoverRule<N>],
    guard_route: GuardRoute,
    numeric_endpoints: NumericEndpoints,
    observe: Option<&'a mut dyn FnMut(CaseQueueEvent<'_, N>)>,
    stats: CaseQueueStats,
}

impl<const N: usize> Default for QueueExtras<'_, N> {
    fn default() -> Self {
        Self {
            cover: &[],
            guard_route: GuardRoute::Native,
            numeric_endpoints: NumericEndpoints::Shared,
            observe: None,
            stats: CaseQueueStats::default(),
        }
    }
}

/// Where [`SectorSolver::enqueue_with_cover`] put a case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Enqueued {
    Queued,
    Dropped,
    Covered,
}

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
        observe: impl FnMut(SectorEvent<'_, N>),
    ) -> Result<SectorSolution<N>, SectorSolveError<N>> {
        self.solve_case_queue(vec![self.initial_case()], options, observe)
    }

    fn initial_case(&self) -> Case<N> {
        let initial = CoordinateCase::new(std::array::from_fn(|i| {
            self.system.fixed()[i].or_else(|| self.config.removed_deltas[i].then_some(1))
        }))
        .expect("one is a representable compact fixed power");
        Case::from(initial)
    }

    /// Opt-in S4: [`Self::solve_sector`], except that an exceptional branch
    /// the case intersection cannot resolve (refinement, S1, S2) is deferred
    /// instead of failing the sector. See [`DeferredSectorSolution`] for the
    /// coverage contract. Rule selection must be `FirstValid`; coverage
    /// pruning of queued cases never relies on a deferred intersection. The
    /// queue always runs inline (no capacity dispatch).
    pub fn solve_sector_deferring(
        &self,
        options: SectorSolveOptions,
    ) -> Result<DeferredSectorSolution<N>, SectorSolveError<N>> {
        self.solve_sector_deferring_with_observer(options, |_| {})
    }

    pub fn solve_sector_deferring_with_observer(
        &self,
        options: SectorSolveOptions,
        observe: impl FnMut(SectorEvent<'_, N>),
    ) -> Result<DeferredSectorSolution<N>, SectorSolveError<N>> {
        self.solve_sector_deferring_refined_with_observer(
            options,
            CaseRefinement::default(),
            observe,
        )
    }

    /// [`Self::solve_sector_deferring`] with the opt-in exact
    /// [`CaseRefinement`] steps (factor-first, saturation, G-split) in the
    /// admission of every found rule's exceptional cases. Queue coverage
    /// pruning keeps the plain intersection. `CaseRefinement::default()` is
    /// exactly [`Self::solve_sector_deferring`].
    pub fn solve_sector_deferring_refined(
        &self,
        options: SectorSolveOptions,
        refinement: CaseRefinement,
    ) -> Result<DeferredSectorSolution<N>, SectorSolveError<N>> {
        self.solve_sector_deferring_refined_with_observer(options, refinement, |_| {})
    }

    pub fn solve_sector_deferring_refined_with_observer(
        &self,
        options: SectorSolveOptions,
        refinement: CaseRefinement,
        mut observe: impl FnMut(SectorEvent<'_, N>),
    ) -> Result<DeferredSectorSolution<N>, SectorSolveError<N>> {
        let initial = self.initial_case();
        if !matches!(self.config.rule_selection, RuleSelectionPolicy::FirstValid) {
            return Err(SectorSolveError::Search {
                case: initial,
                source: SolverError::InvalidInput(
                    "deferred exceptional branches require FirstValid rule selection".into(),
                ),
            });
        }
        let mut log = DeferralLog {
            refinement,
            ..DeferralLog::default()
        };
        let solution = self.solve_case_queue_with_deferral(
            vec![initial],
            options,
            &mut observe,
            Some(&mut log),
            &mut QueueExtras::default(),
        )?;
        debug_assert_eq!(log.rule_parents.len(), solution.rules.len());
        Ok(DeferredSectorSolution {
            solution,
            deferred: log.regions,
            rule_parents: log.rule_parents,
        })
    }

    /// The engine's deferring FirstValid queue (the two-sided path included:
    /// the order program and the sources carry the eps axes), re-entered at
    /// case level: it starts from `queue.initial` (default: the chart's
    /// initial case), prunes queued cases by `queue.cover` (earlier rules of
    /// the same chart in FirstValid order) as by its own rules, and extracts
    /// each rule's exceptions by `queue.guard_route`. With
    /// `CaseQueue::default()` the result is exactly
    /// [`Self::solve_sector_deferring_refined_with_observer`] with
    /// `CaseRefinement::default()`.
    ///
    /// Splitting a queue: solve a prefix, then solve its pending cases (the
    /// last [`CaseQueueEvent::QueueState`]) in pieces, each with the cover
    /// of every rule assembled before it. The assembled table is the prefix
    /// rules, then each piece's rules in order; rules of a piece may only be
    /// dispatched after the cover rules it was given.
    pub fn solve_case_queue_deferring(
        &self,
        options: SectorSolveOptions,
        queue: &CaseQueue<N>,
        mut observe: impl FnMut(SectorEvent<'_, N>),
        mut observe_queue: impl FnMut(CaseQueueEvent<'_, N>),
    ) -> Result<CaseQueueSolution<N>, SectorSolveError<N>> {
        let initial = match &queue.initial {
            Some(cases) => cases.clone(),
            None => vec![self.initial_case()],
        };
        let first = initial.first().cloned().unwrap_or_else(Case::generic);
        if !matches!(self.config.rule_selection, RuleSelectionPolicy::FirstValid) {
            return Err(SectorSolveError::Search {
                case: first,
                source: SolverError::InvalidInput(
                    "deferred exceptional branches require FirstValid rule selection".into(),
                ),
            });
        }
        for case in &initial {
            if !case.is_in_sector(self.order.sector()) {
                return Err(SectorSolveError::Search {
                    case: case.clone(),
                    source: SolverError::InvalidInput(
                        "an initial case lies outside the solver's sector".into(),
                    ),
                });
            }
        }
        let mut log = DeferralLog {
            refinement: queue.refinement,
            ..DeferralLog::default()
        };
        let mut extras = QueueExtras {
            cover: &queue.cover,
            guard_route: queue.guard_route,
            numeric_endpoints: queue.numeric_endpoints,
            observe: Some(&mut observe_queue),
            stats: CaseQueueStats::default(),
        };
        let solution = self.solve_case_queue_with_deferral(
            initial,
            options,
            &mut observe,
            Some(&mut log),
            &mut extras,
        )?;
        debug_assert_eq!(log.rule_parents.len(), solution.rules.len());
        Ok(CaseQueueSolution {
            solved: DeferredSectorSolution {
                solution,
                deferred: log.regions,
                rule_parents: log.rule_parents,
            },
            queue: extras.stats,
        })
    }

    /// Common queue machinery; callers own the distinction between a full
    /// sector and a fragment nominated by a shared campaign's missing frontier.
    fn solve_case_queue(
        &self,
        initial: Vec<Case<N>>,
        options: SectorSolveOptions,
        mut observe: impl FnMut(SectorEvent<'_, N>),
    ) -> Result<SectorSolution<N>, SectorSolveError<N>> {
        #[cfg(feature = "capacity-dispatch")]
        {
            super::capacity::solve_queue(self, initial, options, &mut observe)
        }
        #[cfg(not(feature = "capacity-dispatch"))]
        {
            self.solve_case_queue_inline(initial, options, &mut observe)
        }
    }

    // The observer type is erased here so each host/campaign callback shares
    // the same search body, independently of its physical arity.
    pub(super) fn solve_case_queue_inline(
        &self,
        initial: Vec<Case<N>>,
        options: SectorSolveOptions,
        observe: &mut dyn FnMut(SectorEvent<'_, N>),
    ) -> Result<SectorSolution<N>, SectorSolveError<N>> {
        self.solve_case_queue_with_deferral(
            initial,
            options,
            observe,
            None,
            &mut QueueExtras::default(),
        )
    }

    /// The queue body. `deferral` is `None` for every ordinary solve (atomic
    /// intersection failures, unchanged); `Some` only for the opt-in S4 solve,
    /// and then only the FirstValid admission of a found rule's exceptional
    /// cases defers. Coverage pruning ([`Self::rule_covers`]) never defers.
    fn solve_case_queue_with_deferral(
        &self,
        initial: Vec<Case<N>>,
        options: SectorSolveOptions,
        mut observe: &mut dyn FnMut(SectorEvent<'_, N>),
        mut deferral: Option<&mut DeferralLog<N>>,
        extras: &mut QueueExtras<'_, N>,
    ) -> Result<SectorSolution<N>, SectorSolveError<N>> {
        // Two-sided eps axes are an opt-in of the sources and must be carried
        // by the order program exactly (plan M2 sec.3.4): generation order and
        // the persisted runtime order are then one and the same. Refused up
        // front, so a solve that emits no rule cannot slip through. The error
        // names a queued case: with capacity dispatch `Case::generic()` has
        // open padding coordinates and cannot be mapped back to the caller.
        if self.order.two_sided_axes() != self.system.two_sided_axes() {
            return Err(SectorSolveError::Search {
                case: initial.first().cloned().unwrap_or_else(Case::generic),
                source: SolverError::InvalidInput(
                    "two-sided eps axes of the sources and of the integral-order program differ"
                        .into(),
                ),
            });
        }
        if options.finite_case_policy == FiniteCasePolicy::RetainRankFinite
            && options.max_numerator_rank.is_none()
        {
            return Err(SectorSolveError::FiniteRetention {
                case: Case::generic(),
                source: FiniteRetentionError::MissingNumeratorRank,
            });
        }
        let start = Instant::now();
        let mut pending = Vec::new();
        let mut numerical = Vec::new();
        for case in initial {
            match self.enqueue_with_cover(
                case,
                &mut pending,
                &mut numerical,
                &[],
                extras.cover,
                options.case_intersection_limits,
            )? {
                Enqueued::Queued => extras.stats.initial_queued += 1,
                Enqueued::Dropped => extras.stats.initial_dropped += 1,
                Enqueued::Covered => {
                    extras.stats.initial_dropped += 1;
                    extras.stats.cover_pruned += 1;
                }
            }
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
            if let Some(observe_queue) = extras.observe.as_deref_mut() {
                observe_queue(CaseQueueEvent::QueueState {
                    pending: &pending,
                    numerical: &numerical,
                    rules: rules.len(),
                });
            }
            let current = pending.remove(0);
            observe(SectorEvent::CaseStarted {
                case: current.clone(),
                pending: pending.len(),
            });
            if !matches!(self.config.rule_selection, RuleSelectionPolicy::FirstValid) {
                if deferral.is_some() {
                    return Err(SectorSolveError::Search {
                        case: current,
                        source: SolverError::InvalidInput(
                            "deferred exceptional branches require FirstValid rule selection"
                                .into(),
                        ),
                    });
                }
                let selected = self.select_rule(&current, options, &mut stats, &mut observe)?;
                stats.symbolic_cases += 1;
                stats.discarded_cases += selected.discarded;
                stats.case_lifts += selected.resolutions.lifted;
                stats.case_modular_empty += selected.resolutions.modular_empty;
                let geometry_start = Instant::now();
                for child in selected.children {
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
                    rule: &selected.rule,
                    pending: pending.len(),
                });
                rules.push(selected.rule);
                continue;
            }
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
            let (rule, duration, report) =
                self.finish_rule_routed(candidate, extras.guard_route)?;
            stats.exception_extraction += duration;
            if report.route == GuardRouteTaken::Content {
                extras.stats.content_routes += 1;
            }
            if matches!(
                report.native_try,
                NativeGuardTry::TimedOut | NativeGuardTry::ChildFailed
            ) {
                extras.stats.native_timeouts += 1;
            }
            if let Some(observe_queue) = extras.observe.as_deref_mut() {
                observe_queue(CaseQueueEvent::Guard {
                    case: &current,
                    report,
                });
            }
            observe(SectorEvent::PhaseStarted {
                case: &current,
                phase: SectorPhase::ExceptionalGeometry,
            });
            let geometry_start = Instant::now();
            // Admit every exceptional sibling before mutating the queue or
            // publishing the parent. A newly discovered child is not covered
            // by this rule itself, so only earlier rules may suppress it.
            let (children, discarded, resolutions) =
                match deferral.as_deref_mut() {
                    None => rule.admit_exceptional_cases_in_scope(
                        &self.system.indices,
                        self.order.sector(),
                        options.max_numerator_rank,
                        options.case_intersection_limits,
                    )?,
                    Some(log) => {
                        // S4: the rule is published at index `rules.len()` below
                        // (nothing else is pushed before it); its deferred
                        // regions stay out of `children`.
                        let mut regions = Vec::new();
                        let mut branches = Vec::new();
                        let admitted = rule.admit_exceptional_cases_with_policy(
                            &self.system.indices,
                            self.order.sector(),
                            options.max_numerator_rank,
                            options.case_intersection_limits,
                            UnsupportedBranchPolicy::Defer,
                            log.refinement,
                            &mut regions,
                            &mut branches,
                            || {},
                        )?;
                        let index = rules.len();
                        log.regions
                            .extend(regions.into_iter().map(|(branch, region)| {
                                SectorDeferredRegion {
                                    rule: index,
                                    branch,
                                    region,
                                }
                            }));
                        for (child, &branch) in admitted.0.iter().zip(&branches) {
                            log.producers
                                .entry(child.clone())
                                .or_insert(SectorRuleParent {
                                    rule: index,
                                    branch,
                                });
                        }
                        log.rule_parents.push(log.producers.get(&current).copied());
                        admitted
                    }
                };
            stats.discarded_cases += discarded;
            stats.case_lifts += resolutions.lifted;
            stats.case_modular_empty += resolutions.modular_empty;
            if children.iter().any(|child| child == &current) {
                return Err(SectorSolveError::NonProgress { case: current });
            }
            for child in children {
                match self.enqueue_with_cover(
                    child,
                    &mut pending,
                    &mut numerical,
                    &rules,
                    extras.cover,
                    options.case_intersection_limits,
                )? {
                    Enqueued::Queued => {}
                    Enqueued::Dropped => stats.discarded_cases += 1,
                    Enqueued::Covered => {
                        stats.discarded_cases += 1;
                        extras.stats.cover_pruned += 1;
                    }
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
                order: self.order.clone(),
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
        let search = SearchOptions {
            max_depth: Some(options.numerical_depth),
            ..options.symbolic
        };
        let result = match extras.numeric_endpoints {
            NumericEndpoints::Shared => self
                .solve_numeric_cases(numerical, search)
                .map_err(SectorSolveError::Numeric)?,
            NumericEndpoints::PerCase => {
                self.solve_numeric_cases_per_case(numerical, search, &mut *observe)?
            }
        };
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
            if let Some(log) = deferral.as_deref_mut() {
                log.rule_parents
                    .push(log.producers.get(&rule.candidate.case).copied());
            }
            rules.push(rule);
        }
        stats.elapsed = start.elapsed();
        Ok(SectorSolution {
            order: self.order.clone(),
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

    /// [`NumericEndpoints::PerCase`]: every endpoint through the ordinary
    /// case search (source visit order and symbolic exact backend of the
    /// solver), in the shared search's case order; exhausted = residual.
    fn solve_numeric_cases_per_case(
        &self,
        mut cases: Vec<CoordinateCase<N>>,
        search: SearchOptions,
        observe: &mut dyn FnMut(SectorEvent<'_, N>),
    ) -> Result<super::NumericResult<N>, SectorSolveError<N>> {
        let start = Instant::now();
        cases.sort_unstable_by(|left, right| right.fixed().cmp(left.fixed()));
        cases.dedup();
        let mut rules = Vec::new();
        let mut residuals = Vec::new();
        let count = cases.len();
        for point in cases {
            let case = Case::from(point);
            match self.solve_case_with_observer(case.clone(), search, |event| {
                observe(SectorEvent::Search { case: &case, event })
            }) {
                Ok(candidate) => rules.push(candidate),
                Err(SolverError::SearchExhausted { .. }) => residuals.push(point),
                Err(source) => return Err(SectorSolveError::Search { case, source }),
            }
        }
        Ok(super::NumericResult {
            rules,
            residuals,
            stats: super::NumericStats {
                cases: count,
                elapsed: start.elapsed(),
                ..Default::default()
            },
        })
    }

    fn finish_rule(
        &self,
        candidate: RuleCandidate<N>,
    ) -> Result<(SectorRule<N>, Duration), SectorSolveError<N>> {
        self.finish_rule_routed(candidate, GuardRoute::Native)
            .map(|(rule, duration, _)| (rule, duration))
    }

    fn finish_rule_routed(
        &self,
        candidate: RuleCandidate<N>,
        route: GuardRoute,
    ) -> Result<(SectorRule<N>, Duration, GuardReport), SectorSolveError<N>> {
        self.validate_rule_descent(&candidate)?;
        let start = Instant::now();
        let (exceptions, report) = super::exception::extract_exceptions_routed(
            &candidate,
            &self.system.indices,
            self.order.sector(),
            &self.order.two_sided_axes(),
            route,
        )
        .map_err(|source| SectorSolveError::Exceptions {
            case: candidate.case.clone(),
            source,
        })?;
        Ok((
            SectorRule {
                dispatch_policy: Default::default(),
                candidate,
                exceptions,
            },
            start.elapsed(),
            report,
        ))
    }

    // Shared by ordinary admission and the opt-in pre-screen. A cheap score
    // must never turn a malformed/non-descending identity into a refusal.
    fn validate_rule_descent(
        &self,
        candidate: &RuleCandidate<N>,
    ) -> Result<(), SectorSolveError<N>> {
        if candidate
            .rhs
            .iter()
            .any(|term| self.order.compare(&candidate.target, &term.integral) != Ordering::Less)
        {
            return Err(SectorSolveError::Search {
                case: candidate.case.clone(),
                source: SolverError::ExactReplay("winning rule is not strictly descending".into()),
            });
        }
        Ok(())
    }

    fn enqueue(
        &self,
        case: Case<N>,
        pending: &mut Vec<Case<N>>,
        numerical: &mut Vec<CoordinateCase<N>>,
        rules: &[SectorRule<N>],
        limits: CaseIntersectionLimits,
    ) -> Result<bool, SectorSolveError<N>> {
        self.enqueue_with_cover(case, pending, numerical, rules, &[], limits)
            .map(|outcome| outcome == Enqueued::Queued)
    }

    /// [`Self::enqueue`] with cover rules checked after the own rules.
    fn enqueue_with_cover(
        &self,
        case: Case<N>,
        pending: &mut Vec<Case<N>>,
        numerical: &mut Vec<CoordinateCase<N>>,
        rules: &[SectorRule<N>],
        cover: &[CoverRule<N>],
        limits: CaseIntersectionLimits,
    ) -> Result<Enqueued, SectorSolveError<N>> {
        if !prune_subsumed(pending, &case).map_err(|source| SectorSolveError::Geometry {
            case: case.clone(),
            source,
        })? {
            return Ok(Enqueued::Dropped);
        }
        for rule in rules {
            if self.rule_covers(rule, &case, limits)? {
                return Ok(Enqueued::Dropped);
            }
        }
        for rule in cover {
            if self.case_covers(&rule.case, &rule.exceptions, &case, limits)? {
                return Ok(Enqueued::Covered);
            }
        }
        if case.is_numerical() {
            // A full-rank affine system canonicalizes to a coordinate case.
            // Preserve the existing shared numerical search input contract.
            let fixed = *case.face();
            return match numerical.binary_search_by(|queued| compare_cases(queued, &fixed)) {
                Ok(_) => Ok(Enqueued::Dropped),
                Err(position) => {
                    numerical.insert(position, fixed);
                    Ok(Enqueued::Queued)
                }
            };
        }
        match pending.binary_search_by(|queued| queued.queue_cmp(&case)) {
            Ok(_) => Ok(Enqueued::Dropped),
            Err(position) => {
                pending.insert(position, case);
                Ok(Enqueued::Queued)
            }
        }
    }

    fn rule_covers(
        &self,
        rule: &SectorRule<N>,
        case: &Case<N>,
        limits: CaseIntersectionLimits,
    ) -> Result<bool, SectorSolveError<N>> {
        self.case_covers(&rule.candidate.case, &rule.exceptions, case, limits)
    }

    fn case_covers(
        &self,
        rule_case: &Case<N>,
        exceptions: &ExceptionalConditions,
        case: &Case<N>,
        limits: CaseIntersectionLimits,
    ) -> Result<bool, SectorSolveError<N>> {
        if !rule_case
            .contains(case)
            .map_err(|source| SectorSolveError::Geometry {
                case: case.clone(),
                source,
            })?
        {
            return Ok(false);
        }
        for branch in &exceptions.branches {
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

#[cfg(test)]
#[path = "sector/defer_tests.rs"]
mod defer_tests;

#[cfg(test)]
#[path = "sector/defer_review_tests.rs"]
mod defer_review_tests;

#[cfg(test)]
#[path = "sector/queue_tests.rs"]
mod queue_tests;
