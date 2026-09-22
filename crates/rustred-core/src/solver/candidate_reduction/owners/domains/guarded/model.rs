use super::super::{
    OwnerAppliedFailure, OwnerAppliedLimits, OwnerAppliedNonzero, OwnerAppliedProblem,
    OwnerAppliedStats,
};
use crate::algebra::{IndexedAlgebraError, IndexedCoefficient, IndexedPolynomial};
use crate::foundry::completion::LatticeBox;
use crate::solver::Case;
use crate::solver::candidate_reduction::model::PreparedRule;
use std::fmt;

/// One selected saved candidate is inspected, not first-applicable dispatch.
/// Existing applied/native limits bound RHS work, with outer scratch reserved
/// from the SAME peak boxes/coordinate allowances before any allocation. The two extra cumulative
/// limits admit the borrowed predicate inventory before native restriction.
#[derive(Clone, Copy, Debug)]
pub struct OwnerGuardedLimits {
    pub applied: OwnerAppliedLimits,
    pub max_predicates: usize,
    pub max_predicate_terms: usize,
    pub max_events: usize,
}
impl Default for OwnerGuardedLimits {
    fn default() -> Self {
        Self {
            applied: Default::default(),
            max_predicates: 100_000,
            max_predicate_terms: 4_000_000,
            max_events: 1_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OwnerGuardedStats {
    pub predicates: usize,
    pub predicate_terms: usize,
    /// Charged before delivery, including a consumer-rejected last event.
    pub events: usize,
    pub residuals: usize,
    pub successors: usize,
    /// Shared exact RHS engine's actual work, not a coverage count.
    pub applied: OwnerAppliedStats,
}

/// Exact conjunction in ORIGINAL physical index coordinates:
/// box AND rank AND saved Case AND all source/denominator nonzero predicates
/// AND every whole excluded conjunction's negation. No feasibility assertion.
/// Borrowed data is bound to the immutable programs used by this call. The
/// absence of an epoch number is intentional: this descriptor cannot outlive
/// its borrowed programs, nor is it accepted by the box-only work queue.
#[derive(Debug)]
pub struct OwnerGuardedDomain<'a, const N: usize> {
    pub(super) owner: [bool; N],
    pub(super) batch: usize,
    pub(super) rule: &'a PreparedRule<N>,
    pub(super) cell: LatticeBox,
    pub(super) rank: Option<u32>,
    pub(super) source_conditions: &'a [IndexedPolynomial],
}
impl<const N: usize> OwnerGuardedDomain<'_, N> {
    pub fn owner(&self) -> &[bool; N] {
        &self.owner
    }
    pub fn batch(&self) -> usize {
        self.batch
    }
    pub fn rule_ordinal(&self) -> usize {
        self.rule.ordinal
    }
    pub fn case(&self) -> &Case<N> {
        &self.rule.case
    }
    pub fn lower(&self) -> &[u64] {
        self.cell.lower()
    }
    pub fn upper(&self) -> &[Option<u64>] {
        self.cell.upper()
    }
    pub fn max_numerator_rank(&self) -> Option<u32> {
        self.rank
    }
    /// These equations are zero. They retain the original native variable map.
    pub fn equalities(&self) -> &[IndexedPolynomial] {
        &self.rule.equalities
    }
    /// EACH returned slice means NotAllZero(slice), not per-atom nonvanishing.
    pub fn excluded_conjunctions(&self) -> &[Vec<IndexedPolynomial>] {
        &self.rule.exceptions
    }
    pub fn source_conditions(&self) -> &[IndexedPolynomial] {
        self.source_conditions
    }
    /// All original poles survive, including zero/cancelled RHS terms.
    pub fn original_denominators(&self) -> impl Iterator<Item = &IndexedPolynomial> {
        self.rule.rhs.iter().map(|term| &term.denominator)
    }
}

/// Exact lazy pullback for m=n+s: every source predicate P is P(m-s), source
/// bounds/signs use the same substitution, and the rank predicate is
/// -sum_{i inactive(source)}(m_i + argument_shift_i) <= maximum.
/// Native polynomials/Case are borrowed, not cloned for each RHS edge. Rational
/// charts never replace the original integer-coordinate equality semantics.
#[derive(Debug)]
pub struct OwnerGuardedImage<'a, const N: usize> {
    pub source: &'a OwnerGuardedDomain<'a, N>,
    /// The exact RHS boundary cell further intersects the source domain.
    pub source_lower: &'a [u64],
    pub source_upper: &'a [Option<u64>],
    /// Exact negation of the physical i64 shift; i128 also represents -i64::MIN.
    pub argument_shift: [i128; N],
}

