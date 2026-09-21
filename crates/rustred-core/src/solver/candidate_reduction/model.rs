use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::algebra::{
    Coefficient, ExactAlgebraError, IndexedAlgebraError, IndexedCoefficient, IndexedPolynomial,
};
use crate::family::IntegralKey;
use crate::reduction::{ReductionError, ReductionStatistics};

/// In-memory coefficient storage for the same candidate application engine.
/// Artifacts and returned decompositions always use ordinary coefficients.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CandidateCacheRepresentation {
    #[default]
    Sparse,
    /// Native Symbolica factorized denominators. Returned results are
    /// transiently materialized, including on a top-level cache hit.
    Factorized,
}

/// Exact arithmetic result of candidate formulas, not certified IBP provenance
/// or a proof that the listed finite terminals form a complete master basis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateDecomposition {
    pub(super) family_fingerprint: Arc<String>,
    pub(super) target: IntegralKey,
    pub(super) terms: BTreeMap<IntegralKey, Coefficient>,
}

impl CandidateDecomposition {
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    pub fn target(&self) -> &IntegralKey {
        &self.target
    }
    pub fn terms(&self) -> &BTreeMap<IntegralKey, Coefficient> {
        &self.terms
    }
    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }

    /// Common-mass-squared exponent for one returned terminal. The constructor
    /// admits the generic unit-mass vacuum family through the existing family
    /// service. No mass normalization or numerical master evaluation occurs here.
    pub fn common_mass_squared_power(
        &self,
        terminal: &IntegralKey,
    ) -> Result<i128, CandidateReductionError> {
        if !self.terms.contains_key(terminal) {
            return Err(CandidateReductionError::InvalidInput(
                "requested key is not a returned candidate terminal".into(),
            ));
        }
        let sum = |key: &IntegralKey| {
            key.powers()
                .iter()
                .try_fold(0_i128, |s, &n| s.checked_add(i128::from(n)))
                .ok_or(CandidateReductionError::Application(
                    ReductionError::CommonMassPowerOverflow,
                ))
        };
        sum(terminal)?
            .checked_sub(sum(&self.target)?)
            .ok_or(CandidateReductionError::Application(
                ReductionError::CommonMassPowerOverflow,
            ))
    }
}

/// Work and retained-cache census of an experimental candidate owner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CandidateStatistics {
    pub(super) work: ReductionStatistics,
    pub(super) cached_integrals: usize,
    pub(super) cached_coefficient_terms: usize,
    pub(super) cached_coefficient_bytes: usize,
}

/// Exact finite-DAG reachability report for an explicitly supplied target set.
///
/// This is not a certificate, even for the requested finite entries: candidate
/// formulas do not carry replayed original-source provenance. The reducer only
/// checks their internal applicability, denominators, strict descent, and
/// successor reachability. A missing or invalid successor aborts the operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateReachabilityReport {
    pub(super) requested_targets: usize,
    pub(super) reachable_integrals: usize,
    pub(super) reachable_terminals: usize,
    pub(super) max_negative_index_degree: u128,
    pub(super) max_positive_power_sum: u128,
}

impl CandidateReachabilityReport {
    pub fn requested_targets(&self) -> usize {
        self.requested_targets
    }

    pub fn reachable_integrals(&self) -> usize {
        self.reachable_integrals
    }

    pub fn reachable_terminals(&self) -> usize {
        self.reachable_terminals
    }

    /// Maximum `sum(max(-n_i, 0))` over the requested entries and all
    /// reachable successors retained in the finite reduction DAG.
    pub fn max_negative_index_degree(&self) -> u128 {
        self.max_negative_index_degree
    }

    /// Maximum `sum(max(n_i, 0))` over the same finite DAG. Positive powers
    /// are intentionally reported rather than silently treated as bounded by
    /// the entry rank.
    pub fn max_positive_power_sum(&self) -> u128 {
        self.max_positive_power_sum
    }
}

impl CandidateStatistics {
    pub fn rule_applications(self) -> usize {
        self.work.rule_applications()
    }
    pub fn cache_hits(self) -> usize {
        self.work.cache_hits()
    }
    pub fn coalescing_additions(self) -> usize {
        self.work.coalescing_additions()
    }
    pub fn cached_integrals(self) -> usize {
        self.cached_integrals
    }
    /// Charged sparse terms. Factorized cache mode conservatively charges at
    /// least both stored parts and the expanded-denominator support envelope;
    /// this is not an exact materialized term census in that mode.
    pub fn cached_coefficient_terms(self) -> usize {
        self.cached_coefficient_terms
    }
    pub fn cached_coefficient_bytes(self) -> usize {
        self.cached_coefficient_bytes
    }
}

