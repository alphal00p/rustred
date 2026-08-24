//! Proof-bearing authority for split-recentered cylindrical pivots.
//!
//! This module is the topology-neutral boundary between authenticated
//! cylindrical elimination and generated `WhenBad`.  It deliberately does
//! not publish a reduction rule: a centered row still needs its generated
//! applicability proof.  Empty source assignments produce global candidates;
//! nonempty assignments remain bound to their exactly translated equality
//! locus.

use std::fmt;
use std::fmt::Write as _;
use std::mem::{align_of, size_of};
use std::sync::Arc;

#[cfg(test)]
use std::cell::Cell;

use crate::exact_identity::{
    ExactIdentityError, ExactIdentityLimits, ExactIdentityPayload, ExactIdentityStats,
    ExactIdentityWriter, ExactStructuralIdentity, encode_exact_identity,
};
use crate::generated_cylindrical_persistent_elimination::GENERATED_CYLINDRICAL_PERSISTENT_ELIMINATION_V3_SCHEMA;
use crate::generated_cylindrical_residual_start::GENERATED_CYLINDRICAL_RESIDUAL_START_V1_SCHEMA;
use crate::generated_cylindrical_sector_root_start::GENERATED_CYLINDRICAL_SECTOR_ROOT_START_V1_SCHEMA;
use crate::parametric_relation::{
    ParametricAffineFreeRecenteringLimits, ParametricAffineFreeRecenteringStats,
};
use crate::{
    ConcreteRelation, GeneratedCylindricalPersistentEliminationCertificate,
    GeneratedCylindricalPersistentEliminationError, GeneratedCylindricalPersistentEliminationEvent,
    GeneratedCylindricalPersistentGuardedPivot, GeneratedCylindricalPersistentPivotBaseAssumptions,
    GeneratedCylindricalRowSystemStartCertificate, GeneratedSymbolicRowSpanCertificate, IndexShift,
    IntegralFamily, IntegralOrderingPolicy, ParametricArithmeticLimits,
    ParametricCoefficientContext, ParametricRelation, ParametricRelationError, ParametricRowId,
    SectorMask,
};

/// Stable schema for the first guarded cylindrical candidate authority.
pub const GENERATED_CYLINDRICAL_CANDIDATE_AUTHORITY_V1_SCHEMA: &str =
    "rustred-generated-cylindrical-candidate-authority-v1";

const CANDIDATE_PREFLIGHT_COMPLETE_DETAIL: &str = "internal candidate shallow preflight completed";

/// One operation-scoped authentication cache for immutable persistent
/// cylindrical sources.
///
/// This type is deliberately crate-private and is never retained by a public
/// certificate.  Exact `Arc` allocations, rather than fingerprints or deep
/// payload equality, are the cache keys.  Each strong reference pins its
/// allocation for the complete operation, so pointer identity cannot be
/// recycled while a capability exists.
pub(crate) struct GeneratedCylindricalReplaySession<'scope> {
    family: &'scope IntegralFamily,
    context: &'scope ParametricCoefficientContext,
    replayed_sources: Vec<Arc<GeneratedCylindricalPersistentEliminationCertificate>>,
    replayed_source_pointer_index: Vec<GeneratedCylindricalReplayedSourcePointerIndexEntry>,
}

/// Sorted exact-allocation lookup for the strong-`Arc` table above. The
/// strong references pin every indexed allocation, so an address cannot be
/// recycled while the entry is live. Lookup still confirms `Arc::ptr_eq`
/// before issuing a replay capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GeneratedCylindricalReplayedSourcePointerIndexEntry {
    address: usize,
    source_ordinal: usize,
}

/// Transient batch index used to sort/deduplicate exact input allocations in
/// O(n log n) time while retaining their first input occurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GeneratedCylindricalIncomingSourcePointerIndexEntry {
    address: usize,
    input_ordinal: usize,
}

/// Sealed proof that one exact persistent-source allocation has completed its
/// full public replay in the enclosing operation.
#[derive(Clone, Copy)]
pub(crate) struct ReplayedGeneratedCylindricalPersistentSource<'session, 'scope> {
    session: &'session GeneratedCylindricalReplaySession<'scope>,
    source_ordinal: usize,
}

/// Sealed proof that one exact Global candidate has been reconstructed and
/// locally compared after its exact persistent source was replayed.
#[derive(Clone, Copy)]
pub(crate) struct ReplayedGeneratedCylindricalGlobalCandidate<'candidate, 'session, 'scope> {
    candidate: &'candidate GeneratedCylindricalGlobalCandidateAuthority,
    source: ReplayedGeneratedCylindricalPersistentSource<'session, 'scope>,
}

#[cfg(test)]
thread_local! {
    static OPERATION_SCOPED_PERSISTENT_SOURCE_REPLAYS: Cell<usize> = const { Cell::new(0) };
    static AUTHENTICATED_CANDIDATE_LOCAL_RECONSTRUCTIONS: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_operation_scoped_persistent_source_replay_count_for_test() {
    OPERATION_SCOPED_PERSISTENT_SOURCE_REPLAYS.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn operation_scoped_persistent_source_replay_count_for_test() -> usize {
    OPERATION_SCOPED_PERSISTENT_SOURCE_REPLAYS.with(Cell::get)
}

#[cfg(test)]
fn record_operation_scoped_persistent_source_replay_for_test() {
    OPERATION_SCOPED_PERSISTENT_SOURCE_REPLAYS.with(|count| {
        count.set(count.get().saturating_add(1));
    });
}

#[cfg(test)]
pub(crate) fn reset_authenticated_candidate_local_reconstruction_count_for_test() {
    AUTHENTICATED_CANDIDATE_LOCAL_RECONSTRUCTIONS.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn authenticated_candidate_local_reconstruction_count_for_test() -> usize {
    AUTHENTICATED_CANDIDATE_LOCAL_RECONSTRUCTIONS.with(Cell::get)
}

#[cfg(test)]
fn record_authenticated_candidate_local_reconstruction_for_test() {
    AUTHENTICATED_CANDIDATE_LOCAL_RECONSTRUCTIONS.with(|count| {
        count.set(count.get().saturating_add(1));
    });
}

/// Limits for one `compile_inner` pass.
///
/// Public [`GeneratedCylindricalCandidateAuthority::compile`] fully replays
/// its persistent source once, then performs two independently bounded local
/// candidate passes: construction and reconstruction. Recenter counters are
/// conservative Symbolica preflight envelopes unless their lower-level type
/// documents an exact count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedCylindricalCandidateAuthorityLimits {
    pub arithmetic: ParametricArithmeticLimits,
    pub max_candidates: usize,
    pub max_family_fingerprint_bytes: usize,
    pub max_context_fingerprint_bytes: usize,
    pub max_ordering_identity_bytes: usize,
    pub max_arity: usize,
    pub max_pivot_components: usize,
    pub max_pivot_integer_bit_work: usize,
    pub max_dependency_events: usize,
    pub max_base_assumption_references: usize,
    pub max_base_assumption_origin_references: usize,
    pub max_base_assumption_manifest_bytes: usize,
    pub max_base_assumption_condition_owned_bytes: usize,
    pub max_centered_assignment_entries: usize,
    pub max_centered_assignment_additions: usize,
    pub max_centered_assignment_integer_bit_work: usize,
    pub max_row_label_bytes: usize,
    pub max_centered_rhs_terms: usize,
    pub max_recenter_attempts: usize,
    pub max_recenter_terms: usize,
    pub max_recenter_guards: usize,
    pub max_recenter_translation_components: usize,
    pub max_recenter_key_subtraction_boundary_checks: usize,
    pub max_recenter_source_terms: usize,
    pub max_recenter_source_exponent_entries: usize,
    pub max_recenter_output_terms: usize,
    pub max_recenter_output_exponent_entries: usize,
    pub max_recenter_power_operations: usize,
    pub max_recenter_integer_bit_work: usize,
    pub max_recenter_normalized_coefficient_terms: usize,
    pub max_recenter_retained_bytes: usize,
    pub max_retained_payload_bytes: usize,
    /// Local binding comparison only. Nested persistent-source equality is
    /// bounded and replayed by the retained source certificate itself.
    pub max_local_replay_comparison_units: usize,
    pub max_local_replay_comparison_bytes: usize,
    pub max_exact_identity_bytes: usize,
    pub max_exact_identity_fields: usize,
    pub max_exact_identity_tag_bytes: usize,
    pub max_exact_identity_string_values: usize,
    pub max_exact_identity_string_bytes: usize,
    pub max_exact_identity_nesting_depth: usize,
    pub max_exact_identity_polynomials: usize,
    pub max_exact_identity_polynomial_variables: usize,
    pub max_exact_identity_polynomial_terms: usize,
    pub max_exact_identity_exponent_entries: usize,
    pub max_exact_identity_integers: usize,
    pub max_exact_identity_integer_bits: usize,
}

impl Default for GeneratedCylindricalCandidateAuthorityLimits {
    fn default() -> Self {
        let recentering = ParametricAffineFreeRecenteringLimits::default();
        let identity = ExactIdentityLimits::default();
        Self {
            arithmetic: ParametricArithmeticLimits::default(),
            max_candidates: 1,
            max_family_fingerprint_bytes: portable_limit(1024u128 * 1024 * 1024),
            max_context_fingerprint_bytes: portable_limit(1024u128 * 1024 * 1024),
            max_ordering_identity_bytes: portable_limit(2u128 * 1024 * 1024 * 1024),
            max_arity: 1_000_000,
            max_pivot_components: 1_000_000,
            max_pivot_integer_bit_work: portable_limit(64_000_000_000),
            max_dependency_events: portable_limit(64_000_000_000),
            max_base_assumption_references: portable_limit(64_000_000_000),
            max_base_assumption_origin_references: portable_limit(64_000_000_000),
            max_base_assumption_manifest_bytes: portable_limit(64u128 * 1024 * 1024 * 1024),
            max_base_assumption_condition_owned_bytes: portable_limit(64u128 * 1024 * 1024 * 1024),
            max_centered_assignment_entries: 1_000_000,
            max_centered_assignment_additions: 1_000_000,
            max_centered_assignment_integer_bit_work: portable_limit(64_000_000_000),
            max_row_label_bytes: 1024 * 1024,
            max_centered_rhs_terms: portable_limit(64_000_000_000),
            max_recenter_attempts: 1,
            max_recenter_terms: recentering.max_terms,
            max_recenter_guards: recentering.max_guards,
            max_recenter_translation_components: recentering.max_translation_components,
            max_recenter_key_subtraction_boundary_checks: recentering
                .max_key_subtraction_boundary_checks,
            max_recenter_source_terms: recentering.max_source_terms,
            max_recenter_source_exponent_entries: recentering.max_source_exponent_entries,
            max_recenter_output_terms: recentering.max_output_terms,
            max_recenter_output_exponent_entries: recentering.max_output_exponent_entries,
            max_recenter_power_operations: recentering.max_power_operations,
            max_recenter_integer_bit_work: recentering.max_integer_bit_work,
            max_recenter_normalized_coefficient_terms: recentering.max_normalized_coefficient_terms,
            max_recenter_retained_bytes: recentering.max_retained_bytes,
            max_retained_payload_bytes: portable_limit(128u128 * 1024 * 1024 * 1024),
            max_local_replay_comparison_units: portable_limit(64_000_000_000),
            max_local_replay_comparison_bytes: portable_limit(256u128 * 1024 * 1024 * 1024),
            max_exact_identity_bytes: identity.max_identity_bytes,
            max_exact_identity_fields: identity.max_fields,
            max_exact_identity_tag_bytes: identity.max_tag_bytes,
            max_exact_identity_string_values: identity.max_string_values,
            max_exact_identity_string_bytes: identity.max_string_bytes,
            max_exact_identity_nesting_depth: identity.max_nesting_depth,
            max_exact_identity_polynomials: identity.max_polynomials,
            max_exact_identity_polynomial_variables: identity.max_polynomial_variables,
            max_exact_identity_polynomial_terms: identity.max_polynomial_terms,
            max_exact_identity_exponent_entries: identity.max_exponent_entries,
            max_exact_identity_integers: identity.max_integers,
            max_exact_identity_integer_bits: identity.max_integer_bits,
        }
    }
}

/// Census retained by one `compile_inner` pass.
///
/// Recenter fields reproduce the lower-level conservative envelopes. Local
/// comparison fields exclude nested source work, which remains governed by
/// the persistent certificate's independently replayed limits and statistics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedCylindricalCandidateAuthorityStats {
    candidates: usize,
    global_candidates: usize,
    locus_bound_candidates: usize,
    family_fingerprint_bytes: usize,
    context_fingerprint_bytes: usize,
    ordering_identity_bytes: usize,
    arity: usize,
    pivot_components: usize,
    pivot_integer_bit_work: usize,
    dependency_events: usize,
    base_assumption_references: usize,
    base_assumption_origin_references: usize,
    base_assumption_manifest_bytes: usize,
    base_assumption_condition_owned_bytes: usize,
    centered_assignment_entries: usize,
    centered_assignment_additions: usize,
    centered_assignment_integer_bit_work: usize,
    row_label_bytes: usize,
    centered_rhs_terms: usize,
    recenter_attempts: usize,
    recenter_terms: usize,
    recenter_guards: usize,
    recenter_translation_components: usize,
    recenter_key_subtraction_boundary_checks: usize,
    recenter_source_terms: usize,
    recenter_source_exponent_entries: usize,
    recenter_output_terms: usize,
    recenter_output_exponent_entries: usize,
    recenter_power_operations: usize,
    recenter_integer_bit_work: usize,
    recenter_normalized_coefficient_terms: usize,
    recenter_retained_bytes: usize,
    retained_payload_bytes: usize,
    local_replay_comparison_units: usize,
    local_replay_comparison_bytes: usize,
    exact_identity_bytes: usize,
    exact_identity_fields: usize,
    exact_identity_tag_bytes: usize,
    exact_identity_string_values: usize,
    exact_identity_string_bytes: usize,
    exact_identity_maximum_nesting_depth: usize,
    exact_identity_polynomials: usize,
    exact_identity_polynomial_variables: usize,
    exact_identity_polynomial_terms: usize,
    exact_identity_exponent_entries: usize,
    exact_identity_integers: usize,
    exact_identity_integer_bits: usize,
}

macro_rules! stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedCylindricalCandidateAuthorityStats {
    stats_getters!(
        candidates,
        global_candidates,
        locus_bound_candidates,
        family_fingerprint_bytes,
        context_fingerprint_bytes,
        ordering_identity_bytes,
        arity,
        pivot_components,
        pivot_integer_bit_work,
        dependency_events,
        base_assumption_references,
        base_assumption_origin_references,
        base_assumption_manifest_bytes,
        base_assumption_condition_owned_bytes,
        centered_assignment_entries,
        centered_assignment_additions,
        centered_assignment_integer_bit_work,
        row_label_bytes,
        centered_rhs_terms,
        recenter_attempts,
        recenter_terms,
        recenter_guards,
        recenter_translation_components,
        recenter_key_subtraction_boundary_checks,
        recenter_source_terms,
        recenter_source_exponent_entries,
        recenter_output_terms,
        recenter_output_exponent_entries,
        recenter_power_operations,
        recenter_integer_bit_work,
        recenter_normalized_coefficient_terms,
        recenter_retained_bytes,
        retained_payload_bytes,
        local_replay_comparison_units,
        local_replay_comparison_bytes,
        exact_identity_bytes,
        exact_identity_fields,
        exact_identity_tag_bytes,
        exact_identity_string_values,
        exact_identity_string_bytes,
        exact_identity_maximum_nesting_depth,
        exact_identity_polynomials,
        exact_identity_polynomial_variables,
        exact_identity_polynomial_terms,
        exact_identity_exponent_entries,
        exact_identity_integers,
        exact_identity_integer_bits,
    );
}

/// Sparse fixed-coordinate locus after translating the pivot to zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedCylindricalCenteredAssignment {
    arity: usize,
    entries: Box<[(usize, i64)]>,
}

impl GeneratedCylindricalCenteredAssignment {
    pub const fn arity(&self) -> usize {
        self.arity
    }

    pub fn entries(&self) -> &[(usize, i64)] {
        &self.entries
    }
}

/// Versioned ordering proof carried by cylindrical candidates.
///
/// There is intentionally no anchored arm and no discovery-point accessor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedCylindricalCandidateOrderingAuthority {
    CylindricalV1 {
        policy: IntegralOrderingPolicy,
        identity: Arc<str>,
    },
}

impl GeneratedCylindricalCandidateOrderingAuthority {
    pub const fn policy(&self) -> IntegralOrderingPolicy {
        match self {
            Self::CylindricalV1 { policy, .. } => *policy,
        }
    }

    pub fn identity(&self) -> &str {
        match self {
            Self::CylindricalV1 { identity, .. } => identity,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CandidateArmTag {
    Global,
    LocusBound,
}

impl CandidateArmTag {
    const fn stable_name(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::LocusBound => "locus-bound",
        }
    }
}

#[derive(Clone)]
struct GeneratedCylindricalCandidateBinding {
    schema: &'static str,
    arm: CandidateArmTag,
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    source: Arc<GeneratedCylindricalPersistentEliminationCertificate>,
    sector: SectorMask,
    ordering_authority: GeneratedCylindricalCandidateOrderingAuthority,
    pivot_ordinal: usize,
    source_event: GeneratedCylindricalPersistentEliminationEvent,
    dependency_event_ordinals: Box<[usize]>,
    base_assumption_witness_ordinals: Box<[usize]>,
    original_pivot: IndexShift,
    coefficient_translation: IndexShift,
    centered_assignment: Option<GeneratedCylindricalCenteredAssignment>,
    centered_relation: Arc<ParametricRelation>,
    recentering_stats: ParametricAffineFreeRecenteringStats,
    exact_identity: ExactStructuralIdentity,
    limits: GeneratedCylindricalCandidateAuthorityLimits,
    stats: GeneratedCylindricalCandidateAuthorityStats,
}

impl fmt::Debug for GeneratedCylindricalCandidateBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedCylindricalCandidateBinding")
            .field("schema", &self.schema)
            .field("arm", &self.arm)
            .field("family_fingerprint", &self.family_fingerprint)
            .field("context_fingerprint", &self.context_fingerprint)
            .field("sector", &self.sector)
            .field("ordering_authority", &self.ordering_authority)
            .field("pivot_ordinal", &self.pivot_ordinal)
            .field("source_event", &self.source_event)
            .field("dependency_event_ordinals", &self.dependency_event_ordinals)
            .field(
                "base_assumption_witness_ordinals",
                &self.base_assumption_witness_ordinals,
            )
            .field("original_pivot", &self.original_pivot)
            .field("coefficient_translation", &self.coefficient_translation)
            .field("centered_assignment", &self.centered_assignment)
            .field("centered_term_count", &self.centered_relation.terms().len())
            .field(
                "centered_guard_count",
                &self.centered_relation.guarded_nonzero_conditions().len(),
            )
            .field("private_centered_relation", &"<redacted>")
            .field("limits", &self.limits)
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

/// A globally valid centered candidate from an authenticated empty cylinder.
#[derive(Clone, Debug)]
pub struct GeneratedCylindricalGlobalCandidateAuthority {
    binding: Arc<GeneratedCylindricalCandidateBinding>,
}

/// A centered candidate which remains bound to fixed source coordinates.
#[derive(Clone, Debug)]
pub struct GeneratedCylindricalLocusBoundCandidateAuthority {
    binding: Arc<GeneratedCylindricalCandidateBinding>,
}

/// Type-safe authority split for one generated cylindrical pivot.
#[derive(Clone, Debug)]
pub enum GeneratedCylindricalCandidateAuthority {
    Global(GeneratedCylindricalGlobalCandidateAuthority),
    LocusBound(GeneratedCylindricalLocusBoundCandidateAuthority),
}

/// Typed compilation, replay, or specialization failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedCylindricalCandidateAuthorityError {
    SourceSchemaMismatch,
    ForeignFamily,
    ForeignContext,
    IncompleteSource {
        pending_equality_predicates: usize,
    },
    PivotOutOfRange {
        pivot_ordinal: usize,
    },
    PivotOrdinalMismatch {
        expected: usize,
        actual: usize,
    },
    PivotArityMismatch {
        expected: usize,
        actual: usize,
    },
    GuardedProvenanceUnavailable,
    CoefficientTranslationOverflow {
        position: usize,
    },
    CenteredAssignmentOverflow {
        position: usize,
    },
    MissingCenteredPivot,
    NonUnitCenteredPivot,
    LocusAssignmentMismatch {
        position: usize,
        expected: i64,
        actual: i64,
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
    ExactIdentityFailure {
        detail: String,
    },
    ReplayMismatch {
        detail: &'static str,
    },
    Source(GeneratedCylindricalPersistentEliminationError),
    Relation(ParametricRelationError),
}

impl fmt::Display for GeneratedCylindricalCandidateAuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceSchemaMismatch => formatter.write_str(
                "generated cylindrical candidates require a replayed persistent V2 source",
            ),
            Self::ForeignFamily => {
                formatter.write_str("candidate source belongs to another family")
            }
            Self::ForeignContext => {
                formatter.write_str("candidate source belongs to another parametric context")
            }
            Self::IncompleteSource {
                pending_equality_predicates,
            } => write!(
                formatter,
                "candidate source has {pending_equality_predicates} unresolved dependent equality predicates"
            ),
            Self::PivotOutOfRange { pivot_ordinal } => {
                write!(
                    formatter,
                    "persistent pivot ordinal {pivot_ordinal} is out of range"
                )
            }
            Self::PivotOrdinalMismatch { expected, actual } => write!(
                formatter,
                "guarded pivot ordinal {actual} differs from requested ordinal {expected}"
            ),
            Self::PivotArityMismatch { expected, actual } => write!(
                formatter,
                "guarded pivot arity {actual} differs from candidate arity {expected}"
            ),
            Self::GuardedProvenanceUnavailable => formatter
                .write_str("guarded pivot provenance cannot be resolved through its source"),
            Self::CoefficientTranslationOverflow { position } => write!(
                formatter,
                "negating pivot component {position} overflows the i64 coefficient lattice"
            ),
            Self::CenteredAssignmentOverflow { position } => write!(
                formatter,
                "centering fixed coordinate {position} overflows the i64 index lattice"
            ),
            Self::MissingCenteredPivot => {
                formatter.write_str("split recentering removed the zero-shift pivot")
            }
            Self::NonUnitCenteredPivot => {
                formatter.write_str("split recentering did not retain unit coefficient at zero")
            }
            Self::LocusAssignmentMismatch {
                position,
                expected,
                actual,
            } => write!(
                formatter,
                "assignment coordinate {position} is {actual}, outside the required centered locus {expected}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} units for {resource}"
            ),
            Self::ExactIdentityFailure { detail } => {
                write!(formatter, "exact candidate identity failed: {detail}")
            }
            Self::ReplayMismatch { detail } => {
                write!(formatter, "candidate replay mismatch: {detail}")
            }
            Self::Source(error) => write!(formatter, "persistent source failed: {error}"),
            Self::Relation(error) => write!(formatter, "candidate relation failed: {error}"),
        }
    }
}

