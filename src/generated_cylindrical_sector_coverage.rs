//! Product-free ordered sector coverage for generated cylindrical candidates.
//!
//! This is a new authority path, parallel to the anchored V4 coverage
//! certificate.  It retains exact generated-cylindrical `WhenBad` attempts in
//! caller order and never assumes that their persistent sources share one row
//! span.  Only the common mathematical bad-domain formula is normalized:
//! index-dependent denominator guards and sector-boundary leak events.  Base
//! parameter assumptions remain attached to the selected candidate and are
//! preserved later by concrete specialization.
//!
//! Every supplied attempt is fully replayed, freshly recompiled from its
//! retained Global candidate under its own `WhenBad` limits, and deep-compared
//! before any formula payload is trusted.  The normalized formula is lowered
//! through the unchanged product-free MTBDD compiler.  Exhausting the ordered
//! attempts yields only `Uncovered` or explicit `Unsupported`; this layer has
//! no master- or zero-integral terminal.

use std::fmt;
use std::mem::size_of;
use std::sync::Arc;

#[cfg(test)]
use std::cell::Cell;

use crate::coverage_decision_dag::CoverageDecisionDagError;
use crate::generated_cylindrical_candidate_authority::GeneratedCylindricalReplaySession;
use crate::parametric_sector_coverage::{
    ParametricSectorCoverageError, ParametricSectorCoverageLimits, ParametricSectorCoverageStats,
    ParametricSectorFormulaNormalizationLimits, ParametricSectorFormulaNormalizationStats,
    normalize_candidate_bad_formula, validate_family_context,
};
use crate::parametric_sector_formula_ir::{
    NormalizedCandidateBadFormula, NormalizedCoverageAttempt, NormalizedCoverageIr,
    ParametricSectorFormulaIrError,
};
use crate::parametric_sector_mtbdd::{
    ParametricSectorMtbddCompiler, ParametricSectorMtbddDecisionFunction,
    ParametricSectorMtbddDisposition, ParametricSectorMtbddError, ParametricSectorMtbddLimits,
};
use crate::{
    GeneratedCylindricalCandidateAuthority, GeneratedCylindricalCandidateAuthorityError,
    GeneratedCylindricalCandidateAuthorityLimits,
    GeneratedCylindricalPersistentEliminationCertificate, GeneratedCylindricalWhenBadCertificate,
    GeneratedCylindricalWhenBadCompilation, GeneratedCylindricalWhenBadCompiler,
    GeneratedCylindricalWhenBadUnsupported, IntegralFamily, IntegralOrderingPolicy,
    ParametricArithmeticLimits, ParametricCoefficientContext, ParametricPolynomial, SectorMask,
    WhenBadCompiler, WhenBadCompilerError, WhenBadCompilerLimits, WhenBadLeafDisposition,
};

/// Stable schema for product-free generated cylindrical sector coverage.
pub const GENERATED_CYLINDRICAL_SECTOR_COVERAGE_V1_SCHEMA: &str =
    "rustred-generated-cylindrical-sector-coverage-v1";

/// V2 adds exhaustive one-persistent-source derivation provenance. V1 remains
/// the stable schema for caller-supplied authenticated attempt batches.
pub const GENERATED_CYLINDRICAL_SECTOR_COVERAGE_V2_SCHEMA: &str =
    "rustred-generated-cylindrical-sector-coverage-v2";

/// Independent resource policy for the generated cylindrical coverage path.
///
/// The private normalized-formula and MTBDD policies are deterministically
/// derived from these fields and their own V1 defaults.  No limit is learned
/// from an anchored V4 coverage certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedCylindricalSectorCoverageLimits {
    pub arithmetic: ParametricArithmeticLimits,
    pub max_family_fingerprint_bytes: usize,
    pub max_context_fingerprint_bytes: usize,
    pub max_arity: usize,
    pub max_attempts: usize,
    pub max_attempt_arc_reference_bytes: usize,
    pub max_unique_persistent_sources: usize,
    pub max_persistent_source_dedup_pointer_index_bytes: usize,
    pub max_replay_session_source_reference_bytes: usize,
    pub max_replay_session_source_pointer_index_bytes: usize,
    pub max_candidate_retained_payload_bytes: usize,
    pub max_persistent_source_retained_bytes: usize,
    pub max_when_bad_binding_bytes: usize,
    pub max_when_bad_retained_core_bytes: usize,
    pub max_when_bad_condition_terms: usize,
    pub max_when_bad_condition_bytes: usize,
    pub max_when_bad_guard_origin_retained_bytes: usize,
    pub max_when_bad_leak_event_retained_bytes: usize,
    pub max_base_structural_loci: usize,
    pub max_base_structural_locus_terms: usize,
    pub max_base_structural_locus_bytes: usize,
    pub max_base_locus_associate_comparisons: usize,
    pub max_base_locus_associate_term_pairs: usize,
    pub max_decision_atoms: usize,
    pub max_concrete_locus_evaluations: usize,
}

impl Default for GeneratedCylindricalSectorCoverageLimits {
    fn default() -> Self {
        Self {
            arithmetic: ParametricArithmeticLimits::default(),
            max_family_fingerprint_bytes: 1024 * 1024,
            max_context_fingerprint_bytes: 1024 * 1024,
            max_arity: 1_000_000,
            max_attempts: 1_000_000,
            max_attempt_arc_reference_bytes: 128 * 1024 * 1024,
            max_unique_persistent_sources: 1_000_000,
            max_persistent_source_dedup_pointer_index_bytes: 128 * 1024 * 1024,
            max_replay_session_source_reference_bytes: 128 * 1024 * 1024,
            max_replay_session_source_pointer_index_bytes: 128 * 1024 * 1024,
            max_candidate_retained_payload_bytes: 512 * 1024 * 1024,
            max_persistent_source_retained_bytes: 2 * 1024 * 1024 * 1024,
            max_when_bad_binding_bytes: 512 * 1024 * 1024,
            max_when_bad_retained_core_bytes: 2 * 1024 * 1024 * 1024,
            max_when_bad_condition_terms: 32_000_000,
            max_when_bad_condition_bytes: 2 * 1024 * 1024 * 1024,
            max_when_bad_guard_origin_retained_bytes: 2 * 1024 * 1024 * 1024,
            max_when_bad_leak_event_retained_bytes: 2 * 1024 * 1024 * 1024,
            max_base_structural_loci: 16_000_000,
            max_base_structural_locus_terms: 32_000_000,
            max_base_structural_locus_bytes: 2 * 1024 * 1024 * 1024,
            max_base_locus_associate_comparisons: 32_000_000,
            max_base_locus_associate_term_pairs: 512_000_000,
            max_decision_atoms: 16_000_000,
            max_concrete_locus_evaluations: 16_000_000,
        }
    }
}

impl GeneratedCylindricalSectorCoverageLimits {
    fn normalization_limits(self) -> ParametricSectorFormulaNormalizationLimits {
        ParametricSectorFormulaNormalizationLimits::default().with_max_attempts(self.max_attempts)
    }

    fn normalization_algebra_limits(self) -> ParametricSectorCoverageLimits {
        let mut limits = ParametricSectorCoverageLimits::default();
        limits.generated_when_bad.when_bad.arithmetic = self.arithmetic;
        limits
            .generated_when_bad
            .when_bad
            .sector_cases
            .exact_algebra = self.arithmetic.exact_algebra;
        limits.sector_cases.exact_algebra = self.arithmetic.exact_algebra;
        limits.coordinate_loci.exact_algebra = self.arithmetic.exact_algebra;
        limits.max_candidates = self.max_attempts;
        limits.max_unique_predicates = self.max_base_structural_loci;
        limits.max_retained_structural_locus_terms = self.max_base_structural_locus_terms;
        limits.max_retained_structural_locus_bytes = self.max_base_structural_locus_bytes;
        limits.max_structural_locus_associate_comparisons =
            self.max_base_locus_associate_comparisons;
        limits.max_structural_locus_associate_term_pairs = self.max_base_locus_associate_term_pairs;
        limits
    }

    fn mtbdd_limits(self) -> ParametricSectorMtbddLimits {
        let mut limits = ParametricSectorMtbddLimits::default();
        limits.max_base_structural_loci = limits
            .max_base_structural_loci
            .min(self.max_base_structural_loci);
        limits.max_attempts = limits.max_attempts.min(self.max_attempts);
        limits.max_atoms = limits.max_atoms.min(self.max_decision_atoms);
        limits
    }
}

/// Construction census and conservative retained-payload charge for one
/// certificate.
///
/// Persistent sources are pointer-deduplicated. Candidate/`WhenBad` payloads
/// are charged once per retained attempt reference, so deliberately repeated
/// `Arc`s can be overcharged but can never evade an aggregate limit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedCylindricalSectorCoverageStats {
    attempts: usize,
    certified_attempts: usize,
    unsupported_attempts: usize,
    attempt_arc_reference_bytes: usize,
    unique_persistent_sources: usize,
    persistent_source_dedup_pointer_index_bytes: usize,
    replay_session_source_reference_bytes: usize,
    replay_session_source_pointer_index_bytes: usize,
    candidate_retained_payload_bytes: usize,
    persistent_source_retained_bytes: usize,
    when_bad_binding_bytes: usize,
    when_bad_retained_core_bytes: usize,
    when_bad_condition_terms: usize,
    when_bad_condition_bytes: usize,
    when_bad_guard_origin_retained_bytes: usize,
    when_bad_leak_event_retained_bytes: usize,
    base_structural_loci: usize,
    base_structural_locus_terms: usize,
    base_structural_locus_bytes: usize,
    normalized_clauses: usize,
    normalized_literals: usize,
    normalized_clause_source_references: usize,
    normalized_factor_references: usize,
    decision_atoms: usize,
    decision_nodes: usize,
    decision_terminals: usize,
}

macro_rules! stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedCylindricalSectorCoverageStats {
    stats_getters!(
        attempts,
        certified_attempts,
        unsupported_attempts,
        attempt_arc_reference_bytes,
        unique_persistent_sources,
        persistent_source_dedup_pointer_index_bytes,
        replay_session_source_reference_bytes,
        replay_session_source_pointer_index_bytes,
        candidate_retained_payload_bytes,
        persistent_source_retained_bytes,
        when_bad_binding_bytes,
        when_bad_retained_core_bytes,
        when_bad_condition_terms,
        when_bad_condition_bytes,
        when_bad_guard_origin_retained_bytes,
        when_bad_leak_event_retained_bytes,
        base_structural_loci,
        base_structural_locus_terms,
        base_structural_locus_bytes,
        normalized_clauses,
        normalized_literals,
        normalized_clause_source_references,
        normalized_factor_references,
        decision_atoms,
        decision_nodes,
        decision_terminals,
    );
}