/// No error variant denotes closure or promotes an uncovered point to a terminal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CandidateReductionError {
    InvalidInput(String),
    UnsupportedFamily(String),
    InconsistentNumeratorRank {
        expected: Option<u32>,
        actual: Option<u32>,
    },
    OutsideNumeratorRank {
        target: IntegralKey,
        rank: u128,
        limit: u32,
    },
    TraceInputLimit {
        requested: usize,
        limit: usize,
    },
    TraceIntegralLimit {
        requested: usize,
        limit: usize,
    },
    OutsideRoot {
        target: IntegralKey,
    },
    Uncovered {
        target: IntegralKey,
    },
    SourceConditionVanished {
        target: IntegralKey,
        ordinal: usize,
    },
    NonDescending {
        target: IntegralKey,
        child: IntegralKey,
        rule: usize,
    },
    IndexOverflow {
        target: IntegralKey,
        axis: usize,
        rule: usize,
    },
    Algebra(IndexedAlgebraError),
    Application(ReductionError),
}

impl fmt::Display for CandidateReductionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(s) => write!(f, "invalid candidate program: {s}"),
            Self::UnsupportedFamily(s) => write!(f, "unsupported candidate family: {s}"),
            Self::InconsistentNumeratorRank { expected, actual } => write!(
                f,
                "candidate entry rank scope {actual:?} differs from required scope {expected:?}"
            ),
            Self::OutsideNumeratorRank {
                target,
                rank,
                limit,
            } => write!(
                f,
                "candidate target {:?} has numerator rank {rank}, exceeding entry limit {limit}",
                target.powers()
            ),
            Self::TraceInputLimit { requested, limit } => write!(
                f,
                "candidate traversal received {requested} input targets, exceeding limit {limit}"
            ),
            Self::TraceIntegralLimit { requested, limit } => write!(
                f,
                "candidate traversal needs {requested} distinct integrals, exceeding limit {limit}"
            ),
            Self::OutsideRoot { target } => write!(
                f,
                "candidate target {:?} is outside the supplied root",
                target.powers()
            ),
            Self::Uncovered { target } => write!(
                f,
                "candidate formulas do not cover {:?}; no terminal was inferred",
                target.powers()
            ),
            Self::SourceConditionVanished { target, ordinal } => write!(
                f,
                "candidate source condition {ordinal} vanishes at {:?}",
                target.powers()
            ),
            Self::NonDescending {
                target,
                child,
                rule,
            } => write!(
                f,
                "candidate rule {rule} is not descending: {:?} -> {:?}",
                target.powers(),
                child.powers()
            ),
            Self::IndexOverflow { target, axis, rule } => write!(
                f,
                "candidate rule {rule} overflows index {axis} at {:?}",
                target.powers()
            ),
            Self::Algebra(error) => error.fmt(f),
            Self::Application(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for CandidateReductionError {}
impl From<IndexedAlgebraError> for CandidateReductionError {
    fn from(e: IndexedAlgebraError) -> Self {
        Self::Algebra(e)
    }
}
impl From<ReductionError> for CandidateReductionError {
    fn from(e: ReductionError) -> Self {
        Self::Application(e)
    }
}
impl From<ExactAlgebraError> for CandidateReductionError {
    fn from(e: ExactAlgebraError) -> Self {
        Self::Application(ReductionError::ExactAlgebra(e))
    }
}

#[derive(Debug)]
pub(super) struct PreparedRule<const N: usize> {
    pub ordinal: usize,
    pub fixed: [Option<i16>; N],
    pub equalities: Vec<IndexedPolynomial>,
    pub exceptions: Vec<Vec<IndexedPolynomial>>,
    pub rhs: Vec<PreparedTerm<N>>,
}

#[derive(Debug)]
pub(super) struct PreparedTerm<const N: usize> {
    pub shift: [i64; N],
    pub coefficient: IndexedCoefficient,
    pub denominator: IndexedPolynomial,
}
