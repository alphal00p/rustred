use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use crate::algebra::{
    Coefficient, ExactAlgebraError, IndexedAlgebraError, IndexedCoefficient, IndexedPolynomial,
};
use crate::family::IntegralKey;
use crate::reduction::{ReductionError, ReductionStatistics};
use crate::solver::FiniteCasePolicy;

/// Whether uncovered points abort application or survive as exact unresolved
/// integrals. Preserving a point never declares it a master or certifies scope.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UnresolvedPolicy {
    #[default]
    Reject,
    Preserve,
}

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

/// Family-shape admission of the explicitly uncertified candidate applier.
///
/// Neither policy grants closure authority: candidate formulas carry no
/// original-source replay or cover proof in either case.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CandidateFamilyAdmission {
    /// The publication shape shared with artifact installation: an uncut,
    /// unshifted vacuum family with unit masses whose only parameter is `d`.
    #[default]
    UnitMassVacuum,
    /// UNCERTIFIED lane: any uncut, unshifted vacuum family, including
    /// symbolic or numeric masses and extra parameters (e.g. a deformation
    /// `D_s -> D_s - eta`, or a numeric `d` with only `eta` symbolic). Only
    /// the unit-mass predicate is bypassed. The admission is recorded on the
    /// program ([`crate::solver::CandidateReducer::admission`]). The exact
    /// applier accepts it, but consumers that assume unit masses (for
    /// example common-mass power extraction) must check it and refuse such
    /// programs. A candidate program is never a closing artifact under
    /// any policy.
    ParametricVacuum,
    /// The publication shape of a propagator-type ("p-integral") family:
    /// one external momentum `q` with numeric Gram `q.q = +-1`, rational
    /// denominator constants, sole parameter `d` and no power shifts (see
    /// [`crate::family::UnitScaleKinematics::UnitScalePropagator`]). The
    /// common-scale exponent of [`CandidateDecomposition`] then counts powers
    /// of `q^2 / q.q`, exactly as it counts `m^2` for a unit-mass vacuum.
    UnitScalePropagator,
}

impl CandidateFamilyAdmission {
    /// The unit-scale admission matching a family's kinematics:
    /// [`Self::UnitScalePropagator`] for a family with external momenta and
    /// [`Self::UnitMassVacuum`] otherwise. The selected predicate is still
    /// checked in full when a program is prepared.
    pub fn unit_scale_for(family: &crate::family::IntegralFamily) -> Self {
        if family.external_count() == 0 {
            Self::UnitMassVacuum
        } else {
            Self::UnitScalePropagator
        }
    }
}

/// Exact arithmetic result of candidate formulas, not certified IBP provenance
/// or a proof that the listed finite terminals form a complete master basis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateDecomposition {
    pub(super) family_fingerprint: Arc<String>,
    pub(super) target: IntegralKey,
    pub(super) terms: BTreeMap<IntegralKey, Coefficient>,
    pub(super) unresolved: BTreeSet<IntegralKey>,
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
    /// Nonzero output keys outside the installed canonical terminal inventory.
    /// Exact cancellations have already been removed.
    pub fn unresolved(&self) -> &BTreeSet<IntegralKey> {
        &self.unresolved
    }
    /// Transfer a native result into a core-owned semantic composition without
    /// retaining a second sparse coefficient copy.
    pub(crate) fn into_terms(self) -> BTreeMap<IntegralKey, Coefficient> {
        self.terms
    }
    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }

    /// Common-mass-squared exponent for one returned terminal. The constructor
    /// admits the generic unit-mass vacuum family through the existing family
    /// service. For a [`CandidateFamilyAdmission::UnitScalePropagator`]
    /// program the same exponent counts powers of the external scale
    /// `q^2 / q.q`. No normalization or numerical master evaluation occurs here.
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
    InconsistentFiniteCasePolicy {
        expected: FiniteCasePolicy,
        actual: FiniteCasePolicy,
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
            Self::InconsistentFiniteCasePolicy { expected, actual } => write!(
                f,
                "candidate finite-case policy {} differs from required policy {}",
                actual.as_str(),
                expected.as_str()
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
impl std::error::Error for CandidateReductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Algebra(error) => Some(error),
            Self::Application(error) => Some(error),
            _ => None,
        }
    }
}
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
pub(crate) struct PreparedRule<const N: usize> {
    pub ordinal: usize,
    pub dispatch_policy: crate::solver::RuleDispatchPolicy,
    /// Original authenticated equality chart, Arc-shared for affine cases.
    pub case: crate::solver::Case<N>,
    pub fixed: [Option<i16>; N],
    pub equalities: Vec<IndexedPolynomial>,
    pub exceptions: Vec<Vec<IndexedPolynomial>>,
    pub rhs: RuleBody<N>,
}