/// Transient O(n log n) exact-allocation index for one caller-supplied
/// attempt batch. Sorting by address and then attempt ordinal preserves the
/// first input occurrence after deduplication; the exact source itself remains
/// pinned by the attempt `Arc` throughout this construction pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GeneratedCylindricalCoverageSourcePointerIndexEntry {
    address: usize,
    first_attempt_ordinal: usize,
}

/// Exact exhaustive-pivot provenance for coverage compiled directly from one
/// persistent cylindrical elimination. It remains inseparable from the
/// coverage so replay can re-enumerate every guarded pivot, including the
/// zero-pivot case where no attempt otherwise retains the source.
#[derive(Clone, Debug)]
pub struct GeneratedCylindricalSectorCoverageBatchProvenance {
    source: Arc<GeneratedCylindricalPersistentEliminationCertificate>,
    candidate_limits: GeneratedCylindricalCandidateAuthorityLimits,
    when_bad_limits: WhenBadCompilerLimits,
}

impl GeneratedCylindricalSectorCoverageBatchProvenance {
    pub const fn source(&self) -> &Arc<GeneratedCylindricalPersistentEliminationCertificate> {
        &self.source
    }

    pub const fn candidate_limits(&self) -> GeneratedCylindricalCandidateAuthorityLimits {
        self.candidate_limits
    }

    pub const fn when_bad_limits(&self) -> WhenBadCompilerLimits {
        self.when_bad_limits
    }

    fn shares_exact_source_and_limits(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.source, &other.source)
            && self.candidate_limits == other.candidate_limits
            && self.when_bad_limits == other.when_bad_limits
    }
}

/// One retained generated cylindrical attempt.
///
/// Typed `Arc` arms let a downstream provider cheaply retain the exact
/// selected certified proof. In particular, it never has to deep-clone a
/// certificate out of an `Arc<GeneratedCylindricalWhenBadCompilation>`.
#[derive(Clone, Debug)]
pub enum GeneratedCylindricalSectorCoverageAttempt {
    Certified(Arc<GeneratedCylindricalWhenBadCertificate>),
    Unsupported(Arc<GeneratedCylindricalWhenBadUnsupported>),
}

impl GeneratedCylindricalSectorCoverageAttempt {
    pub fn certified(certificate: Arc<GeneratedCylindricalWhenBadCertificate>) -> Self {
        Self::Certified(certificate)
    }

    pub fn unsupported(unsupported: Arc<GeneratedCylindricalWhenBadUnsupported>) -> Self {
        Self::Unsupported(unsupported)
    }

    pub fn candidate(&self) -> &crate::GeneratedCylindricalGlobalCandidateAuthority {
        match self {
            Self::Certified(certificate) => certificate.candidate(),
            Self::Unsupported(unsupported) => unsupported.candidate(),
        }
    }

    pub const fn is_certified(&self) -> bool {
        matches!(self, Self::Certified(_))
    }

    pub const fn is_unsupported(&self) -> bool {
        matches!(self, Self::Unsupported(_))
    }

    pub const fn certified_arc(&self) -> Option<&Arc<GeneratedCylindricalWhenBadCertificate>> {
        match self {
            Self::Certified(certificate) => Some(certificate),
            Self::Unsupported(_) => None,
        }
    }

    fn binding(&self) -> &crate::WhenBadCandidateBinding {
        match self {
            Self::Certified(certificate) => certificate.binding(),
            Self::Unsupported(unsupported) => unsupported.binding(),
        }
    }

    fn limits(&self) -> crate::WhenBadCompilerLimits {
        match self {
            Self::Certified(certificate) => certificate.limits(),
            Self::Unsupported(unsupported) => unsupported.limits(),
        }
    }

    fn retained_core_bytes(&self) -> usize {
        match self {
            Self::Certified(certificate) => certificate.retained_core_bytes(),
            Self::Unsupported(unsupported) => unsupported.retained_core_bytes(),
        }
    }

    fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), WhenBadCompilerError> {
        match self {
            Self::Certified(certificate) => certificate.replay(family, context),
            Self::Unsupported(unsupported) => unsupported.replay(family, context),
        }
    }

    fn preflight_replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), WhenBadCompilerError> {
        match self {
            Self::Certified(certificate) => certificate.preflight_replay(family, context),
            Self::Unsupported(unsupported) => unsupported.preflight_replay(family, context),
        }
    }

    fn replay_with_authenticated_session(
        &self,
        session: &GeneratedCylindricalReplaySession<'_>,
    ) -> Result<(), WhenBadCompilerError> {
        match self {
            Self::Certified(certificate) => certificate.replay_with_authenticated_session(session),
            Self::Unsupported(unsupported) => {
                unsupported.replay_with_authenticated_session(session)
            }
        }
    }

    fn payload_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Certified(left), Self::Certified(right)) => left.payload_eq(right),
            (Self::Unsupported(left), Self::Unsupported(right)) => left.payload_eq(right),
            _ => false,
        }
    }

    fn shares_exact_allocation(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Certified(left), Self::Certified(right)) => Arc::ptr_eq(left, right),
            (Self::Unsupported(left), Self::Unsupported(right)) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }

    fn payload_eq_with_replayed_source(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Certified(left), Self::Certified(right)) => {
                left.payload_eq_with_replayed_source(right)
            }
            (Self::Unsupported(left), Self::Unsupported(right)) => {
                left.payload_eq_with_replayed_source(right)
            }
            _ => false,
        }
    }
}

/// Concrete result of the ordered coverage decision.
///
/// The unsupported ordinal slice is borrowed directly from the authenticated
/// MTBDD terminal, so concrete classification performs no result allocation.
#[derive(Debug)]
pub enum GeneratedCylindricalSectorLeafDisposition<'a> {
    DescendingRule {
        candidate_ordinal: usize,
        candidate: &'a Arc<GeneratedCylindricalWhenBadCertificate>,
    },
    Uncovered,
    Unsupported {
        candidate_ordinals: &'a [usize],
    },
}

impl<'a> GeneratedCylindricalSectorLeafDisposition<'a> {
    pub const fn candidate_ordinal(&self) -> Option<usize> {
        match self {
            Self::DescendingRule {
                candidate_ordinal, ..
            } => Some(*candidate_ordinal),
            Self::Uncovered | Self::Unsupported { .. } => None,
        }
    }

    /// The exact generated `WhenBad` proof selected by ordered coverage.
    pub fn selected_candidate(&self) -> Option<&'a GeneratedCylindricalWhenBadCertificate> {
        match self {
            Self::DescendingRule { candidate, .. } => Some((*candidate).as_ref()),
            Self::Uncovered | Self::Unsupported { .. } => None,
        }
    }

    /// Cheap owning seam for the later concrete-rule provider.
    pub fn selected_candidate_arc(&self) -> Option<Arc<GeneratedCylindricalWhenBadCertificate>> {
        match self {
            Self::DescendingRule { candidate, .. } => Some(Arc::clone(*candidate)),
            Self::Uncovered | Self::Unsupported { .. } => None,
        }
    }

    pub const fn unsupported_candidate_ordinals(&self) -> Option<&'a [usize]> {
        match self {
            Self::Unsupported { candidate_ordinals } => Some(*candidate_ordinals),
            Self::DescendingRule { .. } | Self::Uncovered => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedCylindricalSectorCoverageError {
    SchemaMismatch,
    WrongFamily,
    WrongContext,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    CandidateWrongFamily {
        ordinal: usize,
    },
    CandidateWrongContext {
        ordinal: usize,
    },
    CandidateWrongSector {
        ordinal: usize,
    },
    CandidateWrongOrdering {
        ordinal: usize,
    },
    BatchSourceIncomplete {
        pending_equality_predicates: usize,
    },
    BatchPivotOrdinalMismatch {
        expected: usize,
        actual: usize,
    },
    BatchLocusBoundSource {
        fixed_coordinates: usize,
    },
    BatchLocusBoundCandidate {
        pivot_ordinal: usize,
    },
    BatchProvenanceMismatch {
        detail: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    FormulaInvariant,
    DecisionInvariant,
    CandidateDispositionMismatch {
        ordinal: usize,
    },
    ReplayMismatch,
    Normalization(Box<ParametricSectorCoverageError>),
    Candidate(Box<GeneratedCylindricalCandidateAuthorityError>),
    WhenBad(Box<WhenBadCompilerError>),
}

impl fmt::Display for GeneratedCylindricalSectorCoverageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "generated cylindrical sector coverage error: {self:?}"
        )
    }
}

impl std::error::Error for GeneratedCylindricalSectorCoverageError {}

impl From<ParametricSectorCoverageError> for GeneratedCylindricalSectorCoverageError {
    fn from(value: ParametricSectorCoverageError) -> Self {
        Self::Normalization(Box::new(value))
    }
}

impl From<WhenBadCompilerError> for GeneratedCylindricalSectorCoverageError {
    fn from(value: WhenBadCompilerError) -> Self {
        Self::WhenBad(Box::new(value))
    }
}

impl From<GeneratedCylindricalCandidateAuthorityError> for GeneratedCylindricalSectorCoverageError {
    fn from(value: GeneratedCylindricalCandidateAuthorityError) -> Self {
        Self::Candidate(Box::new(value))
    }
}

/// Owning, replayable ordered cover for one exact sector orthant.
pub struct GeneratedCylindricalSectorCoverageCertificate {
    schema: &'static str,
    family_fingerprint: String,
    context_fingerprint: String,
    sector: SectorMask,
    ordering_policy: IntegralOrderingPolicy,
    batch_provenance: Option<GeneratedCylindricalSectorCoverageBatchProvenance>,
    attempts: Vec<GeneratedCylindricalSectorCoverageAttempt>,
    // Keep the fallibly grown allocation. Converting a user-sized vector to
    // a boxed slice could request an infallible proportional shrink.
    base_structural_loci: Vec<ParametricPolynomial>,
    normalized_ir: NormalizedCoverageIr,
    decision: ParametricSectorMtbddDecisionFunction,
    limits: GeneratedCylindricalSectorCoverageLimits,
    stats: GeneratedCylindricalSectorCoverageStats,
}