impl std::error::Error for GeneratedCylindricalCandidateAuthorityError {}

impl From<GeneratedCylindricalPersistentEliminationError>
    for GeneratedCylindricalCandidateAuthorityError
{
    fn from(error: GeneratedCylindricalPersistentEliminationError) -> Self {
        Self::Source(error)
    }
}

impl From<ParametricRelationError> for GeneratedCylindricalCandidateAuthorityError {
    fn from(error: ParametricRelationError) -> Self {
        Self::Relation(error)
    }
}

impl<'scope> GeneratedCylindricalReplaySession<'scope> {
    pub(crate) fn new(
        family: &'scope IntegralFamily,
        context: &'scope ParametricCoefficientContext,
    ) -> Self {
        Self {
            family,
            context,
            replayed_sources: Vec::new(),
            replayed_source_pointer_index: Vec::new(),
        }
    }

    pub(crate) fn family(&self) -> &'scope IntegralFamily {
        self.family
    }

    pub(crate) fn context(&self) -> &'scope ParametricCoefficientContext {
        self.context
    }

    fn validate_scope(
        &self,
        source: &GeneratedCylindricalPersistentEliminationCertificate,
    ) -> Result<(), GeneratedCylindricalCandidateAuthorityError> {
        if source.schema() != GENERATED_CYLINDRICAL_PERSISTENT_ELIMINATION_V3_SCHEMA {
            return Err(GeneratedCylindricalCandidateAuthorityError::SourceSchemaMismatch);
        }
        if source.family_fingerprint() != self.family.fingerprint_ref() {
            return Err(GeneratedCylindricalCandidateAuthorityError::ForeignFamily);
        }
        if source.context_fingerprint() != self.context.fingerprint() {
            return Err(GeneratedCylindricalCandidateAuthorityError::ForeignContext);
        }
        Ok(())
    }

    pub(crate) fn preflight_source_scope(
        &self,
        source: &GeneratedCylindricalPersistentEliminationCertificate,
    ) -> Result<(), GeneratedCylindricalCandidateAuthorityError> {
        self.validate_scope(source)
    }

    /// Fully authenticate one exact source allocation unless that allocation
    /// already completed replay in this operation. A source is inserted only
    /// after successful replay.
    pub(crate) fn authenticate_source(
        &mut self,
        source: &Arc<GeneratedCylindricalPersistentEliminationCertificate>,
    ) -> Result<(), GeneratedCylindricalCandidateAuthorityError> {
        self.authenticate_sources(&[source])
    }

    pub(crate) fn authenticate_sources<'source>(
        &mut self,
        sources: &[&'source Arc<GeneratedCylindricalPersistentEliminationCertificate>],
    ) -> Result<(), GeneratedCylindricalCandidateAuthorityError> {
        self.authenticate_sources_with_table_byte_limits(sources, usize::MAX, usize::MAX)
    }

    pub(crate) fn authenticate_sources_with_reference_byte_limit<'source>(
        &mut self,
        sources: &[&'source Arc<GeneratedCylindricalPersistentEliminationCertificate>],
        max_reference_bytes: usize,
    ) -> Result<(), GeneratedCylindricalCandidateAuthorityError> {
        self.authenticate_sources_with_table_byte_limits(sources, max_reference_bytes, usize::MAX)
    }

    pub(crate) fn authenticate_sources_with_table_byte_limits<'source>(
        &mut self,
        sources: &[&'source Arc<GeneratedCylindricalPersistentEliminationCertificate>],
        max_reference_bytes: usize,
        max_pointer_index_bytes: usize,
    ) -> Result<(), GeneratedCylindricalCandidateAuthorityError> {
        // Validate the entire batch and acquire its fallible O(n) transient
        // pointer index before the first expensive replay. Sorting by exact
        // allocation address and then input ordinal lets deduplication keep
        // the first occurrence without quadratic scans.
        let incoming_pointer_index_minimum_bytes = checked_mul(
            "operation-scoped persistent-source pointer-index bytes",
            sources.len(),
            size_of::<GeneratedCylindricalIncomingSourcePointerIndexEntry>(),
        )?;
        check_limit(
            "operation-scoped persistent-source pointer-index bytes",
            incoming_pointer_index_minimum_bytes,
            max_pointer_index_bytes,
        )?;
        let mut incoming = Vec::new();
        try_reserve_exact(
            "operation-scoped incoming persistent-source pointer-index entries",
            &mut incoming,
            sources.len(),
        )?;
        let incoming_pointer_index_bytes = checked_mul(
            "operation-scoped persistent-source pointer-index bytes",
            incoming.capacity(),
            size_of::<GeneratedCylindricalIncomingSourcePointerIndexEntry>(),
        )?;
        check_limit(
            "operation-scoped persistent-source pointer-index bytes",
            incoming_pointer_index_bytes,
            max_pointer_index_bytes,
        )?;
        for (input_ordinal, source) in sources.iter().enumerate() {
            self.validate_scope(source)?;
            incoming.push(GeneratedCylindricalIncomingSourcePointerIndexEntry {
                address: generated_cylindrical_persistent_source_address(source),
                input_ordinal,
            });
        }
        incoming.sort_unstable_by(|left, right| {
            left.address
                .cmp(&right.address)
                .then_with(|| left.input_ordinal.cmp(&right.input_ordinal))
        });
        for adjacent in incoming.windows(2) {
            if adjacent[0].address == adjacent[1].address
                && !Arc::ptr_eq(
                    sources[adjacent[0].input_ordinal],
                    sources[adjacent[1].input_ordinal],
                )
            {
                return Err(
                    GeneratedCylindricalCandidateAuthorityError::ReplayMismatch {
                        detail: "two live persistent-source allocations share one pointer-index address",
                    },
                );
            }
        }
        incoming.dedup_by_key(|entry| entry.address);

        // Remove allocations already authenticated by this session. Compact
        // in place so the batch index has no second proportional allocation.
        let mut retained = 0usize;
        for inspected in 0..incoming.len() {
            let entry = incoming[inspected];
            let source = sources[entry.input_ordinal];
            if self.replayed_source_ordinal(source)?.is_none() {
                incoming[retained] = entry;
                retained = checked_add(
                    "operation-scoped new persistent-source pointer-index entries",
                    retained,
                    1,
                )?;
            }
        }
        incoming.truncate(retained);
        incoming.sort_unstable_by_key(|entry| entry.input_ordinal);

        // The common repeated-candidate path needs only indexed lookups. Keep
        // it O(k log n) for k supplied references and avoid rebuilding the
        // complete strong table when no new capability is required. Existing
        // allocator capacities are still re-admitted under the caller's
        // limits before returning.
        if incoming.is_empty() {
            check_limit(
                "operation-scoped replayed persistent-source reference bytes",
                self.source_reference_bytes()?,
                max_reference_bytes,
            )?;
            check_limit(
                "operation-scoped persistent-source pointer-index bytes",
                self.source_pointer_index_bytes()?,
                max_pointer_index_bytes,
            )?;
            return Ok(());
        }

        let final_source_count = checked_add(
            "operation-scoped replayed persistent-source references",
            self.replayed_sources.len(),
            incoming.len(),
        )?;

        // Stage complete prospective tables from scratch. Their actual
        // allocator capacities are checked independently before any replay.
        // The live session remains unchanged until every new source succeeds.
        let minimum_reference_bytes = checked_mul(
            "operation-scoped replayed persistent-source reference bytes",
            final_source_count,
            size_of::<Arc<GeneratedCylindricalPersistentEliminationCertificate>>(),
        )?;
        check_limit(
            "operation-scoped replayed persistent-source reference bytes",
            minimum_reference_bytes,
            max_reference_bytes,
        )?;
        let mut staged_sources = Vec::new();
        try_reserve_exact(
            "operation-scoped replayed persistent-source references",
            &mut staged_sources,
            final_source_count,
        )?;
        let reference_bytes = checked_mul(
            "operation-scoped replayed persistent-source reference bytes",
            staged_sources.capacity(),
            size_of::<Arc<GeneratedCylindricalPersistentEliminationCertificate>>(),
        )?;
        check_limit(
            "operation-scoped replayed persistent-source reference bytes",
            reference_bytes,
            max_reference_bytes,
        )?;
        staged_sources.extend(self.replayed_sources.iter().cloned());
        for entry in &incoming {
            staged_sources.push(Arc::clone(sources[entry.input_ordinal]));
        }

        let minimum_pointer_index_bytes = checked_mul(
            "operation-scoped persistent-source pointer-index bytes",
            final_source_count,
            size_of::<GeneratedCylindricalReplayedSourcePointerIndexEntry>(),
        )?;
        check_limit(
            "operation-scoped persistent-source pointer-index bytes",
            minimum_pointer_index_bytes,
            max_pointer_index_bytes,
        )?;
        let mut staged_pointer_index = Vec::new();
        try_reserve_exact(
            "operation-scoped replayed persistent-source pointer-index entries",
            &mut staged_pointer_index,
            final_source_count,
        )?;
        let pointer_index_bytes = checked_mul(
            "operation-scoped persistent-source pointer-index bytes",
            staged_pointer_index.capacity(),
            size_of::<GeneratedCylindricalReplayedSourcePointerIndexEntry>(),
        )?;
        check_limit(
            "operation-scoped persistent-source pointer-index bytes",
            pointer_index_bytes,
            max_pointer_index_bytes,
        )?;
        staged_pointer_index.extend(self.replayed_source_pointer_index.iter().copied());
        for (new_ordinal, entry) in incoming.iter().enumerate() {
            staged_pointer_index.push(GeneratedCylindricalReplayedSourcePointerIndexEntry {
                address: entry.address,
                source_ordinal: checked_add(
                    "operation-scoped replayed persistent-source ordinals",
                    self.replayed_sources.len(),
                    new_ordinal,
                )?,
            });
        }
        staged_pointer_index.sort_unstable_by_key(|entry| entry.address);
        if staged_sources.len() != final_source_count
            || staged_pointer_index.len() != final_source_count
        {
            return Err(
                GeneratedCylindricalCandidateAuthorityError::ReplayMismatch {
                    detail: "staged persistent-source pointer tables changed length",
                },
            );
        }
        for (ordinal, entry) in staged_pointer_index.iter().enumerate() {
            let Some(source) = staged_sources.get(entry.source_ordinal) else {
                return Err(
                    GeneratedCylindricalCandidateAuthorityError::ReplayMismatch {
                        detail: "staged persistent-source pointer index contains an out-of-range ordinal",
                    },
                );
            };
            if generated_cylindrical_persistent_source_address(source) != entry.address {
                return Err(
                    GeneratedCylindricalCandidateAuthorityError::ReplayMismatch {
                        detail: "staged persistent-source pointer index differs from its strong Arc",
                    },
                );
            }
            if ordinal > 0 && staged_pointer_index[ordinal - 1].address == entry.address {
                return Err(
                    GeneratedCylindricalCandidateAuthorityError::ReplayMismatch {
                        detail: "staged persistent-source pointer index contains a duplicate allocation",
                    },
                );
            }
        }

        // Replay new exact allocations in first-input order. No capability is
        // published if any replay fails.
        for entry in &incoming {
            let source = sources[entry.input_ordinal];
            source.replay(self.family, self.context)?;
            #[cfg(test)]
            record_operation_scoped_persistent_source_replay_for_test();
        }

        // Atomic, infallible publication of the fully replayed prospective
        // tables. The old tables stay live through the entire staging pass.
        self.replayed_sources = staged_sources;
        self.replayed_source_pointer_index = staged_pointer_index;
        Ok(())
    }

    pub(crate) fn source_reference_bytes(
        &self,
    ) -> Result<usize, GeneratedCylindricalCandidateAuthorityError> {
        checked_mul(
            "operation-scoped replayed persistent-source reference bytes",
            self.replayed_sources.capacity(),
            size_of::<Arc<GeneratedCylindricalPersistentEliminationCertificate>>(),
        )
    }

    pub(crate) fn source_pointer_index_bytes(
        &self,
    ) -> Result<usize, GeneratedCylindricalCandidateAuthorityError> {
        checked_mul(
            "operation-scoped persistent-source pointer-index bytes",
            self.replayed_source_pointer_index.capacity(),
            size_of::<GeneratedCylindricalReplayedSourcePointerIndexEntry>(),
        )
    }

    fn replayed_source_ordinal(
        &self,
        source: &Arc<GeneratedCylindricalPersistentEliminationCertificate>,
    ) -> Result<Option<usize>, GeneratedCylindricalCandidateAuthorityError> {
        let address = generated_cylindrical_persistent_source_address(source);
        let Ok(index_ordinal) = self
            .replayed_source_pointer_index
            .binary_search_by_key(&address, |entry| entry.address)
        else {
            return Ok(None);
        };
        let entry = self.replayed_source_pointer_index[index_ordinal];
        let replayed = self.replayed_sources.get(entry.source_ordinal).ok_or(
            GeneratedCylindricalCandidateAuthorityError::ReplayMismatch {
                detail: "persistent-source pointer index contains an out-of-range ordinal",
            },
        )?;
        if !Arc::ptr_eq(replayed, source) {
            return Err(
                GeneratedCylindricalCandidateAuthorityError::ReplayMismatch {
                    detail: "persistent-source pointer index differs from its strong Arc",
                },
            );
        }
        Ok(Some(entry.source_ordinal))
    }

    pub(crate) fn replayed_source<'session>(
        &'session self,
        source: &Arc<GeneratedCylindricalPersistentEliminationCertificate>,
    ) -> Result<
        ReplayedGeneratedCylindricalPersistentSource<'session, 'scope>,
        GeneratedCylindricalCandidateAuthorityError,
    > {
        self.validate_scope(source)?;
        let source_ordinal = self.replayed_source_ordinal(source)?.ok_or(
            GeneratedCylindricalCandidateAuthorityError::ReplayMismatch {
                detail: "exact persistent-source allocation was not replayed in this operation",
            },
        )?;
        Ok(ReplayedGeneratedCylindricalPersistentSource {
            session: self,
            source_ordinal,
        })
    }
}

impl<'session, 'scope> ReplayedGeneratedCylindricalPersistentSource<'session, 'scope> {
    pub(crate) fn family(self) -> &'scope IntegralFamily {
        self.session.family()
    }

    pub(crate) fn context(self) -> &'scope ParametricCoefficientContext {
        self.session.context()
    }

    pub(crate) fn source(
        self,
    ) -> &'session Arc<GeneratedCylindricalPersistentEliminationCertificate> {
        &self.session.replayed_sources[self.source_ordinal]
    }

    fn validate_exact_source(
        self,
        source: &Arc<GeneratedCylindricalPersistentEliminationCertificate>,
    ) -> Result<(), GeneratedCylindricalCandidateAuthorityError> {
        if Arc::ptr_eq(self.source(), source) {
            Ok(())
        } else {
            Err(
                GeneratedCylindricalCandidateAuthorityError::ReplayMismatch {
                    detail: "candidate source differs from the replayed source allocation",
                },
            )
        }
    }
}

impl<'candidate, 'session, 'scope>
    ReplayedGeneratedCylindricalGlobalCandidate<'candidate, 'session, 'scope>
{
    pub(crate) const fn candidate(
        self,
    ) -> &'candidate GeneratedCylindricalGlobalCandidateAuthority {
        self.candidate
    }

    pub(crate) fn family(self) -> &'scope IntegralFamily {
        self.source.family()
    }

    pub(crate) fn context(self) -> &'scope ParametricCoefficientContext {
        self.source.context()
    }
}

impl GeneratedCylindricalCandidateAuthority {
    /// Allocation-free lower-bound checks shared by the exhaustive batch
    /// composer. These invariants depend only on authenticated scope metadata
    /// and the retained pivot count; pivot-specific provenance and algebra
    /// remain behind persistent-source replay.
    pub(crate) fn preflight_exhaustive_batch_fixed_limits(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source: &GeneratedCylindricalPersistentEliminationCertificate,
        pivot_count: usize,
        limits: GeneratedCylindricalCandidateAuthorityLimits,
    ) -> Result<(), GeneratedCylindricalCandidateAuthorityError> {
        if pivot_count == 0 {
            return Ok(());
        }
        check_limit("cylindrical candidates", 1, limits.max_candidates)?;
        check_limit(
            "candidate family fingerprint bytes",
            family.fingerprint_ref().len(),
            limits.max_family_fingerprint_bytes,
        )?;
        check_limit(
            "candidate context fingerprint bytes",
            context.fingerprint().len(),
            limits.max_context_fingerprint_bytes,
        )?;
        check_limit(
            "candidate ordering identity bytes",
            source.ordering_identity().len(),
            limits.max_ordering_identity_bytes,
        )?;
        check_limit("candidate arity", context.index_count(), limits.max_arity)?;
        check_limit(
            "candidate pivot components",
            context.index_count(),
            limits.max_pivot_components,
        )?;
        check_limit(
            "candidate row label bytes",
            candidate_row_label_byte_len(pivot_count - 1)?,
            limits.max_row_label_bytes,
        )?;
        check_limit(
            "candidate recenter attempts",
            1,
            limits.max_recenter_attempts,
        )
    }

