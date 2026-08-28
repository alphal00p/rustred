//! Standalone, proof-bearing compilation of LiteRed-style `WhenBad` domains.
//!
//! A [`ParametricReductionRuleCandidate`] replays its retained algebraic source
//! rows, but that alone does **not** prove those rows came from RustRed's fresh
//! canonical IBP/LI generator. This low-level module compiles such an
//! algebraically authenticated candidate's coefficient domain and
//! coefficient-aware inactive-boundary leaks into a finite symbolic case
//! partition. Its certificate carries an explicit
//! [`WhenBadSourceAuthentication::AlgebraicOnly`] marker so a future
//! generated-source wrapper cannot confuse it with canonical IBP provenance.
//! The implementation is topology and loop-count independent and invokes
//! neither Mathematica nor FORM.
//!
//! LiteRed conservatively marks a complete inactive-boundary disjunct bad when
//! the coefficient numerator does not vanish identically after substitution.
//! RustRed makes a sound, sharper Symbolica-native refinement: on that boundary
//! it splits the remaining numerator into zero (safe for this term) and nonzero
//! (exceptional) branches.  Persisted and normalized coefficient denominators
//! are processed first, matching LiteRed's ordering of domain failures before
//! sector leaks.

#[cfg(test)]
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fmt::Write as _;
use std::mem::size_of;
use std::sync::Arc;

use crate::exact_identity::{ExactIdentityError, ExactIdentityWriter};
use crate::generated_cylindrical_candidate_authority::{
    GeneratedCylindricalReplaySession, ReplayedGeneratedCylindricalGlobalCandidate,
};
use crate::{
    GeneratedCylindricalCandidateAuthorityError, GeneratedCylindricalGlobalCandidateAuthority,
    GuardOrigin, IndexShift, IntegralFamily, IntegralOrderingPolicy, ParametricArithmeticLimits,
    ParametricCoefficientContext, ParametricCoefficientError, ParametricPolynomial,
    ParametricReductionRuleCandidate, ParametricRelation, ParametricRuleError,
    ParametricRuleLimits, SectorMask, SymbolicPolynomialPredicateKind, SymbolicSectorCaseError,
    SymbolicSectorCaseId, SymbolicSectorCaseLimits, SymbolicSectorCasePartitionBuilder,
    SymbolicSectorCasePartitionCertificate,
};

pub const WHEN_BAD_COMPILER_V1_SCHEMA: &str = "rustred-when-bad-compiler-v1";
pub const WHEN_BAD_COMPILER_V2_SCHEMA: &str = "rustred-when-bad-compiler-v2";
pub(crate) const WHEN_BAD_CERTIFIED_STABLE_VALUE_IDENTITY_V1_SCHEMA: &str =
    "rustred-when-bad-certified-stable-value-identity-v1";
pub(crate) const WHEN_BAD_UNSUPPORTED_STABLE_VALUE_IDENTITY_V1_SCHEMA: &str =
    "rustred-when-bad-unsupported-stable-value-identity-v1";

#[cfg(test)]
thread_local! {
    static REPLAYED_CYLINDRICAL_CORE_CONSTRUCTIONS: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_replayed_cylindrical_core_construction_count_for_test() {
    REPLAYED_CYLINDRICAL_CORE_CONSTRUCTIONS.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn replayed_cylindrical_core_construction_count_for_test() -> usize {
    REPLAYED_CYLINDRICAL_CORE_CONSTRUCTIONS.with(Cell::get)
}

#[cfg(test)]
fn record_replayed_cylindrical_core_construction_for_test() {
    REPLAYED_CYLINDRICAL_CORE_CONSTRUCTIONS.with(|count| {
        count.set(count.get().saturating_add(1));
    });
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WhenBadSourceAuthentication {
    /// Candidate and retained elimination replay exactly, but their root rows
    /// have not been regenerated from an `IntegralFamily` by this layer.
    AlgebraicOnly,
    /// The complete generated cylindrical persistent-elimination source and
    /// the selected global candidate were replayed before this core proof was
    /// compiled. This says nothing about a concrete application: the
    /// candidate remains pre-rule until an enclosing generated certificate
    /// retains this `WhenBad` proof.
    GeneratedCylindricalPersistentEliminationV2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WhenBadCompilerLimits {
    pub arithmetic: ParametricArithmeticLimits,
    pub sector_cases: SymbolicSectorCaseLimits,
    pub max_rhs_terms: usize,
    pub max_domain_conditions: usize,
    pub max_domain_condition_sources: usize,
    pub max_guard_origins: usize,
    /// Aggregate conservative retained-byte bound of every copied guard-origin
    /// atom in the deduplicated domain-condition source payload.  The census
    /// uses [`GuardOrigin::retained_byte_bound`], so boxed offsets,
    /// permutations, assignments, and shared row labels cannot hide behind a
    /// mere origin-count limit.
    pub max_guard_origin_retained_bytes: usize,
    pub max_boundary_values_per_rhs: usize,
    pub max_boundary_values: usize,
    pub max_leak_events: usize,
    /// Aggregate index-shift components retained by all leak events.  One RHS
    /// shift is owned independently by every event, so the event count alone
    /// does not bound this payload for user-sized index spaces.
    pub max_leak_event_shift_components: usize,
    /// Aggregate structural bytes retained by the leak-event vector and all
    /// of its independently owned [`IndexShift`] payloads. Polynomial
    /// payloads are governed separately by `max_retained_condition_bytes`.
    pub max_leak_event_retained_bytes: usize,
    pub max_descent_witnesses: usize,
    /// Aggregate per-coordinate signed deltas retained by all descent
    /// witnesses. This is distinct from the witness count because every
    /// witness owns one component for every integral index.
    pub max_descent_witness_components: usize,
    pub max_leaf_classifications: usize,
    pub max_candidate_binding_bytes: usize,
    pub max_retained_condition_terms: usize,
    pub max_retained_condition_bytes: usize,
}

impl Default for WhenBadCompilerLimits {
    fn default() -> Self {
        Self {
            arithmetic: ParametricArithmeticLimits::default(),
            sector_cases: SymbolicSectorCaseLimits::default(),
            max_rhs_terms: 4_000_000,
            max_domain_conditions: 4_000_000,
            max_domain_condition_sources: 16_000_000,
            max_guard_origins: 16_000_000,
            max_guard_origin_retained_bytes: 2 * 1024 * 1024 * 1024,
            max_boundary_values_per_rhs: 1_000_000,
            max_boundary_values: 16_000_000,
            max_leak_events: 16_000_000,
            max_leak_event_shift_components: usize::try_from(16_384_000_000_u64)
                .unwrap_or(usize::MAX),
            max_leak_event_retained_bytes: usize::try_from(274_877_906_944_u64)
                .unwrap_or(usize::MAX),
            max_descent_witnesses: 4_000_000,
            max_descent_witness_components: usize::try_from(16_384_000_000_u64)
                .unwrap_or(usize::MAX),
            max_leaf_classifications: 4_000_000,
            max_candidate_binding_bytes: 512 * 1024 * 1024,
            max_retained_condition_terms: 32_000_000,
            max_retained_condition_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

/// Persisted ordering authority used by one `WhenBad` proof.
///
/// A concrete discovery point exists only for the legacy anchored
/// elimination arm. Cylindrical ordering is authenticated by its exact stable
/// manifest and never fabricates an anchor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WhenBadOrderingAuthority {
    AnchoredV1 {
        policy: IntegralOrderingPolicy,
        // Retain fallibly reserved buffers. `Arc<str>`/boxed-slice conversion
        // would perform a second infallible proportional allocation.
        manifest: String,
        discovery_anchor: Vec<i64>,
    },
    CylindricalV1 {
        policy: IntegralOrderingPolicy,
        manifest: String,
    },
}

impl WhenBadOrderingAuthority {
    pub const fn policy(&self) -> IntegralOrderingPolicy {
        match self {
            Self::AnchoredV1 { policy, .. } | Self::CylindricalV1 { policy, .. } => *policy,
        }
    }

    pub fn manifest(&self) -> &str {
        match self {
            Self::AnchoredV1 { manifest, .. } | Self::CylindricalV1 { manifest, .. } => manifest,
        }
    }

    pub fn discovery_anchor(&self) -> Option<&[i64]> {
        match self {
            Self::AnchoredV1 {
                discovery_anchor, ..
            } => Some(discovery_anchor),
            Self::CylindricalV1 { .. } => None,
        }
    }
}

/// Exact source identity retained by one `WhenBad` binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WhenBadCandidateSourceAuthority {
    AnchoredEliminationV1 {
        source_manifest: String,
        source_row_count: usize,
        trace_manifest: String,
        rule_limits: ParametricRuleLimits,
    },
    GeneratedCylindricalPersistentV2 {
        local_candidate_identity: String,
        source_row_count: usize,
    },
}

impl WhenBadCandidateSourceAuthority {
    fn source_identity(&self) -> &str {
        match self {
            Self::AnchoredEliminationV1 {
                source_manifest, ..
            } => source_manifest,
            Self::GeneratedCylindricalPersistentV2 {
                local_candidate_identity,
                ..
            } => local_candidate_identity,
        }
    }

    const fn source_row_count(&self) -> usize {
        match self {
            Self::AnchoredEliminationV1 {
                source_row_count, ..
            }
            | Self::GeneratedCylindricalPersistentV2 {
                source_row_count, ..
            } => *source_row_count,
        }
    }
}

/// Exact fields that bind a compiled domain to one replayed candidate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WhenBadCandidateBinding {
    source_authentication: WhenBadSourceAuthentication,
    family_fingerprint: String,
    context_fingerprint: String,
    sector: SectorMask,
    ordering_authority: WhenBadOrderingAuthority,
    source_authority: WhenBadCandidateSourceAuthority,
    pivot_ordinal: usize,
    original_pivot: IndexShift,
    centered_relation_manifest: String,
    retained_bytes: usize,
}

impl WhenBadCandidateBinding {
    pub fn source_authentication(&self) -> WhenBadSourceAuthentication {
        self.source_authentication
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }

    pub fn sector(&self) -> &SectorMask {
        &self.sector
    }

    pub fn ordering(&self) -> &str {
        self.ordering_authority.manifest()
    }

    pub const fn ordering_authority(&self) -> &WhenBadOrderingAuthority {
        &self.ordering_authority
    }

    pub const fn source_authority(&self) -> &WhenBadCandidateSourceAuthority {
        &self.source_authority
    }

    pub fn source_manifest(&self) -> &str {
        self.source_authority.source_identity()
    }

    pub fn source_row_count(&self) -> usize {
        self.source_authority.source_row_count()
    }

    pub fn pivot_ordinal(&self) -> usize {
        self.pivot_ordinal
    }

    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WhenBadDomainConditionSource {
    PersistedGuard {
        ordinal: usize,
        origins: Vec<GuardOrigin>,
    },
    GeneratedCylindricalBaseAssumption {
        witness_ordinal: usize,
        origins: Vec<GuardOrigin>,
    },
    CoefficientDenominator {
        shift: IndexShift,
    },
}

/// Allocation-free source view used while all aggregate limits are checked.
/// It is materialized only after insertion or merge has been admitted.
#[derive(Clone, Copy)]
enum BorrowedWhenBadDomainConditionSource<'a> {
    PersistedGuard {
        ordinal: usize,
        origins: &'a BTreeSet<GuardOrigin>,
    },
    GeneratedCylindricalBaseAssumption {
        witness_ordinal: usize,
        origins: &'a BTreeSet<GuardOrigin>,
    },
    CoefficientDenominator {
        shift: &'a IndexShift,
    },
}

impl BorrowedWhenBadDomainConditionSource<'_> {
    fn origin_count(self) -> usize {
        match self {
            Self::PersistedGuard { origins, .. }
            | Self::GeneratedCylindricalBaseAssumption { origins, .. } => origins.len(),
            Self::CoefficientDenominator { .. } => 0,
        }
    }

    fn origin_retained_bytes(self) -> Result<usize, WhenBadCompilerError> {
        let origins = match self {
            Self::PersistedGuard { origins, .. }
            | Self::GeneratedCylindricalBaseAssumption { origins, .. } => origins,
            Self::CoefficientDenominator { .. } => return Ok(0),
        };
        origins.iter().try_fold(0usize, |total, origin| {
            checked_add(
                "WhenBad guard-origin retained bytes",
                total,
                origin.retained_byte_bound().ok_or(
                    WhenBadCompilerError::ResourceCountOverflow {
                        resource: "WhenBad guard-origin retained bytes",
                    },
                )?,
            )
        })
    }

    fn matches_owned(self, source: &WhenBadDomainConditionSource) -> bool {
        match (self, source) {
            (
                Self::PersistedGuard { ordinal, origins },
                WhenBadDomainConditionSource::PersistedGuard {
                    ordinal: retained_ordinal,
                    origins: retained_origins,
                },
            ) => ordinal == *retained_ordinal && origins.iter().eq(retained_origins.iter()),
            (
                Self::GeneratedCylindricalBaseAssumption {
                    witness_ordinal,
                    origins,
                },
                WhenBadDomainConditionSource::GeneratedCylindricalBaseAssumption {
                    witness_ordinal: retained_ordinal,
                    origins: retained_origins,
                },
            ) => witness_ordinal == *retained_ordinal && origins.iter().eq(retained_origins.iter()),
            (
                Self::CoefficientDenominator { shift },
                WhenBadDomainConditionSource::CoefficientDenominator {
                    shift: retained_shift,
                },
            ) => shift == retained_shift,
            _ => false,
        }
    }

    fn try_to_owned(self) -> Result<WhenBadDomainConditionSource, WhenBadCompilerError> {
        Ok(match self {
            Self::PersistedGuard { ordinal, origins } => {
                WhenBadDomainConditionSource::PersistedGuard {
                    ordinal,
                    origins: try_copy_guard_origins(origins)?,
                }
            }
            Self::GeneratedCylindricalBaseAssumption {
                witness_ordinal,
                origins,
            } => WhenBadDomainConditionSource::GeneratedCylindricalBaseAssumption {
                witness_ordinal,
                origins: try_copy_guard_origins(origins)?,
            },
            Self::CoefficientDenominator { shift } => {
                WhenBadDomainConditionSource::CoefficientDenominator {
                    shift: IndexShift::try_new(shift.values().iter().copied(), shift.arity())?,
                }
            }
        })
    }
}

/// One deduplicated required nonzero polynomial.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WhenBadDomainCondition {
    polynomial: ParametricPolynomial,
    sources: Vec<WhenBadDomainConditionSource>,
    index_dependent: bool,
}

impl WhenBadDomainCondition {
    pub fn polynomial(&self) -> &ParametricPolynomial {
        &self.polynomial
    }

    pub fn sources(&self) -> &[WhenBadDomainConditionSource] {
        &self.sources
    }

    pub fn is_index_dependent(&self) -> bool {
        self.index_dependent
    }
}

/// How the coefficient numerator behaves after one forbidden boundary value
/// is imposed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WhenBadLeakNumeratorGate {
    /// A nonzero coefficient-field element: the whole boundary is exceptional
    /// at generic kinematics.
    CoefficientFieldNonzero(ParametricPolynomial),
    /// A remaining index polynomial is split exactly into zero/nonzero cases.
    Symbolic(ParametricPolynomial),
}

/// Why one exact coordinate value prevents unconditional use of an RHS term.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WhenBadBoundaryHazardKind {
    /// An inactive source line would become active, so the target is outside
    /// the sector and its subsectors.
    InactiveSectorActivation,
    /// `n_i + shift_i` is a mathematical integer but is outside RustRed's
    /// concrete `i64` key representation.  These are finitely many edge
    /// points for every fixed shift and are split exactly rather than making
    /// an otherwise uniformly descending mixed shift globally unsupported.
    ConcreteIndexOverflow,
}

/// One OR-event `n_i = value && numerator != 0` for a sector-leak or concrete
/// key-representation boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WhenBadLeakEvent {
    ordinal: usize,
    rhs_ordinal: usize,
    rhs_shift: IndexShift,
    kind: WhenBadBoundaryHazardKind,
    coordinate: usize,
    boundary_value: i64,
    boundary_polynomial: ParametricPolynomial,
    numerator_gate: WhenBadLeakNumeratorGate,
}

impl WhenBadLeakEvent {
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub fn rhs_ordinal(&self) -> usize {
        self.rhs_ordinal
    }

    pub fn rhs_shift(&self) -> &IndexShift {
        &self.rhs_shift
    }

    pub fn kind(&self) -> WhenBadBoundaryHazardKind {
        self.kind
    }

    pub fn coordinate(&self) -> usize {
        self.coordinate
    }

    /// Backward-compatible coordinate accessor.  New code should use
    /// [`Self::coordinate`] because overflow hazards may be on active slots.
    pub fn inactive_coordinate(&self) -> usize {
        self.coordinate
    }

    pub fn boundary_value(&self) -> i64 {
        self.boundary_value
    }

    /// Exact coordinate polynomial whose zero locus is this boundary.  This
    /// is exposed so higher-level ordered coverage can compose the
    /// authenticated bad-domain formula without replaying an arbitrary local
    /// decision-tree prefix.
    pub fn boundary_polynomial(&self) -> &ParametricPolynomial {
        &self.boundary_polynomial
    }

    pub fn numerator_gate(&self) -> &WhenBadLeakNumeratorGate {
        &self.numerator_gate
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WhenBadDescentComponent {
    CornerDistance,
    DotPower,
    NumeratorPower,
    IndexExcess { position: usize },
}

/// Uniform same-sector descent; any active-line pinch is automatically a
/// lower-sector target, while every inactive activation is removed by a leak
/// event before this witness is used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WhenBadUniformDescentWitness {
    policy: IntegralOrderingPolicy,
    rhs_ordinal: usize,
    rhs_shift: IndexShift,
    corner_delta: i128,
    dot_delta: i128,
    numerator_delta: i128,
    // Retain the fallibly reserved allocation. Converting a user-sized vector
    // into a boxed slice may request a second proportional shrink allocation.
    index_excess_deltas: Vec<i128>,
    decisive_component: WhenBadDescentComponent,
}

impl WhenBadUniformDescentWitness {
    pub const fn policy(&self) -> IntegralOrderingPolicy {
        self.policy
    }

    pub fn rhs_ordinal(&self) -> usize {
        self.rhs_ordinal
    }

    pub fn rhs_shift(&self) -> &IndexShift {
        &self.rhs_shift
    }

    pub fn decisive_component(&self) -> WhenBadDescentComponent {
        self.decisive_component
    }

    pub const fn corner_delta(&self) -> i128 {
        self.corner_delta
    }

    pub const fn dot_delta(&self) -> i128 {
        self.dot_delta
    }

    pub const fn numerator_delta(&self) -> i128 {
        self.numerator_delta
    }

    pub fn index_excess_deltas(&self) -> &[i128] {
        &self.index_excess_deltas
    }