impl GeneratedCylindricalSectorCoverageCertificate {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }

    pub const fn sector(&self) -> &SectorMask {
        &self.sector
    }

    pub const fn ordering_policy(&self) -> IntegralOrderingPolicy {
        self.ordering_policy
    }

    pub const fn batch_provenance(
        &self,
    ) -> Option<&GeneratedCylindricalSectorCoverageBatchProvenance> {
        self.batch_provenance.as_ref()
    }

    /// Exact generated attempts in persisted priority order.  Each `Arc`
    /// remains the caller's allocation; compilation only moves or cheaply
    /// shares it after complete preflight and fresh deep authentication.
    pub fn candidate_attempts(&self) -> &[GeneratedCylindricalSectorCoverageAttempt] {
        &self.attempts
    }

    pub fn base_structural_loci(&self) -> &[ParametricPolynomial] {
        &self.base_structural_loci
    }

    pub const fn limits(&self) -> GeneratedCylindricalSectorCoverageLimits {
        self.limits
    }

    pub const fn stats(&self) -> GeneratedCylindricalSectorCoverageStats {
        self.stats
    }

    /// Resolve one MTBDD candidate ordinal without weakening the certified
    /// arm requirement.  Unsupported attempts can never be returned as a
    /// reduction rule.
    pub fn selected_candidate(
        &self,
        candidate_ordinal: usize,
    ) -> Option<&Arc<GeneratedCylindricalWhenBadCertificate>> {
        self.attempts.get(candidate_ordinal)?.certified_arc()
    }

    /// Evaluate the immutable base-locus table exactly and route it through
    /// the product-free MTBDD.  A selected rule is independently checked
    /// against its local `WhenBad` partition before it is exposed.
    pub fn classification_for_indices<'a>(
        &'a self,
        context: &ParametricCoefficientContext,
        indices: &[i64],
    ) -> Result<
        Option<GeneratedCylindricalSectorLeafDisposition<'a>>,
        GeneratedCylindricalSectorCoverageError,
    > {
        if context.fingerprint() != self.context_fingerprint {
            return Err(GeneratedCylindricalSectorCoverageError::WrongContext);
        }
        if indices.len() != self.sector.arity() {
            return Err(GeneratedCylindricalSectorCoverageError::WrongArity {
                expected: self.sector.arity(),
                actual: indices.len(),
            });
        }
        if !self
            .sector
            .active_bits()
            .iter()
            .zip(indices)
            .all(|(&active, &index)| active == (index >= 1))
        {
            return Ok(None);
        }
        check_limit(
            "concrete base-locus evaluations",
            self.base_structural_loci.len(),
            self.limits.max_concrete_locus_evaluations,
        )?;
        let mut zero_by_locus = Vec::new();
        try_reserve_exact(
            "concrete base-locus truth assignment",
            &mut zero_by_locus,
            self.base_structural_loci.len(),
        )?;
        check_limit(
            "concrete base-locus truth assignment entries",
            zero_by_locus.capacity(),
            self.limits.max_concrete_locus_evaluations,
        )?;
        for polynomial in &self.base_structural_loci {
            zero_by_locus.push(
                context
                    .specialize_polynomial(polynomial, indices, self.limits.arithmetic)
                    .map_err(ParametricSectorCoverageError::from)?
                    .is_zero(),
            );
        }
        let disposition = self
            .decision
            .classify_assignment(&zero_by_locus)
            .map_err(map_mtbdd_error)?;
        match disposition {
            ParametricSectorMtbddDisposition::DescendingRule { candidate_ordinal } => {
                let candidate = self.selected_candidate(*candidate_ordinal).ok_or(
                    GeneratedCylindricalSectorCoverageError::CandidateDispositionMismatch {
                        ordinal: *candidate_ordinal,
                    },
                )?;
                let local = candidate.classification_for_indices(context, indices)?;
                if !matches!(
                    local.map(|classification| classification.disposition()),
                    Some(WhenBadLeafDisposition::CoveredByCandidate)
                ) {
                    return Err(
                        GeneratedCylindricalSectorCoverageError::CandidateDispositionMismatch {
                            ordinal: *candidate_ordinal,
                        },
                    );
                }
                Ok(Some(
                    GeneratedCylindricalSectorLeafDisposition::DescendingRule {
                        candidate_ordinal: *candidate_ordinal,
                        candidate,
                    },
                ))
            }
            ParametricSectorMtbddDisposition::Uncovered => {
                Ok(Some(GeneratedCylindricalSectorLeafDisposition::Uncovered))
            }
            ParametricSectorMtbddDisposition::Unsupported { candidate_ordinals } => {
                for &ordinal in candidate_ordinals.iter() {
                    if !matches!(
                        self.attempts.get(ordinal),
                        Some(GeneratedCylindricalSectorCoverageAttempt::Unsupported(_))
                    ) {
                        return Err(
                            GeneratedCylindricalSectorCoverageError::CandidateDispositionMismatch {
                                ordinal,
                            },
                        );
                    }
                }
                Ok(Some(
                    GeneratedCylindricalSectorLeafDisposition::Unsupported { candidate_ordinals },
                ))
            }
        }
    }

    /// Rebuild every authority and every derived formula/decision payload.
    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedCylindricalSectorCoverageError> {
        if self.family_fingerprint != family.fingerprint_ref() {
            return Err(GeneratedCylindricalSectorCoverageError::WrongFamily);
        }
        if self.context_fingerprint != context.fingerprint() {
            return Err(GeneratedCylindricalSectorCoverageError::WrongContext);
        }
        if let Some(provenance) = &self.batch_provenance {
            if self.schema != GENERATED_CYLINDRICAL_SECTOR_COVERAGE_V2_SCHEMA {
                return Err(GeneratedCylindricalSectorCoverageError::SchemaMismatch);
            }
            let replayed =
                GeneratedCylindricalSectorCoverageCompiler::compile_from_persistent_source(
                    family,
                    context,
                    Arc::clone(&provenance.source),
                    provenance.candidate_limits,
                    provenance.when_bad_limits,
                    self.limits,
                )?;
            return if self.payload_eq_with_replayed_batch(&replayed) {
                Ok(())
            } else {
                Err(GeneratedCylindricalSectorCoverageError::ReplayMismatch)
            };
        }
        if self.schema != GENERATED_CYLINDRICAL_SECTOR_COVERAGE_V1_SCHEMA {
            return Err(GeneratedCylindricalSectorCoverageError::SchemaMismatch);
        }
        let sector = copy_sector(&self.sector)?;
        let mut attempts = Vec::new();
        try_reserve_exact(
            "replayed generated cylindrical coverage attempts",
            &mut attempts,
            self.attempts.len(),
        )?;
        attempts.extend(self.attempts.iter().cloned());
        let replayed = GeneratedCylindricalSectorCoverageCompiler::compile_authenticated(
            family,
            context,
            sector,
            self.ordering_policy,
            attempts,
            self.limits,
        )?;
        if self.payload_eq_with_authenticated_attempts(&replayed) {
            Ok(())
        } else {
            Err(GeneratedCylindricalSectorCoverageError::ReplayMismatch)
        }
    }

    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.payload_eq_common(other)
            && self
                .attempts
                .iter()
                .zip(other.attempts.iter())
                .all(|(left, right)| left.payload_eq(right))
    }

    fn payload_eq_with_authenticated_attempts(&self, other: &Self) -> bool {
        self.payload_eq_common(other)
            && self
                .attempts
                .iter()
                .zip(other.attempts.iter())
                .all(|(left, right)| left.shares_exact_allocation(right))
    }

    fn payload_eq_with_replayed_batch(&self, other: &Self) -> bool {
        self.payload_eq_common(other)
            && self
                .attempts
                .iter()
                .zip(other.attempts.iter())
                .all(|(left, right)| left.payload_eq_with_replayed_source(right))
    }

    fn payload_eq_common(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.family_fingerprint == other.family_fingerprint
            && self.context_fingerprint == other.context_fingerprint
            && self.sector == other.sector
            && self.ordering_policy == other.ordering_policy
            && match (&self.batch_provenance, &other.batch_provenance) {
                (Some(left), Some(right)) => left.shares_exact_source_and_limits(right),
                (None, None) => true,
                _ => false,
            }
            && self.base_structural_loci == other.base_structural_loci
            && self.normalized_ir == other.normalized_ir
            && self.decision == other.decision
            && self.limits == other.limits
            && self.stats == other.stats
            && self.attempts.len() == other.attempts.len()
    }
}

impl fmt::Debug for GeneratedCylindricalSectorCoverageCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedCylindricalSectorCoverageCertificate")
            .field("schema", &self.schema)
            .field("family_fingerprint", &self.family_fingerprint)
            .field("context_fingerprint", &self.context_fingerprint)
            .field("sector", &self.sector)
            .field("ordering_policy", &self.ordering_policy)
            .field("has_batch_provenance", &self.batch_provenance.is_some())
            .field("attempt_count", &self.attempts.len())
            .field(
                "base_structural_locus_count",
                &self.base_structural_loci.len(),
            )
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

/// Compiler for an ordered batch of already persistent Global `WhenBad`
/// attempts.  Input `Arc`s are retained only after all batch-wide source and
/// scope limits have been checked.
pub struct GeneratedCylindricalSectorCoverageCompiler;