    /// Compile exactly one guarded pivot into its global or locus-bound arm.
    ///
    /// The arm is selected solely by the authenticated source assignment.
    /// No concrete anchor, topology tag, or expected recurrence enters this
    /// interface.
    pub fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source: Arc<GeneratedCylindricalPersistentEliminationCertificate>,
        pivot_ordinal: usize,
        limits: GeneratedCylindricalCandidateAuthorityLimits,
    ) -> Result<Self, GeneratedCylindricalCandidateAuthorityError> {
        let mut session = GeneratedCylindricalReplaySession::new(family, context);
        let candidate =
            Self::compile_with_replay_session(source, pivot_ordinal, limits, &mut session)?;
        Ok(candidate)
    }

    pub(crate) fn compile_with_replay_session(
        source: Arc<GeneratedCylindricalPersistentEliminationCertificate>,
        pivot_ordinal: usize,
        limits: GeneratedCylindricalCandidateAuthorityLimits,
        session: &mut GeneratedCylindricalReplaySession<'_>,
    ) -> Result<Self, GeneratedCylindricalCandidateAuthorityError> {
        let candidate =
            compile_inner_authenticating_source(source, pivot_ordinal, limits, session)?;
        candidate.replay_with_replay_session(session)?;
        Ok(candidate)
    }

    /// One compiler-fresh local construction from an already authenticated
    /// exact source. The value must remain behind a sealed composing path
    /// until a second local reconstruction yields a replayed-candidate token.
    pub(crate) fn compile_fresh_with_authenticated_session(
        source: Arc<GeneratedCylindricalPersistentEliminationCertificate>,
        pivot_ordinal: usize,
        limits: GeneratedCylindricalCandidateAuthorityLimits,
        session: &GeneratedCylindricalReplaySession<'_>,
    ) -> Result<Self, GeneratedCylindricalCandidateAuthorityError> {
        let replayed_source = session.replayed_source(&source)?;
        compile_inner_with_replayed_source(source, pivot_ordinal, limits, replayed_source)
    }

    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedCylindricalCandidateAuthorityError> {
        let mut session = GeneratedCylindricalReplaySession::new(family, context);
        self.replay_with_replay_session(&mut session)
    }

    pub(crate) fn replay_with_replay_session(
        &self,
        session: &mut GeneratedCylindricalReplaySession<'_>,
    ) -> Result<(), GeneratedCylindricalCandidateAuthorityError> {
        let binding = self.binding();
        self.preflight_replay(session.family(), session.context())?;
        session.authenticate_source(&binding.source)?;
        self.replay_with_authenticated_session(session)
    }

    fn preflight_replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedCylindricalCandidateAuthorityError> {
        let binding = self.binding();
        if binding.schema != GENERATED_CYLINDRICAL_CANDIDATE_AUTHORITY_V1_SCHEMA
            || binding.family_fingerprint.as_ref() != family.fingerprint_ref()
            || binding.context_fingerprint.as_ref() != context.fingerprint()
        {
            return Err(
                GeneratedCylindricalCandidateAuthorityError::ReplayMismatch {
                    detail: "candidate shallow binding differs from replay scope",
                },
            );
        }
        binding.guarded_pivot()?;
        preflight_compile_inner(
            family,
            context,
            binding.source.clone(),
            binding.pivot_ordinal,
            binding.limits,
        )
    }

    pub(crate) fn replay_with_authenticated_session(
        &self,
        session: &GeneratedCylindricalReplaySession<'_>,
    ) -> Result<(), GeneratedCylindricalCandidateAuthorityError> {
        let binding = self.binding();
        let replayed_source = session.replayed_source(&binding.source)?;
        self.replay_with_replayed_source(replayed_source)
    }

    fn replay_with_replayed_source(
        &self,
        replayed_source: ReplayedGeneratedCylindricalPersistentSource<'_, '_>,
    ) -> Result<(), GeneratedCylindricalCandidateAuthorityError> {
        let binding = self.binding();
        let replayed = compile_inner_with_replayed_source(
            binding.source.clone(),
            binding.pivot_ordinal,
            binding.limits,
            replayed_source,
        )?;
        if self.payload_eq_with_replayed_source(&replayed) {
            Ok(())
        } else {
            Err(
                GeneratedCylindricalCandidateAuthorityError::ReplayMismatch {
                    detail: "complete cylindrical candidate payload differs",
                },
            )
        }
    }

    pub const fn is_global(&self) -> bool {
        matches!(self, Self::Global(_))
    }

    pub const fn is_locus_bound(&self) -> bool {
        matches!(self, Self::LocusBound(_))
    }

    /// This layer is strictly pre-`WhenBad` and never publishes a rule.
    pub const fn is_applicable_rule(&self) -> bool {
        false
    }

    pub fn schema(&self) -> &'static str {
        self.binding().schema
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.binding().family_fingerprint
    }

    pub fn context_fingerprint(&self) -> &str {
        &self.binding().context_fingerprint
    }

    pub fn source(&self) -> &Arc<GeneratedCylindricalPersistentEliminationCertificate> {
        &self.binding().source
    }

    pub fn sector(&self) -> &SectorMask {
        &self.binding().sector
    }

    pub fn ordering_authority(&self) -> &GeneratedCylindricalCandidateOrderingAuthority {
        &self.binding().ordering_authority
    }

    pub fn pivot_ordinal(&self) -> usize {
        self.binding().pivot_ordinal
    }

    pub fn source_event(&self) -> &GeneratedCylindricalPersistentEliminationEvent {
        &self.binding().source_event
    }

    pub fn dependency_event_ordinals(&self) -> &[usize] {
        &self.binding().dependency_event_ordinals
    }

    pub fn base_assumption_witness_ordinals(&self) -> &[usize] {
        &self.binding().base_assumption_witness_ordinals
    }

    /// Resolve the complete transitive base-field closure through the retained
    /// source. Conditions are never cloned or paired with detached locators.
    pub fn base_assumptions(&self) -> GeneratedCylindricalPersistentPivotBaseAssumptions<'_> {
        self.binding()
            .source
            .guarded_pivot(self.binding().pivot_ordinal)
            .expect("replayed candidate guarded pivot")
            .base_assumptions()
    }

    pub fn original_pivot(&self) -> &IndexShift {
        &self.binding().original_pivot
    }

    pub fn coefficient_translation(&self) -> &IndexShift {
        &self.binding().coefficient_translation
    }

    /// Integral keys are centered on the original pivot, not on its negation.
    pub fn key_center(&self) -> &IndexShift {
        &self.binding().original_pivot
    }

    pub fn centered_assignment(&self) -> Option<&GeneratedCylindricalCenteredAssignment> {
        self.binding().centered_assignment.as_ref()
    }

    pub fn centered_term_count(&self) -> usize {
        self.binding().centered_relation.terms().len()
    }

    pub fn centered_guard_count(&self) -> usize {
        self.binding()
            .centered_relation
            .guarded_nonzero_conditions()
            .len()
    }

    pub fn limits(&self) -> GeneratedCylindricalCandidateAuthorityLimits {
        self.binding().limits
    }

    pub fn stats(&self) -> GeneratedCylindricalCandidateAuthorityStats {
        self.binding().stats
    }

    /// Proof-only specialization of the private centered identity together
    /// with every inherited base-field condition. This remains crate-private
    /// until generated `WhenBad` has certified an applicable rule.
    pub(crate) fn specialize_identity_for_proof(
        &self,
        context: &ParametricCoefficientContext,
        assignment: &[i64],
        limits: ParametricArithmeticLimits,
    ) -> Result<ConcreteRelation, GeneratedCylindricalCandidateAuthorityError> {
        self.binding().specialize(context, assignment, limits)
    }

    fn binding(&self) -> &GeneratedCylindricalCandidateBinding {
        match self {
            Self::Global(candidate) => &candidate.binding,
            Self::LocusBound(candidate) => &candidate.binding,
        }
    }

    fn payload_eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Global(_), Self::Global(_)) | (Self::LocusBound(_), Self::LocusBound(_))
        ) && self.binding().payload_eq(other.binding())
    }

    fn payload_eq_with_replayed_source(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Global(_), Self::Global(_)) | (Self::LocusBound(_), Self::LocusBound(_))
        ) && self
            .binding()
            .payload_eq_with_replayed_source(other.binding())
    }
}

impl GeneratedCylindricalGlobalCandidateAuthority {
    /// Replay the complete candidate from its retained persistent source.
    /// This is a Global-arm-only prerequisite for the sealed generated
    /// `WhenBad` view.
    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedCylindricalCandidateAuthorityError> {
        let mut session = GeneratedCylindricalReplaySession::new(family, context);
        self.replay_with_replay_session(&mut session).map(|_| ())
    }

    pub(crate) fn replay_with_replay_session<'candidate, 'session, 'scope>(
        &'candidate self,
        session: &'session mut GeneratedCylindricalReplaySession<'scope>,
    ) -> Result<
        ReplayedGeneratedCylindricalGlobalCandidate<'candidate, 'session, 'scope>,
        GeneratedCylindricalCandidateAuthorityError,
    > {
        self.preflight_replay(session.family(), session.context())?;
        session.authenticate_source(&self.binding.source)?;
        self.replay_with_authenticated_session(session)
    }

    pub(crate) fn replay_with_authenticated_session<'candidate, 'session, 'scope>(
        &'candidate self,
        session: &'session GeneratedCylindricalReplaySession<'scope>,
    ) -> Result<
        ReplayedGeneratedCylindricalGlobalCandidate<'candidate, 'session, 'scope>,
        GeneratedCylindricalCandidateAuthorityError,
    > {
        let replayed_source = session.replayed_source(&self.binding.source)?;
        GeneratedCylindricalCandidateAuthority::Global(self.clone())
            .replay_with_replayed_source(replayed_source)?;
        Ok(ReplayedGeneratedCylindricalGlobalCandidate {
            candidate: self,
            source: replayed_source,
        })
    }

    pub(crate) fn preflight_replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedCylindricalCandidateAuthorityError> {
        GeneratedCylindricalCandidateAuthority::Global(self.clone())
            .preflight_replay(family, context)
    }

    pub(crate) fn family_fingerprint(&self) -> &str {
        self.binding.family_fingerprint.as_ref()
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        self.binding.context_fingerprint.as_ref()
    }

    pub(crate) fn sector(&self) -> &SectorMask {
        &self.binding.sector
    }

    pub(crate) fn ordering_authority(&self) -> &GeneratedCylindricalCandidateOrderingAuthority {
        &self.binding.ordering_authority
    }

    pub(crate) fn ordering_policy(&self) -> IntegralOrderingPolicy {
        self.binding.ordering_authority.policy()
    }

    pub(crate) fn original_pivot(&self) -> &IndexShift {
        &self.binding.original_pivot
    }

    pub(crate) fn base_assumptions(
        &self,
    ) -> GeneratedCylindricalPersistentPivotBaseAssumptions<'_> {
        self.binding
            .source
            .guarded_pivot(self.binding.pivot_ordinal)
            .expect("replayed global candidate guarded pivot")
            .base_assumptions()
    }

    /// Retained row-span allocation authenticated by the candidate's typed
    /// cylindrical start. Callers can cheaply clone this `Arc` without
    /// reconstructing or deep-cloning the generated symbolic basis.
    pub(crate) fn row_span_arc(&self) -> &Arc<GeneratedSymbolicRowSpanCertificate> {
        self.binding.source.row_system().start().row_span_arc()
    }

    /// Local recurrence binding only. A composing proof must separately
    /// retain and replay the candidate through [`Self::replay`].
    pub(crate) fn local_candidate_binding_identity_for_source_composition(&self) -> &str {
        self.binding.exact_identity.as_str()
    }

    pub(crate) fn limits(&self) -> GeneratedCylindricalCandidateAuthorityLimits {
        self.binding.limits
    }

    /// Candidate-local retained-payload census.  Nested persistent-source
    /// retention remains a separate certificate-owned charge so a composer
    /// can deduplicate shared `Arc` sources without undercounting this local
    /// authority payload.
    pub(crate) fn stats(&self) -> GeneratedCylindricalCandidateAuthorityStats {
        self.binding.stats
    }

    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.binding.payload_eq(&other.binding)
    }

    pub(crate) fn payload_eq_with_replayed_source(&self, other: &Self) -> bool {
        self.binding.payload_eq_with_replayed_source(&other.binding)
    }

    pub(crate) fn shares_binding_allocation(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.binding, &other.binding)
    }

    pub fn source(&self) -> &Arc<GeneratedCylindricalPersistentEliminationCertificate> {
        &self.binding.source
    }

    pub fn pivot_ordinal(&self) -> usize {
        self.binding.pivot_ordinal
    }

    pub const fn is_applicable_rule(&self) -> bool {
        false
    }

    pub(crate) fn specialize_identity_for_proof(
        &self,
        context: &ParametricCoefficientContext,
        assignment: &[i64],
        limits: ParametricArithmeticLimits,
    ) -> Result<ConcreteRelation, GeneratedCylindricalCandidateAuthorityError> {
        self.binding.specialize(context, assignment, limits)
    }

    /// Global-only input seam for the ordinary generated-`WhenBad` compiler.
    pub(crate) fn centered_relation_for_generated_when_bad(&self) -> &ParametricRelation {
        &self.binding.centered_relation
    }
}

impl GeneratedCylindricalLocusBoundCandidateAuthority {
    pub fn source(&self) -> &Arc<GeneratedCylindricalPersistentEliminationCertificate> {
        &self.binding.source
    }

    pub fn pivot_ordinal(&self) -> usize {
        self.binding.pivot_ordinal
    }

    pub fn centered_assignment(&self) -> &GeneratedCylindricalCenteredAssignment {
        self.binding
            .centered_assignment
            .as_ref()
            .expect("locus-bound arm has a centered assignment")
    }

    pub const fn is_applicable_rule(&self) -> bool {
        false
    }

    pub(crate) fn specialize_identity_for_proof(
        &self,
        context: &ParametricCoefficientContext,
        assignment: &[i64],
        limits: ParametricArithmeticLimits,
    ) -> Result<ConcreteRelation, GeneratedCylindricalCandidateAuthorityError> {
        self.binding.specialize(context, assignment, limits)
    }

    /// Locus-only input seam for a future conditional generated-`WhenBad`
    /// compiler. Its distinct name prevents accidental ordinary-rule use.
    pub(crate) fn centered_relation_for_conditional_generated_when_bad(
        &self,
    ) -> &ParametricRelation {
        &self.binding.centered_relation
    }
}

impl GeneratedCylindricalCandidateBinding {
    fn guarded_pivot(
        &self,
    ) -> Result<
        GeneratedCylindricalPersistentGuardedPivot<'_>,
        GeneratedCylindricalCandidateAuthorityError,
    > {
        let guarded = self
            .source
            .guarded_pivot(self.pivot_ordinal)
            .ok_or(GeneratedCylindricalCandidateAuthorityError::GuardedProvenanceUnavailable)?;
        if guarded.ordinal() != self.pivot_ordinal
            || guarded.source_event() != &self.source_event
            || guarded.original_pivot() != &self.original_pivot
        {
            return Err(GeneratedCylindricalCandidateAuthorityError::GuardedProvenanceUnavailable);
        }
        if !guarded
            .dependency_events()
            .map(|event| event.event_ordinal())
            .eq(self.dependency_event_ordinals.iter().copied())
        {
            return Err(GeneratedCylindricalCandidateAuthorityError::GuardedProvenanceUnavailable);
        }
        if !guarded
            .base_assumptions()
            .map(|assumption| assumption.witness().ordinal())
            .eq(self.base_assumption_witness_ordinals.iter().copied())
        {
            return Err(GeneratedCylindricalCandidateAuthorityError::GuardedProvenanceUnavailable);
        }
        Ok(guarded)
    }

    fn specialize(
        &self,
        context: &ParametricCoefficientContext,
        assignment: &[i64],
        limits: ParametricArithmeticLimits,
    ) -> Result<ConcreteRelation, GeneratedCylindricalCandidateAuthorityError> {
        if context.fingerprint() != self.context_fingerprint.as_ref() {
            return Err(GeneratedCylindricalCandidateAuthorityError::ForeignContext);
        }
        if assignment.len() != self.original_pivot.arity() {
            return Err(ParametricRelationError::WrongArity {
                expected: self.original_pivot.arity(),
                actual: assignment.len(),
            }
            .into());
        }
        if let Some(centered) = &self.centered_assignment {
            for &(position, expected) in centered.entries() {
                let actual = assignment[position];
                if actual != expected {
                    return Err(
                        GeneratedCylindricalCandidateAuthorityError::LocusAssignmentMismatch {
                            position,
                            expected,
                            actual,
                        },
                    );
                }
            }
        }
        let guarded = self.guarded_pivot()?;
        self.centered_relation
            .specialize_with_additional_nonzero_conditions(
                context,
                assignment,
                guarded
                    .base_assumptions()
                    .map(|assumption| assumption.condition()),
                limits,
            )
            .map_err(Into::into)
    }

    fn payload_eq(&self, other: &Self) -> bool {
        self.source.payload_eq(&other.source) && self.payload_eq_excluding_source(other)
    }

    fn payload_eq_with_replayed_source(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.source, &other.source) && self.payload_eq_excluding_source(other)
    }

    fn payload_eq_excluding_source(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.arm == other.arm
            && self.family_fingerprint == other.family_fingerprint
            && self.context_fingerprint == other.context_fingerprint
            && self.sector == other.sector
            && self.ordering_authority == other.ordering_authority
            && self.pivot_ordinal == other.pivot_ordinal
            && self.source_event == other.source_event
            && self.dependency_event_ordinals == other.dependency_event_ordinals
            && self.base_assumption_witness_ordinals == other.base_assumption_witness_ordinals
            && self.original_pivot == other.original_pivot
            && self.coefficient_translation == other.coefficient_translation
            && self.centered_assignment == other.centered_assignment
            && self
                .centered_relation
                .has_identical_guard_provenance(&other.centered_relation)
            && self.recentering_stats == other.recentering_stats
            && self.exact_identity == other.exact_identity
            && self.limits == other.limits
            && self.stats == other.stats
    }
}

fn preflight_compile_inner(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    source: Arc<GeneratedCylindricalPersistentEliminationCertificate>,
    pivot_ordinal: usize,
    limits: GeneratedCylindricalCandidateAuthorityLimits,
) -> Result<(), GeneratedCylindricalCandidateAuthorityError> {
    match compile_inner_with_source_authenticator(
        family,
        context,
        source,
        pivot_ordinal,
        limits,
        |_| {
            Err(
                GeneratedCylindricalCandidateAuthorityError::ReplayMismatch {
                    detail: CANDIDATE_PREFLIGHT_COMPLETE_DETAIL,
                },
            )
        },
    ) {
        Err(GeneratedCylindricalCandidateAuthorityError::ReplayMismatch { detail })
            if detail == CANDIDATE_PREFLIGHT_COMPLETE_DETAIL =>
        {
            Ok(())
        }
        Err(error) => Err(error),
        Ok(_) => Err(
            GeneratedCylindricalCandidateAuthorityError::ReplayMismatch {
                detail: "candidate shallow preflight crossed its authentication boundary",
            },
        ),
    }
}

fn compile_inner_authenticating_source(
    source: Arc<GeneratedCylindricalPersistentEliminationCertificate>,
    pivot_ordinal: usize,
    limits: GeneratedCylindricalCandidateAuthorityLimits,
    session: &mut GeneratedCylindricalReplaySession<'_>,
) -> Result<GeneratedCylindricalCandidateAuthority, GeneratedCylindricalCandidateAuthorityError> {
    compile_inner_with_source_authenticator(
        session.family(),
        session.context(),
        source,
        pivot_ordinal,
        limits,
        |source| session.authenticate_source(source),
    )
}

fn compile_inner_with_replayed_source(
    source: Arc<GeneratedCylindricalPersistentEliminationCertificate>,
    pivot_ordinal: usize,
    limits: GeneratedCylindricalCandidateAuthorityLimits,
    replayed_source: ReplayedGeneratedCylindricalPersistentSource<'_, '_>,
) -> Result<GeneratedCylindricalCandidateAuthority, GeneratedCylindricalCandidateAuthorityError> {
    replayed_source.validate_exact_source(&source)?;
    let candidate = compile_inner_with_source_authenticator(
        replayed_source.family(),
        replayed_source.context(),
        source,
        pivot_ordinal,
        limits,
        |source| replayed_source.validate_exact_source(source),
    )?;
    #[cfg(test)]
    record_authenticated_candidate_local_reconstruction_for_test();
    Ok(candidate)
}