    pub(crate) fn owned_retained_byte_bound(&self) -> Option<usize> {
        size_of::<Self>()
            .checked_add(self.rhs_shift.owned_retained_byte_bound()?)?
            .checked_add(
                self.index_excess_deltas
                    .capacity()
                    .checked_mul(size_of::<i128>())?,
            )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WhenBadLeafDisposition {
    CoveredByCandidate,
    ExceptionalDomain { condition: usize },
    ExceptionalSectorLeak { event: usize },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WhenBadLeafClassification {
    case: SymbolicSectorCaseId,
    disposition: WhenBadLeafDisposition,
}

impl WhenBadLeafClassification {
    pub fn case(&self) -> SymbolicSectorCaseId {
        self.case
    }

    pub fn disposition(&self) -> &WhenBadLeafDisposition {
        &self.disposition
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WhenBadCompilerStats {
    rhs_terms: usize,
    domain_conditions: usize,
    domain_condition_sources: usize,
    guard_origins: usize,
    guard_origin_retained_bytes: usize,
    base_domain_guards: usize,
    index_domain_guards: usize,
    boundary_values_examined: usize,
    leak_events: usize,
    leak_event_shift_components: usize,
    leak_event_retained_bytes: usize,
    descent_witnesses: usize,
    descent_witness_components: usize,
    leaf_classifications: usize,
    retained_condition_terms: usize,
    retained_condition_bytes: usize,
    /// Conservative capacity-aware bytes owned by the certified core.  This
    /// excludes the retained candidate graph, which is charged by its source
    /// authority, and includes the core value plus every heap allocation
    /// reachable exclusively through the core. Shared polynomial payloads
    /// may be charged more than once to keep this bound fail-closed.
    retained_core_bytes: usize,
}

impl WhenBadCompilerStats {
    pub fn rhs_terms(self) -> usize {
        self.rhs_terms
    }

    pub fn domain_conditions(self) -> usize {
        self.domain_conditions
    }

    pub fn base_domain_guards(self) -> usize {
        self.base_domain_guards
    }

    pub fn domain_condition_sources(self) -> usize {
        self.domain_condition_sources
    }

    pub fn guard_origins(self) -> usize {
        self.guard_origins
    }

    pub fn guard_origin_retained_bytes(self) -> usize {
        self.guard_origin_retained_bytes
    }

    pub fn retained_condition_terms(self) -> usize {
        self.retained_condition_terms
    }

    pub fn retained_condition_bytes(self) -> usize {
        self.retained_condition_bytes
    }

    pub fn index_domain_guards(self) -> usize {
        self.index_domain_guards
    }

    pub fn boundary_values_examined(self) -> usize {
        self.boundary_values_examined
    }

    pub fn leak_events(self) -> usize {
        self.leak_events
    }

    pub fn leak_event_shift_components(self) -> usize {
        self.leak_event_shift_components
    }

    pub fn leak_event_retained_bytes(self) -> usize {
        self.leak_event_retained_bytes
    }

    pub fn descent_witnesses(self) -> usize {
        self.descent_witnesses
    }

    pub fn descent_witness_components(self) -> usize {
        self.descent_witness_components
    }

    pub fn leaf_classifications(self) -> usize {
        self.leaf_classifications
    }

    pub fn retained_core_bytes(self) -> usize {
        self.retained_core_bytes
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WhenBadUnsupportedReason {
    NonUniformSameSectorDescent {
        rhs_ordinal: usize,
        rhs_shift: IndexShift,
        first_nonzero_component: WhenBadDescentComponent,
        delta: i128,
    },
    ZeroSameSectorComplexityDelta {
        rhs_ordinal: usize,
        rhs_shift: IndexShift,
    },
    UnboundedIndexAddition {
        rhs_ordinal: usize,
        rhs_shift: IndexShift,
        coordinate: usize,
        delta: i64,
    },
}

/// Authenticated unsupported result.  It is not a rule, a terminal, or a
/// master claim; callers must retain the sector case as uncovered/requeue it.
#[derive(Clone, Debug)]
pub struct WhenBadUnsupported {
    candidate: Arc<ParametricReductionRuleCandidate>,
    core: WhenBadUnsupportedCore,
}

/// Candidate-independent unsupported payload shared by the anchored and
/// generated cylindrical wrappers. Construction is sealed in this module.
#[derive(Clone, Debug)]
pub(crate) struct WhenBadUnsupportedCore {
    binding: WhenBadCandidateBinding,
    reason: WhenBadUnsupportedReason,
    limits: WhenBadCompilerLimits,
    retained_core_bytes: usize,
}

impl WhenBadUnsupported {
    pub fn binding(&self) -> &WhenBadCandidateBinding {
        self.core.binding()
    }

    pub fn reason(&self) -> &WhenBadUnsupportedReason {
        self.core.reason()
    }

    /// Conservative capacity-aware bytes owned by the unsupported core. The
    /// separately retained candidate graph is intentionally excluded.
    pub fn retained_core_bytes(&self) -> usize {
        self.core.retained_core_bytes()
    }

    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.core.payload_eq(&other.core)
    }

    pub(crate) fn write_stable_value_identity(
        &self,
        writer: &mut ExactIdentityWriter<'_>,
        tag: &str,
    ) -> Result<(), ExactIdentityError> {
        writer.begin_record(tag, 4)?;
        writer.string(
            "identity_schema",
            WHEN_BAD_UNSUPPORTED_STABLE_VALUE_IDENTITY_V1_SCHEMA,
        )?;
        write_candidate_binding_identity(writer, "binding", &self.core.binding)?;
        write_unsupported_reason_identity(writer, "reason", &self.core.reason)?;
        write_when_bad_limits_identity(writer, "limits", self.core.limits)?;
        // Capacity/ABI-dependent retained bytes remain replay diagnostics.
        writer.end_record()
    }

    #[cfg(test)]
    pub(crate) fn invalidate_retained_core_bytes_for_test(&mut self) {
        self.core.retained_core_bytes = 0;
    }

    pub fn replay(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), WhenBadCompilerError> {
        self.core.replay_capacity_census()?;
        match WhenBadCompiler::compile_algebraic_candidate(
            context,
            &self.candidate,
            self.core.limits,
        )? {
            WhenBadCompilation::Unsupported(replayed) if replayed.core.payload_eq(&self.core) => {
                Ok(())
            }
            _ => Err(WhenBadCompilerError::ReplayMismatch),
        }
    }
}

impl WhenBadUnsupportedCore {
    fn try_new(
        binding: WhenBadCandidateBinding,
        reason: WhenBadUnsupportedReason,
        limits: WhenBadCompilerLimits,
    ) -> Result<Self, WhenBadCompilerError> {
        let mut core = Self {
            binding,
            reason,
            limits,
            retained_core_bytes: 0,
        };
        core.retained_core_bytes = core.observed_retained_core_bytes()?;
        core.replay_capacity_census()?;
        Ok(core)
    }

    pub(crate) const fn binding(&self) -> &WhenBadCandidateBinding {
        &self.binding
    }

    pub(crate) const fn reason(&self) -> &WhenBadUnsupportedReason {
        &self.reason
    }

    pub(crate) const fn limits(&self) -> WhenBadCompilerLimits {
        self.limits
    }

    pub(crate) const fn retained_core_bytes(&self) -> usize {
        self.retained_core_bytes
    }

    fn observed_retained_core_bytes(&self) -> Result<usize, WhenBadCompilerError> {
        let mut bytes = size_of::<Self>();
        bytes = add_retained_core_bytes(bytes, candidate_binding_retained_bytes(&self.binding)?)?;
        bytes = add_retained_core_bytes(bytes, unsupported_reason_heap_bytes(&self.reason)?)?;
        Ok(bytes)
    }

    pub(crate) fn replay_capacity_census(&self) -> Result<(), WhenBadCompilerError> {
        replay_candidate_binding_capacity(&self.binding, self.limits)?;
        if self.observed_retained_core_bytes()? > self.retained_core_bytes {
            return Err(WhenBadCompilerError::ReplayMismatch);
        }
        Ok(())
    }

    fn capacity_payload_is_valid(&self) -> bool {
        self.replay_capacity_census().is_ok()
    }

    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.capacity_payload_is_valid()
            && other.capacity_payload_is_valid()
            && self.binding == other.binding
            && self.reason == other.reason
            && self.limits == other.limits
            && self.retained_core_bytes == other.retained_core_bytes
    }
}

#[derive(Clone, Debug)]
pub struct WhenBadCertificate {
    candidate: Arc<ParametricReductionRuleCandidate>,
    core: WhenBadCertifiedCore,
}

/// Candidate-independent certified `WhenBad` proof. The type is crate-visible
/// for the generated cylindrical wrapper, but all construction remains sealed
/// behind [`WhenBadCompiler`].
#[derive(Clone, Debug)]
pub(crate) struct WhenBadCertifiedCore {
    schema: &'static str,
    binding: WhenBadCandidateBinding,
    // Retain the fallibly grown allocation. Boxing a partially filled vector
    // can request an infallible proportional shrink allocation.
    domain_conditions: Vec<WhenBadDomainCondition>,
    // Retain these fallibly reserved vectors. Converting a user-sized vector
    // to a boxed slice may request an infallible proportional shrink
    // allocation after the compiler's admission checks.
    base_domain_guards: Vec<usize>,
    index_domain_guards: Vec<usize>,
    leak_events: Vec<WhenBadLeakEvent>,
    // Retain the fallibly reserved allocation assembled by the compiler.
    descent_witnesses: Vec<WhenBadUniformDescentWitness>,
    partition: SymbolicSectorCasePartitionCertificate,
    classifications: Vec<WhenBadLeafClassification>,
    limits: WhenBadCompilerLimits,
    stats: WhenBadCompilerStats,
}

impl WhenBadCertificate {
    pub fn schema(&self) -> &'static str {
        self.core.schema()
    }

    pub fn candidate(&self) -> &ParametricReductionRuleCandidate {
        &self.candidate
    }

    pub fn binding(&self) -> &WhenBadCandidateBinding {
        self.core.binding()
    }

    pub fn domain_conditions(&self) -> &[WhenBadDomainCondition] {
        self.core.domain_conditions()
    }

    pub fn base_domain_guards(&self) -> impl Iterator<Item = &WhenBadDomainCondition> {
        self.core.base_domain_guards()
    }

    pub fn index_domain_guards(&self) -> impl Iterator<Item = &WhenBadDomainCondition> {
        self.core.index_domain_guards()
    }

    /// Retained domain-condition ordinal together with each index guard.
    ///
    /// Coverage-formula normalization persists this source identity.  The
    /// filtered guard position is not interchangeable with the ordinal in the
    /// authenticated `domain_conditions` payload.
    pub(crate) fn index_domain_guards_with_ordinals(
        &self,
    ) -> impl Iterator<Item = (usize, &WhenBadDomainCondition)> {
        self.core.index_domain_guards_with_ordinals()
    }

    pub fn leak_events(&self) -> &[WhenBadLeakEvent] {
        self.core.leak_events()
    }

    pub fn descent_witnesses(&self) -> &[WhenBadUniformDescentWitness] {
        self.core.descent_witnesses()
    }

    pub fn partition(&self) -> &SymbolicSectorCasePartitionCertificate {
        self.core.partition()
    }

    pub fn classifications(&self) -> &[WhenBadLeafClassification] {
        self.core.classifications()
    }

    pub fn stats(&self) -> WhenBadCompilerStats {
        self.core.stats()
    }

    /// Conservative capacity-aware bytes owned by the certified core. The
    /// separately retained candidate graph is intentionally excluded.
    pub fn retained_core_bytes(&self) -> usize {
        self.core.retained_core_bytes()
    }

    pub fn replay(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), WhenBadCompilerError> {
        if self.core.schema != WHEN_BAD_COMPILER_V2_SCHEMA {
            return Err(WhenBadCompilerError::SchemaMismatch);
        }
        self.replay_payload_without_recompile(context)?;
        match WhenBadCompiler::compile_algebraic_candidate(
            context,
            &self.candidate,
            self.core.limits,
        )? {
            WhenBadCompilation::Certified(replayed) if self.payload_eq(&replayed) => Ok(()),
            _ => Err(WhenBadCompilerError::ReplayMismatch),
        }
    }

    /// Locate the unique structurally classified leaf for a concrete integer
    /// assignment.  Base parameters remain formal: `NonZero` means the
    /// specialized base polynomial is not identically zero.
    pub fn classification_for_indices(
        &self,
        context: &ParametricCoefficientContext,
        indices: &[i64],
    ) -> Result<Option<&WhenBadLeafClassification>, WhenBadCompilerError> {
        self.core.classification_for_indices(context, indices)
    }

    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.core.payload_eq(&other.core)
    }

    pub(crate) fn write_stable_value_identity(
        &self,
        writer: &mut ExactIdentityWriter<'_>,
        tag: &str,
    ) -> Result<(), ExactIdentityError> {
        let core = &self.core;
        writer.begin_record(tag, 12)?;
        writer.string(
            "identity_schema",
            WHEN_BAD_CERTIFIED_STABLE_VALUE_IDENTITY_V1_SCHEMA,
        )?;
        writer.string("certificate_schema", core.schema)?;
        write_candidate_binding_identity(writer, "binding", &core.binding)?;
        writer.begin_sequence("domain_conditions", core.domain_conditions.len())?;
        for condition in &core.domain_conditions {
            write_domain_condition_identity(writer, "condition", condition)?;
        }
        writer.end_sequence()?;
        write_usize_sequence_identity(writer, "base_domain_guards", &core.base_domain_guards)?;
        write_usize_sequence_identity(writer, "index_domain_guards", &core.index_domain_guards)?;
        writer.begin_sequence("leak_events", core.leak_events.len())?;
        for event in &core.leak_events {
            write_leak_event_identity(writer, "event", event)?;
        }
        writer.end_sequence()?;
        writer.begin_sequence("descent_witnesses", core.descent_witnesses.len())?;
        for witness in &core.descent_witnesses {
            write_descent_witness_identity(writer, "witness", witness)?;
        }
        writer.end_sequence()?;
        write_partition_identity(writer, "partition", &core.partition)?;
        writer.begin_sequence("classifications", core.classifications.len())?;
        for classification in &core.classifications {
            write_leaf_classification_identity(writer, "classification", classification)?;
        }
        writer.end_sequence()?;
        write_when_bad_limits_identity(writer, "limits", core.limits)?;
        write_when_bad_stats_identity(writer, "stats", core.stats)?;
        writer.end_record()
    }
}

fn write_sector_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    sector: &SectorMask,
) -> Result<(), ExactIdentityError> {
    writer.begin_sequence(tag, sector.arity())?;
    for &active in sector.active_bits() {
        writer.boolean("active", active)?;
    }
    writer.end_sequence()
}

fn write_i64_sequence_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    values: &[i64],
) -> Result<(), ExactIdentityError> {
    writer.begin_sequence(tag, values.len())?;
    for &value in values {
        writer.signed_i64("value", value)?;
    }
    writer.end_sequence()
}

fn write_usize_sequence_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    values: &[usize],
) -> Result<(), ExactIdentityError> {
    writer.begin_sequence(tag, values.len())?;
    for &value in values {
        writer.usize("value", value)?;
    }
    writer.end_sequence()
}

fn write_candidate_binding_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    binding: &WhenBadCandidateBinding,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 9)?;
    writer.variant(
        "source_authentication",
        match binding.source_authentication {
            WhenBadSourceAuthentication::AlgebraicOnly => "AlgebraicOnly",
            WhenBadSourceAuthentication::GeneratedCylindricalPersistentEliminationV2 => {
                "GeneratedCylindricalPersistentEliminationV2"
            }
        },
    )?;
    writer.string("family_fingerprint", &binding.family_fingerprint)?;
    writer.string("context_fingerprint", &binding.context_fingerprint)?;
    write_sector_identity(writer, "sector", &binding.sector)?;
    write_ordering_authority_identity(writer, "ordering_authority", &binding.ordering_authority)?;
    write_candidate_source_authority_identity(
        writer,
        "source_authority",
        &binding.source_authority,
    )?;
    writer.usize("pivot_ordinal", binding.pivot_ordinal)?;
    write_i64_sequence_identity(writer, "original_pivot", binding.original_pivot.values())?;
    writer.string(
        "centered_relation_manifest",
        &binding.centered_relation_manifest,
    )?;
    // `retained_bytes` is allocator/ABI dependent and is a replay admission
    // diagnostic, not part of the allocation-independent mathematical value.
    writer.end_record()
}

fn write_ordering_authority_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    authority: &WhenBadOrderingAuthority,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 2)?;
    match authority {
        WhenBadOrderingAuthority::AnchoredV1 {
            policy,
            manifest,
            discovery_anchor,
        } => {
            writer.variant("variant", "AnchoredV1")?;
            writer.begin_record("fields", 3)?;
            writer.variant("policy", policy.stable_id())?;
            writer.string("manifest", manifest)?;
            write_i64_sequence_identity(writer, "discovery_anchor", discovery_anchor)?;
        }
        WhenBadOrderingAuthority::CylindricalV1 { policy, manifest } => {
            writer.variant("variant", "CylindricalV1")?;
            writer.begin_record("fields", 2)?;
            writer.variant("policy", policy.stable_id())?;
            writer.string("manifest", manifest)?;
        }
    }
    writer.end_record()?;
    writer.end_record()
}

fn write_candidate_source_authority_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    authority: &WhenBadCandidateSourceAuthority,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 2)?;
    match authority {
        WhenBadCandidateSourceAuthority::AnchoredEliminationV1 {
            source_manifest,
            source_row_count,
            trace_manifest,
            rule_limits,
        } => {
            writer.variant("variant", "AnchoredEliminationV1")?;
            writer.begin_record("fields", 4)?;
            writer.string("source_manifest", source_manifest)?;
            writer.usize("source_row_count", *source_row_count)?;
            writer.string("trace_manifest", trace_manifest)?;
            write_parametric_rule_limits_identity(writer, "rule_limits", *rule_limits)?;
        }
        WhenBadCandidateSourceAuthority::GeneratedCylindricalPersistentV2 {
            local_candidate_identity,
            source_row_count,
        } => {
            writer.variant("variant", "GeneratedCylindricalPersistentV2")?;
            writer.begin_record("fields", 2)?;
            writer.string("local_candidate_identity", local_candidate_identity)?;
            writer.usize("source_row_count", *source_row_count)?;
        }
    }
    writer.end_record()?;
    writer.end_record()
}

fn write_domain_condition_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    condition: &WhenBadDomainCondition,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 3)?;
    writer.polynomial("polynomial", condition.polynomial().raw())?;
    writer.begin_sequence("sources", condition.sources().len())?;
    for source in condition.sources() {
        write_domain_condition_source_identity(writer, "source", source)?;
    }
    writer.end_sequence()?;
    writer.boolean("index_dependent", condition.is_index_dependent())?;
    writer.end_record()
}

fn write_domain_condition_source_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    source: &WhenBadDomainConditionSource,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 2)?;
    match source {
        WhenBadDomainConditionSource::PersistedGuard { ordinal, origins } => {
            writer.variant("variant", "PersistedGuard")?;
            writer.begin_record("fields", 2)?;
            writer.usize("ordinal", *ordinal)?;
            writer.begin_sequence("origins", origins.len())?;
            for origin in origins {
                writer.guard_origin("origin", origin)?;
            }
            writer.end_sequence()?;
        }
        WhenBadDomainConditionSource::GeneratedCylindricalBaseAssumption {
            witness_ordinal,
            origins,
        } => {
            writer.variant("variant", "GeneratedCylindricalBaseAssumption")?;
            writer.begin_record("fields", 2)?;
            writer.usize("witness_ordinal", *witness_ordinal)?;
            writer.begin_sequence("origins", origins.len())?;
            for origin in origins {
                writer.guard_origin("origin", origin)?;
            }
            writer.end_sequence()?;
        }
        WhenBadDomainConditionSource::CoefficientDenominator { shift } => {
            writer.variant("variant", "CoefficientDenominator")?;
            writer.begin_record("fields", 1)?;
            write_i64_sequence_identity(writer, "shift", shift.values())?;
        }
    }
    writer.end_record()?;
    writer.end_record()
}

fn write_leak_event_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    event: &WhenBadLeakEvent,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 8)?;
    writer.usize("ordinal", event.ordinal)?;
    writer.usize("rhs_ordinal", event.rhs_ordinal)?;
    write_i64_sequence_identity(writer, "rhs_shift", event.rhs_shift.values())?;
    writer.variant(
        "kind",
        match event.kind {
            WhenBadBoundaryHazardKind::InactiveSectorActivation => "InactiveSectorActivation",
            WhenBadBoundaryHazardKind::ConcreteIndexOverflow => "ConcreteIndexOverflow",
        },
    )?;
    writer.usize("coordinate", event.coordinate)?;
    writer.signed_i64("boundary_value", event.boundary_value)?;
    writer.polynomial("boundary_polynomial", event.boundary_polynomial.raw())?;
    writer.begin_record("numerator_gate", 2)?;
    match &event.numerator_gate {
        WhenBadLeakNumeratorGate::CoefficientFieldNonzero(polynomial) => {
            writer.variant("variant", "CoefficientFieldNonzero")?;
            writer.polynomial("polynomial", polynomial.raw())?;
        }
        WhenBadLeakNumeratorGate::Symbolic(polynomial) => {
            writer.variant("variant", "Symbolic")?;
            writer.polynomial("polynomial", polynomial.raw())?;
        }
    }
    writer.end_record()?;
    writer.end_record()
}

fn write_descent_witness_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    witness: &WhenBadUniformDescentWitness,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 8)?;
    writer.variant("policy", witness.policy.stable_id())?;
    writer.usize("rhs_ordinal", witness.rhs_ordinal)?;
    write_i64_sequence_identity(writer, "rhs_shift", witness.rhs_shift.values())?;
    writer.signed_i128("corner_delta", witness.corner_delta)?;
    writer.signed_i128("dot_delta", witness.dot_delta)?;
    writer.signed_i128("numerator_delta", witness.numerator_delta)?;
    writer.begin_sequence("index_excess_deltas", witness.index_excess_deltas.len())?;
    for &delta in &witness.index_excess_deltas {
        writer.signed_i128("delta", delta)?;
    }
    writer.end_sequence()?;
    write_descent_component_identity(writer, "decisive_component", witness.decisive_component)?;
    writer.end_record()
}

fn write_descent_component_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    component: WhenBadDescentComponent,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 2)?;
    match component {
        WhenBadDescentComponent::CornerDistance => {
            writer.variant("variant", "CornerDistance")?;
            writer.begin_record("fields", 0)?;
        }
        WhenBadDescentComponent::DotPower => {
            writer.variant("variant", "DotPower")?;
            writer.begin_record("fields", 0)?;
        }
        WhenBadDescentComponent::NumeratorPower => {
            writer.variant("variant", "NumeratorPower")?;
            writer.begin_record("fields", 0)?;
        }
        WhenBadDescentComponent::IndexExcess { position } => {
            writer.variant("variant", "IndexExcess")?;
            writer.begin_record("fields", 1)?;
            writer.usize("position", position)?;
        }
    }
    writer.end_record()?;
    writer.end_record()
}

fn write_partition_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    partition: &SymbolicSectorCasePartitionCertificate,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 6)?;
    writer.string("schema", partition.schema())?;
    writer.string("context_fingerprint", partition.context_fingerprint())?;
    writer.begin_record("orthant", 2)?;
    write_sector_identity(writer, "sector", partition.orthant().sector())?;
    writer.begin_sequence("constraints", partition.orthant().constraints().len())?;
    for constraint in partition.orthant().constraints() {
        writer.begin_record("constraint", 2)?;
        writer.usize("index", constraint.index())?;
        writer.variant(
            "side",
            match constraint.side() {
                crate::SectorOrthantSide::AtLeastOne => "AtLeastOne",
                crate::SectorOrthantSide::AtMostZero => "AtMostZero",
            },
        )?;
        writer.end_record()?;
    }
    writer.end_sequence()?;
    writer.end_record()?;
    writer.begin_sequence("splits", partition.splits().len())?;
    for split in partition.splits() {
        writer.begin_record("split", 5)?;
        writer.usize("ordinal", split.ordinal())?;
        writer.unsigned_u64("parent", split.parent().value())?;
        writer.polynomial("bad_polynomial", split.bad_polynomial().raw())?;
        writer.unsigned_u64(
            "equal_zero_child",
            split.children().equal_zero_case().value(),
        )?;
        writer.unsigned_u64("nonzero_child", split.children().nonzero_case().value())?;
        writer.end_record()?;
    }
    writer.end_sequence()?;
    writer.begin_sequence("cases", partition.cases().len())?;
    for case in partition.cases() {
        writer.begin_record("case", 2)?;
        writer.unsigned_u64("id", case.id().value())?;
        writer.begin_sequence("predicates", case.predicates().len())?;
        for predicate in case.predicates() {
            writer.begin_record("predicate", 2)?;
            writer.variant(
                "kind",
                match predicate.kind() {
                    SymbolicPolynomialPredicateKind::EqualZero => "EqualZero",
                    SymbolicPolynomialPredicateKind::NonZero => "NonZero",
                },
            )?;
            writer.polynomial("polynomial", predicate.polynomial().raw())?;
            writer.end_record()?;
        }
        writer.end_sequence()?;
        writer.end_record()?;
    }
    writer.end_sequence()?;
    // `source_identity` contains a legacy Symbolica integer rendering. The
    // exact typed orthant/split/case payload above is authoritative and avoids
    // importing formatter stability into this identity schema.
    let stats = partition.stats();
    writer.begin_record("stats", 5)?;
    writer.usize("split_count", stats.split_count())?;
    writer.usize("leaf_count", stats.leaf_count())?;
    writer.usize("max_depth", stats.max_depth())?;
    writer.usize("total_leaf_predicates", stats.total_leaf_predicates())?;
    writer.usize(
        "retained_polynomial_terms",
        stats.retained_polynomial_terms(),
    )?;
    // Display-derived retained polynomial bytes are deliberately excluded.
    writer.end_record()?;
    writer.end_record()
}

fn write_leaf_classification_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    classification: &WhenBadLeafClassification,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 2)?;
    writer.unsigned_u64("case", classification.case.value())?;
    writer.begin_record("disposition", 2)?;
    match &classification.disposition {
        WhenBadLeafDisposition::CoveredByCandidate => {
            writer.variant("variant", "CoveredByCandidate")?;
            writer.begin_record("fields", 0)?;
        }
        WhenBadLeafDisposition::ExceptionalDomain { condition } => {
            writer.variant("variant", "ExceptionalDomain")?;
            writer.begin_record("fields", 1)?;
            writer.usize("condition", *condition)?;
        }
        WhenBadLeafDisposition::ExceptionalSectorLeak { event } => {
            writer.variant("variant", "ExceptionalSectorLeak")?;
            writer.begin_record("fields", 1)?;
            writer.usize("event", *event)?;
        }
    }
    writer.end_record()?;
    writer.end_record()?;
    writer.end_record()
}