#[cfg(test)]
thread_local! {
    static EXHAUSTIVE_ATTEMPTS_RETAINED: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
fn reset_exhaustive_attempts_retained_for_test() {
    EXHAUSTIVE_ATTEMPTS_RETAINED.with(|count| count.set(0));
}

#[cfg(test)]
fn exhaustive_attempts_retained_for_test() -> usize {
    EXHAUSTIVE_ATTEMPTS_RETAINED.with(Cell::get)
}

#[cfg(test)]
fn record_exhaustive_attempt_retained_for_test() {
    EXHAUSTIVE_ATTEMPTS_RETAINED.with(|count| {
        count.set(count.get().saturating_add(1));
    });
}

/// Sealed distinction between arbitrary retained attempts, which require
/// local replay, and attempts produced immediately by the exhaustive
/// source→candidate-token→WhenBad compiler in the same operation.
#[derive(Clone, Copy)]
enum GeneratedCylindricalCoverageAttemptTrust {
    Persisted,
    FreshExhaustiveBatch,
}

impl GeneratedCylindricalSectorCoverageCompiler {
    /// Exhaustively derive ordered Global `WhenBad` attempts from every
    /// guarded pivot of one persistent cylindrical elimination.
    ///
    /// Sector, ordering, pivot ordinals, and attempt count are derived only
    /// from the authenticated source. No topology tag, concrete power,
    /// preferred pivot, expected coefficient, or master input enters this
    /// interface. Scope, source-locus, aggregate count, and fixed
    /// candidate/`WhenBad` lower-bound failures take precedence over full
    /// source replay; pivot-dependent limits are checked only after guarded
    /// provenance becomes authenticated.
    pub fn compile_from_persistent_source(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source: Arc<GeneratedCylindricalPersistentEliminationCertificate>,
        candidate_limits: GeneratedCylindricalCandidateAuthorityLimits,
        when_bad_limits: WhenBadCompilerLimits,
        limits: GeneratedCylindricalSectorCoverageLimits,
    ) -> Result<
        GeneratedCylindricalSectorCoverageCertificate,
        GeneratedCylindricalSectorCoverageError,
    > {
        let mut session = GeneratedCylindricalReplaySession::new(family, context);
        session.preflight_source_scope(&source)?;
        let start = source.row_system().start();
        if !start.completeness().is_complete_integer_cylinder() {
            return Err(
                GeneratedCylindricalSectorCoverageError::BatchSourceIncomplete {
                    pending_equality_predicates: start
                        .completeness()
                        .pending_equality_predicate_ordinals()
                        .len(),
                },
            );
        }
        if !start.assignment().is_empty() {
            return Err(
                GeneratedCylindricalSectorCoverageError::BatchLocusBoundSource {
                    fixed_coordinates: start.assignment().entries().len(),
                },
            );
        }
        let pivot_count = source.stats().pivot_assumption_closures();
        preflight_scope(
            family,
            context,
            family.fingerprint_ref(),
            start.sector(),
            pivot_count,
            limits,
        )?;
        GeneratedCylindricalCandidateAuthority::preflight_exhaustive_batch_fixed_limits(
            family,
            context,
            &source,
            pivot_count,
            candidate_limits,
        )?;
        WhenBadCompiler::preflight_replayed_cylindrical_batch_fixed_limits(
            context,
            pivot_count,
            when_bad_limits,
        )?;
        check_limit(
            "generated cylindrical unique persistent sources",
            1,
            limits.max_unique_persistent_sources,
        )?;
        let source_retained_bytes = source.stats().certificate_owned_retained_bytes();
        check_limit(
            "generated cylindrical persistent-source retained bytes",
            source_retained_bytes,
            limits.max_persistent_source_retained_bytes,
        )?;

        let sector = copy_sector(start.sector())?;
        let ordering_policy = start.ordering_policy();

        let mut attempts = Vec::new();
        try_reserve_exact(
            "generated cylindrical exhaustive pivot attempts",
            &mut attempts,
            pivot_count,
        )?;
        let attempt_reference_bytes = checked_mul(
            "generated cylindrical coverage attempt Arc reference bytes",
            attempts.capacity(),
            size_of::<GeneratedCylindricalSectorCoverageAttempt>(),
        )?;
        check_limit(
            "generated cylindrical coverage attempt Arc reference bytes",
            attempt_reference_bytes,
            limits.max_attempt_arc_reference_bytes,
        )?;

        session.authenticate_sources_with_table_byte_limits(
            &[&source],
            limits.max_replay_session_source_reference_bytes,
            limits.max_replay_session_source_pointer_index_bytes,
        )?;
        if source.guarded_pivots().len() != pivot_count {
            return Err(
                GeneratedCylindricalSectorCoverageError::BatchProvenanceMismatch {
                    detail: "authenticated guarded-pivot count differs from retained census",
                },
            );
        }
        let mut aggregate_payload_stats = GeneratedCylindricalSectorCoverageStats::default();
        for (expected_ordinal, guarded) in source.guarded_pivots().enumerate() {
            let pivot_ordinal = guarded.ordinal();
            if pivot_ordinal != expected_ordinal {
                return Err(
                    GeneratedCylindricalSectorCoverageError::BatchPivotOrdinalMismatch {
                        expected: expected_ordinal,
                        actual: pivot_ordinal,
                    },
                );
            }
            let candidate =
                GeneratedCylindricalCandidateAuthority::compile_fresh_with_authenticated_session(
                    Arc::clone(&source),
                    pivot_ordinal,
                    candidate_limits,
                    &session,
                )?;
            let GeneratedCylindricalCandidateAuthority::Global(candidate) = candidate else {
                return Err(
                    GeneratedCylindricalSectorCoverageError::BatchLocusBoundCandidate {
                        pivot_ordinal,
                    },
                );
            };
            let replayed_candidate = candidate.replay_with_authenticated_session(&session)?;
            let attempt = GeneratedCylindricalWhenBadCompiler::compile_replayed_candidate(
                replayed_candidate,
                when_bad_limits,
            )?;
            let attempt = match attempt {
                GeneratedCylindricalWhenBadCompilation::Certified(certificate) => {
                    GeneratedCylindricalSectorCoverageAttempt::certified(Arc::new(certificate))
                }
                GeneratedCylindricalWhenBadCompilation::Unsupported(unsupported) => {
                    GeneratedCylindricalSectorCoverageAttempt::unsupported(Arc::new(unsupported))
                }
            };
            // Exhaustive construction can be substantially more expensive
            // than the final coverage lowering. Charge every aggregate
            // candidate/WhenBad payload before publishing the newly built
            // attempt into the retained batch, and stop at the first
            // overflow. `compile_authenticated_with_replay_session` repeats
            // this census when it builds the final certificate, so the
            // certificate stats remain derived from their retained payload.
            census_attempt_aggregate_payload(&mut aggregate_payload_stats, &attempt, limits)?;
            attempts.push(attempt);
            #[cfg(test)]
            record_exhaustive_attempt_retained_for_test();
        }
        if attempts.len() != pivot_count {
            return Err(
                GeneratedCylindricalSectorCoverageError::BatchProvenanceMismatch {
                    detail: "guarded-pivot enumeration length changed after source replay",
                },
            );
        }

        let mut coverage = Self::compile_authenticated_with_replay_session(
            sector,
            ordering_policy,
            attempts,
            limits,
            &mut session,
            GeneratedCylindricalCoverageAttemptTrust::FreshExhaustiveBatch,
        )?;
        // The provenance source is retained even when there are no pivots and
        // therefore no attempt from which ordinary coverage could infer it.
        coverage.stats.unique_persistent_sources = 1;
        coverage.stats.persistent_source_retained_bytes = source_retained_bytes;
        coverage.stats.replay_session_source_reference_bytes = session.source_reference_bytes()?;
        coverage.stats.replay_session_source_pointer_index_bytes =
            session.source_pointer_index_bytes()?;
        coverage.schema = GENERATED_CYLINDRICAL_SECTOR_COVERAGE_V2_SCHEMA;
        coverage.batch_provenance = Some(GeneratedCylindricalSectorCoverageBatchProvenance {
            source,
            candidate_limits,
            when_bad_limits,
        });
        Ok(coverage)
    }

    pub fn compile_authenticated(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        sector: SectorMask,
        ordering_policy: IntegralOrderingPolicy,
        attempts: Vec<GeneratedCylindricalSectorCoverageAttempt>,
        limits: GeneratedCylindricalSectorCoverageLimits,
    ) -> Result<
        GeneratedCylindricalSectorCoverageCertificate,
        GeneratedCylindricalSectorCoverageError,
    > {
        let mut session = GeneratedCylindricalReplaySession::new(family, context);
        Self::compile_authenticated_with_replay_session(
            sector,
            ordering_policy,
            attempts,
            limits,
            &mut session,
            GeneratedCylindricalCoverageAttemptTrust::Persisted,
        )
    }

    fn compile_authenticated_with_replay_session(
        sector: SectorMask,
        ordering_policy: IntegralOrderingPolicy,
        attempts: Vec<GeneratedCylindricalSectorCoverageAttempt>,
        limits: GeneratedCylindricalSectorCoverageLimits,
        session: &mut GeneratedCylindricalReplaySession<'_>,
        attempt_trust: GeneratedCylindricalCoverageAttemptTrust,
    ) -> Result<
        GeneratedCylindricalSectorCoverageCertificate,
        GeneratedCylindricalSectorCoverageError,
    > {
        let family = session.family();
        let context = session.context();
        let family_fingerprint = family.fingerprint_ref();
        preflight_scope(
            family,
            context,
            family_fingerprint,
            &sector,
            attempts.len(),
            limits,
        )?;
        let (mut stats, unique_sources) = preflight_attempt_payloads(
            family_fingerprint,
            context,
            &sector,
            ordering_policy,
            &attempts,
            limits,
        )?;

        // Cheap core schema/limit/capacity checks retain their precedence over
        // expensive persistent-source replay.
        for attempt in &attempts {
            attempt.preflight_replay(family, context)?;
        }
        session.authenticate_sources_with_table_byte_limits(
            &unique_sources,
            limits.max_replay_session_source_reference_bytes,
            limits.max_replay_session_source_pointer_index_bytes,
        )?;
        stats.replay_session_source_reference_bytes = session.source_reference_bytes()?;
        stats.replay_session_source_pointer_index_bytes = session.source_pointer_index_bytes()?;

        // Every unique exact source allocation is now pinned and fully
        // replayed. Reconstruct every candidate/core locally, including
        // deliberately repeated attempt references.
        if matches!(
            attempt_trust,
            GeneratedCylindricalCoverageAttemptTrust::Persisted
        ) {
            for attempt in &attempts {
                attempt.replay_with_authenticated_session(session)?;
            }
        }

        let coverage_limits = limits.normalization_algebra_limits();
        let normalization_limits = limits.normalization_limits();
        let mut algebra_stats = ParametricSectorCoverageStats::default();
        let mut normalization_stats = ParametricSectorFormulaNormalizationStats::default();
        let mut base_structural_loci = Vec::new();
        let mut normalized_attempts = Vec::new();
        try_reserve_exact(
            "generated cylindrical normalized attempts",
            &mut normalized_attempts,
            attempts.len(),
        )?;
        for (ordinal, attempt) in attempts.iter().enumerate() {
            match attempt {
                GeneratedCylindricalSectorCoverageAttempt::Certified(certificate) => {
                    let body = normalize_candidate_bad_formula(
                        context,
                        certificate.as_ref(),
                        &mut base_structural_loci,
                        &mut algebra_stats,
                        &mut normalization_stats,
                        coverage_limits,
                        normalization_limits,
                    )?;
                    normalized_attempts.push(NormalizedCoverageAttempt::Certified(
                        NormalizedCandidateBadFormula::new(ordinal, body),
                    ));
                }
                GeneratedCylindricalSectorCoverageAttempt::Unsupported(_) => {
                    normalized_attempts.push(NormalizedCoverageAttempt::Unsupported {
                        source_attempt_ordinal: ordinal,
                    });
                }
            }
        }

        census_base_structural_loci(&base_structural_loci, &mut stats, limits)?;
        let normalized_ir = NormalizedCoverageIr::try_new_preallocated(
            base_structural_loci.len(),
            normalized_attempts,
        )
        .map_err(map_formula_ir_error)?;
        let decision =
            ParametricSectorMtbddCompiler::compile(&normalized_ir, limits.mtbdd_limits())
                .map_err(map_mtbdd_error)?;
        census_decision(&decision, &mut stats);

        // Do not retain an arbitrary caller vector capacity. Acquire a fresh,
        // fallibly reserved reference table only after every deep source
        // check has succeeded, then charge its actual allocator capacity.
        let mut retained_attempts = Vec::new();
        try_reserve_exact(
            "retained generated cylindrical coverage attempts",
            &mut retained_attempts,
            attempts.len(),
        )?;
        let retained_reference_bytes = checked_mul(
            "generated cylindrical coverage attempt Arc reference bytes",
            retained_attempts.capacity(),
            size_of::<GeneratedCylindricalSectorCoverageAttempt>(),
        )?;
        check_limit(
            "generated cylindrical coverage attempt Arc reference bytes",
            retained_reference_bytes,
            limits.max_attempt_arc_reference_bytes,
        )?;
        retained_attempts.extend(attempts.iter().cloned());
        stats.attempt_arc_reference_bytes = retained_reference_bytes;
        let context_fingerprint = copy_string(
            context.fingerprint(),
            "generated cylindrical coverage context fingerprint",
        )?;
        let family_fingerprint = copy_string(
            family_fingerprint,
            "generated cylindrical coverage family fingerprint",
        )?;
        Ok(GeneratedCylindricalSectorCoverageCertificate {
            schema: GENERATED_CYLINDRICAL_SECTOR_COVERAGE_V1_SCHEMA,
            family_fingerprint,
            context_fingerprint,
            sector,
            ordering_policy,
            batch_provenance: None,
            attempts: retained_attempts,
            base_structural_loci,
            normalized_ir,
            decision,
            limits,
            stats,
        })
    }
}

fn preflight_scope(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    family_fingerprint: &str,
    sector: &SectorMask,
    attempt_count: usize,
    limits: GeneratedCylindricalSectorCoverageLimits,
) -> Result<(), GeneratedCylindricalSectorCoverageError> {
    validate_family_context(family, context)?;
    check_limit(
        "generated cylindrical coverage family fingerprint bytes",
        family_fingerprint.len(),
        limits.max_family_fingerprint_bytes,
    )?;
    check_limit(
        "generated cylindrical coverage context fingerprint bytes",
        context.fingerprint().len(),
        limits.max_context_fingerprint_bytes,
    )?;
    check_limit(
        "generated cylindrical coverage arity",
        sector.arity(),
        limits.max_arity,
    )?;
    if sector.arity() != context.index_count() {
        return Err(GeneratedCylindricalSectorCoverageError::WrongArity {
            expected: context.index_count(),
            actual: sector.arity(),
        });
    }
    check_limit(
        "generated cylindrical coverage attempts",
        attempt_count,
        limits.max_attempts,
    )?;
    let reference_bytes = checked_mul(
        "generated cylindrical coverage attempt Arc reference bytes",
        attempt_count,
        size_of::<GeneratedCylindricalSectorCoverageAttempt>(),
    )?;
    check_limit(
        "generated cylindrical coverage attempt Arc reference bytes",
        reference_bytes,
        limits.max_attempt_arc_reference_bytes,
    )
}

fn preflight_attempt_payloads<'attempt>(
    family_fingerprint: &str,
    context: &ParametricCoefficientContext,
    sector: &SectorMask,
    ordering_policy: IntegralOrderingPolicy,
    attempts: &'attempt [GeneratedCylindricalSectorCoverageAttempt],
    limits: GeneratedCylindricalSectorCoverageLimits,
) -> Result<
    (
        GeneratedCylindricalSectorCoverageStats,
        Vec<&'attempt Arc<crate::GeneratedCylindricalPersistentEliminationCertificate>>,
    ),
    GeneratedCylindricalSectorCoverageError,