fn compile_inner_with_source_authenticator(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    source: Arc<GeneratedCylindricalPersistentEliminationCertificate>,
    pivot_ordinal: usize,
    limits: GeneratedCylindricalCandidateAuthorityLimits,
    authenticate_source: impl FnOnce(
        &Arc<GeneratedCylindricalPersistentEliminationCertificate>,
    ) -> Result<(), GeneratedCylindricalCandidateAuthorityError>,
) -> Result<GeneratedCylindricalCandidateAuthority, GeneratedCylindricalCandidateAuthorityError> {
    check_limit("cylindrical candidates", 1, limits.max_candidates)?;
    if source.schema() != GENERATED_CYLINDRICAL_PERSISTENT_ELIMINATION_V3_SCHEMA {
        return Err(GeneratedCylindricalCandidateAuthorityError::SourceSchemaMismatch);
    }
    if source.family_fingerprint() != family.fingerprint_ref() {
        return Err(GeneratedCylindricalCandidateAuthorityError::ForeignFamily);
    }
    if source.context_fingerprint() != context.fingerprint() {
        return Err(GeneratedCylindricalCandidateAuthorityError::ForeignContext);
    }
    let row_system = source.row_system();
    let start = row_system.start();
    if !start.completeness().is_complete_integer_cylinder() {
        return Err(
            GeneratedCylindricalCandidateAuthorityError::IncompleteSource {
                pending_equality_predicates: start
                    .completeness()
                    .pending_equality_predicate_ordinals()
                    .len(),
            },
        );
    }
    let assignment = start.assignment();
    let arm = if assignment.is_empty() {
        CandidateArmTag::Global
    } else {
        CandidateArmTag::LocusBound
    };
    let family_fingerprint_bytes = source.family_fingerprint().len();
    let context_fingerprint_bytes = source.context_fingerprint().len();
    let ordering_identity_bytes = source.ordering_identity().len();
    check_limit(
        "candidate family fingerprint bytes",
        family_fingerprint_bytes,
        limits.max_family_fingerprint_bytes,
    )?;
    check_limit(
        "candidate context fingerprint bytes",
        context_fingerprint_bytes,
        limits.max_context_fingerprint_bytes,
    )?;
    check_limit(
        "candidate ordering identity bytes",
        ordering_identity_bytes,
        limits.max_ordering_identity_bytes,
    )?;
    if source.ordering_identity() != start.schedule().ordering().stable_manifest() {
        return Err(GeneratedCylindricalCandidateAuthorityError::GuardedProvenanceUnavailable);
    }

    let arity = context.index_count();
    check_limit("candidate arity", arity, limits.max_arity)?;
    if assignment.arity() != arity || start.sector().arity() != arity {
        return Err(
            GeneratedCylindricalCandidateAuthorityError::PivotArityMismatch {
                expected: arity,
                actual: assignment.arity(),
            },
        );
    }
    let guarded = source
        .guarded_pivot(pivot_ordinal)
        .ok_or(GeneratedCylindricalCandidateAuthorityError::PivotOutOfRange { pivot_ordinal })?;
    if guarded.ordinal() != pivot_ordinal {
        return Err(
            GeneratedCylindricalCandidateAuthorityError::PivotOrdinalMismatch {
                expected: pivot_ordinal,
                actual: guarded.ordinal(),
            },
        );
    }
    let original_pivot_ref = guarded.original_pivot();
    if original_pivot_ref.arity() != arity {
        return Err(
            GeneratedCylindricalCandidateAuthorityError::PivotArityMismatch {
                expected: arity,
                actual: original_pivot_ref.arity(),
            },
        );
    }
    check_limit(
        "candidate pivot components",
        original_pivot_ref.arity(),
        limits.max_pivot_components,
    )?;
    let mut pivot_integer_bit_work = 0usize;
    for &component in original_pivot_ref.values() {
        pivot_integer_bit_work = bounded_add(
            "candidate pivot integer-bit work",
            pivot_integer_bit_work,
            i64_magnitude_bits(component),
            limits.max_pivot_integer_bit_work,
        )?;
    }

    let dependency_event_count = guarded.dependency_event_count();
    check_limit(
        "candidate dependency events",
        dependency_event_count,
        limits.max_dependency_events,
    )?;
    let base_assumption_count = guarded.base_assumption_count();
    check_limit(
        "candidate base-assumption references",
        base_assumption_count,
        limits.max_base_assumption_references,
    )?;
    let mut base_assumption_origin_references = 0usize;
    let mut base_assumption_manifest_bytes = 0usize;
    let mut base_assumption_condition_owned_bytes = 0usize;
    for resolved in guarded.base_assumptions() {
        let witness = resolved.witness();
        base_assumption_origin_references = bounded_add(
            "candidate base-assumption origin references",
            base_assumption_origin_references,
            witness.origin_count(),
            limits.max_base_assumption_origin_references,
        )?;
        base_assumption_manifest_bytes = bounded_add(
            "candidate base-assumption manifest bytes",
            base_assumption_manifest_bytes,
            witness.manifest().len(),
            limits.max_base_assumption_manifest_bytes,
        )?;
        base_assumption_condition_owned_bytes = bounded_add(
            "candidate base-assumption condition owned bytes",
            base_assumption_condition_owned_bytes,
            witness.condition_owned_bytes(),
            limits.max_base_assumption_condition_owned_bytes,
        )?;
    }
    for (position, &component) in original_pivot_ref.values().iter().enumerate() {
        if component == i64::MIN {
            return Err(
                GeneratedCylindricalCandidateAuthorityError::CoefficientTranslationOverflow {
                    position,
                },
            );
        }
    }
    let centered_assignment_entries = assignment.entries().len();
    if centered_assignment_entries > 0 {
        check_limit(
            "candidate centered assignment entries",
            centered_assignment_entries,
            limits.max_centered_assignment_entries,
        )?;
        check_limit(
            "candidate centered assignment additions",
            centered_assignment_entries,
            limits.max_centered_assignment_additions,
        )?;
    }
    let centered_assignment_integer_bit_work =
        centered_assignment_integer_bit_work(assignment.entries(), original_pivot_ref)?;
    check_limit(
        "candidate centered assignment integer-bit work",
        centered_assignment_integer_bit_work,
        limits.max_centered_assignment_integer_bit_work,
    )?;

    let row_label_bytes = candidate_row_label_byte_len(pivot_ordinal)?;
    check_limit(
        "candidate row label bytes",
        row_label_bytes,
        limits.max_row_label_bytes,
    )?;
    let retained_base_bytes = candidate_retained_base_byte_bound(
        arity,
        dependency_event_count,
        base_assumption_count,
        centered_assignment_entries,
        row_label_bytes,
    )?;
    check_limit(
        "candidate retained payload bytes",
        retained_base_bytes,
        limits.max_retained_payload_bytes,
    )?;
    let retained_relation_allowance = remaining(
        "candidate retained payload bytes",
        retained_base_bytes,
        limits.max_retained_payload_bytes,
    )?;
    check_limit(
        "candidate recenter attempts",
        1,
        limits.max_recenter_attempts,
    )?;
    // Nested replay is mandatory, but only after the allocation-free shallow
    // census has admitted fingerprints, arity, provenance counts, assignment
    // arithmetic and overflow, row identity, and the fixed retained envelope.
    // No Symbolica recentering has occurred at this point.
    authenticate_source(&source)?;

    let mut dependency_event_ordinals = Vec::new();
    try_reserve_exact(
        "candidate dependency event ordinals",
        &mut dependency_event_ordinals,
        dependency_event_count,
    )?;
    dependency_event_ordinals.extend(
        guarded
            .dependency_events()
            .map(|event| event.event_ordinal()),
    );
    if dependency_event_ordinals.len() != dependency_event_count {
        return Err(GeneratedCylindricalCandidateAuthorityError::GuardedProvenanceUnavailable);
    }
    let mut base_assumption_witness_ordinals = Vec::new();
    try_reserve_exact(
        "candidate base-assumption witness ordinals",
        &mut base_assumption_witness_ordinals,
        base_assumption_count,
    )?;
    base_assumption_witness_ordinals.extend(
        guarded
            .base_assumptions()
            .map(|resolved| resolved.witness().ordinal()),
    );
    if base_assumption_witness_ordinals.len() != base_assumption_count {
        return Err(GeneratedCylindricalCandidateAuthorityError::GuardedProvenanceUnavailable);
    }
    let original_pivot = copy_shift(original_pivot_ref)?;
    let coefficient_translation = negated_shift(original_pivot_ref)?;
    let sector = copy_sector(start.sector())?;
    let centered_assignment =
        centered_assignment(assignment.entries(), original_pivot_ref, arity, limits)?;
    if (arm == CandidateArmTag::Global) != centered_assignment.is_none() {
        return Err(GeneratedCylindricalCandidateAuthorityError::GuardedProvenanceUnavailable);
    }
    let row_id = candidate_row_id(pivot_ordinal, row_label_bytes)?;
    let mut recentering_limits = recentering_limits(limits);
    recentering_limits.max_retained_bytes = recentering_limits
        .max_retained_bytes
        .min(retained_relation_allowance);
    let (centered_relation, recentering_stats) = guarded
        .affine_free_recentered_for_candidate(context, row_id, recentering_limits)
        .map_err(|error| match error {
            ParametricRelationError::IndexOverflow { position }
                if original_pivot_ref.values()[position] == i64::MIN =>
            {
                GeneratedCylindricalCandidateAuthorityError::CoefficientTranslationOverflow {
                    position,
                }
            }
            other => other.into(),
        })?;
    let zero = IndexShift::try_new(std::iter::repeat(0_i64).take(arity), arity)?;
    let Some(centered_pivot_coefficient) = centered_relation.terms().get(&zero) else {
        return Err(GeneratedCylindricalCandidateAuthorityError::MissingCenteredPivot);
    };
    if centered_pivot_coefficient != &context.one() {
        return Err(GeneratedCylindricalCandidateAuthorityError::NonUnitCenteredPivot);
    }
    let centered_rhs_terms = centered_relation
        .terms()
        .len()
        .checked_sub(1)
        .ok_or(GeneratedCylindricalCandidateAuthorityError::MissingCenteredPivot)?;
    check_limit(
        "candidate centered RHS terms",
        centered_rhs_terms,
        limits.max_centered_rhs_terms,
    )?;

    let retained_before_identity = checked_add(
        "candidate retained payload bytes",
        retained_base_bytes,
        recentering_stats.retained_bytes(),
    )?;
    check_limit(
        "candidate retained payload bytes",
        retained_before_identity,
        limits.max_retained_payload_bytes,
    )?;
    let identity_byte_allowance = remaining(
        "candidate retained payload bytes",
        retained_before_identity,
        limits.max_retained_payload_bytes,
    )?;

    let source_event = *guarded.source_event();
    let identity_payload = CandidateIdentityPayload {
        arm,
        source: &source,
        sector: &sector,
        ordering_policy: start.ordering_policy(),
        ordering_identity: source.ordering_identity(),
        pivot_ordinal,
        source_event,
        dependency_event_ordinals: &dependency_event_ordinals,
        base_assumption_witness_ordinals: &base_assumption_witness_ordinals,
        original_pivot: &original_pivot,
        coefficient_translation: &coefficient_translation,
        centered_assignment: centered_assignment.as_ref(),
        centered_relation: &centered_relation,
        limits,
    };
    let mut exact_limits = exact_identity_limits(limits);
    let identity_is_capped_by_retained_payload =
        identity_byte_allowance < exact_limits.max_identity_bytes;
    exact_limits.max_identity_bytes = exact_limits.max_identity_bytes.min(identity_byte_allowance);
    let exact_identity =
        encode_exact_identity(&identity_payload, exact_limits).map_err(|error| {
            if identity_is_capped_by_retained_payload {
                if let ExactIdentityError::ResourceLimit {
                    resource: "exact structural identity bytes",
                    requested,
                    ..
                } = &error
                {
                    return GeneratedCylindricalCandidateAuthorityError::ResourceLimit {
                        resource: "candidate retained payload bytes",
                        requested: retained_before_identity.saturating_add(*requested),
                        limit: limits.max_retained_payload_bytes,
                    };
                }
            }
            map_exact_identity_error(error)
        })?;
    let exact_stats = exact_identity.stats();
    let retained_payload_bytes = checked_add(
        "candidate retained payload bytes",
        retained_before_identity,
        exact_stats.identity_bytes(),
    )?;
    check_limit(
        "candidate retained payload bytes",
        retained_payload_bytes,
        limits.max_retained_payload_bytes,
    )?;

    let (local_replay_comparison_units, local_replay_comparison_bytes) =
        local_replay_comparison_census(
            &centered_relation,
            dependency_event_ordinals.len(),
            base_assumption_witness_ordinals.len(),
            centered_assignment
                .as_ref()
                .map_or(0, |value| value.entries().len()),
            retained_payload_bytes,
            limits,
        )?;
    let mut stats = GeneratedCylindricalCandidateAuthorityStats {
        candidates: 1,
        global_candidates: usize::from(arm == CandidateArmTag::Global),
        locus_bound_candidates: usize::from(arm == CandidateArmTag::LocusBound),
        family_fingerprint_bytes,
        context_fingerprint_bytes,
        ordering_identity_bytes,
        arity,
        pivot_components: original_pivot.arity(),
        pivot_integer_bit_work,
        dependency_events: dependency_event_ordinals.len(),
        base_assumption_references: base_assumption_witness_ordinals.len(),
        base_assumption_origin_references,
        base_assumption_manifest_bytes,
        base_assumption_condition_owned_bytes,
        centered_assignment_entries: centered_assignment
            .as_ref()
            .map_or(0, |value| value.entries().len()),
        centered_assignment_additions: centered_assignment
            .as_ref()
            .map_or(0, |value| value.entries().len()),
        centered_assignment_integer_bit_work,
        row_label_bytes,
        centered_rhs_terms,
        recenter_attempts: 1,
        recenter_terms: recentering_stats.terms(),
        recenter_guards: recentering_stats.guards(),
        recenter_translation_components: recentering_stats.translation_components(),
        recenter_key_subtraction_boundary_checks: recentering_stats
            .key_subtraction_boundary_checks(),
        recenter_source_terms: recentering_stats.source_terms(),
        recenter_source_exponent_entries: recentering_stats.source_exponent_entries(),
        recenter_output_terms: recentering_stats.output_terms(),
        recenter_output_exponent_entries: recentering_stats.output_exponent_entries(),
        recenter_power_operations: recentering_stats.power_operations(),
        recenter_integer_bit_work: recentering_stats.integer_bit_work(),
        recenter_normalized_coefficient_terms: recentering_stats.normalized_coefficient_terms(),
        recenter_retained_bytes: recentering_stats.retained_bytes(),
        retained_payload_bytes,
        local_replay_comparison_units,
        local_replay_comparison_bytes,
        ..Default::default()
    };
    retain_exact_identity_stats(&mut stats, exact_stats);

    let family_fingerprint = Arc::clone(row_system.family_fingerprint_arc());
    let context_fingerprint = Arc::clone(row_system.context_fingerprint_arc());
    let ordering_authority = GeneratedCylindricalCandidateOrderingAuthority::CylindricalV1 {
        policy: start.ordering_policy(),
        identity: Arc::clone(start.schedule().ordering().stable_manifest_arc()),
    };
    let binding = Arc::new(GeneratedCylindricalCandidateBinding {
        schema: GENERATED_CYLINDRICAL_CANDIDATE_AUTHORITY_V1_SCHEMA,
        arm,
        family_fingerprint,
        context_fingerprint,
        source,
        sector,
        ordering_authority,
        pivot_ordinal,
        source_event,
        dependency_event_ordinals: dependency_event_ordinals.into_boxed_slice(),
        base_assumption_witness_ordinals: base_assumption_witness_ordinals.into_boxed_slice(),
        original_pivot,
        coefficient_translation,
        centered_assignment,
        centered_relation: Arc::new(centered_relation),
        recentering_stats,
        exact_identity,
        limits,
        stats,
    });
    Ok(match arm {
        CandidateArmTag::Global => GeneratedCylindricalCandidateAuthority::Global(
            GeneratedCylindricalGlobalCandidateAuthority { binding },
        ),
        CandidateArmTag::LocusBound => GeneratedCylindricalCandidateAuthority::LocusBound(
            GeneratedCylindricalLocusBoundCandidateAuthority { binding },
        ),
    })
}

/// Test-only owner-equivalent entry point retained for the exhaustive
/// resource/error-precedence matrix. Production callers must select an
/// operation-scoped authentication capability explicitly.
#[cfg(test)]
fn compile_inner(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    source: Arc<GeneratedCylindricalPersistentEliminationCertificate>,
    pivot_ordinal: usize,
    limits: GeneratedCylindricalCandidateAuthorityLimits,
) -> Result<GeneratedCylindricalCandidateAuthority, GeneratedCylindricalCandidateAuthorityError> {
    compile_inner_with_source_authenticator(
        family,
        context,
        source,
        pivot_ordinal,
        limits,
        |source| source.replay(family, context).map_err(Into::into),
    )
}

fn centered_assignment(
    source_entries: &[(usize, i64)],
    pivot: &IndexShift,
    arity: usize,
    limits: GeneratedCylindricalCandidateAuthorityLimits,
) -> Result<
    Option<GeneratedCylindricalCenteredAssignment>,
    GeneratedCylindricalCandidateAuthorityError,
> {
    if source_entries.is_empty() {
        return Ok(None);
    }
    check_limit(
        "candidate centered assignment entries",
        source_entries.len(),
        limits.max_centered_assignment_entries,
    )?;
    check_limit(
        "candidate centered assignment additions",
        source_entries.len(),
        limits.max_centered_assignment_additions,
    )?;
    // Validate every lattice addition before acquiring the retained buffer.
    for &(position, value) in source_entries {
        value.checked_add(pivot.values()[position]).ok_or(
            GeneratedCylindricalCandidateAuthorityError::CenteredAssignmentOverflow { position },
        )?;
    }
    let mut entries = Vec::new();
    try_reserve_exact(
        "candidate centered assignment entries",
        &mut entries,
        source_entries.len(),
    )?;
    let mut bit_work = 0usize;
    for &(position, value) in source_entries {
        let shift = pivot.values()[position];
        let centered = value.checked_add(shift).ok_or(
            GeneratedCylindricalCandidateAuthorityError::CenteredAssignmentOverflow { position },
        )?;
        for operand in [value, shift, centered] {
            bit_work = bounded_add(
                "candidate centered assignment integer-bit work",
                bit_work,
                i64_magnitude_bits(operand),
                limits.max_centered_assignment_integer_bit_work,
            )?;
        }
        entries.push((position, centered));
    }
    Ok(Some(GeneratedCylindricalCenteredAssignment {
        arity,
        entries: entries.into_boxed_slice(),
    }))
}

fn centered_assignment_integer_bit_work(
    source_entries: &[(usize, i64)],
    pivot: &IndexShift,
) -> Result<usize, GeneratedCylindricalCandidateAuthorityError> {
    let mut work = 0usize;
    for &(position, value) in source_entries {
        let shift = pivot.values()[position];
        let centered = value.checked_add(shift).ok_or(
            GeneratedCylindricalCandidateAuthorityError::CenteredAssignmentOverflow { position },
        )?;
        for operand in [value, shift, centered] {
            work = checked_add(
                "candidate centered assignment integer-bit work",
                work,
                i64_magnitude_bits(operand),
            )?;
        }
    }
    Ok(work)
}

struct CandidateIdentityPayload<'a> {
    arm: CandidateArmTag,
    source: &'a GeneratedCylindricalPersistentEliminationCertificate,
    sector: &'a SectorMask,
    ordering_policy: IntegralOrderingPolicy,
    ordering_identity: &'a str,
    pivot_ordinal: usize,
    source_event: GeneratedCylindricalPersistentEliminationEvent,
    dependency_event_ordinals: &'a [usize],
    base_assumption_witness_ordinals: &'a [usize],
    original_pivot: &'a IndexShift,
    coefficient_translation: &'a IndexShift,
    centered_assignment: Option<&'a GeneratedCylindricalCenteredAssignment>,
    centered_relation: &'a ParametricRelation,
    limits: GeneratedCylindricalCandidateAuthorityLimits,
}

impl ExactIdentityPayload for CandidateIdentityPayload<'_> {
    const SCHEMA: &'static str = GENERATED_CYLINDRICAL_CANDIDATE_AUTHORITY_V1_SCHEMA;

    fn write_exact_identity(
        &self,
        writer: &mut ExactIdentityWriter<'_>,
    ) -> Result<(), ExactIdentityError> {
        let start = self.source.row_system().start();
        let (start_schema, start_arm) = match start {
            GeneratedCylindricalRowSystemStartCertificate::Residual(_) => (
                GENERATED_CYLINDRICAL_RESIDUAL_START_V1_SCHEMA,
                "residual-v1",
            ),
            GeneratedCylindricalRowSystemStartCertificate::SectorRoot(_) => (
                GENERATED_CYLINDRICAL_SECTOR_ROOT_START_V1_SCHEMA,
                "sector-root-v1",
            ),
        };
        writer.begin_record("candidate", 23)?;
        writer.string("persistent_schema", self.source.schema())?;
        writer.string("row_system_schema", self.source.row_system().schema())?;
        writer.string("start_schema", start_schema)?;
        writer.variant("start_arm", start_arm)?;
        writer.string("family", self.source.family_fingerprint())?;
        writer.string("context", self.source.context_fingerprint())?;
        write_sector_identity(writer, "sector", self.sector)?;
        write_assignment_identity(
            writer,
            "source_assignment",
            start.assignment().arity(),
            start.assignment().entries(),
        )?;
        writer.variant("ordering_authority", "cylindrical-v1")?;
        writer.string("ordering_policy", self.ordering_policy.stable_id())?;
        writer.string("ordering_identity", self.ordering_identity)?;
        writer.variant("candidate_arm", self.arm.stable_name())?;
        writer.usize("pivot_ordinal", self.pivot_ordinal)?;
        write_event_identity(writer, "source_event", self.source_event)?;
        writer.begin_sequence("dependency_events", self.dependency_event_ordinals.len())?;
        for &ordinal in self.dependency_event_ordinals {
            let event = *self
                .source
                .events()
                .get(ordinal)
                .expect("replayed dependency event ordinal");
            write_event_identity(writer, "event", event)?;
        }
        writer.end_sequence()?;
        writer.begin_sequence(
            "base_assumptions",
            self.base_assumption_witness_ordinals.len(),
        )?;
        for &ordinal in self.base_assumption_witness_ordinals {
            let witness = self
                .source
                .base_assumptions()
                .get(ordinal)
                .expect("replayed base-assumption witness ordinal");
            writer.begin_record("assumption", 7)?;
            writer.usize("ordinal", witness.ordinal())?;
            writer.usize("retained_source_ordinal", witness.retained_source_ordinal())?;
            writer.usize("expanded_ordinal", witness.expanded_ordinal())?;
            writer.usize("assumption_ordinal", witness.assumption_ordinal())?;
            writer.string("manifest", witness.manifest())?;
            writer.usize("origin_count", witness.origin_count())?;
            writer.usize("condition_owned_bytes", witness.condition_owned_bytes())?;
            writer.end_record()?;
        }
        writer.end_sequence()?;
        write_shift_identity(writer, "original_pivot", self.original_pivot)?;
        write_shift_identity(
            writer,
            "coefficient_translation",
            self.coefficient_translation,
        )?;
        write_shift_identity(writer, "key_center", self.original_pivot)?;
        match self.centered_assignment {
            None => {
                writer.begin_record("centered_assignment", 1)?;
                writer.variant("kind", "none")?;
                writer.end_record()?;
            }
            Some(assignment) => {
                writer.begin_record("centered_assignment", 2)?;
                writer.variant("kind", "sparse-equality-locus")?;
                write_assignment_identity(
                    writer,
                    "assignment",
                    assignment.arity(),
                    assignment.entries(),
                )?;
                writer.end_record()?;
            }
        }
        writer.parametric_relation("centered_relation", self.centered_relation)?;
        write_limits_identity(writer, self.limits)?;
        writer.identity_stats("identity_census")?;
        writer.end_record()
    }
}