fn write_unsupported_reason_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    reason: &WhenBadUnsupportedReason,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 2)?;
    match reason {
        WhenBadUnsupportedReason::NonUniformSameSectorDescent {
            rhs_ordinal,
            rhs_shift,
            first_nonzero_component,
            delta,
        } => {
            writer.variant("variant", "NonUniformSameSectorDescent")?;
            writer.begin_record("fields", 4)?;
            writer.usize("rhs_ordinal", *rhs_ordinal)?;
            write_i64_sequence_identity(writer, "rhs_shift", rhs_shift.values())?;
            write_descent_component_identity(
                writer,
                "first_nonzero_component",
                *first_nonzero_component,
            )?;
            writer.signed_i128("delta", *delta)?;
        }
        WhenBadUnsupportedReason::ZeroSameSectorComplexityDelta {
            rhs_ordinal,
            rhs_shift,
        } => {
            writer.variant("variant", "ZeroSameSectorComplexityDelta")?;
            writer.begin_record("fields", 2)?;
            writer.usize("rhs_ordinal", *rhs_ordinal)?;
            write_i64_sequence_identity(writer, "rhs_shift", rhs_shift.values())?;
        }
        WhenBadUnsupportedReason::UnboundedIndexAddition {
            rhs_ordinal,
            rhs_shift,
            coordinate,
            delta,
        } => {
            writer.variant("variant", "UnboundedIndexAddition")?;
            writer.begin_record("fields", 4)?;
            writer.usize("rhs_ordinal", *rhs_ordinal)?;
            write_i64_sequence_identity(writer, "rhs_shift", rhs_shift.values())?;
            writer.usize("coordinate", *coordinate)?;
            writer.signed_i64("delta", *delta)?;
        }
    }
    writer.end_record()?;
    writer.end_record()
}

fn write_parametric_rule_limits_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    limits: ParametricRuleLimits,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 3)?;
    write_parametric_arithmetic_limits_identity(writer, "arithmetic", limits.arithmetic)?;
    writer.usize("max_rhs_terms", limits.max_rhs_terms)?;
    writer.usize(
        "max_source_rows_for_replay",
        limits.max_source_rows_for_replay,
    )?;
    writer.end_record()
}

fn write_parametric_arithmetic_limits_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    limits: ParametricArithmeticLimits,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 6)?;
    write_exact_algebra_limits_identity(writer, "exact_algebra", limits.exact_algebra)?;
    writer.usize("max_source_terms", limits.max_source_terms)?;
    writer.usize("max_output_terms", limits.max_output_terms)?;
    writer.usize(
        "max_specialization_power_operations",
        limits.max_specialization_power_operations,
    )?;
    writer.usize(
        "max_specialization_integer_bits",
        limits.max_specialization_integer_bits,
    )?;
    writer.usize("max_guard_origins", limits.max_guard_origins)?;
    writer.end_record()
}

fn write_exact_algebra_limits_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    limits: crate::algebra::ExactAlgebraLimits,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 3)?;
    writer.unsigned_u128("max_exponent", limits.max_exponent)?;
    writer.usize("max_polynomial_terms", limits.max_polynomial_terms)?;
    writer.usize("max_term_operations", limits.max_term_operations)?;
    writer.end_record()
}

pub(crate) fn write_symbolic_sector_case_limits_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    limits: SymbolicSectorCaseLimits,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 10)?;
    write_exact_algebra_limits_identity(writer, "exact_algebra", limits.exact_algebra)?;
    writer.usize("max_indices", limits.max_indices)?;
    writer.usize(
        "max_context_fingerprint_bytes",
        limits.max_context_fingerprint_bytes,
    )?;
    writer.usize("max_splits", limits.max_splits)?;
    writer.usize("max_live_cases", limits.max_live_cases)?;
    writer.usize("max_predicates_per_case", limits.max_predicates_per_case)?;
    writer.usize(
        "max_total_leaf_predicates",
        limits.max_total_leaf_predicates,
    )?;
    writer.usize(
        "max_retained_polynomial_terms",
        limits.max_retained_polynomial_terms,
    )?;
    writer.usize(
        "max_retained_polynomial_bytes",
        limits.max_retained_polynomial_bytes,
    )?;
    writer.usize(
        "max_source_identity_bytes",
        limits.max_source_identity_bytes,
    )?;
    writer.end_record()
}

pub(crate) fn write_when_bad_limits_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    limits: WhenBadCompilerLimits,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 18)?;
    write_parametric_arithmetic_limits_identity(writer, "arithmetic", limits.arithmetic)?;
    write_symbolic_sector_case_limits_identity(writer, "sector_cases", limits.sector_cases)?;
    writer.usize("max_rhs_terms", limits.max_rhs_terms)?;
    writer.usize("max_domain_conditions", limits.max_domain_conditions)?;
    writer.usize(
        "max_domain_condition_sources",
        limits.max_domain_condition_sources,
    )?;
    writer.usize("max_guard_origins", limits.max_guard_origins)?;
    writer.usize(
        "max_guard_origin_retained_bytes",
        limits.max_guard_origin_retained_bytes,
    )?;
    writer.usize(
        "max_boundary_values_per_rhs",
        limits.max_boundary_values_per_rhs,
    )?;
    writer.usize("max_boundary_values", limits.max_boundary_values)?;
    writer.usize("max_leak_events", limits.max_leak_events)?;
    writer.usize(
        "max_leak_event_shift_components",
        limits.max_leak_event_shift_components,
    )?;
    writer.usize(
        "max_leak_event_retained_bytes",
        limits.max_leak_event_retained_bytes,
    )?;
    writer.usize("max_descent_witnesses", limits.max_descent_witnesses)?;
    writer.usize(
        "max_descent_witness_components",
        limits.max_descent_witness_components,
    )?;
    writer.usize("max_leaf_classifications", limits.max_leaf_classifications)?;
    writer.usize(
        "max_candidate_binding_bytes",
        limits.max_candidate_binding_bytes,
    )?;
    writer.usize(
        "max_retained_condition_terms",
        limits.max_retained_condition_terms,
    )?;
    writer.usize(
        "max_retained_condition_bytes",
        limits.max_retained_condition_bytes,
    )?;
    writer.end_record()
}

fn write_when_bad_stats_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    stats: WhenBadCompilerStats,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 13)?;
    writer.usize("rhs_terms", stats.rhs_terms())?;
    writer.usize("domain_conditions", stats.domain_conditions())?;
    writer.usize("base_domain_guards", stats.base_domain_guards())?;
    writer.usize("domain_condition_sources", stats.domain_condition_sources())?;
    writer.usize("guard_origins", stats.guard_origins())?;
    writer.usize("retained_condition_terms", stats.retained_condition_terms())?;
    writer.usize("index_domain_guards", stats.index_domain_guards())?;
    writer.usize("boundary_values_examined", stats.boundary_values_examined())?;
    writer.usize("leak_events", stats.leak_events())?;
    writer.usize(
        "leak_event_shift_components",
        stats.leak_event_shift_components(),
    )?;
    // Origin/event retained bytes are capacity/`size_of` diagnostics and
    // condition display bytes depend on Symbolica's formatter. Exact origins,
    // shifts, and sparse polynomials above already bind their stable content.
    writer.usize("descent_witnesses", stats.descent_witnesses())?;
    writer.usize(
        "descent_witness_components",
        stats.descent_witness_components(),
    )?;
    writer.usize("leaf_classifications", stats.leaf_classifications())?;
    // `retained_core_bytes` is capacity/`size_of` derived and intentionally
    // excluded from the durable value.
    writer.end_record()
}

impl WhenBadCertifiedCore {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) const fn binding(&self) -> &WhenBadCandidateBinding {
        &self.binding
    }

    pub(crate) fn domain_conditions(&self) -> &[WhenBadDomainCondition] {
        &self.domain_conditions
    }

    pub(crate) fn base_domain_guards(&self) -> impl Iterator<Item = &WhenBadDomainCondition> {
        self.base_domain_guards
            .iter()
            .map(|&ordinal| &self.domain_conditions[ordinal])
    }

    pub(crate) fn index_domain_guards(&self) -> impl Iterator<Item = &WhenBadDomainCondition> {
        self.index_domain_guards
            .iter()
            .map(|&ordinal| &self.domain_conditions[ordinal])
    }

    pub(crate) fn index_domain_guards_with_ordinals(
        &self,
    ) -> impl Iterator<Item = (usize, &WhenBadDomainCondition)> {
        self.index_domain_guards
            .iter()
            .copied()
            .map(|ordinal| (ordinal, &self.domain_conditions[ordinal]))
    }

    pub(crate) fn leak_events(&self) -> &[WhenBadLeakEvent] {
        &self.leak_events
    }

    pub(crate) fn descent_witnesses(&self) -> &[WhenBadUniformDescentWitness] {
        &self.descent_witnesses
    }

    pub(crate) const fn partition(&self) -> &SymbolicSectorCasePartitionCertificate {
        &self.partition
    }

    pub(crate) fn classifications(&self) -> &[WhenBadLeafClassification] {
        &self.classifications
    }

    pub(crate) const fn limits(&self) -> WhenBadCompilerLimits {
        self.limits
    }

    pub(crate) const fn stats(&self) -> WhenBadCompilerStats {
        self.stats
    }

    pub(crate) const fn retained_core_bytes(&self) -> usize {
        self.stats.retained_core_bytes
    }

    pub(crate) fn replay_capacity_census(&self) -> Result<(), WhenBadCompilerError> {
        replay_candidate_binding_capacity(&self.binding, self.limits)?;
        let source_census = retained_domain_source_census(&self.domain_conditions, self.limits)?;
        let leak_census = retained_leak_event_census(&self.leak_events, self.limits)?;
        if source_census.origin_bytes > self.stats.guard_origin_retained_bytes
            || self.stats.guard_origin_retained_bytes > self.limits.max_guard_origin_retained_bytes
            || leak_census.retained_bytes > self.stats.leak_event_retained_bytes
            || self.stats.leak_event_retained_bytes > self.limits.max_leak_event_retained_bytes
            || self.observed_retained_core_bytes()? > self.stats.retained_core_bytes
        {
            return Err(WhenBadCompilerError::ReplayMismatch);
        }
        Ok(())
    }

    fn capacity_payload_is_valid(&self) -> bool {
        self.replay_capacity_census().is_ok()
    }

    pub(crate) fn classification_for_indices(
        &self,
        context: &ParametricCoefficientContext,
        indices: &[i64],
    ) -> Result<Option<&WhenBadLeafClassification>, WhenBadCompilerError> {
        if context.fingerprint() != self.binding.context_fingerprint() {
            return Err(WhenBadCompilerError::ContextMismatch);
        }
        if !self.partition.orthant().contains_integer_point(indices)? {
            return Ok(None);
        }
        let mut matched = None;
        for case in self.partition.cases() {
            let mut accepts = true;
            for predicate in case.predicates() {
                let specialized = context.specialize_polynomial(
                    predicate.polynomial(),
                    indices,
                    self.limits.arithmetic,
                )?;
                accepts &= match predicate.kind() {
                    SymbolicPolynomialPredicateKind::EqualZero => specialized.is_zero(),
                    SymbolicPolynomialPredicateKind::NonZero => !specialized.is_zero(),
                };
            }
            if accepts {
                if matched.is_some() {
                    return Err(WhenBadCompilerError::PartitionEvaluationMismatch);
                }
                matched = self
                    .classifications
                    .iter()
                    .find(|classification| classification.case == case.id());
            }
        }
        matched
            .map(Some)
            .ok_or(WhenBadCompilerError::PartitionEvaluationMismatch)
    }

    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.capacity_payload_is_valid()
            && other.capacity_payload_is_valid()
            && self.schema == other.schema
            && self.binding == other.binding
            && self.domain_conditions == other.domain_conditions
            && self.base_domain_guards == other.base_domain_guards
            && self.index_domain_guards == other.index_domain_guards
            && self.leak_events == other.leak_events
            && self.descent_witnesses == other.descent_witnesses
            && self.partition == other.partition
            && self.classifications == other.classifications
            && self.limits == other.limits
            && self.stats == other.stats
    }
}

/// Sealed pre-rule candidate view consumed by the single `WhenBad` algorithm.
/// In particular there is no locus-bound arm and no caller-implementable
/// trait which could manufacture global authority.
#[derive(Clone, Copy)]
enum WhenBadGlobalCandidateView<'a> {
    AnchoredV1(&'a ParametricReductionRuleCandidate),
    CylindricalGlobalV1(&'a GeneratedCylindricalGlobalCandidateAuthority),
}

impl<'a> WhenBadGlobalCandidateView<'a> {
    fn family_fingerprint(self) -> &'a str {
        match self {
            Self::AnchoredV1(candidate) => candidate.family_fingerprint(),
            Self::CylindricalGlobalV1(candidate) => candidate.family_fingerprint(),
        }
    }

    fn context_fingerprint(self) -> &'a str {
        match self {
            Self::AnchoredV1(candidate) => candidate.context_fingerprint(),
            Self::CylindricalGlobalV1(candidate) => candidate.context_fingerprint(),
        }
    }

    fn sector(self) -> &'a SectorMask {
        match self {
            Self::AnchoredV1(candidate) => candidate.sector(),
            Self::CylindricalGlobalV1(candidate) => candidate.sector(),
        }
    }

    fn ordering_policy(self) -> IntegralOrderingPolicy {
        match self {
            Self::AnchoredV1(candidate) => candidate.ordering().policy(),
            Self::CylindricalGlobalV1(candidate) => candidate.ordering_policy(),
        }
    }

    fn pivot_ordinal(self) -> usize {
        match self {
            Self::AnchoredV1(candidate) => candidate.pivot_ordinal(),
            Self::CylindricalGlobalV1(candidate) => candidate.pivot_ordinal(),
        }
    }

    fn original_pivot(self) -> &'a IndexShift {
        match self {
            Self::AnchoredV1(candidate) => candidate.original_pivot(),
            Self::CylindricalGlobalV1(candidate) => candidate.original_pivot(),
        }
    }

    fn centered_relation(self) -> &'a ParametricRelation {
        match self {
            Self::AnchoredV1(candidate) => candidate.centered_relation(),
            Self::CylindricalGlobalV1(candidate) => {
                candidate.centered_relation_for_generated_when_bad()
            }
        }
    }
}

/// Candidate-independent result returned by the sealed shared compiler. A
/// generated wrapper must retain the exact cylindrical Global authority next
/// to this payload before it can become an application certificate.
#[derive(Clone, Debug)]
pub(crate) enum WhenBadCoreCompilation {
    Certified(WhenBadCertifiedCore),
    Unsupported(WhenBadUnsupportedCore),
}

#[derive(Clone, Debug)]
pub enum WhenBadCompilation {
    Certified(WhenBadCertificate),
    Unsupported(WhenBadUnsupported),
}

impl WhenBadCompilation {
    pub fn retained_core_bytes(&self) -> usize {
        match self {
            Self::Certified(certificate) => certificate.retained_core_bytes(),
            Self::Unsupported(unsupported) => unsupported.retained_core_bytes(),
        }
    }
}

pub struct WhenBadCompiler;

impl WhenBadCompiler {
    /// Allocation-free fixed lower bounds for an exhaustive cylindrical
    /// batch. Only the index arity is consumed on every low-level path;
    /// partition-builder limits cannot be preflighted here because a pivot
    /// may return `Unsupported` before any symbolic sector case is built.
    /// Candidate-dependent RHS, guard, case, and retained-payload checks
    /// remain in the authenticated per-pivot compiler.
    pub(crate) fn preflight_replayed_cylindrical_batch_fixed_limits(
        context: &ParametricCoefficientContext,
        pivot_count: usize,
        limits: WhenBadCompilerLimits,
    ) -> Result<(), WhenBadCompilerError> {
        if pivot_count == 0 {
            return Ok(());
        }
        check_limit(
            "WhenBad indices",
            context.index_count(),
            limits.sector_cases.max_indices,
        )
    }

    /// Compile a self-replaying algebraic candidate. The output is explicitly
    /// *not* authenticated as freshly generated IBP/LI source; provider wiring
    /// must use a future canonical-source wrapper.
    ///
    /// This borrowed compatibility entry point must deep-clone the candidate
    /// before the certificate can retain it. New owners should prefer
    /// [`Self::compile_algebraic_candidate_arc`] to share an existing `Arc`
    /// without that infallible proportional clone.
    pub fn compile_algebraic_candidate(
        context: &ParametricCoefficientContext,
        candidate: &ParametricReductionRuleCandidate,
        limits: WhenBadCompilerLimits,
    ) -> Result<WhenBadCompilation, WhenBadCompilerError> {
        Self::replay_algebraic_candidate_input(context, candidate, limits)?;
        Self::compile_replayed_algebraic_candidate_arc(context, Arc::new(candidate.clone()), limits)
    }

    /// Compile and retain an already shared algebraic candidate.
    ///
    /// This is the preferred ownership-aware entry point. It preserves the
    /// exact `Arc` allocation supplied by the caller and performs no deep
    /// candidate clone.
    pub fn compile_algebraic_candidate_arc(
        context: &ParametricCoefficientContext,
        candidate: Arc<ParametricReductionRuleCandidate>,
        limits: WhenBadCompilerLimits,
    ) -> Result<WhenBadCompilation, WhenBadCompilerError> {
        Self::replay_algebraic_candidate_input(context, candidate.as_ref(), limits)?;
        Self::compile_replayed_algebraic_candidate_arc(context, candidate, limits)
    }

    fn replay_algebraic_candidate_input(
        context: &ParametricCoefficientContext,
        candidate: &ParametricReductionRuleCandidate,
        limits: WhenBadCompilerLimits,
    ) -> Result<(), WhenBadCompilerError> {
        if candidate.context_fingerprint() != context.fingerprint() {
            return Err(WhenBadCompilerError::ContextMismatch);
        }
        check_limit(
            "WhenBad indices",
            context.index_count(),
            limits.sector_cases.max_indices,
        )?;
        candidate.replay_retained(context)?;
        Ok(())
    }

    fn compile_replayed_algebraic_candidate_arc(
        context: &ParametricCoefficientContext,
        candidate: Arc<ParametricReductionRuleCandidate>,
        limits: WhenBadCompilerLimits,
    ) -> Result<WhenBadCompilation, WhenBadCompilerError> {
        let core = compile_replayed_global_view(
            context,
            WhenBadGlobalCandidateView::AnchoredV1(candidate.as_ref()),
            limits,
        )?;
        Ok(match core {
            WhenBadCoreCompilation::Certified(core) => {
                WhenBadCompilation::Certified(WhenBadCertificate { candidate, core })
            }
            WhenBadCoreCompilation::Unsupported(core) => {
                WhenBadCompilation::Unsupported(WhenBadUnsupported { candidate, core })
            }
        })
    }

    /// Compile the shared symbolic-domain core for an authenticated global
    /// cylindrical candidate. The exact parameter type deliberately excludes
    /// the umbrella and locus-bound candidate arms.
    ///
    /// This remains crate-private and returns no applicable rule. A future
    /// generated wrapper must retain the candidate next to this core proof.
    pub(crate) fn compile_cylindrical_global_candidate(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        candidate: &GeneratedCylindricalGlobalCandidateAuthority,
        limits: WhenBadCompilerLimits,
    ) -> Result<WhenBadCoreCompilation, WhenBadCompilerError> {
        // Preserve the public compiler's typed scope errors before entering
        // the lower-level authenticated replay operation. The replay layer
        // intentionally reports a generic shallow-binding mismatch for the
        // same condition, but callers of this wrapper can distinguish the two
        // independent scope coordinates without weakening authentication.
        if candidate.family_fingerprint() != family.fingerprint_ref() {
            return Err(WhenBadCompilerError::FamilyMismatch);
        }
        if candidate.context_fingerprint() != context.fingerprint() {
            return Err(WhenBadCompilerError::ContextMismatch);
        }
        let mut session = GeneratedCylindricalReplaySession::new(family, context);
        let replayed_candidate = candidate.replay_with_replay_session(&mut session)?;
        Self::compile_replayed_cylindrical_global_candidate(replayed_candidate, limits)
    }

    /// Compile from a sealed candidate-local replay capability. The context is
    /// derived from the operation scope, so callers cannot pair the candidate
    /// with a foreign algebra context while bypassing nested source replay.
    pub(crate) fn compile_replayed_cylindrical_global_candidate(
        replayed_candidate: ReplayedGeneratedCylindricalGlobalCandidate<'_, '_, '_>,
        limits: WhenBadCompilerLimits,
    ) -> Result<WhenBadCoreCompilation, WhenBadCompilerError> {
        let family = replayed_candidate.family();
        let context = replayed_candidate.context();
        let candidate = replayed_candidate.candidate();
        if candidate.family_fingerprint() != family.fingerprint_ref() {
            return Err(WhenBadCompilerError::FamilyMismatch);
        }
        if candidate.context_fingerprint() != context.fingerprint() {
            return Err(WhenBadCompilerError::ContextMismatch);
        }
        check_limit(
            "WhenBad indices",
            context.index_count(),
            limits.sector_cases.max_indices,
        )?;
        let core = compile_replayed_global_view(
            context,
            WhenBadGlobalCandidateView::CylindricalGlobalV1(candidate),
            limits,
        )?;
        #[cfg(test)]
        record_replayed_cylindrical_core_construction_for_test();
        Ok(core)
    }
}