> {
    let mut stats = GeneratedCylindricalSectorCoverageStats {
        attempts: attempts.len(),
        attempt_arc_reference_bytes: checked_mul(
            "generated cylindrical coverage attempt Arc reference bytes",
            attempts.len(),
            size_of::<GeneratedCylindricalSectorCoverageAttempt>(),
        )?,
        ..GeneratedCylindricalSectorCoverageStats::default()
    };
    let pointer_index_minimum_bytes = checked_mul(
        "generated cylindrical persistent-source deduplication pointer-index bytes",
        attempts.len(),
        size_of::<GeneratedCylindricalCoverageSourcePointerIndexEntry>(),
    )?;
    check_limit(
        "generated cylindrical persistent-source deduplication pointer-index bytes",
        pointer_index_minimum_bytes,
        limits.max_persistent_source_dedup_pointer_index_bytes,
    )?;
    let mut pointer_index = Vec::new();
    try_reserve_exact(
        "generated cylindrical persistent-source deduplication pointer-index entries",
        &mut pointer_index,
        attempts.len(),
    )?;
    stats.persistent_source_dedup_pointer_index_bytes = checked_mul(
        "generated cylindrical persistent-source deduplication pointer-index bytes",
        pointer_index.capacity(),
        size_of::<GeneratedCylindricalCoverageSourcePointerIndexEntry>(),
    )?;
    check_limit(
        "generated cylindrical persistent-source deduplication pointer-index bytes",
        stats.persistent_source_dedup_pointer_index_bytes,
        limits.max_persistent_source_dedup_pointer_index_bytes,
    )?;
    for (ordinal, attempt) in attempts.iter().enumerate() {
        let candidate = attempt.candidate();
        if candidate.family_fingerprint() != family_fingerprint {
            return Err(GeneratedCylindricalSectorCoverageError::CandidateWrongFamily { ordinal });
        }
        if candidate.context_fingerprint() != context.fingerprint() {
            return Err(GeneratedCylindricalSectorCoverageError::CandidateWrongContext { ordinal });
        }
        if candidate.sector() != sector {
            return Err(GeneratedCylindricalSectorCoverageError::CandidateWrongSector { ordinal });
        }
        if candidate.ordering_policy() != ordering_policy {
            return Err(
                GeneratedCylindricalSectorCoverageError::CandidateWrongOrdering { ordinal },
            );
        }
        census_attempt_aggregate_payload(&mut stats, attempt, limits)?;
        pointer_index.push(GeneratedCylindricalCoverageSourcePointerIndexEntry {
            address: generated_cylindrical_coverage_source_address(candidate.source()),
            first_attempt_ordinal: ordinal,
        });
    }

    pointer_index.sort_unstable_by(|left, right| {
        left.address
            .cmp(&right.address)
            .then_with(|| left.first_attempt_ordinal.cmp(&right.first_attempt_ordinal))
    });
    for adjacent in pointer_index.windows(2) {
        if adjacent[0].address == adjacent[1].address
            && !Arc::ptr_eq(
                attempts[adjacent[0].first_attempt_ordinal]
                    .candidate()
                    .source(),
                attempts[adjacent[1].first_attempt_ordinal]
                    .candidate()
                    .source(),
            )
        {
            return Err(GeneratedCylindricalSectorCoverageError::ReplayMismatch);
        }
    }
    pointer_index.dedup_by_key(|entry| entry.address);
    check_limit(
        "generated cylindrical unique persistent sources",
        pointer_index.len(),
        limits.max_unique_persistent_sources,
    )?;
    pointer_index.sort_unstable_by_key(|entry| entry.first_attempt_ordinal);

    let mut unique_sources: Vec<
        &'attempt Arc<crate::GeneratedCylindricalPersistentEliminationCertificate>,
    > = Vec::new();
    try_reserve_exact(
        "generated cylindrical persistent-source first-use references",
        &mut unique_sources,
        pointer_index.len(),
    )?;
    check_limit(
        "generated cylindrical persistent-source first-use reference entries",
        unique_sources.capacity(),
        limits.max_unique_persistent_sources,
    )?;
    for entry in pointer_index {
        let source = attempts[entry.first_attempt_ordinal].candidate().source();
        stats.persistent_source_retained_bytes = bounded_add(
            "generated cylindrical persistent-source retained bytes",
            stats.persistent_source_retained_bytes,
            source.stats().certificate_owned_retained_bytes(),
            limits.max_persistent_source_retained_bytes,
        )?;
        unique_sources.push(source);
    }
    stats.unique_persistent_sources = unique_sources.len();
    Ok((stats, unique_sources))
}

fn census_attempt_aggregate_payload(
    stats: &mut GeneratedCylindricalSectorCoverageStats,
    attempt: &GeneratedCylindricalSectorCoverageAttempt,
    limits: GeneratedCylindricalSectorCoverageLimits,
) -> Result<(), GeneratedCylindricalSectorCoverageError> {
    let candidate_stats = attempt.candidate().stats();
    stats.candidate_retained_payload_bytes = bounded_add(
        "generated cylindrical candidate retained payload bytes",
        stats.candidate_retained_payload_bytes,
        candidate_stats.retained_payload_bytes(),
        limits.max_candidate_retained_payload_bytes,
    )?;
    stats.when_bad_binding_bytes = bounded_add(
        "generated cylindrical WhenBad binding bytes",
        stats.when_bad_binding_bytes,
        attempt.binding().retained_bytes(),
        limits.max_when_bad_binding_bytes,
    )?;
    stats.when_bad_retained_core_bytes = bounded_add(
        "generated cylindrical WhenBad retained core bytes",
        stats.when_bad_retained_core_bytes,
        attempt.retained_core_bytes(),
        limits.max_when_bad_retained_core_bytes,
    )?;
    match attempt {
        GeneratedCylindricalSectorCoverageAttempt::Certified(certificate) => {
            stats.certified_attempts = checked_add(
                "generated cylindrical certified attempts",
                stats.certified_attempts,
                1,
            )?;
            let when_bad = certificate.stats();
            stats.when_bad_condition_terms = bounded_add(
                "generated cylindrical WhenBad condition terms",
                stats.when_bad_condition_terms,
                when_bad.retained_condition_terms(),
                limits.max_when_bad_condition_terms,
            )?;
            stats.when_bad_condition_bytes = bounded_add(
                "generated cylindrical WhenBad condition bytes",
                stats.when_bad_condition_bytes,
                when_bad.retained_condition_bytes(),
                limits.max_when_bad_condition_bytes,
            )?;
            stats.when_bad_guard_origin_retained_bytes = bounded_add(
                "generated cylindrical WhenBad guard-origin retained bytes",
                stats.when_bad_guard_origin_retained_bytes,
                when_bad.guard_origin_retained_bytes(),
                limits.max_when_bad_guard_origin_retained_bytes,
            )?;
            stats.when_bad_leak_event_retained_bytes = bounded_add(
                "generated cylindrical WhenBad leak-event retained bytes",
                stats.when_bad_leak_event_retained_bytes,
                when_bad.leak_event_retained_bytes(),
                limits.max_when_bad_leak_event_retained_bytes,
            )?;
        }
        GeneratedCylindricalSectorCoverageAttempt::Unsupported(_) => {
            stats.unsupported_attempts = checked_add(
                "generated cylindrical unsupported attempts",
                stats.unsupported_attempts,
                1,
            )?;
        }
    }
    Ok(())
}

fn census_base_structural_loci(
    loci: &Vec<ParametricPolynomial>,
    stats: &mut GeneratedCylindricalSectorCoverageStats,
    limits: GeneratedCylindricalSectorCoverageLimits,
) -> Result<(), GeneratedCylindricalSectorCoverageError> {
    check_limit(
        "generated cylindrical base structural loci",
        loci.capacity(),
        limits.max_base_structural_loci,
    )?;
    let mut terms = 0usize;
    let mut bytes = checked_mul(
        "generated cylindrical base structural-locus bytes",
        loci.capacity().saturating_sub(loci.len()),
        size_of::<ParametricPolynomial>(),
    )?;
    for polynomial in loci {
        terms = bounded_add(
            "generated cylindrical base structural-locus terms",
            terms,
            polynomial.term_count(),
            limits.max_base_structural_locus_terms,
        )?;
        let owned = polynomial.owned_retained_byte_bound().ok_or(
            GeneratedCylindricalSectorCoverageError::ResourceCountOverflow {
                resource: "generated cylindrical base structural-locus bytes",
            },
        )?;
        bytes = bounded_add(
            "generated cylindrical base structural-locus bytes",
            bytes,
            owned,
            limits.max_base_structural_locus_bytes,
        )?;
    }
    stats.base_structural_loci = loci.len();
    stats.base_structural_locus_terms = terms;
    stats.base_structural_locus_bytes = bytes;
    Ok(())
}