fn write_sector_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    sector: &SectorMask,
) -> Result<(), ExactIdentityError> {
    writer.begin_sequence(tag, sector.active_bits().len())?;
    for &active in sector.active_bits() {
        writer.boolean("active", active)?;
    }
    writer.end_sequence()
}

fn write_assignment_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    arity: usize,
    entries: &[(usize, i64)],
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 2)?;
    writer.usize("arity", arity)?;
    writer.begin_sequence("entries", entries.len())?;
    for &(position, value) in entries {
        writer.begin_record("entry", 2)?;
        writer.usize("position", position)?;
        writer.signed_i64("value", value)?;
        writer.end_record()?;
    }
    writer.end_sequence()?;
    writer.end_record()
}

fn write_shift_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    shift: &IndexShift,
) -> Result<(), ExactIdentityError> {
    writer.begin_sequence(tag, shift.arity())?;
    for &component in shift.values() {
        writer.signed_i64("component", component)?;
    }
    writer.end_sequence()
}

fn write_event_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    event: GeneratedCylindricalPersistentEliminationEvent,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 13)?;
    writer.usize("event_ordinal", event.event_ordinal())?;
    writer.usize("batch_ordinal", event.batch_ordinal())?;
    writer.usize("within_batch_ordinal", event.within_batch_ordinal())?;
    writer.usize("retained_source_ordinal", event.retained_source_ordinal())?;
    writer.usize("expanded_ordinal", event.expanded_ordinal())?;
    writer.usize("layer_ordinal", event.layer_ordinal())?;
    writer.usize("depth", event.depth())?;
    writer.usize("prepare_point_ordinal", event.prepare_point_ordinal())?;
    writer.usize(
        "generated_source_row_ordinal",
        event.generated_source_row_ordinal(),
    )?;
    writer.usize(
        "first_base_assumption_ordinal",
        event.first_base_assumption_ordinal(),
    )?;
    writer.usize("base_assumption_count", event.base_assumption_count())?;
    writer.usize("prefix_column_count", event.prefix_column_count())?;
    write_event_outcome_identity(writer, event.outcome())?;
    writer.end_record()
}

fn write_event_outcome_identity(
    writer: &mut ExactIdentityWriter<'_>,
    outcome: crate::GeneratedCylindricalPersistentEliminationRowOutcome,
) -> Result<(), ExactIdentityError> {
    match outcome {
        crate::GeneratedCylindricalPersistentEliminationRowOutcome::Pivot { pivot_ordinal } => {
            writer.begin_record("outcome", 2)?;
            writer.variant("kind", "pivot")?;
            writer.begin_record("pivot", 1)?;
            writer.usize("ordinal", pivot_ordinal)?;
            writer.end_record()?;
        }
        crate::GeneratedCylindricalPersistentEliminationRowOutcome::Dependent => {
            writer.begin_record("outcome", 2)?;
            writer.variant("kind", "dependent")?;
            // Absence is part of the grammar, not a platform-sized sentinel.
            writer.begin_record("pivot", 0)?;
            writer.end_record()?;
        }
    }
    writer.end_record()
}

fn write_limits_identity(
    writer: &mut ExactIdentityWriter<'_>,
    limits: GeneratedCylindricalCandidateAuthorityLimits,
) -> Result<(), ExactIdentityError> {
    writer.begin_record("limits", 46)?;
    writer.begin_record("arithmetic", 6)?;
    writer.begin_record("exact_algebra", 3)?;
    writer.u128("max_exponent", limits.arithmetic.exact_algebra.max_exponent)?;
    writer.usize(
        "max_polynomial_terms",
        limits.arithmetic.exact_algebra.max_polynomial_terms,
    )?;
    writer.usize(
        "max_term_operations",
        limits.arithmetic.exact_algebra.max_term_operations,
    )?;
    writer.end_record()?;
    writer.usize("max_source_terms", limits.arithmetic.max_source_terms)?;
    writer.usize("max_output_terms", limits.arithmetic.max_output_terms)?;
    writer.usize(
        "max_specialization_power_operations",
        limits.arithmetic.max_specialization_power_operations,
    )?;
    writer.usize(
        "max_specialization_integer_bits",
        limits.arithmetic.max_specialization_integer_bits,
    )?;
    writer.usize("max_guard_origins", limits.arithmetic.max_guard_origins)?;
    writer.end_record()?;
    macro_rules! limit_fields {
        ($($field:ident),+ $(,)?) => {$ (
            writer.usize(stringify!($field), limits.$field)?;
        )+ };
    }
    limit_fields!(
        max_candidates,
        max_family_fingerprint_bytes,
        max_context_fingerprint_bytes,
        max_ordering_identity_bytes,
        max_arity,
        max_pivot_components,
        max_pivot_integer_bit_work,
        max_dependency_events,
        max_base_assumption_references,
        max_base_assumption_origin_references,
        max_base_assumption_manifest_bytes,
        max_base_assumption_condition_owned_bytes,
        max_centered_assignment_entries,
        max_centered_assignment_additions,
        max_centered_assignment_integer_bit_work,
        max_row_label_bytes,
        max_centered_rhs_terms,
        max_recenter_attempts,
        max_recenter_terms,
        max_recenter_guards,
        max_recenter_translation_components,
        max_recenter_key_subtraction_boundary_checks,
        max_recenter_source_terms,
        max_recenter_source_exponent_entries,
        max_recenter_output_terms,
        max_recenter_output_exponent_entries,
        max_recenter_power_operations,
        max_recenter_integer_bit_work,
        max_recenter_normalized_coefficient_terms,
        max_recenter_retained_bytes,
        max_retained_payload_bytes,
        max_local_replay_comparison_units,
        max_local_replay_comparison_bytes,
        max_exact_identity_bytes,
        max_exact_identity_fields,
        max_exact_identity_tag_bytes,
        max_exact_identity_string_values,
        max_exact_identity_string_bytes,
        max_exact_identity_nesting_depth,
        max_exact_identity_polynomials,
        max_exact_identity_polynomial_variables,
        max_exact_identity_polynomial_terms,
        max_exact_identity_exponent_entries,
        max_exact_identity_integers,
        max_exact_identity_integer_bits,
    );
    writer.end_record()
}

fn recentering_limits(
    limits: GeneratedCylindricalCandidateAuthorityLimits,
) -> ParametricAffineFreeRecenteringLimits {
    ParametricAffineFreeRecenteringLimits {
        arithmetic: limits.arithmetic,
        max_terms: limits.max_recenter_terms,
        max_guards: limits.max_recenter_guards,
        max_translation_components: limits.max_recenter_translation_components,
        max_key_subtraction_boundary_checks: limits.max_recenter_key_subtraction_boundary_checks,
        max_source_terms: limits.max_recenter_source_terms,
        max_source_exponent_entries: limits.max_recenter_source_exponent_entries,
        max_output_terms: limits.max_recenter_output_terms,
        max_output_exponent_entries: limits.max_recenter_output_exponent_entries,
        max_power_operations: limits.max_recenter_power_operations,
        max_integer_bit_work: limits.max_recenter_integer_bit_work,
        max_normalized_coefficient_terms: limits.max_recenter_normalized_coefficient_terms,
        max_retained_bytes: limits.max_recenter_retained_bytes,
    }
}

fn exact_identity_limits(
    limits: GeneratedCylindricalCandidateAuthorityLimits,
) -> ExactIdentityLimits {
    ExactIdentityLimits {
        max_identity_bytes: limits.max_exact_identity_bytes,
        max_fields: limits.max_exact_identity_fields,
        max_tag_bytes: limits.max_exact_identity_tag_bytes,
        max_string_values: limits.max_exact_identity_string_values,
        max_string_bytes: limits.max_exact_identity_string_bytes,
        max_nesting_depth: limits.max_exact_identity_nesting_depth,
        max_polynomials: limits.max_exact_identity_polynomials,
        max_polynomial_variables: limits.max_exact_identity_polynomial_variables,
        max_polynomial_terms: limits.max_exact_identity_polynomial_terms,
        max_exponent_entries: limits.max_exact_identity_exponent_entries,
        max_integers: limits.max_exact_identity_integers,
        max_integer_bits: limits.max_exact_identity_integer_bits,
    }
}

fn retain_exact_identity_stats(
    stats: &mut GeneratedCylindricalCandidateAuthorityStats,
    identity: ExactIdentityStats,
) {
    stats.exact_identity_bytes = identity.identity_bytes();
    stats.exact_identity_fields = identity.fields();
    stats.exact_identity_tag_bytes = identity.tag_bytes();
    stats.exact_identity_string_values = identity.string_values();
    stats.exact_identity_string_bytes = identity.string_bytes();
    stats.exact_identity_maximum_nesting_depth = identity.maximum_nesting_depth();
    stats.exact_identity_polynomials = identity.polynomials();
    stats.exact_identity_polynomial_variables = identity.polynomial_variables();
    stats.exact_identity_polynomial_terms = identity.polynomial_terms();
    stats.exact_identity_exponent_entries = identity.exponent_entries();
    stats.exact_identity_integers = identity.integers();
    stats.exact_identity_integer_bits = identity.integer_bits();
}

fn map_exact_identity_error(
    error: ExactIdentityError,
) -> GeneratedCylindricalCandidateAuthorityError {
    match error {
        ExactIdentityError::ResourceLimit {
            resource,
            requested,
            limit,
        } => GeneratedCylindricalCandidateAuthorityError::ResourceLimit {
            resource,
            requested,
            limit,
        },
        ExactIdentityError::ResourceCountOverflow { resource } => {
            GeneratedCylindricalCandidateAuthorityError::ResourceCountOverflow { resource }
        }
        ExactIdentityError::AllocationFailure {
            resource,
            requested,
        } => GeneratedCylindricalCandidateAuthorityError::AllocationFailure {
            resource,
            requested,
        },
        other => GeneratedCylindricalCandidateAuthorityError::ExactIdentityFailure {
            detail: other.to_string(),
        },
    }
}

fn local_replay_comparison_census(
    relation: &ParametricRelation,
    dependency_events: usize,
    base_assumptions: usize,
    centered_assignment_entries: usize,
    retained_payload_bytes: usize,
    limits: GeneratedCylindricalCandidateAuthorityLimits,
) -> Result<(usize, usize), GeneratedCylindricalCandidateAuthorityError> {
    let mut units = 24usize;
    for count in [
        dependency_events,
        base_assumptions,
        centered_assignment_entries,
        relation.terms().len(),
        relation.guarded_nonzero_conditions().len(),
    ] {
        units = checked_add("candidate local replay comparison units", units, count)?;
    }
    check_limit(
        "candidate local replay comparison units",
        units,
        limits.max_local_replay_comparison_units,
    )?;
    // This is deliberately local. Complete nested persistent/row-system
    // comparison is performed and bounded by `source.replay`/`source.payload_eq`.
    let bytes = retained_payload_bytes;
    check_limit(
        "candidate local replay comparison bytes",
        bytes,
        limits.max_local_replay_comparison_bytes,
    )?;
    Ok((units, bytes))
}

fn candidate_retained_base_byte_bound(
    arity: usize,
    dependency_events: usize,
    base_assumptions: usize,
    centered_assignment_entries: usize,
    row_label_bytes: usize,
) -> Result<usize, GeneratedCylindricalCandidateAuthorityError> {
    let mut bytes = checked_add(
        "candidate retained payload bytes",
        size_of::<GeneratedCylindricalCandidateAuthority>(),
        size_of::<GeneratedCylindricalCandidateBinding>(),
    )?;
    bytes = checked_add(
        "candidate retained payload bytes",
        bytes,
        arc_control_and_padding_byte_bound()?,
    )?;
    bytes = checked_add(
        "candidate retained payload bytes",
        bytes,
        arc_control_and_padding_byte_bound()?,
    )?;
    bytes = checked_add(
        "candidate retained payload bytes",
        bytes,
        arc_control_and_padding_byte_bound()?,
    )?;
    bytes = checked_add(
        "candidate retained payload bytes",
        bytes,
        size_of::<String>(),
    )?;
    for (count, element_size) in [
        (arity, size_of::<bool>()),
        (arity, size_of::<i64>()),
        (arity, size_of::<i64>()),
        (dependency_events, size_of::<usize>()),
        (base_assumptions, size_of::<usize>()),
        (centered_assignment_entries, size_of::<(usize, i64)>()),
    ] {
        bytes = checked_add(
            "candidate retained payload bytes",
            bytes,
            checked_mul("candidate retained payload bytes", count, element_size)?,
        )?;
    }
    // The derived row label is the only new Arc<str> payload excluded from
    // the relation helper's owned envelope.
    checked_add(
        "candidate retained payload bytes",
        bytes,
        checked_add(
            "candidate retained payload bytes",
            arc_control_and_padding_byte_bound()?,
            row_label_bytes,
        )?,
    )
}

fn copy_shift(
    source: &IndexShift,
) -> Result<IndexShift, GeneratedCylindricalCandidateAuthorityError> {
    let mut values = Vec::new();
    try_reserve_exact("candidate pivot components", &mut values, source.arity())?;
    values.extend_from_slice(source.values());
    IndexShift::try_from_preallocated(values, source.arity()).map_err(Into::into)
}

fn negated_shift(
    source: &IndexShift,
) -> Result<IndexShift, GeneratedCylindricalCandidateAuthorityError> {
    // Reject the unique non-negatable component before acquiring the output
    // allocation.
    for (position, &value) in source.values().iter().enumerate() {
        if value == i64::MIN {
            return Err(
                GeneratedCylindricalCandidateAuthorityError::CoefficientTranslationOverflow {
                    position,
                },
            );
        }
    }
    let mut values = Vec::new();
    try_reserve_exact(
        "candidate coefficient-translation components",
        &mut values,
        source.arity(),
    )?;
    for (position, &value) in source.values().iter().enumerate() {
        values.push(value.checked_neg().ok_or(
            GeneratedCylindricalCandidateAuthorityError::CoefficientTranslationOverflow {
                position,
            },
        )?);
    }
    IndexShift::try_from_preallocated(values, source.arity()).map_err(Into::into)
}

fn copy_sector(
    source: &SectorMask,
) -> Result<SectorMask, GeneratedCylindricalCandidateAuthorityError> {
    let mut active = Vec::new();
    try_reserve_exact("candidate sector bits", &mut active, source.arity())?;
    active.extend_from_slice(source.active_bits());
    SectorMask::try_from_preallocated(active).map_err(|error| {
        GeneratedCylindricalCandidateAuthorityError::ReplayMismatch {
            detail: match error {
                crate::SectorFoundationError::EmptyIndexSpace => "source sector became empty",
                _ => "source sector copy failed",
            },
        }
    })
}

fn candidate_row_label_byte_len(
    pivot_ordinal: usize,
) -> Result<usize, GeneratedCylindricalCandidateAuthorityError> {
    checked_add(
        "candidate row label bytes",
        "generated-cylindrical-candidate-pivot-".len(),
        decimal_digits(pivot_ordinal),
    )
}

fn candidate_row_id(
    pivot_ordinal: usize,
    exact_bytes: usize,
) -> Result<ParametricRowId, GeneratedCylindricalCandidateAuthorityError> {
    let mut label = String::new();
    label.try_reserve_exact(exact_bytes).map_err(|_| {
        GeneratedCylindricalCandidateAuthorityError::AllocationFailure {
            resource: "candidate row label bytes",
            requested: exact_bytes,
        }
    })?;
    write!(
        label,
        "generated-cylindrical-candidate-pivot-{pivot_ordinal}"
    )
    .map_err(
        |_| GeneratedCylindricalCandidateAuthorityError::AllocationFailure {
            resource: "candidate row label bytes",
            requested: exact_bytes,
        },
    )?;
    if label.len() != exact_bytes {
        return Err(
            GeneratedCylindricalCandidateAuthorityError::ReplayMismatch {
                detail: "candidate row-label census changed",
            },
        );
    }
    Ok(ParametricRowId::Derived {
        label: Arc::from(label),
    })
}

fn arc_control_and_padding_byte_bound() -> Result<usize, GeneratedCylindricalCandidateAuthorityError>
{
    size_of::<usize>()
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(align_of::<usize>().saturating_sub(1)))
        .ok_or(
            GeneratedCylindricalCandidateAuthorityError::ResourceCountOverflow {
                resource: "candidate retained payload bytes",
            },
        )
}