/// batch 12c (on-demand step 3): restores and prepares the right-hand side of
/// a lazily loaded rule at its first use. `load` must give the terms that the
/// eager preparation gives for the same rule (same order, same values);
/// `denominators` are the rule's prepared dispatch denominators, for sharing.
pub(crate) trait RuleBodyLoader<const N: usize>: Send + Sync {
    fn load(
        &self,
        rule: usize,
        denominators: &[Arc<IndexedPolynomial>],
    ) -> Result<Vec<PreparedTerm<N>>, CandidateReductionError>;
}

/// The lazy part of a [`RuleBody`]: where to load it and what dispatch needs
/// before it is loaded.
pub(crate) struct LazyRuleBody<const N: usize> {
    pub loader: Arc<dyn RuleBodyLoader<N>>,
    /// The rule's index for `loader`.
    pub index: usize,
    /// Number of right-hand-side terms.
    pub len: usize,
    /// `len` minus the number of distinct term shifts (the batch's coalescing
    /// bound, [`super::owners::PreparedOwnerBatch`]).
    pub duplicate_shifts: usize,
    /// The distinct original denominators of the terms, first-appearance
    /// order (each prepared like a term's denominator).
    pub denominators: Vec<Arc<IndexedPolynomial>>,
}

/// The right-hand side of a prepared rule: prepared at load (eager), or
/// restored and prepared at the rule's first use (batch 12c, lazy bodies of a
/// p library payload; `RUSTRED_P_RULE_BODIES=eager` prepares every rule at
/// load as before). Dispatch reads only [`Self::dispatch_denominators`], so a
/// rule that is never selected is never restored. A loaded body is kept.
pub(crate) struct RuleBody<const N: usize> {
    terms: std::sync::OnceLock<Vec<PreparedTerm<N>>>,
    lazy: Option<LazyRuleBody<N>>,
}

impl<const N: usize> fmt::Debug for RuleBody<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.lazy, self.terms.get()) {
            (_, Some(terms)) => f.debug_list().entries(terms).finish(),
            (Some(lazy), None) => write!(f, "<lazy body: {} terms, not loaded>", lazy.len),
            (None, None) => f.write_str("[]"),
        }
    }
}

impl<const N: usize> From<Vec<PreparedTerm<N>>> for RuleBody<N> {
    fn from(terms: Vec<PreparedTerm<N>>) -> Self {
        Self::eager(terms)
    }
}

impl<const N: usize> RuleBody<N> {
    pub fn eager(terms: Vec<PreparedTerm<N>>) -> Self {
        let cell = std::sync::OnceLock::new();
        let _ = cell.set(terms);
        Self {
            terms: cell,
            lazy: None,
        }
    }

    pub fn lazy(body: LazyRuleBody<N>) -> Self {
        Self {
            terms: std::sync::OnceLock::new(),
            lazy: Some(body),
        }
    }

    /// The terms, loaded on first use; a load failure is returned (and the
    /// next call tries again).
    pub fn terms(&self) -> Result<&[PreparedTerm<N>], CandidateReductionError> {
        if let Some(terms) = self.terms.get() {
            return Ok(terms);
        }
        let lazy = self.lazy.as_ref().expect("an eager body is always set");
        let terms = lazy.loader.load(lazy.index, &lazy.denominators)?;
        if terms.len() != lazy.len {
            return Err(CandidateReductionError::InvalidInput(format!(
                "lazy rule body {}: {} terms loaded, {} recorded",
                lazy.index,
                terms.len(),
                lazy.len
            )));
        }
        // Two threads may load the same body; the values are equal, the
        // first one is kept.
        let _ = self.terms.set(terms);
        Ok(self.terms.get().expect("just set"))
    }