fn census_decision(
    decision: &ParametricSectorMtbddDecisionFunction,
    stats: &mut GeneratedCylindricalSectorCoverageStats,
) {
    let decision_stats = decision.stats();
    stats.normalized_clauses = decision_stats.normalized_clauses;
    stats.normalized_literals = decision_stats.normalized_literals;
    stats.normalized_clause_source_references = decision_stats.clause_source_references;
    stats.normalized_factor_references = decision_stats.factor_references;
    stats.decision_atoms = decision_stats.atoms;
    stats.decision_nodes = decision_stats.rooted_retained.nodes;
    stats.decision_terminals = decision_stats.rooted_retained.terminals;
}

fn copy_sector(sector: &SectorMask) -> Result<SectorMask, GeneratedCylindricalSectorCoverageError> {
    SectorMask::try_new(sector.active_bits().iter().copied()).map_err(|error| match error {
        crate::SectorFoundationError::AllocationFailure {
            resource,
            requested,
        } => GeneratedCylindricalSectorCoverageError::AllocationFailure {
            resource,
            requested,
        },
        _ => GeneratedCylindricalSectorCoverageError::DecisionInvariant,
    })
}

fn copy_string(
    source: &str,
    resource: &'static str,
) -> Result<String, GeneratedCylindricalSectorCoverageError> {
    let mut retained = String::new();
    retained.try_reserve_exact(source.len()).map_err(|_| {
        GeneratedCylindricalSectorCoverageError::AllocationFailure {
            resource,
            requested: source.len(),
        }
    })?;
    retained.push_str(source);
    Ok(retained)
}

fn map_formula_ir_error(
    error: ParametricSectorFormulaIrError,
) -> GeneratedCylindricalSectorCoverageError {
    match error {
        ParametricSectorFormulaIrError::AllocationFailure {
            resource,
            requested,
        } => GeneratedCylindricalSectorCoverageError::AllocationFailure {
            resource,
            requested,
        },
        ParametricSectorFormulaIrError::ResourceCountOverflow { resource } => {
            GeneratedCylindricalSectorCoverageError::ResourceCountOverflow { resource }
        }
        _ => GeneratedCylindricalSectorCoverageError::FormulaInvariant,
    }
}