fn compile_replayed_global_view(
    context: &ParametricCoefficientContext,
    candidate: WhenBadGlobalCandidateView<'_>,
    limits: WhenBadCompilerLimits,
) -> Result<WhenBadCoreCompilation, WhenBadCompilerError> {
    let binding = candidate_binding(candidate, limits.max_candidate_binding_bytes)?;

    let zero = crate::IndexSpace::try_new(context.index_count())?.try_zero()?;
    let rhs_count = candidate
        .centered_relation()
        .terms()
        .keys()
        .filter(|shift| *shift != &zero)
        .count();
    check_limit("WhenBad RHS terms", rhs_count, limits.max_rhs_terms)?;
    check_limit(
        "WhenBad descent witnesses",
        rhs_count,
        limits.max_descent_witnesses,
    )?;
    let descent_witness_components = checked_mul(
        "WhenBad descent witness components",
        rhs_count,
        candidate.sector().arity(),
    )?;
    check_limit(
        "WhenBad descent witness components",
        descent_witness_components,
        limits.max_descent_witness_components,
    )?;
    let mut rhs = Vec::new();
    try_reserve_core("WhenBad RHS references", &mut rhs, rhs_count)?;
    rhs.extend(
        candidate
            .centered_relation()
            .terms()
            .iter()
            .filter(|(shift, _)| *shift != &zero),
    );
    debug_assert_eq!(rhs.len(), rhs_count);

    let mut descent_witnesses = Vec::new();
    try_reserve_core(
        "WhenBad descent witnesses",
        &mut descent_witnesses,
        rhs.len(),
    )?;
    for (rhs_ordinal, (shift, _)) in rhs.iter().enumerate() {
        match prove_uniform_same_sector_descent_with_policy(
            candidate.ordering_policy(),
            candidate.sector(),
            rhs_ordinal,
            shift,
        )? {
            Ok(witness) => descent_witnesses.push(witness),
            Err(reason) => {
                return Ok(WhenBadCoreCompilation::Unsupported(
                    WhenBadUnsupportedCore::try_new(binding, reason, limits)?,
                ));
            }
        }
    }

    let mut domain_conditions = Vec::<WhenBadDomainCondition>::new();
    let mut retained_census = RetainedPolynomialCensus::default();
    for (ordinal, condition) in candidate
        .centered_relation()
        .guarded_nonzero_conditions()
        .iter()
        .enumerate()
    {
        insert_borrowed_domain_condition(
            context,
            &mut domain_conditions,
            condition.polynomial(),
            BorrowedWhenBadDomainConditionSource::PersistedGuard {
                ordinal,
                origins: condition.origins(),
            },
            limits,
            &mut retained_census,
        )?;
    }
    if let WhenBadGlobalCandidateView::CylindricalGlobalV1(candidate) = candidate {
        for resolved in candidate.base_assumptions() {
            let witness_ordinal = resolved.witness().ordinal();
            let condition = resolved.condition();
            if context.polynomial_depends_on_indices_with_limits(
                condition.polynomial(),
                limits.arithmetic.exact_algebra,
            )? {
                return Err(
                    WhenBadCompilerError::IndexDependentCylindricalBaseAssumption {
                        witness_ordinal,
                    },
                );
            }
            insert_borrowed_domain_condition(
                context,
                &mut domain_conditions,
                condition.polynomial(),
                BorrowedWhenBadDomainConditionSource::GeneratedCylindricalBaseAssumption {
                    witness_ordinal,
                    origins: condition.origins(),
                },
                limits,
                &mut retained_census,
            )?;
        }
    }
    for (shift, coefficient) in candidate.centered_relation().terms() {
        let denominator = context
            .denominator_condition_with_limits(coefficient, limits.arithmetic.exact_algebra)?;
        if !denominator.is_nonzero_constant() {
            insert_borrowed_domain_condition(
                context,
                &mut domain_conditions,
                &denominator,
                BorrowedWhenBadDomainConditionSource::CoefficientDenominator { shift },
                limits,
                &mut retained_census,
            )?;
        }
    }

    let index_domain_guard_count = domain_conditions
        .iter()
        .filter(|condition| condition.index_dependent)
        .count();
    let base_domain_guard_count = domain_conditions
        .len()
        .checked_sub(index_domain_guard_count)
        .ok_or(WhenBadCompilerError::ResourceCountOverflow {
            resource: "WhenBad base-domain guards",
        })?;
    check_limit(
        "WhenBad base-domain guards",
        base_domain_guard_count,
        limits.max_domain_conditions,
    )?;
    check_limit(
        "WhenBad index-domain guards",
        index_domain_guard_count,
        limits.max_domain_conditions,
    )?;
    let mut base_domain_guards = Vec::new();
    try_reserve_core(
        "WhenBad base-domain guards",
        &mut base_domain_guards,
        base_domain_guard_count,
    )?;
    let mut index_domain_guards = Vec::new();
    try_reserve_core(
        "WhenBad index-domain guards",
        &mut index_domain_guards,
        index_domain_guard_count,
    )?;
    for (ordinal, condition) in domain_conditions.iter().enumerate() {
        if condition.index_dependent {
            index_domain_guards.push(ordinal);
        } else {
            base_domain_guards.push(ordinal);
        }
    }
    debug_assert_eq!(base_domain_guards.len(), base_domain_guard_count);
    debug_assert_eq!(index_domain_guards.len(), index_domain_guard_count);

    let boundary_value_bound = boundary_value_bound(candidate.sector(), &rhs, limits)?;
    let mut boundary_values_examined = 0usize;
    let mut leak_events = Vec::new();
    let mut leak_event_shift_components = 0usize;
    let mut leak_event_shift_bytes = 0usize;
    for (rhs_ordinal, (shift, coefficient)) in rhs.iter().enumerate() {
        let numerator = context
            .numerator_condition_with_limits(coefficient, limits.arithmetic.exact_algebra)?;
        for (coordinate, (&active, &delta)) in candidate
            .sector()
            .active_bits()
            .iter()
            .zip(shift.values())
            .enumerate()
        {
            let Some(hazard) = finite_boundary_hazard_range(active, delta, coordinate)? else {
                continue;
            };
            let mut boundary_value = hazard.first();
            loop {
                boundary_values_examined =
                    checked_add("WhenBad boundary values", boundary_values_examined, 1)?;
                let boundary_numerator = context.specialize_polynomial_index(
                    &numerator,
                    coordinate,
                    boundary_value,
                    limits.arithmetic,
                )?;
                if boundary_numerator.is_zero() {
                    if boundary_value == hazard.last() {
                        break;
                    }
                    boundary_value = boundary_value
                        .checked_add(1)
                        .ok_or(WhenBadCompilerError::BoundaryArithmeticOverflow { coordinate })?;
                    continue;
                }
                let ordinal = leak_events.len();
                let requested_event_count =
                    ordinal
                        .checked_add(1)
                        .ok_or(WhenBadCompilerError::ResourceCountOverflow {
                            resource: "WhenBad leak events",
                        })?;
                check_limit(
                    "WhenBad leak events",
                    requested_event_count,
                    limits.max_leak_events,
                )?;
                let requested_shift_components = checked_add(
                    "WhenBad leak-event shift components",
                    leak_event_shift_components,
                    shift.arity(),
                )?;
                check_limit(
                    "WhenBad leak-event shift components",
                    requested_shift_components,
                    limits.max_leak_event_shift_components,
                )?;
                let minimum_shift_bytes = checked_mul(
                    "WhenBad leak-event retained bytes",
                    shift.arity(),
                    size_of::<i64>(),
                )?;
                let minimum_requested_shift_bytes = checked_add(
                    "WhenBad leak-event retained bytes",
                    leak_event_shift_bytes,
                    minimum_shift_bytes,
                )?;
                let minimum_event_buffer_bytes = checked_mul(
                    "WhenBad leak-event retained bytes",
                    requested_event_count,
                    size_of::<WhenBadLeakEvent>(),
                )?;
                let minimum_retained_bytes = checked_add(
                    "WhenBad leak-event retained bytes",
                    minimum_event_buffer_bytes,
                    minimum_requested_shift_bytes,
                )?;
                check_limit(
                    "WhenBad leak-event retained bytes",
                    minimum_retained_bytes,
                    limits.max_leak_event_retained_bytes,
                )?;

                // Admit both proportional retained allocations before either
                // is committed to the proof. `IndexShift::try_new` is the
                // fallible counterpart of the former infallible clone.
                try_reserve_core("WhenBad leak events", &mut leak_events, 1)?;
                let rhs_shift = IndexShift::try_new(shift.values().iter().copied(), shift.arity())?;
                let retained_shift_bytes = rhs_shift.owned_retained_byte_bound().ok_or(
                    WhenBadCompilerError::ResourceCountOverflow {
                        resource: "WhenBad leak-event retained bytes",
                    },
                )?;
                let requested_shift_bytes = checked_add(
                    "WhenBad leak-event retained bytes",
                    leak_event_shift_bytes,
                    retained_shift_bytes,
                )?;
                let retained_event_buffer_bytes = checked_mul(
                    "WhenBad leak-event retained bytes",
                    leak_events.capacity(),
                    size_of::<WhenBadLeakEvent>(),
                )?;
                let requested_retained_bytes = checked_add(
                    "WhenBad leak-event retained bytes",
                    retained_event_buffer_bytes,
                    requested_shift_bytes,
                )?;
                check_limit(
                    "WhenBad leak-event retained bytes",
                    requested_retained_bytes,
                    limits.max_leak_event_retained_bytes,
                )?;
                let boundary_polynomial =
                    boundary_polynomial(context, coordinate, boundary_value, limits.arithmetic)?;
                let numerator_gate = if context.polynomial_depends_on_indices_with_limits(
                    &boundary_numerator,
                    limits.arithmetic.exact_algebra,
                )? {
                    WhenBadLeakNumeratorGate::Symbolic(boundary_numerator)
                } else {
                    WhenBadLeakNumeratorGate::CoefficientFieldNonzero(boundary_numerator)
                };
                // The event owns one boundary polynomial and one
                // numerator-gate polynomial in addition to any copies
                // charged independently by the partition transcript.
                charge_retained_polynomial(&boundary_polynomial, limits, &mut retained_census)?;
                charge_retained_polynomial(
                    match &numerator_gate {
                        WhenBadLeakNumeratorGate::CoefficientFieldNonzero(polynomial)
                        | WhenBadLeakNumeratorGate::Symbolic(polynomial) => polynomial,
                    },
                    limits,
                    &mut retained_census,
                )?;
                leak_events.push(WhenBadLeakEvent {
                    ordinal,
                    rhs_ordinal,
                    rhs_shift,
                    kind: hazard.kind(),
                    coordinate,
                    boundary_value,
                    boundary_polynomial,
                    numerator_gate,
                });
                leak_event_shift_components = requested_shift_components;
                leak_event_shift_bytes = requested_shift_bytes;
                if boundary_value == hazard.last() {
                    break;
                }
                boundary_value = boundary_value
                    .checked_add(1)
                    .ok_or(WhenBadCompilerError::BoundaryArithmeticOverflow { coordinate })?;
            }
        }
    }
    debug_assert_eq!(boundary_values_examined, boundary_value_bound);
    let leak_event_retained_bytes = checked_add(
        "WhenBad leak-event retained bytes",
        checked_mul(
            "WhenBad leak-event retained bytes",
            leak_events.capacity(),
            size_of::<WhenBadLeakEvent>(),
        )?,
        leak_event_shift_bytes,
    )?;
    check_limit(
        "WhenBad leak-event retained bytes",
        leak_event_retained_bytes,
        limits.max_leak_event_retained_bytes,
    )?;

    // A later predicate can split every still-continuing leaf, so a
    // linear estimate from the number of predicates is not a sound upper
    // bound.  Enforce the classification cap transactionally inside the
    // partition builder, before each split allocates either child.
    let mut sector_case_limits = limits.sector_cases;
    sector_case_limits.max_live_cases = sector_case_limits
        .max_live_cases
        .min(limits.max_leaf_classifications);
    let mut builder = SymbolicSectorCasePartitionBuilder::try_new(
        context,
        SectorMask::try_new(candidate.sector().active_bits().iter().copied())?,
        sector_case_limits,
    )?;
    let mut continuing = BTreeSet::from([builder.root_case()]);
    let mut dispositions = BTreeMap::<SymbolicSectorCaseId, WhenBadLeafDisposition>::new();

    // LiteRed processes denominator/nonzero conditions before RHS leaks.
    for &condition_ordinal in &index_domain_guards {
        let polynomial = domain_conditions[condition_ordinal].polynomial.clone();
        let mut next = BTreeSet::new();
        for case in continuing {
            let (equal, nonzero) =
                route_polynomial(&mut builder, context, case, polynomial.clone())?;
            if let Some(equal) = equal {
                dispositions.insert(
                    equal,
                    WhenBadLeafDisposition::ExceptionalDomain {
                        condition: condition_ordinal,
                    },
                );
            }
            if let Some(nonzero) = nonzero {
                next.insert(nonzero);
            }
        }
        continuing = next;
    }

    for event in &leak_events {
        let mut next = BTreeSet::new();
        for case in continuing {
            let (on_boundary, off_boundary) = route_polynomial(
                &mut builder,
                context,
                case,
                event.boundary_polynomial.clone(),
            )?;
            if let Some(off_boundary) = off_boundary {
                next.insert(off_boundary);
            }
            let Some(on_boundary) = on_boundary else {
                continue;
            };
            match &event.numerator_gate {
                WhenBadLeakNumeratorGate::CoefficientFieldNonzero(_) => {
                    dispositions.insert(
                        on_boundary,
                        WhenBadLeafDisposition::ExceptionalSectorLeak {
                            event: event.ordinal,
                        },
                    );
                }
                WhenBadLeakNumeratorGate::Symbolic(polynomial) => {
                    let (zero, nonzero) =
                        route_polynomial(&mut builder, context, on_boundary, polynomial.clone())?;
                    if let Some(zero) = zero {
                        next.insert(zero);
                    }
                    if let Some(nonzero) = nonzero {
                        dispositions.insert(
                            nonzero,
                            WhenBadLeafDisposition::ExceptionalSectorLeak {
                                event: event.ordinal,
                            },
                        );
                    }
                }
            }
        }
        continuing = next;
    }

    for case in continuing {
        dispositions.insert(case, WhenBadLeafDisposition::CoveredByCandidate);
    }
    let partition = builder.finish(context)?;
    check_limit(
        "WhenBad leaf classifications",
        partition.cases().len(),
        limits.max_leaf_classifications,
    )?;
    let mut classifications = Vec::new();
    try_reserve_core(
        "WhenBad leaf classifications",
        &mut classifications,
        partition.cases().len(),
    )?;
    for case in partition.cases() {
        classifications.push(WhenBadLeafClassification {
            case: case.id(),
            disposition: dispositions
                .remove(&case.id())
                .ok_or(WhenBadCompilerError::InternalClassificationMismatch)?,
        });
    }
    if !dispositions.is_empty() {
        return Err(WhenBadCompilerError::InternalClassificationMismatch);
    }

    // Re-census the materialized source payload so allocator spare capacity in
    // copied guard-origin vectors is admitted and replay-bound, rather than
    // trusting only the allocation-free logical preflight.
    let retained_sources = retained_domain_source_census(&domain_conditions, limits)?;
    let stats = WhenBadCompilerStats {
        rhs_terms: rhs.len(),
        domain_conditions: domain_conditions.len(),
        domain_condition_sources: retained_sources.sources,
        guard_origins: retained_sources.origins,
        guard_origin_retained_bytes: retained_sources.origin_bytes,
        base_domain_guards: base_domain_guards.len(),
        index_domain_guards: index_domain_guards.len(),
        boundary_values_examined,
        leak_events: leak_events.len(),
        leak_event_shift_components,
        leak_event_retained_bytes,
        descent_witnesses: descent_witnesses.len(),
        descent_witness_components,
        leaf_classifications: classifications.len(),
        retained_condition_terms: retained_census.terms,
        retained_condition_bytes: retained_census.bytes,
        retained_core_bytes: 0,
    };
    let mut core = WhenBadCertifiedCore {
        schema: WHEN_BAD_COMPILER_V2_SCHEMA,
        binding,
        domain_conditions,
        base_domain_guards,
        index_domain_guards,
        leak_events,
        descent_witnesses,
        partition,
        classifications,
        limits,
        stats,
    };
    core.stats.retained_core_bytes = core.observed_retained_core_bytes()?;
    core.replay_payload_without_recompile(context, candidate)?;
    Ok(WhenBadCoreCompilation::Certified(core))
}

impl WhenBadCertificate {
    fn replay_payload_without_recompile(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), WhenBadCompilerError> {
        self.candidate.replay_retained(context)?;
        self.core.replay_payload_without_recompile(
            context,
            WhenBadGlobalCandidateView::AnchoredV1(&self.candidate),
        )
    }
}

impl WhenBadCertifiedCore {
    fn replay_payload_without_recompile(
        &self,
        context: &ParametricCoefficientContext,
        candidate: WhenBadGlobalCandidateView<'_>,
    ) -> Result<(), WhenBadCompilerError> {
        if self.schema != WHEN_BAD_COMPILER_V2_SCHEMA {
            return Err(WhenBadCompilerError::SchemaMismatch);
        }
        self.replay_capacity_census()?;
        let binding = candidate_binding(candidate, self.limits.max_candidate_binding_bytes)?;
        let zero = crate::IndexSpace::try_new(context.index_count())?.try_zero()?;
        let rhs_count = candidate
            .centered_relation()
            .terms()
            .keys()
            .filter(|shift| *shift != &zero)
            .count();
        check_limit("WhenBad RHS terms", rhs_count, self.limits.max_rhs_terms)?;
        let mut rhs = Vec::new();
        try_reserve_core("WhenBad replay RHS references", &mut rhs, rhs_count)?;
        rhs.extend(
            candidate
                .centered_relation()
                .terms()
                .iter()
                .filter(|(shift, _)| *shift != &zero),
        );
        let boundary_values_examined = boundary_value_bound(candidate.sector(), &rhs, self.limits)?;
        let retained =
            retained_payload_census(&self.domain_conditions, &self.leak_events, self.limits)?;
        let retained_sources = retained_domain_source_census(&self.domain_conditions, self.limits)?;
        let retained_leak_events = retained_leak_event_census(&self.leak_events, self.limits)?;
        let descent_witness_components =
            self.descent_witnesses
                .iter()
                .try_fold(0usize, |total, witness| {
                    checked_add(
                        "WhenBad descent witness components",
                        total,
                        witness.index_excess_deltas.len(),
                    )
                })?;
        if binding != self.binding
            || self.stats.rhs_terms != rhs_count
            || self.stats.boundary_values_examined != boundary_values_examined
            || self.stats.domain_conditions != self.domain_conditions.len()
            || self.stats.domain_condition_sources != retained_sources.sources
            || self.stats.guard_origins != retained_sources.origins
            || retained_sources.origin_bytes > self.stats.guard_origin_retained_bytes
            || self.stats.retained_condition_terms != retained.terms
            || self.stats.retained_condition_bytes != retained.bytes
            || self.stats.base_domain_guards != self.base_domain_guards.len()
            || self.stats.index_domain_guards != self.index_domain_guards.len()
            || self.stats.leak_events != self.leak_events.len()
            || self.stats.leak_event_shift_components != retained_leak_events.shift_components
            || retained_leak_events.retained_bytes > self.stats.leak_event_retained_bytes
            || self
                .leak_events
                .iter()
                .any(|event| event.rhs_shift.arity() != self.binding.sector().arity())
            || self.stats.descent_witnesses != self.descent_witnesses.len()
            || self.stats.descent_witness_components != descent_witness_components
            || self.descent_witnesses.iter().any(|witness| {
                witness.policy != self.binding.ordering_authority.policy()
                    || witness.rhs_shift.arity() != self.binding.sector().arity()
                    || witness.index_excess_deltas.len() != self.binding.sector().arity()
            })
            || self.stats.leaf_classifications != self.classifications.len()
            || self.observed_retained_core_bytes()? > self.stats.retained_core_bytes
            || self.partition.cases().len() != self.classifications.len()
            || !self
                .partition
                .cases()
                .iter()
                .map(|case| case.id())
                .eq(self.classifications.iter().map(|entry| entry.case))
        {
            return Err(WhenBadCompilerError::ReplayMismatch);
        }
        self.partition.replay_with_limits(
            context,
            self.binding.sector(),
            self.limits.sector_cases,
        )?;
        Ok(())
    }
}

impl WhenBadCertifiedCore {
    /// Capacity-aware conservative census of the complete core-owned payload.
    ///
    /// Inline children are already charged by their containing `Vec`/core
    /// allocation, so their helpers contribute only nested heap payloads.
    /// Partition polynomial `Arc` payloads are charged once per retained split;
    /// case predicates share those allocations. Guard-origin bounds remain
    /// deliberately conservative and may include shared row-label bytes.
    fn observed_retained_core_bytes(&self) -> Result<usize, WhenBadCompilerError> {
        let mut bytes = size_of::<Self>();
        bytes = add_retained_core_bytes(bytes, candidate_binding_retained_bytes(&self.binding)?)?;

        bytes =
            add_retained_core_bytes(bytes, retained_vec_buffer_bytes(&self.domain_conditions)?)?;
        for condition in &self.domain_conditions {
            bytes = add_retained_core_bytes(
                bytes,
                polynomial_exclusive_heap_bytes(&condition.polynomial)?,
            )?;
            bytes = add_retained_core_bytes(bytes, retained_vec_buffer_bytes(&condition.sources)?)?;
            for source in &condition.sources {
                bytes =
                    add_retained_core_bytes(bytes, domain_condition_source_heap_bytes(source)?)?;
            }
        }

        for ordinals in [&self.base_domain_guards, &self.index_domain_guards] {
            bytes = add_retained_core_bytes(bytes, retained_vec_buffer_bytes(ordinals)?)?;
        }

        bytes = add_retained_core_bytes(bytes, retained_vec_buffer_bytes(&self.leak_events)?)?;
        for event in &self.leak_events {
            bytes = add_retained_core_bytes(
                bytes,
                event.rhs_shift.owned_retained_byte_bound().ok_or(
                    WhenBadCompilerError::ResourceCountOverflow {
                        resource: "WhenBad retained core bytes",
                    },
                )?,
            )?;
            bytes = add_retained_core_bytes(
                bytes,
                polynomial_exclusive_heap_bytes(&event.boundary_polynomial)?,
            )?;
            bytes = add_retained_core_bytes(
                bytes,
                polynomial_exclusive_heap_bytes(match &event.numerator_gate {
                    WhenBadLeakNumeratorGate::CoefficientFieldNonzero(polynomial)
                    | WhenBadLeakNumeratorGate::Symbolic(polynomial) => polynomial,
                })?,
            )?;
        }

        bytes =
            add_retained_core_bytes(bytes, retained_vec_buffer_bytes(&self.descent_witnesses)?)?;
        for witness in &self.descent_witnesses {
            let retained = witness.owned_retained_byte_bound().ok_or(
                WhenBadCompilerError::ResourceCountOverflow {
                    resource: "WhenBad retained core bytes",
                },
            )?;
            bytes = add_retained_core_bytes(
                bytes,
                retained
                    .checked_sub(size_of::<WhenBadUniformDescentWitness>())
                    .ok_or(WhenBadCompilerError::ResourceCountOverflow {
                        resource: "WhenBad retained core bytes",
                    })?,
            )?;
        }

        bytes = add_retained_core_bytes(bytes, partition_heap_bytes(&self.partition)?)?;
        bytes = add_retained_core_bytes(bytes, retained_vec_buffer_bytes(&self.classifications)?)?;
        Ok(bytes)
    }
}

fn add_retained_core_bytes(
    retained: usize,
    additional: usize,
) -> Result<usize, WhenBadCompilerError> {
    checked_add("WhenBad retained core bytes", retained, additional)
}

fn retained_vec_buffer_bytes<T>(values: &Vec<T>) -> Result<usize, WhenBadCompilerError> {
    checked_mul(
        "WhenBad retained core bytes",
        values.capacity(),
        size_of::<T>(),
    )
}

fn polynomial_exclusive_heap_bytes(
    polynomial: &ParametricPolynomial,
) -> Result<usize, WhenBadCompilerError> {
    polynomial
        .owned_retained_byte_bound()
        .and_then(|bytes| bytes.checked_sub(size_of::<ParametricPolynomial>()))
        .ok_or(WhenBadCompilerError::ResourceCountOverflow {
            resource: "WhenBad retained core bytes",
        })
}

