use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use symbolica::prelude::Integer;

use crate::algebra::CoefficientPolynomial;

use super::super::{AffineGeometryError, Case};

/// Optional exceptional-geometry work diagnostics, disabled by default.
/// Ambient RAM/time scopes govern ordinary execution. Native calls are checked
/// before and after execution; cooperative checks do not preempt native algebra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaseIntersectionLimits {
    /// Enforce the optional count thresholds only when explicitly enabled.
    pub diagnostics_enabled: bool,
    pub max_work_items: usize,
    pub max_terms_per_conjunction: usize,
    pub max_normalizations: usize,
    pub max_factorizations: usize,
}

impl Default for CaseIntersectionLimits {
    fn default() -> Self {
        Self {
            diagnostics_enabled: false,
            max_work_items: 4096,
            max_terms_per_conjunction: 100_000,
            max_normalizations: 1024,
            max_factorizations: 4096,
        }
    }
}

impl CaseIntersectionLimits {
    /// Explicitly opt into work-count diagnostics.
    pub fn with_diagnostics(mut self) -> Self {
        self.diagnostics_enabled = true;
        self
    }

    pub(super) const fn diagnostic_limit(self, limit: usize) -> Option<usize> {
        if self.diagnostics_enabled {
            Some(limit)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaseIntersectionBudget {
    WorkItems,
    ConjunctionTerms,
    Normalizations,
    Factorizations,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CaseIntersectionStats {
    pub work_items: usize,
    pub restrictions: usize,
    pub affine_admissions: usize,
    pub normalizations: usize,
    pub factorizations: usize,
    pub factor_children: usize,
    /// Native integer factorizations (also included in `factorizations`).
    pub integer_factorizations: usize,
    /// Signed divisors tested exhaustively (also included in `work_items`).
    pub integer_divisors: usize,
    pub bilinear_children: usize,
    /// Native integer resultants (also included in `normalizations`).
    pub integer_resultants: usize,
    pub resultant_children: usize,
    /// Exact bounded-negative-coordinate refinements, not sampled solutions.
    pub rank_splits: usize,
    pub rank_children: usize,
    /// Stuck branches resolved because the ORIGINAL conjunction restricts to
    /// zero on their chart (S1 lift, [`CaseResolutionReason::Lifted`]).
    pub lifted_branches: usize,
    /// Stuck branches proved integer-empty by residues modulo a small prime
    /// on the chart lattice (S2, [`CaseResolutionReason::ModularEmpty`]).
    pub modular_empty_branches: usize,
    pub peak_conjunction_terms: usize,
    pub restriction_time: Duration,
    pub admission_time: Duration,
    pub normalization_time: Duration,
    pub factorization_time: Duration,
    pub union_time: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseIntersectionResult<const N: usize> {
    /// Exact OR after intersection with `max_numerator_rank`, when present.
    /// Cases may over-cover outside that scope. An empty vector proves only
    /// the declared scope empty, not the unrestricted exceptional locus.
    pub cases: Vec<Case<N>>,
    pub max_numerator_rank: Option<u32>,
    pub stats: CaseIntersectionStats,
    /// Provenance of every branch that no refinement step could refine but
    /// that was nevertheless resolved exactly (S1 lift / S2 modular emptiness).
    /// Empty when the ordinary refinement path resolved every branch.
    pub resolutions: Vec<CaseResolution<N>>,
    /// S4, only under [`UnsupportedBranchPolicy::Defer`]: branches that no
    /// exact step (refinement, S1, S2) resolved. They are NOT part of `cases`
    /// and never claimed covered: `cases` is then the exact OR of everything
    /// else, and the exceptional locus is contained in `cases` together with
    /// these regions. Always empty under the default
    /// [`UnsupportedBranchPolicy::Refuse`], which fails atomically instead.
    pub deferred: Vec<DeferredRegion<N>>,
}

/// What the engine does with an AND branch that refinement, S1 and S2 all
/// leave unresolved (`UnsupportedGeometry` or `RepeatedState`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UnsupportedBranchPolicy {
    /// Fail the complete intersection atomically (the established behaviour).
    #[default]
    Refuse,
    /// S4 (opt-in): record the branch as a [`DeferredRegion`], keep it out of
    /// the resolved union and finish every other branch. The caller owns the
    /// deferred points: no rule may be applied there.
    Defer,
}

/// One unresolved AND branch kept out of a successful intersection (S4).
///
/// The region is exactly the set of integer points `n` of `chart` (fixed
/// coordinates and coupled affine equalities, within the caller's sector
/// signs) at which every polynomial of `conjunction` vanishes. It is a subset
/// of `parent` intersected with the ORIGINAL conjunction (every engine step is
/// an exact refinement), possibly empty, never approximated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeferredRegion<const N: usize> {
    /// The refusal the engine would have returned without deferral:
    /// `UnsupportedGeometry` or `RepeatedState`.
    pub reason: CaseIntersectionFailure,
    /// The parent case of the whole intersection (`self` of the call).
    pub parent: Case<N>,
    /// The branch chart: an exact affine sub-case of `parent`.
    pub chart: Case<N>,
    /// The unresolved conjunction restricted to `chart` (or a same-ideal
    /// basis of it), equations equal to zero, in the original variables.
    pub conjunction: Arc<[CoefficientPolynomial]>,
    /// What S1 and S2 found on this branch (diagnostic record only).
    pub checks: StuckBranchChecks,
}

/// Why S1 and S2 left a deferred branch unresolved. Recomputed for the record
/// after the deferral decision with the same exact tests; never used to
/// decide anything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StuckBranchChecks {
    /// S1 precondition: the chart lies in the original parent (exact
    /// rational-affine containment). When true, the ORIGINAL conjunction does
    /// not restrict to zero on the chart (otherwise S1 would have lifted it).
    pub chart_in_original_parent: bool,
    /// S2 at each prime of 2, 3, 5, 7, in order, on the chart lattice.
    pub modular: Vec<(u32, ModularCheck)>,
    /// A non-resource failure inside these checks (the branch was deferred
    /// exactly as it would have been refused).
    pub failure: Option<CaseIntersectionFailure>,
}

/// Outcome of the S2 residue test at one prime (see `modular`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModularCheck {
    /// Every usable equation vanishes identically modulo the prime.
    NoConstraint,
    /// The equations have a common zero modulo the prime on the chart lattice.
    CommonZero,
    /// The joint test was over budget; each equation alone has a zero.
    ZeroPerEquation,
    /// Over budget: no conclusion at this prime.
    OverBudget,
    /// No residue is a common zero (not expected on a deferred branch: S2
    /// drops such a branch before deferral).
    ProvedEmpty,
}

impl<const N: usize> DeferredRegion<N> {
    /// The region as one AND-list of polynomial equations, the same form as
    /// an exception branch: `n_i - v` for every fixed coordinate of `chart`,
    /// the chart's coupled affine equalities, then `conjunction`. An integer
    /// point of the sector lies in the region iff all of them vanish there.
    /// `None` only if there is no polynomial to take the variable map from
    /// (never for a region built by the engine, whose conjunction is nonempty).
    pub fn predicates(&self, indices: &[usize; N]) -> Option<Vec<CoefficientPolynomial>> {
        let template = self
            .conjunction
            .first()
            .or_else(|| self.chart.affine().and_then(|a| a.equations().first()))?;
        let mut predicates = Vec::new();
        for (axis, value) in self.chart.fixed().iter().enumerate() {
            if let Some(value) = value {
                let mut exponents = vec![0_u16; template.nvars()];
                exponents[indices[axis]] = 1;
                let variable = template.monomial(Integer::from(1), exponents);
                predicates.push(variable - template.constant(Integer::from(i64::from(*value))));
            }
        }
        if let Some(affine) = self.chart.affine() {
            predicates.extend(affine.equations().iter().cloned());
        }
        predicates.extend(self.conjunction.iter().cloned());
        Some(predicates)
    }
}

/// Why an otherwise unrefinable AND branch was resolved exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CaseResolutionReason {
    /// S1: the ORIGINAL conjunction restricts to zero on the branch chart, so
    /// the whole chart (a subset of the original parent) lies in the locus and
    /// replaces the branch (which is a subset of the chart). The union stays
    /// exactly the exceptional locus.
    Lifted,
    /// S2: the branch conjunction has no common zero modulo `prime` at any
    /// residue of the chart lattice (fixed coordinates and primitive integer
    /// chart rows reduced modulo `prime`), so the branch has no integer point.
    ModularEmpty { prime: u32 },
}