fn map_mtbdd_error(error: ParametricSectorMtbddError) -> GeneratedCylindricalSectorCoverageError {
    match error {
        ParametricSectorMtbddError::ResourceLimit {
            resource,
            requested,
            limit,
        }
        | ParametricSectorMtbddError::Core(CoverageDecisionDagError::ResourceLimit {
            resource,
            requested,
            limit,
        }) => GeneratedCylindricalSectorCoverageError::ResourceLimit {
            resource,
            requested,
            limit,
        },
        ParametricSectorMtbddError::ResourceCountOverflow { resource }
        | ParametricSectorMtbddError::Core(CoverageDecisionDagError::ResourceCountOverflow {
            resource,
        }) => GeneratedCylindricalSectorCoverageError::ResourceCountOverflow { resource },
        ParametricSectorMtbddError::AllocationFailure {
            resource,
            requested,
        }
        | ParametricSectorMtbddError::Core(CoverageDecisionDagError::AllocationFailure {
            resource,
            requested,
        }) => GeneratedCylindricalSectorCoverageError::AllocationFailure {
            resource,
            requested,
        },
        ParametricSectorMtbddError::FormulaIr(error) => map_formula_ir_error(error),
        _ => GeneratedCylindricalSectorCoverageError::DecisionInvariant,
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedCylindricalSectorCoverageError> {
    left.checked_add(right)
        .ok_or(GeneratedCylindricalSectorCoverageError::ResourceCountOverflow { resource })
}

fn generated_cylindrical_coverage_source_address(
    source: &Arc<crate::GeneratedCylindricalPersistentEliminationCertificate>,
) -> usize {
    Arc::as_ptr(source) as usize
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedCylindricalSectorCoverageError> {
    left.checked_mul(right)
        .ok_or(GeneratedCylindricalSectorCoverageError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedCylindricalSectorCoverageError> {
    if requested > limit {
        Err(GeneratedCylindricalSectorCoverageError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, GeneratedCylindricalSectorCoverageError> {
    let requested = checked_add(resource, left, right)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn try_reserve_exact<T>(
    resource: &'static str,
    values: &mut Vec<T>,
    additional: usize,
) -> Result<(), GeneratedCylindricalSectorCoverageError> {
    values.try_reserve_exact(additional).map_err(|_| {
        GeneratedCylindricalSectorCoverageError::AllocationFailure {
            resource,
            requested: additional,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated_cylindrical_candidate_authority::{
        GeneratedCylindricalReplaySession,
        authenticated_candidate_local_reconstruction_count_for_test,
        operation_scoped_persistent_source_replay_count_for_test,
        reset_authenticated_candidate_local_reconstruction_count_for_test,
        reset_operation_scoped_persistent_source_replay_count_for_test,
    };
    use crate::when_bad::{
        replayed_cylindrical_core_construction_count_for_test,
        reset_replayed_cylindrical_core_construction_count_for_test,
    };
    use crate::{
        AffineDenominator, FamilySectorInventoryCompiler, FamilySectorInventoryLimits,
        GeneratedCylindricalCandidateAuthority, GeneratedCylindricalCandidateAuthorityLimits,
        GeneratedCylindricalPersistentEliminationCertificate,
        GeneratedCylindricalPersistentEliminationLimits, GeneratedCylindricalRowSystemCertificate,
        GeneratedCylindricalRowSystemLimits, GeneratedCylindricalSectorRootStartCertificate,
        GeneratedCylindricalSectorRootStartLimits, GeneratedCylindricalWhenBadCompilation,
        GeneratedCylindricalWhenBadCompiler, GeneratedSymbolicRowSpanConfig, ParametricIbpConfig,
        ParametricIbpGenerator, PowerShiftPolicy, SectorRestrictions, WhenBadCompilerLimits,
        algebra::CoefficientContext,
    };

    // Exact replay-work ceilings used by the authentic one-loop persistent
    // fixture. The lower-level persistent suite separately proves the exact
    // and one-below behavior of these work fields.
    const FIXTURE_MAX_PREFIX_CONSTRUCTION_INTEGER_BIT_WORK: usize = 129_362_930_506_106_837;
    const FIXTURE_MAX_PREFIX_REPLAY_INTEGER_BIT_WORK: usize = 341_650_130_121_813_484;
    const FIXTURE_CUMULATIVE_CONSTRUCTION_INTEGER_BIT_WORK: usize = 132_091_743_156_607_887;
    const FIXTURE_CUMULATIVE_REPLAY_INTEGER_BIT_WORK: usize = 415_859_729_172_547_172;

    fn massive_tadpole(name: &str) -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        IntegralFamily::new(
            name,
            vec!["ell".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                coefficients.parse("-m2").unwrap(),
                vec![coefficients.one()],
            )],
            Vec::new(),
            vec![coefficients.zero()],
        )
        .unwrap()
    }

    fn tadpole_persistent_source(
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedCylindricalPersistentEliminationCertificate>,
    ) {
        let family = massive_tadpole(name);
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let sector = SectorMask::try_new([true]).unwrap();
        let inventory = Arc::new(
            FamilySectorInventoryCompiler::compile(
                &family,
                SectorRestrictions::unrestricted(family.denominator_count()).unwrap(),
                PowerShiftPolicy::FormalGeneric,
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                FamilySectorInventoryLimits::default(),
            )
            .unwrap(),
        );
        let root = Arc::new(
            GeneratedCylindricalSectorRootStartCertificate::compile(
                &family,
                &context,
                inventory,
                sector,
                ParametricIbpConfig::default(),
                GeneratedSymbolicRowSpanConfig::default(),
                1,
                GeneratedCylindricalSectorRootStartLimits::default(),
            )
            .unwrap(),
        );
        let rows = Arc::new(
            GeneratedCylindricalRowSystemCertificate::compile_from_sector_root(
                &family,
                &context,
                root,
                GeneratedCylindricalRowSystemLimits::default(),
            )
            .unwrap(),
        );
        let mut persistent_limits = GeneratedCylindricalPersistentEliminationLimits::default();
        persistent_limits
            .elimination
            .max_replay_coefficient_integer_bit_work = FIXTURE_MAX_PREFIX_REPLAY_INTEGER_BIT_WORK;
        persistent_limits
            .elimination
            .max_construction_coefficient_integer_bit_work =
            FIXTURE_MAX_PREFIX_CONSTRUCTION_INTEGER_BIT_WORK;
        persistent_limits.max_cumulative_construction_coefficient_integer_bit_work =
            FIXTURE_CUMULATIVE_CONSTRUCTION_INTEGER_BIT_WORK;
        persistent_limits.max_cumulative_replay_coefficient_integer_bit_work =
            FIXTURE_CUMULATIVE_REPLAY_INTEGER_BIT_WORK;
        let source = Arc::new(
            GeneratedCylindricalPersistentEliminationCertificate::compile(
                &family,
                &context,
                rows,
                persistent_limits,
            )
            .unwrap(),
        );
        (family, context, source)
    }

    fn certified_tadpole_attempt(
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedCylindricalWhenBadCertificate>,
    ) {
        let (family, context, source) = tadpole_persistent_source(name);
        let certificate = certified_tadpole_attempt_from_source(&family, &context, source);
        (family, context, certificate)
    }

    fn certified_tadpole_attempt_from_source(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source: Arc<GeneratedCylindricalPersistentEliminationCertificate>,
    ) -> Arc<GeneratedCylindricalWhenBadCertificate> {
        let pivot_ordinal = source
            .guarded_pivots()
            .find(|pivot| pivot.original_pivot().values() == [1])
            .or_else(|| source.guarded_pivots().next())
            .expect("the tadpole fixture must retain a forward pivot")
            .ordinal();
        let authority = GeneratedCylindricalCandidateAuthority::compile(
            family,
            context,
            Arc::clone(&source),
            pivot_ordinal,
            GeneratedCylindricalCandidateAuthorityLimits::default(),
        )
        .unwrap();
        let global = match &authority {
            GeneratedCylindricalCandidateAuthority::Global(global) => global,
            GeneratedCylindricalCandidateAuthority::LocusBound(_) => {
                panic!("the sector-root tadpole pivot must be Global")
            }
        };
        let compilation = GeneratedCylindricalWhenBadCompiler::compile(
            family,
            context,
            global,
            WhenBadCompilerLimits::default(),
        )
        .unwrap();
        let GeneratedCylindricalWhenBadCompilation::Certified(certificate) = compilation else {
            panic!("the tadpole forward recurrence must be certified")
        };
        Arc::new(certificate)
    }

    #[test]
    fn empty_authenticated_cover_is_explicitly_uncovered_and_replays() {
        let family = massive_tadpole("generated-cylindrical-coverage-empty");
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let coverage = GeneratedCylindricalSectorCoverageCompiler::compile_authenticated(
            &family,
            &context,
            SectorMask::try_new([true]).unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            Vec::new(),
            GeneratedCylindricalSectorCoverageLimits::default(),
        )
        .unwrap();

        assert_eq!(coverage.stats().attempts(), 0);
        assert!(coverage.candidate_attempts().is_empty());
        assert!(matches!(
            coverage.classification_for_indices(&context, &[2]).unwrap(),
            Some(GeneratedCylindricalSectorLeafDisposition::Uncovered)
        ));
        assert!(
            coverage
                .classification_for_indices(&context, &[0])
                .unwrap()
                .is_none()
        );
        coverage.replay(&family, &context).unwrap();
    }

    #[test]
    fn authenticated_cover_rejects_corrupted_when_bad_schema_and_limits() {
        let (family, context, candidate) =
            certified_tadpole_attempt("generated-cylindrical-coverage-corrupt-attempt");
        let compile = |compilation| {
            let attempt = match compilation {
                GeneratedCylindricalWhenBadCompilation::Certified(certificate) => {
                    GeneratedCylindricalSectorCoverageAttempt::certified(Arc::new(certificate))
                }
                GeneratedCylindricalWhenBadCompilation::Unsupported(unsupported) => {
                    GeneratedCylindricalSectorCoverageAttempt::unsupported(Arc::new(unsupported))
                }
            };
            GeneratedCylindricalSectorCoverageCompiler::compile_authenticated(
                &family,
                &context,
                SectorMask::try_new([true]).unwrap(),
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                vec![attempt],
                GeneratedCylindricalSectorCoverageLimits::default(),
            )
        };
        let compilation =
            GeneratedCylindricalWhenBadCompilation::Certified(candidate.as_ref().clone());

        let mut corrupt_schema = compilation.clone();
        corrupt_schema.corrupt_schema_for_test();
        assert!(matches!(
            compile(corrupt_schema),
            Err(GeneratedCylindricalSectorCoverageError::WhenBad(error))
                if matches!(error.as_ref(), WhenBadCompilerError::SchemaMismatch)
        ));

        let mut corrupt_limits = compilation;
        corrupt_limits.corrupt_limits_for_test();
        assert!(matches!(
            compile(corrupt_limits),
            Err(GeneratedCylindricalSectorCoverageError::WhenBad(error))
                if matches!(error.as_ref(), WhenBadCompilerError::ReplayMismatch)
        ));
    }

    #[test]
    fn tadpole_coverage_selects_i2_rejects_boundary_i1_and_replays() {
        let (family, context, candidate) =
            certified_tadpole_attempt("generated-cylindrical-coverage-tadpole");
        let sector = SectorMask::try_new([true]).unwrap();
        let coverage = GeneratedCylindricalSectorCoverageCompiler::compile_authenticated(
            &family,
            &context,
            sector.clone(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            vec![GeneratedCylindricalSectorCoverageAttempt::certified(
                Arc::clone(&candidate),
            )],
            GeneratedCylindricalSectorCoverageLimits::default(),
        )
        .unwrap();

        assert_eq!(
            coverage.schema(),
            GENERATED_CYLINDRICAL_SECTOR_COVERAGE_V1_SCHEMA
        );
        assert_eq!(coverage.sector(), &sector);
        assert_eq!(
            coverage.ordering_policy(),
            IntegralOrderingPolicy::RustRedUnshiftedV1
        );
        assert_eq!(coverage.stats().attempts(), 1);
        assert_eq!(
            coverage.stats().attempt_arc_reference_bytes(),
            size_of::<GeneratedCylindricalSectorCoverageAttempt>()
        );
        assert_eq!(coverage.stats().certified_attempts(), 1);
        assert_eq!(coverage.stats().unsupported_attempts(), 0);
        assert_eq!(coverage.stats().unique_persistent_sources(), 1);
        assert_eq!(
            coverage.stats().when_bad_retained_core_bytes(),
            candidate.retained_core_bytes()
        );
        assert_eq!(
            coverage.stats().when_bad_guard_origin_retained_bytes(),
            candidate.stats().guard_origin_retained_bytes()
        );
        assert!(Arc::ptr_eq(
            coverage.candidate_attempts()[0]
                .certified_arc()
                .expect("attempt zero is certified"),
            &candidate,
        ));
        coverage.replay(&family, &context).unwrap();

        let local_i2 = candidate
            .classification_for_indices(&context, &[2])
            .unwrap()
            .expect("I(2) lies in the active orthant");
        assert_eq!(
            local_i2.disposition(),
            &WhenBadLeafDisposition::CoveredByCandidate
        );
        let i2 = coverage
            .classification_for_indices(&context, &[2])
            .unwrap()
            .expect("I(2) lies in the covered orthant");
        assert_eq!(i2.candidate_ordinal(), Some(0));
        assert!(Arc::ptr_eq(
            &i2.selected_candidate_arc()
                .expect("I(2) selects the candidate"),
            &candidate,
        ));

        let reduction = crate::ConcreteReduction::apply_generated_cylindrical(
            i2.selected_candidate_arc().unwrap(),
            &context,
            &[2],
        )
        .unwrap();
        assert_eq!(reduction.rhs().len(), 1);
        assert_eq!(
            reduction.rhs().get(
                &crate::ConcreteIntegralKey::try_new([1]).expect("one-dimensional integral key")
            ),
            Some(&context.base().parse("(d-2)/(2*m2)").unwrap())
        );

        let local_i1 = candidate
            .classification_for_indices(&context, &[1])
            .unwrap()
            .expect("I(1) lies in the active orthant");
        assert!(!matches!(
            local_i1.disposition(),
            WhenBadLeafDisposition::CoveredByCandidate
        ));
        assert!(matches!(
            coverage.classification_for_indices(&context, &[1]).unwrap(),
            Some(GeneratedCylindricalSectorLeafDisposition::Uncovered)
        ));
        assert!(
            coverage
                .classification_for_indices(&context, &[0])
                .unwrap()
                .is_none()
        );

        let mut too_small = GeneratedCylindricalSectorCoverageLimits::default();
        too_small.max_attempts = 0;
        assert!(matches!(
            GeneratedCylindricalSectorCoverageCompiler::compile_authenticated(
                &family,
                &context,
                sector,
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                vec![GeneratedCylindricalSectorCoverageAttempt::certified(
                    candidate
                )],
                too_small,
            ),
            Err(GeneratedCylindricalSectorCoverageError::ResourceLimit {
                resource: "generated cylindrical coverage attempts",
                requested: 1,
                limit: 0,
            })
        ));
    }

    #[test]
    fn exhaustive_persistent_batch_replays_its_exact_source_once_per_public_operation() {
        let (family, context, source) =
            tadpole_persistent_source("generated-cylindrical-coverage-exhaustive-batch");
        let pivot_count = source.guarded_pivots().len();
        assert!(pivot_count > 0);

        reset_operation_scoped_persistent_source_replay_count_for_test();
        reset_authenticated_candidate_local_reconstruction_count_for_test();
        reset_replayed_cylindrical_core_construction_count_for_test();
        let coverage = GeneratedCylindricalSectorCoverageCompiler::compile_from_persistent_source(
            &family,
            &context,
            Arc::clone(&source),
            GeneratedCylindricalCandidateAuthorityLimits::default(),
            WhenBadCompilerLimits::default(),
            GeneratedCylindricalSectorCoverageLimits::default(),
        )
        .unwrap();
        assert_eq!(
            operation_scoped_persistent_source_replay_count_for_test(),
            1
        );
        assert_eq!(
            authenticated_candidate_local_reconstruction_count_for_test(),
            2 * pivot_count,
            "each batch pivot needs one fresh candidate pass and one local replay pass"
        );
        assert_eq!(
            replayed_cylindrical_core_construction_count_for_test(),
            2 * pivot_count,
            "each sealed replayed candidate needs independently equal W1/W2 core passes"
        );
        assert_eq!(
            coverage.schema(),
            GENERATED_CYLINDRICAL_SECTOR_COVERAGE_V2_SCHEMA
        );
        assert_eq!(coverage.candidate_attempts().len(), pivot_count);
        assert_eq!(coverage.stats().unique_persistent_sources(), 1);
        assert!(Arc::ptr_eq(
            coverage
                .batch_provenance()
                .expect("V2 batch provenance")
                .source(),
            &source,
        ));
        for (ordinal, attempt) in coverage.candidate_attempts().iter().enumerate() {
            assert_eq!(attempt.candidate().pivot_ordinal(), ordinal);
            assert!(Arc::ptr_eq(attempt.candidate().source(), &source));
        }

        reset_operation_scoped_persistent_source_replay_count_for_test();
        reset_authenticated_candidate_local_reconstruction_count_for_test();
        reset_replayed_cylindrical_core_construction_count_for_test();
        coverage.replay(&family, &context).unwrap();
        assert_eq!(
            operation_scoped_persistent_source_replay_count_for_test(),
            1,
            "public V2 replay must create a fresh non-sticky source session"
        );
        assert_eq!(
            authenticated_candidate_local_reconstruction_count_for_test(),
            2 * pivot_count
        );
        assert_eq!(
            replayed_cylindrical_core_construction_count_for_test(),
            2 * pivot_count
        );
    }

    #[test]
    fn exhaustive_batch_charges_aggregate_payload_before_retaining_each_attempt() {
        let (family, context, source) = tadpole_persistent_source(
            "generated-cylindrical-coverage-exhaustive-aggregate-payload",
        );
        let pivot_count = source.guarded_pivots().len();
        assert!(pivot_count > 0);

        reset_exhaustive_attempts_retained_for_test();
        let baseline = GeneratedCylindricalSectorCoverageCompiler::compile_from_persistent_source(
            &family,
            &context,
            Arc::clone(&source),
            GeneratedCylindricalCandidateAuthorityLimits::default(),
            WhenBadCompilerLimits::default(),
            GeneratedCylindricalSectorCoverageLimits::default(),
        )
        .unwrap();
        assert_eq!(exhaustive_attempts_retained_for_test(), pivot_count);
        let exact_stats = baseline.stats();
        assert!(exact_stats.candidate_retained_payload_bytes() > 0);
        assert!(exact_stats.when_bad_retained_core_bytes() > 0);

        let mut exact = GeneratedCylindricalSectorCoverageLimits::default();
        exact.max_candidate_retained_payload_bytes = exact_stats.candidate_retained_payload_bytes();
        exact.max_when_bad_binding_bytes = exact_stats.when_bad_binding_bytes();
        exact.max_when_bad_retained_core_bytes = exact_stats.when_bad_retained_core_bytes();
        exact.max_when_bad_condition_terms = exact_stats.when_bad_condition_terms();
        exact.max_when_bad_condition_bytes = exact_stats.when_bad_condition_bytes();
        exact.max_when_bad_guard_origin_retained_bytes =
            exact_stats.when_bad_guard_origin_retained_bytes();
        exact.max_when_bad_leak_event_retained_bytes =
            exact_stats.when_bad_leak_event_retained_bytes();

        reset_exhaustive_attempts_retained_for_test();
        let exact_coverage =
            GeneratedCylindricalSectorCoverageCompiler::compile_from_persistent_source(
                &family,
                &context,
                Arc::clone(&source),
                GeneratedCylindricalCandidateAuthorityLimits::default(),
                WhenBadCompilerLimits::default(),
                exact,
            )
            .unwrap();
        assert_eq!(exact_coverage.stats(), exact_stats);
        assert_eq!(exhaustive_attempts_retained_for_test(), pivot_count);

        let mut candidate_one_below = exact;
        candidate_one_below.max_candidate_retained_payload_bytes =
            exact_stats.candidate_retained_payload_bytes() - 1;
        reset_exhaustive_attempts_retained_for_test();
        assert!(matches!(
            GeneratedCylindricalSectorCoverageCompiler::compile_from_persistent_source(
                &family,
                &context,
                Arc::clone(&source),
                GeneratedCylindricalCandidateAuthorityLimits::default(),
                WhenBadCompilerLimits::default(),
                candidate_one_below,
            ),
            Err(GeneratedCylindricalSectorCoverageError::ResourceLimit {
                resource: "generated cylindrical candidate retained payload bytes",
                requested,
                limit,
            }) if requested == exact_stats.candidate_retained_payload_bytes()
                && limit + 1 == requested
        ));
        assert_eq!(
            exhaustive_attempts_retained_for_test(),
            pivot_count - 1,
            "the overflowing candidate must not enter the retained attempt vector"
        );

        let mut core_one_below = exact;
        core_one_below.max_when_bad_retained_core_bytes =
            exact_stats.when_bad_retained_core_bytes() - 1;
        reset_exhaustive_attempts_retained_for_test();
        assert!(matches!(
            GeneratedCylindricalSectorCoverageCompiler::compile_from_persistent_source(
                &family,
                &context,
                source,
                GeneratedCylindricalCandidateAuthorityLimits::default(),
                WhenBadCompilerLimits::default(),
                core_one_below,
            ),
            Err(GeneratedCylindricalSectorCoverageError::ResourceLimit {
                resource: "generated cylindrical WhenBad retained core bytes",
                requested,
                limit,
            }) if requested == exact_stats.when_bad_retained_core_bytes()
                && limit + 1 == requested
        ));
        assert_eq!(
            exhaustive_attempts_retained_for_test(),
            pivot_count - 1,
            "the overflowing WhenBad core must not enter the retained attempt vector"
        );
    }

    #[test]
    fn coverage_pointer_indexes_are_separately_bounded_exactly_and_one_below() {
        let (family, context, candidate) =
            certified_tadpole_attempt("generated-cylindrical-coverage-pointer-index-limits");
        let attempts = || {
            vec![
                GeneratedCylindricalSectorCoverageAttempt::certified(Arc::clone(&candidate)),
                GeneratedCylindricalSectorCoverageAttempt::certified(Arc::clone(&candidate)),
            ]
        };
        let sector = SectorMask::try_new([true]).unwrap();

        let baseline = GeneratedCylindricalSectorCoverageCompiler::compile_authenticated(
            &family,
            &context,
            sector.clone(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            attempts(),
            GeneratedCylindricalSectorCoverageLimits::default(),
        )
        .unwrap();
        let exact_dedup_index_bytes = baseline
            .stats()
            .persistent_source_dedup_pointer_index_bytes();
        let exact_session_pointer_index_bytes =
            baseline.stats().replay_session_source_pointer_index_bytes();
        assert!(exact_dedup_index_bytes > 0);
        assert!(exact_session_pointer_index_bytes > 0);
        assert_eq!(baseline.stats().unique_persistent_sources(), 1);

        let mut exact = GeneratedCylindricalSectorCoverageLimits::default();
        exact.max_persistent_source_dedup_pointer_index_bytes = exact_dedup_index_bytes;
        exact.max_replay_session_source_pointer_index_bytes = exact_session_pointer_index_bytes;
        reset_operation_scoped_persistent_source_replay_count_for_test();
        let exact_coverage = GeneratedCylindricalSectorCoverageCompiler::compile_authenticated(
            &family,
            &context,
            sector.clone(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            attempts(),
            exact,
        )
        .unwrap();
        assert_eq!(
            exact_coverage
                .stats()
                .persistent_source_dedup_pointer_index_bytes(),
            exact_dedup_index_bytes
        );
        assert_eq!(
            exact_coverage
                .stats()
                .replay_session_source_pointer_index_bytes(),
            exact_session_pointer_index_bytes
        );
        assert_eq!(
            operation_scoped_persistent_source_replay_count_for_test(),
            1
        );

        let mut dedup_one_below = exact;
        dedup_one_below.max_persistent_source_dedup_pointer_index_bytes =
            exact_dedup_index_bytes - 1;
        reset_operation_scoped_persistent_source_replay_count_for_test();
        assert!(matches!(
            GeneratedCylindricalSectorCoverageCompiler::compile_authenticated(
                &family,
                &context,
                sector.clone(),
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                attempts(),
                dedup_one_below,
            ),
            Err(GeneratedCylindricalSectorCoverageError::ResourceLimit {
                resource: "generated cylindrical persistent-source deduplication pointer-index bytes",
                requested,
                limit,
            }) if requested == exact_dedup_index_bytes && limit + 1 == requested
        ));
        assert_eq!(
            operation_scoped_persistent_source_replay_count_for_test(),
            0
        );

        let mut session_one_below = exact;
        session_one_below.max_replay_session_source_pointer_index_bytes =
            exact_session_pointer_index_bytes - 1;
        reset_operation_scoped_persistent_source_replay_count_for_test();
        assert!(matches!(
            GeneratedCylindricalSectorCoverageCompiler::compile_authenticated(
                &family,
                &context,
                sector,
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                attempts(),
                session_one_below,
            ),
            Err(GeneratedCylindricalSectorCoverageError::Candidate(error))
                if matches!(
                    error.as_ref(),
                    GeneratedCylindricalCandidateAuthorityError::ResourceLimit {
                        resource: "operation-scoped persistent-source pointer-index bytes",
                        requested,
                        limit,
                    } if *requested == exact_session_pointer_index_bytes
                        && *limit + 1 == *requested
                )
        ));
        assert_eq!(
            operation_scoped_persistent_source_replay_count_for_test(),
            0
        );
    }

    #[test]
    fn repeated_attempt_arc_replays_one_exact_source_and_preserves_both_ordinals() {
        let (family, context, candidate) =
            certified_tadpole_attempt("generated-cylindrical-coverage-repeated-attempt");
        reset_operation_scoped_persistent_source_replay_count_for_test();
        let coverage = GeneratedCylindricalSectorCoverageCompiler::compile_authenticated(
            &family,
            &context,
            SectorMask::try_new([true]).unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            vec![
                GeneratedCylindricalSectorCoverageAttempt::certified(Arc::clone(&candidate)),
                GeneratedCylindricalSectorCoverageAttempt::certified(Arc::clone(&candidate)),
            ],
            GeneratedCylindricalSectorCoverageLimits::default(),
        )
        .unwrap();
        assert_eq!(
            operation_scoped_persistent_source_replay_count_for_test(),
            1
        );
        assert_eq!(coverage.stats().attempts(), 2);
        assert_eq!(coverage.stats().unique_persistent_sources(), 1);
        assert!(Arc::ptr_eq(
            coverage.candidate_attempts()[0].certified_arc().unwrap(),
            &candidate,
        ));
        assert!(Arc::ptr_eq(
            coverage.candidate_attempts()[1].certified_arc().unwrap(),
            &candidate,
        ));

        let required_reference_bytes = coverage.stats().replay_session_source_reference_bytes();
        assert!(required_reference_bytes > 0);
        let mut one_below = GeneratedCylindricalSectorCoverageLimits::default();
        one_below.max_replay_session_source_reference_bytes = required_reference_bytes - 1;
        reset_operation_scoped_persistent_source_replay_count_for_test();
        assert!(matches!(
            GeneratedCylindricalSectorCoverageCompiler::compile_authenticated(
                &family,
                &context,
                SectorMask::try_new([true]).unwrap(),
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                vec![GeneratedCylindricalSectorCoverageAttempt::certified(
                    candidate,
                )],
                one_below,
            ),
            Err(GeneratedCylindricalSectorCoverageError::Candidate(error))
                if matches!(
                    error.as_ref(),
                    GeneratedCylindricalCandidateAuthorityError::ResourceLimit {
                        resource: "operation-scoped replayed persistent-source reference bytes",
                        requested,
                        limit,
                    } if *requested == required_reference_bytes && *limit + 1 == *requested
                )
        ));
        assert_eq!(
            operation_scoped_persistent_source_replay_count_for_test(),
            0,
            "session-table byte admission must precede persistent replay"
        );
    }

    #[test]
    fn payload_equal_distinct_persistent_source_arcs_replay_separately() {
        let (family, context, source) =
            tadpole_persistent_source("generated-cylindrical-coverage-distinct-source-arcs");
        let distinct_source = Arc::new(source.as_ref().clone());
        assert!(!Arc::ptr_eq(&source, &distinct_source));
        assert!(source.payload_eq(&distinct_source));
        let first = certified_tadpole_attempt_from_source(&family, &context, Arc::clone(&source));
        let second =
            certified_tadpole_attempt_from_source(&family, &context, Arc::clone(&distinct_source));

        let mut first_source_only_session =
            GeneratedCylindricalReplaySession::new(&family, &context);
        first_source_only_session
            .authenticate_source(&source)
            .unwrap();
        assert!(matches!(
            second
                .candidate()
                .replay_with_authenticated_session(&first_source_only_session),
            Err(
                GeneratedCylindricalCandidateAuthorityError::ReplayMismatch {
                    detail: "exact persistent-source allocation was not replayed in this operation",
                }
            )
        ));

        reset_operation_scoped_persistent_source_replay_count_for_test();
        let coverage = GeneratedCylindricalSectorCoverageCompiler::compile_authenticated(
            &family,
            &context,
            SectorMask::try_new([true]).unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            vec![
                GeneratedCylindricalSectorCoverageAttempt::certified(first),
                GeneratedCylindricalSectorCoverageAttempt::certified(second),
            ],
            GeneratedCylindricalSectorCoverageLimits::default(),
        )
        .unwrap();
        assert_eq!(
            operation_scoped_persistent_source_replay_count_for_test(),
            2
        );
        assert_eq!(coverage.stats().unique_persistent_sources(), 2);
    }
}