fn guard_origin_capacity_retained_bytes(
    origin: &GuardOrigin,
) -> Result<usize, WhenBadCompilerError> {
    let mut retained =
        origin
            .retained_byte_bound()
            .ok_or(WhenBadCompilerError::ResourceCountOverflow {
                resource: "WhenBad guard-origin retained bytes",
            })?;
    if let GuardOrigin::RelationAffineFreeRecentering {
        coefficient_offset,
        key_center,
        ..
    } = origin
    {
        // The generic origin bound charges these retained vectors by logical
        // length. Add spare capacity so it cannot evade the core census.
        for values in [coefficient_offset, key_center] {
            let spare = values.capacity().checked_sub(values.len()).ok_or(
                WhenBadCompilerError::ResourceCountOverflow {
                    resource: "WhenBad guard-origin retained bytes",
                },
            )?;
            retained = checked_add(
                "WhenBad guard-origin retained bytes",
                retained,
                checked_mul(
                    "WhenBad guard-origin retained bytes",
                    spare,
                    size_of::<i64>(),
                )?,
            )?;
        }
    }
    Ok(retained)
}

fn guard_origin_exclusive_heap_bytes(origin: &GuardOrigin) -> Result<usize, WhenBadCompilerError> {
    // `retained_byte_bound` is intentionally conservative (including a tree
    // node allowance and shared row-label payloads). Subtract the one inline
    // enum value already charged by the owning origins-vector buffer.
    let retained = guard_origin_capacity_retained_bytes(origin)?;
    retained.checked_sub(size_of::<GuardOrigin>()).ok_or(
        WhenBadCompilerError::ResourceCountOverflow {
            resource: "WhenBad retained core bytes",
        },
    )
}

fn domain_condition_source_heap_bytes(
    source: &WhenBadDomainConditionSource,
) -> Result<usize, WhenBadCompilerError> {
    match source {
        WhenBadDomainConditionSource::PersistedGuard { origins, .. }
        | WhenBadDomainConditionSource::GeneratedCylindricalBaseAssumption { origins, .. } => {
            let mut bytes = retained_vec_buffer_bytes(origins)?;
            for origin in origins {
                bytes = add_retained_core_bytes(bytes, guard_origin_exclusive_heap_bytes(origin)?)?;
            }
            Ok(bytes)
        }
        WhenBadDomainConditionSource::CoefficientDenominator { shift } => shift
            .owned_retained_byte_bound()
            .ok_or(WhenBadCompilerError::ResourceCountOverflow {
                resource: "WhenBad retained core bytes",
            }),
    }
}

fn unsupported_reason_heap_bytes(
    reason: &WhenBadUnsupportedReason,
) -> Result<usize, WhenBadCompilerError> {
    let shift = match reason {
        WhenBadUnsupportedReason::NonUniformSameSectorDescent { rhs_shift, .. }
        | WhenBadUnsupportedReason::ZeroSameSectorComplexityDelta { rhs_shift, .. }
        | WhenBadUnsupportedReason::UnboundedIndexAddition { rhs_shift, .. } => rhs_shift,
    };
    shift
        .owned_retained_byte_bound()
        .ok_or(WhenBadCompilerError::ResourceCountOverflow {
            resource: "WhenBad retained core bytes",
        })
}

fn arc_allocation_overhead_bytes() -> Result<usize, WhenBadCompilerError> {
    checked_mul("WhenBad retained core bytes", 2, size_of::<usize>())
}

fn retained_arc_str_allocation_bytes(value: &Arc<str>) -> Result<usize, WhenBadCompilerError> {
    add_retained_core_bytes(value.len(), arc_allocation_overhead_bytes()?)
}

fn partition_heap_bytes(
    partition: &SymbolicSectorCasePartitionCertificate,
) -> Result<usize, WhenBadCompilerError> {
    let mut bytes = retained_arc_str_allocation_bytes(partition.source_identity())?;
    // `context_fingerprint` has no Arc-returning accessor, but this partition
    // owns one `Arc<str>` allocation with the exact exposed string length.
    bytes = add_retained_core_bytes(
        bytes,
        add_retained_core_bytes(
            partition.context_fingerprint().len(),
            arc_allocation_overhead_bytes()?,
        )?,
    )?;
    bytes = add_retained_core_bytes(
        bytes,
        partition
            .orthant()
            .sector()
            .owned_retained_byte_bound()
            .ok_or(WhenBadCompilerError::ResourceCountOverflow {
                resource: "WhenBad retained core bytes",
            })?,
    )?;
    bytes = add_retained_core_bytes(
        bytes,
        checked_mul(
            "WhenBad retained core bytes",
            partition.orthant().constraints().len(),
            size_of::<crate::SectorOrthantConstraint>(),
        )?,
    )?;
    bytes = add_retained_core_bytes(
        bytes,
        checked_mul(
            "WhenBad retained core bytes",
            partition.splits().len(),
            size_of::<crate::SymbolicSectorCaseSplit>(),
        )?,
    )?;
    for split in partition.splits() {
        let polynomial_arc_bytes = add_retained_core_bytes(
            split.bad_polynomial().owned_retained_byte_bound().ok_or(
                WhenBadCompilerError::ResourceCountOverflow {
                    resource: "WhenBad retained core bytes",
                },
            )?,
            arc_allocation_overhead_bytes()?,
        )?;
        bytes = add_retained_core_bytes(bytes, polynomial_arc_bytes)?;
    }
    bytes = add_retained_core_bytes(
        bytes,
        checked_mul(
            "WhenBad retained core bytes",
            partition.cases().len(),
            size_of::<crate::SymbolicSectorCase>(),
        )?,
    )?;
    for case in partition.cases() {
        bytes = add_retained_core_bytes(
            bytes,
            checked_mul(
                "WhenBad retained core bytes",
                case.predicates().len(),
                size_of::<crate::SymbolicPolynomialPredicate>(),
            )?,
        )?;
    }
    Ok(bytes)
}

fn retained_payload_census(
    domain_conditions: &[WhenBadDomainCondition],
    leak_events: &[WhenBadLeakEvent],
    limits: WhenBadCompilerLimits,
) -> Result<RetainedPolynomialCensus, WhenBadCompilerError> {
    let mut census = RetainedPolynomialCensus::default();
    for condition in domain_conditions {
        charge_retained_polynomial(&condition.polynomial, limits, &mut census)?;
    }
    for event in leak_events {
        charge_retained_polynomial(&event.boundary_polynomial, limits, &mut census)?;
        charge_retained_polynomial(
            match &event.numerator_gate {
                WhenBadLeakNumeratorGate::CoefficientFieldNonzero(polynomial)
                | WhenBadLeakNumeratorGate::Symbolic(polynomial) => polynomial,
            },
            limits,
            &mut census,
        )?;
    }
    Ok(census)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct RetainedDomainSourceCensus {
    sources: usize,
    origins: usize,
    origin_bytes: usize,
}

fn retained_domain_source_census(
    domain_conditions: &[WhenBadDomainCondition],
    limits: WhenBadCompilerLimits,
) -> Result<RetainedDomainSourceCensus, WhenBadCompilerError> {
    let mut census = RetainedDomainSourceCensus::default();
    for condition in domain_conditions {
        for source in &condition.sources {
            census.sources = checked_add("WhenBad domain condition sources", census.sources, 1)?;
            check_limit(
                "WhenBad domain condition sources",
                census.sources,
                limits.max_domain_condition_sources,
            )?;
            let origins = match source {
                WhenBadDomainConditionSource::PersistedGuard { origins, .. }
                | WhenBadDomainConditionSource::GeneratedCylindricalBaseAssumption {
                    origins,
                    ..
                } => origins.as_slice(),
                WhenBadDomainConditionSource::CoefficientDenominator { .. } => &[],
            };
            census.origins = checked_add("WhenBad guard origins", census.origins, origins.len())?;
            check_limit(
                "WhenBad guard origins",
                census.origins,
                limits.max_guard_origins,
            )?;
            for origin in origins {
                census.origin_bytes = checked_add(
                    "WhenBad guard-origin retained bytes",
                    census.origin_bytes,
                    guard_origin_capacity_retained_bytes(origin)?,
                )?;
                check_limit(
                    "WhenBad guard-origin retained bytes",
                    census.origin_bytes,
                    limits.max_guard_origin_retained_bytes,
                )?;
            }
        }
    }
    Ok(census)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct RetainedLeakEventCensus {
    shift_components: usize,
    retained_bytes: usize,
}

fn retained_leak_event_census(
    leak_events: &Vec<WhenBadLeakEvent>,
    limits: WhenBadCompilerLimits,
) -> Result<RetainedLeakEventCensus, WhenBadCompilerError> {
    check_limit(
        "WhenBad leak events",
        leak_events.len(),
        limits.max_leak_events,
    )?;
    let event_buffer_bytes = checked_mul(
        "WhenBad leak-event retained bytes",
        leak_events.capacity(),
        size_of::<WhenBadLeakEvent>(),
    )?;
    check_limit(
        "WhenBad leak-event retained bytes",
        event_buffer_bytes,
        limits.max_leak_event_retained_bytes,
    )?;
    let mut census = RetainedLeakEventCensus {
        shift_components: 0,
        retained_bytes: event_buffer_bytes,
    };
    for event in leak_events {
        census.shift_components = checked_add(
            "WhenBad leak-event shift components",
            census.shift_components,
            event.rhs_shift.arity(),
        )?;
        check_limit(
            "WhenBad leak-event shift components",
            census.shift_components,
            limits.max_leak_event_shift_components,
        )?;
        let retained_bytes = event.rhs_shift.owned_retained_byte_bound().ok_or(
            WhenBadCompilerError::ResourceCountOverflow {
                resource: "WhenBad leak-event retained bytes",
            },
        )?;
        census.retained_bytes = checked_add(
            "WhenBad leak-event retained bytes",
            census.retained_bytes,
            retained_bytes,
        )?;
        check_limit(
            "WhenBad leak-event retained bytes",
            census.retained_bytes,
            limits.max_leak_event_retained_bytes,
        )?;
    }
    Ok(census)
}

fn candidate_binding(
    candidate: WhenBadGlobalCandidateView<'_>,
    byte_limit: usize,
) -> Result<WhenBadCandidateBinding, WhenBadCompilerError> {
    enum BranchPreflight<'a> {
        Anchored {
            candidate: &'a ParametricReductionRuleCandidate,
            ordering_bytes: usize,
            trace_manifest_bytes: usize,
        },
        Cylindrical {
            candidate: &'a GeneratedCylindricalGlobalCandidateAuthority,
            ordering: &'a str,
            local_identity: &'a str,
        },
    }

    // Count the complete retained payload without copying any candidate-owned
    // string, vector, manifest, or provenance geometry.
    let sector_bytes = checked_mul(
        "WhenBad candidate binding bytes",
        candidate.sector().arity(),
        size_of::<bool>(),
    )?;
    let original_pivot_bytes = checked_mul(
        "WhenBad candidate binding bytes",
        candidate.original_pivot().arity(),
        size_of::<i64>(),
    )?;
    let centered_relation_manifest_bytes = candidate
        .centered_relation()
        .stable_manifest_byte_len_with_limit(byte_limit)?;
    let mut retained_bytes = 0usize;
    for component_bytes in [
        candidate.family_fingerprint().len(),
        candidate.context_fingerprint().len(),
        sector_bytes,
        original_pivot_bytes,
        centered_relation_manifest_bytes,
    ] {
        retained_bytes = add_candidate_binding_bytes(retained_bytes, component_bytes, byte_limit)?;
    }
    let branch = match candidate {
        WhenBadGlobalCandidateView::AnchoredV1(candidate) => {
            let ordering_bytes = anchored_ordering_manifest_byte_len(candidate, byte_limit)?;
            retained_bytes =
                add_candidate_binding_bytes(retained_bytes, ordering_bytes, byte_limit)?;
            let discovery_anchor_bytes = checked_mul(
                "WhenBad candidate binding bytes",
                candidate.discovery_anchor().len(),
                size_of::<i64>(),
            )?;
            retained_bytes =
                add_candidate_binding_bytes(retained_bytes, discovery_anchor_bytes, byte_limit)?;
            retained_bytes = add_candidate_binding_bytes(
                retained_bytes,
                candidate.source_manifest().len(),
                byte_limit,
            )?;
            let trace_manifest_bytes = trace_manifest_byte_len(candidate, byte_limit)?;
            retained_bytes =
                add_candidate_binding_bytes(retained_bytes, trace_manifest_bytes, byte_limit)?;
            BranchPreflight::Anchored {
                candidate,
                ordering_bytes,
                trace_manifest_bytes,
            }
        }
        WhenBadGlobalCandidateView::CylindricalGlobalV1(candidate) => {
            let ordering = candidate.ordering_authority().identity();
            let local_identity =
                candidate.local_candidate_binding_identity_for_source_composition();
            for component_bytes in [ordering.len(), local_identity.len()] {
                retained_bytes =
                    add_candidate_binding_bytes(retained_bytes, component_bytes, byte_limit)?;
            }
            BranchPreflight::Cylindrical {
                candidate,
                ordering,
                local_identity,
            }
        }
    };
    // Only now, after the complete aggregate has been admitted, request the
    // fallible retained allocations. Every string writer uses the same
    // streaming encoder as its allocation-free count pass.
    let centered_relation_manifest = candidate
        .centered_relation()
        .stable_manifest_with_limit(centered_relation_manifest_bytes)?;
    let family_fingerprint = try_copy_string(
        candidate.family_fingerprint(),
        "WhenBad candidate family fingerprint",
    )?;
    let context_fingerprint = try_copy_string(
        candidate.context_fingerprint(),
        "WhenBad candidate context fingerprint",
    )?;
    let sector = SectorMask::try_new(candidate.sector().active_bits().iter().copied())?;
    let original_pivot = IndexShift::try_new(
        candidate.original_pivot().values().iter().copied(),
        candidate.original_pivot().arity(),
    )?;
    let (source_authentication, ordering_authority, source_authority) = match branch {
        BranchPreflight::Anchored {
            candidate,
            ordering_bytes,
            trace_manifest_bytes,
        } => (
            WhenBadSourceAuthentication::AlgebraicOnly,
            WhenBadOrderingAuthority::AnchoredV1 {
                policy: candidate.ordering().policy(),
                manifest: anchored_ordering_manifest(candidate, ordering_bytes)?,
                discovery_anchor: try_copy_vec(
                    candidate.discovery_anchor(),
                    "WhenBad candidate discovery anchor",
                )?,
            },
            WhenBadCandidateSourceAuthority::AnchoredEliminationV1 {
                source_manifest: try_copy_string(
                    candidate.source_manifest(),
                    "WhenBad candidate source manifest",
                )?,
                source_row_count: candidate.source_row_count(),
                trace_manifest: trace_manifest(candidate, trace_manifest_bytes)?,
                rule_limits: candidate.limits(),
            },
        ),
        BranchPreflight::Cylindrical {
            candidate,
            ordering,
            local_identity,
        } => (
            WhenBadSourceAuthentication::GeneratedCylindricalPersistentEliminationV2,
            WhenBadOrderingAuthority::CylindricalV1 {
                policy: candidate.ordering_policy(),
                manifest: try_copy_string(ordering, "WhenBad cylindrical ordering identity")?,
            },
            WhenBadCandidateSourceAuthority::GeneratedCylindricalPersistentV2 {
                local_candidate_identity: try_copy_string(
                    local_identity,
                    "WhenBad cylindrical local candidate identity",
                )?,
                source_row_count: candidate.source().stats().retained_source_rows(),
            },
        ),
    };
    let mut binding = WhenBadCandidateBinding {
        source_authentication,
        family_fingerprint,
        context_fingerprint,
        sector,
        ordering_authority,
        source_authority,
        pivot_ordinal: candidate.pivot_ordinal(),
        original_pivot,
        centered_relation_manifest,
        retained_bytes: 0,
    };
    let observed_retained_bytes = candidate_binding_retained_bytes(&binding)?;
    check_limit(
        "WhenBad candidate binding bytes",
        observed_retained_bytes,
        byte_limit,
    )?;
    // The allocation-free pass admits the logical minimum. Allocators may
    // retain a larger capacity even after `try_reserve_exact`, so census and
    // bind the actual post-allocation capacity before publishing the proof.
    if observed_retained_bytes < retained_bytes {
        return Err(WhenBadCompilerError::ReplayMismatch);
    }
    binding.retained_bytes = observed_retained_bytes;
    Ok(binding)
}

fn add_candidate_binding_bytes(
    retained: usize,
    additional: usize,
    limit: usize,
) -> Result<usize, WhenBadCompilerError> {
    let requested = checked_add("WhenBad candidate binding bytes", retained, additional)?;
    check_limit("WhenBad candidate binding bytes", requested, limit)?;
    Ok(requested)
}

fn candidate_binding_retained_bytes(
    binding: &WhenBadCandidateBinding,
) -> Result<usize, WhenBadCompilerError> {
    let mut bytes = 0usize;
    for component in [
        binding.family_fingerprint.capacity(),
        binding.context_fingerprint.capacity(),
        binding.sector.owned_retained_byte_bound().ok_or(
            WhenBadCompilerError::ResourceCountOverflow {
                resource: "WhenBad candidate binding bytes",
            },
        )?,
        binding.original_pivot.owned_retained_byte_bound().ok_or(
            WhenBadCompilerError::ResourceCountOverflow {
                resource: "WhenBad candidate binding bytes",
            },
        )?,
        binding.centered_relation_manifest.capacity(),
        match &binding.ordering_authority {
            WhenBadOrderingAuthority::AnchoredV1 { manifest, .. }
            | WhenBadOrderingAuthority::CylindricalV1 { manifest, .. } => manifest.capacity(),
        },
    ] {
        bytes = checked_add("WhenBad candidate binding bytes", bytes, component)?;
    }
    if let WhenBadOrderingAuthority::AnchoredV1 {
        discovery_anchor, ..
    } = &binding.ordering_authority
    {
        bytes = checked_add(
            "WhenBad candidate binding bytes",
            bytes,
            checked_mul(
                "WhenBad candidate binding bytes",
                discovery_anchor.capacity(),
                size_of::<i64>(),
            )?,
        )?;
    }
    match &binding.source_authority {
        WhenBadCandidateSourceAuthority::AnchoredEliminationV1 {
            source_manifest,
            trace_manifest,
            ..
        } => {
            bytes = checked_add(
                "WhenBad candidate binding bytes",
                bytes,
                source_manifest.capacity(),
            )?;
            bytes = checked_add(
                "WhenBad candidate binding bytes",
                bytes,
                trace_manifest.capacity(),
            )?;
        }
        WhenBadCandidateSourceAuthority::GeneratedCylindricalPersistentV2 {
            local_candidate_identity,
            ..
        } => {
            bytes = checked_add(
                "WhenBad candidate binding bytes",
                bytes,
                local_candidate_identity.capacity(),
            )?;
        }
    }
    Ok(bytes)
}

fn replay_candidate_binding_capacity(
    binding: &WhenBadCandidateBinding,
    limits: WhenBadCompilerLimits,
) -> Result<(), WhenBadCompilerError> {
    let observed = candidate_binding_retained_bytes(binding)?;
    if observed > binding.retained_bytes
        || binding.retained_bytes > limits.max_candidate_binding_bytes
    {
        return Err(WhenBadCompilerError::ReplayMismatch);
    }
    Ok(())
}

fn anchored_ordering_manifest_byte_len(
    candidate: &ParametricReductionRuleCandidate,
    limit: usize,
) -> Result<usize, WhenBadCompilerError> {
    count_formatted_bytes("WhenBad candidate ordering manifest", limit, |writer| {
        write_anchored_ordering_manifest(writer, candidate)
    })
}

fn anchored_ordering_manifest(
    candidate: &ParametricReductionRuleCandidate,
    exact_bytes: usize,
) -> Result<String, WhenBadCompilerError> {
    build_precounted_string(
        "WhenBad candidate ordering manifest",
        exact_bytes,
        |writer| write_anchored_ordering_manifest(writer, candidate),
    )
}

fn write_anchored_ordering_manifest(
    writer: &mut impl fmt::Write,
    candidate: &ParametricReductionRuleCandidate,
) -> fmt::Result {
    writer.write_str(candidate.ordering().policy().stable_id())?;
    writer.write_str("|anchor=[")?;
    for (ordinal, value) in candidate.ordering().anchor().iter().enumerate() {
        if ordinal != 0 {
            writer.write_char(',')?;
        }
        write!(writer, "{value}")?;
    }
    writer.write_char(']')
}

fn trace_manifest_byte_len(
    candidate: &ParametricReductionRuleCandidate,
    limit: usize,
) -> Result<usize, WhenBadCompilerError> {
    count_formatted_bytes("WhenBad candidate trace manifest", limit, |writer| {
        write_trace_manifest(writer, candidate)
    })
}

fn trace_manifest(
    candidate: &ParametricReductionRuleCandidate,
    exact_bytes: usize,
) -> Result<String, WhenBadCompilerError> {
    build_precounted_string("WhenBad candidate trace manifest", exact_bytes, |writer| {
        write_trace_manifest(writer, candidate)
    })
}

fn write_trace_manifest(
    writer: &mut impl fmt::Write,
    candidate: &ParametricReductionRuleCandidate,
) -> fmt::Result {
    let trace = candidate.trace();
    write!(
        writer,
        "base={}|reductions={}|divisor={}",
        trace.base_source_row_index(),
        trace.reductions().len(),
        trace.divisor().raw(),
    )?;
    for reduction in trace.reductions() {
        write!(
            writer,
            "|{}:{}",
            reduction.prior_pivot_ordinal(),
            reduction.factor().raw(),
        )?;
    }
    Ok(())
}

fn count_formatted_bytes(
    resource: &'static str,
    limit: usize,
    write_payload: impl FnOnce(&mut BoundedByteCounter) -> fmt::Result,
) -> Result<usize, WhenBadCompilerError> {
    let mut counter = BoundedByteCounter { bytes: 0, limit };
    if write_payload(&mut counter).is_err() {
        return Err(WhenBadCompilerError::ResourceLimit {
            resource,
            requested: counter.bytes.max(limit.saturating_add(1)),
            limit,
        });
    }
    Ok(counter.bytes)
}

fn build_precounted_string(
    resource: &'static str,
    exact_bytes: usize,
    write_payload: impl FnOnce(&mut PreallocatedStringWriter) -> fmt::Result,
) -> Result<String, WhenBadCompilerError> {
    let mut writer = PreallocatedStringWriter::try_new(resource, exact_bytes)?;
    if write_payload(&mut writer).is_err() || writer.output.len() != exact_bytes {
        return Err(WhenBadCompilerError::ReplayMismatch);
    }
    Ok(writer.output)
}

struct PreallocatedStringWriter {
    output: String,
    exact_bytes: usize,
}

impl PreallocatedStringWriter {
    fn try_new(resource: &'static str, exact_bytes: usize) -> Result<Self, WhenBadCompilerError> {
        let mut output = String::new();
        output.try_reserve_exact(exact_bytes).map_err(|_| {
            WhenBadCompilerError::Relation(crate::ParametricRelationError::AllocationFailure {
                resource,
                requested: exact_bytes,
            })
        })?;
        Ok(Self {
            output,
            exact_bytes,
        })
    }
}

impl fmt::Write for PreallocatedStringWriter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let requested = self
            .output
            .len()
            .checked_add(value.len())
            .ok_or(fmt::Error)?;
        if requested > self.exact_bytes {
            return Err(fmt::Error);
        }
        self.output.push_str(value);
        Ok(())
    }
}