/// Exact provenance of one branch resolved by [`CaseResolutionReason`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseResolution<const N: usize> {
    pub reason: CaseResolutionReason,
    /// The refusal the engine would have returned without this resolution.
    pub replaced: CaseIntersectionFailure,
    /// The branch chart; for [`CaseResolutionReason::Lifted`] also the case
    /// added to the union (before canonical pruning and rank scoping).
    pub chart: Case<N>,
    /// The branch conjunction restricted to `chart`, equations equal to zero.
    pub conjunction: Arc<[CoefficientPolynomial]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaseIntersectionFailure {
    Resources(symbolica::utils::resources::ResourceLimitError),
    InvalidInput(&'static str),
    Admission(AffineGeometryError),
    NativeAlgebra,
    /// Exact operations made no progress towards coordinate/affine admission.
    UnsupportedGeometry,
    /// A canonical work state repeated. It is not treated as an empty branch.
    RepeatedState,
    Budget {
        kind: CaseIntersectionBudget,
        limit: usize,
    },
}

/// Atomic failure of the complete OR result, including original provenance and
/// the unresolved AND branch. Resolved siblings are deliberately not returned
/// as a successful cover. Coefficient polynomials are equations equal to zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseIntersectionError<const N: usize> {
    pub max_numerator_rank: Option<u32>,
    /// Effective resources for this complete conjunction, not a domain bound.
    pub limits: CaseIntersectionLimits,
    pub original_parent: Case<N>,
    pub original_conjunction: Arc<[CoefficientPolynomial]>,
    pub unresolved_parent: Case<N>,
    pub unresolved_conjunction: Arc<[CoefficientPolynomial]>,
    pub failure: CaseIntersectionFailure,
    pub stats: CaseIntersectionStats,
}

impl<const N: usize> fmt::Display for CaseIntersectionError<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "incomplete exact case intersection: ")?;
        match &self.failure {
            CaseIntersectionFailure::Resources(error) => error.fmt(f),
            CaseIntersectionFailure::InvalidInput(message) => f.write_str(message),
            CaseIntersectionFailure::Admission(error) => error.fmt(f),
            CaseIntersectionFailure::NativeAlgebra => f.write_str("native algebra failed"),
            CaseIntersectionFailure::UnsupportedGeometry => {
                f.write_str("a branch retains unsupported nonlinear equalities")
            }
            CaseIntersectionFailure::RepeatedState => f.write_str("a work state repeated"),
            CaseIntersectionFailure::Budget { kind, limit } => {
                write!(f, "{kind:?} budget {limit} exhausted")
            }
        }?;
        super::diagnostic::write_context(self, f)
    }
}

impl<const N: usize> std::error::Error for CaseIntersectionError<N> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.failure {
            CaseIntersectionFailure::Resources(error) => Some(error),
            CaseIntersectionFailure::Admission(error) => Some(error),
            _ => None,
        }
    }
}