/// The target box/rank alone is NOT this guarded domain. All fields and the
/// attached image must travel together. Nonzero dependencies additionally
/// require coefficient != 0 on the source; Conditional never means reached.
#[derive(Debug)]
pub struct OwnerGuardedSuccessor<'a, const N: usize> {
    pub image: OwnerGuardedImage<'a, N>,
    pub target_sector: &'a [bool; N],
    pub target_lower: &'a [u64],
    pub target_upper: &'a [Option<u64>],
    pub target_rank_limit: Option<u32>,
    pub coefficient: &'a IndexedCoefficient,
    pub coefficient_nonzero: OwnerAppliedNonzero,
    pub has_installed_target_owner: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Residuals are retained diagnostic obligations, NOT an asserted disjoint
/// partition or a nonempty missing-rule region. A candidate rejection can
/// overlap IncomingComplement (the candidate guarded domain may be empty).
pub enum OwnerGuardedResidualKind {
    /// Exact requested box/rank MINUS the attached guarded domain. It may be
    /// empty; no feasibility or missing-rule conclusion is implied.
    IncomingComplement,
    EmptyFixedFace,
    RankEmpty,
    /// Invalid on the candidate subdomain, NOT on all off-case input points.
    InvalidSourceCondition {
        ordinal: usize,
    },
    ExcludedConjunction {
        branch: usize,
    },
    OriginalDenominatorZero {
        term: usize,
    },
    UnresolvedFixedCoordinate {
        axis: usize,
    },
}

#[derive(Debug)]
pub enum OwnerGuardedEvent<'a, const N: usize> {
    Admitted(&'a OwnerGuardedDomain<'a, N>),
    Residual {
        kind: OwnerGuardedResidualKind,
        requested_lower: &'a [u64],
        requested_upper: &'a [Option<u64>],
        requested_rank: Option<u32>,
        /// Some only when the residual references this guarded domain.
        domain: Option<&'a OwnerGuardedDomain<'a, N>>,
    },
    Successor(OwnerGuardedSuccessor<'a, N>),
    /// The problem's box is scoped by domain; it is not a whole-box verdict.
    /// Its nested source/SelectedRule identifies the saved candidate only and
    /// does NOT carry ordinary first-priority matcher applicability authority.
    Problem {
        domain: &'a OwnerGuardedDomain<'a, N>,
        problem: OwnerAppliedProblem<'a, N>,
    },
    OptionalCoefficientRefusal {
        domain: &'a OwnerGuardedDomain<'a, N>,
        source_lower: &'a [u64],
        source_upper: &'a [Option<u64>],
        shift: &'a [i64; N],
        original_term_ordinal: Option<usize>,
        failure: &'a IndexedAlgebraError,
    },
    /// All earlier successors remain provisional until this event with no
    /// problems AND an untruncated successful return from the visitor. This
    /// never discharges the incoming complement or proves integer feasibility.
    RuleFinished {
        domain: &'a OwnerGuardedDomain<'a, N>,
        successors: usize,
        problems: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnerGuardedFailure {
    /// This optional lazy pullback API has not admitted correlated source
    /// domains yet. Never silently widen them to a box/rank query.
    UnsupportedPowerBounds(crate::solver::candidate_reduction::power_domain::DomainPowerBounds),
    UnknownOwner,
    UnknownCandidate,
    InvalidInput(&'static str),
    Cancelled,
    StoppedByConsumer,
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    CountOverflow {
        resource: &'static str,
    },
    Applied(OwnerAppliedFailure),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnerGuardedError {
    pub failure: OwnerGuardedFailure,
    pub stats: OwnerGuardedStats,
}
impl fmt::Display for OwnerGuardedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "incomplete guarded candidate application: {:?}",
            self.failure
        )
    }
}
impl std::error::Error for OwnerGuardedError {}