    /// Number of terms, without loading.
    pub fn len(&self) -> usize {
        match (self.terms.get(), &self.lazy) {
            (Some(terms), _) => terms.len(),
            (None, Some(lazy)) => lazy.len,
            (None, None) => 0,
        }
    }

    /// `len` minus the number of distinct shifts, without loading.
    pub fn duplicate_shifts(&self) -> usize {
        match &self.lazy {
            Some(lazy) => lazy.duplicate_shifts,
            None => {
                let terms = self.terms.get().map_or(&[][..], Vec::as_slice);
                let unique: BTreeSet<_> = terms.iter().map(|term| &term.shift).collect();
                terms.len() - unique.len()
            }
        }
    }

    /// The original denominators that dispatch tests at a key, without
    /// loading: every term's (eager, as before) or the distinct ones in
    /// first-appearance order (lazy). Both give the same applicability and
    /// the same conditions in the same order (equal denominators specialise
    /// equally and a condition is recorded once).
    pub fn dispatch_denominators(&self) -> Box<dyn Iterator<Item = &IndexedPolynomial> + '_> {
        match (&self.lazy, self.terms.get()) {
            (Some(lazy), _) => Box::new(lazy.denominators.iter().map(|d| &**d)),
            (None, Some(terms)) => Box::new(terms.iter().map(|term| &*term.denominator)),
            (None, None) => Box::new(std::iter::empty()),
        }
    }

    /// The loaded terms. Panics if a lazy body fails to load: for analysis
    /// paths outside the step path (the step path calls [`Self::terms`]).
    fn loaded(&self) -> &Vec<PreparedTerm<N>> {
        if let Err(error) = self.terms() {
            panic!("lazy rule body failed to load: {error}");
        }
        self.terms.get().expect("loaded")
    }
}

impl<const N: usize> std::ops::Deref for RuleBody<N> {
    type Target = Vec<PreparedTerm<N>>;
    fn deref(&self) -> &Self::Target {
        self.loaded()
    }
}

impl<const N: usize> std::ops::DerefMut for RuleBody<N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.loaded();
        self.terms.get_mut().expect("loaded")
    }
}

impl<'a, const N: usize> IntoIterator for &'a mut RuleBody<N> {
    type Item = &'a mut PreparedTerm<N>;
    type IntoIter = std::slice::IterMut<'a, PreparedTerm<N>>;
    fn into_iter(self) -> Self::IntoIter {
        std::ops::DerefMut::deref_mut(self).iter_mut()
    }
}

impl<'a, const N: usize> IntoIterator for &'a RuleBody<N> {
    type Item = &'a PreparedTerm<N>;
    type IntoIter = std::slice::Iter<'a, PreparedTerm<N>>;
    fn into_iter(self) -> Self::IntoIter {
        self.loaded().iter()
    }
}

/// One RHS term of a prepared rule. t162-export (memory of the loaded form):
/// the shift is stored as `i8` (a difference of two powers of
/// [`crate::solver::Power`]'s range [-64, 63], checked at preparation;
/// [`Self::shift`] widens it) and the coefficient and
/// its denominator are shared (`Arc`): preparation interns equal values, so a
/// table whose terms repeat coefficients or pivot denominators holds each one
/// once. The values and their arithmetic are unchanged.
#[derive(Debug)]
pub(crate) struct PreparedTerm<const N: usize> {
    pub shift: [i8; N],
    pub coefficient: Arc<IndexedCoefficient>,
    pub denominator: Arc<IndexedPolynomial>,
}

impl<const N: usize> PreparedTerm<N> {
    /// The shift widened to `i64` (the arithmetic type of keys).
    pub fn shift(&self) -> [i64; N] {
        self.shift.map(i64::from)
    }
}