fn try_copy_string(source: &str, resource: &'static str) -> Result<String, WhenBadCompilerError> {
    let mut retained = String::new();
    retained.try_reserve_exact(source.len()).map_err(|_| {
        WhenBadCompilerError::Relation(crate::ParametricRelationError::AllocationFailure {
            resource,
            requested: source.len(),
        })
    })?;
    retained.push_str(source);
    Ok(retained)
}

fn insert_borrowed_domain_condition(
    context: &ParametricCoefficientContext,
    conditions: &mut Vec<WhenBadDomainCondition>,
    polynomial: &ParametricPolynomial,
    source: BorrowedWhenBadDomainConditionSource<'_>,
    limits: WhenBadCompilerLimits,
    census: &mut RetainedPolynomialCensus,
) -> Result<(), WhenBadCompilerError> {
    context.validate_polynomial_with_limits(polynomial, limits.arithmetic.exact_algebra)?;
    if polynomial.is_zero() {
        return Err(WhenBadCompilerError::UnsatisfiableCandidateDomain);
    }
    let source_origins = source.origin_count();
    let source_origin_bytes = source.origin_retained_bytes()?;
    if let Some(existing_ordinal) = conditions
        .iter()
        .position(|condition| condition.polynomial == *polynomial)
    {
        let existing = &conditions[existing_ordinal];
        if existing
            .sources
            .iter()
            .any(|retained| source.matches_owned(retained))
        {
            return Ok(());
        }
        let requested_condition_sources = existing.sources.len().checked_add(1).ok_or(
            WhenBadCompilerError::ResourceCountOverflow {
                resource: "WhenBad domain condition sources",
            },
        )?;
        check_limit(
            "WhenBad domain condition sources",
            requested_condition_sources,
            limits.max_domain_condition_sources,
        )?;
        let requested_sources = checked_add("WhenBad domain condition sources", census.sources, 1)?;
        check_limit(
            "WhenBad domain condition sources",
            requested_sources,
            limits.max_domain_condition_sources,
        )?;
        let requested_origins =
            checked_add("WhenBad guard origins", census.origins, source_origins)?;
        check_limit(
            "WhenBad guard origins",
            requested_origins,
            limits.max_guard_origins,
        )?;
        let requested_origin_bytes = checked_add(
            "WhenBad guard-origin retained bytes",
            census.origin_bytes,
            source_origin_bytes,
        )?;
        check_limit(
            "WhenBad guard-origin retained bytes",
            requested_origin_bytes,
            limits.max_guard_origin_retained_bytes,
        )?;

        // Every count and deep-payload byte is admitted before the first
        // proportional allocation. Grow the retained vector in place so the
        // already authenticated source payloads are moved, never deep-cloned.
        try_reserve_core(
            "WhenBad merged domain condition sources",
            &mut conditions[existing_ordinal].sources,
            1,
        )?;
        let source = source.try_to_owned()?;
        conditions[existing_ordinal].sources.push(source);
        conditions[existing_ordinal].sources.sort();
        census.sources = requested_sources;
        census.origins = requested_origins;
        census.origin_bytes = requested_origin_bytes;
        return Ok(());
    }
    let requested =
        conditions
            .len()
            .checked_add(1)
            .ok_or(WhenBadCompilerError::ResourceCountOverflow {
                resource: "WhenBad domain conditions",
            })?;
    check_limit(
        "WhenBad domain conditions",
        requested,
        limits.max_domain_conditions,
    )?;
    let index_dependent = context
        .polynomial_depends_on_indices_with_limits(polynomial, limits.arithmetic.exact_algebra)?;
    let requested_sources = checked_add("WhenBad domain condition sources", census.sources, 1)?;
    check_limit(
        "WhenBad domain condition sources",
        requested_sources,
        limits.max_domain_condition_sources,
    )?;
    let requested_origins = checked_add("WhenBad guard origins", census.origins, source_origins)?;
    check_limit(
        "WhenBad guard origins",
        requested_origins,
        limits.max_guard_origins,
    )?;
    let requested_origin_bytes = checked_add(
        "WhenBad guard-origin retained bytes",
        census.origin_bytes,
        source_origin_bytes,
    )?;
    check_limit(
        "WhenBad guard-origin retained bytes",
        requested_origin_bytes,
        limits.max_guard_origin_retained_bytes,
    )?;
    let (requested_terms, requested_bytes) =
        retained_polynomial_totals(polynomial, limits, *census)?;
    try_reserve_core("WhenBad domain conditions", conditions, 1)?;
    let polynomial = try_copy_domain_polynomial(polynomial)?;
    let source = source.try_to_owned()?;
    let mut sources = Vec::new();
    try_reserve_core("WhenBad domain condition sources", &mut sources, 1)?;
    sources.push(source);
    conditions.push(WhenBadDomainCondition {
        polynomial,
        sources,
        index_dependent,
    });
    census.sources = requested_sources;
    census.origins = requested_origins;
    census.origin_bytes = requested_origin_bytes;
    census.terms = requested_terms;
    census.bytes = requested_bytes;
    Ok(())
}

fn try_copy_domain_polynomial(
    polynomial: &ParametricPolynomial,
) -> Result<ParametricPolynomial, WhenBadCompilerError> {
    polynomial
        .try_copy_authenticated_sparse_payload()
        .map_err(|resource| {
            WhenBadCompilerError::Relation(crate::ParametricRelationError::AllocationFailure {
                resource,
                requested: polynomial.term_count(),
            })
        })
}

fn try_copy_guard_origins(
    origins: &BTreeSet<GuardOrigin>,
) -> Result<Vec<GuardOrigin>, WhenBadCompilerError> {
    let mut retained = Vec::new();
    try_reserve_core(
        "WhenBad domain condition guard origins",
        &mut retained,
        origins.len(),
    )?;
    for origin in origins {
        retained.push(try_copy_guard_origin(origin)?);
    }
    Ok(retained)
}

fn try_copy_guard_origin(origin: &GuardOrigin) -> Result<GuardOrigin, WhenBadCompilerError> {
    use GuardOrigin::*;

    Ok(match origin {
        FamilyInputCoefficientDenominator { location } => FamilyInputCoefficientDenominator {
            location: location.clone(),
        },
        FamilyBasisDeterminantNumerator => FamilyBasisDeterminantNumerator,
        PowerShiftSupport { denominator } => PowerShiftSupport {
            denominator: *denominator,
        },
        GuardedDivisionDividendDenominator => GuardedDivisionDividendDenominator,
        GuardedDivisionDivisorDenominator => GuardedDivisionDivisorDenominator,
        GuardedDivisionDivisorNumerator => GuardedDivisionDivisorNumerator,
        ExplicitRelationCondition => ExplicitRelationCondition,
        GeneratedAffineSealedCondition => GeneratedAffineSealedCondition,
        RelationConditionAttached { row } => RelationConditionAttached { row: row.clone() },
        RelationInputTermDenominator { row, shift } => RelationInputTermDenominator {
            row: row.clone(),
            shift: try_copy_boxed_slice(shift, "WhenBad guard-origin relation-input shift")?,
        },
        RelationCollectedTermDenominator { row, shift } => RelationCollectedTermDenominator {
            row: row.clone(),
            shift: try_copy_boxed_slice(shift, "WhenBad guard-origin relation-collected shift")?,
        },
        RelationScaleFactorDenominator {
            target_row,
            source_row,
        } => RelationScaleFactorDenominator {
            target_row: target_row.clone(),
            source_row: source_row.clone(),
        },
        RelationTranslation {
            source_row,
            target_row,
            offset,
        } => RelationTranslation {
            source_row: source_row.clone(),
            target_row: target_row.clone(),
            offset: try_copy_boxed_slice(offset, "WhenBad guard-origin translation offset")?,
        },
        RelationAffineFreeRecentering {
            source_row,
            target_row,
            coefficient_offset,
            key_center,
        } => RelationAffineFreeRecentering {
            source_row: source_row.clone(),
            target_row: target_row.clone(),
            coefficient_offset: try_copy_vec(
                coefficient_offset,
                "WhenBad guard-origin affine coefficient offset",
            )?,
            key_center: try_copy_vec(key_center, "WhenBad guard-origin affine key center")?,
        },
        RelationIndexPermutation {
            source_row,
            target_row,
            source_to_target,
        } => RelationIndexPermutation {
            source_row: source_row.clone(),
            target_row: target_row.clone(),
            source_to_target: try_copy_boxed_slice(
                source_to_target,
                "WhenBad guard-origin relation permutation",
            )?,
        },
        IndexTranslation { offset } => IndexTranslation {
            offset: try_copy_boxed_slice(offset, "WhenBad guard-origin index translation")?,
        },
        IndexPermutation { source_to_target } => IndexPermutation {
            source_to_target: try_copy_boxed_slice(
                source_to_target,
                "WhenBad guard-origin index permutation",
            )?,
        },
        VerifiedSymmetryMapDomain {
            source_to_target,
            condition_ordinal,
        } => VerifiedSymmetryMapDomain {
            source_to_target: try_copy_boxed_slice(
                source_to_target,
                "WhenBad guard-origin symmetry permutation",
            )?,
            condition_ordinal: *condition_ordinal,
        },
        IndexSpecialization { assignment } => IndexSpecialization {
            assignment: try_copy_boxed_slice(
                assignment,
                "WhenBad guard-origin index specialization",
            )?,
        },
        PartialIndexSpecialization { assignments } => PartialIndexSpecialization {
            assignments: try_copy_boxed_slice(
                assignments,
                "WhenBad guard-origin partial specialization",
            )?,
        },
        ResidualUnitAffineIndexSubstitution {
            source_case,
            predicate_ordinal,
            bound_position,
        } => ResidualUnitAffineIndexSubstitution {
            source_case: *source_case,
            predicate_ordinal: *predicate_ordinal,
            bound_position: *bound_position,
        },
        ResidualAffineBranchNonzeroGuardSubstitution {
            source_case,
            source_work_item_ordinal,
            ready_terminal_ordinal,
            structural_locus_ordinal,
        } => ResidualAffineBranchNonzeroGuardSubstitution {
            source_case: *source_case,
            source_work_item_ordinal: *source_work_item_ordinal,
            ready_terminal_ordinal: *ready_terminal_ordinal,
            structural_locus_ordinal: *structural_locus_ordinal,
        },
        CoefficientSpecializationDenominator => CoefficientSpecializationDenominator,
        CoefficientPartialSpecializationDenominator => CoefficientPartialSpecializationDenominator,
        RelationPartialSpecializationTermDenominator { row, shift } => {
            RelationPartialSpecializationTermDenominator {
                row: row.clone(),
                shift: try_copy_boxed_slice(shift, "WhenBad guard-origin partial relation shift")?,
            }
        }
        CoefficientResidualUnitAffineSubstitutionDenominator {
            source_case,
            predicate_ordinal,
            bound_position,
        } => CoefficientResidualUnitAffineSubstitutionDenominator {
            source_case: *source_case,
            predicate_ordinal: *predicate_ordinal,
            bound_position: *bound_position,
        },
        RelationResidualUnitAffineSubstitutionTermDenominator {
            row,
            shift,
            source_case,
            predicate_ordinal,
            bound_position,
        } => RelationResidualUnitAffineSubstitutionTermDenominator {
            row: row.clone(),
            shift: try_copy_boxed_slice(
                shift,
                "WhenBad guard-origin residual-unit-affine relation shift",
            )?,
            source_case: *source_case,
            predicate_ordinal: *predicate_ordinal,
            bound_position: *bound_position,
        },
        RelationResidualAffineBranchSubstitutionTermDenominator {
            row,
            shift,
            source_case,
            source_work_item_ordinal,
            ready_terminal_ordinal,
        } => RelationResidualAffineBranchSubstitutionTermDenominator {
            row: row.clone(),
            shift: try_copy_boxed_slice(
                shift,
                "WhenBad guard-origin residual-affine relation shift",
            )?,
            source_case: *source_case,
            source_work_item_ordinal: *source_work_item_ordinal,
            ready_terminal_ordinal: *ready_terminal_ordinal,
        },
        RelationResidualUnitAffineSubstitution {
            source_row,
            target_row,
            source_case,
            predicate_ordinal,
            bound_position,
        } => RelationResidualUnitAffineSubstitution {
            source_row: source_row.clone(),
            target_row: target_row.clone(),
            source_case: *source_case,
            predicate_ordinal: *predicate_ordinal,
            bound_position: *bound_position,
        },
        RelationResidualAffineBranchSubstitution {
            source_row,
            target_row,
            source_case,
            source_work_item_ordinal,
            ready_terminal_ordinal,
        } => RelationResidualAffineBranchSubstitution {
            source_row: source_row.clone(),
            target_row: target_row.clone(),
            source_case: *source_case,
            source_work_item_ordinal: *source_work_item_ordinal,
            ready_terminal_ordinal: *ready_terminal_ordinal,
        },
        QuotientPivotNumerator => QuotientPivotNumerator,
        ConcreteQuotientEliminationPivotNumerator { pivot } => {
            ConcreteQuotientEliminationPivotNumerator { pivot: *pivot }
        }
        ExplicitShiftOperatorCondition => ExplicitShiftOperatorCondition,
        ShiftOperatorConditionAttached { row } => {
            ShiftOperatorConditionAttached { row: row.clone() }
        }
        ShiftOperatorInputTermDenominator { row } => {
            ShiftOperatorInputTermDenominator { row: row.clone() }
        }
        ShiftOperatorCollectedTermDenominator { row } => {
            ShiftOperatorCollectedTermDenominator { row: row.clone() }
        }
        ShiftOperatorFromRelationAdapter { row } => {
            ShiftOperatorFromRelationAdapter { row: row.clone() }
        }
        ShiftOperatorToRelationAdapter { row } => {
            ShiftOperatorToRelationAdapter { row: row.clone() }
        }
        GeneratedAffineGroupRecentering {
            solve_group_ordinal,
            database_epoch,
            event_ordinal,
        } => GeneratedAffineGroupRecentering {
            solve_group_ordinal: *solve_group_ordinal,
            database_epoch: *database_epoch,
            event_ordinal: *event_ordinal,
        },
        GeneratedAffineGroupTopReductionCoefficientDenominator {
            solve_group_ordinal,
            database_epoch,
            event_ordinal,
            operation_ordinal,
            term_ordinal,
            pivot_normalization,
        } => GeneratedAffineGroupTopReductionCoefficientDenominator {
            solve_group_ordinal: *solve_group_ordinal,
            database_epoch: *database_epoch,
            event_ordinal: *event_ordinal,
            operation_ordinal: *operation_ordinal,
            term_ordinal: *term_ordinal,
            pivot_normalization: *pivot_normalization,
        },
    })
}

fn try_copy_vec<T: Copy>(
    source: &[T],
    resource: &'static str,
) -> Result<Vec<T>, WhenBadCompilerError> {
    let mut retained = Vec::new();
    try_reserve_core(resource, &mut retained, source.len())?;
    retained.extend_from_slice(source);
    Ok(retained)
}

fn try_copy_boxed_slice<T: Copy>(
    source: &[T],
    resource: &'static str,
) -> Result<Box<[T]>, WhenBadCompilerError> {
    let retained = try_copy_vec(source, resource)?;
    // `try_reserve_exact` on an empty vector retains exactly the requested
    // logical capacity in Rust's `Vec`; with no excess capacity this conversion
    // transfers the allocation instead of requesting a shrink allocation.
    if retained.capacity() != retained.len() {
        return Err(WhenBadCompilerError::ResourceLimit {
            resource,
            requested: retained.capacity(),
            limit: retained.len(),
        });
    }
    Ok(retained.into_boxed_slice())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct RetainedPolynomialCensus {
    sources: usize,
    origins: usize,
    origin_bytes: usize,
    terms: usize,
    bytes: usize,
}

fn charge_retained_polynomial(
    polynomial: &ParametricPolynomial,
    limits: WhenBadCompilerLimits,
    census: &mut RetainedPolynomialCensus,
) -> Result<(), WhenBadCompilerError> {
    let (terms, bytes) = retained_polynomial_totals(polynomial, limits, *census)?;
    census.terms = terms;
    census.bytes = bytes;
    Ok(())
}

fn retained_polynomial_totals(
    polynomial: &ParametricPolynomial,
    limits: WhenBadCompilerLimits,
    census: RetainedPolynomialCensus,
) -> Result<(usize, usize), WhenBadCompilerError> {
    let requested_terms = checked_add(
        "WhenBad retained condition terms",
        census.terms,
        polynomial.term_count(),
    )?;
    check_limit(
        "WhenBad retained condition terms",
        requested_terms,
        limits.max_retained_condition_terms,
    )?;
    let remaining = limits
        .max_retained_condition_bytes
        .checked_sub(census.bytes)
        .ok_or(WhenBadCompilerError::ResourceLimit {
            resource: "WhenBad retained condition bytes",
            requested: census.bytes,
            limit: limits.max_retained_condition_bytes,
        })?;
    let polynomial_bytes = polynomial_display_bytes(polynomial, remaining).map_err(|local| {
        let requested = census
            .bytes
            .checked_add(local.requested)
            .unwrap_or(usize::MAX);
        WhenBadCompilerError::ResourceLimit {
            resource: "WhenBad retained condition bytes",
            requested,
            limit: limits.max_retained_condition_bytes,
        }
    })?;
    let requested_bytes = checked_add(
        "WhenBad retained condition bytes",
        census.bytes,
        polynomial_bytes,
    )?;
    check_limit(
        "WhenBad retained condition bytes",
        requested_bytes,
        limits.max_retained_condition_bytes,
    )?;
    Ok((requested_terms, requested_bytes))
}

fn polynomial_display_bytes(
    polynomial: &ParametricPolynomial,
    limit: usize,
) -> Result<usize, BoundedByteLimit> {
    let mut writer = BoundedByteCounter { bytes: 0, limit };
    if write!(&mut writer, "{}", polynomial.raw()).is_err() {
        return Err(BoundedByteLimit {
            requested: writer.bytes.max(limit.saturating_add(1)),
        });
    }
    Ok(writer.bytes)
}

struct BoundedByteLimit {
    requested: usize,
}

struct BoundedByteCounter {
    bytes: usize,
    limit: usize,
}

impl fmt::Write for BoundedByteCounter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.bytes = self.bytes.checked_add(value.len()).ok_or(fmt::Error)?;
        if self.bytes > self.limit {
            Err(fmt::Error)
        } else {
            Ok(())
        }
    }
}