fn decimal_digits(mut value: usize) -> usize {
    let mut digits = 1usize;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

fn i64_magnitude_bits(value: i64) -> usize {
    ((i64::BITS - value.unsigned_abs().leading_zeros()) as usize).max(1)
}

fn portable_limit(preferred: u128) -> usize {
    usize::try_from(preferred).unwrap_or(usize::MAX)
}

fn generated_cylindrical_persistent_source_address(
    source: &Arc<GeneratedCylindricalPersistentEliminationCertificate>,
) -> usize {
    Arc::as_ptr(source) as usize
}

fn remaining(
    resource: &'static str,
    used: usize,
    limit: usize,
) -> Result<usize, GeneratedCylindricalCandidateAuthorityError> {
    limit
        .checked_sub(used)
        .ok_or(GeneratedCylindricalCandidateAuthorityError::ResourceLimit {
            resource,
            requested: used,
            limit,
        })
}

fn try_reserve_exact<T>(
    resource: &'static str,
    values: &mut Vec<T>,
    additional: usize,
) -> Result<(), GeneratedCylindricalCandidateAuthorityError> {
    let requested = checked_add(resource, values.len(), additional)?;
    values.try_reserve_exact(additional).map_err(|_| {
        GeneratedCylindricalCandidateAuthorityError::AllocationFailure {
            resource,
            requested,
        }
    })
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedCylindricalCandidateAuthorityError> {
    left.checked_add(right)
        .ok_or(GeneratedCylindricalCandidateAuthorityError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedCylindricalCandidateAuthorityError> {
    left.checked_mul(right)
        .ok_or(GeneratedCylindricalCandidateAuthorityError::ResourceCountOverflow { resource })
}

fn bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, GeneratedCylindricalCandidateAuthorityError> {
    let requested = checked_add(resource, left, right)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedCylindricalCandidateAuthorityError> {
    if requested > limit {
        Err(GeneratedCylindricalCandidateAuthorityError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::when_bad::{
        WHEN_BAD_COMPILER_V2_SCHEMA, WhenBadCandidateSourceAuthority, WhenBadCompiler,
        WhenBadCompilerError, WhenBadCompilerLimits, WhenBadCoreCompilation,
        WhenBadDomainConditionSource, WhenBadOrderingAuthority, WhenBadSourceAuthentication,
    };
    use crate::{
        AffineDenominator, CoefficientContext, ExactAlgebraLimits, FamilySectorInventoryCompiler,
        FamilySectorInventoryLimits, GeneratedCylindricalPersistentEliminationLimits,
        GeneratedCylindricalResidualStartCertificate, GeneratedCylindricalResidualStartLimits,
        GeneratedCylindricalRowSystemCertificate, GeneratedCylindricalRowSystemLimits,
        GeneratedCylindricalSectorCoverageAttempt, GeneratedCylindricalSectorCoverageCompiler,
        GeneratedCylindricalSectorCoverageLimits, GeneratedCylindricalSectorRootStartCertificate,
        GeneratedCylindricalSectorRootStartLimits, GeneratedCylindricalSectorRuleProvider,
        GeneratedCylindricalSectorRuleProviderLimits, GeneratedCylindricalWhenBadCompilation,
        GeneratedCylindricalWhenBadCompiler, GeneratedSectorDiscoveryCompiler,
        GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafQueueCompiler,
        GeneratedSectorLiveLeafQueueLimits, GeneratedSymbolicRowSpanConfig, IntegralFamily,
        ParametricIbpConfig, ParametricIbpGenerator, PowerShiftPolicy, SectorRestrictions,
        SymbolicPolynomialPredicateKind, WhenBadLeafDisposition,
    };
    use crate::{ConcreteRuleDecision, ConcreteRuleProvider, ConcreteTerminalStatus};

    // Exact work censuses for the one-pass guarded external-bubble
    // elimination. The underlying elimination and persistent-certificate
    // suites separately exercise exact-limit success and one-below failure
    // for these fields; repeating those late failures here would rebuild this
    // deliberately expensive fixture several additional times.
    const FIXTURE_ONE_PASS_CONSTRUCTION_INTEGER_BIT_WORK: usize = 129_362_930_506_106_837;
    const FIXTURE_ONE_PASS_REPLAY_INTEGER_BIT_WORK: usize = 341_650_130_121_813_484;

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

    fn guarded_external_bubble(name: &str) -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "a", "b", "s", "g"]);
        IntegralFamily::new(
            name,
            vec!["k".into()],
            vec!["p".into()],
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(
                    coefficients.zero(),
                    vec![coefficients.parse("a/s").unwrap(), coefficients.one()],
                ),
                AffineDenominator::new(
                    coefficients.zero(),
                    vec![
                        coefficients.parameter("b").unwrap(),
                        coefficients.integer(2),
                    ],
                ),
            ],
            vec![vec![coefficients.parameter("g").unwrap()]],
            vec![coefficients.zero(), coefficients.zero()],
        )
        .unwrap()
    }

    fn sector_root_source(
        family: IntegralFamily,
        sector: SectorMask,
        through_depth: usize,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedCylindricalPersistentEliminationCertificate>,
    ) {
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
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
                through_depth,
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
            .max_replay_coefficient_integer_bit_work = FIXTURE_ONE_PASS_REPLAY_INTEGER_BIT_WORK;
        persistent_limits
            .elimination
            .max_construction_coefficient_integer_bit_work =
            FIXTURE_ONE_PASS_CONSTRUCTION_INTEGER_BIT_WORK;
        persistent_limits.max_cumulative_construction_coefficient_integer_bit_work =
            FIXTURE_ONE_PASS_CONSTRUCTION_INTEGER_BIT_WORK;
        persistent_limits.max_cumulative_replay_coefficient_integer_bit_work =
            FIXTURE_ONE_PASS_REPLAY_INTEGER_BIT_WORK;
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

    fn tadpole_sector_root(
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedCylindricalPersistentEliminationCertificate>,
    ) {
        sector_root_source(
            massive_tadpole(name),
            SectorMask::try_new([true]).unwrap(),
            1,
        )
    }

    fn tadpole_locus_source(
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
        let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
        discovery_limits.adaptive.max_search_depth = 0;
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_new([true]).unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            discovery_limits,
        )
        .unwrap();
        let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
        queue_limits.translation_radius = 0;
        queue_limits.max_translation_points = 1;
        let queue = Arc::new(
            GeneratedSectorLiveLeafQueueCompiler::compile(
                &family,
                &context,
                &discovery,
                queue_limits,
            )
            .unwrap(),
        );
        let item_ordinal = queue
            .work_items()
            .iter()
            .find(|item| {
                !item.extraction().assignment().is_empty()
                    && !item
                        .extraction()
                        .unresolved_predicates()
                        .iter()
                        .any(|predicate| {
                            predicate.kind() == SymbolicPolynomialPredicateKind::EqualZero
                        })
            })
            .expect("tadpole residual must contain an independent integer locus")
            .ordinal();
        let start = Arc::new(
            GeneratedCylindricalResidualStartCertificate::compile(
                &family,
                &context,
                queue,
                item_ordinal,
                1,
                GeneratedCylindricalResidualStartLimits::default(),
            )
            .unwrap(),
        );
        assert!(!start.assignment().is_empty());
        let rows = Arc::new(
            GeneratedCylindricalRowSystemCertificate::compile(
                &family,
                &context,
                start,
                GeneratedCylindricalRowSystemLimits::default(),
            )
            .unwrap(),
        );
        let source = Arc::new(
            GeneratedCylindricalPersistentEliminationCertificate::compile(
                &family,
                &context,
                rows,
                GeneratedCylindricalPersistentEliminationLimits::default(),
            )
            .unwrap(),
        );
        (family, context, source)
    }

    fn forward_pivot_ordinal(
        source: &GeneratedCylindricalPersistentEliminationCertificate,
    ) -> usize {
        source
            .guarded_pivots()
            .find(|pivot| pivot.original_pivot().values() == [1])
            .or_else(|| source.guarded_pivots().next())
            .expect("fixture must retain a guarded pivot")
            .ordinal()
    }

    #[test]
    fn replay_session_pointer_index_is_exact_first_use_bounded_and_transactional() {
        let (family, context, source) =
            tadpole_sector_root("candidate-authority-replay-session-pointer-index");
        let distinct_source = Arc::new(source.as_ref().clone());
        assert!(!Arc::ptr_eq(&source, &distinct_source));
        assert!(source.payload_eq(&distinct_source));

        reset_operation_scoped_persistent_source_replay_count_for_test();
        let mut baseline = GeneratedCylindricalReplaySession::new(&family, &context);
        baseline
            .authenticate_sources(&[&distinct_source, &source, &distinct_source])
            .unwrap();
        assert_eq!(
            operation_scoped_persistent_source_replay_count_for_test(),
            2
        );
        assert_eq!(baseline.replayed_sources.len(), 2);
        assert!(Arc::ptr_eq(&baseline.replayed_sources[0], &distinct_source));
        assert!(Arc::ptr_eq(&baseline.replayed_sources[1], &source));
        assert_eq!(baseline.replayed_source_pointer_index.len(), 2);
        assert!(
            baseline
                .replayed_source_pointer_index
                .windows(2)
                .all(|entries| entries[0].address < entries[1].address)
        );
        assert!(Arc::ptr_eq(
            baseline.replayed_source(&distinct_source).unwrap().source(),
            &distinct_source,
        ));
        assert!(Arc::ptr_eq(
            baseline.replayed_source(&source).unwrap().source(),
            &source,
        ));
        let exact_reference_bytes = baseline.source_reference_bytes().unwrap();
        let exact_pointer_index_bytes = baseline.source_pointer_index_bytes().unwrap();
        assert!(exact_reference_bytes > 0);
        assert!(exact_pointer_index_bytes > 0);

        // Reversed/repeated inputs neither replay nor reorder allocations that
        // already own capabilities in this operation.
        baseline
            .authenticate_sources(&[&source, &distinct_source, &source])
            .unwrap();
        assert_eq!(
            operation_scoped_persistent_source_replay_count_for_test(),
            2
        );
        assert!(Arc::ptr_eq(&baseline.replayed_sources[0], &distinct_source));
        assert!(Arc::ptr_eq(&baseline.replayed_sources[1], &source));

        let mut exact = GeneratedCylindricalReplaySession::new(&family, &context);
        exact
            .authenticate_sources_with_table_byte_limits(
                &[&distinct_source, &source],
                exact_reference_bytes,
                exact_pointer_index_bytes,
            )
            .unwrap();
        assert_eq!(
            exact.source_reference_bytes().unwrap(),
            exact_reference_bytes
        );
        assert_eq!(
            exact.source_pointer_index_bytes().unwrap(),
            exact_pointer_index_bytes
        );

        reset_operation_scoped_persistent_source_replay_count_for_test();
        let mut one_below = GeneratedCylindricalReplaySession::new(&family, &context);
        assert!(matches!(
            one_below.authenticate_sources_with_table_byte_limits(
                &[&distinct_source, &source],
                exact_reference_bytes,
                exact_pointer_index_bytes - 1,
            ),
            Err(GeneratedCylindricalCandidateAuthorityError::ResourceLimit {
                resource: "operation-scoped persistent-source pointer-index bytes",
                requested,
                limit,
            }) if requested == exact_pointer_index_bytes && limit + 1 == requested
        ));
        assert_eq!(
            operation_scoped_persistent_source_replay_count_for_test(),
            0
        );
        assert!(one_below.replayed_sources.is_empty());
        assert!(one_below.replayed_source_pointer_index.is_empty());

        // A failed extension preserves the previously published exact
        // capability while withholding the new allocation entirely.
        let mut incremental = GeneratedCylindricalReplaySession::new(&family, &context);
        incremental.authenticate_source(&source).unwrap();
        let one_source_pointer_index_bytes = incremental.source_pointer_index_bytes().unwrap();
        reset_operation_scoped_persistent_source_replay_count_for_test();
        assert!(matches!(
            incremental.authenticate_sources_with_table_byte_limits(
                &[&distinct_source],
                usize::MAX,
                one_source_pointer_index_bytes,
            ),
            Err(GeneratedCylindricalCandidateAuthorityError::ResourceLimit {
                resource: "operation-scoped persistent-source pointer-index bytes",
                requested,
                limit,
            }) if requested == exact_pointer_index_bytes
                && limit == one_source_pointer_index_bytes
                && requested > limit
        ));
        assert_eq!(
            operation_scoped_persistent_source_replay_count_for_test(),
            0
        );
        assert_eq!(incremental.replayed_sources.len(), 1);
        assert!(Arc::ptr_eq(
            incremental.replayed_source(&source).unwrap().source(),
            &source
        ));
        assert!(matches!(
            incremental.replayed_source(&distinct_source),
            Err(
                GeneratedCylindricalCandidateAuthorityError::ReplayMismatch {
                    detail: "exact persistent-source allocation was not replayed in this operation",
                }
            )
        ));
    }

    #[test]
    fn replay_session_rejects_tampered_pointer_index_without_issuing_a_capability() {
        let (family, context, source) =
            tadpole_sector_root("candidate-authority-replay-session-pointer-index-tamper");
        let distinct_source = Arc::new(source.as_ref().clone());
        let mut session = GeneratedCylindricalReplaySession::new(&family, &context);
        session
            .authenticate_sources(&[&source, &distinct_source])
            .unwrap();
        assert_eq!(session.replayed_source_pointer_index.len(), 2);

        for entry in &mut session.replayed_source_pointer_index {
            entry.source_ordinal = 1 - entry.source_ordinal;
        }
        assert!(matches!(
            session.replayed_source(&source),
            Err(
                GeneratedCylindricalCandidateAuthorityError::ReplayMismatch {
                    detail: "persistent-source pointer index differs from its strong Arc",
                }
            )
        ));
        assert!(matches!(
            session.replayed_source(&distinct_source),
            Err(
                GeneratedCylindricalCandidateAuthorityError::ReplayMismatch {
                    detail: "persistent-source pointer index differs from its strong Arc",
                }
            )
        ));
    }

    #[test]
    fn dependent_event_identity_encodes_absent_pivot_as_structure_not_a_usize_sentinel() {
        #[derive(Clone, Copy)]
        struct OutcomePayload(crate::GeneratedCylindricalPersistentEliminationRowOutcome);

        impl ExactIdentityPayload for OutcomePayload {
            const SCHEMA: &'static str = "rustred-test-cylindrical-event-outcome-v1";

            fn write_exact_identity(
                &self,
                writer: &mut ExactIdentityWriter<'_>,
            ) -> Result<(), ExactIdentityError> {
                write_event_outcome_identity(writer, self.0)
            }
        }

        let dependent = encode_exact_identity(
            &OutcomePayload(crate::GeneratedCylindricalPersistentEliminationRowOutcome::Dependent),
            ExactIdentityLimits::default(),
        )
        .unwrap();
        let pivot = encode_exact_identity(
            &OutcomePayload(
                crate::GeneratedCylindricalPersistentEliminationRowOutcome::Pivot {
                    pivot_ordinal: 0,
                },
            ),
            ExactIdentityLimits::default(),
        )
        .unwrap();

        assert!(dependent.as_str().contains("R5:pivot#0{}"));
        assert!(!dependent.as_str().contains("U13:pivot_ordinal="));
        assert_eq!(dependent.stats().fields() + 1, pivot.stats().fields());
        // One extra typed field contributes both its tag-length integer and
        // its semantic ordinal to the complete exact-identity census.
        assert_eq!(dependent.stats().integers() + 2, pivot.stats().integers());
    }

    #[test]
    fn empty_root_is_global_and_nonempty_residual_is_only_locus_bound() {
        let (family, context, source) = tadpole_sector_root("candidate-authority-global-split");
        let pivot_ordinal = forward_pivot_ordinal(&source);
        let global = GeneratedCylindricalCandidateAuthority::compile(
            &family,
            &context,
            source,
            pivot_ordinal,
            GeneratedCylindricalCandidateAuthorityLimits::default(),
        )
        .unwrap();
        assert!(matches!(
            global,
            GeneratedCylindricalCandidateAuthority::Global(_)
        ));
        assert!(global.centered_assignment().is_none());
        assert!(!global.is_applicable_rule());
        assert_eq!(
            global.ordering_authority().identity(),
            global.source().ordering_identity()
        );
        assert_eq!(
            global.ordering_authority().policy(),
            IntegralOrderingPolicy::RustRedUnshiftedV1
        );

        let (family, context, source) = tadpole_locus_source("candidate-authority-locus-split");
        let pivot_ordinal = forward_pivot_ordinal(&source);
        let locus = GeneratedCylindricalCandidateAuthority::compile(
            &family,
            &context,
            source.clone(),
            pivot_ordinal,
            GeneratedCylindricalCandidateAuthorityLimits::default(),
        )
        .unwrap();
        let GeneratedCylindricalCandidateAuthority::LocusBound(locus_arm) = &locus else {
            panic!("nonempty source assignment must not become global");
        };
        let original = source.row_system().start().assignment();
        let centered = locus_arm.centered_assignment();
        assert_eq!(centered.entries().len(), original.entries().len());
        for (&(position, value), &(centered_position, centered_value)) in
            original.entries().iter().zip(centered.entries())
        {
            assert_eq!(position, centered_position);
            assert_eq!(
                centered_value,
                value + locus.original_pivot().values()[position]
            );
        }
        let mut matching = vec![2_i64; centered.arity()];
        for &(position, value) in centered.entries() {
            matching[position] = value;
        }
        locus
            .specialize_identity_for_proof(
                &context,
                &matching,
                ParametricArithmeticLimits::default(),
            )
            .unwrap();
        let (position, expected) = centered.entries()[0];
        let mut foreign = matching;
        foreign[position] = expected.checked_add(1).unwrap();
        assert_eq!(
            locus.specialize_identity_for_proof(
                &context,
                &foreign,
                ParametricArithmeticLimits::default()
            ),
            Err(
                GeneratedCylindricalCandidateAuthorityError::LocusAssignmentMismatch {
                    position,
                    expected,
                    actual: foreign[position],
                }
            )
        );
    }

    #[test]
    fn every_guarded_sector_root_pivot_compiles_as_global_and_replays() {
        let (family, context, source) =
            tadpole_sector_root("candidate-authority-all-global-pivots");
        let pivot_count = source.guarded_pivots().len();
        assert!(pivot_count > 0);
        for pivot_ordinal in 0..pivot_count {
            let candidate = GeneratedCylindricalCandidateAuthority::compile(
                &family,
                &context,
                source.clone(),
                pivot_ordinal,
                GeneratedCylindricalCandidateAuthorityLimits::default(),
            )
            .unwrap();
            assert!(candidate.is_global());
            candidate.replay(&family, &context).unwrap();
        }
    }

    #[test]
    fn split_centering_uses_negative_coefficient_translation_and_checked_c_plus_s() {
        let negative = IndexShift::try_new([-3, 2], 2).unwrap();
        assert_eq!(negated_shift(&negative).unwrap().values(), &[3, -2]);
        let centered = centered_assignment(
            &[(0, 7), (1, -5)],
            &negative,
            2,
            GeneratedCylindricalCandidateAuthorityLimits::default(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(centered.entries(), &[(0, 4), (1, -3)]);

        let minimum = IndexShift::try_new([i64::MIN], 1).unwrap();
        assert_eq!(
            negated_shift(&minimum),
            Err(
                GeneratedCylindricalCandidateAuthorityError::CoefficientTranslationOverflow {
                    position: 0
                }
            )
        );
        let positive = IndexShift::try_new([1], 1).unwrap();
        assert_eq!(
            centered_assignment(
                &[(0, i64::MAX)],
                &positive,
                1,
                GeneratedCylindricalCandidateAuthorityLimits::default(),
            ),
            Err(
                GeneratedCylindricalCandidateAuthorityError::CenteredAssignmentOverflow {
                    position: 0
                }
            )
        );
    }

    #[test]
    fn massive_tadpole_global_candidate_derives_and_specializes_the_dot_recurrence() {
        let (family, context, source) =
            tadpole_sector_root("candidate-authority-tadpole-recurrence");
        assert!(source.row_system().start().is_sector_root());
        assert!(source.row_system().start().assignment().is_empty());
        let pivot_ordinal = forward_pivot_ordinal(&source);
        let candidate = GeneratedCylindricalCandidateAuthority::compile(
            &family,
            &context,
            source,
            pivot_ordinal,
            GeneratedCylindricalCandidateAuthorityLimits::default(),
        )
        .unwrap();
        let GeneratedCylindricalCandidateAuthority::Global(global) = &candidate else {
            panic!("empty sector root must derive a global candidate");
        };
        assert_eq!(candidate.original_pivot().values(), &[1]);
        assert_eq!(candidate.coefficient_translation().values(), &[-1]);
        assert_eq!(candidate.key_center().values(), &[1]);
        assert_eq!(
            global
                .centered_relation_for_generated_when_bad()
                .terms()
                .get(&IndexShift::try_new([0], 1).unwrap()),
            Some(&context.one())
        );
        assert!(matches!(
            candidate.specialize_identity_for_proof(
                &context,
                &[1],
                ParametricArithmeticLimits::default(),
            ),
            Err(GeneratedCylindricalCandidateAuthorityError::Relation(
                ParametricRelationError::UnsatisfiableDomain
            ))
        ));

        let base = context.base();
        let mut accumulated = base.one();
        for power in 2_i64..=4 {
            let concrete = candidate
                .specialize_identity_for_proof(
                    &context,
                    &[power],
                    ParametricArithmeticLimits::default(),
                )
                .unwrap();
            assert_eq!(
                concrete
                    .terms()
                    .get(&crate::ConcreteIntegralKey::try_new([power]).unwrap()),
                Some(&base.one())
            );
            let previous = crate::ConcreteIntegralKey::try_new([power - 1]).unwrap();
            let rhs_coefficient = concrete.terms().get(&previous).unwrap();
            let step = base
                .try_neg(rhs_coefficient, ExactAlgebraLimits::default())
                .unwrap();
            accumulated = base
                .try_mul(&accumulated, &step, ExactAlgebraLimits::default())
                .unwrap();
            let expected = match power {
                2 => base.parse("(d-2)/(2*m2)").unwrap(),
                3 => base.parse("(d-2)*(d-4)/(8*m2^2)").unwrap(),
                4 => base.parse("(d-2)*(d-4)*(d-6)/(48*m2^3)").unwrap(),
                _ => unreachable!(),
            };
            assert_eq!(accumulated, expected, "I({power})/I(1)");
        }
        // Algebraic oracle use above does not publish the pre-WhenBad row.
        assert!(!candidate.is_applicable_rule());
    }

    #[test]
    fn external_momentum_family_keeps_global_base_assumptions_inseparable() {
        let family = guarded_external_bubble("candidate-authority-external-guards");
        let sector = SectorMask::try_new([true, true]).unwrap();
        let (family, context, source) = sector_root_source(family, sector, 1);
        assert_eq!(
            source
                .stats()
                .cumulative_construction_coefficient_integer_bit_work(),
            FIXTURE_ONE_PASS_CONSTRUCTION_INTEGER_BIT_WORK,
        );
        assert_eq!(
            source
                .stats()
                .cumulative_replay_coefficient_integer_bit_work(),
            FIXTURE_ONE_PASS_REPLAY_INTEGER_BIT_WORK,
        );
        let pivot_ordinal = forward_pivot_ordinal(&source);
        let candidate = GeneratedCylindricalCandidateAuthority::compile(
            &family,
            &context,
            source,
            pivot_ordinal,
            GeneratedCylindricalCandidateAuthorityLimits::default(),
        )
        .unwrap();
        assert!(candidate.is_global());
        // Stay well inside the active orthant and off simple diagonal guard
        // loci while retaining completely concrete integer powers.
        let assignment = [32_i64, 37_i64];
        let assumptions = candidate.base_assumptions().collect::<Vec<_>>();
        assert!(
            !assumptions.is_empty(),
            "base-parameter Gram/input guards must not be detached"
        );
        let concrete = candidate
            .specialize_identity_for_proof(
                &context,
                &assignment,
                ParametricArithmeticLimits::default(),
            )
            .unwrap();
        let mut retained_base_guards = 0usize;
        for assumption in assumptions {
            let specialized = context
                .specialize_nonzero_condition(
                    assumption.condition(),
                    &assignment,
                    ParametricArithmeticLimits::default(),
                )
                .unwrap();
            if !specialized.polynomial().is_nonzero_constant() {
                retained_base_guards += 1;
                concrete
                    .guarded_nonzero_conditions()
                    .iter()
                    .find(|condition| condition.polynomial() == specialized.polynomial())
                    .expect("every nonconstant cylindrical base guard must reach specialization");
            }
        }
        assert!(
            retained_base_guards > 0,
            "the external-bubble fixture must exercise retained base guards"
        );

        let compilation = GeneratedCylindricalWhenBadCompiler::compile(
            &family,
            &context,
            global_arm(&candidate),
            WhenBadCompilerLimits::default(),
        )
        .unwrap();
        let GeneratedCylindricalWhenBadCompilation::Certified(certificate) = compilation else {
            panic!("the generated external-bubble recurrence must be certified")
        };
        let certificate = Arc::new(certificate);
        assert_eq!(
            certificate
                .classification_for_indices(&context, &assignment)
                .unwrap()
                .expect("the assignment lies in the active external-bubble orthant")
                .disposition(),
            &WhenBadLeafDisposition::CoveredByCandidate
        );
        let reduction = crate::ConcreteReduction::apply_generated_cylindrical(
            Arc::clone(&certificate),
            &context,
            &assignment,
        )
        .unwrap();

        // The proof-only specialization is the independent oracle for every
        // retained base polynomial and its complete merged GuardOrigin set.
        // Exact vector equality also rejects dropped, inserted, reordered, or
        // partially merged guards outside the base-assumption subset.
        assert_eq!(
            reduction.required_nonzero(),
            concrete.guarded_nonzero_conditions()
        );
        assert!(
            reduction
                .specialized_relation()
                .has_identical_guard_provenance(&concrete)
        );
        for resolved in global_arm(&candidate).base_assumptions() {
            let specialized = context
                .specialize_nonzero_condition(
                    resolved.condition(),
                    &assignment,
                    ParametricArithmeticLimits::default(),
                )
                .unwrap();
            if specialized.polynomial().is_nonzero_constant() {
                continue;
            }
            let expected = concrete
                .guarded_nonzero_conditions()
                .iter()
                .find(|condition| condition.polynomial() == specialized.polynomial())
                .expect("proof specialization must retain the cylindrical base guard");
            let actual = reduction
                .required_nonzero()
                .iter()
                .find(|condition| condition.polynomial() == specialized.polynomial())
                .expect("concrete application must retain the cylindrical base guard");
            assert_eq!(
                actual, expected,
                "concrete application changed a base guard polynomial or its complete origins"
            );
        }
        assert!(reduction.replay_application(&family, &context).unwrap());
    }

    #[test]
    fn foreign_scope_and_out_of_range_pivots_fail_typed_before_recentering() {
        let (family, context, source) = tadpole_sector_root("candidate-authority-typed-errors");
        let foreign_family = massive_tadpole("candidate-authority-foreign-family");
        let foreign_context = ParametricIbpGenerator::try_new(&foreign_family)
            .unwrap()
            .context()
            .clone();
        assert!(matches!(
            compile_inner(
                &foreign_family,
                &foreign_context,
                source.clone(),
                0,
                GeneratedCylindricalCandidateAuthorityLimits::default(),
            ),
            Err(GeneratedCylindricalCandidateAuthorityError::ForeignFamily)
        ));
        assert_ne!(foreign_context.fingerprint(), context.fingerprint());
        assert!(matches!(
            compile_inner(
                &family,
                &foreign_context,
                source.clone(),
                0,
                GeneratedCylindricalCandidateAuthorityLimits::default(),
            ),
            Err(GeneratedCylindricalCandidateAuthorityError::ForeignContext)
        ));
        assert!(matches!(
            compile_inner(
                &family,
                &context,
                source.clone(),
                usize::MAX,
                GeneratedCylindricalCandidateAuthorityLimits::default(),
            ),
            Err(
                GeneratedCylindricalCandidateAuthorityError::PivotOutOfRange {
                    pivot_ordinal: usize::MAX
                }
            )
        ));
        let mut limits = GeneratedCylindricalCandidateAuthorityLimits::default();
        limits.max_arity = 0;
        assert!(matches!(
            compile_inner(&family, &context, source, 0, limits),
            Err(GeneratedCylindricalCandidateAuthorityError::ResourceLimit {
                resource: "candidate arity",
                requested: 1,
                limit: 0,
            })
        ));
    }

    fn binding_mut(
        candidate: &mut GeneratedCylindricalCandidateAuthority,
    ) -> &mut GeneratedCylindricalCandidateBinding {
        match candidate {
            GeneratedCylindricalCandidateAuthority::Global(candidate) => {
                Arc::make_mut(&mut candidate.binding)
            }
            GeneratedCylindricalCandidateAuthority::LocusBound(candidate) => {
                Arc::make_mut(&mut candidate.binding)
            }
        }
    }

    fn global_arm(
        candidate: &GeneratedCylindricalCandidateAuthority,
    ) -> &GeneratedCylindricalGlobalCandidateAuthority {
        match candidate {
            GeneratedCylindricalCandidateAuthority::Global(candidate) => candidate,
            GeneratedCylindricalCandidateAuthority::LocusBound(_) => {
                panic!("fixture must compile a global candidate")
            }
        }
    }

    struct DetachedIdentity;

    impl ExactIdentityPayload for DetachedIdentity {
        const SCHEMA: &'static str = "rustred-test-detached-candidate-identity-v1";

        fn write_exact_identity(
            &self,
            writer: &mut ExactIdentityWriter<'_>,
        ) -> Result<(), ExactIdentityError> {
            writer.string("detached", "not-a-candidate")
        }
    }

    #[test]
    fn global_when_bad_view_delegates_and_retains_row_span_allocation() {
        let (family, context, source) =
            tadpole_sector_root("candidate-authority-global-when-bad-view");
        let pivot_ordinal = forward_pivot_ordinal(&source);
        let candidate = GeneratedCylindricalCandidateAuthority::compile(
            &family,
            &context,
            source.clone(),
            pivot_ordinal,
            GeneratedCylindricalCandidateAuthorityLimits::default(),
        )
        .unwrap();
        let global = global_arm(&candidate);

        global.replay(&family, &context).unwrap();
        assert_eq!(global.family_fingerprint(), family.fingerprint_ref());
        assert_eq!(global.context_fingerprint(), context.fingerprint());
        assert_eq!(global.sector(), source.row_system().start().sector());
        assert_eq!(global.ordering_authority(), candidate.ordering_authority(),);
        assert_eq!(
            global.ordering_policy(),
            candidate.ordering_authority().policy()
        );
        assert_eq!(global.original_pivot(), candidate.original_pivot());
        assert_eq!(global.limits(), candidate.limits());
        assert!(Arc::ptr_eq(
            global.row_span_arc(),
            source.row_system().start().row_span_arc(),
        ));
        assert!(
            !global
                .local_candidate_binding_identity_for_source_composition()
                .is_empty()
        );

        let expected_assumptions = source
            .guarded_pivot(pivot_ordinal)
            .unwrap()
            .base_assumptions()
            .map(|assumption| assumption.witness().ordinal())
            .collect::<Vec<_>>();
        let delegated_assumptions = global
            .base_assumptions()
            .map(|assumption| assumption.witness().ordinal())
            .collect::<Vec<_>>();
        assert_eq!(delegated_assumptions, expected_assumptions);

        let cloned = global.clone();
        assert!(global.payload_eq(&cloned));
        let mut distinct = cloned;
        Arc::make_mut(&mut distinct.binding)
            .stats
            .centered_rhs_terms += 1;
        assert!(!global.payload_eq(&distinct));
    }

    #[test]
    fn standalone_global_when_bad_wrapper_recompiles_and_deep_compares() {
        // Compile-time proof that the public persistence boundary accepts the
        // exact Global arm, never the umbrella or locus-bound authority.
        let _: fn(
            &IntegralFamily,
            &ParametricCoefficientContext,
            &GeneratedCylindricalGlobalCandidateAuthority,
            WhenBadCompilerLimits,
        ) -> Result<GeneratedCylindricalWhenBadCompilation, WhenBadCompilerError> =
            GeneratedCylindricalWhenBadCompiler::compile;

        let (family, context, source) =
            tadpole_sector_root("candidate-authority-standalone-generated-when-bad");
        let pivot_ordinal = forward_pivot_ordinal(&source);
        let candidate = GeneratedCylindricalCandidateAuthority::compile(
            &family,
            &context,
            source.clone(),
            pivot_ordinal,
            GeneratedCylindricalCandidateAuthorityLimits::default(),
        );
        let candidate = match candidate {
            // Keep the source borrow and compile visually separate so a
            // future API change cannot silently select an arbitrary pivot.
            Err(error) => panic!("global candidate failed: {error}"),
            Ok(candidate) => candidate,
        };
        let global = global_arm(&candidate);
        let first = GeneratedCylindricalWhenBadCompiler::compile(
            &family,
            &context,
            global,
            WhenBadCompilerLimits::default(),
        )
        .unwrap();
        assert_eq!(
            first.schema(),
            crate::GENERATED_CYLINDRICAL_WHEN_BAD_V1_SCHEMA
        );
        assert!(first.candidate().payload_eq(global));
        assert_eq!(
            first.binding().source_authentication(),
            WhenBadSourceAuthentication::GeneratedCylindricalPersistentEliminationV2
        );
        assert!(first.is_certified());
        assert!(!first.is_unsupported());
        first.replay(&family, &context).unwrap();
        let GeneratedCylindricalWhenBadCompilation::Certified(certificate) = &first else {
            panic!("the tadpole forward recurrence must be certified")
        };
        assert_eq!(
            certificate.schema(),
            crate::GENERATED_CYLINDRICAL_WHEN_BAD_V1_SCHEMA
        );
        assert!(certificate.candidate().payload_eq(global));
        assert_eq!(
            certificate.binding().source_authentication(),
            WhenBadSourceAuthentication::GeneratedCylindricalPersistentEliminationV2
        );
        assert_eq!(
            certificate
                .binding()
                .ordering_authority()
                .discovery_anchor(),
            None
        );
        certificate.replay(&family, &context).unwrap();
        let debug = format!("{certificate:?}");
        assert!(debug.contains("candidate: \"<redacted>\""));
        assert!(!debug.contains("IndexShift"));
        assert!(!debug.contains("ParametricRelation"));

        let second = GeneratedCylindricalWhenBadCompiler::compile(
            &family,
            &context,
            global,
            WhenBadCompilerLimits::default(),
        )
        .unwrap();
        assert!(first.payload_eq(&second));

        let mut bad_schema = first.clone();
        bad_schema.corrupt_schema_for_test();
        assert!(matches!(
            bad_schema.replay(&family, &context),
            Err(WhenBadCompilerError::SchemaMismatch)
        ));

        let mut bad_limits = first.clone();
        bad_limits.corrupt_limits_for_test();
        assert!(matches!(
            bad_limits.replay(&family, &context),
            Err(WhenBadCompilerError::ReplayMismatch)
        ));

        let foreign = massive_tadpole("candidate-authority-standalone-foreign");
        assert!(matches!(
            first.replay(&foreign, &context),
            Err(WhenBadCompilerError::FamilyMismatch)
        ));
        let foreign_context = ParametricIbpGenerator::try_new(&foreign)
            .unwrap()
            .context()
            .clone();
        assert!(matches!(
            first.replay(&family, &foreign_context),
            Err(WhenBadCompilerError::ContextMismatch)
        ));
    }

    #[test]
    fn certified_global_cylindrical_tadpole_applies_i2_with_exact_provenance() {
        let (family, context, source) =
            tadpole_sector_root("candidate-authority-cylindrical-concrete-i2");
        let pivot_ordinal = forward_pivot_ordinal(&source);
        let candidate = GeneratedCylindricalCandidateAuthority::compile(
            &family,
            &context,
            source,
            pivot_ordinal,
            GeneratedCylindricalCandidateAuthorityLimits::default(),
        )
        .unwrap();
        let global = global_arm(&candidate);
        let compilation = GeneratedCylindricalWhenBadCompiler::compile(
            &family,
            &context,
            global,
            WhenBadCompilerLimits::default(),
        )
        .unwrap();
        let GeneratedCylindricalWhenBadCompilation::Certified(certificate) = compilation else {
            panic!("the generated tadpole forward recurrence must be certified")
        };
        let certificate = Arc::new(certificate);
        assert_eq!(
            certificate
                .classification_for_indices(&context, &[2])
                .unwrap()
                .expect("I(2) lies in the active tadpole orthant")
                .disposition(),
            &WhenBadLeafDisposition::CoveredByCandidate
        );

        let reduction = crate::ConcreteReduction::apply_generated_cylindrical(
            Arc::clone(&certificate),
            &context,
            &[2],
        )
        .unwrap();
        assert_eq!(
            reduction.source(),
            &crate::ConcreteIntegralKey::try_new([2]).unwrap()
        );
        assert_eq!(reduction.rhs().len(), 1);
        assert_eq!(
            reduction
                .rhs()
                .get(&crate::ConcreteIntegralKey::try_new([1]).unwrap()),
            Some(&context.base().parse("(d-2)/(2*m2)").unwrap())
        );
        assert!(reduction.anchored_candidate().is_none());
        assert!(
            reduction
                .generated_cylindrical_certificate()
                .is_some_and(|retained| retained.payload_eq(certificate.as_ref()))
        );
        assert_eq!(reduction.sector(), &SectorMask::try_new([true]).unwrap());
        assert_eq!(
            reduction.ordering_policy(),
            IntegralOrderingPolicy::RustRedUnshiftedV1
        );
        assert_eq!(reduction.pivot_ordinal(), pivot_ordinal);
        let expected_positive_m2 = context.base().parse("2*m2").unwrap().numerator;
        let expected_negative_m2 = context.base().parse("-2*m2").unwrap().numerator;
        assert_eq!(reduction.required_nonzero().len(), 2);
        for expected in [&expected_positive_m2, &expected_negative_m2] {
            let m2_guard = reduction
                .required_nonzero()
                .iter()
                .find(|condition| condition.polynomial().raw() == expected)
                .expect("the generated tadpole recurrence must retain both exact m2 associates");
            assert!(
                !m2_guard.origins().is_empty(),
                "the retained m2 guard must not lose its exact derivation origins"
            );
        }
        assert_eq!(
            reduction.required_nonzero(),
            reduction
                .specialized_relation()
                .guarded_nonzero_conditions()
        );
        assert!(
            reduction
                .verify_application(
                    family.coefficient_context(),
                    IntegralOrderingPolicy::RustRedUnshiftedV1,
                    ExactAlgebraLimits::default(),
                )
                .unwrap()
        );
        assert!(reduction.replay_application(&family, &context).unwrap());

        // Complete provider boundary: the product-free global cover selects
        // the exact same Arc allocation and concrete application retains it.
        // The fixture remains wholly generated; no tadpole recurrence enters
        // either coverage construction or provider application.
        let coverage = GeneratedCylindricalSectorCoverageCompiler::compile_authenticated(
            &family,
            &context,
            SectorMask::try_new([true]).unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            vec![GeneratedCylindricalSectorCoverageAttempt::certified(
                Arc::clone(&certificate),
            )],
            GeneratedCylindricalSectorCoverageLimits::default(),
        )
        .unwrap();
        let mut provider = GeneratedCylindricalSectorRuleProvider::try_new(
            &family,
            &context,
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            [coverage],
            GeneratedCylindricalSectorRuleProviderLimits::default(),
        )
        .unwrap();
        assert_eq!(provider.build_stats().sector_certificates(), 1);
        assert_eq!(provider.build_stats().attempts(), 1);
        assert_eq!(provider.build_stats().certified_attempts(), 1);
        assert_eq!(provider.build_stats().unsupported_attempts(), 0);
        assert_eq!(
            provider.build_stats().when_bad_retained_core_bytes(),
            certificate.retained_core_bytes()
        );
        provider.replay().unwrap();

        let ConcreteRuleDecision::Reduction(provider_reduction) = provider
            .decision_for(&crate::ConcreteIntegralKey::try_new([2]).unwrap())
            .unwrap()
        else {
            panic!("the cylindrical provider must select the generated I(2) recurrence")
        };
        assert_eq!(provider_reduction.rhs(), reduction.rhs());
        assert!(std::ptr::eq(
            provider_reduction
                .generated_cylindrical_certificate()
                .expect("provider reduction must retain cylindrical provenance"),
            certificate.as_ref(),
        ));
        assert!(
            provider_reduction
                .replay_application(&family, &context)
                .unwrap()
        );
        assert!(matches!(
            provider
                .decision_for(&crate::ConcreteIntegralKey::try_new([1]).unwrap())
                .unwrap(),
            ConcreteRuleDecision::Terminal(ConcreteTerminalStatus::Uncovered)
        ));
        assert!(matches!(
            provider
                .decision_for(&crate::ConcreteIntegralKey::try_new([0]).unwrap())
                .unwrap(),
            ConcreteRuleDecision::Terminal(ConcreteTerminalStatus::Uncovered)
        ));
        assert_eq!(provider.stats().queries(), 3);
        assert_eq!(provider.stats().reductions(), 1);
        assert_eq!(provider.stats().uncovered(), 2);
        assert_eq!(provider.stats().unsupported(), 0);
        provider.replay().unwrap();

        let foreign_family = massive_tadpole("candidate-authority-concrete-replay-foreign");
        let foreign_context = ParametricIbpGenerator::try_new(&foreign_family)
            .unwrap()
            .context()
            .clone();
        assert!(matches!(
            reduction.replay_application(&foreign_family, &context),
            Ok(false)
        ));
        assert!(matches!(
            reduction.replay_application(&family, &foreign_context),
            Ok(false)
        ));

        let exceptional = crate::ConcreteReduction::apply_generated_cylindrical(
            Arc::clone(&certificate),
            &context,
            &[1],
        )
        .unwrap_err();
        assert!(matches!(
            exceptional,
            crate::ParametricRuleError::GeneratedCylindricalApplicationMismatch(
                crate::GeneratedCylindricalApplicationMismatch::LeafNotCovered { .. }
            )
        ));
    }

    #[test]
    fn global_when_bad_core_is_v2_typed_replayable_and_retains_base_assumptions() {
        // The function-pointer type is a compile-time assertion that ordinary
        // generated WhenBad accepts the exact Global arm, not the umbrella or
        // the LocusBound arm.
        let _: fn(
            &IntegralFamily,
            &ParametricCoefficientContext,
            &GeneratedCylindricalGlobalCandidateAuthority,
            WhenBadCompilerLimits,
        ) -> Result<WhenBadCoreCompilation, WhenBadCompilerError> =
            WhenBadCompiler::compile_cylindrical_global_candidate;

        let family = guarded_external_bubble("candidate-authority-generated-when-bad");
        let sector = SectorMask::try_new([true, true]).unwrap();
        let (family, context, source) = sector_root_source(family, sector, 1);
        let pivot_ordinal = forward_pivot_ordinal(&source);
        let candidate = GeneratedCylindricalCandidateAuthority::compile(
            &family,
            &context,
            source.clone(),
            pivot_ordinal,
            GeneratedCylindricalCandidateAuthorityLimits::default(),
        )
        .unwrap();
        let global = global_arm(&candidate);
        let WhenBadCoreCompilation::Certified(core) =
            WhenBadCompiler::compile_cylindrical_global_candidate(
                &family,
                &context,
                global,
                WhenBadCompilerLimits::default(),
            )
            .unwrap()
        else {
            panic!("the generated forward recurrence must have uniform same-sector descent")
        };

        assert_eq!(core.schema(), WHEN_BAD_COMPILER_V2_SCHEMA);
        assert_eq!(
            core.binding().source_authentication(),
            WhenBadSourceAuthentication::GeneratedCylindricalPersistentEliminationV2,
        );
        assert!(matches!(
            core.binding().ordering_authority(),
            WhenBadOrderingAuthority::CylindricalV1 { policy, manifest }
                if *policy == IntegralOrderingPolicy::RustRedUnshiftedV1
                    && manifest.as_str() == global.ordering_authority().identity()
        ));
        assert_eq!(
            core.binding().ordering_authority().discovery_anchor(),
            None,
            "cylindrical authority must never fabricate a concrete anchor",
        );
        assert!(matches!(
            core.binding().source_authority(),
            WhenBadCandidateSourceAuthority::GeneratedCylindricalPersistentV2 {
                local_candidate_identity,
                source_row_count,
            } if local_candidate_identity.as_str()
                    == global.local_candidate_binding_identity_for_source_composition()
                && *source_row_count == source.stats().retained_source_rows()
        ));
        assert!(!core.descent_witnesses().is_empty());
        assert!(
            core.descent_witnesses()
                .iter()
                .all(|witness| { witness.policy() == IntegralOrderingPolicy::RustRedUnshiftedV1 })
        );

        let assumptions = global.base_assumptions().collect::<Vec<_>>();
        assert!(
            !assumptions.is_empty(),
            "external-family base assumptions must reach generated WhenBad",
        );
        for resolved in assumptions {
            let expected_source =
                WhenBadDomainConditionSource::GeneratedCylindricalBaseAssumption {
                    witness_ordinal: resolved.witness().ordinal(),
                    origins: resolved.condition().origins().iter().cloned().collect(),
                };
            let retained = core
                .domain_conditions()
                .iter()
                .find(|condition| {
                    condition.polynomial() == resolved.condition().polynomial()
                        && condition.sources().contains(&expected_source)
                })
                .expect("transitive base assumption must retain witness and origins");
            assert!(!retained.is_index_dependent());
            assert!(
                core.base_domain_guards()
                    .any(|condition| condition == retained)
            );
        }

        let WhenBadCoreCompilation::Certified(recompiled) =
            WhenBadCompiler::compile_cylindrical_global_candidate(
                &family,
                &context,
                global,
                WhenBadCompilerLimits::default(),
            )
            .unwrap()
        else {
            panic!("recompilation changed a certified candidate to unsupported")
        };
        assert!(core.payload_eq(&recompiled));

        let foreign_family = guarded_external_bubble("candidate-authority-when-bad-foreign");
        assert!(matches!(
            WhenBadCompiler::compile_cylindrical_global_candidate(
                &foreign_family,
                &context,
                global,
                WhenBadCompilerLimits::default(),
            ),
            Err(WhenBadCompilerError::FamilyMismatch)
        ));
        let foreign_context = ParametricIbpGenerator::try_new(&foreign_family)
            .unwrap()
            .context()
            .clone();
        assert!(matches!(
            WhenBadCompiler::compile_cylindrical_global_candidate(
                &family,
                &foreign_context,
                global,
                WhenBadCompilerLimits::default(),
            ),
            Err(WhenBadCompilerError::ContextMismatch)
        ));

        let mut tampered = candidate.clone();
        binding_mut(&mut tampered).stats.centered_rhs_terms += 1;
        assert!(matches!(
            WhenBadCompiler::compile_cylindrical_global_candidate(
                &family,
                &context,
                global_arm(&tampered),
                WhenBadCompilerLimits::default(),
            ),
            Err(WhenBadCompilerError::GeneratedCylindricalCandidate(_))
        ));
    }

    #[test]
    fn replay_rejects_binding_tampering_and_accepts_authenticated_source_rebinding() {
        let family_name = "candidate-authority-replay-tamper";
        let (family, context, source) = tadpole_sector_root(family_name);
        let pivot_ordinal = forward_pivot_ordinal(&source);
        let baseline = GeneratedCylindricalCandidateAuthority::compile(
            &family,
            &context,
            source,
            pivot_ordinal,
            GeneratedCylindricalCandidateAuthorityLimits::default(),
        )
        .unwrap();
        baseline.replay(&family, &context).unwrap();

        let rejects = |label: &str, candidate: &GeneratedCylindricalCandidateAuthority| {
            assert!(
                candidate.replay(&family, &context).is_err(),
                "tampered candidate unexpectedly replayed: {label}",
            );
        };

        let mut candidate = baseline.clone();
        binding_mut(&mut candidate).family_fingerprint = Arc::from("foreign-family");
        rejects("family fingerprint", &candidate);

        let mut candidate = baseline.clone();
        binding_mut(&mut candidate).context_fingerprint = Arc::from("foreign-context");
        rejects("context fingerprint", &candidate);

        let mut candidate = baseline.clone();
        binding_mut(&mut candidate).arm = CandidateArmTag::LocusBound;
        rejects("candidate arm", &candidate);

        let mut candidate = baseline.clone();
        binding_mut(&mut candidate).sector = SectorMask::try_new([false]).unwrap();
        rejects("sector", &candidate);

        let mut candidate = baseline.clone();
        binding_mut(&mut candidate).ordering_authority =
            GeneratedCylindricalCandidateOrderingAuthority::CylindricalV1 {
                policy: IntegralOrderingPolicy::RustRedUnshiftedV1,
                identity: Arc::from("invented-cylindrical-order"),
            };
        rejects("ordering authority", &candidate);

        let mut candidate = baseline.clone();
        binding_mut(&mut candidate).pivot_ordinal = usize::MAX;
        rejects("pivot ordinal", &candidate);

        if baseline.source().events().len() > 1 {
            let mut candidate = baseline.clone();
            binding_mut(&mut candidate).source_event = baseline.source().events()[1];
            rejects("source event", &candidate);
        }

        let mut candidate = baseline.clone();
        let binding = binding_mut(&mut candidate);
        if !binding.dependency_event_ordinals.is_empty() {
            binding.dependency_event_ordinals[0] = usize::MAX;
            rejects("dependency event ordinal", &candidate);
        }

        let mut candidate = baseline.clone();
        let binding = binding_mut(&mut candidate);
        if !binding.base_assumption_witness_ordinals.is_empty() {
            binding.base_assumption_witness_ordinals[0] = usize::MAX;
            rejects("base-assumption witness ordinal", &candidate);
        }

        let mut candidate = baseline.clone();
        binding_mut(&mut candidate).original_pivot = IndexShift::try_new([0], 1).unwrap();
        rejects("original pivot", &candidate);

        let mut candidate = baseline.clone();
        binding_mut(&mut candidate).coefficient_translation = IndexShift::try_new([0], 1).unwrap();
        rejects("coefficient translation", &candidate);

        let mut candidate = baseline.clone();
        let empty = ParametricRelation::new(
            family.fingerprint_ref(),
            ParametricRowId::Derived {
                label: Arc::from("tampered-centered-candidate"),
            },
            &context,
        );
        binding_mut(&mut candidate).centered_relation = Arc::new(empty);
        rejects("centered relation", &candidate);

        let mut candidate = baseline.clone();
        binding_mut(&mut candidate).limits.max_candidates = 2;
        rejects("candidate limits", &candidate);

        let mut candidate = baseline.clone();
        binding_mut(&mut candidate).stats.centered_rhs_terms += 1;
        rejects("candidate stats", &candidate);

        let mut candidate = baseline.clone();
        binding_mut(&mut candidate).exact_identity =
            encode_exact_identity(&DetachedIdentity, ExactIdentityLimits::default()).unwrap();
        rejects("exact identity", &candidate);

        let (_, _, alternative_source) = sector_root_source(
            massive_tadpole(family_name),
            SectorMask::try_new([true]).unwrap(),
            0,
        );
        alternative_source.replay(&family, &context).unwrap();
        let baseline_rows = baseline.source().row_system();
        let alternative_rows = alternative_source.row_system();
        let baseline_root = baseline_rows
            .start()
            .sector_root_start()
            .expect("baseline uses a sector-root start");
        let alternative_root = alternative_rows
            .start()
            .sector_root_start()
            .expect("alternative uses a sector-root start");
        assert_eq!(baseline_root.schedule().through_depth(), 1);
        assert_eq!(alternative_root.schedule().through_depth(), 0);
        assert!(!baseline_root.payload_eq(alternative_root));
        assert!(!baseline_rows.payload_eq(alternative_rows));
        assert!(!baseline.source().payload_eq(&alternative_source));

        // The source certificates are complete and distinct, but the selected
        // guarded pivot has the same local recurrence binding. Rebinding by
        // compiling from the independently replayed source is valid; complete
        // candidate equality still retains the source distinction.
        let alternative_pivot_ordinal = forward_pivot_ordinal(&alternative_source);
        assert_eq!(alternative_pivot_ordinal, pivot_ordinal);
        let rebound = GeneratedCylindricalCandidateAuthority::compile(
            &family,
            &context,
            alternative_source,
            alternative_pivot_ordinal,
            GeneratedCylindricalCandidateAuthorityLimits::default(),
        )
        .unwrap();
        assert_eq!(
            global_arm(&baseline).local_candidate_binding_identity_for_source_composition(),
            global_arm(&rebound).local_candidate_binding_identity_for_source_composition(),
        );
        assert!(!baseline.payload_eq(&rebound));
        rebound.replay(&family, &context).unwrap();

        let (locus_family, locus_context, locus_source) =
            tadpole_locus_source("candidate-authority-assignment-tamper");
        let locus_pivot = forward_pivot_ordinal(&locus_source);
        let mut locus = GeneratedCylindricalCandidateAuthority::compile(
            &locus_family,
            &locus_context,
            locus_source,
            locus_pivot,
            GeneratedCylindricalCandidateAuthorityLimits::default(),
        )
        .unwrap();
        let binding = binding_mut(&mut locus);
        binding.centered_assignment.as_mut().unwrap().entries[0].1 += 1;
        assert!(
            locus.replay(&locus_family, &locus_context).is_err(),
            "tampered locus candidate unexpectedly replayed: centered assignment",
        );
    }

    #[test]
    fn local_binding_identity_never_replaces_nested_source_authentication() {
        let (family, context, source) =
            tadpole_sector_root("candidate-authority-local-identity-scope");
        let pivot_ordinal = forward_pivot_ordinal(&source);
        let mut distinct_source_limits = source.limits();
        distinct_source_limits.max_batches =
            distinct_source_limits.max_batches.checked_add(1).unwrap();
        let distinct_source = Arc::new(
            GeneratedCylindricalPersistentEliminationCertificate::compile(
                &family,
                &context,
                source.row_system().clone(),
                distinct_source_limits,
            )
            .unwrap(),
        );
        assert!(!source.payload_eq(&distinct_source));

        let first = GeneratedCylindricalCandidateAuthority::compile(
            &family,
            &context,
            source,
            pivot_ordinal,
            GeneratedCylindricalCandidateAuthorityLimits::default(),
        )
        .unwrap();
        let second = GeneratedCylindricalCandidateAuthority::compile(
            &family,
            &context,
            distinct_source,
            pivot_ordinal,
            GeneratedCylindricalCandidateAuthorityLimits::default(),
        )
        .unwrap();
        // Local recurrence binding is intentionally insensitive to a
        // nonbinding upstream resource allowance.
        assert_eq!(
            global_arm(&first).local_candidate_binding_identity_for_source_composition(),
            global_arm(&second).local_candidate_binding_identity_for_source_composition()
        );
        // Complete proof equality is not: each candidate retains and replays
        // its distinct persistent source.
        assert!(!first.payload_eq(&second));
        first.replay(&family, &context).unwrap();
        second.replay(&family, &context).unwrap();
    }

    #[test]
    fn every_positive_candidate_resource_has_exact_and_one_below_evidence() {
        let (family, context, source) =
            tadpole_locus_source("candidate-authority-resource-evidence");
        let pivot_ordinal = forward_pivot_ordinal(&source);
        let baseline = compile_inner(
            &family,
            &context,
            source.clone(),
            pivot_ordinal,
            GeneratedCylindricalCandidateAuthorityLimits::default(),
        )
        .unwrap();

        macro_rules! expected_resource {
            (max_candidates) => {
                "cylindrical candidates"
            };
            (max_family_fingerprint_bytes) => {
                "candidate family fingerprint bytes"
            };
            (max_context_fingerprint_bytes) => {
                "candidate context fingerprint bytes"
            };
            (max_ordering_identity_bytes) => {
                "candidate ordering identity bytes"
            };
            (max_arity) => {
                "candidate arity"
            };
            (max_pivot_components) => {
                "candidate pivot components"
            };
            (max_pivot_integer_bit_work) => {
                "candidate pivot integer-bit work"
            };
            (max_dependency_events) => {
                "candidate dependency events"
            };
            (max_base_assumption_references) => {
                "candidate base-assumption references"
            };
            (max_base_assumption_origin_references) => {
                "candidate base-assumption origin references"
            };
            (max_base_assumption_manifest_bytes) => {
                "candidate base-assumption manifest bytes"
            };
            (max_base_assumption_condition_owned_bytes) => {
                "candidate base-assumption condition owned bytes"
            };
            (max_centered_assignment_entries) => {
                "candidate centered assignment entries"
            };
            (max_centered_assignment_additions) => {
                "candidate centered assignment additions"
            };
            (max_centered_assignment_integer_bit_work) => {
                "candidate centered assignment integer-bit work"
            };
            (max_row_label_bytes) => {
                "candidate row label bytes"
            };
            (max_centered_rhs_terms) => {
                "candidate centered RHS terms"
            };
            (max_recenter_attempts) => {
                "candidate recenter attempts"
            };
            (max_recenter_terms) => {
                "affine free recentering terms"
            };
            (max_recenter_guards) => {
                "affine free recentering guards"
            };
            (max_recenter_translation_components) => {
                "affine free recentering translation components"
            };
            (max_recenter_key_subtraction_boundary_checks) => {
                "affine free recentering key-subtraction boundary checks"
            };
            (max_recenter_source_terms) => {
                "affine free recentering source terms"
            };
            (max_recenter_source_exponent_entries) => {
                "affine free recentering source exponent entries"
            };
            (max_recenter_output_terms) => {
                "affine free recentering output terms"
            };
            (max_recenter_output_exponent_entries) => {
                "affine free recentering output exponent entries"
            };
            (max_recenter_power_operations) => {
                "affine free recentering power operations"
            };
            (max_recenter_integer_bit_work) => {
                "affine free recentering integer-bit work"
            };
            (max_recenter_normalized_coefficient_terms) => {
                "affine free recentering normalized coefficient terms"
            };
            (max_recenter_retained_bytes) => {
                "affine free recentering retained bytes"
            };
            (max_retained_payload_bytes) => {
                "candidate retained payload bytes"
            };
            (max_local_replay_comparison_units) => {
                "candidate local replay comparison units"
            };
            (max_local_replay_comparison_bytes) => {
                "candidate local replay comparison bytes"
            };
            (max_exact_identity_bytes) => {
                "exact structural identity bytes"
            };
            (max_exact_identity_fields) => {
                "exact structural identity fields"
            };
            (max_exact_identity_tag_bytes) => {
                "exact structural identity tag bytes"
            };
            (max_exact_identity_string_values) => {
                "exact structural identity string values"
            };
            (max_exact_identity_string_bytes) => {
                "exact structural identity string bytes"
            };
            (max_exact_identity_nesting_depth) => {
                "exact structural identity nesting depth"
            };
            (max_exact_identity_polynomials) => {
                "exact structural identity polynomials"
            };
            (max_exact_identity_polynomial_variables) => {
                "exact structural identity polynomial variables"
            };
            (max_exact_identity_polynomial_terms) => {
                "exact structural identity polynomial terms"
            };
            (max_exact_identity_exponent_entries) => {
                "exact structural identity polynomial exponent entries"
            };
            (max_exact_identity_integers) => {
                "exact structural identity integer values"
            };
            (max_exact_identity_integer_bits) => {
                "exact structural identity integer bits"
            };
        }

        macro_rules! exact_and_one_below {
            ($limit:ident, $stat:ident) => {{
                let mut exact_value = baseline.stats().$stat();
                if exact_value > 0 {
                    let mut exact_limits = GeneratedCylindricalCandidateAuthorityLimits::default();
                    let mut exact_candidate = None;
                    for _ in 0..4 {
                        exact_limits.$limit = exact_value;
                        let candidate = compile_inner(
                            &family,
                            &context,
                            source.clone(),
                            pivot_ordinal,
                            exact_limits,
                        )
                        .unwrap_or_else(|error| {
                            panic!(
                                "{} exact limit {exact_value} rejected: {error}",
                                stringify!($limit)
                            )
                        });
                        let observed = candidate.stats().$stat();
                        if observed == exact_value {
                            exact_candidate = Some(candidate);
                            break;
                        }
                        exact_value = observed;
                    }
                    let candidate = exact_candidate.unwrap_or_else(|| {
                        panic!("{} did not reach an exact fixed point", stringify!($limit))
                    });
                    assert_eq!(candidate.stats().$stat(), exact_limits.$limit);
                    let mut one_below = exact_limits;
                    one_below.$limit = exact_value - 1;
                    let error =
                        compile_inner(&family, &context, source.clone(), pivot_ordinal, one_below)
                            .err()
                            .unwrap_or_else(|| {
                                panic!(
                                    "{} admitted one below its observed census {exact_value}",
                                    stringify!($limit)
                                )
                            });
                    let (resource, requested, rejected_limit) = match error {
                        GeneratedCylindricalCandidateAuthorityError::ResourceLimit {
                            resource,
                            requested,
                            limit,
                        }
                        | GeneratedCylindricalCandidateAuthorityError::Relation(
                            ParametricRelationError::ResourceLimit {
                                resource,
                                requested,
                                limit,
                            },
                        ) => (resource, requested, limit),
                        other => panic!(
                            "{} one-below produced non-resource error: {other}",
                            stringify!($limit)
                        ),
                    };
                    assert_eq!(resource, expected_resource!($limit));
                    assert_eq!(rejected_limit, exact_value - 1);
                    assert!(requested > rejected_limit);
                }
            }};
        }

        exact_and_one_below!(max_candidates, candidates);
        exact_and_one_below!(max_family_fingerprint_bytes, family_fingerprint_bytes);
        exact_and_one_below!(max_context_fingerprint_bytes, context_fingerprint_bytes);
        exact_and_one_below!(max_ordering_identity_bytes, ordering_identity_bytes);
        exact_and_one_below!(max_arity, arity);
        exact_and_one_below!(max_pivot_components, pivot_components);
        exact_and_one_below!(max_pivot_integer_bit_work, pivot_integer_bit_work);
        exact_and_one_below!(max_dependency_events, dependency_events);
        exact_and_one_below!(max_base_assumption_references, base_assumption_references);
        exact_and_one_below!(
            max_base_assumption_origin_references,
            base_assumption_origin_references
        );
        exact_and_one_below!(
            max_base_assumption_manifest_bytes,
            base_assumption_manifest_bytes
        );
        exact_and_one_below!(
            max_base_assumption_condition_owned_bytes,
            base_assumption_condition_owned_bytes
        );
        exact_and_one_below!(max_centered_assignment_entries, centered_assignment_entries);
        exact_and_one_below!(
            max_centered_assignment_additions,
            centered_assignment_additions
        );
        exact_and_one_below!(
            max_centered_assignment_integer_bit_work,
            centered_assignment_integer_bit_work
        );
        exact_and_one_below!(max_row_label_bytes, row_label_bytes);
        exact_and_one_below!(max_centered_rhs_terms, centered_rhs_terms);
        exact_and_one_below!(max_recenter_attempts, recenter_attempts);
        exact_and_one_below!(max_recenter_terms, recenter_terms);
        exact_and_one_below!(max_recenter_guards, recenter_guards);
        exact_and_one_below!(
            max_recenter_translation_components,
            recenter_translation_components
        );
        exact_and_one_below!(
            max_recenter_key_subtraction_boundary_checks,
            recenter_key_subtraction_boundary_checks
        );
        exact_and_one_below!(max_recenter_source_terms, recenter_source_terms);
        exact_and_one_below!(
            max_recenter_source_exponent_entries,
            recenter_source_exponent_entries
        );
        exact_and_one_below!(max_recenter_output_terms, recenter_output_terms);
        exact_and_one_below!(
            max_recenter_output_exponent_entries,
            recenter_output_exponent_entries
        );
        exact_and_one_below!(max_recenter_power_operations, recenter_power_operations);
        exact_and_one_below!(max_recenter_integer_bit_work, recenter_integer_bit_work);
        exact_and_one_below!(
            max_recenter_normalized_coefficient_terms,
            recenter_normalized_coefficient_terms
        );
        exact_and_one_below!(max_recenter_retained_bytes, recenter_retained_bytes);
        exact_and_one_below!(max_retained_payload_bytes, retained_payload_bytes);
        exact_and_one_below!(
            max_local_replay_comparison_units,
            local_replay_comparison_units
        );
        exact_and_one_below!(
            max_local_replay_comparison_bytes,
            local_replay_comparison_bytes
        );
        exact_and_one_below!(max_exact_identity_bytes, exact_identity_bytes);
        exact_and_one_below!(max_exact_identity_fields, exact_identity_fields);
        exact_and_one_below!(max_exact_identity_tag_bytes, exact_identity_tag_bytes);
        exact_and_one_below!(
            max_exact_identity_string_values,
            exact_identity_string_values
        );
        exact_and_one_below!(max_exact_identity_string_bytes, exact_identity_string_bytes);
        exact_and_one_below!(
            max_exact_identity_nesting_depth,
            exact_identity_maximum_nesting_depth
        );
        exact_and_one_below!(max_exact_identity_polynomials, exact_identity_polynomials);
        exact_and_one_below!(
            max_exact_identity_polynomial_variables,
            exact_identity_polynomial_variables
        );
        exact_and_one_below!(
            max_exact_identity_polynomial_terms,
            exact_identity_polynomial_terms
        );
        exact_and_one_below!(
            max_exact_identity_exponent_entries,
            exact_identity_exponent_entries
        );
        exact_and_one_below!(max_exact_identity_integers, exact_identity_integers);
        exact_and_one_below!(max_exact_identity_integer_bits, exact_identity_integer_bits);
    }
}