fn boundary_value_bound(
    sector: &SectorMask,
    rhs: &[(&IndexShift, &crate::ParametricCoefficient)],
    limits: WhenBadCompilerLimits,
) -> Result<usize, WhenBadCompilerError> {
    let mut aggregate = 0usize;
    for (shift, _) in rhs {
        let mut per_rhs = 0usize;
        for (coordinate, (&active, &delta)) in
            sector.active_bits().iter().zip(shift.values()).enumerate()
        {
            let Some(hazard) = finite_boundary_hazard_range(active, delta, coordinate)? else {
                continue;
            };
            per_rhs = checked_add("WhenBad boundary values per RHS", per_rhs, hazard.count())?;
            check_limit(
                "WhenBad boundary values per RHS",
                per_rhs,
                limits.max_boundary_values_per_rhs,
            )?;
        }
        aggregate = checked_add("WhenBad boundary values", aggregate, per_rhs)?;
        check_limit(
            "WhenBad boundary values",
            aggregate,
            limits.max_boundary_values,
        )?;
    }
    Ok(aggregate)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WhenBadCoreError {
    WrongArity {
        expected: usize,
        actual: usize,
    },
    BoundaryArithmeticOverflow {
        coordinate: usize,
    },
    DescentArithmeticOverflow,
    RetainedCapacityEnvelopeExceeded {
        resource: &'static str,
        observed_bytes: usize,
        admitted_bytes: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    ParametricRelation(crate::ParametricRelationError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BoundaryHazardRange {
    kind: WhenBadBoundaryHazardKind,
    first: i64,
    last: i64,
    count: usize,
}

impl BoundaryHazardRange {
    pub(crate) const fn kind(self) -> WhenBadBoundaryHazardKind {
        self.kind
    }

    pub(crate) const fn first(self) -> i64 {
        self.first
    }

    pub(crate) const fn last(self) -> i64 {
        self.last
    }

    pub(crate) const fn count(self) -> usize {
        self.count
    }
}

/// Return the only possible finite bad interval for one orthant coordinate.
///
/// Sector activation occurs for an inactive coordinate shifted upward across
/// zero.  Representation overflow can occur only in the opposite, outward
/// direction of an orthant: upward on an active slot or downward on an
/// inactive slot.  The other two directions remain representable for every
/// source point in their respective orthants.
pub(crate) fn finite_boundary_hazard_range(
    active: bool,
    delta: i64,
    coordinate: usize,
) -> Result<Option<BoundaryHazardRange>, WhenBadCoreError> {
    if delta == 0 {
        return Ok(None);
    }
    let (kind, first, last, count_u64) = match (active, delta.is_positive()) {
        (false, true) => {
            let first = 1_i64
                .checked_sub(delta)
                .ok_or(WhenBadCoreError::BoundaryArithmeticOverflow { coordinate })?;
            (
                WhenBadBoundaryHazardKind::InactiveSectorActivation,
                first,
                0,
                delta.unsigned_abs(),
            )
        }
        (true, true) => {
            let first = i64::MAX
                .checked_sub(delta)
                .and_then(|value| value.checked_add(1))
                .ok_or(WhenBadCoreError::BoundaryArithmeticOverflow { coordinate })?;
            (
                WhenBadBoundaryHazardKind::ConcreteIndexOverflow,
                first,
                i64::MAX,
                delta.unsigned_abs(),
            )
        }
        (false, false) => {
            let magnitude = delta.unsigned_abs();
            let last = i128::from(i64::MIN)
                .checked_add(i128::from(magnitude))
                .and_then(|value| value.checked_sub(1))
                .and_then(|value| i64::try_from(value).ok())
                .ok_or(WhenBadCoreError::BoundaryArithmeticOverflow { coordinate })?;
            (
                WhenBadBoundaryHazardKind::ConcreteIndexOverflow,
                i64::MIN,
                last,
                magnitude,
            )
        }
        (true, false) => return Ok(None),
    };
    let count =
        usize::try_from(count_u64).map_err(|_| WhenBadCoreError::ResourceCountOverflow {
            resource: "WhenBad boundary values per RHS",
        })?;
    Ok(Some(BoundaryHazardRange {
        kind,
        first,
        last,
        count,
    }))
}

fn boundary_polynomial(
    context: &ParametricCoefficientContext,
    coordinate: usize,
    boundary_value: i64,
    limits: ParametricArithmeticLimits,
) -> Result<ParametricPolynomial, WhenBadCompilerError> {
    let value = context.integer(boundary_value);
    let difference =
        context.sub_with_limits(&context.index(coordinate)?, &value, limits.exact_algebra)?;
    Ok(context.numerator_condition_with_limits(&difference, limits.exact_algebra)?)
}

fn route_polynomial(
    builder: &mut SymbolicSectorCasePartitionBuilder,
    context: &ParametricCoefficientContext,
    case: SymbolicSectorCaseId,
    polynomial: ParametricPolynomial,
) -> Result<(Option<SymbolicSectorCaseId>, Option<SymbolicSectorCaseId>), WhenBadCompilerError> {
    let mut decided = None;
    if let Some(current) = builder.case(case) {
        for predicate in current.predicates() {
            if context.polynomial_loci_are_associates_with_limits(
                predicate.polynomial(),
                &polynomial,
                builder.limits().exact_algebra,
            )? {
                decided = Some(predicate.kind());
                break;
            }
        }
    }
    if let Some(kind) = decided {
        return Ok(match kind {
            SymbolicPolynomialPredicateKind::EqualZero => (Some(case), None),
            SymbolicPolynomialPredicateKind::NonZero => (None, Some(case)),
        });
    }
    let children = builder.split_on_bad_polynomial(context, case, polynomial)?;
    Ok((
        Some(children.equal_zero_case()),
        Some(children.nonzero_case()),
    ))
}

/// Build one owned signed-descent witness after the caller has prospectively
/// charged both one witness and `shift.arity()` aggregate witness components.
/// Every allocation performed here is fallible; this helper owns no aggregate
/// budget because global and target-relative compilers have different limits.
pub(crate) fn prove_uniform_same_sector_descent(
    sector: &SectorMask,
    rhs_ordinal: usize,
    shift: &IndexShift,
) -> Result<Result<WhenBadUniformDescentWitness, WhenBadUnsupportedReason>, WhenBadCoreError> {
    prove_uniform_same_sector_descent_with_policy(
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        sector,
        rhs_ordinal,
        shift,
    )
}

/// Policy-explicit form used by authenticated candidate compilation. Keeping
/// this match exhaustive makes adding an ordering policy a compile-time audit
/// point instead of silently reusing the V1 descent tuple.
pub(crate) fn prove_uniform_same_sector_descent_with_policy(
    policy: IntegralOrderingPolicy,
    sector: &SectorMask,
    rhs_ordinal: usize,
    shift: &IndexShift,
) -> Result<Result<WhenBadUniformDescentWitness, WhenBadUnsupportedReason>, WhenBadCoreError> {
    match policy {
        IntegralOrderingPolicy::RustRedUnshiftedV1 => {}
    }
    if sector.arity() != shift.arity() {
        return Err(WhenBadCoreError::WrongArity {
            expected: sector.arity(),
            actual: shift.arity(),
        });
    }
    let mut corner_delta = 0i128;
    let mut dot_delta = 0i128;
    let mut numerator_delta = 0i128;
    let admitted_component_capacity =
        shift
            .arity()
            .checked_mul(2)
            .ok_or(WhenBadCoreError::ResourceCountOverflow {
                resource: "WhenBad descent component capacity envelope",
            })?;
    let admitted_index_excess_bytes = admitted_component_capacity
        .checked_mul(size_of::<i128>())
        .ok_or(WhenBadCoreError::ResourceCountOverflow {
            resource: "WhenBad descent index-excess capacity envelope bytes",
        })?;
    let admitted_rhs_shift_bytes = admitted_component_capacity
        .checked_mul(size_of::<i64>())
        .ok_or(WhenBadCoreError::ResourceCountOverflow {
            resource: "WhenBad descent RHS-shift capacity envelope bytes",
        })?;
    let mut index_excess_deltas = Vec::new();
    try_reserve_core(
        "WhenBad descent index-excess components",
        &mut index_excess_deltas,
        shift.arity(),
    )?;
    let observed_index_excess_bytes = index_excess_deltas
        .capacity()
        .checked_mul(size_of::<i128>())
        .ok_or(WhenBadCoreError::ResourceCountOverflow {
            resource: "WhenBad descent index-excess retained bytes",
        })?;
    if observed_index_excess_bytes > admitted_index_excess_bytes {
        return Err(WhenBadCoreError::RetainedCapacityEnvelopeExceeded {
            resource: "WhenBad descent index-excess components",
            observed_bytes: observed_index_excess_bytes,
            admitted_bytes: admitted_index_excess_bytes,
        });
    }
    for (&active, &delta) in sector.active_bits().iter().zip(shift.values()) {
        let delta = i128::from(delta);
        let excess_delta = if active { delta } else { -delta };
        corner_delta = corner_delta
            .checked_add(excess_delta)
            .ok_or(WhenBadCoreError::DescentArithmeticOverflow)?;
        if active {
            dot_delta = dot_delta
                .checked_add(delta)
                .ok_or(WhenBadCoreError::DescentArithmeticOverflow)?;
        } else {
            numerator_delta = numerator_delta
                .checked_sub(delta)
                .ok_or(WhenBadCoreError::DescentArithmeticOverflow)?;
        }
        index_excess_deltas.push(excess_delta);
    }
    let components = [
        (WhenBadDescentComponent::CornerDistance, corner_delta),
        (WhenBadDescentComponent::DotPower, dot_delta),
        (WhenBadDescentComponent::NumeratorPower, numerator_delta),
    ];
    let first = components
        .into_iter()
        .chain(
            index_excess_deltas
                .iter()
                .copied()
                .enumerate()
                .map(|(position, delta)| {
                    (WhenBadDescentComponent::IndexExcess { position }, delta)
                }),
        )
        .find(|(_, delta)| *delta != 0);
    let rhs_shift = IndexShift::try_new(shift.values().iter().copied(), shift.arity())
        .map_err(WhenBadCoreError::ParametricRelation)?;
    let observed_rhs_shift_bytes =
        rhs_shift
            .owned_retained_byte_bound()
            .ok_or(WhenBadCoreError::ResourceCountOverflow {
                resource: "WhenBad descent RHS-shift retained bytes",
            })?;
    if observed_rhs_shift_bytes > admitted_rhs_shift_bytes {
        return Err(WhenBadCoreError::RetainedCapacityEnvelopeExceeded {
            resource: "WhenBad descent RHS-shift components",
            observed_bytes: observed_rhs_shift_bytes,
            admitted_bytes: admitted_rhs_shift_bytes,
        });
    }
    let Some((decisive_component, delta)) = first else {
        return Ok(Err(
            WhenBadUnsupportedReason::ZeroSameSectorComplexityDelta {
                rhs_ordinal,
                rhs_shift,
            },
        ));
    };
    if delta > 0 {
        return Ok(Err(WhenBadUnsupportedReason::NonUniformSameSectorDescent {
            rhs_ordinal,
            rhs_shift,
            first_nonzero_component: decisive_component,
            delta,
        }));
    }
    Ok(Ok(WhenBadUniformDescentWitness {
        policy,
        rhs_ordinal,
        rhs_shift,
        corner_delta,
        dot_delta,
        numerator_delta,
        index_excess_deltas,
        decisive_component,
    }))
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, WhenBadCompilerError> {
    left.checked_add(right)
        .ok_or(WhenBadCompilerError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, WhenBadCompilerError> {
    left.checked_mul(right)
        .ok_or(WhenBadCompilerError::ResourceCountOverflow { resource })
}

fn try_reserve_core<T>(
    resource: &'static str,
    target: &mut Vec<T>,
    requested: usize,
) -> Result<(), WhenBadCoreError> {
    target
        .try_reserve_exact(requested)
        .map_err(|_| WhenBadCoreError::AllocationFailure {
            resource,
            requested,
        })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), WhenBadCompilerError> {
    if requested > limit {
        Err(WhenBadCompilerError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WhenBadCompilerError {
    FamilyMismatch,
    ContextMismatch,
    SchemaMismatch,
    ReplayMismatch,
    IndexDependentCylindricalBaseAssumption {
        witness_ordinal: usize,
    },
    WrongArity {
        expected: usize,
        actual: usize,
    },
    BoundaryArithmeticOverflow {
        coordinate: usize,
    },
    DescentArithmeticOverflow,
    UnsatisfiableCandidateDomain,
    InternalClassificationMismatch,
    PartitionEvaluationMismatch,
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ParametricRule(ParametricRuleError),
    ParametricCoefficient(ParametricCoefficientError),
    GeneratedCylindricalCandidate(Box<GeneratedCylindricalCandidateAuthorityError>),
    SectorCase(SymbolicSectorCaseError),
    Relation(crate::ParametricRelationError),
    Sector(crate::SectorFoundationError),
}

impl fmt::Display for WhenBadCompilerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FamilyMismatch => formatter.write_str("WhenBad candidate family mismatch"),
            Self::ContextMismatch => formatter.write_str("WhenBad candidate context mismatch"),
            Self::SchemaMismatch => formatter.write_str("WhenBad certificate schema mismatch"),
            Self::ReplayMismatch => formatter.write_str("WhenBad certificate replay mismatch"),
            Self::IndexDependentCylindricalBaseAssumption { witness_ordinal } => write!(
                formatter,
                "generated cylindrical base assumption witness {witness_ordinal} depends on integral indices"
            ),
            Self::WrongArity { expected, actual } => {
                write!(formatter, "WhenBad arity is {actual}, expected {expected}")
            }
            Self::BoundaryArithmeticOverflow { coordinate } => write!(
                formatter,
                "WhenBad boundary arithmetic overflow at coordinate {coordinate}"
            ),
            Self::DescentArithmeticOverflow => {
                formatter.write_str("WhenBad uniform-descent arithmetic overflow")
            }
            Self::UnsatisfiableCandidateDomain => {
                formatter.write_str("WhenBad candidate has an identically zero domain guard")
            }
            Self::InternalClassificationMismatch => {
                formatter.write_str("WhenBad final leaf classification mismatch")
            }
            Self::PartitionEvaluationMismatch => {
                formatter.write_str("WhenBad partition did not evaluate to exactly one leaf")
            }
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requested {requested}, configured limit is {limit}"
            ),
            Self::ParametricRule(error) => error.fmt(formatter),
            Self::ParametricCoefficient(error) => error.fmt(formatter),
            Self::GeneratedCylindricalCandidate(error) => error.fmt(formatter),
            Self::SectorCase(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
            Self::Sector(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for WhenBadCompilerError {}

impl From<WhenBadCoreError> for WhenBadCompilerError {
    fn from(value: WhenBadCoreError) -> Self {
        match value {
            WhenBadCoreError::WrongArity { expected, actual } => {
                Self::WrongArity { expected, actual }
            }
            WhenBadCoreError::BoundaryArithmeticOverflow { coordinate } => {
                Self::BoundaryArithmeticOverflow { coordinate }
            }
            WhenBadCoreError::DescentArithmeticOverflow => Self::DescentArithmeticOverflow,
            WhenBadCoreError::RetainedCapacityEnvelopeExceeded {
                resource,
                observed_bytes,
                admitted_bytes,
            } => Self::ResourceLimit {
                resource,
                requested: observed_bytes,
                limit: admitted_bytes,
            },
            WhenBadCoreError::ResourceCountOverflow { resource } => {
                Self::ResourceCountOverflow { resource }
            }
            WhenBadCoreError::AllocationFailure {
                resource,
                requested,
            } => Self::Relation(crate::ParametricRelationError::AllocationFailure {
                resource,
                requested,
            }),
            WhenBadCoreError::ParametricRelation(error) => Self::Relation(error),
        }
    }
}

impl From<ParametricRuleError> for WhenBadCompilerError {
    fn from(value: ParametricRuleError) -> Self {
        Self::ParametricRule(value)
    }
}

impl From<ParametricCoefficientError> for WhenBadCompilerError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::ParametricCoefficient(value)
    }
}

impl From<GeneratedCylindricalCandidateAuthorityError> for WhenBadCompilerError {
    fn from(value: GeneratedCylindricalCandidateAuthorityError) -> Self {
        Self::GeneratedCylindricalCandidate(Box::new(value))
    }
}

impl From<SymbolicSectorCaseError> for WhenBadCompilerError {
    fn from(value: SymbolicSectorCaseError) -> Self {
        Self::SectorCase(value)
    }
}

impl From<crate::ParametricRelationError> for WhenBadCompilerError {
    fn from(value: crate::ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}

impl From<crate::SectorFoundationError> for WhenBadCompilerError {
    fn from(value: crate::SectorFoundationError) -> Self {
        Self::Sector(value)
    }
}

#[cfg(test)]
mod adversarial_replay_tests {
    use super::*;
    use crate::{
        IntegralOrderingPolicy, ParametricElimination, ParametricEliminationLimits,
        ParametricEliminationOrdering, ParametricRelation, ParametricRowId,
        algebra::CoefficientContext,
    };
    use symbolica::atom::AtomCore;

    #[test]
    fn generated_affine_top_reduction_origin_copy_preserves_locator_only_census() {
        let origin = GuardOrigin::GeneratedAffineGroupTopReductionCoefficientDenominator {
            solve_group_ordinal: 17,
            database_epoch: 23,
            event_ordinal: 31,
            operation_ordinal: 37,
            term_ordinal: 41,
            pivot_normalization: true,
        };
        let copied = try_copy_guard_origin(&origin).unwrap();

        assert_eq!(copied, origin);
        assert_eq!(
            guard_origin_capacity_retained_bytes(&copied).unwrap(),
            origin.retained_byte_bound().unwrap()
        );
    }

    fn overflow_certificate() -> (ParametricCoefficientContext, WhenBadCertificate) {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context = ParametricCoefficientContext::try_new(&base, "when-bad-tamper", 2).unwrap();
        let zero = IndexShift::try_new([0, 0], 2).unwrap();
        let mut row = ParametricRelation::new(
            "when-bad-tamper-family",
            ParametricRowId::Derived {
                label: Arc::from("when-bad-tamper-row"),
            },
            &context,
        );
        row.add_term(&context, zero.clone(), context.one()).unwrap();
        row.add_term(
            &context,
            IndexShift::try_new([1, -2], 2).unwrap(),
            context.one(),
        )
        .unwrap();
        let rows = vec![row];
        let elimination = ParametricElimination::build(
            &context,
            &rows,
            ParametricEliminationOrdering::try_new(
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                [1, 3],
            )
            .unwrap(),
            ParametricEliminationLimits::default(),
        )
        .unwrap();
        let pivot = elimination
            .pivots()
            .iter()
            .position(|entry| entry.pivot() == &zero)
            .unwrap();
        let candidate = ParametricReductionRuleCandidate::try_from_elimination_pivot(
            &context,
            &rows,
            &elimination,
            pivot,
            SectorMask::try_new([true, true]).unwrap(),
            ParametricRuleLimits::default(),
        )
        .unwrap();
        let WhenBadCompilation::Certified(certificate) =
            WhenBadCompiler::compile_algebraic_candidate(
                &context,
                &candidate,
                WhenBadCompilerLimits::default(),
            )
            .unwrap()
        else {
            panic!("mixed descending fixture must certify")
        };
        (context, certificate)
    }

    fn multi_payload_certificate() -> (ParametricCoefficientContext, WhenBadCertificate) {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context =
            ParametricCoefficientContext::try_new(&base, "when-bad-multi-payload", 2).unwrap();
        let zero = IndexShift::try_new([0, 0], 2).unwrap();
        let mut row = ParametricRelation::new(
            "when-bad-multi-payload-family",
            ParametricRowId::Derived {
                label: Arc::from("when-bad-multi-payload-row"),
            },
            &context,
        );
        row.add_term(&context, zero.clone(), context.one()).unwrap();
        // Both shifts descend by one unit in the complete ordering tuple, but
        // their positive first components generate one and two distinct active
        // integer-overflow boundary events respectively.
        row.add_term(
            &context,
            IndexShift::try_new([1, -2], 2).unwrap(),
            context.one(),
        )
        .unwrap();
        row.add_term(
            &context,
            IndexShift::try_new([2, -3], 2).unwrap(),
            context.one(),
        )
        .unwrap();
        let rows = vec![row];
        let elimination = ParametricElimination::build(
            &context,
            &rows,
            ParametricEliminationOrdering::try_new(
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                [10, 10],
            )
            .unwrap(),
            ParametricEliminationLimits::default(),
        )
        .unwrap();
        let pivot = elimination
            .pivots()
            .iter()
            .position(|entry| entry.pivot() == &zero)
            .expect("the strictly hardest zero shift must be the retained pivot");
        let candidate = ParametricReductionRuleCandidate::try_from_elimination_pivot(
            &context,
            &rows,
            &elimination,
            pivot,
            SectorMask::try_new([true, true]).unwrap(),
            ParametricRuleLimits::default(),
        )
        .unwrap();
        let WhenBadCompilation::Certified(certificate) =
            WhenBadCompiler::compile_algebraic_candidate(
                &context,
                &candidate,
                WhenBadCompilerLimits::default(),
            )
            .unwrap()
        else {
            panic!("both mixed shifts must certify as uniformly descending")
        };
        (context, certificate)
    }

    fn guarded_certificate() -> (ParametricCoefficientContext, WhenBadCertificate) {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context =
            ParametricCoefficientContext::try_new(&base, "when-bad-origin-bytes", 1).unwrap();
        let zero = IndexShift::try_new([0], 1).unwrap();
        let mut row = ParametricRelation::new(
            "when-bad-origin-bytes-family",
            ParametricRowId::Derived {
                label: Arc::from("when-bad-origin-bytes-row"),
            },
            &context,
        );
        row.add_term(&context, zero.clone(), context.one()).unwrap();
        row.add_term(
            &context,
            IndexShift::try_new([-1], 1).unwrap(),
            context.one(),
        )
        .unwrap();
        let guard = context
            .sub(&context.index(0).unwrap(), &context.integer(2))
            .unwrap();
        row.add_nonzero_condition(&context, context.numerator_condition(&guard).unwrap())
            .unwrap();
        let rows = vec![row];
        let elimination = ParametricElimination::build(
            &context,
            &rows,
            ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [3])
                .unwrap(),
            ParametricEliminationLimits::default(),
        )
        .unwrap();
        let pivot = elimination
            .pivots()
            .iter()
            .position(|entry| entry.pivot() == &zero)
            .unwrap();
        let candidate = ParametricReductionRuleCandidate::try_from_elimination_pivot(
            &context,
            &rows,
            &elimination,
            pivot,
            SectorMask::try_new([true]).unwrap(),
            ParametricRuleLimits::default(),
        )
        .unwrap();
        let WhenBadCompilation::Certified(certificate) =
            WhenBadCompiler::compile_algebraic_candidate(
                &context,
                &candidate,
                WhenBadCompilerLimits::default(),
            )
            .unwrap()
        else {
            panic!("guarded descending fixture must certify")
        };
        (context, certificate)
    }

    #[test]
    fn public_replay_rejects_tampered_boundary_kind_and_leaf_disposition() {
        let (context, certificate) = overflow_certificate();
        certificate.replay(&context).unwrap();
        assert_eq!(certificate.schema(), WHEN_BAD_COMPILER_V2_SCHEMA);
        assert_eq!(certificate.leak_events().len(), 1);
        assert_eq!(
            certificate.leak_events()[0].kind(),
            WhenBadBoundaryHazardKind::ConcreteIndexOverflow,
        );
        assert_eq!(
            certificate.descent_witnesses()[0].policy(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
        );
        assert!(matches!(
            certificate.binding().ordering_authority(),
            WhenBadOrderingAuthority::AnchoredV1 { .. }
        ));
        assert!(
            certificate
                .binding()
                .ordering_authority()
                .discovery_anchor()
                .is_some()
        );

        let mut tampered_kind = certificate.clone();
        tampered_kind.core.leak_events[0].kind =
            WhenBadBoundaryHazardKind::InactiveSectorActivation;
        assert!(matches!(
            tampered_kind.replay(&context),
            Err(WhenBadCompilerError::ReplayMismatch)
        ));

        let mut tampered_leaf = certificate.clone();
        let exceptional = tampered_leaf
            .core
            .classifications
            .iter_mut()
            .find(|entry| {
                matches!(
                    entry.disposition,
                    WhenBadLeafDisposition::ExceptionalSectorLeak { .. }
                )
            })
            .unwrap();
        exceptional.disposition = WhenBadLeafDisposition::CoveredByCandidate;
        assert!(matches!(
            tampered_leaf.replay(&context),
            Err(WhenBadCompilerError::ReplayMismatch)
        ));
    }

    #[test]
    fn small_exhaustive_shifts_agree_with_the_persisted_concrete_order() {
        let policy = IntegralOrderingPolicy::RustRedUnshiftedV1;
        for mask in 0_u8..8 {
            let sector =
                SectorMask::try_new([mask & 1 != 0, mask & 2 != 0, mask & 4 != 0]).unwrap();
            for delta0 in -3_i64..=3 {
                for delta1 in -3_i64..=3 {
                    for delta2 in -3_i64..=3 {
                        if [delta0, delta1, delta2] == [0, 0, 0] {
                            continue;
                        }
                        let shift = IndexShift::try_new([delta0, delta1, delta2], 3).unwrap();
                        let proof = prove_uniform_same_sector_descent(&sector, 0, &shift).unwrap();
                        if proof.is_ok() {
                            for choice in 0_u8..8 {
                                let source: [i64; 3] = std::array::from_fn(|position| {
                                    let farther = choice & (1 << position) != 0;
                                    if sector.active_bits()[position] {
                                        if farther { 4 } else { 1 }
                                    } else if farther {
                                        -4
                                    } else {
                                        0
                                    }
                                });
                                let target = [
                                    source[0].checked_add(delta0).unwrap(),
                                    source[1].checked_add(delta1).unwrap(),
                                    source[2].checked_add(delta2).unwrap(),
                                ];
                                let target_sector = SectorMask::try_from_indices(&target).unwrap();
                                if !target_sector.is_subsector_of(&sector).unwrap() {
                                    // Precisely the inactive-activation cases
                                    // handled by the finite hazard partition.
                                    continue;
                                }
                                assert_eq!(
                                    policy.compare(&target, &source).unwrap(),
                                    std::cmp::Ordering::Less,
                                    "certified shift {shift:?} failed in sector {sector} at {source:?}",
                                );
                            }
                        } else {
                            // Deep inside the orthant every |delta|<=3 target
                            // stays in the same sector, exposing the exact
                            // non-descent diagnosed by the affine proof.
                            let source: [i64; 3] = std::array::from_fn(|position| {
                                if sector.active_bits()[position] {
                                    10
                                } else {
                                    -10
                                }
                            });
                            let target =
                                [source[0] + delta0, source[1] + delta1, source[2] + delta2];
                            assert_eq!(SectorMask::try_from_indices(&target).unwrap(), sector);
                            assert_ne!(
                                policy.compare(&target, &source).unwrap(),
                                std::cmp::Ordering::Less,
                                "rejected shift {shift:?} unexpectedly descended in sector {sector}",
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn finite_boundary_hazard_ranges_cover_zero_units_and_integer_extremes() {
        assert_eq!(finite_boundary_hazard_range(true, 0, 0).unwrap(), None);
        assert_eq!(
            finite_boundary_hazard_range(true, -1, 0).unwrap(),
            None,
            "an active downward shift can only pinch to a lower sector",
        );

        let activation = finite_boundary_hazard_range(false, 1, 3).unwrap().unwrap();
        assert_eq!(
            activation.kind(),
            WhenBadBoundaryHazardKind::InactiveSectorActivation
        );
        assert_eq!(
            (activation.first(), activation.last(), activation.count()),
            (0, 0, 1)
        );

        if usize::BITS >= 64 {
            let active_overflow = finite_boundary_hazard_range(true, i64::MAX, 0)
                .unwrap()
                .unwrap();
            assert_eq!(
                active_overflow.kind(),
                WhenBadBoundaryHazardKind::ConcreteIndexOverflow
            );
            assert_eq!(active_overflow.first(), 1);
            assert_eq!(active_overflow.last(), i64::MAX);
            assert_eq!(
                active_overflow.count(),
                usize::try_from(i64::MAX.unsigned_abs()).unwrap()
            );

            let inactive_underflow = finite_boundary_hazard_range(false, i64::MIN, 1)
                .unwrap()
                .unwrap();
            assert_eq!(
                inactive_underflow.kind(),
                WhenBadBoundaryHazardKind::ConcreteIndexOverflow
            );
            assert_eq!(inactive_underflow.first(), i64::MIN);
            assert_eq!(inactive_underflow.last(), -1);
            assert_eq!(
                inactive_underflow.count(),
                usize::try_from(i64::MIN.unsigned_abs()).unwrap()
            );
        } else {
            assert!(matches!(
                finite_boundary_hazard_range(true, i64::MAX, 0),
                Err(WhenBadCoreError::ResourceCountOverflow {
                    resource: "WhenBad boundary values per RHS"
                })
            ));
            assert!(matches!(
                finite_boundary_hazard_range(false, i64::MIN, 1),
                Err(WhenBadCoreError::ResourceCountOverflow {
                    resource: "WhenBad boundary values per RHS"
                })
            ));
        }
    }

    #[test]
    fn extreme_signed_descent_is_exact_and_arity_checked() {
        let sector = SectorMask::try_new([true, false]).unwrap();
        let descending = IndexShift::try_new([i64::MIN, i64::MAX], 2).unwrap();
        let witness = prove_uniform_same_sector_descent(&sector, 7, &descending)
            .unwrap()
            .unwrap();
        assert_eq!(witness.rhs_ordinal(), 7);
        assert_eq!(witness.rhs_shift(), &descending);
        assert_eq!(
            witness.decisive_component(),
            WhenBadDescentComponent::CornerDistance
        );
        assert_eq!(witness.index_excess_deltas.len(), 2);

        let ascending = IndexShift::try_new([i64::MAX, i64::MIN], 2).unwrap();
        assert!(matches!(
            prove_uniform_same_sector_descent(&sector, 8, &ascending).unwrap(),
            Err(WhenBadUnsupportedReason::NonUniformSameSectorDescent {
                rhs_ordinal: 8,
                first_nonzero_component: WhenBadDescentComponent::CornerDistance,
                delta,
                ..
            }) if delta > 0
        ));

        let zero = IndexShift::try_new([0, 0], 2).unwrap();
        assert!(matches!(
            prove_uniform_same_sector_descent(&sector, 9, &zero).unwrap(),
            Err(WhenBadUnsupportedReason::ZeroSameSectorComplexityDelta { rhs_ordinal: 9, .. })
        ));

        let wrong_arity = IndexShift::try_new([0], 1).unwrap();
        assert!(matches!(
            prove_uniform_same_sector_descent(&sector, 0, &wrong_arity),
            Err(WhenBadCoreError::WrongArity {
                expected: 2,
                actual: 1,
            })
        ));
    }

    #[test]
    fn descent_witness_owned_retained_bytes_charge_spare_vector_capacity() {
        let sector = SectorMask::try_new([true, false, true]).unwrap();
        let shift = IndexShift::try_new([-1, 0, 0], 3).unwrap();
        let mut witness = prove_uniform_same_sector_descent(&sector, 4, &shift)
            .unwrap()
            .unwrap();
        witness.index_excess_deltas.try_reserve_exact(5).unwrap();
        assert!(witness.index_excess_deltas.capacity() > witness.index_excess_deltas.len());

        let expected = size_of::<WhenBadUniformDescentWitness>()
            .checked_add(witness.rhs_shift.owned_retained_byte_bound().unwrap())
            .unwrap()
            .checked_add(
                witness
                    .index_excess_deltas
                    .capacity()
                    .checked_mul(size_of::<i128>())
                    .unwrap(),
            )
            .unwrap();
        let length_based = size_of::<WhenBadUniformDescentWitness>()
            + witness.rhs_shift.owned_retained_byte_bound().unwrap()
            + witness.index_excess_deltas.len() * size_of::<i128>();
        assert_ne!(expected, length_based);
        assert_eq!(witness.owned_retained_byte_bound(), Some(expected));
    }

    #[test]
    fn aggregate_descent_component_limit_is_checked_before_witness_retention() {
        let (context, certificate) = overflow_certificate();
        assert_eq!(certificate.stats().descent_witnesses(), 1);
        assert_eq!(certificate.stats().descent_witness_components(), 2);

        let mut exact = certificate.core.limits;
        exact.max_descent_witness_components = 2;
        let WhenBadCompilation::Certified(exact_certificate) =
            WhenBadCompiler::compile_algebraic_candidate(&context, &certificate.candidate, exact)
                .unwrap()
        else {
            panic!("the exact aggregate component bound must certify")
        };
        assert_eq!(exact_certificate.stats().descent_witness_components(), 2);

        let mut one_below = exact;
        one_below.max_descent_witness_components = 1;
        assert!(matches!(
            WhenBadCompiler::compile_algebraic_candidate(
                &context,
                &certificate.candidate,
                one_below,
            ),
            Err(WhenBadCompilerError::ResourceLimit {
                resource: "WhenBad descent witness components",
                requested: 2,
                limit: 1,
            })
        ));

        let mut bad_component_stats = certificate.clone();
        bad_component_stats.core.stats.descent_witness_components += 1;
        assert!(matches!(
            bad_component_stats.replay_payload_without_recompile(&context),
            Err(WhenBadCompilerError::ReplayMismatch)
        ));

        let mut truncated_components = certificate.clone();
        truncated_components.core.descent_witnesses[0]
            .index_excess_deltas
            .pop();
        assert!(matches!(
            truncated_components.replay_payload_without_recompile(&context),
            Err(WhenBadCompilerError::ReplayMismatch)
        ));
        certificate.replay(&context).unwrap();
    }

    #[test]
    fn aggregate_leak_shift_limits_are_exact_and_replay_authenticated() {
        let (context, certificate) = overflow_certificate();
        let stats = certificate.stats();
        assert_eq!(stats.leak_events(), 1);
        assert_eq!(stats.leak_event_shift_components(), 2);
        let expected_retained_bytes = certificate
            .core
            .leak_events
            .capacity()
            .checked_mul(size_of::<WhenBadLeakEvent>())
            .unwrap()
            .checked_add(
                certificate.leak_events()[0]
                    .rhs_shift
                    .owned_retained_byte_bound()
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(stats.leak_event_retained_bytes(), expected_retained_bytes);

        let mut exact = certificate.core.limits;
        exact.max_leak_event_shift_components = stats.leak_event_shift_components();
        exact.max_leak_event_retained_bytes = stats.leak_event_retained_bytes();
        let WhenBadCompilation::Certified(exact_certificate) =
            WhenBadCompiler::compile_algebraic_candidate(&context, &certificate.candidate, exact)
                .unwrap()
        else {
            panic!("the exact aggregate leak-shift bounds must certify")
        };
        assert_eq!(exact_certificate.stats(), stats);

        let mut component_one_below = exact;
        component_one_below.max_leak_event_shift_components =
            stats.leak_event_shift_components() - 1;
        assert!(matches!(
            WhenBadCompiler::compile_algebraic_candidate(
                &context,
                &certificate.candidate,
                component_one_below,
            ),
            Err(WhenBadCompilerError::ResourceLimit {
                resource: "WhenBad leak-event shift components",
                requested: 2,
                limit: 1,
            })
        ));

        let mut byte_one_below = exact;
        byte_one_below.max_leak_event_retained_bytes = stats.leak_event_retained_bytes() - 1;
        assert!(matches!(
            WhenBadCompiler::compile_algebraic_candidate(
                &context,
                &certificate.candidate,
                byte_one_below,
            ),
            Err(WhenBadCompilerError::ResourceLimit {
                resource: "WhenBad leak-event retained bytes",
                requested,
                limit,
            }) if requested == stats.leak_event_retained_bytes()
                && limit + 1 == stats.leak_event_retained_bytes()
        ));

        let mut tampered_stats = certificate.clone();
        tampered_stats.core.stats.leak_event_shift_components += 1;
        assert!(matches!(
            tampered_stats.replay_payload_without_recompile(&context),
            Err(WhenBadCompilerError::ReplayMismatch)
        ));

        let mut tampered_bytes = certificate.clone();
        tampered_bytes.core.stats.leak_event_retained_bytes += 1;
        assert!(matches!(
            tampered_bytes.replay(&context),
            Err(WhenBadCompilerError::ReplayMismatch)
        ));
    }

    #[test]
    fn multi_event_and_multi_witness_aggregate_limits_are_exact() {
        let (context, certificate) = multi_payload_certificate();
        let stats = certificate.stats();
        assert_eq!(stats.rhs_terms(), 2);
        assert_eq!(stats.descent_witnesses(), 2);
        assert_eq!(stats.descent_witness_components(), 4);
        assert_eq!(stats.leak_events(), 3);
        assert_eq!(stats.leak_event_shift_components(), 6);
        assert_eq!(
            stats.leak_event_shift_components(),
            certificate
                .leak_events()
                .iter()
                .map(|event| event.rhs_shift().arity())
                .sum::<usize>(),
        );
        let expected_retained_bytes = certificate
            .core
            .leak_events
            .capacity()
            .checked_mul(size_of::<WhenBadLeakEvent>())
            .unwrap()
            .checked_add(
                certificate
                    .leak_events()
                    .iter()
                    .map(|event| event.rhs_shift.owned_retained_byte_bound().unwrap())
                    .sum::<usize>(),
            )
            .unwrap();
        assert_eq!(stats.leak_event_retained_bytes(), expected_retained_bytes);

        let mut exact = certificate.core.limits;
        exact.max_descent_witness_components = stats.descent_witness_components();
        exact.max_leak_event_shift_components = stats.leak_event_shift_components();
        exact.max_leak_event_retained_bytes = stats.leak_event_retained_bytes();
        let WhenBadCompilation::Certified(exact_certificate) =
            WhenBadCompiler::compile_algebraic_candidate(&context, &certificate.candidate, exact)
                .unwrap()
        else {
            panic!("all exact aggregate bounds must certify")
        };
        assert_eq!(exact_certificate.stats(), stats);

        let mut witness_one_below = exact;
        witness_one_below.max_descent_witness_components = stats.descent_witness_components() - 1;
        assert!(matches!(
            WhenBadCompiler::compile_algebraic_candidate(
                &context,
                &certificate.candidate,
                witness_one_below,
            ),
            Err(WhenBadCompilerError::ResourceLimit {
                resource: "WhenBad descent witness components",
                requested: 4,
                limit: 3,
            })
        ));

        let mut shift_one_below = exact;
        shift_one_below.max_leak_event_shift_components = stats.leak_event_shift_components() - 1;
        assert!(matches!(
            WhenBadCompiler::compile_algebraic_candidate(
                &context,
                &certificate.candidate,
                shift_one_below,
            ),
            Err(WhenBadCompilerError::ResourceLimit {
                resource: "WhenBad leak-event shift components",
                requested: 6,
                limit: 5,
            })
        ));

        let mut bytes_one_below = exact;
        bytes_one_below.max_leak_event_retained_bytes = stats.leak_event_retained_bytes() - 1;
        assert!(matches!(
            WhenBadCompiler::compile_algebraic_candidate(
                &context,
                &certificate.candidate,
                bytes_one_below,
            ),
            Err(WhenBadCompilerError::ResourceLimit {
                resource: "WhenBad leak-event retained bytes",
                requested,
                limit,
            }) if requested == stats.leak_event_retained_bytes()
                && limit + 1 == stats.leak_event_retained_bytes()
        ));
    }

    #[test]
    fn guard_origin_payload_bytes_are_exact_limited_and_replay_bound() {
        let (context, certificate) = guarded_certificate();
        let stats = certificate.stats();
        assert!(stats.guard_origins() > 0);
        assert!(stats.guard_origin_retained_bytes() > 0);
        let replayed =
            retained_domain_source_census(certificate.domain_conditions(), certificate.core.limits)
                .unwrap();
        assert_eq!(replayed.origins, stats.guard_origins());
        assert_eq!(replayed.origin_bytes, stats.guard_origin_retained_bytes());

        let mut exact = certificate.core.limits;
        exact.max_guard_origin_retained_bytes = stats.guard_origin_retained_bytes();
        let WhenBadCompilation::Certified(exact_certificate) =
            WhenBadCompiler::compile_algebraic_candidate(&context, &certificate.candidate, exact)
                .unwrap()
        else {
            panic!("the exact guard-origin payload byte bound must certify")
        };
        assert_eq!(
            exact_certificate.stats().guard_origin_retained_bytes(),
            stats.guard_origin_retained_bytes(),
        );

        let mut one_below = exact;
        one_below.max_guard_origin_retained_bytes = stats.guard_origin_retained_bytes() - 1;
        assert!(matches!(
            WhenBadCompiler::compile_algebraic_candidate(
                &context,
                &certificate.candidate,
                one_below,
            ),
            Err(WhenBadCompilerError::ResourceLimit {
                resource: "WhenBad guard-origin retained bytes",
                requested,
                limit,
            }) if requested == stats.guard_origin_retained_bytes()
                && limit + 1 == stats.guard_origin_retained_bytes()
        ));

        let mut tampered = certificate.clone();
        tampered.core.stats.guard_origin_retained_bytes += 1;
        assert!(matches!(
            tampered.replay(&context),
            Err(WhenBadCompilerError::ReplayMismatch)
        ));
    }

    #[test]
    fn candidate_binding_charges_every_anchored_payload_exactly() {
        let (context, certificate) = overflow_certificate();
        let binding = certificate.binding();
        let geometry_bytes = binding
            .sector
            .owned_retained_byte_bound()
            .unwrap()
            .checked_add(binding.original_pivot.owned_retained_byte_bound().unwrap())
            .unwrap();
        assert!(binding.retained_bytes() >= geometry_bytes);
        let discovery_anchor_bytes = match &binding.ordering_authority {
            WhenBadOrderingAuthority::AnchoredV1 {
                discovery_anchor, ..
            } => discovery_anchor.capacity(),
            WhenBadOrderingAuthority::CylindricalV1 { .. } => unreachable!(),
        }
        .checked_mul(size_of::<i64>())
        .unwrap();
        assert!(discovery_anchor_bytes > 0);
        let WhenBadCandidateSourceAuthority::AnchoredEliminationV1 {
            source_manifest,
            trace_manifest,
            ..
        } = &binding.source_authority
        else {
            panic!("anchored fixture must retain anchored source authority")
        };
        assert_eq!(
            binding.ordering_authority.manifest(),
            certificate.candidate.ordering().stable_string(),
            "the fallible streaming encoder must preserve the V2 ordering identity",
        );
        let trace = certificate.candidate.trace();
        let mut legacy_trace = format!(
            "base={}|reductions={}|divisor={}",
            trace.base_source_row_index(),
            trace.reductions().len(),
            trace.divisor().to_expression().to_canonical_string(),
        );
        for reduction in trace.reductions() {
            legacy_trace.push_str(&format!(
                "|{}:{}",
                reduction.prior_pivot_ordinal(),
                reduction.factor().to_expression().to_canonical_string(),
            ));
        }
        assert_eq!(
            trace_manifest, &legacy_trace,
            "the fallible streaming encoder must preserve the V2 trace identity",
        );
        let expected = [
            binding.family_fingerprint.capacity(),
            binding.context_fingerprint.capacity(),
            binding.sector.owned_retained_byte_bound().unwrap(),
            binding.original_pivot.owned_retained_byte_bound().unwrap(),
            binding.centered_relation_manifest.capacity(),
            match &binding.ordering_authority {
                WhenBadOrderingAuthority::AnchoredV1 { manifest, .. }
                | WhenBadOrderingAuthority::CylindricalV1 { manifest, .. } => manifest.capacity(),
            },
            discovery_anchor_bytes,
            source_manifest.capacity(),
            trace_manifest.capacity(),
        ]
        .into_iter()
        .sum::<usize>();
        assert_eq!(binding.retained_bytes(), expected,);
        assert_eq!(
            binding.retained_bytes(),
            candidate_binding_retained_bytes(binding).unwrap(),
        );

        let mut exact = certificate.core.limits;
        exact.max_candidate_binding_bytes = binding.retained_bytes();
        let WhenBadCompilation::Certified(exact_certificate) =
            WhenBadCompiler::compile_algebraic_candidate(&context, &certificate.candidate, exact)
                .unwrap()
        else {
            panic!("the exact candidate-binding byte bound must certify")
        };
        assert_eq!(
            exact_certificate.binding().retained_bytes(),
            binding.retained_bytes(),
        );

        let mut one_below = exact;
        one_below.max_candidate_binding_bytes = binding.retained_bytes() - 1;
        assert!(matches!(
            WhenBadCompiler::compile_algebraic_candidate(
                &context,
                &certificate.candidate,
                one_below,
            ),
            Err(WhenBadCompilerError::ResourceLimit {
                resource: "WhenBad candidate binding bytes",
                requested,
                limit,
            }) if requested == binding.retained_bytes() && limit + 1 == binding.retained_bytes()
        ));

        let mut tampered = certificate.clone();
        tampered.core.binding.retained_bytes -= geometry_bytes;
        assert!(matches!(
            tampered.replay_payload_without_recompile(&context),
            Err(WhenBadCompilerError::ReplayMismatch)
        ));
    }

    #[test]
    fn preferred_arc_compiler_retains_the_callers_candidate_allocation() {
        let (context, certificate) = overflow_certificate();
        let candidate = Arc::clone(&certificate.candidate);
        let retained = Arc::clone(&candidate);
        let WhenBadCompilation::Certified(compiled) =
            WhenBadCompiler::compile_algebraic_candidate_arc(
                &context,
                candidate,
                certificate.core.limits,
            )
            .unwrap()
        else {
            panic!("the shared candidate fixture must certify")
        };
        assert!(Arc::ptr_eq(&retained, &compiled.candidate));
    }

    #[test]
    fn public_replay_rejects_unbound_spare_candidate_and_core_capacity() {
        let (context, mut binding_capacity) = overflow_certificate();
        let old_binding_capacity = binding_capacity.core.binding.family_fingerprint.capacity();
        binding_capacity
            .core
            .binding
            .family_fingerprint
            .try_reserve_exact(128)
            .unwrap();
        assert!(binding_capacity.core.binding.family_fingerprint.capacity() > old_binding_capacity);
        assert!(matches!(
            binding_capacity.replay(&context),
            Err(WhenBadCompilerError::ReplayMismatch)
        ));

        let (context, mut leak_capacity) = overflow_certificate();
        let old_leak_capacity = leak_capacity.core.leak_events.capacity();
        leak_capacity
            .core
            .leak_events
            .try_reserve_exact(16)
            .unwrap();
        assert!(leak_capacity.core.leak_events.capacity() > old_leak_capacity);
        assert!(matches!(
            leak_capacity.replay(&context),
            Err(WhenBadCompilerError::ReplayMismatch)
        ));

        let (context, mut witness_capacity) = overflow_certificate();
        let old_witness_capacity = witness_capacity.core.descent_witnesses[0]
            .index_excess_deltas
            .capacity();
        witness_capacity.core.descent_witnesses[0]
            .index_excess_deltas
            .try_reserve_exact(16)
            .unwrap();
        assert!(
            witness_capacity.core.descent_witnesses[0]
                .index_excess_deltas
                .capacity()
                > old_witness_capacity
        );
        assert!(matches!(
            witness_capacity.replay(&context),
            Err(WhenBadCompilerError::ReplayMismatch)
        ));
    }

    #[test]
    fn retained_core_census_is_bound_for_certified_and_unsupported_payloads() {
        let (_context, certificate) = overflow_certificate();
        assert_eq!(
            certificate.retained_core_bytes(),
            certificate.core.observed_retained_core_bytes().unwrap(),
        );
        certificate.core.replay_capacity_census().unwrap();

        let binding = candidate_binding(
            WhenBadGlobalCandidateView::AnchoredV1(&certificate.candidate),
            certificate.core.limits.max_candidate_binding_bytes,
        )
        .unwrap();
        let reason = WhenBadUnsupportedReason::ZeroSameSectorComplexityDelta {
            rhs_ordinal: 0,
            rhs_shift: IndexShift::try_new([0, 0], 2).unwrap(),
        };
        let mut unsupported =
            WhenBadUnsupportedCore::try_new(binding, reason, certificate.core.limits).unwrap();
        assert_eq!(
            unsupported.retained_core_bytes(),
            unsupported.observed_retained_core_bytes().unwrap(),
        );
        unsupported.replay_capacity_census().unwrap();

        let WhenBadUnsupportedReason::ZeroSameSectorComplexityDelta { rhs_shift, .. } =
            &mut unsupported.reason
        else {
            unreachable!()
        };
        let mut oversized_shift = Vec::new();
        oversized_shift.try_reserve_exact(18).unwrap();
        oversized_shift.extend_from_slice(rhs_shift.values());
        *rhs_shift = IndexShift::try_from_preallocated(oversized_shift, 2).unwrap();
        assert!(matches!(
            unsupported.replay_capacity_census(),
            Err(WhenBadCompilerError::ReplayMismatch)
        ));
    }

    #[test]
    fn cloned_core_may_shrink_capacity_below_its_authenticated_upper_bound() {
        let (context, mut certificate) = multi_payload_certificate();
        certificate.clone().replay(&context).unwrap();
        certificate
            .core
            .classifications
            .try_reserve_exact(32)
            .unwrap();
        certificate.core.stats.retained_core_bytes =
            certificate.core.observed_retained_core_bytes().unwrap();
        certificate
            .replay_payload_without_recompile(&context)
            .unwrap();

        let cloned = certificate.clone();
        assert!(
            cloned.core.observed_retained_core_bytes().unwrap()
                < cloned.core.stats.retained_core_bytes,
            "Vec::clone must discard the deliberately admitted spare classification capacity",
        );
        cloned.replay_payload_without_recompile(&context).unwrap();
    }
}
