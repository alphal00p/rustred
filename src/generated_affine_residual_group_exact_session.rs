//! Allocation-sealed session for exact generated-affine group solving.
//!
//! This topology-neutral owner is the only production seam that pairs the
//! persistent exact row database with the unresolved targets derived from the
//! same solve-plan allocation.  A staged transaction retains both the
//! database's consume-once row token and the exact target-state `Arc`; neither
//! component is exposed separately.  Likewise, exact recentering receives one
//! borrowed, jointly authenticated view rather than caller-supplied database,
//! staged-row, or target-state parts.
//!
//! Both retained-source schemas expose a typed dependent-row commit and a
//! consuming, inert recenter classification. The recenter outcome retains the
//! transaction behind sealed NoTarget, equality-refinement, or Ready
//! typestates and provides no direct Ready commit. NoTarget may commit through
//! a consuming typed continuation;
//! equality may commit only into a one-way refined-epoch suspension. Neither
//! path publishes a rule or infers a master. A private unconsumed-commit kernel
//! proves the atomic database/target-state transition, and no raw successor
//! transition is exposed outside this module. Dropping an unconsumed staged
//! transaction or recenter outcome leaves both retained owners unchanged.

use std::fmt;
use std::mem::size_of;
use std::ops::Range;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::prelude::Integer;

use crate::generated_affine_residual_case_inventory::{
    GeneratedAffineResidualCaseAuthoritySourceKind, GeneratedAffineResidualInventoryGroupSourceView,
};
use crate::generated_affine_residual_case_premises::GeneratedAffineResidualCaseEqualityRefinementCertificate;
use crate::generated_affine_residual_group_exact_database::{
    GeneratedAffineResidualGroupAuthenticatedStagedNewPivotView,
    GeneratedAffineResidualGroupExactDatabase, GeneratedAffineResidualGroupExactDatabaseError,
    GeneratedAffineResidualGroupExactDatabaseLimits,
    GeneratedAffineResidualGroupExactNativeSparseScalingStats,
    GeneratedAffineResidualGroupExactReductionStep,
    GeneratedAffineResidualGroupPreparedExactRowCommit,
    GeneratedAffineResidualGroupRetainedExactDependentReductions,
    GeneratedAffineResidualGroupRetainedExactSourceRecipe,
    GeneratedAffineResidualGroupRetainedExactUnitPivot, GeneratedAffineResidualGroupStagedExactRow,
};
use crate::generated_affine_residual_group_exact_physical_row::GeneratedAffineResidualGroupExactPhysicalRow;
use crate::generated_affine_residual_group_exact_publication::{
    PreparedPublication, PublicationLeaf, PublicationLeafDisposition, PublicationPayload,
    PublicationStats,
};
use crate::generated_affine_residual_group_exact_recenter_kernel::{
    ExactRecenterKernelError, ExactRecenterKernelLimits, ExactRecenterKernelStats,
    ExactRecenteredApplicationRow, ExactRecenteredRow, ExactRecenteredTerm, ExactTargetOffset,
    admit_inert_owner, bounded_add, checked_add, exact_offsets_equal, execute_target_offset,
    observe_inert_owner, preflight_exact_geometry, translate_centered_row,
    verify_target_offset_census,
};
use crate::generated_affine_residual_group_exact_targets::{
    GeneratedAffineResidualGroupExactTargetCatalog,
    GeneratedAffineResidualGroupExactTargetCatalogLimits,
    GeneratedAffineResidualGroupExactTargetError, GeneratedAffineResidualGroupExactTargetState,
    GeneratedAffineResidualGroupExactTargetStateLimits,
    GeneratedAffineResidualGroupExactTargetStateView,
    GeneratedAffineResidualGroupRetainedEqualityRefinementExactTarget,
    GeneratedAffineResidualGroupRetainedExactTarget,
    GeneratedAffineResidualGroupRetainedReadyExactTarget,
};
use crate::generated_affine_residual_group_physical_key::{
    GeneratedAffineResidualGroupLatticeShift, GeneratedAffineResidualGroupPhysicalFrame,
    GeneratedAffineResidualGroupPhysicalKey, GeneratedAffineResidualGroupPhysicalKeyError,
};
use crate::generated_affine_residual_group_solve_plan::{
    GeneratedAffineResidualGroupSolvePlan, GeneratedAffineResidualGroupSolveTargetLocator,
};
use crate::generated_residual_affine_when_bad::{
    AffineWhenBadArbitraryRelativeCase, AffineWhenBadArbitraryRelativePredicate,
};
use crate::{
    GuardOrigin, IntegralFamily, IntegralOrderingPolicy, ParametricCoefficient,
    ParametricCoefficientContext, ParametricNonZeroCondition, ParametricPolynomial, SectorMask,
    SymbolicPolynomialPredicateKind,
};

#[cfg(test)]
use crate::generated_affine_residual_group_exact_database::GeneratedAffineResidualGroupExactRowOutcome;

pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_SESSION_V1_SCHEMA: &str =
    "rustred-generated-affine-residual-group-exact-session-v1";
pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_SESSION_V2_SCHEMA: &str =
    "rustred-generated-affine-residual-group-exact-session-v2";
const GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_SESSION_EVENT_V1_SCHEMA: &str =
    "rustred-generated-affine-residual-group-exact-session-event-v1";
const GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_SESSION_EVENT_V2_SCHEMA: &str =
    "rustred-generated-affine-residual-group-exact-session-event-v2";

const fn exact_session_schema_for_source(
    source_kind: GeneratedAffineResidualCaseAuthoritySourceKind,
) -> &'static str {
    match source_kind {
        GeneratedAffineResidualCaseAuthoritySourceKind::LegacyInventory => {
            GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_SESSION_V1_SCHEMA
        }
        GeneratedAffineResidualCaseAuthoritySourceKind::DirectFormulaSingleton => {
            GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_SESSION_V2_SCHEMA
        }
    }
}

const fn exact_session_event_schema_for_source(
    source_kind: GeneratedAffineResidualCaseAuthoritySourceKind,
) -> &'static str {
    match source_kind {
        GeneratedAffineResidualCaseAuthoritySourceKind::LegacyInventory => {
            GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_SESSION_EVENT_V1_SCHEMA
        }
        GeneratedAffineResidualCaseAuthoritySourceKind::DirectFormulaSingleton => {
            GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_SESSION_EVENT_V2_SCHEMA
        }
    }
}

#[cfg(test)]
std::thread_local! {
    static EVENT_LEDGER_REPLACEMENT_RESERVATIONS_FOR_TEST: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn reset_event_ledger_replacement_reservations_for_test() {
    EVENT_LEDGER_REPLACEMENT_RESERVATIONS_FOR_TEST.with(|count| count.set(0));
}

#[cfg(test)]
fn event_ledger_replacement_reservations_for_test() -> usize {
    EVENT_LEDGER_REPLACEMENT_RESERVATIONS_FOR_TEST.with(std::cell::Cell::get)
}

#[cfg(test)]
fn record_event_ledger_replacement_reservation_for_test() {
    EVENT_LEDGER_REPLACEMENT_RESERVATIONS_FOR_TEST.with(|count| {
        count.set(count.get().saturating_add(1));
    });
}

/// Unforgeable safe-Rust capability for the session-only exact-database API.
///
/// The type is visible to the sibling database module solely so protected
/// methods can name it in their signatures. Its seal and constructor remain
/// private here, it is neither `Clone` nor `Default`, and the owning session
/// never returns a borrow. Consequently another production sibling may name
/// the type but cannot produce a value with which to stage, authenticate, or
/// commit a database transition.
pub(crate) struct GeneratedAffineResidualGroupExactSessionDatabaseCapability {
    _seal: GeneratedAffineResidualGroupExactSessionDatabaseCapabilitySeal,
}

struct GeneratedAffineResidualGroupExactSessionDatabaseCapabilitySeal;

impl GeneratedAffineResidualGroupExactSessionDatabaseCapability {
    fn mint() -> Self {
        Self {
            _seal: GeneratedAffineResidualGroupExactSessionDatabaseCapabilitySeal,
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionDatabaseCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionDatabaseCapability")
            .field("private_seal", &"<redacted>")
            .finish()
    }
}

/// Complete child limits for construction and replay of one exact session.
///
/// Each child owns its own arithmetic, replay, allocation, and retained-byte
/// accounting.  The session adds no unbounded collection of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactSessionLimits {
    pub(crate) database: GeneratedAffineResidualGroupExactDatabaseLimits,
    pub(crate) target_catalog: GeneratedAffineResidualGroupExactTargetCatalogLimits,
    pub(crate) target_state: GeneratedAffineResidualGroupExactTargetStateLimits,
    pub(crate) recenter: GeneratedAffineResidualGroupExactSessionRecenterLimits,
    pub(crate) events: GeneratedAffineResidualGroupExactSessionEventLimits,
}

impl Default for GeneratedAffineResidualGroupExactSessionLimits {
    fn default() -> Self {
        Self {
            database: GeneratedAffineResidualGroupExactDatabaseLimits::default(),
            target_catalog: GeneratedAffineResidualGroupExactTargetCatalogLimits::default(),
            target_state: GeneratedAffineResidualGroupExactTargetStateLimits::default(),
            recenter: GeneratedAffineResidualGroupExactSessionRecenterLimits::default(),
            events: GeneratedAffineResidualGroupExactSessionEventLimits::default(),
        }
    }
}

/// Resource envelope for the allocation-bound chronological session ledger.
///
/// This ledger is deliberately bounded independently of the algebra database:
/// dependent reduction traces and raw source recipes remain live here even
/// when the database itself does not retain them. Replay work has separate
/// limits so authentication cannot silently turn staging into an unbounded
/// historical scan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactSessionEventLimits {
    pub(crate) max_events: usize,
    pub(crate) max_source_recipe_allocation_comparisons: usize,
    pub(crate) max_target_state_successor_copies: usize,
    pub(crate) max_ledger_arc_copies: usize,
    pub(crate) max_reduction_steps: usize,
    pub(crate) max_source_recipe_retained_bytes: usize,
    pub(crate) max_target_offset_components: usize,
    pub(crate) max_target_offset_integer_bits: usize,
    pub(crate) max_target_offset_retained_bytes: usize,
    pub(crate) max_equality_predicates: usize,
    pub(crate) max_individual_event_retained_bytes: usize,
    pub(crate) max_ledger_outer_buffer_bytes: usize,
    pub(crate) max_ledger_retained_bytes: usize,
    pub(crate) max_ledger_replacement_peak_bytes: usize,
    pub(crate) max_replay_events: usize,
    pub(crate) max_replay_reduction_steps: usize,
    pub(crate) max_replay_pivot_terms: usize,
    pub(crate) max_replay_pivot_guards: usize,
    pub(crate) max_replay_target_offset_components: usize,
    pub(crate) max_replay_equality_predicates: usize,
    pub(crate) max_replay_target_scans: usize,
    pub(crate) max_replay_target_state_successor_copies: usize,
    pub(crate) max_replay_ledger_arc_copies: usize,
    /// Session-local coexistence while a fresh replay shadow is built. The
    /// pointer-shared plan-local allocation is charged once. Deeper immutable
    /// ancestry (inventory, frame parents, compiler certificates) follows the
    /// child-owner conventions and is excluded; this is not a whole-process
    /// RSS bound.
    pub(crate) max_replay_combined_retained_bytes: usize,
}

impl Default for GeneratedAffineResidualGroupExactSessionEventLimits {
    fn default() -> Self {
        Self {
            max_events: 256_000_000,
            max_source_recipe_allocation_comparisons: 1_000_000_000,
            max_target_state_successor_copies: 1_000_000_000,
            max_ledger_arc_copies: 1_000_000_000,
            max_reduction_steps: 1_000_000_000,
            max_source_recipe_retained_bytes: usize::MAX,
            max_target_offset_components: 256_000_000,
            max_target_offset_integer_bits: usize::MAX,
            max_target_offset_retained_bytes: usize::MAX,
            max_equality_predicates: 256_000_000,
            max_individual_event_retained_bytes: usize::MAX,
            max_ledger_outer_buffer_bytes: usize::MAX,
            max_ledger_retained_bytes: usize::MAX,
            max_ledger_replacement_peak_bytes: usize::MAX,
            max_replay_events: 256_000_000,
            max_replay_reduction_steps: 1_000_000_000,
            max_replay_pivot_terms: 1_000_000_000,
            max_replay_pivot_guards: 1_000_000_000,
            max_replay_target_offset_components: 1_000_000_000,
            max_replay_equality_predicates: 1_000_000_000,
            max_replay_target_scans: 1_000_000_000,
            max_replay_target_state_successor_copies: 1_000_000_000,
            max_replay_ledger_arc_copies: 1_000_000_000,
            max_replay_combined_retained_bytes: usize::MAX,
        }
    }
}

/// Exact/observed owner census for the chronological event ledger.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactSessionEventStats {
    events: usize,
    source_recipe_allocation_comparisons: usize,
    target_state_successor_copies: usize,
    ledger_arc_copies: usize,
    reduction_steps: usize,
    unique_source_recipe_retained_bytes: usize,
    target_offset_components: usize,
    target_offset_integer_bits: usize,
    target_offset_retained_bytes: usize,
    equality_predicates: usize,
    publication_retained_bytes: usize,
    ledger_outer_buffer_bytes: usize,
    ledger_retained_bytes: usize,
    ledger_replacement_peak_bytes: usize,
}

impl GeneratedAffineResidualGroupExactSessionEventStats {
    pub(crate) const fn events(self) -> usize {
        self.events
    }

    pub(crate) const fn reduction_steps(self) -> usize {
        self.reduction_steps
    }

    pub(crate) const fn source_recipe_allocation_comparisons(self) -> usize {
        self.source_recipe_allocation_comparisons
    }

    pub(crate) const fn publication_retained_bytes(self) -> usize {
        self.publication_retained_bytes
    }

    pub(crate) const fn target_state_successor_copies(self) -> usize {
        self.target_state_successor_copies
    }

    pub(crate) const fn ledger_arc_copies(self) -> usize {
        self.ledger_arc_copies
    }

    pub(crate) const fn unique_source_recipe_retained_bytes(self) -> usize {
        self.unique_source_recipe_retained_bytes
    }

    pub(crate) const fn target_offset_components(self) -> usize {
        self.target_offset_components
    }

    pub(crate) const fn target_offset_integer_bits(self) -> usize {
        self.target_offset_integer_bits
    }

    pub(crate) const fn target_offset_retained_bytes(self) -> usize {
        self.target_offset_retained_bytes
    }

    pub(crate) const fn equality_predicates(self) -> usize {
        self.equality_predicates
    }

    pub(crate) const fn ledger_outer_buffer_bytes(self) -> usize {
        self.ledger_outer_buffer_bytes
    }

    pub(crate) const fn ledger_retained_bytes(self) -> usize {
        self.ledger_retained_bytes
    }

    pub(crate) const fn ledger_replacement_peak_bytes(self) -> usize {
        self.ledger_replacement_peak_bytes
    }
}

/// Resource envelope for matching and translating one staged session pivot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactSessionRecenterLimits {
    pub(crate) kernel: ExactRecenterKernelLimits,
    pub(crate) max_target_scans: usize,
}

impl Default for GeneratedAffineResidualGroupExactSessionRecenterLimits {
    fn default() -> Self {
        Self {
            kernel: ExactRecenterKernelLimits::default(),
            max_target_scans: 256_000_000,
        }
    }
}

/// Auditable accounting for one session-owned recenter attempt.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactSessionRecenterStats {
    staged_live_prospective_retained_bytes: usize,
    staged_live_observed_retained_bytes: usize,
    target_state_combined_retained_byte_envelope: usize,
    external_live_retained_bytes: usize,
    target_scans: usize,
    unresolved_target_scans: usize,
    kernel: ExactRecenterKernelStats,
}

impl GeneratedAffineResidualGroupExactSessionRecenterStats {
    pub(crate) const fn staged_live_prospective_retained_bytes(self) -> usize {
        self.staged_live_prospective_retained_bytes
    }

    pub(crate) const fn staged_live_observed_retained_bytes(self) -> usize {
        self.staged_live_observed_retained_bytes
    }

    pub(crate) const fn target_state_combined_retained_byte_envelope(self) -> usize {
        self.target_state_combined_retained_byte_envelope
    }

    pub(crate) const fn external_live_retained_bytes(self) -> usize {
        self.external_live_retained_bytes
    }

    pub(crate) const fn target_scans(self) -> usize {
        self.target_scans
    }

    pub(crate) const fn unresolved_target_scans(self) -> usize {
        self.unresolved_target_scans
    }

    pub(crate) const fn kernel(self) -> ExactRecenterKernelStats {
        self.kernel
    }
}

/// Exact parent-allocation authority shared by one session and every event it
/// commits. Pointer identity is deliberately private: visible epoch/version
/// scalars can never substitute for the retained plan/catalog allocations.
struct GeneratedAffineResidualGroupExactSessionEventAuthority {
    schema: &'static str,
    source_kind: GeneratedAffineResidualCaseAuthoritySourceKind,
    plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
    catalog: Arc<GeneratedAffineResidualGroupExactTargetCatalog>,
    database_epoch: usize,
    group_ordinal: usize,
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionEventAuthority {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionEventAuthority")
            .field("schema", &self.schema)
            .field("database_epoch", &self.database_epoch)
            .field("group_ordinal", &self.group_ordinal)
            .field("private_plan", &"<redacted>")
            .field("private_catalog", &"<redacted>")
            .finish()
    }
}

enum GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence {
    Dependent(GeneratedAffineResidualGroupRetainedExactDependentReductions),
    NewPivot(GeneratedAffineResidualGroupRetainedExactUnitPivot),
}

impl GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence {
    fn reduction_count(&self) -> usize {
        match self {
            Self::Dependent(evidence) => evidence.reductions().len(),
            Self::NewPivot(evidence) => evidence.reductions().len(),
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Dependent(_) => "Dependent(<redacted>)",
            Self::NewPivot(_) => "NewPivot(<redacted>)",
        })
    }
}

/// Minimal ownership for interpreting one chronological event. Replayable
/// algebraic transitions retain their exact source/evidence; a compact
/// application event retains only the coordinate of the pivot installed in
/// the database by the same atomic transition.
enum GeneratedAffineResidualGroupExactSessionEventHead {
    Replayable {
        source_recipe: GeneratedAffineResidualGroupRetainedExactSourceRecipe,
        database_evidence: GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence,
    },
    Publication {
        pivot_ordinal: usize,
    },
}

#[derive(Clone, Copy)]
enum GeneratedAffineResidualGroupExactSessionEventHeadView<'a> {
    Replayable {
        source_recipe: &'a GeneratedAffineResidualGroupRetainedExactSourceRecipe,
        database_evidence: &'a GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence,
    },
    Publication {
        pivot_ordinal: usize,
    },
}

impl GeneratedAffineResidualGroupExactSessionEventHeadView<'_> {
    fn is_replayable_dependent(self) -> bool {
        matches!(
            self,
            Self::Replayable {
                database_evidence:
                    GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence::Dependent(_),
                ..
            }
        )
    }

    fn is_replayable_new_pivot(self) -> bool {
        matches!(
            self,
            Self::Replayable {
                database_evidence:
                    GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence::NewPivot(_),
                ..
            }
        )
    }

    fn is_publication(self) -> bool {
        matches!(self, Self::Publication { .. })
    }
}

impl GeneratedAffineResidualGroupExactSessionEventHead {
    fn reduction_count(&self) -> usize {
        match self {
            Self::Replayable {
                database_evidence, ..
            } => database_evidence.reduction_count(),
            Self::Publication { .. } => 0,
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionEventHead {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Replayable { .. } => formatter.write_str("Replayable(<redacted>)"),
            Self::Publication { pivot_ordinal } => formatter
                .debug_struct("Publication")
                .field("pivot_ordinal", pivot_ordinal)
                .finish(),
        }
    }
}

enum GeneratedAffineResidualGroupExactSessionEventDisposition {
    Dependent,
    NoTarget {
        target_offset: Arc<ExactTargetOffset>,
        stats: GeneratedAffineResidualGroupExactSessionRecenterStats,
    },
    RequiresAffineEqualityRefinement {
        target_offset: Arc<ExactTargetOffset>,
        locator: GeneratedAffineResidualGroupSolveTargetLocator,
        equality_predicate_ordinals: Vec<usize>,
        stats: GeneratedAffineResidualGroupExactSessionRecenterStats,
    },
    Publication {
        target_offset: Arc<ExactTargetOffset>,
        locator: GeneratedAffineResidualGroupSolveTargetLocator,
        row: ExactRecenteredApplicationRow,
        publication: PublicationPayload,
    },
    #[cfg(test)]
    TestSeedPivot,
}

/// Borrow-only sizing/classification view used while the exact prepared
/// publication must remain intact for transactional error recovery.
#[derive(Clone, Copy)]
enum GeneratedAffineResidualGroupExactSessionEventDispositionView<'a> {
    Dependent,
    NoTarget {
        target_offset: &'a ExactTargetOffset,
    },
    RequiresAffineEqualityRefinement {
        target_offset: &'a ExactTargetOffset,
        equality_predicate_ordinals: &'a [usize],
        equality_predicate_capacity: usize,
    },
    Publication {
        target_offset: &'a ExactTargetOffset,
        row: &'a ExactRecenteredRow,
        publication: &'a PublicationPayload,
    },
    #[cfg(test)]
    TestSeedPivot,
}

impl GeneratedAffineResidualGroupExactSessionEventDisposition {
    fn view(&self) -> GeneratedAffineResidualGroupExactSessionEventDispositionView<'_> {
        match self {
            Self::Dependent => {
                GeneratedAffineResidualGroupExactSessionEventDispositionView::Dependent
            }
            Self::NoTarget { target_offset, .. } => {
                GeneratedAffineResidualGroupExactSessionEventDispositionView::NoTarget {
                    target_offset,
                }
            }
            Self::RequiresAffineEqualityRefinement {
                target_offset,
                equality_predicate_ordinals,
                ..
            } => GeneratedAffineResidualGroupExactSessionEventDispositionView::RequiresAffineEqualityRefinement {
                target_offset,
                equality_predicate_ordinals,
                equality_predicate_capacity: equality_predicate_ordinals.capacity(),
            },
            Self::Publication { .. } => {
                unreachable!("compact publication uses its dedicated borrowed preflight view")
            }
            #[cfg(test)]
            Self::TestSeedPivot => {
                GeneratedAffineResidualGroupExactSessionEventDispositionView::TestSeedPivot
            }
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionEventDisposition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dependent => formatter.write_str("Dependent"),
            Self::NoTarget { stats, .. } => formatter
                .debug_struct("NoTarget")
                .field("stats", stats)
                .field("private_target_offset", &"<redacted>")
                .finish(),
            Self::RequiresAffineEqualityRefinement { locator, stats, .. } => formatter
                .debug_struct("RequiresAffineEqualityRefinement")
                .field("locator", locator)
                .field("stats", stats)
                .field("private_target_offset", &"<redacted>")
                .field("private_equality_predicates", &"<redacted>")
                .finish(),
            Self::Publication {
                locator,
                publication,
                ..
            } => formatter
                .debug_struct("Publication")
                .field("locator", locator)
                .field("publication", publication)
                .field("private_target_offset", &"<redacted>")
                .field("private_row", &"<redacted>")
                .finish(),
            #[cfg(test)]
            Self::TestSeedPivot => formatter.write_str("TestSeedPivot"),
        }
    }
}

/// Immutable chronological state for one exact commit transition.
///
/// Replayable algebraic variants retain their exact source/evidence
/// allocations. The compact application variant retains only its installed
/// pivot ordinal and application payload. The type is non-Clone; receipts and
/// suspended owners may share only its exact `Arc`.
struct GeneratedAffineResidualGroupExactSessionEvent {
    authority: Arc<GeneratedAffineResidualGroupExactSessionEventAuthority>,
    event_ordinal: usize,
    source_ordinal: usize,
    predecessor_state_version: usize,
    successor_state_version: usize,
    head: GeneratedAffineResidualGroupExactSessionEventHead,
    disposition: GeneratedAffineResidualGroupExactSessionEventDisposition,
    retained_bytes: usize,
}

impl GeneratedAffineResidualGroupExactSessionEvent {
    fn reduction_count(&self) -> usize {
        self.head.reduction_count()
    }

    fn has_production_source(&self) -> bool {
        match &self.head {
            GeneratedAffineResidualGroupExactSessionEventHead::Replayable {
                source_recipe, ..
            } => source_recipe.has_production_source(),
            GeneratedAffineResidualGroupExactSessionEventHead::Publication { .. } => false,
        }
    }

    fn account_replay_work(
        &self,
        target_count: usize,
        work: &mut GeneratedAffineResidualGroupExactSessionReplayWork,
        limits: GeneratedAffineResidualGroupExactSessionEventLimits,
    ) -> Result<(), GeneratedAffineResidualGroupExactSessionError> {
        work.events = session_event_bounded_add(
            "exact session replay events",
            work.events,
            1,
            limits.max_replay_events,
        )?;
        work.reduction_steps = session_event_bounded_add(
            "exact session replay reduction steps",
            work.reduction_steps,
            self.reduction_count(),
            limits.max_replay_reduction_steps,
        )?;
        work.target_state_successor_copies = session_event_bounded_add(
            "exact session replay target-state successor copies",
            work.target_state_successor_copies,
            target_count,
            limits.max_replay_target_state_successor_copies,
        )?;
        work.ledger_arc_copies = session_event_bounded_add(
            "exact session replay ledger Arc copies",
            work.ledger_arc_copies,
            self.event_ordinal,
            limits.max_replay_ledger_arc_copies,
        )?;
        if let GeneratedAffineResidualGroupExactSessionEventHead::Replayable {
            database_evidence:
                GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence::NewPivot(pivot),
            ..
        } = &self.head
        {
            work.pivot_terms = session_event_bounded_add(
                "exact session replay pivot terms",
                work.pivot_terms,
                pivot.terms().len(),
                limits.max_replay_pivot_terms,
            )?;
            work.pivot_guards = session_event_bounded_add(
                "exact session replay pivot guards",
                work.pivot_guards,
                pivot.guards().len(),
                limits.max_replay_pivot_guards,
            )?;
        }
        match &self.disposition {
            GeneratedAffineResidualGroupExactSessionEventDisposition::Dependent => {}
            GeneratedAffineResidualGroupExactSessionEventDisposition::NoTarget {
                target_offset,
                ..
            } => {
                work.target_offset_components = session_event_bounded_add(
                    "exact session replay target-offset components",
                    work.target_offset_components,
                    target_offset.values().len(),
                    limits.max_replay_target_offset_components,
                )?;
                work.target_scans = session_event_bounded_add(
                    "exact session replay target scans",
                    work.target_scans,
                    target_count,
                    limits.max_replay_target_scans,
                )?;
            }
            GeneratedAffineResidualGroupExactSessionEventDisposition::RequiresAffineEqualityRefinement {
                target_offset,
                equality_predicate_ordinals,
                ..
            } => {
                work.target_offset_components = session_event_bounded_add(
                    "exact session replay target-offset components",
                    work.target_offset_components,
                    target_offset.values().len(),
                    limits.max_replay_target_offset_components,
                )?;
                work.equality_predicates = session_event_bounded_add(
                    "exact session replay equality predicates",
                    work.equality_predicates,
                    equality_predicate_ordinals.len(),
                    limits.max_replay_equality_predicates,
                )?;
                work.target_scans = session_event_bounded_add(
                    "exact session replay target scans",
                    work.target_scans,
                    target_count,
                    limits.max_replay_target_scans,
                )?;
            }
            GeneratedAffineResidualGroupExactSessionEventDisposition::Publication { .. } => {
                unreachable!("compact publication is rejected before replay accounting")
            }
            #[cfg(test)]
            GeneratedAffineResidualGroupExactSessionEventDisposition::TestSeedPivot => {}
        }
        Ok(())
    }

    fn semantically_equal(&self, other: &Self) -> bool {
        if self.event_ordinal != other.event_ordinal
            || self.source_ordinal != other.source_ordinal
            || self.predecessor_state_version != other.predecessor_state_version
            || self.successor_state_version != other.successor_state_version
            || self.retained_bytes != other.retained_bytes
        {
            return false;
        }
        let head_equal = match (&self.head, &other.head) {
            (
                GeneratedAffineResidualGroupExactSessionEventHead::Replayable {
                    source_recipe: left_recipe,
                    database_evidence:
                        GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence::Dependent(left),
                },
                GeneratedAffineResidualGroupExactSessionEventHead::Replayable {
                    source_recipe: right_recipe,
                    database_evidence:
                        GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence::Dependent(right),
                },
            ) => left_recipe.same_source_allocation(right_recipe) && left.structurally_equal(right),
            (
                GeneratedAffineResidualGroupExactSessionEventHead::Replayable {
                    source_recipe: left_recipe,
                    database_evidence:
                        GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence::NewPivot(left),
                },
                GeneratedAffineResidualGroupExactSessionEventHead::Replayable {
                    source_recipe: right_recipe,
                    database_evidence:
                        GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence::NewPivot(right),
                },
            ) => left_recipe.same_source_allocation(right_recipe) && left.structurally_equal(right),
            (
                GeneratedAffineResidualGroupExactSessionEventHead::Publication {
                    pivot_ordinal: left,
                },
                GeneratedAffineResidualGroupExactSessionEventHead::Publication {
                    pivot_ordinal: right,
                },
            ) => left == right,
            _ => false,
        };
        if !head_equal {
            return false;
        }
        match (&self.disposition, &other.disposition) {
            (
                GeneratedAffineResidualGroupExactSessionEventDisposition::Dependent,
                GeneratedAffineResidualGroupExactSessionEventDisposition::Dependent,
            ) => true,
            (
                GeneratedAffineResidualGroupExactSessionEventDisposition::NoTarget {
                    target_offset: left_offset,
                    stats: left_stats,
                },
                GeneratedAffineResidualGroupExactSessionEventDisposition::NoTarget {
                    target_offset: right_offset,
                    stats: right_stats,
                },
            ) => left_offset.values() == right_offset.values() && left_stats == right_stats,
            (
                GeneratedAffineResidualGroupExactSessionEventDisposition::RequiresAffineEqualityRefinement {
                    target_offset: left_offset,
                    locator: left_locator,
                    equality_predicate_ordinals: left_predicates,
                    stats: left_stats,
                },
                GeneratedAffineResidualGroupExactSessionEventDisposition::RequiresAffineEqualityRefinement {
                    target_offset: right_offset,
                    locator: right_locator,
                    equality_predicate_ordinals: right_predicates,
                    stats: right_stats,
                },
            ) => {
                left_offset.values() == right_offset.values()
                    && left_locator == right_locator
                    && left_predicates == right_predicates
                    && left_stats == right_stats
            }
            #[cfg(test)]
            (
                GeneratedAffineResidualGroupExactSessionEventDisposition::TestSeedPivot,
                GeneratedAffineResidualGroupExactSessionEventDisposition::TestSeedPivot,
            ) => true,
            _ => false,
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionEvent")
            .field("event_ordinal", &self.event_ordinal)
            .field("source_ordinal", &self.source_ordinal)
            .field("predecessor_state_version", &self.predecessor_state_version)
            .field("successor_state_version", &self.successor_state_version)
            .field("reduction_count", &self.reduction_count())
            .field("retained_bytes", &self.retained_bytes)
            .field("disposition", &self.disposition)
            .field("head", &self.head)
            .field("private_authority", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct GeneratedAffineResidualGroupExactSessionReplayWork {
    events: usize,
    reduction_steps: usize,
    pivot_terms: usize,
    pivot_guards: usize,
    target_offset_components: usize,
    equality_predicates: usize,
    target_scans: usize,
    target_state_successor_copies: usize,
    ledger_arc_copies: usize,
}

fn session_event_checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactSessionError> {
    left.checked_add(right)
        .ok_or(GeneratedAffineResidualGroupExactSessionError::EventCountOverflow { resource })
}

fn session_event_checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactSessionError> {
    left.checked_mul(right)
        .ok_or(GeneratedAffineResidualGroupExactSessionError::EventCountOverflow { resource })
}

fn session_event_checked_sub(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactSessionError> {
    left.checked_sub(right)
        .ok_or(GeneratedAffineResidualGroupExactSessionError::EventCountOverflow { resource })
}

fn session_event_check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineResidualGroupExactSessionError> {
    if requested > limit {
        return Err(
            GeneratedAffineResidualGroupExactSessionError::EventResourceLimit {
                resource,
                requested,
                limit,
            },
        );
    }
    Ok(())
}

fn session_event_bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactSessionError> {
    let requested = session_event_checked_add(resource, left, right)?;
    session_event_check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn session_event_saturating_sum(values: impl IntoIterator<Item = usize>) -> usize {
    values
        .into_iter()
        .fold(0usize, |total, value| total.saturating_add(value))
}

fn session_event_arc_retained_bytes<T>()
-> Result<usize, GeneratedAffineResidualGroupExactSessionError> {
    session_event_checked_add(
        "exact session Arc owner retained bytes",
        session_event_checked_mul(
            "exact session Arc owner retained bytes",
            2,
            size_of::<usize>(),
        )?,
        size_of::<T>(),
    )
}

fn session_event_outer_buffer_bytes(
    capacity: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactSessionError> {
    session_event_checked_mul(
        "exact session event-ledger outer buffer bytes",
        capacity,
        size_of::<Arc<GeneratedAffineResidualGroupExactSessionEvent>>(),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactSessionError {
    Database(GeneratedAffineResidualGroupExactDatabaseError),
    Target(GeneratedAffineResidualGroupExactTargetError),
    WrongTargetStateAllocation,
    GeometryAuthentication,
    GeometryCountOverflow,
    MalformedGeometry,
    EventResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    EventCountOverflow {
        resource: &'static str,
    },
    EventAllocationFailure {
        resource: &'static str,
    },
    ReplayMismatch,
    SymbolicaPanic,
}

impl GeneratedAffineResidualGroupExactSessionError {
    const fn kind(self) -> &'static str {
        match self {
            Self::Database(_) => "Database",
            Self::Target(_) => "Target",
            Self::WrongTargetStateAllocation => "WrongTargetStateAllocation",
            Self::GeometryAuthentication => "GeometryAuthentication",
            Self::GeometryCountOverflow => "GeometryCountOverflow",
            Self::MalformedGeometry => "MalformedGeometry",
            Self::EventResourceLimit { .. } => "EventResourceLimit",
            Self::EventCountOverflow { .. } => "EventCountOverflow",
            Self::EventAllocationFailure { .. } => "EventAllocationFailure",
            Self::ReplayMismatch => "ReplayMismatch",
            Self::SymbolicaPanic => "SymbolicaPanic",
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionError")
            .field("kind", &self.kind())
            .field("private_detail", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualGroupExactSessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Database(_) => "exact session database operation failed",
            Self::Target(_) => "exact session target operation failed",
            Self::WrongTargetStateAllocation => {
                "exact session transaction belongs to another target-state allocation"
            }
            Self::GeometryAuthentication => "exact session affine geometry authentication failed",
            Self::GeometryCountOverflow => "exact session affine geometry size overflowed",
            Self::MalformedGeometry => "exact session affine geometry is malformed",
            Self::EventResourceLimit { .. } => {
                "exact session chronological event resource limit exceeded"
            }
            Self::EventCountOverflow { .. } => {
                "exact session chronological event accounting overflowed"
            }
            Self::EventAllocationFailure { .. } => {
                "exact session chronological event allocation failed"
            }
            Self::ReplayMismatch => "exact session retained allocation replay mismatch",
            Self::SymbolicaPanic => "Symbolica panicked inside the exact session boundary",
        })
    }
}

impl std::error::Error for GeneratedAffineResidualGroupExactSessionError {}

impl From<GeneratedAffineResidualGroupExactDatabaseError>
    for GeneratedAffineResidualGroupExactSessionError
{
    fn from(error: GeneratedAffineResidualGroupExactDatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<GeneratedAffineResidualGroupExactTargetError>
    for GeneratedAffineResidualGroupExactSessionError
{
    fn from(error: GeneratedAffineResidualGroupExactTargetError) -> Self {
        Self::Target(error)
    }
}

/// Failure kind for a transaction-preserving session recenter attempt.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactSessionRecenterError {
    Session(GeneratedAffineResidualGroupExactSessionError),
    Kernel(ExactRecenterKernelError),
    PhysicalKey(GeneratedAffineResidualGroupPhysicalKeyError),
}

impl GeneratedAffineResidualGroupExactSessionRecenterError {
    const fn kind(self) -> &'static str {
        match self {
            Self::Session(_) => "Session",
            Self::Kernel(_) => "Kernel",
            Self::PhysicalKey(_) => "PhysicalKey",
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionRecenterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionRecenterError")
            .field("kind", &self.kind())
            .field("private_detail", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualGroupExactSessionRecenterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Session(_) => "exact session recenter authentication failed",
            Self::Kernel(_) => "exact session recenter arithmetic failed",
            Self::PhysicalKey(_) => "exact session recenter target geometry failed",
        })
    }
}

impl std::error::Error for GeneratedAffineResidualGroupExactSessionRecenterError {}

impl From<GeneratedAffineResidualGroupExactSessionError>
    for GeneratedAffineResidualGroupExactSessionRecenterError
{
    fn from(error: GeneratedAffineResidualGroupExactSessionError) -> Self {
        Self::Session(error)
    }
}

impl From<ExactRecenterKernelError> for GeneratedAffineResidualGroupExactSessionRecenterError {
    fn from(error: ExactRecenterKernelError) -> Self {
        Self::Kernel(error)
    }
}

impl From<GeneratedAffineResidualGroupPhysicalKeyError>
    for GeneratedAffineResidualGroupExactSessionRecenterError
{
    fn from(error: GeneratedAffineResidualGroupPhysicalKeyError) -> Self {
        Self::PhysicalKey(error)
    }
}

/// Recenter failure that returns the exact consume-once transaction.
pub(crate) struct GeneratedAffineResidualGroupExactSessionRecenterFailure {
    error: GeneratedAffineResidualGroupExactSessionRecenterError,
    transaction: GeneratedAffineResidualGroupExactSessionStagedTransaction,
}

impl GeneratedAffineResidualGroupExactSessionRecenterFailure {
    pub(crate) const fn error(&self) -> GeneratedAffineResidualGroupExactSessionRecenterError {
        self.error
    }

    pub(crate) fn into_transaction(
        self,
    ) -> GeneratedAffineResidualGroupExactSessionStagedTransaction {
        self.transaction
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionRecenterFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionRecenterFailure")
            .field("error", &self.error)
            .field("private_transaction", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualGroupExactSessionRecenterFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("exact session recentering failed before classification")
    }
}

impl std::error::Error for GeneratedAffineResidualGroupExactSessionRecenterFailure {}

/// Owning, non-forgeable result of matching one staged post-reduction pivot.
///
/// Every branch keeps the consume-once transaction private. Matching never
/// consumes a target, and the Ready branch deliberately has no commit or
/// transaction-extraction method: a later `WhenBad` classifier must refine it
/// into an authorized transition.
pub(crate) enum GeneratedAffineResidualGroupExactSessionRecenterOutcome {
    NoTarget(GeneratedAffineResidualGroupExactSessionRecenterNoTarget),
    RequiresAffineEqualityRefinement(
        GeneratedAffineResidualGroupExactSessionRecenterRequiresAffineEqualityRefinement,
    ),
    Ready(GeneratedAffineResidualGroupExactSessionRecenterReady),
}

impl GeneratedAffineResidualGroupExactSessionRecenterOutcome {
    pub(crate) const fn stats(&self) -> GeneratedAffineResidualGroupExactSessionRecenterStats {
        match self {
            Self::NoTarget(outcome) => outcome.stats(),
            Self::RequiresAffineEqualityRefinement(outcome) => outcome.stats(),
            Self::Ready(outcome) => outcome.stats(),
        }
    }

    pub(crate) const fn source_ordinal(&self) -> usize {
        match self {
            Self::NoTarget(outcome) => outcome.source_ordinal(),
            Self::RequiresAffineEqualityRefinement(outcome) => outcome.source_ordinal(),
            Self::Ready(outcome) => outcome.source_ordinal(),
        }
    }

    pub(crate) const fn pivot_ordinal(&self) -> usize {
        match self {
            Self::NoTarget(outcome) => outcome.pivot_ordinal(),
            Self::RequiresAffineEqualityRefinement(outcome) => outcome.pivot_ordinal(),
            Self::Ready(outcome) => outcome.pivot_ordinal(),
        }
    }

    pub(crate) const fn targets_consumed(&self) -> usize {
        0
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionRecenterOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoTarget(outcome) => outcome.fmt(formatter),
            Self::RequiresAffineEqualityRefinement(outcome) => outcome.fmt(formatter),
            Self::Ready(outcome) => outcome.fmt(formatter),
        }
    }
}

pub(crate) struct GeneratedAffineResidualGroupExactSessionRecenterNoTarget {
    transaction: GeneratedAffineResidualGroupExactSessionStagedTransaction,
    target_offset: Arc<ExactTargetOffset>,
    source_ordinal: usize,
    pivot_ordinal: usize,
    stats: GeneratedAffineResidualGroupExactSessionRecenterStats,
}

impl GeneratedAffineResidualGroupExactSessionRecenterNoTarget {
    pub(crate) const fn stats(&self) -> GeneratedAffineResidualGroupExactSessionRecenterStats {
        self.stats
    }

    pub(crate) const fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }

    pub(crate) const fn pivot_ordinal(&self) -> usize {
        self.pivot_ordinal
    }

    pub(crate) const fn targets_consumed(&self) -> usize {
        0
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionRecenterNoTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionRecenterNoTarget")
            .field("source_ordinal", &self.source_ordinal)
            .field("pivot_ordinal", &self.pivot_ordinal)
            .field("stats", &self.stats)
            .field("targets_consumed", &0)
            .field("private_transaction", &"<redacted>")
            .field("private_target_offset", &"<redacted>")
            .finish()
    }
}

pub(crate) struct GeneratedAffineResidualGroupExactSessionRecenterRequiresAffineEqualityRefinement {
    transaction: GeneratedAffineResidualGroupExactSessionStagedTransaction,
    target: GeneratedAffineResidualGroupRetainedEqualityRefinementExactTarget,
    target_offset: Arc<ExactTargetOffset>,
    source_ordinal: usize,
    pivot_ordinal: usize,
    stats: GeneratedAffineResidualGroupExactSessionRecenterStats,
}

impl GeneratedAffineResidualGroupExactSessionRecenterRequiresAffineEqualityRefinement {
    pub(crate) const fn stats(&self) -> GeneratedAffineResidualGroupExactSessionRecenterStats {
        self.stats
    }

    pub(crate) const fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }

    pub(crate) const fn pivot_ordinal(&self) -> usize {
        self.pivot_ordinal
    }

    pub(crate) fn target_locator(&self) -> &GeneratedAffineResidualGroupSolveTargetLocator {
        self.target.locator()
    }

    pub(crate) fn refinement(&self) -> &GeneratedAffineResidualCaseEqualityRefinementCertificate {
        self.target.refinement()
    }

    pub(crate) const fn targets_consumed(&self) -> usize {
        0
    }
}

impl fmt::Debug
    for GeneratedAffineResidualGroupExactSessionRecenterRequiresAffineEqualityRefinement
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct(
                "GeneratedAffineResidualGroupExactSessionRecenterRequiresAffineEqualityRefinement",
            )
            .field("source_ordinal", &self.source_ordinal)
            .field("pivot_ordinal", &self.pivot_ordinal)
            .field("target_solve_ordinal", &self.target.solve_ordinal())
            .field("stats", &self.stats)
            .field("targets_consumed", &0)
            .field("private_transaction", &"<redacted>")
            .field("private_target", &"<redacted>")
            .field("private_target_offset", &"<redacted>")
            .finish()
    }
}

pub(crate) struct GeneratedAffineResidualGroupExactSessionRecenterReady {
    transaction: GeneratedAffineResidualGroupExactSessionStagedTransaction,
    target: GeneratedAffineResidualGroupRetainedReadyExactTarget,
    target_offset: Arc<ExactTargetOffset>,
    recentered: ExactRecenteredRow,
    source_ordinal: usize,
    pivot_ordinal: usize,
    stats: GeneratedAffineResidualGroupExactSessionRecenterStats,
}

/// Allocation-free, borrow-only geometry admitted for exact Ready analysis.
///
/// This view can be minted only after the session has jointly reauthenticated
/// the staged database transaction, unresolved-target state, selected target,
/// and exact matched anchor.  It intentionally exposes neither an owning plan
/// nor a transaction-extraction seam; a caller may only inspect the exact
/// geometry and feed it to current-lineage analysis while the session and
/// Ready token remain borrowed.
pub(crate) struct GeneratedAffineResidualGroupExactSessionReadyGeometryView<'authority> {
    frame: &'authority Arc<GeneratedAffineResidualGroupPhysicalFrame>,
    group: GeneratedAffineResidualInventoryGroupSourceView<'authority>,
    locator: GeneratedAffineResidualGroupSolveTargetLocator,
    target_anchor: &'authority GeneratedAffineResidualGroupLatticeShift,
    target_offset: &'authority [Integer],
}

impl<'authority> GeneratedAffineResidualGroupExactSessionReadyGeometryView<'authority> {
    pub(crate) const fn frame(&self) -> &'authority Arc<GeneratedAffineResidualGroupPhysicalFrame> {
        self.frame
    }

    pub(crate) const fn locator(&self) -> GeneratedAffineResidualGroupSolveTargetLocator {
        self.locator
    }

    pub(crate) const fn ambient_arity(&self) -> usize {
        self.group.ambient_arity()
    }

    pub(crate) const fn free_positions(&self) -> &'authority [usize] {
        self.group.free_positions()
    }

    /// Row-major `ambient_arity * free_positions().len()` exact matrix.
    pub(crate) const fn compact_affine_matrix(&self) -> &'authority [Integer] {
        self.group.compact_linear_coefficients()
    }

    pub(crate) const fn target_anchor(
        &self,
    ) -> &'authority GeneratedAffineResidualGroupLatticeShift {
        self.target_anchor
    }

    /// Exact affine translation `p - A p_F` retained by the sealed Ready
    /// token.  The session authenticates the offset's complete GMP/retained
    /// census immediately before minting this borrow; callers never
    /// reconstruct it from an anchor or coefficient translation.
    pub(crate) const fn target_offset(&self) -> &'authority [Integer] {
        self.target_offset
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionReadyGeometryView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionReadyGeometryView")
            .field("locator", &self.locator)
            .field("ambient_arity", &self.group.ambient_arity())
            .field("free_position_count", &self.group.free_positions().len())
            .field("private_frame", &"<redacted>")
            .field("private_geometry", &"<redacted>")
            .field("private_target_offset", &"<redacted>")
            .finish()
    }
}

impl GeneratedAffineResidualGroupExactSessionRecenterReady {
    pub(crate) const fn stats(&self) -> GeneratedAffineResidualGroupExactSessionRecenterStats {
        self.stats
    }

    pub(crate) const fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }

    pub(crate) const fn pivot_ordinal(&self) -> usize {
        self.pivot_ordinal
    }

    pub(crate) fn target_locator(&self) -> &GeneratedAffineResidualGroupSolveTargetLocator {
        self.target.locator()
    }

    /// Premises owned by the matched target, kept separate from translated
    /// row guards generated from the staged pivot.
    pub(crate) fn target_premises(&self) -> &[ParametricNonZeroCondition] {
        self.target.premises()
    }

    pub(crate) fn coefficient_translation(&self) -> &[Integer] {
        self.recentered.coefficient_translation().values()
    }

    pub(crate) fn terms(&self) -> &[ExactRecenteredTerm] {
        self.recentered.terms()
    }

    pub(crate) fn row_guards(&self) -> &[ParametricNonZeroCondition] {
        self.recentered.guards()
    }

    pub(crate) const fn targets_consumed(&self) -> usize {
        0
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionRecenterReady {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionRecenterReady")
            .field("source_ordinal", &self.source_ordinal)
            .field("pivot_ordinal", &self.pivot_ordinal)
            .field("target_solve_ordinal", &self.target.solve_ordinal())
            .field("target_premise_count", &self.target.premises().len())
            .field("term_count", &self.recentered.terms().len())
            .field("row_guard_count", &self.recentered.guards().len())
            .field("stats", &self.stats)
            .field("targets_consumed", &0)
            .field("private_transaction", &"<redacted>")
            .field("private_target", &"<redacted>")
            .field("private_target_offset", &"<redacted>")
            .field("private_recentered_row", &"<redacted>")
            .finish()
    }
}

/// Shallow owning handle for one committed compact publication event.
///
/// Cloning this handle clones one `Arc`, never the centered relation or its
/// relative partition.  A scheduler may therefore retain an event across
/// later mutable session epochs and borrow zero-copy projections only while it
/// inspects that event.
#[derive(Clone)]
pub(crate) struct CommittedPublicationEventHandle {
    event: Arc<GeneratedAffineResidualGroupExactSessionEvent>,
}

impl CommittedPublicationEventHandle {
    pub(crate) fn view(&self) -> CommittedPublicationEventView<'_> {
        CommittedPublicationEventView {
            event: self.event.as_ref(),
        }
    }
}

impl fmt::Debug for CommittedPublicationEventHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.view().fmt(formatter)
    }
}

/// Borrow-only view of one committed compact publication event.
///
/// The event allocation remains the sole deep payload owner, shared shallowly
/// by the session ledger and any retained event handle. This view performs no
/// replay, algebra, allocation, or target-state authentication: the atomic
/// commit has already established those in-process invariants. It is therefore
/// suitable for the later rule provider and residual scheduler without cloning
/// the centered row or relative partition.
#[derive(Clone, Copy)]
pub(crate) struct CommittedPublicationEventView<'session> {
    event: &'session GeneratedAffineResidualGroupExactSessionEvent,
}

impl<'session> CommittedPublicationEventView<'session> {
    fn from_event(event: &'session GeneratedAffineResidualGroupExactSessionEvent) -> Option<Self> {
        matches!(
            (&event.head, &event.disposition),
            (
                GeneratedAffineResidualGroupExactSessionEventHead::Publication { .. },
                GeneratedAffineResidualGroupExactSessionEventDisposition::Publication { .. }
            )
        )
        .then_some(Self { event })
    }

    fn publication_parts(
        self,
    ) -> (
        usize,
        &'session ExactTargetOffset,
        GeneratedAffineResidualGroupSolveTargetLocator,
        &'session ExactRecenteredApplicationRow,
        &'session PublicationPayload,
    ) {
        match (&self.event.head, &self.event.disposition) {
            (
                GeneratedAffineResidualGroupExactSessionEventHead::Publication { pivot_ordinal },
                GeneratedAffineResidualGroupExactSessionEventDisposition::Publication {
                    target_offset,
                    locator,
                    row,
                    publication,
                },
            ) => (*pivot_ordinal, target_offset, *locator, row, publication),
            _ => unreachable!("committed publication view lost its publication event"),
        }
    }

    pub(crate) const fn event_ordinal(self) -> usize {
        self.event.event_ordinal
    }

    pub(crate) const fn source_ordinal(self) -> usize {
        self.event.source_ordinal
    }

    pub(crate) fn pivot_ordinal(self) -> usize {
        self.publication_parts().0
    }

    pub(crate) fn target_locator(self) -> GeneratedAffineResidualGroupSolveTargetLocator {
        self.publication_parts().2
    }

    pub(crate) fn target_offset(self) -> &'session [Integer] {
        self.publication_parts().1.values()
    }

    pub(crate) fn terms(self) -> &'session [ExactRecenteredTerm] {
        self.publication_parts().3.terms()
    }

    pub(crate) fn pivot_term_ordinal(self) -> usize {
        self.publication_parts().3.pivot_term_ordinal()
    }

    pub(crate) fn target_premises(self) -> &'session [ParametricNonZeroCondition] {
        self.event
            .authority
            .catalog
            .ready_target_premises(self.target_locator())
            .expect("committed publication lost its immutable Ready-target premises")
    }

    pub(crate) fn ambient_arity(self) -> usize {
        self.event
            .authority
            .plan
            .authority()
            .retained_source_neutral_group_view()
            .expect("committed publication lost its sealed affine-group geometry")
            .ambient_arity()
    }

    pub(crate) fn free_positions(self) -> &'session [usize] {
        self.event
            .authority
            .plan
            .authority()
            .retained_source_neutral_group_view()
            .expect("committed publication lost its sealed affine-group geometry")
            .free_positions()
    }

    /// Row-major `ambient_arity * free_positions().len()` exact matrix.
    pub(crate) fn compact_affine_matrix(self) -> &'session [Integer] {
        self.event
            .authority
            .plan
            .authority()
            .retained_source_neutral_group_view()
            .expect("committed publication lost its sealed affine-group geometry")
            .compact_linear_coefficients()
    }

    pub(crate) fn database_epoch(self) -> usize {
        self.event.authority.database_epoch
    }

    pub(crate) fn group_ordinal(self) -> usize {
        self.event.authority.group_ordinal
    }

    pub(crate) fn family_fingerprint(self) -> &'session str {
        self.event.authority.plan.authority().family_fingerprint()
    }

    pub(crate) fn context_fingerprint(self) -> &'session str {
        self.event.authority.plan.authority().context_fingerprint()
    }

    pub(crate) fn sector(self) -> &'session SectorMask {
        self.event.authority.plan.authority().sector()
    }

    pub(crate) fn ordering(self) -> IntegralOrderingPolicy {
        self.event.authority.plan.authority().ordering()
    }

    fn loci(self) -> &'session [ParametricPolynomial] {
        self.publication_parts().4.loci()
    }

    fn cases(self) -> &'session [AffineWhenBadArbitraryRelativeCase] {
        self.publication_parts().4.cases()
    }

    #[cfg(test)]
    pub(crate) fn loci_for_test(self) -> &'session [ParametricPolynomial] {
        self.loci()
    }

    #[cfg(test)]
    pub(crate) fn cases_for_test(self) -> &'session [AffineWhenBadArbitraryRelativeCase] {
        self.cases()
    }

    pub(crate) fn leaf_count(self) -> usize {
        self.cases().len()
    }

    pub(crate) fn leaf(self, ordinal: usize) -> Option<CommittedPublicationLeafView<'session>> {
        let leaf = self.publication_parts().4.leaf(ordinal)?;
        Some(match leaf.disposition() {
            PublicationLeafDisposition::Applicable => {
                CommittedPublicationLeafView::Applicable(ApplicableRuleHandle { event: self, leaf })
            }
            PublicationLeafDisposition::ExceptionalDomain => {
                CommittedPublicationLeafView::Exceptional(ExceptionalResidualHandle {
                    event: self,
                    leaf,
                    kind: ExceptionalResidualKind::Domain,
                })
            }
            PublicationLeafDisposition::ExceptionalLeak => {
                CommittedPublicationLeafView::Exceptional(ExceptionalResidualHandle {
                    event: self,
                    leaf,
                    kind: ExceptionalResidualKind::SectorLeak,
                })
            }
        })
    }

    pub(crate) fn leaves(
        self,
    ) -> impl ExactSizeIterator<Item = CommittedPublicationLeafView<'session>> + 'session {
        (0..self.leaf_count()).map(move |ordinal| {
            self.leaf(ordinal)
                .expect("committed publication payload lengths diverged")
        })
    }

    pub(crate) fn applicable_rules(
        self,
    ) -> impl Iterator<Item = ApplicableRuleHandle<'session>> + 'session {
        self.leaves().filter_map(|leaf| match leaf {
            CommittedPublicationLeafView::Applicable(rule) => Some(rule),
            CommittedPublicationLeafView::Exceptional(_) => None,
        })
    }

    pub(crate) fn exceptional_residuals(
        self,
    ) -> impl Iterator<Item = ExceptionalResidualHandle<'session>> + 'session {
        self.leaves().filter_map(|leaf| match leaf {
            CommittedPublicationLeafView::Applicable(_) => None,
            CommittedPublicationLeafView::Exceptional(residual) => Some(residual),
        })
    }
}

impl fmt::Debug for CommittedPublicationEventView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommittedPublicationEventView")
            .field("event_ordinal", &self.event_ordinal())
            .field("source_ordinal", &self.source_ordinal())
            .field("pivot_ordinal", &self.pivot_ordinal())
            .field("pivot_term_ordinal", &self.pivot_term_ordinal())
            .field("target_locator", &self.target_locator())
            .field("target_premise_count", &self.target_premises().len())
            .field("ambient_arity", &self.ambient_arity())
            .field("free_position_count", &self.free_positions().len())
            .field("database_epoch", &self.database_epoch())
            .field("group_ordinal", &self.group_ordinal())
            .field("sector", &self.sector())
            .field("ordering", &self.ordering())
            .field("term_count", &self.terms().len())
            .field("locus_count", &self.loci().len())
            .field("leaf_count", &self.leaf_count())
            .field("private_payload", &"<borrowed>")
            .finish()
    }
}

/// One leaf of a committed event, classified exclusively as either an
/// applicable rule or exceptional residual. Exactly-once queue consumption is
/// a later scheduler responsibility; these inspection views are repeatable.
#[derive(Clone, Copy, Debug)]
pub(crate) enum CommittedPublicationLeafView<'session> {
    Applicable(ApplicableRuleHandle<'session>),
    Exceptional(ExceptionalResidualHandle<'session>),
}

/// One event-bound relative predicate with its table entry resolved.
///
/// The polynomial and predicate kind are minted together from the same
/// committed event, so a provider cannot accidentally index a case through a
/// different event's locus table.
#[derive(Clone, Copy)]
pub(crate) struct CommittedPublicationPredicateView<'session> {
    locus_ordinal: usize,
    kind: SymbolicPolynomialPredicateKind,
    polynomial: &'session ParametricPolynomial,
}

impl<'session> CommittedPublicationPredicateView<'session> {
    pub(crate) const fn locus_ordinal(self) -> usize {
        self.locus_ordinal
    }

    pub(crate) const fn kind(self) -> SymbolicPolynomialPredicateKind {
        self.kind
    }

    pub(crate) const fn polynomial(self) -> &'session ParametricPolynomial {
        self.polynomial
    }
}

impl fmt::Debug for CommittedPublicationPredicateView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommittedPublicationPredicateView")
            .field("locus_ordinal", &self.locus_ordinal)
            .field("kind", &self.kind)
            .field("private_polynomial", &"<borrowed>")
            .finish()
    }
}

/// Complete zero-copy domain of one committed publication leaf.
///
/// The leaf applies on the conjunction of `target_premises()` and every
/// resolved relative predicate returned by `predicates()`.  Keeping both
/// projections behind this event-bound view prevents a provider from silently
/// dropping the parent domain or pairing table indices with another event.
#[derive(Clone, Copy)]
pub(crate) struct CommittedPublicationDomainView<'session> {
    event: CommittedPublicationEventView<'session>,
    case: &'session AffineWhenBadArbitraryRelativeCase,
}

impl<'session> CommittedPublicationDomainView<'session> {
    pub(crate) const fn event(self) -> CommittedPublicationEventView<'session> {
        self.event
    }

    pub(crate) fn target_premises(self) -> &'session [ParametricNonZeroCondition] {
        self.event.target_premises()
    }

    pub(crate) const fn relative_case(self) -> &'session AffineWhenBadArbitraryRelativeCase {
        self.case
    }

    pub(crate) fn predicate_count(self) -> usize {
        self.case.predicates().len()
    }

    pub(crate) fn predicate(
        self,
        ordinal: usize,
    ) -> Option<CommittedPublicationPredicateView<'session>> {
        let predicate = *self.case.predicates().get(ordinal)?;
        self.resolve_predicate(predicate)
    }

    pub(crate) fn predicates(
        self,
    ) -> impl ExactSizeIterator<Item = CommittedPublicationPredicateView<'session>> + 'session {
        (0..self.predicate_count()).map(move |ordinal| {
            self.predicate(ordinal)
                .expect("committed publication predicate lost its event-local locus")
        })
    }

    fn resolve_predicate(
        self,
        predicate: AffineWhenBadArbitraryRelativePredicate,
    ) -> Option<CommittedPublicationPredicateView<'session>> {
        Some(CommittedPublicationPredicateView {
            locus_ordinal: predicate.locus_ordinal(),
            kind: predicate.kind(),
            polynomial: self.event.loci().get(predicate.locus_ordinal())?,
        })
    }
}

impl fmt::Debug for CommittedPublicationDomainView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommittedPublicationDomainView")
            .field("event_ordinal", &self.event.event_ordinal())
            .field("target_premise_count", &self.target_premises().len())
            .field("relative_predicate_count", &self.predicate_count())
            .field("private_domain", &"<borrowed>")
            .finish()
    }
}

/// Shallow handle for one relative case on which the committed relation is
/// applicable.
#[derive(Clone, Copy)]
pub(crate) struct ApplicableRuleHandle<'session> {
    event: CommittedPublicationEventView<'session>,
    leaf: PublicationLeaf<'session>,
}

impl<'session> ApplicableRuleHandle<'session> {
    pub(crate) const fn event(self) -> CommittedPublicationEventView<'session> {
        self.event
    }

    pub(crate) const fn leaf_ordinal(self) -> usize {
        self.leaf.ordinal()
    }

    /// Event-bound conjunction of the parent premises and this relative case.
    pub(crate) const fn domain(self) -> CommittedPublicationDomainView<'session> {
        CommittedPublicationDomainView {
            event: self.event,
            case: self.leaf.case(),
        }
    }
}

impl fmt::Debug for ApplicableRuleHandle<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicableRuleHandle")
            .field("event_ordinal", &self.event.event_ordinal())
            .field("leaf_ordinal", &self.leaf_ordinal())
            .field("private_case", &"<borrowed>")
            .finish()
    }
}

/// Algebra-free classification of one exceptional publication leaf.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExceptionalResidualKind {
    Domain,
    SectorLeak,
}

/// Shallow handle for one exact relative case that must return to the closure
/// scheduler instead of entering the applicable-rule provider.
#[derive(Clone, Copy)]
pub(crate) struct ExceptionalResidualHandle<'session> {
    event: CommittedPublicationEventView<'session>,
    leaf: PublicationLeaf<'session>,
    kind: ExceptionalResidualKind,
}

impl<'session> ExceptionalResidualHandle<'session> {
    pub(crate) const fn event(self) -> CommittedPublicationEventView<'session> {
        self.event
    }

    pub(crate) const fn leaf_ordinal(self) -> usize {
        self.leaf.ordinal()
    }

    pub(crate) const fn kind(self) -> ExceptionalResidualKind {
        self.kind
    }

    /// Event-bound conjunction to re-enter through the residual scheduler.
    pub(crate) const fn domain(self) -> CommittedPublicationDomainView<'session> {
        CommittedPublicationDomainView {
            event: self.event,
            case: self.leaf.case(),
        }
    }
}

impl fmt::Debug for ExceptionalResidualHandle<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExceptionalResidualHandle")
            .field("event_ordinal", &self.event.event_ordinal())
            .field("leaf_ordinal", &self.leaf_ordinal())
            .field("kind", &self.kind)
            .field("private_case", &"<borrowed>")
            .finish()
    }
}

/// Shallow receipt for one committed compact application event.
#[derive(Debug)]
pub(crate) struct PublicationReceipt {
    event_ordinal: usize,
    source_ordinal: usize,
    pivot_ordinal: usize,
    retained_event_bytes: usize,
    stats: PublicationStats,
    event: CommittedPublicationEventHandle,
}

impl PublicationReceipt {
    pub(crate) const fn event_ordinal(&self) -> usize {
        self.event_ordinal
    }

    pub(crate) const fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }

    pub(crate) const fn pivot_ordinal(&self) -> usize {
        self.pivot_ordinal
    }

    pub(crate) const fn retained_event_bytes(&self) -> usize {
        self.retained_event_bytes
    }

    pub(crate) const fn stats(&self) -> PublicationStats {
        self.stats
    }

    pub(crate) fn event(&self) -> CommittedPublicationEventView<'_> {
        self.event.view()
    }

    pub(crate) fn into_event_handle(self) -> CommittedPublicationEventHandle {
        self.event
    }
}

/// Transactional failure retaining the exact prepared publication.
#[derive(Debug)]
pub(crate) struct PublicationCommitFailure {
    error: GeneratedAffineResidualGroupExactSessionError,
    publication: PreparedPublication,
}

impl PublicationCommitFailure {
    pub(crate) const fn error(&self) -> GeneratedAffineResidualGroupExactSessionError {
        self.error
    }

    pub(crate) fn into_publication(self) -> PreparedPublication {
        self.publication
    }
}

/// Typed successful result of committing a recentered pivot that matched no
/// solve target.
pub(crate) struct GeneratedAffineResidualGroupExactSessionCommittedNoTarget {
    session: GeneratedAffineResidualGroupExactSession,
    event: Arc<GeneratedAffineResidualGroupExactSessionEvent>,
    source_ordinal: usize,
    pivot_ordinal: usize,
    stats: GeneratedAffineResidualGroupExactSessionRecenterStats,
}

impl GeneratedAffineResidualGroupExactSessionCommittedNoTarget {
    pub(crate) const fn database_epoch(&self) -> usize {
        self.session.database_epoch()
    }

    pub(crate) const fn group_ordinal(&self) -> usize {
        self.session.group_ordinal()
    }

    pub(crate) const fn state_version(&self) -> usize {
        self.session.state_version()
    }

    pub(crate) const fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }

    pub(crate) const fn pivot_ordinal(&self) -> usize {
        self.pivot_ordinal
    }

    pub(crate) const fn stats(&self) -> GeneratedAffineResidualGroupExactSessionRecenterStats {
        self.stats
    }

    pub(crate) const fn targets_consumed(&self) -> usize {
        0
    }

    pub(crate) const fn publishes_rule(&self) -> bool {
        false
    }

    pub(crate) const fn infers_master(&self) -> bool {
        false
    }

    /// Continue the same exact session only after the typed NoTarget commit
    /// has completed successfully.
    pub(crate) fn into_session(self) -> GeneratedAffineResidualGroupExactSession {
        self.session
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionCommittedNoTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionCommittedNoTarget")
            .field("database_epoch", &self.database_epoch())
            .field("group_ordinal", &self.group_ordinal())
            .field("state_version", &self.state_version())
            .field("source_ordinal", &self.source_ordinal)
            .field("pivot_ordinal", &self.pivot_ordinal)
            .field("stats", &self.stats)
            .field("targets_consumed", &0)
            .field("publishes_rule", &false)
            .field("infers_master", &false)
            .field("private_session", &"<redacted>")
            .field("private_event", &"<redacted>")
            .finish()
    }
}

/// Failure of the typed NoTarget commit path.
///
/// Every failure returns the complete sealed outcome, including its
/// consume-once transaction. The database transition is represented by an
/// owning prepared token and its final commit tail is infallible.
pub(crate) enum GeneratedAffineResidualGroupExactSessionCommitNoTargetFailure {
    Preflight {
        error: GeneratedAffineResidualGroupExactSessionError,
        session: GeneratedAffineResidualGroupExactSession,
        outcome: GeneratedAffineResidualGroupExactSessionRecenterNoTarget,
    },
}

impl GeneratedAffineResidualGroupExactSessionCommitNoTargetFailure {
    pub(crate) const fn error(&self) -> GeneratedAffineResidualGroupExactSessionError {
        match self {
            Self::Preflight { error, .. } => *error,
        }
    }

    pub(crate) fn into_recovery(
        self,
    ) -> Result<
        (
            GeneratedAffineResidualGroupExactSession,
            GeneratedAffineResidualGroupExactSessionRecenterNoTarget,
        ),
        GeneratedAffineResidualGroupExactSessionError,
    > {
        match self {
            Self::Preflight {
                session, outcome, ..
            } => Ok((session, outcome)),
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionCommitNoTargetFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionCommitNoTargetFailure")
            .field("phase", &"preflight")
            .field("error", &self.error())
            .field("private_session", &"<redacted>")
            .field("private_outcome", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualGroupExactSessionCommitNoTargetFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("exact NoTarget transition failed before commit")
    }
}

impl std::error::Error for GeneratedAffineResidualGroupExactSessionCommitNoTargetFailure {}

/// Sealed owner for a committed pivot whose matched target requires affine
/// equality refinement in a later solve epoch.
///
/// This type intentionally offers no session accessor, extraction, resume, or
/// staging method.  It keeps the committed session, successor-bound unresolved
/// target, and source replay recipe alive until a future refined-epoch
/// transition consumes the whole owner.
pub(crate) struct GeneratedAffineResidualGroupExactSessionSuspendedForRefinedEpoch {
    committed_session: GeneratedAffineResidualGroupExactSession,
    event: Arc<GeneratedAffineResidualGroupExactSessionEvent>,
    target: GeneratedAffineResidualGroupRetainedEqualityRefinementExactTarget,
    source_ordinal: usize,
    pivot_ordinal: usize,
    stats: GeneratedAffineResidualGroupExactSessionRecenterStats,
}

impl GeneratedAffineResidualGroupExactSessionSuspendedForRefinedEpoch {
    pub(crate) const fn database_epoch(&self) -> usize {
        self.committed_session.database_epoch()
    }

    pub(crate) const fn group_ordinal(&self) -> usize {
        self.committed_session.group_ordinal()
    }

    pub(crate) const fn state_version(&self) -> usize {
        self.committed_session.state_version()
    }

    pub(crate) const fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }

    pub(crate) const fn pivot_ordinal(&self) -> usize {
        self.pivot_ordinal
    }

    pub(crate) const fn stats(&self) -> GeneratedAffineResidualGroupExactSessionRecenterStats {
        self.stats
    }

    pub(crate) fn target_locator(&self) -> &GeneratedAffineResidualGroupSolveTargetLocator {
        self.target.locator()
    }

    pub(crate) fn refinement(&self) -> &GeneratedAffineResidualCaseEqualityRefinementCertificate {
        self.target.refinement()
    }

    pub(crate) fn has_production_source(&self) -> bool {
        self.event.has_production_source()
    }

    pub(crate) const fn targets_consumed(&self) -> usize {
        0
    }

    pub(crate) const fn publishes_rule(&self) -> bool {
        false
    }

    pub(crate) const fn infers_master(&self) -> bool {
        false
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedAffineResidualGroupExactSessionError> {
        self.committed_session.replay(family, context)?;
        let Some(last) = self.committed_session.events.last() else {
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
        };
        if !Arc::ptr_eq(last, &self.event)
            || !self
                .target
                .authenticates_source_state(&self.committed_session.target_state)
            || self.source_ordinal != self.event.source_ordinal
        {
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
        }
        let GeneratedAffineResidualGroupExactSessionEventDisposition::RequiresAffineEqualityRefinement {
            locator,
            equality_predicate_ordinals,
            stats,
            ..
        } = &self.event.disposition
        else {
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
        };
        let GeneratedAffineResidualGroupExactSessionEventHead::Replayable {
            database_evidence:
                GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence::NewPivot(pivot),
            ..
        } = &self.event.head
        else {
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
        };
        if self.pivot_ordinal != pivot.ordinal()
            || pivot.source_ordinal() != self.source_ordinal
            || self.stats != *stats
            || self.target.locator() != locator
            || self.target.refinement().equality_predicate_ordinals()
                != equality_predicate_ordinals.as_slice()
        {
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
        }
        Ok(())
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionSuspendedForRefinedEpoch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionSuspendedForRefinedEpoch")
            .field("database_epoch", &self.database_epoch())
            .field("group_ordinal", &self.group_ordinal())
            .field("state_version", &self.state_version())
            .field("source_ordinal", &self.source_ordinal)
            .field("pivot_ordinal", &self.pivot_ordinal)
            .field("target_solve_ordinal", &self.target.solve_ordinal())
            .field("has_production_source", &self.has_production_source())
            .field("stats", &self.stats)
            .field("targets_consumed", &0)
            .field("publishes_rule", &false)
            .field("infers_master", &false)
            .field("private_committed_session", &"<redacted>")
            .field("private_event", &"<redacted>")
            .field("private_successor_target", &"<redacted>")
            .finish()
    }
}

/// Failure of the consuming equality-refinement suspension transition.
///
/// Ownership is fully reconstructible as the original running session plus
/// the complete recenter outcome until the infallible prepared commit tail.
pub(crate) enum GeneratedAffineResidualGroupExactSessionSuspendRefinedEpochFailure {
    Preflight {
        error: GeneratedAffineResidualGroupExactSessionError,
        session: GeneratedAffineResidualGroupExactSession,
        outcome: GeneratedAffineResidualGroupExactSessionRecenterRequiresAffineEqualityRefinement,
    },
}

impl GeneratedAffineResidualGroupExactSessionSuspendRefinedEpochFailure {
    pub(crate) const fn error(&self) -> GeneratedAffineResidualGroupExactSessionError {
        match self {
            Self::Preflight { error, .. } => *error,
        }
    }

    pub(crate) fn into_recovery(
        self,
    ) -> Result<
        (
            GeneratedAffineResidualGroupExactSession,
            GeneratedAffineResidualGroupExactSessionRecenterRequiresAffineEqualityRefinement,
        ),
        GeneratedAffineResidualGroupExactSessionError,
    > {
        match self {
            Self::Preflight {
                session, outcome, ..
            } => Ok((session, outcome)),
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionSuspendRefinedEpochFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionSuspendRefinedEpochFailure")
            .field("phase", &"preflight")
            .field("error", &self.error())
            .field("private_session", &"<redacted>")
            .field("private_outcome", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualGroupExactSessionSuspendRefinedEpochFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("exact equality-refinement suspension failed before commit")
    }
}

impl std::error::Error for GeneratedAffineResidualGroupExactSessionSuspendRefinedEpochFailure {}

struct PreparedSessionUnconsumedTransition {
    successor: Arc<GeneratedAffineResidualGroupExactTargetState>,
    transaction_target_state: Arc<GeneratedAffineResidualGroupExactTargetState>,
    database_commit: GeneratedAffineResidualGroupPreparedExactRowCommit,
    event: Arc<GeneratedAffineResidualGroupExactSessionEvent>,
    replacement_events: Vec<Arc<GeneratedAffineResidualGroupExactSessionEvent>>,
    event_stats: GeneratedAffineResidualGroupExactSessionEventStats,
}

struct PreparedSessionPublicationTransition {
    successor: Arc<GeneratedAffineResidualGroupExactTargetState>,
    ledger: PreparedSessionEventLedgerReplacement,
    source_ordinal: usize,
    pivot_ordinal: usize,
}

struct PreparedSessionEventLedgerReplacement {
    event_ordinal: usize,
    predecessor_state_version: usize,
    successor_state_version: usize,
    individual_event_retained_bytes: usize,
    replacement_events: Vec<Arc<GeneratedAffineResidualGroupExactSessionEvent>>,
    event_stats: GeneratedAffineResidualGroupExactSessionEventStats,
}

struct PreparedSessionEqualityRefinementSuspension {
    successor: Arc<GeneratedAffineResidualGroupExactTargetState>,
    target: GeneratedAffineResidualGroupRetainedEqualityRefinementExactTarget,
    locator: GeneratedAffineResidualGroupSolveTargetLocator,
    equality_predicate_ordinals: Vec<usize>,
}

struct GeneratedAffineResidualGroupExactSessionEventPreparationFailure {
    error: GeneratedAffineResidualGroupExactSessionError,
    transaction: GeneratedAffineResidualGroupExactSessionStagedTransaction,
}

enum ExpectedSessionEventDatabaseOutcome {
    Dependent { reduction_count: usize },
    NewPivot { pivot_ordinal: usize },
}

enum PreparedSessionRecenter {
    NoTarget {
        target_offset: Arc<ExactTargetOffset>,
        source_ordinal: usize,
        pivot_ordinal: usize,
        stats: GeneratedAffineResidualGroupExactSessionRecenterStats,
    },
    RequiresAffineEqualityRefinement {
        target: GeneratedAffineResidualGroupRetainedEqualityRefinementExactTarget,
        target_offset: Arc<ExactTargetOffset>,
        source_ordinal: usize,
        pivot_ordinal: usize,
        stats: GeneratedAffineResidualGroupExactSessionRecenterStats,
    },
    Ready {
        target: GeneratedAffineResidualGroupRetainedReadyExactTarget,
        target_offset: Arc<ExactTargetOffset>,
        recentered: ExactRecenteredRow,
        source_ordinal: usize,
        pivot_ordinal: usize,
        stats: GeneratedAffineResidualGroupExactSessionRecenterStats,
    },
}

/// Failure of an unconsumed session transition.
///
/// Every error before the infallible prepared database commit returns the
/// complete sealed transaction, so a caller may drop or retry it without
/// reconstructing authority.
enum GeneratedAffineResidualGroupExactSessionCommitUnconsumedFailure {
    Preflight {
        error: GeneratedAffineResidualGroupExactSessionError,
        transaction: GeneratedAffineResidualGroupExactSessionStagedTransaction,
    },
}

impl GeneratedAffineResidualGroupExactSessionCommitUnconsumedFailure {
    const fn error(&self) -> GeneratedAffineResidualGroupExactSessionError {
        match self {
            Self::Preflight { error, .. } => *error,
        }
    }

    fn into_transaction(
        self,
    ) -> Result<
        GeneratedAffineResidualGroupExactSessionStagedTransaction,
        GeneratedAffineResidualGroupExactSessionError,
    > {
        match self {
            Self::Preflight { transaction, .. } => Ok(transaction),
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionCommitUnconsumedFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionCommitUnconsumedFailure")
            .field("phase", &"preflight")
            .field("error", &self.error())
            .field("private_transaction", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualGroupExactSessionCommitUnconsumedFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("exact unconsumed session transition failed before commit")
    }
}

impl std::error::Error for GeneratedAffineResidualGroupExactSessionCommitUnconsumedFailure {}

/// Recoverable failure to seal a staged transaction as dependent.
///
/// Classification is read-only. Every failure therefore returns the exact
/// consume-once transaction supplied by the caller; no database or target
/// state has changed.
pub(crate) struct GeneratedAffineResidualGroupExactSessionDependentClassificationFailure {
    error: GeneratedAffineResidualGroupExactSessionError,
    transaction: GeneratedAffineResidualGroupExactSessionStagedTransaction,
}

impl GeneratedAffineResidualGroupExactSessionDependentClassificationFailure {
    pub(crate) const fn error(&self) -> GeneratedAffineResidualGroupExactSessionError {
        self.error
    }

    pub(crate) fn into_transaction(
        self,
    ) -> GeneratedAffineResidualGroupExactSessionStagedTransaction {
        self.transaction
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionDependentClassificationFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionDependentClassificationFailure")
            .field("error", &self.error)
            .field("private_transaction", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualGroupExactSessionDependentClassificationFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("exact session dependent-row classification failed")
    }
}

impl std::error::Error for GeneratedAffineResidualGroupExactSessionDependentClassificationFailure {}

/// Non-forgeable, non-`Clone` owning proof that one live session transaction
/// has a dependent database payload.
///
/// Its raw database stage and retained target-state allocation remain private.
/// The scalar coordinates are retained only to check the supposedly
/// infallible typed publication tail after the private common commit kernel.
pub(crate) struct GeneratedAffineResidualGroupExactSessionClassifiedDependent {
    transaction: GeneratedAffineResidualGroupExactSessionStagedTransaction,
    source_ordinal: usize,
    reduction_count: usize,
}

impl GeneratedAffineResidualGroupExactSessionClassifiedDependent {
    pub(crate) fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }

    pub(crate) const fn reduction_count(&self) -> usize {
        self.reduction_count
    }

    pub(crate) fn into_transaction(
        self,
    ) -> GeneratedAffineResidualGroupExactSessionStagedTransaction {
        self.transaction
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionClassifiedDependent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionClassifiedDependent")
            .field("source_ordinal", &self.source_ordinal)
            .field("reduction_count", &self.reduction_count)
            .field("private_transaction", &"<redacted>")
            .finish()
    }
}

/// Typed successful result of committing a sealed dependent row.
pub(crate) struct GeneratedAffineResidualGroupExactSessionCommittedDependent {
    event: Arc<GeneratedAffineResidualGroupExactSessionEvent>,
}

impl GeneratedAffineResidualGroupExactSessionCommittedDependent {
    pub(crate) fn source_ordinal(&self) -> usize {
        self.event.source_ordinal
    }

    pub(crate) fn reductions(&self) -> &[GeneratedAffineResidualGroupExactReductionStep] {
        match &self.event.head {
            GeneratedAffineResidualGroupExactSessionEventHead::Replayable {
                database_evidence:
                    GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence::Dependent(evidence),
                ..
            } => evidence.reductions(),
            _ => {
                unreachable!("sealed dependent receipt changed database evidence")
            }
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionCommittedDependent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionCommittedDependent")
            .field("source_ordinal", &self.source_ordinal())
            .field("reduction_count", &self.reductions().len())
            .field("private_reductions", &"<redacted>")
            .field("private_event", &"<redacted>")
            .finish()
    }
}

/// Failure of the typed dependent commit path.
///
/// Every failure retains the complete sealed classification, including its
/// consume-once transaction. The prepared database commit tail cannot fail.
pub(crate) enum GeneratedAffineResidualGroupExactSessionCommitDependentFailure {
    Preflight {
        error: GeneratedAffineResidualGroupExactSessionError,
        classified: GeneratedAffineResidualGroupExactSessionClassifiedDependent,
    },
}

impl GeneratedAffineResidualGroupExactSessionCommitDependentFailure {
    pub(crate) const fn error(&self) -> GeneratedAffineResidualGroupExactSessionError {
        match self {
            Self::Preflight { error, .. } => *error,
        }
    }

    pub(crate) fn into_classified(
        self,
    ) -> Result<
        GeneratedAffineResidualGroupExactSessionClassifiedDependent,
        GeneratedAffineResidualGroupExactSessionError,
    > {
        match self {
            Self::Preflight { classified, .. } => Ok(classified),
        }
    }

    pub(crate) fn into_transaction(
        self,
    ) -> Result<
        GeneratedAffineResidualGroupExactSessionStagedTransaction,
        GeneratedAffineResidualGroupExactSessionError,
    > {
        self.into_classified()
            .map(GeneratedAffineResidualGroupExactSessionClassifiedDependent::into_transaction)
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionCommitDependentFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionCommitDependentFailure")
            .field("phase", &"preflight")
            .field("error", &self.error())
            .field("private_classification", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualGroupExactSessionCommitDependentFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("exact dependent session transition failed before commit")
    }
}

impl std::error::Error for GeneratedAffineResidualGroupExactSessionCommitDependentFailure {}

/// One allocation-bound exact solve session.
///
/// Construction is the unique source-profiled minting path for the initial target state:
/// the database first creates an opaque, non-`Clone` binding, which is consumed
/// by the state owner and immediately authenticated back against that same
/// database allocation.
pub(crate) struct GeneratedAffineResidualGroupExactSession {
    schema: &'static str,
    source_kind: GeneratedAffineResidualCaseAuthoritySourceKind,
    plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
    database_capability: GeneratedAffineResidualGroupExactSessionDatabaseCapability,
    database: GeneratedAffineResidualGroupExactDatabase,
    catalog: Arc<GeneratedAffineResidualGroupExactTargetCatalog>,
    target_state: Arc<GeneratedAffineResidualGroupExactTargetState>,
    event_authority: Arc<GeneratedAffineResidualGroupExactSessionEventAuthority>,
    events: Vec<Arc<GeneratedAffineResidualGroupExactSessionEvent>>,
    event_stats: GeneratedAffineResidualGroupExactSessionEventStats,
    limits: GeneratedAffineResidualGroupExactSessionLimits,
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSession")
            .field("schema", &self.schema)
            .field("database_epoch", &self.database.database_epoch())
            .field("group_ordinal", &self.database.group_ordinal())
            .field("state_version", &self.database.state_version())
            .field("pivot_count", &self.database.pivot_count())
            .field("target_count", &self.catalog.len())
            .field("event_count", &self.events.len())
            .field("event_stats", &self.event_stats)
            .field("private_plan", &"<redacted>")
            .field("private_database_capability", &"<redacted>")
            .field("private_database", &"<redacted>")
            .field("private_catalog", &"<redacted>")
            .field("private_target_state", &"<redacted>")
            .field("private_event_authority", &"<redacted>")
            .field("private_events", &"<redacted>")
            .field("publishes_rule", &false)
            .field("infers_master", &false)
            .finish()
    }
}

impl GeneratedAffineResidualGroupExactSession {
    pub(crate) fn try_new(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
        database_epoch: usize,
        limits: GeneratedAffineResidualGroupExactSessionLimits,
    ) -> Result<Self, GeneratedAffineResidualGroupExactSessionError> {
        catch_unwind(AssertUnwindSafe(|| {
            let source_kind = plan.source_kind();
            let database_capability =
                GeneratedAffineResidualGroupExactSessionDatabaseCapability::mint();
            let database = GeneratedAffineResidualGroupExactDatabase::try_new(
                family,
                context,
                Arc::clone(&plan),
                Arc::clone(plan.physical_frame()),
                database_epoch,
                limits.database,
            )?;
            let catalog = Arc::new(plan.compile_exact_target_catalog(
                family,
                context,
                limits.target_catalog,
            )?);
            let binding =
                database.initial_target_state_binding_for_session(&database_capability)?;
            let target_state = GeneratedAffineResidualGroupExactTargetState::try_new(
                family,
                context,
                Arc::clone(&catalog),
                binding,
                limits.target_state,
            )?;
            database.authenticate_target_state_binding(target_state.binding())?;
            if database.source_kind() != source_kind
                || catalog.source_kind() != source_kind
                || target_state.source_kind() != source_kind
            {
                return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
            }
            let event_authority =
                Arc::new(GeneratedAffineResidualGroupExactSessionEventAuthority {
                    schema: exact_session_event_schema_for_source(source_kind),
                    source_kind,
                    plan: Arc::clone(&plan),
                    catalog: Arc::clone(&catalog),
                    database_epoch,
                    group_ordinal: plan.group_ordinal(),
                });
            let authority_retained_bytes = session_event_arc_retained_bytes::<
                GeneratedAffineResidualGroupExactSessionEventAuthority,
            >()?;
            session_event_check_limit(
                "exact session event-ledger retained bytes",
                authority_retained_bytes,
                limits.events.max_ledger_retained_bytes,
            )?;
            Ok(Self {
                schema: exact_session_schema_for_source(source_kind),
                source_kind,
                plan,
                database_capability,
                database,
                catalog,
                target_state,
                event_authority,
                events: Vec::new(),
                event_stats: GeneratedAffineResidualGroupExactSessionEventStats {
                    ledger_retained_bytes: authority_retained_bytes,
                    ledger_replacement_peak_bytes: authority_retained_bytes,
                    ..GeneratedAffineResidualGroupExactSessionEventStats::default()
                },
                limits,
            })
        }))
        .map_err(|_| GeneratedAffineResidualGroupExactSessionError::SymbolicaPanic)?
    }

    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) const fn source_kind(&self) -> GeneratedAffineResidualCaseAuthoritySourceKind {
        self.source_kind
    }

    pub(crate) const fn limits(&self) -> GeneratedAffineResidualGroupExactSessionLimits {
        self.limits
    }

    pub(crate) const fn database_epoch(&self) -> usize {
        self.database.database_epoch()
    }

    pub(crate) const fn group_ordinal(&self) -> usize {
        self.database.group_ordinal()
    }

    pub(crate) const fn state_version(&self) -> usize {
        self.database.state_version()
    }

    pub(crate) fn target_count(&self) -> usize {
        self.catalog.len()
    }

    /// Borrow one committed compact event by its chronological ordinal.
    /// Non-publication events are deliberately invisible through this seam.
    pub(crate) fn committed_publication_event(
        &self,
        event_ordinal: usize,
    ) -> Option<CommittedPublicationEventView<'_>> {
        let event = self.events.get(event_ordinal)?;
        debug_assert_eq!(event.event_ordinal, event_ordinal);
        CommittedPublicationEventView::from_event(event)
    }

    /// Retain one compact event independently of the mutable session epoch.
    /// The returned owner adds only one shallow `Arc` reference.
    pub(crate) fn committed_publication_event_handle(
        &self,
        event_ordinal: usize,
    ) -> Option<CommittedPublicationEventHandle> {
        let event = self.events.get(event_ordinal)?;
        CommittedPublicationEventView::from_event(event)?;
        Some(CommittedPublicationEventHandle {
            event: Arc::clone(event),
        })
    }

    /// Iterate committed compact events in deterministic ledger order without
    /// cloning their rows, partitions, or `Arc` owners.
    pub(crate) fn committed_publication_events(
        &self,
    ) -> impl Iterator<Item = CommittedPublicationEventView<'_>> + '_ {
        self.events
            .iter()
            .filter_map(|event| CommittedPublicationEventView::from_event(event))
    }

    /// Retain every compact event in deterministic ledger order using one
    /// shallow `Arc` per event and no leaf- or payload-sized clone.
    pub(crate) fn committed_publication_event_handles(
        &self,
    ) -> impl Iterator<Item = CommittedPublicationEventHandle> + '_ {
        self.events.iter().filter_map(|event| {
            CommittedPublicationEventView::from_event(event).map(|_| {
                CommittedPublicationEventHandle {
                    event: Arc::clone(event),
                }
            })
        })
    }

    #[cfg(test)]
    pub(crate) fn consumed_target_count(&self) -> usize {
        self.target_state.stats().consumed()
    }

    #[cfg(test)]
    pub(crate) fn publication_retained_bytes_for_test(
        &self,
        publication: &PreparedPublication,
    ) -> usize {
        let ready = publication.ready();
        ready
            .recentered
            .application_row_deep_owned_retained_byte_bound()
            .unwrap()
            + publication
                .payload()
                .deep_owned_retained_byte_bound()
                .unwrap()
    }

    #[cfg(test)]
    pub(crate) fn authenticate_event_ledger_census_for_test(
        &self,
    ) -> Result<usize, GeneratedAffineResidualGroupExactSessionError> {
        self.authenticate_event_ledger_census()
    }

    #[cfg(test)]
    pub(crate) fn last_publication_payload_for_test(&self) -> Option<&PublicationPayload> {
        match &self.events.last()?.disposition {
            GeneratedAffineResidualGroupExactSessionEventDisposition::Publication {
                publication,
                ..
            } => Some(publication),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn last_publication_term_count_for_test(&self) -> Option<usize> {
        match &self.events.last()?.disposition {
            GeneratedAffineResidualGroupExactSessionEventDisposition::Publication {
                row, ..
            } => Some(row.terms().len()),
            _ => None,
        }
    }

    pub(crate) const fn event_stats(&self) -> GeneratedAffineResidualGroupExactSessionEventStats {
        self.event_stats
    }

    /// Deterministic committed native-reconstruction telemetry for scaling
    /// harnesses. This diagnostic snapshot is excluded from replay identity.
    pub(crate) const fn native_sparse_scaling_stats(
        &self,
    ) -> GeneratedAffineResidualGroupExactNativeSparseScalingStats {
        self.database.stats().native_sparse_scaling()
    }

    pub(crate) const fn publishes_rule(&self) -> bool {
        false
    }

    pub(crate) const fn infers_master(&self) -> bool {
        false
    }

    fn authenticate_event_head(&self) -> Result<(), GeneratedAffineResidualGroupExactSessionError> {
        if self.event_authority.schema != exact_session_event_schema_for_source(self.source_kind)
            || self.event_authority.source_kind != self.source_kind
            || !Arc::ptr_eq(&self.event_authority.plan, &self.plan)
            || !Arc::ptr_eq(&self.event_authority.catalog, &self.catalog)
            || self.event_authority.database_epoch != self.database_epoch()
            || self.event_authority.group_ordinal != self.group_ordinal()
            || self.events.len() != self.event_stats.events
            || self.events.len() != self.database.state_version()
            || session_event_outer_buffer_bytes(self.events.capacity())?
                != self.event_stats.ledger_outer_buffer_bytes
        {
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
        }
        let authority_bytes = session_event_arc_retained_bytes::<
            GeneratedAffineResidualGroupExactSessionEventAuthority,
        >()?;
        if self.event_stats.ledger_retained_bytes < authority_bytes
            || self.event_stats.ledger_replacement_peak_bytes
                < self.event_stats.ledger_retained_bytes
        {
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
        }
        let Some(last) = self.events.last() else {
            if self.database.state_version() != 0
                || self.event_stats
                    != (GeneratedAffineResidualGroupExactSessionEventStats {
                        ledger_retained_bytes: authority_bytes,
                        ledger_replacement_peak_bytes: authority_bytes,
                        ..GeneratedAffineResidualGroupExactSessionEventStats::default()
                    })
            {
                return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
            }
            return Ok(());
        };
        if !Arc::ptr_eq(&last.authority, &self.event_authority)
            || last.event_ordinal.checked_add(1) != Some(self.events.len())
            || last.source_ordinal != last.event_ordinal
            || last.successor_state_version != self.database.state_version()
            || last.predecessor_state_version.checked_add(1) != Some(last.successor_state_version)
        {
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
        }
        match (&last.head, &last.disposition) {
            (
                GeneratedAffineResidualGroupExactSessionEventHead::Replayable {
                    source_recipe,
                    database_evidence:
                        GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence::Dependent(_),
                },
                GeneratedAffineResidualGroupExactSessionEventDisposition::Dependent,
            ) if source_recipe.database_epoch() == self.database_epoch()
                && source_recipe.group_ordinal() == self.group_ordinal() => {}
            (
                GeneratedAffineResidualGroupExactSessionEventHead::Replayable {
                    source_recipe,
                    database_evidence:
                        GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence::NewPivot(
                            pivot,
                        ),
                },
                GeneratedAffineResidualGroupExactSessionEventDisposition::NoTarget { .. }
                | GeneratedAffineResidualGroupExactSessionEventDisposition::RequiresAffineEqualityRefinement { .. },
            ) if source_recipe.database_epoch() == self.database_epoch()
                && source_recipe.group_ordinal() == self.group_ordinal()
                && pivot.source_ordinal() == last.source_ordinal => {}
            (
                GeneratedAffineResidualGroupExactSessionEventHead::Publication { pivot_ordinal },
                GeneratedAffineResidualGroupExactSessionEventDisposition::Publication { .. },
            ) if self.database.pivot(*pivot_ordinal).is_some_and(|pivot| {
                pivot.source_ordinal() == last.source_ordinal
            }) => {}
            #[cfg(test)]
            (
                GeneratedAffineResidualGroupExactSessionEventHead::Replayable {
                    database_evidence:
                        GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence::NewPivot(
                            pivot,
                        ),
                    ..
                },
                GeneratedAffineResidualGroupExactSessionEventDisposition::TestSeedPivot,
            ) if pivot.source_ordinal() == last.source_ordinal => {}
            _ => return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch),
        }
        Ok(())
    }

    /// Recompute the complete ledger-level census from allocation-bound event
    /// payloads before replay trusts the persisted aggregate ledger scalars.
    /// Child owners retain their own authenticated census conventions.
    /// Production staging deliberately uses only the constant-time head
    /// authentication above; this historical walk belongs exclusively to
    /// chronological replay.
    fn authenticate_event_ledger_census(
        &self,
    ) -> Result<usize, GeneratedAffineResidualGroupExactSessionError> {
        let limits = self.limits.events;
        let authority_bytes = session_event_arc_retained_bytes::<
            GeneratedAffineResidualGroupExactSessionEventAuthority,
        >()?;
        let mut computed = GeneratedAffineResidualGroupExactSessionEventStats::default();
        let mut event_payload_bytes = 0usize;

        for (position, event) in self.events.iter().enumerate() {
            if !Arc::ptr_eq(&event.authority, &self.event_authority)
                || event.event_ordinal != position
                || event.source_ordinal != position
                || event.predecessor_state_version != position
                || event.successor_state_version
                    != position.checked_add(1).ok_or(
                        GeneratedAffineResidualGroupExactSessionError::EventCountOverflow {
                            resource: "exact session authenticated event successor version",
                        },
                    )?
            {
                return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
            }

            computed.events = session_event_checked_add(
                "exact session authenticated events",
                computed.events,
                1,
            )?;
            computed.target_state_successor_copies = session_event_checked_add(
                "exact session authenticated target-state successor copies",
                computed.target_state_successor_copies,
                self.target_count(),
            )?;
            computed.ledger_arc_copies = session_event_checked_add(
                "exact session authenticated ledger Arc copies",
                computed.ledger_arc_copies,
                position,
            )?;

            let (source_recipe_deep_bytes, dependent_evidence_deep_bytes, reduction_count) =
                match &event.head {
                    GeneratedAffineResidualGroupExactSessionEventHead::Replayable {
                        source_recipe,
                        database_evidence,
                    } => {
                        if source_recipe.database_epoch() != self.database_epoch()
                            || source_recipe.group_ordinal() != self.group_ordinal()
                        {
                            return Err(
                                GeneratedAffineResidualGroupExactSessionError::ReplayMismatch,
                            );
                        }
                        let mut source_is_new = true;
                        for prior in &self.events[..position] {
                            let GeneratedAffineResidualGroupExactSessionEventHead::Replayable {
                                source_recipe: prior_recipe,
                                ..
                            } = &prior.head
                            else {
                                continue;
                            };
                            computed.source_recipe_allocation_comparisons =
                                session_event_bounded_add(
                                    "exact session source-recipe allocation comparisons",
                                    computed.source_recipe_allocation_comparisons,
                                    1,
                                    limits.max_source_recipe_allocation_comparisons,
                                )?;
                            if prior_recipe.same_source_allocation(source_recipe) {
                                source_is_new = false;
                                break;
                            }
                        }
                        let source_recipe_deep_bytes = if source_is_new {
                            session_event_checked_sub(
                                "exact session authenticated source-recipe retained bytes",
                                source_recipe.retained_byte_bound()?,
                                size_of::<GeneratedAffineResidualGroupRetainedExactSourceRecipe>(),
                            )?
                        } else {
                            0
                        };
                        let dependent_evidence_deep_bytes = match (
                            database_evidence,
                            &event.disposition,
                        ) {
                            (
                                GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence::Dependent(
                                    evidence,
                                ),
                                GeneratedAffineResidualGroupExactSessionEventDisposition::Dependent,
                            ) => evidence.retained_byte_bound()?,
                            (
                                GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence::NewPivot(
                                    pivot,
                                ),
                                GeneratedAffineResidualGroupExactSessionEventDisposition::NoTarget { .. }
                                | GeneratedAffineResidualGroupExactSessionEventDisposition::RequiresAffineEqualityRefinement { .. },
                            ) if pivot.source_ordinal() == event.source_ordinal => 0,
                            #[cfg(test)]
                            (
                                GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence::NewPivot(
                                    pivot,
                                ),
                                GeneratedAffineResidualGroupExactSessionEventDisposition::TestSeedPivot,
                            ) if pivot.source_ordinal() == event.source_ordinal => 0,
                            _ => {
                                return Err(
                                    GeneratedAffineResidualGroupExactSessionError::ReplayMismatch,
                                );
                            }
                        };
                        (
                            source_recipe_deep_bytes,
                            dependent_evidence_deep_bytes,
                            database_evidence.reduction_count(),
                        )
                    }
                    GeneratedAffineResidualGroupExactSessionEventHead::Publication {
                        pivot_ordinal,
                    } => {
                        if !matches!(
                            &event.disposition,
                            GeneratedAffineResidualGroupExactSessionEventDisposition::Publication { .. }
                        ) || !self
                            .database
                            .pivot(*pivot_ordinal)
                            .is_some_and(|pivot| pivot.source_ordinal() == event.source_ordinal)
                        {
                            return Err(
                                GeneratedAffineResidualGroupExactSessionError::ReplayMismatch,
                            );
                        }
                        (0, 0, 0)
                    }
                };
            computed.unique_source_recipe_retained_bytes = session_event_checked_add(
                "exact session authenticated unique source-recipe retained bytes",
                computed.unique_source_recipe_retained_bytes,
                source_recipe_deep_bytes,
            )?;
            computed.reduction_steps = session_event_checked_add(
                "exact session authenticated retained reduction steps",
                computed.reduction_steps,
                reduction_count,
            )?;

            let (
                target_offset_components,
                target_offset_integer_bits,
                target_offset_retained_bytes,
                equality_predicates,
                equality_predicate_buffer_bytes,
                publication_retained_bytes,
            ) = match &event.disposition {
                GeneratedAffineResidualGroupExactSessionEventDisposition::Dependent => {
                    (0, 0, 0, 0, 0, 0)
                }
                GeneratedAffineResidualGroupExactSessionEventDisposition::NoTarget {
                    target_offset,
                    ..
                } => {
                    let (retained_integer_bits, retained_bytes) = target_offset
                        .authenticate_retained_census()
                        .map_err(|_| GeneratedAffineResidualGroupExactSessionError::ReplayMismatch)?;
                    (
                        target_offset.values().len(),
                        retained_integer_bits,
                        retained_bytes,
                        0,
                        0,
                        0,
                    )
                }
                GeneratedAffineResidualGroupExactSessionEventDisposition::RequiresAffineEqualityRefinement {
                    target_offset,
                    equality_predicate_ordinals,
                    ..
                } => {
                    let (retained_integer_bits, retained_bytes) = target_offset
                        .authenticate_retained_census()
                        .map_err(|_| GeneratedAffineResidualGroupExactSessionError::ReplayMismatch)?;
                    (
                        target_offset.values().len(),
                        retained_integer_bits,
                        retained_bytes,
                        equality_predicate_ordinals.len(),
                        session_event_checked_mul(
                            "exact session authenticated equality-predicate bytes",
                            equality_predicate_ordinals.capacity(),
                            size_of::<usize>(),
                        )?,
                        0,
                    )
                }
                GeneratedAffineResidualGroupExactSessionEventDisposition::Publication {
                    target_offset,
                    row,
                    publication,
                    ..
                } => {
                    let (retained_integer_bits, retained_bytes) = target_offset
                        .authenticate_retained_census()
                        .map_err(|_| GeneratedAffineResidualGroupExactSessionError::ReplayMismatch)?;
                    let row_deep = row
                        .deep_owned_retained_byte_bound()
                        .map_err(|_| GeneratedAffineResidualGroupExactSessionError::ReplayMismatch)?;
                    let payload_deep = publication
                        .deep_owned_retained_byte_bound()
                        .map_err(|_| GeneratedAffineResidualGroupExactSessionError::ReplayMismatch)?;
                    (
                        target_offset.values().len(),
                        retained_integer_bits,
                        retained_bytes,
                        0,
                        0,
                        session_event_checked_add(
                            "exact session authenticated publication retained bytes",
                            row_deep,
                            payload_deep,
                        )?,
                    )
                }
                #[cfg(test)]
                GeneratedAffineResidualGroupExactSessionEventDisposition::TestSeedPivot => {
                    (0, 0, 0, 0, 0, 0)
                }
            };
            computed.target_offset_components = session_event_checked_add(
                "exact session authenticated target-offset components",
                computed.target_offset_components,
                target_offset_components,
            )?;
            computed.target_offset_integer_bits = session_event_checked_add(
                "exact session authenticated target-offset integer bits",
                computed.target_offset_integer_bits,
                target_offset_integer_bits,
            )?;
            computed.target_offset_retained_bytes = session_event_checked_add(
                "exact session authenticated target-offset retained bytes",
                computed.target_offset_retained_bytes,
                target_offset_retained_bytes,
            )?;
            computed.equality_predicates = session_event_checked_add(
                "exact session authenticated equality predicates",
                computed.equality_predicates,
                equality_predicates,
            )?;
            computed.publication_retained_bytes = session_event_checked_add(
                "exact session authenticated publication retained bytes",
                computed.publication_retained_bytes,
                publication_retained_bytes,
            )?;

            let individual_event_retained_bytes = [
                session_event_arc_retained_bytes::<GeneratedAffineResidualGroupExactSessionEvent>(
                )?,
                source_recipe_deep_bytes,
                dependent_evidence_deep_bytes,
                target_offset_retained_bytes,
                equality_predicate_buffer_bytes,
                publication_retained_bytes,
            ]
            .into_iter()
            .try_fold(0usize, |total, bytes| {
                session_event_checked_add(
                    "exact session authenticated individual event retained bytes",
                    total,
                    bytes,
                )
            })?;
            if event.retained_bytes != individual_event_retained_bytes {
                return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
            }
            event_payload_bytes = session_event_checked_add(
                "exact session authenticated event payload bytes",
                event_payload_bytes,
                individual_event_retained_bytes,
            )?;
        }

        computed.ledger_outer_buffer_bytes =
            session_event_outer_buffer_bytes(self.events.capacity())?;
        computed.ledger_retained_bytes = [
            authority_bytes,
            computed.ledger_outer_buffer_bytes,
            event_payload_bytes,
        ]
        .into_iter()
        .try_fold(0usize, |total, bytes| {
            session_event_checked_add(
                "exact session authenticated event-ledger retained bytes",
                total,
                bytes,
            )
        })?;
        computed.ledger_replacement_peak_bytes = self.event_stats.ledger_replacement_peak_bytes;
        if computed != self.event_stats
            || self.event_stats.ledger_replacement_peak_bytes
                < self.event_stats.ledger_retained_bytes
            || self.event_stats.ledger_replacement_peak_bytes
                > self.event_stats.ledger_retained_bytes.saturating_mul(2)
            || self.event_stats.ledger_replacement_peak_bytes
                > limits.max_ledger_replacement_peak_bytes
        {
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
        }
        Ok(computed.ledger_retained_bytes)
    }

    /// Authenticate every retained child and the opaque database/state
    /// handshake without walking committed source history.
    ///
    /// Production staging deliberately calls this bounded current-state seam.
    /// The chronological event replayer uses a separate path so appending row
    /// `k` never recursively replays prefixes `0..k`.
    fn authenticate_live_state(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedAffineResidualGroupExactSessionError> {
        catch_unwind(AssertUnwindSafe(|| {
            if self.schema != exact_session_schema_for_source(self.source_kind)
                || self.source_kind != self.plan.source_kind()
                || self.database.source_kind() != self.source_kind
                || self.catalog.source_kind() != self.source_kind
                || self.target_state.source_kind() != self.source_kind
                || self.database.group_ordinal() != self.plan.group_ordinal()
                || self.database.database_epoch() != self.target_state.database_epoch()
                || self.database.state_version() != self.target_state.state_version()
                || self.catalog.group_ordinal() != self.plan.group_ordinal()
                || !self.catalog.same_plan_allocation(&self.plan)
            {
                return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
            }
            self.authenticate_event_head()?;
            self.database
                .authenticate_target_state_binding(self.target_state.binding())?;
            self.catalog.replay(family, context, &self.plan)?;
            self.target_state.replay(
                family,
                context,
                &self.plan,
                self.database.group_ordinal(),
                self.database.database_epoch(),
                self.database.state_version(),
            )?;
            Ok(())
        }))
        .map_err(|_| GeneratedAffineResidualGroupExactSessionError::SymbolicaPanic)?
    }

    fn stage_retained_event_recipe_for_replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        recipe: &GeneratedAffineResidualGroupRetainedExactSourceRecipe,
    ) -> Result<
        GeneratedAffineResidualGroupExactSessionStagedTransaction,
        GeneratedAffineResidualGroupExactSessionError,
    > {
        self.authenticate_event_head()?;
        self.database
            .authenticate_target_state_binding(self.target_state.binding())?;
        let staged = self.database.stage_retained_source_recipe_for_session(
            &self.database_capability,
            family,
            context,
            recipe,
        )?;
        Ok(GeneratedAffineResidualGroupExactSessionStagedTransaction {
            staged,
            target_state: Arc::clone(&self.target_state),
        })
    }

    fn compare_replayed_final_state(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        shadow: &Self,
    ) -> Result<(), GeneratedAffineResidualGroupExactSessionError> {
        if self.database.state_version() != shadow.database.state_version()
            || self.database.pivot_count() != shadow.database.pivot_count()
            || !self
                .database
                .stats()
                .replay_semantically_equal(shadow.database.stats())
            || self.target_state.stats() != shadow.target_state.stats()
            || self.event_stats != shadow.event_stats
            || self.events.len() != shadow.events.len()
        {
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
        }
        let left = self.target_state.authenticated_view(family, context)?;
        let right = shadow.target_state.authenticated_view(family, context)?;
        if left.iter().len() != right.iter().len() {
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
        }
        for solve_ordinal in left.iter() {
            if left.is_unresolved(solve_ordinal)? != right.is_unresolved(solve_ordinal)? {
                return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
            }
        }
        Ok(())
    }

    fn preflight_replay_work(&self) -> Result<(), GeneratedAffineResidualGroupExactSessionError> {
        if self.events.iter().any(|recorded| {
            matches!(
                &recorded.disposition,
                GeneratedAffineResidualGroupExactSessionEventDisposition::Publication { .. }
            )
        }) {
            // Compact publication deliberately retains application data, not
            // the derivation transcript required by this audit path.
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
        }
        let mut work = GeneratedAffineResidualGroupExactSessionReplayWork::default();
        for recorded in &self.events {
            recorded.account_replay_work(self.target_count(), &mut work, self.limits.events)?;
        }
        // Final state comparison scans the complete target catalog once after
        // all events. Admit that work together with the event history before
        // constructing the fresh shadow owner.
        let _ = session_event_bounded_add(
            "exact session replay target scans",
            work.target_scans,
            self.target_count(),
            self.limits.events.max_replay_target_scans,
        )?;
        Ok(())
    }

    fn replay_combined_retained_peak_bound(
        &self,
        authenticated_ledger_retained_bytes: usize,
    ) -> usize {
        // Child limits, rather than persisted peak scalars, bound the fresh
        // shadow's session-local staging coexistence. Every child enforces
        // these same limits while constructing the shadow. Immutable parent
        // ancestry follows the child-owner conventions and is pointer-shared,
        // so this is deliberately not a transitive whole-process RSS census.
        // The plan-local census includes its pointee but not its Arc control
        // block. Original and shadow share that allocation, so charge it once;
        // their two inline Arc handles are already present in the two Self
        // envelopes below.
        let shared_plan_arc_owner_bytes = self
            .plan
            .stats()
            .owner_retained_bytes()
            .saturating_add(2usize.saturating_mul(size_of::<usize>()));
        let original_owner_bound = session_event_saturating_sum([
            size_of::<Self>(),
            self.limits.database.max_database_retained_bytes,
            self.limits.target_state.max_combined_retained_byte_envelope,
            authenticated_ledger_retained_bytes,
        ]);
        let shadow_peak_bound = session_event_saturating_sum([
            size_of::<Self>(),
            self.limits
                .database
                .max_database_retained_bytes
                .max(self.limits.database.max_staged_live_retained_bytes),
            self.limits.target_catalog.max_peak_staging_byte_envelope,
            self.limits
                .target_state
                .max_combined_retained_byte_envelope
                .max(
                    self.limits
                        .target_state
                        .max_successor_peak_retained_byte_envelope,
                ),
            self.limits.events.max_ledger_replacement_peak_bytes,
        ]);
        session_event_saturating_sum([
            shared_plan_arc_owner_bytes,
            original_owner_bound,
            shadow_peak_bound,
        ])
    }

    fn replay_committed_events_inner(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedAffineResidualGroupExactSessionError> {
        session_event_check_limit(
            "exact session replay events",
            self.events.len(),
            self.limits.events.max_replay_events,
        )?;
        // This pass reads only retained collection lengths and admits event
        // replay plus every deep reduction/pivot/offset traversal before the
        // census walks coefficient factors. Source-allocation comparisons are
        // separately bounded immediately before each census comparison.
        self.preflight_replay_work()?;
        let authenticated_ledger_retained_bytes = self.authenticate_event_ledger_census()?;
        let replay_combined_retained_bytes =
            self.replay_combined_retained_peak_bound(authenticated_ledger_retained_bytes);
        session_event_check_limit(
            "exact session replay combined retained bytes",
            replay_combined_retained_bytes,
            self.limits.events.max_replay_combined_retained_bytes,
        )?;
        let mut shadow = GeneratedAffineResidualGroupExactSession::try_new(
            family,
            context,
            Arc::clone(&self.plan),
            self.database_epoch(),
            self.limits,
        )?;
        for (position, recorded) in self.events.iter().enumerate() {
            if !Arc::ptr_eq(&recorded.authority, &self.event_authority)
                || recorded.event_ordinal != position
                || recorded.source_ordinal != position
                || recorded.predecessor_state_version != position
                || recorded.successor_state_version != position + 1
            {
                return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
            }
            let GeneratedAffineResidualGroupExactSessionEventHead::Replayable {
                source_recipe,
                database_evidence,
            } = &recorded.head
            else {
                unreachable!("compact publication was rejected before replay construction")
            };
            let transaction =
                shadow.stage_retained_event_recipe_for_replay(family, context, source_recipe)?;
            match &recorded.disposition {
                GeneratedAffineResidualGroupExactSessionEventDisposition::Dependent => {
                    let expected = match database_evidence {
                        GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence::Dependent(
                            evidence,
                        ) => evidence,
                        GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence::NewPivot(
                            _,
                        ) => {
                            return Err(
                                GeneratedAffineResidualGroupExactSessionError::ReplayMismatch,
                            );
                        }
                    };
                    let replayed = shadow.database.authenticate_staged_dependent_for_session(
                        &shadow.database_capability,
                        &transaction.staged,
                    )?;
                    if replayed.source_ordinal() != recorded.source_ordinal
                        || replayed.reductions() != expected.reductions()
                    {
                        return Err(
                            GeneratedAffineResidualGroupExactSessionError::ReplayMismatch,
                        );
                    }
                    drop(replayed);
                    let classified = shadow
                        .classify_dependent(transaction)
                        .map_err(|failure| failure.error())?;
                    let receipt = shadow
                        .commit_dependent(family, context, classified)
                        .map_err(|failure| failure.error())?;
                    if !recorded.semantically_equal(&receipt.event) {
                        return Err(
                            GeneratedAffineResidualGroupExactSessionError::ReplayMismatch,
                        );
                    }
                }
                GeneratedAffineResidualGroupExactSessionEventDisposition::NoTarget { .. } => {
                    let outcome = shadow
                        .recenter_staged_new_pivot(family, context, transaction)
                        .map_err(|_| {
                            GeneratedAffineResidualGroupExactSessionError::ReplayMismatch
                        })?;
                    let GeneratedAffineResidualGroupExactSessionRecenterOutcome::NoTarget(
                        outcome,
                    ) = outcome
                    else {
                        return Err(
                            GeneratedAffineResidualGroupExactSessionError::ReplayMismatch,
                        );
                    };
                    let committed = shadow
                        .commit_no_target(family, context, outcome)
                        .map_err(|failure| failure.error())?;
                    if !recorded.semantically_equal(&committed.event) {
                        return Err(
                            GeneratedAffineResidualGroupExactSessionError::ReplayMismatch,
                        );
                    }
                    shadow = committed.into_session();
                }
                GeneratedAffineResidualGroupExactSessionEventDisposition::RequiresAffineEqualityRefinement { .. } => {
                    if position.checked_add(1) != Some(self.events.len()) {
                        return Err(
                            GeneratedAffineResidualGroupExactSessionError::ReplayMismatch,
                        );
                    }
                    let outcome = shadow
                        .recenter_staged_new_pivot(family, context, transaction)
                        .map_err(|_| {
                            GeneratedAffineResidualGroupExactSessionError::ReplayMismatch
                        })?;
                    let GeneratedAffineResidualGroupExactSessionRecenterOutcome::RequiresAffineEqualityRefinement(outcome) = outcome else {
                        return Err(
                            GeneratedAffineResidualGroupExactSessionError::ReplayMismatch,
                        );
                    };
                    let suspended = shadow
                        .commit_and_suspend_affine_equality_refinement(
                            family,
                            context,
                            outcome,
                        )
                        .map_err(|failure| failure.error())?;
                    if !recorded.semantically_equal(&suspended.event) {
                        return Err(
                            GeneratedAffineResidualGroupExactSessionError::ReplayMismatch,
                        );
                    }
                    return self.compare_replayed_final_state(
                        family,
                        context,
                        &suspended.committed_session,
                    );
                }
                GeneratedAffineResidualGroupExactSessionEventDisposition::Publication { .. } => {
                    unreachable!("compact publication was rejected before replay construction")
                }
                #[cfg(test)]
                GeneratedAffineResidualGroupExactSessionEventDisposition::TestSeedPivot => {
                    let outcome = shadow
                        .commit_unconsumed(family, context, transaction)
                        .map_err(|failure| failure.error())?;
                    if !matches!(
                        outcome,
                        GeneratedAffineResidualGroupExactRowOutcome::NewPivot { .. }
                    ) || !recorded.semantically_equal(
                        shadow.events.last().ok_or(
                            GeneratedAffineResidualGroupExactSessionError::ReplayMismatch,
                        )?,
                    ) {
                        return Err(
                            GeneratedAffineResidualGroupExactSessionError::ReplayMismatch,
                        );
                    }
                }
            }
        }
        self.compare_replayed_final_state(family, context, &shadow)
    }

    /// Authenticate current authority and chronologically re-execute every
    /// retained raw source recipe into fresh database/target/event owners.
    /// Production staging uses only `authenticate_live_state`, so appending
    /// row `k` does not recursively replay `0..k`.
    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedAffineResidualGroupExactSessionError> {
        self.authenticate_live_state(family, context)?;
        catch_unwind(AssertUnwindSafe(|| {
            self.replay_committed_events_inner(family, context)
        }))
        .map_err(|_| GeneratedAffineResidualGroupExactSessionError::SymbolicaPanic)?
    }

    /// Stage one authenticated production row without mutating either owner.
    ///
    /// The returned token retains the exact current target-state allocation;
    /// callers cannot replace it or extract the raw database stage.
    pub(crate) fn stage_replayed_row(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source: &Arc<GeneratedAffineResidualGroupExactPhysicalRow>,
    ) -> Result<
        GeneratedAffineResidualGroupExactSessionStagedTransaction,
        GeneratedAffineResidualGroupExactSessionError,
    > {
        self.authenticate_live_state(family, context)?;
        let staged = self.database.stage_replayed_row_for_session(
            &self.database_capability,
            family,
            context,
            &self.plan,
            self.plan.physical_frame(),
            self.database.database_epoch(),
            source,
        )?;
        Ok(GeneratedAffineResidualGroupExactSessionStagedTransaction {
            staged,
            target_state: Arc::clone(&self.target_state),
        })
    }

    /// Test-only ingress for a sibling module that has already constructed
    /// authenticated physical keys, coefficients, and guards. The database
    /// call remains protected by the private session capability, and callers
    /// receive the same inseparable transaction used by production staging.
    #[cfg(test)]
    pub(crate) fn stage_authenticated_terms_for_test(
        &self,
        context: &ParametricCoefficientContext,
        terms: Vec<(
            GeneratedAffineResidualGroupPhysicalKey,
            ParametricCoefficient,
        )>,
        guards: Vec<ParametricNonZeroCondition>,
    ) -> Result<
        GeneratedAffineResidualGroupExactSessionStagedTransaction,
        GeneratedAffineResidualGroupExactSessionError,
    > {
        self.database
            .authenticate_target_state_binding(self.target_state.binding())?;
        let staged = self.database.stage_authenticated_terms_for_session(
            &self.database_capability,
            context,
            terms,
            guards,
        )?;
        Ok(GeneratedAffineResidualGroupExactSessionStagedTransaction {
            staged,
            target_state: Arc::clone(&self.target_state),
        })
    }

    /// Test-only construction of a genuine competing live transition from the
    /// same predecessor as a prepared publication. Committing it advances the
    /// session and makes the untouched publication stale without forging any
    /// scalar identity.
    #[cfg(test)]
    pub(crate) fn advance_competing_publication_head_for_test(
        &mut self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source: &Arc<GeneratedAffineResidualGroupExactPhysicalRow>,
        publication: &PreparedPublication,
    ) -> Result<(), GeneratedAffineResidualGroupExactSessionError> {
        self.current_publication_pivot(publication)?;
        let transaction = self.stage_replayed_row(family, context, source)?;
        let outcome = self
            .commit_unconsumed(family, context, transaction)
            .map_err(|failure| failure.error())?;
        if !matches!(
            outcome,
            GeneratedAffineResidualGroupExactRowOutcome::NewPivot { .. }
        ) {
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
        }
        Ok(())
    }

    /// Jointly authenticate one staged new pivot and its exact unresolved
    /// target state. This is the sole recentering ingress for either
    /// retained-source schema.
    fn authenticate_staged_new_pivot<'a>(
        &'a self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        transaction: &'a GeneratedAffineResidualGroupExactSessionStagedTransaction,
    ) -> Result<
        GeneratedAffineResidualGroupExactSessionStagedNewPivotView<'a>,
        GeneratedAffineResidualGroupExactSessionError,
    > {
        self.authenticate_target_state_allocation(&transaction.target_state)?;
        self.database
            .authenticate_target_state_binding(transaction.target_state.binding())?;
        let staged_pivot = self.database.authenticate_staged_new_pivot_for_session(
            &self.database_capability,
            &transaction.staged,
        )?;
        let targets = transaction
            .target_state
            .authenticated_view(family, context)?;
        if !targets.authenticates_state_allocation(&self.target_state)
            || !Arc::ptr_eq(
                staged_pivot.plan_for_session(&self.database_capability),
                &self.plan,
            )
            || !Arc::ptr_eq(staged_pivot.frame(), self.plan.physical_frame())
        {
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
        }

        // Borrow the compact affine geometry from the retained, authenticated
        // plan authority only after the database and target-state transaction
        // have been jointly authenticated. The authority/plan owners never
        // cross this API; recentering receives only the two shape scalars and
        // borrowed row-major matrix it actually needs.
        let plan = staged_pivot.plan_for_session(&self.database_capability);
        let group = plan
            .authority()
            .authenticated_source_neutral_group_view(context)
            .map_err(|_| GeneratedAffineResidualGroupExactSessionError::GeometryAuthentication)?;
        let ambient_arity = group.ambient_arity();
        let free_positions = group.free_positions();
        let matrix_entries = ambient_arity
            .checked_mul(free_positions.len())
            .ok_or(GeneratedAffineResidualGroupExactSessionError::GeometryCountOverflow)?;
        let compact_affine_matrix = group.compact_linear_coefficients();
        if group.ordinal() != staged_pivot.group_ordinal()
            || ambient_arity != context.index_count()
            || ambient_arity != staged_pivot.frame().arity()
            || free_positions != plan.free_positions()
            || free_positions
                .iter()
                .any(|&position| position >= ambient_arity)
            || compact_affine_matrix.len() != matrix_entries
        {
            return Err(GeneratedAffineResidualGroupExactSessionError::MalformedGeometry);
        }
        let staged_live_prospective_retained_bytes =
            staged_pivot.staged_live_prospective_retained_bytes();
        let staged_live_observed_retained_bytes =
            staged_pivot.staged_live_observed_retained_bytes();
        let target_state_combined_retained_byte_envelope = transaction
            .target_state
            .stats()
            .combined_retained_byte_envelope();
        let anchor_case_ordinal = plan.anchor_case_ordinal();
        let free_positions = plan.free_positions();
        let target_locators = plan.targets();
        Ok(GeneratedAffineResidualGroupExactSessionStagedNewPivotView {
            staged_pivot,
            targets,
            anchor_case_ordinal,
            free_positions,
            target_locators,
            ambient_arity,
            compact_affine_matrix,
            staged_live_prospective_retained_bytes,
            staged_live_observed_retained_bytes,
            target_state_combined_retained_byte_envelope,
        })
    }

    /// Reauthenticate a sealed Ready token and expose only the exact geometry
    /// required by the current-lineage publication analysis.
    ///
    /// The returned view borrows both owners.  It cannot outlive this session
    /// or the Ready token, cannot extract the staged transaction, and cannot
    /// be used to consume a different target.
    pub(crate) fn authenticated_ready_geometry<'authority>(
        &'authority self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        ready: &'authority GeneratedAffineResidualGroupExactSessionRecenterReady,
    ) -> Result<
        GeneratedAffineResidualGroupExactSessionReadyGeometryView<'authority>,
        GeneratedAffineResidualGroupExactSessionError,
    > {
        let joint = self.authenticate_staged_new_pivot(family, context, &ready.transaction)?;
        let solve_ordinal = ready.target.solve_ordinal();
        let locator = *ready.target.locator();
        if joint.source_ordinal() != ready.source_ordinal
            || joint.pivot_ordinal() != ready.pivot_ordinal
            || !ready
                .target
                .authenticates_source_state(&ready.transaction.target_state)
            || !joint.is_target_unresolved(solve_ordinal)?
            || joint.target_locators().get(solve_ordinal).copied() != Some(locator)
        {
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
        }

        let group = self
            .plan
            .authority()
            .authenticated_source_neutral_group_view(context)
            .map_err(|_| GeneratedAffineResidualGroupExactSessionError::GeometryAuthentication)?;
        let frame = self.plan.physical_frame();
        let matrix_entries = group
            .ambient_arity()
            .checked_mul(group.free_positions().len())
            .ok_or(GeneratedAffineResidualGroupExactSessionError::GeometryCountOverflow)?;
        if group.ordinal() != self.group_ordinal()
            || group.ambient_arity() != context.index_count()
            || group.ambient_arity() != frame.arity()
            || group.free_positions() != self.plan.free_positions()
            || group.compact_linear_coefficients().len() != matrix_entries
        {
            return Err(GeneratedAffineResidualGroupExactSessionError::MalformedGeometry);
        }
        let target_anchor = frame
            .anchor_offset(locator.inventory_position(), locator.case_ordinal())
            .map_err(|_| GeneratedAffineResidualGroupExactSessionError::GeometryAuthentication)?;
        ready
            .target_offset
            .authenticate_retained_census()
            .map_err(|_| GeneratedAffineResidualGroupExactSessionError::GeometryAuthentication)?;
        if target_anchor.values().len() != ready.target_offset.values().len()
            || !target_anchor
                .values()
                .iter()
                .zip(ready.target_offset.values())
                .all(|(left, right)| left.cmp(right).is_eq())
        {
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
        }

        Ok(GeneratedAffineResidualGroupExactSessionReadyGeometryView {
            frame,
            group,
            locator,
            target_anchor,
            target_offset: ready.target_offset.values(),
        })
    }

    /// Consume one staged new-pivot transaction into a sealed recenter
    /// typestate. Preparation borrows the transaction behind an unwind
    /// boundary; every ordinary error and every caught panic therefore returns
    /// the exact original token without reconstructing either authority.
    pub(crate) fn recenter_staged_new_pivot(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        transaction: GeneratedAffineResidualGroupExactSessionStagedTransaction,
    ) -> Result<
        GeneratedAffineResidualGroupExactSessionRecenterOutcome,
        GeneratedAffineResidualGroupExactSessionRecenterFailure,
    > {
        let prepared = catch_unwind(AssertUnwindSafe(|| {
            self.prepare_staged_new_pivot_recenter(family, context, &transaction)
        }));
        let prepared = match prepared {
            Ok(Ok(prepared)) => prepared,
            Ok(Err(error)) => {
                return Err(GeneratedAffineResidualGroupExactSessionRecenterFailure {
                    error,
                    transaction,
                });
            }
            Err(_) => {
                return Err(GeneratedAffineResidualGroupExactSessionRecenterFailure {
                    error: GeneratedAffineResidualGroupExactSessionRecenterError::Session(
                        GeneratedAffineResidualGroupExactSessionError::SymbolicaPanic,
                    ),
                    transaction,
                });
            }
        };

        Ok(match prepared {
            PreparedSessionRecenter::NoTarget {
                target_offset,
                source_ordinal,
                pivot_ordinal,
                stats,
            } => GeneratedAffineResidualGroupExactSessionRecenterOutcome::NoTarget(
                GeneratedAffineResidualGroupExactSessionRecenterNoTarget {
                    transaction,
                    target_offset,
                    source_ordinal,
                    pivot_ordinal,
                    stats,
                },
            ),
            PreparedSessionRecenter::RequiresAffineEqualityRefinement {
                target,
                target_offset,
                source_ordinal,
                pivot_ordinal,
                stats,
            } => GeneratedAffineResidualGroupExactSessionRecenterOutcome::RequiresAffineEqualityRefinement(
                GeneratedAffineResidualGroupExactSessionRecenterRequiresAffineEqualityRefinement {
                    transaction,
                    target,
                    target_offset,
                    source_ordinal,
                    pivot_ordinal,
                    stats,
                },
            ),
            PreparedSessionRecenter::Ready {
                target,
                target_offset,
                recentered,
                source_ordinal,
                pivot_ordinal,
                stats,
            } => GeneratedAffineResidualGroupExactSessionRecenterOutcome::Ready(
                GeneratedAffineResidualGroupExactSessionRecenterReady {
                    transaction,
                    target,
                    target_offset,
                    recentered,
                    source_ordinal,
                    pivot_ordinal,
                    stats,
                },
            ),
        })
    }

    fn prepare_staged_new_pivot_recenter(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        transaction: &GeneratedAffineResidualGroupExactSessionStagedTransaction,
    ) -> Result<PreparedSessionRecenter, GeneratedAffineResidualGroupExactSessionRecenterError>
    {
        let joint = self.authenticate_staged_new_pivot(family, context, transaction)?;
        let recenter_limits = self.limits.recenter;
        let kernel_limits = recenter_limits.kernel;
        let mut kernel = ExactRecenterKernelStats::for_row(
            joint.terms().len(),
            joint.guards().len(),
            kernel_limits,
        )?;
        let staged_live_prospective_retained_bytes = joint.staged_live_prospective_retained_bytes();
        let staged_live_observed_retained_bytes = joint.staged_live_observed_retained_bytes();
        let target_state_combined_retained_byte_envelope =
            joint.target_state_combined_retained_byte_envelope();
        let external_live_retained_bytes = checked_add(
            "exact session recenter external live retained bytes",
            checked_add(
                "exact session recenter external live retained bytes",
                staged_live_prospective_retained_bytes.max(staged_live_observed_retained_bytes),
                target_state_combined_retained_byte_envelope,
            )?,
            self.event_stats.ledger_retained_bytes,
        )?;
        let mut stats = GeneratedAffineResidualGroupExactSessionRecenterStats {
            staged_live_prospective_retained_bytes,
            staged_live_observed_retained_bytes,
            target_state_combined_retained_byte_envelope,
            external_live_retained_bytes,
            ..GeneratedAffineResidualGroupExactSessionRecenterStats::default()
        };

        // The key of an authenticated staged unit pivot is its post-top
        // leader. Geometry preflight performs no GMP construction. Admit the
        // complete outcome owner and all already-live session allocations
        // before materializing the target offset.
        let pivot = joint.key().shift();
        preflight_exact_geometry(
            pivot,
            joint.compact_affine_matrix(),
            joint.free_positions(),
            kernel_limits,
            &mut kernel,
        )?;
        // Every classified outcome retains the exact offset used for target
        // selection.  The geometry preflight's temporary-byte envelope is a
        // conservative upper bound for the retained offset payload. The
        // outcome wrapper already includes its inline Arc handle, while the
        // kernel census adds only the Arc pointee/control-block allocation.
        let target_offset_owner_bound = kernel.target_offset_arc_retained_bytes();
        admit_inert_owner(
            checked_add(
                "exact session recenter outcome retained bytes",
                size_of::<GeneratedAffineResidualGroupExactSessionRecenterOutcome>(),
                target_offset_owner_bound,
            )?,
            external_live_retained_bytes,
            0,
            true,
            kernel_limits,
            &mut kernel,
        )?;
        let target_offset = Arc::new(execute_target_offset(
            pivot,
            joint.compact_affine_matrix(),
            joint.free_positions(),
            joint.ambient_arity(),
        )?);
        verify_target_offset_census(&target_offset, &mut kernel)?;
        let target_offset_observed_retained_bytes = target_offset.arc_retained_bytes()?;
        observe_inert_owner(
            checked_add(
                "exact session observed recenter outcome retained bytes",
                size_of::<GeneratedAffineResidualGroupExactSessionRecenterOutcome>(),
                target_offset_observed_retained_bytes,
            )?,
            external_live_retained_bytes,
            &mut kernel,
        )?;

        let target_ordinals = joint.target_ordinals();
        if target_ordinals.start != 0 || target_ordinals.end != joint.target_locators().len() {
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch.into());
        }
        let mut selected = None;
        for solve_ordinal in target_ordinals {
            stats.target_scans = bounded_add(
                "exact session recenter target scans",
                stats.target_scans,
                1,
                recenter_limits.max_target_scans,
            )?;
            let locator = joint
                .target_locators()
                .get(solve_ordinal)
                .copied()
                .ok_or(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch)?;
            if locator.solve_ordinal() != solve_ordinal {
                return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch.into());
            }
            if !joint.is_target_unresolved(solve_ordinal)? {
                continue;
            }
            stats.unresolved_target_scans = checked_add(
                "exact session recenter unresolved target scans",
                stats.unresolved_target_scans,
                1,
            )?;
            if exact_offsets_equal(
                joint
                    .physical_frame()
                    .anchor_offset(locator.inventory_position(), locator.case_ordinal())?
                    .values(),
                target_offset.values(),
                kernel_limits,
                &mut kernel,
            )? {
                let retained = joint.retain_target(solve_ordinal)?;
                if retained.solve_ordinal() != solve_ordinal || retained.locator() != &locator {
                    return Err(
                        GeneratedAffineResidualGroupExactSessionError::ReplayMismatch.into(),
                    );
                }
                selected = Some(retained);
                break;
            }
        }

        let source_ordinal = joint.source_ordinal();
        let pivot_ordinal = joint.pivot_ordinal();
        let Some(target) = selected else {
            stats.kernel = kernel;
            return Ok(PreparedSessionRecenter::NoTarget {
                target_offset,
                source_ordinal,
                pivot_ordinal,
                stats,
            });
        };

        match target {
            GeneratedAffineResidualGroupRetainedExactTarget::RequiresAffineEqualityRefinement(
                target,
            ) => {
                // First-match semantics are final. Equality-bearing targets
                // return before coefficient, centered-shift, or guard
                // translation. They retain the exact matched offset for the
                // chronological event but never construct a recentered row.
                if !target.authenticates_source_state(&transaction.target_state) {
                    return Err(
                        GeneratedAffineResidualGroupExactSessionError::ReplayMismatch.into(),
                    );
                }
                stats.kernel = kernel;
                Ok(PreparedSessionRecenter::RequiresAffineEqualityRefinement {
                    target,
                    target_offset,
                    source_ordinal,
                    pivot_ordinal,
                    stats,
                })
            }
            GeneratedAffineResidualGroupRetainedExactTarget::Ready(target) => {
                if !target.authenticates_source_state(&transaction.target_state) {
                    return Err(
                        GeneratedAffineResidualGroupExactSessionError::ReplayMismatch.into(),
                    );
                }
                let locator_origin = GuardOrigin::GeneratedAffineGroupRecentering {
                    solve_group_ordinal: joint.group_ordinal(),
                    database_epoch: joint.database_epoch(),
                    event_ordinal: source_ordinal,
                };
                let recentered = translate_centered_row(
                    context,
                    joint
                        .terms()
                        .map(|(key, coefficient)| (key.shift(), coefficient)),
                    joint.guards().iter(),
                    pivot,
                    joint.free_positions(),
                    &locator_origin,
                    size_of::<GeneratedAffineResidualGroupExactSessionRecenterOutcome>(),
                    target_offset_owner_bound,
                    target_offset_observed_retained_bytes,
                    true,
                    external_live_retained_bytes,
                    0,
                    kernel_limits,
                    &mut kernel,
                )?;
                stats.kernel = recentered.stats();
                Ok(PreparedSessionRecenter::Ready {
                    target,
                    target_offset,
                    recentered,
                    source_ordinal,
                    pivot_ordinal,
                    stats,
                })
            }
        }
    }

    fn preflight_event_ledger_replacement(
        &self,
        source_ordinal: usize,
        head: GeneratedAffineResidualGroupExactSessionEventHeadView<'_>,
        disposition: GeneratedAffineResidualGroupExactSessionEventDispositionView<'_>,
    ) -> Result<PreparedSessionEventLedgerReplacement, GeneratedAffineResidualGroupExactSessionError>
    {
        (|| {
            let limits = self.limits.events;
            let event_ordinal = self.events.len();
            let event_count = session_event_bounded_add(
                "exact session committed events",
                event_ordinal,
                1,
                limits.max_events,
            )?;
            let target_state_successor_copies = session_event_bounded_add(
                "exact session target-state successor copies",
                self.event_stats.target_state_successor_copies,
                self.target_count(),
                limits.max_target_state_successor_copies,
            )?;
            let ledger_arc_copies = session_event_bounded_add(
                "exact session event-ledger Arc copies",
                self.event_stats.ledger_arc_copies,
                event_ordinal,
                limits.max_ledger_arc_copies,
            )?;
            if source_ordinal != event_ordinal {
                return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
            }

            let predecessor_state_version = self.database.state_version();
            let successor_state_version = session_event_checked_add(
                "exact session event successor version",
                predecessor_state_version,
                1,
            )?;

            let (
                source_recipe_allocation_comparisons,
                source_recipe_deep_bytes,
                unique_source_recipe_retained_bytes,
                reduction_steps,
                dependent_evidence_deep_bytes,
            ) = match head {
                GeneratedAffineResidualGroupExactSessionEventHeadView::Replayable {
                    source_recipe,
                    database_evidence,
                } => {
                    if source_recipe.database_epoch() != self.database_epoch()
                        || source_recipe.group_ordinal() != self.group_ordinal()
                    {
                        return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
                    }
                    let mut source_is_new = true;
                    let mut source_recipe_allocation_comparisons =
                        self.event_stats.source_recipe_allocation_comparisons;
                    for event in &self.events {
                        let GeneratedAffineResidualGroupExactSessionEventHead::Replayable {
                            source_recipe: prior_recipe,
                            ..
                        } = &event.head
                        else {
                            continue;
                        };
                        source_recipe_allocation_comparisons = session_event_bounded_add(
                            "exact session source-recipe allocation comparisons",
                            source_recipe_allocation_comparisons,
                            1,
                            limits.max_source_recipe_allocation_comparisons,
                        )?;
                        if prior_recipe.same_source_allocation(source_recipe) {
                            source_is_new = false;
                            break;
                        }
                    }
                    let source_recipe_deep_bytes = if source_is_new {
                        session_event_checked_sub(
                            "exact session source-recipe retained bytes",
                            source_recipe.retained_byte_bound()?,
                            size_of::<GeneratedAffineResidualGroupRetainedExactSourceRecipe>(),
                        )?
                    } else {
                        0
                    };
                    let unique_source_recipe_retained_bytes = session_event_bounded_add(
                        "exact session unique source-recipe retained bytes",
                        self.event_stats.unique_source_recipe_retained_bytes,
                        source_recipe_deep_bytes,
                        limits.max_source_recipe_retained_bytes,
                    )?;
                    let reduction_steps = session_event_bounded_add(
                        "exact session retained reduction steps",
                        self.event_stats.reduction_steps,
                        database_evidence.reduction_count(),
                        limits.max_reduction_steps,
                    )?;
                    let dependent_evidence_deep_bytes = match database_evidence {
                        GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence::Dependent(
                            evidence,
                        ) => evidence.retained_byte_bound()?,
                        GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence::NewPivot(
                            _,
                        ) => 0,
                    };
                    (
                        source_recipe_allocation_comparisons,
                        source_recipe_deep_bytes,
                        unique_source_recipe_retained_bytes,
                        reduction_steps,
                        dependent_evidence_deep_bytes,
                    )
                }
                GeneratedAffineResidualGroupExactSessionEventHeadView::Publication {
                    pivot_ordinal,
                } => {
                    if pivot_ordinal != self.database.pivot_count() {
                        return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
                    }
                    (
                        self.event_stats.source_recipe_allocation_comparisons,
                        0,
                        self.event_stats.unique_source_recipe_retained_bytes,
                        self.event_stats.reduction_steps,
                        0,
                    )
                }
            };

            let (
                new_target_offset_components,
                new_target_offset_integer_bits,
                new_target_offset_retained_bytes,
                new_equality_predicates,
                equality_predicate_buffer_bytes,
                new_publication_retained_bytes,
            ) = match disposition {
                GeneratedAffineResidualGroupExactSessionEventDispositionView::Dependent => {
                    if !head.is_replayable_dependent() {
                        return Err(
                            GeneratedAffineResidualGroupExactSessionError::ReplayMismatch,
                        );
                    }
                    (0, 0, 0, 0, 0, 0)
                }
                GeneratedAffineResidualGroupExactSessionEventDispositionView::NoTarget {
                    target_offset,
                } => {
                    if !head.is_replayable_new_pivot() {
                        return Err(
                            GeneratedAffineResidualGroupExactSessionError::ReplayMismatch,
                        );
                    }
                    (
                        target_offset.values().len(),
                        target_offset.retained_integer_bits(),
                        target_offset.arc_retained_bytes().map_err(|_| {
                            GeneratedAffineResidualGroupExactSessionError::EventCountOverflow {
                                resource: "exact session retained target-offset bytes",
                            }
                        })?,
                        0,
                        0,
                        0,
                    )
                }
                GeneratedAffineResidualGroupExactSessionEventDispositionView::RequiresAffineEqualityRefinement {
                    target_offset,
                    equality_predicate_ordinals,
                    equality_predicate_capacity,
                } => {
                    if !head.is_replayable_new_pivot() {
                        return Err(
                            GeneratedAffineResidualGroupExactSessionError::ReplayMismatch,
                        );
                    }
                    (
                        target_offset.values().len(),
                        target_offset.retained_integer_bits(),
                        target_offset.arc_retained_bytes().map_err(|_| {
                            GeneratedAffineResidualGroupExactSessionError::EventCountOverflow {
                                resource: "exact session retained target-offset bytes",
                            }
                        })?,
                        equality_predicate_ordinals.len(),
                        session_event_checked_mul(
                            "exact session retained equality-predicate bytes",
                            equality_predicate_capacity,
                            size_of::<usize>(),
                        )?,
                        0,
                    )
                }
                GeneratedAffineResidualGroupExactSessionEventDispositionView::Publication {
                    target_offset,
                    row,
                    publication,
                } => {
                    if !head.is_publication() {
                        return Err(
                            GeneratedAffineResidualGroupExactSessionError::ReplayMismatch,
                        );
                    }
                    let row_deep = row
                        .application_row_deep_owned_retained_byte_bound()
                        .map_err(|_| {
                            GeneratedAffineResidualGroupExactSessionError::EventCountOverflow {
                                resource: "exact session publication application-row bytes",
                            }
                        })?;
                    let payload_deep = publication.deep_owned_retained_byte_bound().map_err(|_| {
                        GeneratedAffineResidualGroupExactSessionError::EventCountOverflow {
                            resource: "exact session compact publication payload bytes",
                        }
                    })?;
                    (
                        target_offset.values().len(),
                        target_offset.retained_integer_bits(),
                        target_offset.arc_retained_bytes().map_err(|_| {
                            GeneratedAffineResidualGroupExactSessionError::EventCountOverflow {
                                resource: "exact session retained target-offset bytes",
                            }
                        })?,
                        0,
                        0,
                        session_event_checked_add(
                            "exact session retained publication bytes",
                            row_deep,
                            payload_deep,
                        )?,
                    )
                }
                #[cfg(test)]
                GeneratedAffineResidualGroupExactSessionEventDispositionView::TestSeedPivot => {
                    if !head.is_replayable_new_pivot() {
                        return Err(
                            GeneratedAffineResidualGroupExactSessionError::ReplayMismatch,
                        );
                    }
                    (0, 0, 0, 0, 0, 0)
                }
            };
            let target_offset_components = session_event_bounded_add(
                "exact session retained target-offset components",
                self.event_stats.target_offset_components,
                new_target_offset_components,
                limits.max_target_offset_components,
            )?;
            let target_offset_integer_bits = session_event_bounded_add(
                "exact session retained target-offset integer bits",
                self.event_stats.target_offset_integer_bits,
                new_target_offset_integer_bits,
                limits.max_target_offset_integer_bits,
            )?;
            let target_offset_retained_bytes = session_event_bounded_add(
                "exact session retained target-offset bytes",
                self.event_stats.target_offset_retained_bytes,
                new_target_offset_retained_bytes,
                limits.max_target_offset_retained_bytes,
            )?;
            let equality_predicates = session_event_bounded_add(
                "exact session retained equality predicates",
                self.event_stats.equality_predicates,
                new_equality_predicates,
                limits.max_equality_predicates,
            )?;
            let publication_retained_bytes = session_event_checked_add(
                "exact session retained publication bytes",
                self.event_stats.publication_retained_bytes,
                new_publication_retained_bytes,
            )?;

            let individual_event_retained_bytes = [
                session_event_arc_retained_bytes::<GeneratedAffineResidualGroupExactSessionEvent>(
                )?,
                source_recipe_deep_bytes,
                dependent_evidence_deep_bytes,
                new_target_offset_retained_bytes,
                equality_predicate_buffer_bytes,
                new_publication_retained_bytes,
            ]
            .into_iter()
            .try_fold(0usize, |total, bytes| {
                session_event_checked_add(
                    "exact session individual event retained bytes",
                    total,
                    bytes,
                )
            })?;
            session_event_check_limit(
                "exact session individual event retained bytes",
                individual_event_retained_bytes,
                limits.max_individual_event_retained_bytes,
            )?;

            // Never let a later ledger replacement have a smaller outer
            // capacity than its predecessor. This makes twice the final
            // authenticated ledger a sound upper bound for every historical
            // old-plus-replacement ledger coexistence, independently of the
            // allocator's `try_reserve_exact` growth policy.
            let replacement_capacity = event_count.max(self.events.capacity());
            let prospective_ledger_outer_buffer_bytes =
                session_event_outer_buffer_bytes(replacement_capacity)?;
            session_event_check_limit(
                "exact session event-ledger outer buffer bytes",
                prospective_ledger_outer_buffer_bytes,
                limits.max_ledger_outer_buffer_bytes,
            )?;
            let retained_without_old_outer = session_event_checked_sub(
                "exact session event-ledger retained bytes",
                self.event_stats.ledger_retained_bytes,
                self.event_stats.ledger_outer_buffer_bytes,
            )?;
            let prospective_ledger_retained_bytes = [
                retained_without_old_outer,
                prospective_ledger_outer_buffer_bytes,
                individual_event_retained_bytes,
            ]
            .into_iter()
            .try_fold(0usize, |total, bytes| {
                session_event_checked_add("exact session event-ledger retained bytes", total, bytes)
            })?;
            session_event_check_limit(
                "exact session event-ledger retained bytes",
                prospective_ledger_retained_bytes,
                limits.max_ledger_retained_bytes,
            )?;
            let prospective_current_replacement_peak_bytes = [
                self.event_stats.ledger_retained_bytes,
                prospective_ledger_outer_buffer_bytes,
                individual_event_retained_bytes,
            ]
            .into_iter()
            .try_fold(0usize, |total, bytes| {
                session_event_checked_add(
                    "exact session event-ledger replacement peak bytes",
                    total,
                    bytes,
                )
            })?;
            let prospective_ledger_replacement_peak_bytes = self
                .event_stats
                .ledger_replacement_peak_bytes
                .max(prospective_current_replacement_peak_bytes);
            session_event_check_limit(
                "exact session event-ledger replacement peak bytes",
                prospective_ledger_replacement_peak_bytes,
                limits.max_ledger_replacement_peak_bytes,
            )?;

            let mut replacement_events = Vec::new();
            #[cfg(test)]
            record_event_ledger_replacement_reservation_for_test();
            replacement_events
                .try_reserve_exact(replacement_capacity)
                .map_err(|_| {
                    GeneratedAffineResidualGroupExactSessionError::EventAllocationFailure {
                        resource: "exact session event-ledger replacement",
                    }
                })?;
            replacement_events.extend(self.events.iter().cloned());
            let ledger_outer_buffer_bytes =
                session_event_outer_buffer_bytes(replacement_events.capacity())?;
            if replacement_events.capacity() < replacement_capacity
                || replacement_events.len() != event_ordinal
            {
                return Err(
                    GeneratedAffineResidualGroupExactSessionError::EventAllocationFailure {
                        resource: "exact session event-ledger replacement",
                    },
                );
            }
            session_event_check_limit(
                "exact session event-ledger outer buffer bytes",
                ledger_outer_buffer_bytes,
                limits.max_ledger_outer_buffer_bytes,
            )?;
            let ledger_retained_bytes = [
                retained_without_old_outer,
                ledger_outer_buffer_bytes,
                individual_event_retained_bytes,
            ]
            .into_iter()
            .try_fold(0usize, |total, bytes| {
                session_event_checked_add("exact session event-ledger retained bytes", total, bytes)
            })?;
            session_event_check_limit(
                "exact session event-ledger retained bytes",
                ledger_retained_bytes,
                limits.max_ledger_retained_bytes,
            )?;
            let current_replacement_peak_bytes = [
                self.event_stats.ledger_retained_bytes,
                ledger_outer_buffer_bytes,
                individual_event_retained_bytes,
            ]
            .into_iter()
            .try_fold(0usize, |total, bytes| {
                session_event_checked_add(
                    "exact session event-ledger replacement peak bytes",
                    total,
                    bytes,
                )
            })?;
            let ledger_replacement_peak_bytes = self
                .event_stats
                .ledger_replacement_peak_bytes
                .max(current_replacement_peak_bytes);
            session_event_check_limit(
                "exact session event-ledger replacement peak bytes",
                ledger_replacement_peak_bytes,
                limits.max_ledger_replacement_peak_bytes,
            )?;

            Ok(PreparedSessionEventLedgerReplacement {
                event_ordinal,
                predecessor_state_version,
                successor_state_version,
                individual_event_retained_bytes,
                replacement_events,
                event_stats: GeneratedAffineResidualGroupExactSessionEventStats {
                    events: event_count,
                    source_recipe_allocation_comparisons,
                    target_state_successor_copies,
                    ledger_arc_copies,
                    reduction_steps,
                    unique_source_recipe_retained_bytes,
                    target_offset_components,
                    target_offset_integer_bits,
                    target_offset_retained_bytes,
                    equality_predicates,
                    publication_retained_bytes,
                    ledger_outer_buffer_bytes,
                    ledger_retained_bytes,
                    ledger_replacement_peak_bytes,
                },
            })
        })()
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_session_event_transition(
        &self,
        transaction: GeneratedAffineResidualGroupExactSessionStagedTransaction,
        successor: Arc<GeneratedAffineResidualGroupExactTargetState>,
        disposition: GeneratedAffineResidualGroupExactSessionEventDisposition,
        expected_source_ordinal: usize,
        expected: ExpectedSessionEventDatabaseOutcome,
    ) -> Result<
        PreparedSessionUnconsumedTransition,
        GeneratedAffineResidualGroupExactSessionEventPreparationFailure,
    > {
        let GeneratedAffineResidualGroupExactSessionStagedTransaction {
            staged,
            target_state: transaction_target_state,
        } = transaction;
        if transaction_target_state.state_version().checked_add(1)
            != Some(successor.state_version())
            || transaction_target_state.database_epoch() != successor.database_epoch()
            || transaction_target_state.group_ordinal() != successor.group_ordinal()
        {
            return Err(
                GeneratedAffineResidualGroupExactSessionEventPreparationFailure {
                    error: GeneratedAffineResidualGroupExactSessionError::ReplayMismatch,
                    transaction: GeneratedAffineResidualGroupExactSessionStagedTransaction {
                        staged,
                        target_state: transaction_target_state,
                    },
                },
            );
        }
        let database_commit = match self
            .database
            .prepare_staged_row_commit_for_session(&self.database_capability, staged)
        {
            Ok(prepared) => prepared,
            Err(failure) => {
                let error = failure.error();
                return Err(
                    GeneratedAffineResidualGroupExactSessionEventPreparationFailure {
                        error: GeneratedAffineResidualGroupExactSessionError::Database(error),
                        transaction: GeneratedAffineResidualGroupExactSessionStagedTransaction {
                            staged: failure.into_staged(),
                            target_state: transaction_target_state,
                        },
                    },
                );
            }
        };
        let source_ordinal = database_commit.source_ordinal();
        let source_recipe =
            database_commit.retain_source_recipe_for_session(&self.database_capability);
        let database_evidence = match &disposition {
            GeneratedAffineResidualGroupExactSessionEventDisposition::Dependent => database_commit
                .retain_dependent_evidence_for_session(&self.database_capability)
                .map(GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence::Dependent),
            GeneratedAffineResidualGroupExactSessionEventDisposition::NoTarget { .. }
            | GeneratedAffineResidualGroupExactSessionEventDisposition::RequiresAffineEqualityRefinement { .. } => database_commit
                .retain_new_pivot_evidence_for_session(&self.database_capability)
                .map(GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence::NewPivot),
            GeneratedAffineResidualGroupExactSessionEventDisposition::Publication { .. } => {
                unreachable!("compact publication uses its dedicated transition")
            }
            #[cfg(test)]
            GeneratedAffineResidualGroupExactSessionEventDisposition::TestSeedPivot => database_commit
                .retain_new_pivot_evidence_for_session(&self.database_capability)
                .map(GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence::NewPivot),
        };
        let evidence_matches = source_ordinal == expected_source_ordinal
            && match (&database_evidence, expected) {
                (
                    Some(GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence::Dependent(
                        _,
                    )),
                    ExpectedSessionEventDatabaseOutcome::Dependent { reduction_count },
                ) => database_evidence
                    .as_ref()
                    .is_some_and(|evidence| evidence.reduction_count() == reduction_count),
                (
                    Some(GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence::NewPivot(
                        evidence,
                    )),
                    ExpectedSessionEventDatabaseOutcome::NewPivot { pivot_ordinal },
                ) => {
                    evidence.ordinal() == pivot_ordinal
                        && evidence.source_ordinal() == source_ordinal
                }
                _ => false,
            };
        let Some(database_evidence) = database_evidence.filter(|_| evidence_matches) else {
            let staged = self.database.abort_prepared_staged_row_commit_for_session(
                &self.database_capability,
                database_commit,
            );
            return Err(
                GeneratedAffineResidualGroupExactSessionEventPreparationFailure {
                    error: GeneratedAffineResidualGroupExactSessionError::ReplayMismatch,
                    transaction: GeneratedAffineResidualGroupExactSessionStagedTransaction {
                        staged,
                        target_state: transaction_target_state,
                    },
                },
            );
        };
        let ledger_preflight = catch_unwind(AssertUnwindSafe(|| {
            self.preflight_event_ledger_replacement(
                source_ordinal,
                GeneratedAffineResidualGroupExactSessionEventHeadView::Replayable {
                    source_recipe: &source_recipe,
                    database_evidence: &database_evidence,
                },
                disposition.view(),
            )
        }));
        let ledger_preflight = match ledger_preflight {
            Ok(Ok(prepared)) => prepared,
            Ok(Err(error)) => {
                let staged = self.database.abort_prepared_staged_row_commit_for_session(
                    &self.database_capability,
                    database_commit,
                );
                return Err(
                    GeneratedAffineResidualGroupExactSessionEventPreparationFailure {
                        error,
                        transaction: GeneratedAffineResidualGroupExactSessionStagedTransaction {
                            staged,
                            target_state: transaction_target_state,
                        },
                    },
                );
            }
            Err(_) => {
                let staged = self.database.abort_prepared_staged_row_commit_for_session(
                    &self.database_capability,
                    database_commit,
                );
                return Err(
                    GeneratedAffineResidualGroupExactSessionEventPreparationFailure {
                        error: GeneratedAffineResidualGroupExactSessionError::SymbolicaPanic,
                        transaction: GeneratedAffineResidualGroupExactSessionStagedTransaction {
                            staged,
                            target_state: transaction_target_state,
                        },
                    },
                );
            }
        };
        let PreparedSessionEventLedgerReplacement {
            event_ordinal,
            predecessor_state_version,
            successor_state_version,
            individual_event_retained_bytes,
            mut replacement_events,
            event_stats,
        } = ledger_preflight;
        let event = Arc::new(GeneratedAffineResidualGroupExactSessionEvent {
            authority: Arc::clone(&self.event_authority),
            event_ordinal,
            source_ordinal,
            predecessor_state_version,
            successor_state_version,
            head: GeneratedAffineResidualGroupExactSessionEventHead::Replayable {
                source_recipe,
                database_evidence,
            },
            disposition,
            retained_bytes: individual_event_retained_bytes,
        });
        replacement_events.push(Arc::clone(&event));
        Ok(PreparedSessionUnconsumedTransition {
            successor,
            transaction_target_state,
            database_commit,
            event,
            replacement_events,
            event_stats,
        })
    }

    /// Atomically commit one already-derived compact application event through
    /// the ordinary exclusive session API. The commit boundary performs exactly
    /// one combined live check (current target-state Arc plus current staged
    /// database head), then only checked resource/allocation preparation.  It
    /// invokes no Symbolica operation and does not replay the derivation.
    pub(crate) fn commit_publication(
        &mut self,
        publication: PreparedPublication,
    ) -> Result<PublicationReceipt, PublicationCommitFailure> {
        let prepared = match self.prepare_publication_transition(&publication) {
            Ok(prepared) => prepared,
            Err(error) => {
                return Err(PublicationCommitFailure { error, publication });
            }
        };

        let PreparedSessionPublicationTransition {
            successor,
            ledger,
            source_ordinal,
            pivot_ordinal,
        } = prepared;
        let publication_stats = publication.stats();
        let (ready, publication, pivot_term_ordinal) = publication.into_parts_for_session();
        let GeneratedAffineResidualGroupExactSessionRecenterReady {
            transaction,
            target,
            target_offset,
            recentered,
            source_ordinal: ready_source_ordinal,
            pivot_ordinal: ready_pivot_ordinal,
            stats: _,
        } = ready;
        debug_assert_eq!(ready_source_ordinal, source_ordinal);
        debug_assert_eq!(ready_pivot_ordinal, pivot_ordinal);
        let locator = *target.locator();
        let row = recentered.into_application_row(pivot_term_ordinal);
        let GeneratedAffineResidualGroupExactSessionStagedTransaction {
            staged,
            target_state: transaction_target_state,
        } = transaction;
        let PreparedSessionEventLedgerReplacement {
            event_ordinal,
            predecessor_state_version,
            successor_state_version,
            individual_event_retained_bytes,
            mut replacement_events,
            event_stats,
        } = ledger;
        let event = Arc::new(GeneratedAffineResidualGroupExactSessionEvent {
            authority: Arc::clone(&self.event_authority),
            event_ordinal,
            source_ordinal,
            predecessor_state_version,
            successor_state_version,
            head: GeneratedAffineResidualGroupExactSessionEventHead::Publication { pivot_ordinal },
            disposition: GeneratedAffineResidualGroupExactSessionEventDisposition::Publication {
                target_offset,
                locator,
                row,
                publication,
            },
            retained_bytes: individual_event_retained_bytes,
        });
        replacement_events.push(Arc::clone(&event));
        drop(target);
        self.database
            .commit_current_staged_new_pivot_for_session(&self.database_capability, staged);
        let prior_target_state = std::mem::replace(&mut self.target_state, successor);
        let prior_events = std::mem::replace(&mut self.events, replacement_events);
        self.event_stats = event_stats;
        drop(transaction_target_state);
        drop(prior_target_state);
        drop(prior_events);
        let event = CommittedPublicationEventHandle { event };
        Ok(PublicationReceipt {
            event_ordinal,
            source_ordinal,
            pivot_ordinal,
            retained_event_bytes: individual_event_retained_bytes,
            stats: publication_stats,
            event,
        })
    }

    fn current_publication_pivot<'a>(
        &'a self,
        publication: &'a PreparedPublication,
    ) -> Result<
        GeneratedAffineResidualGroupAuthenticatedStagedNewPivotView<'a>,
        GeneratedAffineResidualGroupExactSessionError,
    > {
        let ready = publication.ready();
        if !Arc::ptr_eq(&ready.transaction.target_state, &self.target_state) {
            return Err(GeneratedAffineResidualGroupExactSessionError::WrongTargetStateAllocation);
        }
        self.database
            .authenticate_staged_new_pivot_for_session(
                &self.database_capability,
                &ready.transaction.staged,
            )
            .map_err(|_| GeneratedAffineResidualGroupExactSessionError::WrongTargetStateAllocation)
    }

    fn prepare_publication_transition(
        &self,
        publication: &PreparedPublication,
    ) -> Result<PreparedSessionPublicationTransition, GeneratedAffineResidualGroupExactSessionError>
    {
        let ready = publication.ready();

        // One recoverable helper checks the predecessor allocation and staged
        // database head together. The resulting borrow mints every shallow
        // owner needed by the preflight below.
        let pivot = self.current_publication_pivot(publication)?;
        let source_ordinal = pivot.source_ordinal();
        let pivot_ordinal = pivot.pivot_ordinal();
        let successor_binding =
            pivot.successor_target_state_binding_for_session(&self.database_capability);
        debug_assert_eq!(source_ordinal, ready.source_ordinal);
        debug_assert_eq!(pivot_ordinal, ready.pivot_ordinal);
        debug_assert!(
            ready
                .target
                .authenticates_source_state(&ready.transaction.target_state),
            "sealed publication target changed source state"
        );

        self.preflight_target_state_successor_copy_work()?;
        let ledger = self.preflight_event_ledger_replacement(
            source_ordinal,
            GeneratedAffineResidualGroupExactSessionEventHeadView::Publication { pivot_ordinal },
            GeneratedAffineResidualGroupExactSessionEventDispositionView::Publication {
                target_offset: &ready.target_offset,
                row: &ready.recentered,
                publication: publication.payload(),
            },
        )?;
        // Last fallible operation: its process-global allocation nonce is not
        // consumed before any later recoverable limit/allocation branch.
        let successor = ready
            .transaction
            .target_state
            .prepare_publication_successor(successor_binding, &ready.target)?;
        debug_assert_eq!(successor.state_version(), ledger.successor_state_version);
        Ok(PreparedSessionPublicationTransition {
            successor,
            ledger,
            source_ordinal,
            pivot_ordinal,
        })
    }

    /// Commit one sealed NoTarget recenter outcome without consuming a solve
    /// target, publishing a rule, or inferring a master.
    ///
    /// The outcome's exact staged-pivot coordinates and complete successor
    /// target state are authenticated and prepared before the database may
    /// mutate. The running session is consumed structurally: every preflight
    /// failure returns it with the original outcome, while success returns the
    /// sole authorized continuation owner.
    pub(crate) fn commit_no_target(
        mut self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        outcome: GeneratedAffineResidualGroupExactSessionRecenterNoTarget,
    ) -> Result<
        GeneratedAffineResidualGroupExactSessionCommittedNoTarget,
        GeneratedAffineResidualGroupExactSessionCommitNoTargetFailure,
    > {
        let successor = match catch_unwind(AssertUnwindSafe(|| {
            self.prepare_no_target_successor(family, context, &outcome)
        })) {
            Ok(Ok(successor)) => successor,
            Ok(Err(error)) => {
                return Err(
                    GeneratedAffineResidualGroupExactSessionCommitNoTargetFailure::Preflight {
                        error,
                        session: self,
                        outcome,
                    },
                );
            }
            Err(_) => {
                return Err(
                    GeneratedAffineResidualGroupExactSessionCommitNoTargetFailure::Preflight {
                        error: GeneratedAffineResidualGroupExactSessionError::SymbolicaPanic,
                        session: self,
                        outcome,
                    },
                );
            }
        };
        let GeneratedAffineResidualGroupExactSessionRecenterNoTarget {
            transaction,
            target_offset,
            source_ordinal,
            pivot_ordinal,
            stats,
        } = outcome;
        let prepared = match self.prepare_session_event_transition(
            transaction,
            successor,
            GeneratedAffineResidualGroupExactSessionEventDisposition::NoTarget {
                target_offset: Arc::clone(&target_offset),
                stats,
            },
            source_ordinal,
            ExpectedSessionEventDatabaseOutcome::NewPivot { pivot_ordinal },
        ) {
            Ok(prepared) => prepared,
            Err(failure) => {
                return Err(
                    GeneratedAffineResidualGroupExactSessionCommitNoTargetFailure::Preflight {
                        error: failure.error,
                        session: self,
                        outcome: GeneratedAffineResidualGroupExactSessionRecenterNoTarget {
                            transaction: failure.transaction,
                            target_offset,
                            source_ordinal,
                            pivot_ordinal,
                            stats,
                        },
                    },
                );
            }
        };
        let event = self.commit_prepared_unconsumed(prepared);
        drop(target_offset);
        Ok(GeneratedAffineResidualGroupExactSessionCommittedNoTarget {
            session: self,
            event,
            source_ordinal,
            pivot_ordinal,
            stats,
        })
    }

    fn prepare_no_target_successor(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        outcome: &GeneratedAffineResidualGroupExactSessionRecenterNoTarget,
    ) -> Result<
        Arc<GeneratedAffineResidualGroupExactTargetState>,
        GeneratedAffineResidualGroupExactSessionError,
    > {
        let staged_pivot =
            self.authenticate_staged_new_pivot(family, context, &outcome.transaction)?;
        if staged_pivot.source_ordinal() != outcome.source_ordinal
            || staged_pivot.pivot_ordinal() != outcome.pivot_ordinal
        {
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
        }
        drop(staged_pivot);
        self.prepare_unchanged_target_successor(family, context, &outcome.transaction)
    }

    /// Commit an equality-bearing pivot and consume the running session into a
    /// sealed owner that must wait for a refined solve epoch.
    ///
    /// This transition deliberately does not treat the equality target as
    /// IdenticallyBad or unsupported, and it exposes no way to continue staging
    /// work in the committed epoch. The target remains unresolved and is
    /// rebound to the exact successor state before database mutation.
    pub(crate) fn commit_and_suspend_affine_equality_refinement(
        mut self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        outcome: GeneratedAffineResidualGroupExactSessionRecenterRequiresAffineEqualityRefinement,
    ) -> Result<
        GeneratedAffineResidualGroupExactSessionSuspendedForRefinedEpoch,
        GeneratedAffineResidualGroupExactSessionSuspendRefinedEpochFailure,
    > {
        let prepared = match catch_unwind(AssertUnwindSafe(|| {
            self.prepare_equality_refinement_suspension(family, context, &outcome)
        })) {
            Ok(Ok(prepared)) => prepared,
            Ok(Err(error)) => {
                return Err(
                    GeneratedAffineResidualGroupExactSessionSuspendRefinedEpochFailure::Preflight {
                        error,
                        session: self,
                        outcome,
                    },
                );
            }
            Err(_) => {
                return Err(
                    GeneratedAffineResidualGroupExactSessionSuspendRefinedEpochFailure::Preflight {
                        error: GeneratedAffineResidualGroupExactSessionError::SymbolicaPanic,
                        session: self,
                        outcome,
                    },
                );
            }
        };
        let PreparedSessionEqualityRefinementSuspension {
            successor,
            target: successor_target,
            locator,
            equality_predicate_ordinals,
        } = prepared;
        let GeneratedAffineResidualGroupExactSessionRecenterRequiresAffineEqualityRefinement {
            transaction,
            target: predecessor_target,
            target_offset,
            source_ordinal,
            pivot_ordinal,
            stats,
        } = outcome;
        let prepared = match self.prepare_session_event_transition(
            transaction,
            successor,
            GeneratedAffineResidualGroupExactSessionEventDisposition::RequiresAffineEqualityRefinement {
                target_offset: Arc::clone(&target_offset),
                locator,
                equality_predicate_ordinals,
                stats,
            },
            source_ordinal,
            ExpectedSessionEventDatabaseOutcome::NewPivot { pivot_ordinal },
        ) {
            Ok(prepared) => prepared,
            Err(failure) => {
                return Err(
                    GeneratedAffineResidualGroupExactSessionSuspendRefinedEpochFailure::Preflight {
                        error: failure.error,
                        session: self,
                        outcome: GeneratedAffineResidualGroupExactSessionRecenterRequiresAffineEqualityRefinement {
                            transaction: failure.transaction,
                            target: predecessor_target,
                            target_offset,
                            source_ordinal,
                            pivot_ordinal,
                            stats,
                        },
                    },
                );
            }
        };
        let event = self.commit_prepared_unconsumed(prepared);
        drop(predecessor_target);
        drop(target_offset);
        Ok(
            GeneratedAffineResidualGroupExactSessionSuspendedForRefinedEpoch {
                committed_session: self,
                event,
                target: successor_target,
                source_ordinal,
                pivot_ordinal,
                stats,
            },
        )
    }

    fn prepare_equality_refinement_suspension(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        outcome: &GeneratedAffineResidualGroupExactSessionRecenterRequiresAffineEqualityRefinement,
    ) -> Result<
        PreparedSessionEqualityRefinementSuspension,
        GeneratedAffineResidualGroupExactSessionError,
    > {
        let staged_pivot =
            self.authenticate_staged_new_pivot(family, context, &outcome.transaction)?;
        if staged_pivot.source_ordinal() != outcome.source_ordinal
            || staged_pivot.pivot_ordinal() != outcome.pivot_ordinal
        {
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
        }
        self.preflight_target_state_successor_copy_work()?;
        let locator = *outcome.target.locator();
        let equality_predicates = outcome.refinement().equality_predicate_ordinals();
        session_event_check_limit(
            "exact session equality predicates",
            equality_predicates.len(),
            self.limits.events.max_equality_predicates,
        )?;
        let mut equality_predicate_ordinals = Vec::new();
        equality_predicate_ordinals
            .try_reserve_exact(equality_predicates.len())
            .map_err(
                |_| GeneratedAffineResidualGroupExactSessionError::EventAllocationFailure {
                    resource: "exact session equality-predicate manifest",
                },
            )?;
        equality_predicate_ordinals.extend_from_slice(equality_predicates);
        drop(staged_pivot);
        self.authenticate_target_state_allocation(&outcome.transaction.target_state)?;
        self.database
            .authenticate_target_state_binding(outcome.transaction.target_state.binding())?;
        let successor_binding = self.database.successor_target_state_binding_for_session(
            &self.database_capability,
            &outcome.transaction.staged,
        )?;
        let target_successor = outcome
            .transaction
            .target_state
            .prepare_equality_refinement_successor(
                family,
                context,
                successor_binding,
                &outcome.target,
            )?;
        let (successor, target) =
            target_successor.into_parts_for_session(&self.database_capability);
        if target.locator() != &locator
            || target.refinement().equality_predicate_ordinals()
                != equality_predicate_ordinals.as_slice()
        {
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
        }
        Ok(PreparedSessionEqualityRefinementSuspension {
            successor,
            target,
            locator,
            equality_predicate_ordinals,
        })
    }

    /// Consume one raw staged transaction into a non-forgeable dependent
    /// classification. Classification authenticates both retained owners and
    /// the database payload, mutates nothing, and returns the intact
    /// transaction on every rejection (including new-pivot, stale, and
    /// foreign cases).
    pub(crate) fn classify_dependent(
        &self,
        transaction: GeneratedAffineResidualGroupExactSessionStagedTransaction,
    ) -> Result<
        GeneratedAffineResidualGroupExactSessionClassifiedDependent,
        GeneratedAffineResidualGroupExactSessionDependentClassificationFailure,
    > {
        let classification = (|| {
            self.authenticate_target_state_allocation(&transaction.target_state)?;
            self.database
                .authenticate_target_state_binding(transaction.target_state.binding())?;
            let dependent = self.database.authenticate_staged_dependent_for_session(
                &self.database_capability,
                &transaction.staged,
            )?;
            Ok((dependent.source_ordinal(), dependent.reductions().len()))
        })();
        match classification {
            Ok((source_ordinal, reduction_count)) => Ok(
                GeneratedAffineResidualGroupExactSessionClassifiedDependent {
                    transaction,
                    source_ordinal,
                    reduction_count,
                },
            ),
            Err(error) => Err(
                GeneratedAffineResidualGroupExactSessionDependentClassificationFailure {
                    error,
                    transaction,
                },
            ),
        }
    }

    /// Commit one sealed dependent row without consuming a solve target.
    ///
    /// This is the only crate-visible route from a dependent session
    /// transaction to the private common transition kernel. A failure before
    /// database commit returns the complete classified transaction.
    pub(crate) fn commit_dependent(
        &mut self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        classified: GeneratedAffineResidualGroupExactSessionClassifiedDependent,
    ) -> Result<
        GeneratedAffineResidualGroupExactSessionCommittedDependent,
        GeneratedAffineResidualGroupExactSessionCommitDependentFailure,
    > {
        let successor = match catch_unwind(AssertUnwindSafe(|| {
            self.prepare_dependent_successor(family, context, &classified)
        })) {
            Ok(Ok(successor)) => successor,
            Ok(Err(error)) => {
                return Err(
                    GeneratedAffineResidualGroupExactSessionCommitDependentFailure::Preflight {
                        error,
                        classified,
                    },
                );
            }
            Err(_) => {
                return Err(
                    GeneratedAffineResidualGroupExactSessionCommitDependentFailure::Preflight {
                        error: GeneratedAffineResidualGroupExactSessionError::SymbolicaPanic,
                        classified,
                    },
                );
            }
        };
        let GeneratedAffineResidualGroupExactSessionClassifiedDependent {
            transaction,
            source_ordinal: classified_source_ordinal,
            reduction_count: classified_reduction_count,
        } = classified;
        let prepared = match self.prepare_session_event_transition(
            transaction,
            successor,
            GeneratedAffineResidualGroupExactSessionEventDisposition::Dependent,
            classified_source_ordinal,
            ExpectedSessionEventDatabaseOutcome::Dependent {
                reduction_count: classified_reduction_count,
            },
        ) {
            Ok(prepared) => prepared,
            Err(failure) => {
                return Err(
                    GeneratedAffineResidualGroupExactSessionCommitDependentFailure::Preflight {
                        error: failure.error,
                        classified: GeneratedAffineResidualGroupExactSessionClassifiedDependent {
                            transaction: failure.transaction,
                            source_ordinal: classified_source_ordinal,
                            reduction_count: classified_reduction_count,
                        },
                    },
                );
            }
        };
        let event = self.commit_prepared_unconsumed(prepared);
        Ok(GeneratedAffineResidualGroupExactSessionCommittedDependent { event })
    }

    fn prepare_dependent_successor(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        classified: &GeneratedAffineResidualGroupExactSessionClassifiedDependent,
    ) -> Result<
        Arc<GeneratedAffineResidualGroupExactTargetState>,
        GeneratedAffineResidualGroupExactSessionError,
    > {
        let successor =
            self.prepare_unchanged_target_successor(family, context, &classified.transaction)?;
        let dependent = self.database.authenticate_staged_dependent_for_session(
            &self.database_capability,
            &classified.transaction.staged,
        )?;
        if dependent.source_ordinal() != classified.source_ordinal
            || dependent.reductions().len() != classified.reduction_count
        {
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
        }
        drop(dependent);
        Ok(successor)
    }

    /// Commit a staged row without consuming any solve target.
    ///
    /// This is the common transition for dependent rows and for new pivots
    /// that produce no target, require affine-equality refinement, or are
    /// rejected/unsupported by a later `WhenBad` policy. It never publishes a
    /// rule or infers a master. The complete successor target state is built
    /// fallibly before database mutation; after database commit, installing
    /// the prebuilt `Arc` is an allocation-free move. This untyped kernel is
    /// intentionally module-private: exposing it directly would let a caller
    /// skip recentering/`WhenBad` and advance an arbitrary new pivot. The
    /// crate-visible wrappers require their corresponding sealed
    /// classification or recenter outcome.
    #[cfg(test)]
    fn commit_unconsumed(
        &mut self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        transaction: GeneratedAffineResidualGroupExactSessionStagedTransaction,
    ) -> Result<
        GeneratedAffineResidualGroupExactRowOutcome,
        GeneratedAffineResidualGroupExactSessionCommitUnconsumedFailure,
    > {
        let preflight = match catch_unwind(AssertUnwindSafe(|| {
            let successor =
                self.prepare_unchanged_target_successor(family, context, &transaction)?;
            match self.database.authenticate_staged_new_pivot_for_session(
                &self.database_capability,
                &transaction.staged,
            ) {
                Ok(pivot) => Ok((
                    successor,
                    GeneratedAffineResidualGroupExactSessionEventDisposition::TestSeedPivot,
                    pivot.source_ordinal(),
                    ExpectedSessionEventDatabaseOutcome::NewPivot {
                        pivot_ordinal: pivot.pivot_ordinal(),
                    },
                    GeneratedAffineResidualGroupExactRowOutcome::NewPivot {
                        source_ordinal: pivot.source_ordinal(),
                        pivot_ordinal: pivot.pivot_ordinal(),
                    },
                )),
                Err(new_pivot_error) => {
                    let dependent = self
                        .database
                        .authenticate_staged_dependent_for_session(
                            &self.database_capability,
                            &transaction.staged,
                        )
                        .map_err(|_| {
                            GeneratedAffineResidualGroupExactSessionError::Database(new_pivot_error)
                        })?;
                    let reductions = dependent.reductions().to_vec();
                    Ok((
                        successor,
                        GeneratedAffineResidualGroupExactSessionEventDisposition::Dependent,
                        dependent.source_ordinal(),
                        ExpectedSessionEventDatabaseOutcome::Dependent {
                            reduction_count: reductions.len(),
                        },
                        GeneratedAffineResidualGroupExactRowOutcome::Dependent {
                            source_ordinal: dependent.source_ordinal(),
                            reductions,
                        },
                    ))
                }
            }
        })) {
            Ok(Ok(preflight)) => preflight,
            Ok(Err(error)) => {
                return Err(
                    GeneratedAffineResidualGroupExactSessionCommitUnconsumedFailure::Preflight {
                        error,
                        transaction,
                    },
                );
            }
            Err(_) => {
                return Err(
                    GeneratedAffineResidualGroupExactSessionCommitUnconsumedFailure::Preflight {
                        error: GeneratedAffineResidualGroupExactSessionError::SymbolicaPanic,
                        transaction,
                    },
                );
            }
        };
        let (successor, disposition, source_ordinal, expected, outcome) = preflight;
        let prepared = match self.prepare_session_event_transition(
            transaction,
            successor,
            disposition,
            source_ordinal,
            expected,
        ) {
            Ok(prepared) => prepared,
            Err(failure) => {
                return Err(
                    GeneratedAffineResidualGroupExactSessionCommitUnconsumedFailure::Preflight {
                        error: failure.error,
                        transaction: failure.transaction,
                    },
                );
            }
        };
        drop(self.commit_prepared_unconsumed(prepared));
        Ok(outcome)
    }

    /// Finalize a completely prepared unconsumed transition.
    ///
    /// The database repeats its token authentication before its own first
    /// mutation and reports any rejection as a fail-stop invariant. Once that
    /// call succeeds, the session tail contains only preallocated moves and
    /// infallible drops; it performs no replay, allocation, or authentication.
    fn commit_prepared_unconsumed(
        &mut self,
        prepared: PreparedSessionUnconsumedTransition,
    ) -> Arc<GeneratedAffineResidualGroupExactSessionEvent> {
        let PreparedSessionUnconsumedTransition {
            successor,
            transaction_target_state,
            database_commit,
            event,
            replacement_events,
            event_stats,
        } = prepared;
        self.database
            .commit_prepared_staged_row_for_session(&self.database_capability, database_commit);

        // Infallible, allocation-free commit tail. The old target state
        // stays live through `transaction_target_state` until both retained
        // owners have advanced coherently.
        let prior_target_state = std::mem::replace(&mut self.target_state, successor);
        let prior_events = std::mem::replace(&mut self.events, replacement_events);
        self.event_stats = event_stats;
        drop(transaction_target_state);
        drop(prior_target_state);
        drop(prior_events);
        event
    }

    fn prepare_unchanged_target_successor(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        transaction: &GeneratedAffineResidualGroupExactSessionStagedTransaction,
    ) -> Result<
        Arc<GeneratedAffineResidualGroupExactTargetState>,
        GeneratedAffineResidualGroupExactSessionError,
    > {
        self.authenticate_target_state_allocation(&transaction.target_state)?;
        self.database
            .authenticate_target_state_binding(transaction.target_state.binding())?;
        self.preflight_target_state_successor_copy_work()?;
        let successor_binding = self.database.successor_target_state_binding_for_session(
            &self.database_capability,
            &transaction.staged,
        )?;
        transaction
            .target_state
            .prepare_successor(family, context, successor_binding, None)
            .map_err(GeneratedAffineResidualGroupExactSessionError::from)
    }

    fn preflight_target_state_successor_copy_work(
        &self,
    ) -> Result<(), GeneratedAffineResidualGroupExactSessionError> {
        session_event_bounded_add(
            "exact session target-state successor copies",
            self.event_stats.target_state_successor_copies,
            self.target_count(),
            self.limits.events.max_target_state_successor_copies,
        )?;
        Ok(())
    }

    fn authenticate_target_state_allocation(
        &self,
        target_state: &Arc<GeneratedAffineResidualGroupExactTargetState>,
    ) -> Result<(), GeneratedAffineResidualGroupExactSessionError> {
        if !Arc::ptr_eq(target_state, &self.target_state) {
            return Err(GeneratedAffineResidualGroupExactSessionError::WrongTargetStateAllocation);
        }
        Ok(())
    }
}

/// Consume-once staged database row inseparably paired with its target state.
///
/// This type is intentionally neither `Clone` nor decomposable outside this
/// module. The private atomic kernel may consume it exactly once; every
/// crate-visible transition additionally requires a sealed classification or
/// policy outcome.
pub(crate) struct GeneratedAffineResidualGroupExactSessionStagedTransaction {
    staged: GeneratedAffineResidualGroupStagedExactRow,
    target_state: Arc<GeneratedAffineResidualGroupExactTargetState>,
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionStagedTransaction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionStagedTransaction")
            .field("database_epoch", &self.target_state.database_epoch())
            .field("group_ordinal", &self.target_state.group_ordinal())
            .field("state_version", &self.target_state.state_version())
            .field("private_database_stage", &"<redacted>")
            .field("private_target_state", &"<redacted>")
            .finish()
    }
}

/// Sealed simultaneous borrow of a database-authenticated new pivot and the
/// exact unresolved targets belonging to the same live session state.
struct GeneratedAffineResidualGroupExactSessionStagedNewPivotView<'a> {
    staged_pivot: GeneratedAffineResidualGroupAuthenticatedStagedNewPivotView<'a>,
    targets: GeneratedAffineResidualGroupExactTargetStateView<'a>,
    anchor_case_ordinal: usize,
    free_positions: &'a [usize],
    target_locators: &'a [GeneratedAffineResidualGroupSolveTargetLocator],
    ambient_arity: usize,
    compact_affine_matrix: &'a [Integer],
    staged_live_prospective_retained_bytes: usize,
    staged_live_observed_retained_bytes: usize,
    target_state_combined_retained_byte_envelope: usize,
}

impl<'a> GeneratedAffineResidualGroupExactSessionStagedNewPivotView<'a> {
    fn key(&self) -> &'a GeneratedAffineResidualGroupPhysicalKey {
        self.staged_pivot.key()
    }

    fn terms(
        &self,
    ) -> impl ExactSizeIterator<
        Item = (
            &'a GeneratedAffineResidualGroupPhysicalKey,
            &'a ParametricCoefficient,
        ),
    > + DoubleEndedIterator
    + 'a {
        self.staged_pivot.terms()
    }

    fn guards(&self) -> &'a [ParametricNonZeroCondition] {
        self.staged_pivot.guards()
    }

    fn reductions(&self) -> &'a [GeneratedAffineResidualGroupExactReductionStep] {
        self.staged_pivot.reductions()
    }

    const fn normalization_divisor(&self) -> &'a ParametricCoefficient {
        self.staged_pivot.normalization_divisor()
    }

    const fn source_ordinal(&self) -> usize {
        self.staged_pivot.source_ordinal()
    }

    const fn pivot_ordinal(&self) -> usize {
        self.staged_pivot.pivot_ordinal()
    }

    fn production_source(&self) -> Option<&'a Arc<GeneratedAffineResidualGroupExactPhysicalRow>> {
        self.staged_pivot.production_source()
    }

    fn target_ordinals(&self) -> Range<usize> {
        self.targets.iter()
    }

    fn is_target_unresolved(
        &self,
        solve_ordinal: usize,
    ) -> Result<bool, GeneratedAffineResidualGroupExactSessionError> {
        self.targets
            .is_unresolved(solve_ordinal)
            .map_err(GeneratedAffineResidualGroupExactSessionError::from)
    }

    fn retain_target(
        &self,
        solve_ordinal: usize,
    ) -> Result<
        GeneratedAffineResidualGroupRetainedExactTarget,
        GeneratedAffineResidualGroupExactSessionError,
    > {
        self.targets
            .retain_target(solve_ordinal)
            .map_err(GeneratedAffineResidualGroupExactSessionError::from)
    }

    fn physical_frame(&self) -> &'a Arc<GeneratedAffineResidualGroupPhysicalFrame> {
        self.staged_pivot.frame()
    }

    const fn database_epoch(&self) -> usize {
        self.staged_pivot.database_epoch()
    }

    const fn group_ordinal(&self) -> usize {
        self.staged_pivot.group_ordinal()
    }

    fn anchor_case_ordinal(&self) -> usize {
        self.anchor_case_ordinal
    }

    fn free_positions(&self) -> &[usize] {
        self.free_positions
    }

    fn target_locators(&self) -> &[GeneratedAffineResidualGroupSolveTargetLocator] {
        self.target_locators
    }

    const fn ambient_arity(&self) -> usize {
        self.ambient_arity
    }

    /// Borrowed row-major `ambient_arity * free_positions().len()` compact
    /// affine matrix authenticated from the retained plan authority.
    const fn compact_affine_matrix(&self) -> &'a [Integer] {
        self.compact_affine_matrix
    }

    const fn staged_live_prospective_retained_bytes(&self) -> usize {
        self.staged_live_prospective_retained_bytes
    }

    const fn staged_live_observed_retained_bytes(&self) -> usize {
        self.staged_live_observed_retained_bytes
    }

    const fn target_state_combined_retained_byte_envelope(&self) -> usize {
        self.target_state_combined_retained_byte_envelope
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionStagedNewPivotView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionStagedNewPivotView")
            .field("database_epoch", &self.database_epoch())
            .field("group_ordinal", &self.group_ordinal())
            .field("state_version", &self.staged_pivot.state_version())
            .field("source_ordinal", &self.staged_pivot.source_ordinal())
            .field("pivot_ordinal", &self.staged_pivot.pivot_ordinal())
            .field("ambient_arity", &self.ambient_arity)
            .field(
                "compact_affine_matrix_entries",
                &self.compact_affine_matrix.len(),
            )
            .field("target_count", &self.target_locators().len())
            .field(
                "staged_live_prospective_retained_bytes",
                &self.staged_live_prospective_retained_bytes,
            )
            .field(
                "staged_live_observed_retained_bytes",
                &self.staged_live_observed_retained_bytes,
            )
            .field(
                "target_state_combined_retained_byte_envelope",
                &self.target_state_combined_retained_byte_envelope,
            )
            .field("private_staged_pivot", &"<redacted>")
            .field("private_compact_affine_matrix", &"<redacted>")
            .field("private_target_state", &"<redacted>")
            .finish()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::generated_affine_parametric_ordering::{
        GeneratedAffineParametricOrderingCertificate, GeneratedAffineParametricOrderingLimits,
    };
    use crate::generated_affine_prepare_point_schedule::{
        GeneratedAffinePreparePointScheduleCertificate, GeneratedAffinePreparePointScheduleLimits,
    };
    use crate::generated_affine_residual_boolean_cover::{
        GeneratedAffineResidualBooleanCoverCompiler, GeneratedAffineResidualBooleanCoverLimits,
    };
    use crate::generated_affine_residual_case_inventory::{
        GeneratedAffineResidualCaseAuthority, GeneratedAffineResidualCaseAuthorityLimits,
        GeneratedAffineResidualCaseInventoryCompiler, GeneratedAffineResidualCaseInventoryLimits,
    };
    use crate::generated_affine_residual_case_premises::{
        GeneratedAffineResidualCasePremisesLimits, GeneratedAffineResidualCasePremisesOutcome,
        compile_generated_affine_residual_case_premises,
    };
    use crate::generated_affine_residual_case_reelimination::{
        GeneratedAffineResidualCaseReeliminationCompilation,
        GeneratedAffineResidualCaseReeliminationCompiler,
        GeneratedAffineResidualCaseReeliminationLimits,
    };
    use crate::generated_affine_residual_group_exact_physical_row::{
        GeneratedAffineResidualGroupExactPhysicalRowCompiler,
        GeneratedAffineResidualGroupExactPhysicalRowLimits,
    };
    use crate::generated_affine_residual_group_exact_recenter_kernel::{
        centered_shift_arithmetic_operations_for_test,
        reset_centered_shift_arithmetic_operations_for_test,
    };
    use crate::generated_affine_residual_group_exact_targets::{
        GeneratedAffineResidualGroupAuthenticatedExactTargetView,
        GeneratedAffineResidualGroupExactTargetStateStats,
    };
    use crate::generated_affine_residual_group_physical_key::{
        GeneratedAffineResidualGroupPhysicalFrame, GeneratedAffineResidualGroupPhysicalKeyLimits,
    };
    use crate::generated_affine_residual_group_ready_publication::{
        GENERATED_AFFINE_RESIDUAL_GROUP_READY_PUBLICATION_ANALYSIS_V2_SCHEMA,
        GeneratedAffineResidualGroupReadyForConditions,
        GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler,
        GeneratedAffineResidualGroupReadyPublicationAnalysisLimits,
        GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome,
    };
    use crate::generated_affine_residual_group_solve_plan::GeneratedAffineResidualGroupSolvePlanLimits;
    use crate::generated_affine_residual_source_authority::GeneratedAffineResidualSourceAuthority;
    use crate::generated_sector_affine_effective_coverage::{
        GeneratedSectorAffineEffectiveCoverageCompiler,
        GeneratedSectorAffineEffectiveCoverageConfig, GeneratedSectorAffineEffectiveCoverageLimits,
    };
    use crate::generated_sector_affine_effective_residual_queue::{
        GeneratedSectorAffineEffectiveResidualQueueCompiler,
        GeneratedSectorAffineEffectiveResidualQueueLimits,
    };
    use crate::parametric_sector_formula_affine_terminal::{
        ParametricSectorFormulaAffineTerminalCertificate,
        ParametricSectorFormulaAffineTerminalCompiler, ParametricSectorFormulaAffineTerminalLimits,
    };
    use crate::parametric_sector_formula_residual::{
        ParametricSectorFormulaResidualCursor, ParametricSectorFormulaResidualLimits,
        ParametricSectorFormulaResidualRequest,
    };
    use crate::parametric_sector_normalized_source::{
        ParametricSectorNormalizedCoverageSource, ParametricSectorNormalizedCoverageSourceCompiler,
        ParametricSectorNormalizedCoverageSourceLimits,
    };
    use crate::{
        AffineDenominator, CoefficientContext, GeneratedResidualAffineCaseInventoryCompiler,
        GeneratedResidualAffineCaseInventoryLimits, GeneratedSectorDiscoveryCompiler,
        GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafQueueCompiler,
        GeneratedSectorLiveLeafQueueLimits, IntegralOrderingPolicy, ParametricIbpGenerator,
        SectorMask,
    };

    const M: i64 = i64::MAX;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct SessionStateSnapshot {
        database_state_version: usize,
        target_state_version: usize,
        pivot_count: usize,
        target_stats: GeneratedAffineResidualGroupExactTargetStateStats,
        event_count: usize,
        event_capacity: usize,
        event_stats: GeneratedAffineResidualGroupExactSessionEventStats,
    }

    fn session_state_snapshot(
        session: &GeneratedAffineResidualGroupExactSession,
    ) -> SessionStateSnapshot {
        SessionStateSnapshot {
            database_state_version: session.database.state_version(),
            target_state_version: session.target_state.state_version(),
            pivot_count: session.database.pivot_count(),
            target_stats: session.target_state.stats(),
            event_count: session.events.len(),
            event_capacity: session.events.capacity(),
            event_stats: session.event_stats,
        }
    }

    fn physical_key(
        plan: &Arc<GeneratedAffineResidualGroupSolvePlan>,
        values: &[Integer],
    ) -> GeneratedAffineResidualGroupPhysicalKey {
        plan.physical_frame()
            .test_key_for_borrowed_physical_values(values)
            .unwrap()
    }

    fn symbolic_test_coefficient(context: &ParametricCoefficientContext) -> ParametricCoefficient {
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        context
            .add(&n0, &context.mul(&context.integer(2), &n1).unwrap())
            .unwrap()
    }

    fn symbolic_test_guard(context: &ParametricCoefficientContext) -> ParametricNonZeroCondition {
        let d = context
            .lift(&context.base().parameter("d").unwrap())
            .unwrap();
        let n0 = context.index(0).unwrap();
        context
            .nonzero_condition(
                context
                    .numerator_condition(&context.add(&d, &n0).unwrap())
                    .unwrap(),
                GuardOrigin::GuardedDivisionDivisorNumerator,
            )
            .unwrap()
    }

    fn test_family(name: &str) -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let zero = coefficients.zero();
        let one = coefficients.one();
        let minus_m2 = coefficients.parse("-m2").unwrap();
        IntegralFamily::new(
            name,
            vec!["k1".into(), "k2".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![one.clone(), zero.clone(), zero.clone()],
                ),
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![zero.clone(), zero.clone(), one.clone()],
                ),
                AffineDenominator::new(minus_m2, vec![one.clone(), coefficients.integer(2), one]),
            ],
            Vec::new(),
            vec![zero.clone(), zero.clone(), zero],
        )
        .unwrap()
    }

    fn plan_fixture(
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedAffineResidualGroupSolvePlan>,
    ) {
        let family = test_family(name);
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
        discovery_limits.adaptive.max_search_depth = 0;
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_from_bit_string("011").unwrap(),
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
        let boolean = Arc::new(
            GeneratedAffineResidualBooleanCoverCompiler::compile(
                &family,
                &context,
                GeneratedAffineResidualSourceAuthority::initial_global(queue),
                GeneratedAffineResidualBooleanCoverLimits::default(),
            )
            .unwrap(),
        );
        let inventory = Arc::new(
            GeneratedAffineResidualCaseInventoryCompiler::compile(
                &family,
                &context,
                boolean,
                GeneratedAffineResidualCaseInventoryLimits::default(),
            )
            .unwrap(),
        );
        let group_ordinal = (0..inventory.group_count())
            .max_by_key(|&ordinal| {
                inventory
                    .authenticated_group_view(&context, ordinal)
                    .unwrap()
                    .case_ordinals()
                    .len()
            })
            .unwrap();
        let group = inventory
            .authenticated_group_view(&context, group_ordinal)
            .unwrap();
        let authority = Arc::new(
            GeneratedAffineResidualCaseAuthority::try_new(
                &family,
                &context,
                Arc::clone(&inventory),
                group.anchor_case_ordinal(),
                GeneratedAffineResidualCaseAuthorityLimits::default(),
            )
            .unwrap(),
        );
        let frame = Arc::new(
            GeneratedAffineResidualGroupPhysicalFrame::try_new(
                &family,
                &context,
                Arc::clone(&authority),
                GeneratedAffineResidualGroupPhysicalKeyLimits::default(),
            )
            .unwrap(),
        );
        let plan = Arc::new(
            GeneratedAffineResidualGroupSolvePlan::try_new(
                &family,
                &context,
                inventory,
                authority,
                frame,
                GeneratedAffineResidualGroupSolvePlanLimits::default(),
            )
            .unwrap(),
        );
        (family, context, plan)
    }

    struct DirectProductionFixture {
        family: IntegralFamily,
        context: ParametricCoefficientContext,
        normalized: Arc<ParametricSectorNormalizedCoverageSource>,
        terminal: Arc<ParametricSectorFormulaAffineTerminalCertificate>,
        plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
        rows: Vec<Arc<GeneratedAffineResidualGroupExactPhysicalRow>>,
    }

    #[derive(Clone, Copy)]
    enum DirectProductionCoverage {
        NonemptyAttemptResidual,
        EmptyUncovered,
    }

    /// Build the complete production Direct ingress chain.  The returned rows
    /// are the naturally retained re-elimination rows in deterministic witness
    /// order; no test-only physical terms or authored recurrence enter it.
    fn direct_production_fixture(
        name: &str,
        sector: SectorMask,
        coverage: DirectProductionCoverage,
    ) -> DirectProductionFixture {
        let family = test_family(name);
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let (normalized, request) = match coverage {
            DirectProductionCoverage::NonemptyAttemptResidual => {
                let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
                discovery_limits.adaptive.max_search_depth = 0;
                discovery_limits
                    .coverage
                    .max_materialized_product_zero_support_terms = 0;
                let discovery = GeneratedSectorDiscoveryCompiler::compile(
                    &family,
                    &context,
                    sector,
                    IntegralOrderingPolicy::RustRedUnshiftedV1,
                    discovery_limits,
                )
                .unwrap();
                let compilations = discovery
                    .coverage()
                    .candidate_attempts()
                    .iter()
                    .map(|attempt| attempt.compilation().clone())
                    .collect();
                (
                    Arc::new(
                        ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated(
                            &family,
                            &context,
                            discovery.sector().clone(),
                            IntegralOrderingPolicy::RustRedUnshiftedV1,
                            compilations,
                            ParametricSectorNormalizedCoverageSourceLimits::default(),
                        )
                        .unwrap(),
                    ),
                    ParametricSectorFormulaResidualRequest::AnyResidual,
                )
            }
            DirectProductionCoverage::EmptyUncovered => (
                Arc::new(
                    ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated(
                        &family,
                        &context,
                        sector,
                        IntegralOrderingPolicy::RustRedUnshiftedV1,
                        Vec::new(),
                        ParametricSectorNormalizedCoverageSourceLimits::default(),
                    )
                    .unwrap(),
                ),
                ParametricSectorFormulaResidualRequest::Uncovered,
            ),
        };
        assert!(!normalized.row_span().rows().is_empty());
        let mut cursor = ParametricSectorFormulaResidualCursor::try_new(
            &family,
            &context,
            Arc::clone(&normalized),
            request,
            ParametricSectorFormulaResidualLimits::default(),
        )
        .unwrap();
        let path = Arc::new(cursor.next_path().unwrap().unwrap());
        if matches!(coverage, DirectProductionCoverage::EmptyUncovered) {
            assert!(cursor.next_path().unwrap().is_none());
        }
        let terminal = Arc::new(
            ParametricSectorFormulaAffineTerminalCompiler::compile(
                &family,
                &context,
                path,
                ParametricSectorFormulaAffineTerminalLimits::default(),
            )
            .unwrap(),
        );
        assert!(terminal.geometry().is_some());
        let authority = Arc::new(
            GeneratedAffineResidualCaseAuthority::try_new_direct_formula_singleton(
                &family,
                &context,
                Arc::clone(&terminal),
                GeneratedAffineResidualCaseAuthorityLimits::default(),
            )
            .unwrap(),
        );
        let premises = match compile_generated_affine_residual_case_premises(
            &family,
            &context,
            Arc::clone(&authority),
            GeneratedAffineResidualCasePremisesLimits::default(),
        )
        .unwrap()
        {
            GeneratedAffineResidualCasePremisesOutcome::Ready(value) => Arc::new(value),
            GeneratedAffineResidualCasePremisesOutcome::RequiresAffineEqualityRefinement(_) => {
                panic!("natural Direct terminal unexpectedly requires equality refinement")
            }
        };
        let ordering = Arc::new(
            GeneratedAffineParametricOrderingCertificate::try_new(
                &family,
                &context,
                Arc::clone(&authority),
                GeneratedAffineParametricOrderingLimits::default(),
            )
            .unwrap(),
        );
        let schedule = Arc::new(
            GeneratedAffinePreparePointScheduleCertificate::compile(
                &family,
                &context,
                Arc::clone(&ordering),
                &authority,
                0,
                GeneratedAffinePreparePointScheduleLimits::default(),
            )
            .unwrap(),
        );
        let compilation = GeneratedAffineResidualCaseReeliminationCompiler::compile(
            &family,
            &context,
            Arc::clone(&authority),
            premises,
            ordering,
            schedule,
            GeneratedAffineResidualCaseReeliminationLimits::default(),
        )
        .unwrap();
        let GeneratedAffineResidualCaseReeliminationCompilation::Eliminated(certificate) =
            compilation
        else {
            panic!("natural Direct terminal produced no eliminable rows")
        };
        let certificate = Arc::new(certificate);
        let frame = Arc::new(
            GeneratedAffineResidualGroupPhysicalFrame::try_new(
                &family,
                &context,
                Arc::clone(&authority),
                GeneratedAffineResidualGroupPhysicalKeyLimits::default(),
            )
            .unwrap(),
        );
        let plan = Arc::new(
            GeneratedAffineResidualGroupSolvePlan::try_new_direct_formula_singleton(
                &family,
                &context,
                authority,
                Arc::clone(&frame),
                GeneratedAffineResidualGroupSolvePlanLimits::default(),
            )
            .unwrap(),
        );
        let mut retained_row_ordinal = 0usize;
        let mut rows = Vec::new();
        for (witness_ordinal, witness) in certificate.witnesses().iter().enumerate() {
            if !witness.outcome().is_retained() {
                continue;
            }
            rows.push(Arc::new(
                GeneratedAffineResidualGroupExactPhysicalRowCompiler::compile(
                    &family,
                    &context,
                    Arc::clone(&certificate),
                    retained_row_ordinal,
                    witness_ordinal,
                    Arc::clone(&frame),
                    GeneratedAffineResidualGroupExactPhysicalRowLimits::default(),
                )
                .unwrap(),
            ));
            retained_row_ordinal += 1;
        }
        assert_eq!(rows.len(), certificate.retained_row_count());
        assert!(!rows.is_empty());
        DirectProductionFixture {
            family,
            context,
            normalized,
            terminal,
            plan,
            rows,
        }
    }

    /// Current-lineage production fixture shared only by sibling unit tests
    /// that exercise the post-Ready typestate.  It contains no authored row:
    /// every source is generated through the Direct residual pipeline above.
    pub(crate) struct ExactConditionPlanTestFixture {
        pub(crate) family: IntegralFamily,
        pub(crate) context: ParametricCoefficientContext,
        pub(crate) session: GeneratedAffineResidualGroupExactSession,
        pub(crate) source: Arc<GeneratedAffineResidualGroupExactPhysicalRow>,
        pub(crate) ready: GeneratedAffineResidualGroupReadyForConditions,
    }

    pub(crate) fn exact_condition_plan_test_fixture(
        name: &str,
        constrained_compact: bool,
    ) -> ExactConditionPlanTestFixture {
        exact_condition_plan_test_fixture_in_sector(name, "111", constrained_compact)
    }

    pub(crate) fn exact_condition_plan_test_fixture_in_sector(
        name: &str,
        sector_bits: &str,
        constrained_compact: bool,
    ) -> ExactConditionPlanTestFixture {
        exact_condition_plan_test_fixture_in_sector_with_session_limits(
            name,
            sector_bits,
            constrained_compact,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
    }

    pub(crate) fn exact_condition_plan_test_fixture_in_sector_with_session_limits(
        name: &str,
        sector_bits: &str,
        constrained_compact: bool,
        session_limits: GeneratedAffineResidualGroupExactSessionLimits,
    ) -> ExactConditionPlanTestFixture {
        let coverage = if constrained_compact {
            DirectProductionCoverage::NonemptyAttemptResidual
        } else {
            DirectProductionCoverage::EmptyUncovered
        };
        let fixture = direct_production_fixture(
            name,
            SectorMask::try_from_bit_string(sector_bits).unwrap(),
            coverage,
        );
        let mut session = GeneratedAffineResidualGroupExactSession::try_new(
            &fixture.family,
            &fixture.context,
            Arc::clone(&fixture.plan),
            211,
            session_limits,
        )
        .unwrap();
        for row in &fixture.rows {
            let transaction = session
                .stage_replayed_row(&fixture.family, &fixture.context, row)
                .unwrap();
            let transaction = match session.classify_dependent(transaction) {
                Ok(classified) => {
                    session
                        .commit_dependent(&fixture.family, &fixture.context, classified)
                        .unwrap();
                    continue;
                }
                Err(failure) => failure.into_transaction(),
            };
            match session
                .recenter_staged_new_pivot(&fixture.family, &fixture.context, transaction)
                .unwrap()
            {
                GeneratedAffineResidualGroupExactSessionRecenterOutcome::NoTarget(no_target) => {
                    session = session
                        .commit_no_target(&fixture.family, &fixture.context, no_target)
                        .unwrap()
                        .into_session();
                }
                GeneratedAffineResidualGroupExactSessionRecenterOutcome::RequiresAffineEqualityRefinement(
                    _,
                ) => panic!("condition-plan fixture unexpectedly requires equality refinement"),
                GeneratedAffineResidualGroupExactSessionRecenterOutcome::Ready(ready) => {
                    let analyzed =
                        GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler::analyze(
                            &fixture.family,
                            &fixture.context,
                            &session,
                            ready,
                            GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default(),
                        )
                        .unwrap();
                    let GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome::ReadyForConditions(
                        ready,
                    ) = analyzed
                    else {
                        panic!("condition-plan fixture Ready row failed exact descent")
                    };
                    return ExactConditionPlanTestFixture {
                        family: fixture.family,
                        context: fixture.context,
                        session,
                        source: Arc::clone(row),
                        ready,
                    };
                }
            }
        }
        panic!("condition-plan fixture exhausted generated rows before exact Ready")
    }

    fn equality_refinement_plan_fixture(
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedAffineResidualGroupSolvePlan>,
    ) {
        let family = test_family(name);
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
        discovery_limits.adaptive.max_search_depth = 0;
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_from_bit_string("001").unwrap(),
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
        let prior_inventory = Arc::new(
            GeneratedResidualAffineCaseInventoryCompiler::compile(
                &family,
                &context,
                queue,
                GeneratedResidualAffineCaseInventoryLimits::default(),
            )
            .unwrap(),
        );
        let effective = Arc::new(
            GeneratedSectorAffineEffectiveCoverageCompiler::compile(
                &family,
                &context,
                prior_inventory,
                GeneratedSectorAffineEffectiveCoverageConfig::new(0),
                GeneratedSectorAffineEffectiveCoverageLimits::default(),
            )
            .unwrap(),
        );
        let prior_queue = Arc::new(
            GeneratedSectorAffineEffectiveResidualQueueCompiler::compile(
                &family,
                &context,
                effective,
                GeneratedSectorAffineEffectiveResidualQueueLimits::default(),
            )
            .unwrap(),
        );
        let boolean = Arc::new(
            GeneratedAffineResidualBooleanCoverCompiler::compile(
                &family,
                &context,
                GeneratedAffineResidualSourceAuthority::prior_effective(prior_queue),
                GeneratedAffineResidualBooleanCoverLimits::default(),
            )
            .unwrap(),
        );
        let inventory = Arc::new(
            GeneratedAffineResidualCaseInventoryCompiler::compile(
                &family,
                &context,
                boolean,
                GeneratedAffineResidualCaseInventoryLimits::default(),
            )
            .unwrap(),
        );
        let group_ordinal = (0..inventory.group_count())
            .max_by_key(|&ordinal| {
                inventory
                    .authenticated_group_view(&context, ordinal)
                    .unwrap()
                    .case_ordinals()
                    .len()
            })
            .unwrap();
        let group = inventory
            .authenticated_group_view(&context, group_ordinal)
            .unwrap();
        let authority = Arc::new(
            GeneratedAffineResidualCaseAuthority::try_new(
                &family,
                &context,
                Arc::clone(&inventory),
                group.anchor_case_ordinal(),
                GeneratedAffineResidualCaseAuthorityLimits::default(),
            )
            .unwrap(),
        );
        let frame = Arc::new(
            GeneratedAffineResidualGroupPhysicalFrame::try_new(
                &family,
                &context,
                Arc::clone(&authority),
                GeneratedAffineResidualGroupPhysicalKeyLimits::default(),
            )
            .unwrap(),
        );
        let plan = Arc::new(
            GeneratedAffineResidualGroupSolvePlan::try_new(
                &family,
                &context,
                inventory,
                authority,
                frame,
                GeneratedAffineResidualGroupSolvePlanLimits::default(),
            )
            .unwrap(),
        );
        (family, context, plan)
    }

    fn production_row(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        plan: &Arc<GeneratedAffineResidualGroupSolvePlan>,
    ) -> Arc<GeneratedAffineResidualGroupExactPhysicalRow> {
        let frame = plan.physical_frame();
        for &case_ordinal in frame.case_ordinals() {
            let authority = Arc::new(
                GeneratedAffineResidualCaseAuthority::try_new(
                    family,
                    context,
                    Arc::clone(plan.inventory().unwrap()),
                    case_ordinal,
                    GeneratedAffineResidualCaseAuthorityLimits::default(),
                )
                .unwrap(),
            );
            let premises = match compile_generated_affine_residual_case_premises(
                family,
                context,
                Arc::clone(&authority),
                GeneratedAffineResidualCasePremisesLimits::default(),
            )
            .unwrap()
            {
                GeneratedAffineResidualCasePremisesOutcome::Ready(value) => Arc::new(value),
                GeneratedAffineResidualCasePremisesOutcome::RequiresAffineEqualityRefinement(_) => {
                    continue;
                }
            };
            let ordering = Arc::new(
                GeneratedAffineParametricOrderingCertificate::try_new(
                    family,
                    context,
                    Arc::clone(&authority),
                    GeneratedAffineParametricOrderingLimits::default(),
                )
                .unwrap(),
            );
            let schedule = Arc::new(
                GeneratedAffinePreparePointScheduleCertificate::compile(
                    family,
                    context,
                    Arc::clone(&ordering),
                    &authority,
                    0,
                    GeneratedAffinePreparePointScheduleLimits::default(),
                )
                .unwrap(),
            );
            let compilation = GeneratedAffineResidualCaseReeliminationCompiler::compile(
                family,
                context,
                authority,
                premises,
                ordering,
                schedule,
                GeneratedAffineResidualCaseReeliminationLimits::default(),
            )
            .unwrap();
            let GeneratedAffineResidualCaseReeliminationCompilation::Eliminated(certificate) =
                compilation
            else {
                continue;
            };
            let certificate = Arc::new(certificate);
            let Some(witness_ordinal) = certificate
                .witnesses()
                .iter()
                .position(|witness| witness.outcome().is_retained())
            else {
                continue;
            };
            let retained_row_ordinal = certificate.witnesses()[..witness_ordinal]
                .iter()
                .filter(|witness| witness.outcome().is_retained())
                .count();
            return Arc::new(
                GeneratedAffineResidualGroupExactPhysicalRowCompiler::compile(
                    family,
                    context,
                    certificate,
                    retained_row_ordinal,
                    witness_ordinal,
                    Arc::clone(frame),
                    GeneratedAffineResidualGroupExactPhysicalRowLimits::default(),
                )
                .unwrap(),
            );
        }
        panic!("the generated-affine fixture produced no authenticated physical row")
    }

    #[test]
    fn natural_direct_constrained_residual_reaches_v2_ready_for_conditions() {
        let fixture = direct_production_fixture(
            "exact-session-direct-production-constrained",
            SectorMask::try_from_bit_string("111").unwrap(),
            DirectProductionCoverage::NonemptyAttemptResidual,
        );
        assert!(!fixture.normalized.attempts().is_empty());
        fixture
            .normalized
            .replay(&fixture.family, &fixture.context)
            .unwrap();
        let geometry = fixture.terminal.geometry().unwrap();
        assert_eq!(geometry.ambient_arity(), 3);
        assert_eq!(geometry.free_positions(), &[1]);
        assert_eq!(
            geometry.compact_linear_coefficients(),
            &[Integer::from(0), Integer::from(1), Integer::from(0)]
        );

        let mut session = GeneratedAffineResidualGroupExactSession::try_new(
            &fixture.family,
            &fixture.context,
            Arc::clone(&fixture.plan),
            211,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        for row in &fixture.rows {
            let transaction = session
                .stage_replayed_row(&fixture.family, &fixture.context, row)
                .unwrap();
            let transaction = match session.classify_dependent(transaction) {
                Ok(classified) => {
                    session
                        .commit_dependent(&fixture.family, &fixture.context, classified)
                        .unwrap();
                    continue;
                }
                Err(failure) => failure.into_transaction(),
            };
            match session
                .recenter_staged_new_pivot(&fixture.family, &fixture.context, transaction)
                .unwrap()
            {
                GeneratedAffineResidualGroupExactSessionRecenterOutcome::NoTarget(no_target) => {
                    session = session
                        .commit_no_target(&fixture.family, &fixture.context, no_target)
                        .unwrap()
                        .into_session();
                }
                GeneratedAffineResidualGroupExactSessionRecenterOutcome::RequiresAffineEqualityRefinement(
                    _,
                ) => panic!("natural constrained Direct target unexpectedly requires refinement"),
                GeneratedAffineResidualGroupExactSessionRecenterOutcome::Ready(ready) => {
                    let source_ordinal = ready.source_ordinal();
                    let pivot_ordinal = ready.pivot_ordinal();
                    let locator = *ready.target_locator();
                    let before_analysis = session_state_snapshot(&session);
                    let analyzed =
                        GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler::analyze(
                            &fixture.family,
                            &fixture.context,
                            &session,
                            ready,
                            GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default(),
                        )
                        .unwrap();
                    let GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome::ReadyForConditions(
                        ready_for_conditions,
                    ) = analyzed
                    else {
                        panic!("constrained Direct compact map must pass V2 Ready analysis")
                    };
                    assert_eq!(
                        ready_for_conditions.schema(),
                        GENERATED_AFFINE_RESIDUAL_GROUP_READY_PUBLICATION_ANALYSIS_V2_SCHEMA,
                    );
                    let geometry = ready_for_conditions.geometry();
                    assert_eq!(
                        geometry.ambient_arity(),
                        3,
                    );
                    assert_eq!(geometry.free_count(), 1);
                    assert_eq!(geometry.matrix_entries_inspected(), 3);
                    assert_eq!(geometry.selector_entries_inspected(), 1);
                    assert_eq!(geometry.constant_rows(), 2);
                    assert_eq!(geometry.symbolic_rows(), 1);
                    assert_eq!(ready_for_conditions.targets_consumed(), 0);
                    assert_eq!(ready_for_conditions.stats().arity(), 3);
                    let rhs_terms = ready_for_conditions.stats().rhs_terms();
                    assert_eq!(rhs_terms, 6);
                    assert_eq!(
                        ready_for_conditions.stats().physical_keys_constructed(),
                        rhs_terms + 1,
                    );
                    assert_eq!(ready_for_conditions.stats().key_comparisons(), rhs_terms);
                    assert_eq!(ready_for_conditions.descent().len(), rhs_terms);
                    assert_eq!(
                        ready_for_conditions.stats().hazard_ranges(),
                        ready_for_conditions.hazards().len(),
                    );
                    for witness in ready_for_conditions.descent() {
                        let rhs = fixture
                            .plan
                            .physical_frame()
                            .key_for_exact_local(
                                locator.inventory_position(),
                                locator.case_ordinal(),
                                ready_for_conditions.ready().terms()[witness.term_ordinal()]
                                    .shift()
                                    .values(),
                            )
                            .unwrap();
                        assert!(witness.replay(&rhs, ready_for_conditions.source_key()));
                    }
                    assert_eq!(session_state_snapshot(&session), before_analysis);
                    ready_for_conditions
                        .replay(&fixture.family, &fixture.context, &session)
                        .unwrap();
                    assert_eq!(session_state_snapshot(&session), before_analysis);
                    let recovered = ready_for_conditions.into_ready();
                    assert_eq!(recovered.source_ordinal(), source_ordinal);
                    assert_eq!(recovered.pivot_ordinal(), pivot_ordinal);
                    assert_eq!(recovered.target_locator(), &locator);
                    assert_eq!(session_state_snapshot(&session), before_analysis);
                    session.replay(&fixture.family, &fixture.context).unwrap();
                    return;
                }
            }
        }
        panic!("natural constrained Direct rows produced no exact Ready token");
    }

    #[test]
    fn constrained_direct_v2_ready_proves_nontrivial_rhs_under_free_translation() {
        let fixture = direct_production_fixture(
            "exact-session-direct-constrained-nontrivial-ready",
            SectorMask::try_from_bit_string("111").unwrap(),
            DirectProductionCoverage::NonemptyAttemptResidual,
        );
        let compact_geometry = fixture.terminal.geometry().unwrap();
        assert_eq!(compact_geometry.ambient_arity(), 3);
        assert_eq!(compact_geometry.free_positions(), &[1]);
        assert_eq!(
            compact_geometry.compact_linear_coefficients(),
            &[Integer::zero(), Integer::one(), Integer::zero()],
        );

        let locator = fixture.plan.targets()[0];
        let frame = fixture.plan.physical_frame();
        let target_values = frame
            .anchor_offset(locator.inventory_position(), locator.case_ordinal())
            .unwrap()
            .values()
            .to_vec();
        let centered_rhs_shift = [Integer::zero(), Integer::from(-1), Integer::zero()];
        let target_key = physical_key(&fixture.plan, &target_values);
        let rhs_key = frame
            .key_for_exact_local(
                locator.inventory_position(),
                locator.case_ordinal(),
                &centered_rhs_shift,
            )
            .unwrap();
        assert!(rhs_key < target_key);

        let session = GeneratedAffineResidualGroupExactSession::try_new(
            &fixture.family,
            &fixture.context,
            Arc::clone(&fixture.plan),
            211,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        let before = session_state_snapshot(&session);
        // The Direct geometry, target, frame, solve plan, exact recentering,
        // and V2 analysis are all production owners.  Only this deliberately
        // minimal two-term validation row uses the authenticated test ingress.
        let transaction = session
            .stage_authenticated_terms_for_test(
                &fixture.context,
                vec![
                    (rhs_key, fixture.context.one()),
                    (target_key, fixture.context.one()),
                ],
                Vec::new(),
            )
            .unwrap();
        let GeneratedAffineResidualGroupExactSessionRecenterOutcome::Ready(ready) = session
            .recenter_staged_new_pivot(&fixture.family, &fixture.context, transaction)
            .unwrap()
        else {
            panic!("the constrained two-term row must reach exact Ready");
        };
        assert_eq!(ready.terms().len(), 2);
        assert_eq!(session_state_snapshot(&session), before);

        let analyzed = GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler::analyze(
            &fixture.family,
            &fixture.context,
            &session,
            ready,
            GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default(),
        )
        .unwrap();
        let GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome::ReadyForConditions(
            ready_for_conditions,
        ) = analyzed
        else {
            panic!("the constrained two-term row must pass V2 descent analysis");
        };
        let geometry = ready_for_conditions.geometry();
        assert_eq!(geometry.ambient_arity(), 3);
        assert_eq!(geometry.free_count(), 1);
        assert_eq!(geometry.constant_rows(), 2);
        assert_eq!(geometry.symbolic_rows(), 1);
        assert_eq!(ready_for_conditions.stats().terms(), 2);
        assert_eq!(ready_for_conditions.stats().rhs_terms(), 1);
        assert_eq!(ready_for_conditions.stats().physical_keys_constructed(), 2,);
        assert_eq!(ready_for_conditions.stats().key_comparisons(), 1);
        assert_eq!(ready_for_conditions.descent().len(), 1);
        assert!(ready_for_conditions.hazards().is_empty());

        let descent = &ready_for_conditions.descent()[0];
        let rhs_term = &ready_for_conditions.ready().terms()[descent.term_ordinal()];
        assert_eq!(rhs_term.shift().values(), &centered_rhs_shift);
        let exact_rhs_key = frame
            .key_for_exact_local(
                locator.inventory_position(),
                locator.case_ordinal(),
                rhs_term.shift().values(),
            )
            .unwrap();
        assert!(descent.replay(&exact_rhs_key, ready_for_conditions.source_key()));

        // Translate both exact keys by B*z along the authenticated free row.
        // The stored comparison transcript must remain valid throughout the
        // source chamber, including at arbitrary precision.
        let huge = Integer::one() << 4096_u32;
        for free_translation in [Integer::zero(), Integer::from(7), Integer::from(13), huge] {
            let mut translated_source = target_values.clone();
            translated_source[1] = &translated_source[1] + &free_translation;
            let translated_source_key = frame
                .test_key_for_borrowed_physical_values(&translated_source)
                .unwrap();
            let mut translated_rhs = translated_source;
            translated_rhs[1] = &translated_rhs[1] - Integer::one();
            let translated_rhs_key = frame
                .test_key_for_borrowed_physical_values(&translated_rhs)
                .unwrap();
            assert!(translated_rhs_key < translated_source_key);
            assert!(descent.replay(&translated_rhs_key, &translated_source_key));
        }

        ready_for_conditions
            .replay(&fixture.family, &fixture.context, &session)
            .unwrap();
        assert_eq!(session_state_snapshot(&session), before);
        session.replay(&fixture.family, &fixture.context).unwrap();
    }

    #[test]
    fn natural_direct_uncovered_rows_reach_ready_for_conditions_and_reject_foreign_source() {
        let fixture_name = "exact-session-direct-production-ready";
        let fixture = direct_production_fixture(
            fixture_name,
            SectorMask::try_from_bit_string("111").unwrap(),
            DirectProductionCoverage::EmptyUncovered,
        );
        let foreign = direct_production_fixture(
            fixture_name,
            SectorMask::try_from_bit_string("111").unwrap(),
            DirectProductionCoverage::EmptyUncovered,
        );
        assert!(fixture.normalized.attempts().is_empty());
        assert!(!fixture.normalized.row_span().rows().is_empty());
        assert!(Arc::ptr_eq(
            fixture.terminal.path_arc().source_arc(),
            &fixture.normalized,
        ));
        fixture
            .normalized
            .replay(&fixture.family, &fixture.context)
            .unwrap();
        let geometry = fixture.terminal.geometry().unwrap();
        assert_eq!(geometry.ambient_arity(), 3);
        assert_eq!(
            geometry.constants(),
            &[Integer::from(0), Integer::from(0), Integer::from(0)]
        );
        assert_eq!(geometry.free_positions(), &[0, 1, 2]);
        assert_eq!(
            geometry.compact_linear_coefficients(),
            &[
                Integer::from(1),
                Integer::from(0),
                Integer::from(0),
                Integer::from(0),
                Integer::from(1),
                Integer::from(0),
                Integer::from(0),
                Integer::from(0),
                Integer::from(1),
            ]
        );
        assert_eq!(
            fixture.plan.authority().source_row_count(),
            fixture.normalized.row_span().rows().len()
        );
        assert!(fixture.plan.authority().source_row_count() > 0);
        assert!(!Arc::ptr_eq(&fixture.terminal, &foreign.terminal));
        assert_eq!(
            fixture
                .plan
                .authority()
                .stable_value_identity()
                .unwrap()
                .bytes(),
            foreign
                .plan
                .authority()
                .stable_value_identity()
                .unwrap()
                .bytes(),
        );
        assert!(
            !fixture
                .plan
                .authority()
                .same_source_allocation_as(foreign.plan.authority())
        );
        assert_eq!(
            fixture.plan.source_kind(),
            GeneratedAffineResidualCaseAuthoritySourceKind::DirectFormulaSingleton
        );

        let mut session = GeneratedAffineResidualGroupExactSession::try_new(
            &fixture.family,
            &fixture.context,
            Arc::clone(&fixture.plan),
            211,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        assert_eq!(
            session.schema(),
            GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_SESSION_V2_SCHEMA
        );
        assert_eq!(
            session.database.schema(),
            crate::generated_affine_residual_group_exact_database::GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_DATABASE_V2_SCHEMA
        );
        assert_eq!(
            session.catalog.schema(),
            crate::generated_affine_residual_group_exact_targets::GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_TARGET_CATALOG_V2_SCHEMA
        );
        assert_eq!(
            session.target_state.schema(),
            crate::generated_affine_residual_group_exact_targets::GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_TARGET_STATE_V2_SCHEMA
        );
        assert_eq!(session.source_kind(), fixture.plan.source_kind());
        assert_eq!(session.database.source_kind(), fixture.plan.source_kind());
        assert_eq!(session.catalog.source_kind(), fixture.plan.source_kind());
        assert_eq!(
            session.target_state.source_kind(),
            fixture.plan.source_kind()
        );
        assert!(session.catalog.same_plan_allocation(&fixture.plan));
        assert!(
            session
                .catalog
                .target_uses_exact_plan_authority_allocation_for_test(0)
        );
        session
            .catalog
            .replay(&fixture.family, &fixture.context, &fixture.plan)
            .unwrap();
        session.replay(&fixture.family, &fixture.context).unwrap();

        assert_eq!(
            session
                .catalog
                .replay(&fixture.family, &fixture.context, &foreign.plan),
            Err(GeneratedAffineResidualGroupExactTargetError::WrongPlanAllocation)
        );

        let before_foreign = session_state_snapshot(&session);
        assert!(matches!(
            session.stage_replayed_row(&fixture.family, &fixture.context, &foreign.rows[0],),
            Err(GeneratedAffineResidualGroupExactSessionError::Database(
                GeneratedAffineResidualGroupExactDatabaseError::RowReplay
            ))
        ));
        assert_eq!(session_state_snapshot(&session), before_foreign);

        let mut observed_dependent = 0usize;
        let mut observed_no_target = 0usize;
        for row in &fixture.rows {
            let transaction = session
                .stage_replayed_row(&fixture.family, &fixture.context, row)
                .unwrap();
            let transaction = match session.classify_dependent(transaction) {
                Ok(classified) => {
                    session
                        .commit_dependent(&fixture.family, &fixture.context, classified)
                        .unwrap();
                    observed_dependent += 1;
                    session.replay(&fixture.family, &fixture.context).unwrap();
                    continue;
                }
                Err(failure) => failure.into_transaction(),
            };
            let outcome = session
                .recenter_staged_new_pivot(&fixture.family, &fixture.context, transaction)
                .unwrap();
            match outcome {
                GeneratedAffineResidualGroupExactSessionRecenterOutcome::NoTarget(no_target) => {
                    let committed = session
                        .commit_no_target(&fixture.family, &fixture.context, no_target)
                        .unwrap();
                    session = committed.into_session();
                    observed_no_target += 1;
                    session.replay(&fixture.family, &fixture.context).unwrap();
                }
                GeneratedAffineResidualGroupExactSessionRecenterOutcome::RequiresAffineEqualityRefinement(
                    _,
                ) => panic!("natural Direct Ready target unexpectedly requires refinement"),
                GeneratedAffineResidualGroupExactSessionRecenterOutcome::Ready(ready) => {
                    let before_analysis = session_state_snapshot(&session);
                    let analyzed =
                        GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler::analyze(
                            &fixture.family,
                            &fixture.context,
                            &session,
                            ready,
                            GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default(),
                        )
                        .unwrap();
                    assert_eq!(session_state_snapshot(&session), before_analysis);
                    match analyzed {
                        GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome::ReadyForConditions(
                            ready,
                        ) => {
                            assert_eq!(ready.targets_consumed(), 0);
                            assert_eq!(ready.ready().target_locator(), &fixture.plan.targets()[0]);
                            assert!(!ready.descent().is_empty());
                            session.replay(&fixture.family, &fixture.context).unwrap();
                            return;
                        }
                        GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome::Unsupported(
                            unsupported,
                        ) => panic!(
                            "natural full-cylinder Direct Ready candidate was unsupported: {:?}",
                            unsupported.reason()
                        ),
                    }
                }
            }
        }
        panic!(
            "natural Direct retained rows exhausted without ReadyForConditions: rows={}, dependent={observed_dependent}, no_target={observed_no_target}",
            fixture.rows.len(),
        );
    }

    #[test]
    fn recenter_natural_ready_uses_exact_centering_and_keeps_target_premises_separate() {
        let (family, context, plan) = plan_fixture("exact-session-recenter-natural-ready");
        let session = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            79,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        assert_eq!(plan.free_positions(), [0]);
        assert_eq!(
            plan.authority()
                .authenticated_group_view(&context)
                .unwrap()
                .compact_linear_coefficients(),
            [Integer::from(1), Integer::from(0), Integer::from(0)]
        );

        let pivot_values = [Integer::from(7), Integer::from(M - 1), Integer::from(M - 1)];
        let second_values = [Integer::from(7), Integer::from(M - 2), Integer::from(M - 1)];
        let expected_target_offset = [Integer::from(0), Integer::from(M - 1), Integer::from(M - 1)];
        let target_ordinal = plan
            .targets()
            .iter()
            .position(|locator| {
                plan.physical_frame()
                    .anchor_offset(locator.inventory_position(), locator.case_ordinal())
                    .unwrap()
                    .values()
                    == expected_target_offset
            })
            .expect("the natural affine group must contain the non-anchor target");
        let target_view = session
            .target_state
            .authenticated_view(&family, &context)
            .unwrap();
        let expected_target_premises = match target_view
            .authenticated_target(target_ordinal)
            .unwrap()
        {
            GeneratedAffineResidualGroupAuthenticatedExactTargetView::Ready(target) => {
                target.premises().to_vec()
            }
            GeneratedAffineResidualGroupAuthenticatedExactTargetView::RequiresAffineEqualityRefinement(
                _,
            ) => panic!("the natural target unexpectedly requires equality refinement"),
        };
        drop(target_view);

        let source_guard = symbolic_test_guard(&context);
        let transaction = session
            .stage_authenticated_terms_for_test(
                &context,
                vec![
                    (
                        physical_key(&plan, &second_values),
                        symbolic_test_coefficient(&context),
                    ),
                    (physical_key(&plan, &pivot_values), context.one()),
                ],
                vec![source_guard.clone()],
            )
            .unwrap();
        let before = session_state_snapshot(&session);
        let outcome = session
            .recenter_staged_new_pivot(&family, &context, transaction)
            .unwrap();
        assert_eq!(outcome.targets_consumed(), 0);
        let GeneratedAffineResidualGroupExactSessionRecenterOutcome::Ready(ready) = outcome else {
            panic!("the natural non-anchor target must classify Ready")
        };

        assert_eq!(ready.target_locator(), &plan.targets()[target_ordinal]);
        assert_eq!(
            ready.coefficient_translation(),
            [Integer::from(-7), Integer::from(0), Integer::from(0)]
        );
        assert_eq!(ready.terms().len(), 2);
        let zero_term = ready
            .terms()
            .iter()
            .find(|term| {
                term.shift().values() == [Integer::from(0), Integer::from(0), Integer::from(0)]
            })
            .expect("the pivot must center to zero");
        assert_eq!(zero_term.coefficient(), &context.one());
        let second_term = ready
            .terms()
            .iter()
            .find(|term| {
                term.shift().values() == [Integer::from(0), Integer::from(-1), Integer::from(0)]
            })
            .expect("the second row must center independently of coefficients");
        let expected_second_coefficient = context
            .add(&symbolic_test_coefficient(&context), &context.integer(-7))
            .unwrap();
        assert_eq!(second_term.coefficient(), &expected_second_coefficient);

        assert_eq!(ready.row_guards().len(), 1);
        let d = context
            .lift(&context.base().parameter("d").unwrap())
            .unwrap();
        let expected_guard_polynomial = context
            .numerator_condition(
                &context
                    .add(
                        &context.add(&d, &context.index(0).unwrap()).unwrap(),
                        &context.integer(-7),
                    )
                    .unwrap(),
            )
            .unwrap();
        let recenter_origin = GuardOrigin::GeneratedAffineGroupRecentering {
            solve_group_ordinal: plan.group_ordinal(),
            database_epoch: 79,
            event_ordinal: ready.source_ordinal(),
        };
        assert_eq!(
            ready.row_guards()[0].polynomial(),
            &expected_guard_polynomial
        );
        assert!(
            ready.row_guards()[0]
                .origins()
                .contains(&GuardOrigin::GuardedDivisionDivisorNumerator)
        );
        assert!(ready.row_guards()[0].origins().contains(&recenter_origin));
        assert_eq!(
            ready.row_guards()[0]
                .origins()
                .iter()
                .filter(|origin| *origin == &recenter_origin)
                .count(),
            1
        );
        assert_eq!(ready.target_premises(), expected_target_premises);
        assert!(
            ready
                .target_premises()
                .iter()
                .all(|premise| !premise.origins().contains(&recenter_origin))
        );
        assert_eq!(ready.stats().target_scans(), target_ordinal + 1);
        assert_eq!(ready.stats().unresolved_target_scans(), target_ordinal + 1);
        assert_eq!(ready.source_ordinal(), ready.pivot_ordinal());
        assert_eq!(session_state_snapshot(&session), before);
        assert_eq!(session.target_state.stats().consumed(), 0);
        drop(ready);
        session.replay(&family, &context).unwrap();
        assert_eq!(session_state_snapshot(&session), before);
    }

    #[test]
    fn typed_no_target_commit_is_exact_recoverable_and_resource_atomic() {
        let (family, context, plan) = plan_fixture("exact-session-recenter-no-target");
        let session = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            83,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        let beyond_i64 = Integer::from(i128::from(M) + 1);
        let values = [Integer::from(0), beyond_i64.clone(), beyond_i64];
        assert!(
            plan.targets().iter().all(|locator| {
                plan.physical_frame()
                    .anchor_offset(locator.inventory_position(), locator.case_ordinal())
                    .unwrap()
                    .values()
                    != values
            }),
            "the independent fixture premise must truly be outside the target set"
        );
        let classify_no_target = |owner: &GeneratedAffineResidualGroupExactSession| {
            let transaction = owner
                .stage_authenticated_terms_for_test(
                    &context,
                    vec![(physical_key(&plan, &values), context.one())],
                    Vec::new(),
                )
                .unwrap();
            match owner
                .recenter_staged_new_pivot(&family, &context, transaction)
                .unwrap()
            {
                GeneratedAffineResidualGroupExactSessionRecenterOutcome::NoTarget(outcome) => {
                    outcome
                }
                _ => panic!("an exact offset absent from the plan must return NoTarget"),
            }
        };

        // Dropping the owning classification remains inert.
        let before = session_state_snapshot(&session);
        let dropped = classify_no_target(&session);
        assert_eq!(dropped.source_ordinal(), 0);
        assert_eq!(dropped.pivot_ordinal(), 0);
        assert_eq!(dropped.targets_consumed(), 0);
        assert_eq!(dropped.stats().target_scans(), plan.targets().len());
        assert_eq!(
            dropped.stats().unresolved_target_scans(),
            plan.targets().len()
        );
        assert_eq!(session_state_snapshot(&session), before);
        drop(dropped);
        session.replay(&family, &context).unwrap();
        assert_eq!(session_state_snapshot(&session), before);

        // Two classifications at the same exact transition compete. A foreign
        // owner must return the first one intact before its rightful owner may
        // commit it.
        let accepted = classify_no_target(&session);
        let accepted_offset_weak = Arc::downgrade(&accepted.target_offset);
        let accepted_offset_pointer = Arc::as_ptr(&accepted.target_offset);
        let competing = classify_no_target(&session);
        let initial_target_stats = session.target_state.stats();
        let initial_target_state = Arc::clone(&session.target_state);
        let foreign = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            83,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        let foreign_before = session_state_snapshot(&foreign);
        let failure = foreign
            .commit_no_target(&family, &context, accepted)
            .unwrap_err();
        assert_eq!(
            failure.error(),
            GeneratedAffineResidualGroupExactSessionError::WrongTargetStateAllocation
        );
        assert!(format!("{failure:?}").contains("private_session: \"<redacted>\""));
        assert!(format!("{failure:?}").contains("private_outcome: \"<redacted>\""));
        let (foreign, accepted) = failure.into_recovery().unwrap();
        assert_eq!(accepted.source_ordinal(), 0);
        assert_eq!(accepted.pivot_ordinal(), 0);
        assert_eq!(
            Arc::as_ptr(&accepted.target_offset),
            accepted_offset_pointer
        );
        assert_eq!(session_state_snapshot(&foreign), foreign_before);
        assert_eq!(session_state_snapshot(&session), before);

        let committed = session
            .commit_no_target(&family, &context, accepted)
            .unwrap();
        assert_eq!(committed.database_epoch(), 83);
        assert_eq!(committed.group_ordinal(), plan.group_ordinal());
        assert_eq!(committed.state_version(), 1);
        assert_eq!(committed.source_ordinal(), 0);
        assert_eq!(committed.pivot_ordinal(), 0);
        assert_eq!(committed.targets_consumed(), 0);
        assert_eq!(committed.stats().target_scans(), plan.targets().len());
        assert!(!committed.publishes_rule());
        assert!(!committed.infers_master());
        assert!(Arc::ptr_eq(
            &committed.event,
            committed.session.events.last().unwrap()
        ));
        let event_offset = match &committed.event.disposition {
            GeneratedAffineResidualGroupExactSessionEventDisposition::NoTarget {
                target_offset,
                ..
            } => target_offset,
            _ => panic!("typed NoTarget commit must retain a NoTarget event"),
        };
        assert_eq!(Arc::as_ptr(event_offset), accepted_offset_pointer);
        assert_eq!(
            committed.session.event_stats.target_offset_retained_bytes(),
            committed
                .stats()
                .kernel()
                .target_offset_observed_arc_retained_bytes()
        );
        assert!(format!("{committed:?}").contains("private_session: \"<redacted>\""));
        let mut session = committed.into_session();
        assert!(accepted_offset_weak.upgrade().is_some());
        drop(accepted_offset_weak);
        assert_eq!(session.state_version(), 1);
        assert_eq!(session.database.pivot_count(), 1);
        assert!(!session.target_state.same_allocation(&initial_target_state));
        assert_eq!(
            session.target_state.stats().unresolved(),
            initial_target_stats.unresolved()
        );
        assert_eq!(session.target_state.stats().consumed(), 0);
        session.replay(&family, &context).unwrap();

        // Replay charges the authenticated catalog-size upper bound before
        // re-executing this recenter event. A corrupted historical scan scalar
        // therefore cannot under-admit the work envelope.
        let target_count = session.target_count();
        assert!(target_count > 0);
        let event = Arc::get_mut(session.events.last_mut().unwrap())
            .expect("the committed event has no external strong owner");
        let original_target_scans = match &mut event.disposition {
            GeneratedAffineResidualGroupExactSessionEventDisposition::NoTarget {
                stats, ..
            } => {
                let original = stats.target_scans;
                stats.target_scans = 0;
                original
            }
            _ => panic!("typed NoTarget commit must retain a NoTarget event"),
        };
        session.limits.events.max_replay_target_scans = target_count - 1;
        assert!(matches!(
            session.replay(&family, &context),
            Err(GeneratedAffineResidualGroupExactSessionError::EventResourceLimit {
                resource: "exact session replay target scans",
                requested,
                limit,
            }) if requested == target_count && limit == target_count - 1
        ));
        let event = Arc::get_mut(session.events.last_mut().unwrap())
            .expect("the committed event has no external strong owner");
        match &mut event.disposition {
            GeneratedAffineResidualGroupExactSessionEventDisposition::NoTarget {
                stats, ..
            } => stats.target_scans = original_target_scans,
            _ => unreachable!(),
        }
        session.limits.events.max_replay_target_scans =
            GeneratedAffineResidualGroupExactSessionEventLimits::default().max_replay_target_scans;
        session.replay(&family, &context).unwrap();

        // Ledger replay independently re-censuses the GMP offset payload, so
        // coupled outer-stat edits cannot make a smaller child byte claim
        // authoritative before shadow admission.
        let (offset_integer_bits, offset_retained_bytes) = {
            let event = Arc::get_mut(session.events.last_mut().unwrap())
                .expect("the committed event has no external strong owner");
            let GeneratedAffineResidualGroupExactSessionEventDisposition::NoTarget {
                target_offset,
                ..
            } = &mut event.disposition
            else {
                unreachable!()
            };
            let target_offset = Arc::get_mut(target_offset)
                .expect("the committed offset has no external Arc or Weak owner");
            let exact = (
                target_offset.retained_integer_bits(),
                target_offset.retained_bytes(),
            );
            assert!(exact.1 > 0);
            target_offset.replace_retained_census_for_test(exact.0, exact.1 - 1);
            exact
        };
        assert_eq!(
            session.replay(&family, &context),
            Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch)
        );
        {
            let event = Arc::get_mut(session.events.last_mut().unwrap())
                .expect("the committed event has no external strong owner");
            let GeneratedAffineResidualGroupExactSessionEventDisposition::NoTarget {
                target_offset,
                ..
            } = &mut event.disposition
            else {
                unreachable!()
            };
            Arc::get_mut(target_offset)
                .expect("the committed offset has no external Arc or Weak owner")
                .replace_retained_census_for_test(offset_integer_bits, offset_retained_bytes);
        }
        session.replay(&family, &context).unwrap();

        let committed_snapshot = session_state_snapshot(&session);
        let failure = session
            .commit_no_target(&family, &context, competing)
            .unwrap_err();
        assert_eq!(
            failure.error(),
            GeneratedAffineResidualGroupExactSessionError::WrongTargetStateAllocation
        );
        let (session, competing) = failure.into_recovery().unwrap();
        assert_eq!(competing.source_ordinal(), 0);
        assert_eq!(competing.pivot_ordinal(), 0);
        drop(competing);
        assert_eq!(session_state_snapshot(&session), committed_snapshot);
        session.replay(&family, &context).unwrap();

        // Target-successor resource rejection happens before either retained
        // owner mutates and returns the complete NoTarget outcome.
        let mut limited_limits = GeneratedAffineResidualGroupExactSessionLimits::default();
        limited_limits.events.max_target_state_successor_copies = plan.targets().len() - 1;
        let limited = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            87,
            limited_limits,
        )
        .unwrap();
        let limited_before = session_state_snapshot(&limited);
        let limited_outcome = classify_no_target(&limited);
        let failure = limited
            .commit_no_target(&family, &context, limited_outcome)
            .unwrap_err();
        assert!(matches!(
            failure.error(),
            GeneratedAffineResidualGroupExactSessionError::EventResourceLimit {
                resource: "exact session target-state successor copies",
                requested,
                limit,
            } if requested == plan.targets().len() && limit == plan.targets().len() - 1
        ));
        let (limited, recovered) = failure.into_recovery().unwrap();
        assert_eq!(recovered.source_ordinal(), 0);
        assert_eq!(recovered.pivot_ordinal(), 0);
        drop(recovered);
        assert_eq!(session_state_snapshot(&limited), limited_before);
        limited.replay(&family, &context).unwrap();
    }

    #[test]
    fn recenter_cancels_a_4096_bit_free_coordinate_and_keeps_exact_negative_delta() {
        let (family, context, plan) = plan_fixture("exact-session-recenter-gmp-cancellation");
        let session = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            89,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        let huge = Integer::from(1) << 4096_u32;
        let values = [huge.clone(), Integer::from(0), Integer::from(0)];
        let transaction = session
            .stage_authenticated_terms_for_test(
                &context,
                vec![(physical_key(&plan, &values), context.one())],
                Vec::new(),
            )
            .unwrap();
        let before = session_state_snapshot(&session);
        let outcome = session
            .recenter_staged_new_pivot(&family, &context, transaction)
            .unwrap();
        let GeneratedAffineResidualGroupExactSessionRecenterOutcome::Ready(ready) = outcome else {
            panic!("the exact zero target must be Ready in the natural fixture")
        };
        assert_eq!(
            plan.physical_frame()
                .anchor_offset(
                    ready.target_locator().inventory_position(),
                    ready.target_locator().case_ordinal(),
                )
                .unwrap()
                .values(),
            [Integer::from(0), Integer::from(0), Integer::from(0)]
        );
        assert_eq!(
            ready.coefficient_translation(),
            [-huge, Integer::from(0), Integer::from(0)]
        );
        assert_eq!(ready.terms().len(), 1);
        assert_eq!(
            ready.terms()[0].shift().values(),
            [Integer::from(0), Integer::from(0), Integer::from(0)]
        );
        assert_eq!(ready.terms()[0].coefficient(), &context.one());
        assert!(ready.stats().kernel().geometry_integer_bit_work() > 4096);
        assert_eq!(session_state_snapshot(&session), before);
        drop(ready);
        session.replay(&family, &context).unwrap();
        assert_eq!(session_state_snapshot(&session), before);
    }

    #[test]
    fn recenter_uses_post_top_reduction_leader_and_source_event_not_pivot_ordinal() {
        let (family, context, plan) = plan_fixture("exact-session-recenter-post-top-leader");
        let mut session = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            97,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        let a_values = [Integer::from(7), Integer::from(M - 1), Integer::from(M - 1)];
        let b_values = [Integer::from(6), Integer::from(M - 1), Integer::from(M - 1)];
        let a = physical_key(&plan, &a_values);
        let b = physical_key(&plan, &b_values);
        assert!(b > a, "b must be the raw hardest key of P0 = 2a + b");

        let seed = session
            .stage_authenticated_terms_for_test(
                &context,
                vec![(a.clone(), context.integer(2)), (b.clone(), context.one())],
                Vec::new(),
            )
            .unwrap();
        assert_eq!(
            session.commit_unconsumed(&family, &context, seed).unwrap(),
            GeneratedAffineResidualGroupExactRowOutcome::NewPivot {
                source_ordinal: 0,
                pivot_ordinal: 0,
            }
        );

        // Advance the source event once without inserting another pivot.
        let dependent = session
            .stage_authenticated_terms_for_test(
                &context,
                vec![(a.clone(), context.integer(2)), (b.clone(), context.one())],
                Vec::new(),
            )
            .unwrap();
        let dependent = session.classify_dependent(dependent).unwrap();
        assert_eq!(dependent.source_ordinal(), 1);
        session
            .commit_dependent(&family, &context, dependent)
            .unwrap();
        assert_eq!(session.database.pivot_count(), 1);
        assert_eq!(session.state_version(), 2);

        // Raw b reduces by P0 to -2a. The recenter wrapper must use staged a,
        // not raw b, and must retain the source-event ordinal 2 even though
        // the prospective pivot ordinal is 1.
        let transaction = session
            .stage_authenticated_terms_for_test(
                &context,
                vec![(b, context.one())],
                vec![symbolic_test_guard(&context)],
            )
            .unwrap();
        let joint = session
            .authenticate_staged_new_pivot(&family, &context, &transaction)
            .unwrap();
        assert_eq!(joint.key(), &a);
        assert_eq!(joint.terms().len(), 1);
        assert_eq!(joint.terms().next().unwrap().1, &context.one());
        assert_eq!(joint.normalization_divisor(), &context.integer(-2));
        assert_eq!(joint.reductions().len(), 1);
        assert_eq!(joint.reductions()[0].pivot_ordinal(), 0);
        assert_eq!(joint.reductions()[0].factor(), &context.one());
        assert_eq!(joint.source_ordinal(), 2);
        assert_eq!(joint.pivot_ordinal(), 1);
        drop(joint);

        let before = session_state_snapshot(&session);
        let outcome = session
            .recenter_staged_new_pivot(&family, &context, transaction)
            .unwrap();
        let GeneratedAffineResidualGroupExactSessionRecenterOutcome::Ready(ready) = outcome else {
            panic!("post-top-reduction a must match the natural Ready target")
        };
        assert_eq!(ready.source_ordinal(), 2);
        assert_eq!(ready.pivot_ordinal(), 1);
        assert_eq!(
            ready.coefficient_translation(),
            [Integer::from(-7), Integer::from(0), Integer::from(0)]
        );
        assert_eq!(ready.terms().len(), 1);
        assert_eq!(
            ready.terms()[0].shift().values(),
            [Integer::from(0), Integer::from(0), Integer::from(0)]
        );
        assert_eq!(ready.terms()[0].coefficient(), &context.one());
        assert_eq!(ready.row_guards().len(), 1);
        let source_event_origin = GuardOrigin::GeneratedAffineGroupRecentering {
            solve_group_ordinal: plan.group_ordinal(),
            database_epoch: 97,
            event_ordinal: 2,
        };
        let pivot_ordinal_origin = GuardOrigin::GeneratedAffineGroupRecentering {
            solve_group_ordinal: plan.group_ordinal(),
            database_epoch: 97,
            event_ordinal: 1,
        };
        assert!(
            ready.row_guards()[0]
                .origins()
                .contains(&source_event_origin)
        );
        assert!(
            !ready.row_guards()[0]
                .origins()
                .contains(&pivot_ordinal_origin)
        );
        assert_eq!(session_state_snapshot(&session), before);
        drop(ready);
        session.replay(&family, &context).unwrap();
        assert_eq!(session_state_snapshot(&session), before);
    }

    #[test]
    fn recenter_foreign_stale_and_resource_failures_return_the_exact_transaction() {
        let (family, context, plan) = plan_fixture("exact-session-recenter-recoverable-errors");
        let values = [Integer::from(7), Integer::from(M - 1), Integer::from(M - 1)];
        let key = physical_key(&plan, &values);

        let source = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            101,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        let foreign = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            101,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        let transaction = source
            .stage_authenticated_terms_for_test(
                &context,
                vec![(key.clone(), context.one())],
                Vec::new(),
            )
            .unwrap();
        let transaction_state = Arc::clone(&transaction.target_state);
        let source_before = session_state_snapshot(&source);
        let foreign_before = session_state_snapshot(&foreign);
        let failure = foreign
            .recenter_staged_new_pivot(&family, &context, transaction)
            .unwrap_err();
        assert_eq!(
            failure.error(),
            GeneratedAffineResidualGroupExactSessionRecenterError::Session(
                GeneratedAffineResidualGroupExactSessionError::WrongTargetStateAllocation
            )
        );
        let recovered = failure.into_transaction();
        assert!(Arc::ptr_eq(&recovered.target_state, &transaction_state));
        let recovered_view = source
            .authenticate_staged_new_pivot(&family, &context, &recovered)
            .unwrap();
        assert_eq!(recovered_view.source_ordinal(), 0);
        drop(recovered_view);
        drop(recovered);
        assert_eq!(session_state_snapshot(&source), source_before);
        assert_eq!(session_state_snapshot(&foreign), foreign_before);

        let mut stale_owner = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            103,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        let stale = stale_owner
            .stage_authenticated_terms_for_test(
                &context,
                vec![(key.clone(), context.one())],
                Vec::new(),
            )
            .unwrap();
        let stale_state = Arc::clone(&stale.target_state);
        let accepted = stale_owner
            .stage_authenticated_terms_for_test(
                &context,
                vec![(key.clone(), context.one())],
                Vec::new(),
            )
            .unwrap();
        stale_owner
            .commit_unconsumed(&family, &context, accepted)
            .unwrap();
        let stale_before = session_state_snapshot(&stale_owner);
        let failure = stale_owner
            .recenter_staged_new_pivot(&family, &context, stale)
            .unwrap_err();
        assert_eq!(
            failure.error(),
            GeneratedAffineResidualGroupExactSessionRecenterError::Session(
                GeneratedAffineResidualGroupExactSessionError::WrongTargetStateAllocation
            )
        );
        let recovered = failure.into_transaction();
        assert!(Arc::ptr_eq(&recovered.target_state, &stale_state));
        let failure = stale_owner
            .recenter_staged_new_pivot(&family, &context, recovered)
            .unwrap_err();
        assert_eq!(
            failure.error(),
            GeneratedAffineResidualGroupExactSessionRecenterError::Session(
                GeneratedAffineResidualGroupExactSessionError::WrongTargetStateAllocation
            )
        );
        drop(failure.into_transaction());
        assert_eq!(session_state_snapshot(&stale_owner), stale_before);
        stale_owner.replay(&family, &context).unwrap();

        let mut resource_limits = GeneratedAffineResidualGroupExactSessionLimits::default();
        resource_limits.recenter.kernel.max_terms = 0;
        let limited = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            107,
            resource_limits,
        )
        .unwrap();
        let transaction = limited
            .stage_authenticated_terms_for_test(&context, vec![(key, context.one())], Vec::new())
            .unwrap();
        let transaction_state = Arc::clone(&transaction.target_state);
        let limited_before = session_state_snapshot(&limited);
        let failure = limited
            .recenter_staged_new_pivot(&family, &context, transaction)
            .unwrap_err();
        assert!(matches!(
            failure.error(),
            GeneratedAffineResidualGroupExactSessionRecenterError::Kernel(
                ExactRecenterKernelError::ResourceLimit {
                    resource: "exact recentering terms",
                    requested: 1,
                    limit: 0,
                }
            )
        ));
        let recovered = failure.into_transaction();
        assert!(Arc::ptr_eq(&recovered.target_state, &transaction_state));
        let recovered_view = limited
            .authenticate_staged_new_pivot(&family, &context, &recovered)
            .unwrap();
        assert_eq!(recovered_view.source_ordinal(), 0);
        drop(recovered_view);
        drop(recovered);
        assert_eq!(session_state_snapshot(&limited), limited_before);
        limited.replay(&family, &context).unwrap();
    }

    #[test]
    fn equality_target_commits_only_into_a_sealed_refined_epoch_suspension() {
        let (family, context, plan) =
            equality_refinement_plan_fixture("exact-session-recenter-first-refinement");
        let mut limits = GeneratedAffineResidualGroupExactSessionLimits::default();
        limits.recenter.kernel.max_exact_shift_components = 0;
        limits.recenter.kernel.max_centered_shift_outer_buffer_bytes = 0;
        limits.recenter.kernel.max_borrowed_reference_buffer_bytes = 0;
        limits.recenter.kernel.max_translation_preflight_passes = 0;
        limits.target_state.max_disposition_copies = plan.targets().len();
        let session = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            109,
            limits,
        )
        .unwrap();
        assert!(!plan.targets().is_empty());
        assert_eq!(session.catalog.stats().ready_targets(), 0);
        assert_eq!(
            session.catalog.stats().equality_refinement_targets(),
            plan.targets().len()
        );

        let first_locator = plan.targets()[0];
        let first_anchor = plan
            .physical_frame()
            .anchor_offset(
                first_locator.inventory_position(),
                first_locator.case_ordinal(),
            )
            .unwrap()
            .values()
            .to_vec();
        assert!(
            plan.free_positions()
                .iter()
                .all(|&position| first_anchor[position] == Integer::from(0)),
            "an anchor offset must have zero free coordinates"
        );
        let transaction = session
            .stage_authenticated_terms_for_test(
                &context,
                vec![(physical_key(&plan, &first_anchor), context.one())],
                vec![symbolic_test_guard(&context)],
            )
            .unwrap();
        let before = session_state_snapshot(&session);
        reset_centered_shift_arithmetic_operations_for_test();
        let outcome = session
            .recenter_staged_new_pivot(&family, &context, transaction)
            .unwrap();
        let GeneratedAffineResidualGroupExactSessionRecenterOutcome::RequiresAffineEqualityRefinement(
            refinement,
        ) = outcome
        else {
            panic!("the first exact equality-bearing target must stop matching immediately")
        };
        assert_eq!(refinement.target_locator(), &first_locator);
        assert!(
            !refinement
                .refinement()
                .equality_predicate_ordinals()
                .is_empty()
        );
        assert_eq!(refinement.stats().target_scans(), 1);
        assert_eq!(refinement.stats().unresolved_target_scans(), 1);
        assert_eq!(
            refinement.stats().kernel().exact_shift_components(),
            0,
            "no centered-row preflight may run on the refinement branch"
        );
        assert_eq!(
            refinement.stats().kernel().translation_preflight_passes(),
            0,
            "zero translation limits prove the refinement branch did not translate coefficients"
        );
        assert_eq!(
            centered_shift_arithmetic_operations_for_test(),
            0,
            "the refinement branch must return before centered GMP subtraction"
        );
        assert_eq!(refinement.targets_consumed(), 0);
        assert_eq!(session_state_snapshot(&session), before);

        // A foreign consuming owner must return both the original session and
        // the complete equality outcome before any commit.
        let foreign = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            109,
            limits,
        )
        .unwrap();
        let foreign_before = session_state_snapshot(&foreign);
        let failure = foreign
            .commit_and_suspend_affine_equality_refinement(&family, &context, refinement)
            .unwrap_err();
        assert_eq!(
            failure.error(),
            GeneratedAffineResidualGroupExactSessionError::WrongTargetStateAllocation
        );
        let failure_debug = format!("{failure:?}");
        assert!(failure_debug.contains("private_session: \"<redacted>\""));
        assert!(failure_debug.contains("private_outcome: \"<redacted>\""));
        let (foreign, refinement) = failure.into_recovery().unwrap();
        assert_eq!(session_state_snapshot(&foreign), foreign_before);
        foreign.replay(&family, &context).unwrap();
        assert_eq!(refinement.target_locator(), &first_locator);
        assert_eq!(refinement.source_ordinal(), 0);
        assert_eq!(refinement.pivot_ordinal(), 0);
        assert_eq!(session_state_snapshot(&session), before);

        // Two preparations for the same sealed database transition allocate
        // sibling successor states. Each prepared target must authenticate
        // only the exact successor Arc with which the target layer paired it.
        let first_prepared = session
            .prepare_equality_refinement_suspension(&family, &context, &refinement)
            .unwrap();
        let second_prepared = session
            .prepare_equality_refinement_suspension(&family, &context, &refinement)
            .unwrap();
        let PreparedSessionEqualityRefinementSuspension {
            successor: first_successor,
            target: first_target,
            ..
        } = first_prepared;
        let PreparedSessionEqualityRefinementSuspension {
            successor: second_successor,
            target: second_target,
            ..
        } = second_prepared;
        assert!(!first_successor.same_allocation(&second_successor));
        assert!(first_target.authenticates_source_state(&first_successor));
        assert!(!first_target.authenticates_source_state(&second_successor));
        assert!(second_target.authenticates_source_state(&second_successor));
        assert!(!second_target.authenticates_source_state(&first_successor));
        drop(first_target);
        drop(second_target);
        drop(first_successor);
        drop(second_successor);
        assert_eq!(session_state_snapshot(&session), before);

        // A correct-owner successor limit set exactly one below the required
        // disposition copy count returns both the running session and the full
        // equality outcome without mutation.
        let disposition_copy_limit = plan.targets().len() - 1;
        let mut limited_limits = limits;
        limited_limits.target_state.max_disposition_copies = disposition_copy_limit;
        let limited = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            113,
            limited_limits,
        )
        .unwrap();
        let limited_transaction = limited
            .stage_authenticated_terms_for_test(
                &context,
                vec![(physical_key(&plan, &first_anchor), context.one())],
                vec![symbolic_test_guard(&context)],
            )
            .unwrap();
        let limited_outcome = limited
            .recenter_staged_new_pivot(&family, &context, limited_transaction)
            .unwrap();
        let GeneratedAffineResidualGroupExactSessionRecenterOutcome::RequiresAffineEqualityRefinement(
            limited_refinement,
        ) = limited_outcome
        else {
            panic!("the limited exact target must retain its equality classification")
        };
        let limited_before = session_state_snapshot(&limited);
        let failure = limited
            .commit_and_suspend_affine_equality_refinement(&family, &context, limited_refinement)
            .unwrap_err();
        assert!(matches!(
            failure.error(),
            GeneratedAffineResidualGroupExactSessionError::Target(
                GeneratedAffineResidualGroupExactTargetError::ResourceLimit {
                    resource: "exact target disposition copies",
                    requested,
                    limit,
                }
            ) if requested == plan.targets().len() && limit == disposition_copy_limit
        ));
        let (limited, limited_refinement) = failure.into_recovery().unwrap();
        assert_eq!(limited_refinement.target_locator(), &first_locator);
        assert_eq!(limited_refinement.targets_consumed(), 0);
        assert_eq!(session_state_snapshot(&limited), limited_before);
        limited.replay(&family, &context).unwrap();
        drop(limited_refinement);

        // The equality-pair source-allocation comparison is independently
        // gated. A correct owner with a zero allowance recovers its complete
        // session/outcome pair and leaves the exact state untouched.
        let mut comparison_limited_limits = limits;
        comparison_limited_limits
            .target_state
            .max_source_state_allocation_comparisons = 0;
        let comparison_limited = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            127,
            comparison_limited_limits,
        )
        .unwrap();
        let comparison_limited_transaction = comparison_limited
            .stage_authenticated_terms_for_test(
                &context,
                vec![(physical_key(&plan, &first_anchor), context.one())],
                vec![symbolic_test_guard(&context)],
            )
            .unwrap();
        let comparison_limited_outcome = comparison_limited
            .recenter_staged_new_pivot(&family, &context, comparison_limited_transaction)
            .unwrap();
        let GeneratedAffineResidualGroupExactSessionRecenterOutcome::RequiresAffineEqualityRefinement(
            comparison_limited_refinement,
        ) = comparison_limited_outcome
        else {
            panic!("the comparison-limited target must retain its equality classification")
        };
        let comparison_limited_before = session_state_snapshot(&comparison_limited);
        let failure = comparison_limited
            .commit_and_suspend_affine_equality_refinement(
                &family,
                &context,
                comparison_limited_refinement,
            )
            .unwrap_err();
        assert!(matches!(
            failure.error(),
            GeneratedAffineResidualGroupExactSessionError::Target(
                GeneratedAffineResidualGroupExactTargetError::ResourceLimit {
                    resource: "exact target source-state allocation comparisons",
                    requested: 1,
                    limit: 0,
                }
            )
        ));
        let (comparison_limited, comparison_limited_refinement) = failure.into_recovery().unwrap();
        assert_eq!(
            comparison_limited_refinement.target_locator(),
            &first_locator
        );
        assert_eq!(comparison_limited_refinement.targets_consumed(), 0);
        assert_eq!(
            session_state_snapshot(&comparison_limited),
            comparison_limited_before
        );
        comparison_limited.replay(&family, &context).unwrap();
        drop(comparison_limited_refinement);

        let predecessor_target_state = Arc::clone(&session.target_state);
        let initial_unresolved = session.target_state.stats().unresolved();
        let expected_stats = refinement.stats();
        let expected_predicates = refinement
            .refinement()
            .equality_predicate_ordinals()
            .to_vec();
        let suspended = session
            .commit_and_suspend_affine_equality_refinement(&family, &context, refinement)
            .unwrap();
        assert_eq!(suspended.database_epoch(), 109);
        assert_eq!(suspended.group_ordinal(), plan.group_ordinal());
        assert_eq!(suspended.state_version(), 1);
        assert_eq!(suspended.source_ordinal(), 0);
        assert_eq!(suspended.pivot_ordinal(), 0);
        assert_eq!(suspended.stats(), expected_stats);
        assert_eq!(suspended.target_locator(), &first_locator);
        assert_eq!(
            suspended.refinement().equality_predicate_ordinals(),
            expected_predicates
        );
        assert!(!suspended.has_production_source());
        assert!(Arc::ptr_eq(
            &suspended.event,
            suspended.committed_session.events.last().unwrap()
        ));
        assert_eq!(
            suspended
                .committed_session
                .event_stats
                .target_offset_retained_bytes(),
            expected_stats
                .kernel()
                .target_offset_observed_arc_retained_bytes()
        );
        assert!(matches!(
            &suspended.event.disposition,
            GeneratedAffineResidualGroupExactSessionEventDisposition::RequiresAffineEqualityRefinement { .. }
        ));
        assert_eq!(suspended.targets_consumed(), 0);
        assert!(!suspended.publishes_rule());
        assert!(!suspended.infers_master());
        assert_eq!(suspended.committed_session.database.pivot_count(), 1);
        assert_eq!(
            suspended
                .committed_session
                .target_state
                .stats()
                .unresolved(),
            initial_unresolved
        );
        assert_eq!(
            suspended.committed_session.target_state.stats().consumed(),
            0
        );
        assert!(
            suspended
                .target
                .authenticates_source_state(&suspended.committed_session.target_state)
        );
        assert!(
            !suspended
                .target
                .authenticates_source_state(&predecessor_target_state)
        );
        suspended
            .committed_session
            .database
            .authenticate_target_state_binding(suspended.committed_session.target_state.binding())
            .unwrap();
        suspended.replay(&family, &context).unwrap();
        let suspended_debug = format!("{suspended:?}");
        assert!(suspended_debug.contains("private_committed_session: \"<redacted>\""));
        assert!(suspended_debug.contains("private_event: \"<redacted>\""));
        assert!(suspended_debug.contains("private_successor_target: \"<redacted>\""));
        drop(suspended);
    }

    #[test]
    fn construction_owns_one_database_bound_catalog_and_initial_state() {
        let (family, context, plan) = plan_fixture("exact-session-construction-private");
        let session = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            53,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();

        assert_eq!(
            session.schema(),
            GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_SESSION_V1_SCHEMA
        );
        assert_eq!(session.database_epoch(), 53);
        assert_eq!(session.group_ordinal(), plan.group_ordinal());
        assert_eq!(session.state_version(), 0);
        assert_eq!(session.target_count(), plan.targets().len());
        assert_eq!(
            session.limits(),
            GeneratedAffineResidualGroupExactSessionLimits::default()
        );
        assert!(!session.publishes_rule());
        assert!(!session.infers_master());
        session.replay(&family, &context).unwrap();
        session
            .database
            .authenticate_target_state_binding(session.target_state.binding())
            .unwrap();
    }

    #[test]
    fn production_database_transition_surface_is_session_capability_gated() {
        let database_source = include_str!("generated_affine_residual_group_exact_database.rs");
        let session_source = include_str!("generated_affine_residual_group_exact_session.rs");
        let target_source = include_str!("generated_affine_residual_group_exact_targets.rs");
        let capability = "GeneratedAffineResidualGroupExactSessionDatabaseCapability";

        // Every production entry capable of minting, classifying, or
        // consuming database transition authority names the unforgeable
        // session capability in its signature.
        for (method, expected_occurrences) in [
            ("retain_source_recipe_for_session", 2),
            ("retain_dependent_evidence_for_session", 1),
            ("retain_new_pivot_evidence_for_session", 1),
            ("plan_for_session", 1),
            ("retain_exact_reduction_evidence_for_session", 1),
            ("initial_target_state_binding_for_session", 1),
            ("successor_target_state_binding_for_session", 2),
            ("stage_replayed_row_for_session", 1),
            ("stage_retained_source_recipe_for_session", 1),
            ("authenticate_staged_new_pivot_for_session", 1),
            ("authenticate_staged_dependent_for_session", 1),
            ("prepare_staged_row_commit_for_session", 1),
            ("abort_prepared_staged_row_commit_for_session", 1),
            ("commit_prepared_staged_row_for_session", 1),
        ] {
            let marker = format!("fn {method}");
            let occurrences = database_source
                .match_indices(&marker)
                .map(|(offset, _)| offset)
                .collect::<Vec<_>>();
            assert_eq!(
                occurrences.len(),
                expected_occurrences,
                "capability-gated database seam {method} has an unexpected definition count"
            );
            for start in occurrences {
                let signature_end = database_source[start..]
                    .find(" {")
                    .map(|offset| start + offset)
                    .unwrap_or_else(|| panic!("unterminated signature for {method}"));
                assert!(
                    database_source[start..signature_end].contains(capability),
                    "production database method {method} lacks the session capability"
                );
            }
        }

        // Legacy direct database names no longer exist. Explicit `_for_test`
        // adapters complement Rust's compile-time private-field seal and are
        // absent from a normal library build.
        for method in [
            "initial_target_state_binding",
            "successor_target_state_binding",
            "stage_replayed_row",
            "ingest_replayed_row",
            "authenticate_staged_new_pivot",
            "commit_staged_row",
            "commit_staged_row_for_session",
            "plan",
        ] {
            assert!(
                !database_source.contains(&format!("fn {method}(")),
                "legacy direct database API {method} remains nameable"
            );
        }
        for method in [
            "initial_target_state_binding_for_test",
            "successor_target_state_binding_for_test",
            "stage_replayed_row_for_test",
            "ingest_replayed_row_for_test",
            "commit_staged_row_for_test",
        ] {
            let marker = format!("    pub(crate) fn {method}(");
            let occurrences = database_source.match_indices(&marker).collect::<Vec<_>>();
            assert_eq!(
                occurrences.len(),
                1,
                "test transition adapter {method} must have exactly one definition"
            );
            let prefix = &database_source[..occurrences[0].0];
            assert!(
                prefix.ends_with("    #[cfg(test)]\n"),
                "test transition adapter {method} is not cfg(test)-sealed"
            );
        }
        for method in [
            "stage_retained_source_recipe_for_test",
            "prepare_staged_row_commit_for_test",
            "abort_prepared_staged_row_commit_for_test",
            "commit_prepared_staged_row_for_test",
        ] {
            let marker = format!("    fn {method}(");
            let occurrences = database_source.match_indices(&marker).collect::<Vec<_>>();
            assert_eq!(
                occurrences.len(),
                1,
                "private test authority adapter {method} must have exactly one definition"
            );
            let prefix = &database_source[..occurrences[0].0];
            assert!(
                prefix.ends_with("    #[cfg(test)]\n"),
                "private test authority adapter {method} is not cfg(test)-sealed"
            );
        }
        assert!(!database_source.contains("pub(crate) fn authenticate_staged_dependent("));
        assert!(
            database_source.contains("#[cfg(test)]\n    fn authenticate_staged_new_pivot_for_test")
        );
        assert!(database_source.contains("#[cfg(test)]\n    fn plan_for_test("));

        // Synthetic term ingress exists only so this module can exercise the
        // sealed session wrapper. It must remain absent from production and
        // must still require the same unforgeable session capability.
        let synthetic_marker = "    pub(crate) fn stage_authenticated_terms_for_session(";
        let synthetic_occurrences = database_source
            .match_indices(synthetic_marker)
            .collect::<Vec<_>>();
        assert_eq!(
            synthetic_occurrences.len(),
            1,
            "synthetic session ingress must have exactly one definition"
        );
        let synthetic_start = synthetic_occurrences[0].0;
        assert!(
            database_source[..synthetic_start].ends_with("    #[cfg(test)]\n"),
            "synthetic session ingress is not cfg(test)-sealed"
        );
        let synthetic_signature_end = database_source[synthetic_start..]
            .find(" {\n")
            .map(|offset| synthetic_start + offset)
            .expect("unterminated synthetic session-ingress signature");
        assert!(
            database_source[synthetic_start..synthetic_signature_end].contains(capability),
            "synthetic session ingress lacks the session capability"
        );

        // The capability itself has a private seal and a module-private mint;
        // the session stores it privately and exposes no capability accessor.
        assert!(
            session_source
                .contains("struct GeneratedAffineResidualGroupExactSessionDatabaseCapabilitySeal;")
        );
        assert!(session_source.contains("    fn mint() -> Self"));
        let crate_visible_mint = ["pub(crate)", " fn mint() -> Self"].concat();
        let public_mint = ["pub", " fn mint() -> Self"].concat();
        let capability_accessor = ["fn database_", "capability(&self)"].concat();
        assert!(!session_source.contains(&crate_visible_mint));
        assert!(!session_source.contains(&public_mint));
        assert!(!session_source.contains(&capability_accessor));

        // New pivots cross crate-visible commit authority only through their
        // exact sealed classification. The generic kernel remains private and
        // Ready has no direct commit or transaction extraction surface.
        for method in [
            "commit_no_target",
            "commit_and_suspend_affine_equality_refinement",
        ] {
            assert_eq!(
                session_source
                    .match_indices(&format!("    pub(crate) fn {method}("))
                    .count(),
                1,
                "typed session transition {method} must have exactly one definition"
            );
        }
        let no_target_commit_marker = format!("    pub(crate) fn {}(", "commit_no_target");
        let no_target_commit_start = session_source
            .find(&no_target_commit_marker)
            .expect("missing typed NoTarget commit");
        let no_target_commit_signature_end = session_source[no_target_commit_start..]
            .find(") -> Result")
            .map(|offset| no_target_commit_start + offset)
            .expect("unterminated typed NoTarget commit signature");
        let no_target_commit_signature =
            &session_source[no_target_commit_start..no_target_commit_signature_end];
        assert!(no_target_commit_signature.contains("mut self"));
        assert!(!no_target_commit_signature.contains("&mut self"));

        let committed_no_target = "GeneratedAffineResidualGroupExactSessionCommittedNoTarget";
        let committed_no_target_declaration_start = session_source
            .find(&format!("pub(crate) struct {committed_no_target} {{"))
            .expect("missing committed NoTarget owner declaration");
        let committed_no_target_declaration_end = session_source
            [committed_no_target_declaration_start..]
            .find("\n}\n")
            .map(|offset| committed_no_target_declaration_start + offset)
            .expect("unterminated committed NoTarget owner declaration");
        let committed_no_target_declaration = &session_source
            [committed_no_target_declaration_start..committed_no_target_declaration_end];
        assert!(committed_no_target_declaration.contains("\n    session:"));
        assert!(!committed_no_target_declaration.contains("pub(crate) session:"));
        let committed_no_target_impl_start = session_source
            .find(&format!("impl {committed_no_target} {{"))
            .expect("missing committed NoTarget owner impl");
        let committed_no_target_impl_end = session_source[committed_no_target_impl_start..]
            .find(&format!("impl fmt::Debug for {committed_no_target}"))
            .map(|offset| committed_no_target_impl_start + offset)
            .expect("unterminated committed NoTarget owner impl");
        assert!(
            session_source[committed_no_target_impl_start..committed_no_target_impl_end]
                .contains("fn into_session(")
        );
        let private_unconsumed = ["    fn commit_", "unconsumed("].concat();
        assert!(session_source.contains(&private_unconsumed));
        let crate_visible_unconsumed = ["pub(crate) fn commit_", "unconsumed("].concat();
        assert!(!session_source.contains(&crate_visible_unconsumed));
        for forbidden_method in [
            "commit_recenter_outcome",
            "commit_ready",
            "commit_recenter_ready",
        ] {
            let forbidden = format!("pub(crate) fn {forbidden_method}(");
            assert!(
                !session_source.contains(&forbidden),
                "generic or Ready commit authority escaped through {forbidden}"
            );
        }
        let ready_impl_start = session_source
            .find("impl GeneratedAffineResidualGroupExactSessionRecenterReady {")
            .expect("missing Ready recenter impl");
        let ready_impl_end = session_source[ready_impl_start..]
            .find("impl fmt::Debug for GeneratedAffineResidualGroupExactSessionRecenterReady")
            .map(|offset| ready_impl_start + offset)
            .expect("unterminated Ready recenter impl");
        let ready_impl = &session_source[ready_impl_start..ready_impl_end];
        assert!(!ready_impl.contains("fn commit"));
        assert!(!ready_impl.contains("fn into_transaction"));

        // The successful equality branch is a one-way suspended owner. Its
        // terminal event and all authorities are private, and the safe metadata
        // impl cannot recover, resume, or stage the committed session. The old
        // equality-only source recipe has been retired in favor of the common
        // chronological event recipe.
        let retired_source_recipe = [
            "GeneratedAffineResidualGroupExactSessionSuspended",
            "SourceRecipe",
        ]
        .concat();
        assert!(!session_source.contains(&retired_source_recipe));
        let suspended = "GeneratedAffineResidualGroupExactSessionSuspendedForRefinedEpoch";
        let suspended_declaration_start = session_source
            .find(&format!("pub(crate) struct {suspended} {{"))
            .expect("missing suspended owner declaration");
        let suspended_declaration_end = session_source[suspended_declaration_start..]
            .find("\n}\n")
            .map(|offset| suspended_declaration_start + offset)
            .expect("unterminated suspended owner declaration");
        let suspended_declaration =
            &session_source[suspended_declaration_start..suspended_declaration_end];
        for field in ["committed_session", "event", "target"] {
            assert!(suspended_declaration.contains(&format!("\n    {field}:")));
            assert!(!suspended_declaration.contains(&format!("pub(crate) {field}:")));
            assert!(!suspended_declaration.contains(&format!("pub {field}:")));
        }
        let suspended_impl_start = session_source
            .find(&format!("impl {suspended} {{"))
            .expect("missing suspended owner impl");
        let suspended_impl_end = session_source[suspended_impl_start..]
            .find(&format!("impl fmt::Debug for {suspended}"))
            .map(|offset| suspended_impl_start + offset)
            .expect("unterminated suspended owner impl");
        let suspended_impl = &session_source[suspended_impl_start..suspended_impl_end];
        for forbidden in [
            "fn session(",
            "fn session_mut(",
            "fn into_session(",
            "fn resume(",
            "fn stage_",
            "fn transaction(",
            "fn into_transaction(",
            "fn source_recipe(",
            "fn target_offset(",
            "fn target_offset_values(",
        ] {
            assert!(
                !suspended_impl.contains(forbidden),
                "suspended session authority escaped through {forbidden}"
            );
        }

        // Equality successor allocation and retained-target authority leave
        // the target layer only as one sealed pair. Decomposition requires the
        // private session capability, and no after-the-fact rebind API remains.
        let prepared_pair =
            "GeneratedAffineResidualGroupPreparedEqualityRefinementExactTargetSuccessor";
        let prepared_pair_declaration_start = target_source
            .find(&format!("pub(crate) struct {prepared_pair} {{"))
            .expect("missing prepared equality target-successor pair");
        let prepared_pair_declaration_end = target_source[prepared_pair_declaration_start..]
            .find("\n}\n")
            .map(|offset| prepared_pair_declaration_start + offset)
            .expect("unterminated prepared equality target-successor pair");
        let prepared_pair_declaration =
            &target_source[prepared_pair_declaration_start..prepared_pair_declaration_end];
        for field in ["successor", "target"] {
            assert!(prepared_pair_declaration.contains(&format!("\n    {field}:")));
            assert!(!prepared_pair_declaration.contains(&format!("pub(crate) {field}:")));
            assert!(!prepared_pair_declaration.contains(&format!("pub {field}:")));
        }
        let pair_extraction_marker = format!("    pub(crate) fn {}(", "into_parts_for_session");
        let pair_extraction_start = target_source
            .find(&pair_extraction_marker)
            .expect("missing session-gated equality pair extraction");
        let pair_extraction_end = target_source[pair_extraction_start..]
            .find(") -> (")
            .map(|offset| pair_extraction_start + offset)
            .expect("unterminated equality pair extraction signature");
        assert!(target_source[pair_extraction_start..pair_extraction_end].contains(capability));
        let prepared_pair_impl_start = target_source
            .find(&format!("impl {prepared_pair} {{"))
            .expect("missing prepared equality pair impl");
        let prepared_pair_impl_end = target_source[prepared_pair_impl_start..]
            .find(&format!("impl fmt::Debug for {prepared_pair}"))
            .map(|offset| prepared_pair_impl_start + offset)
            .expect("unterminated prepared equality pair impl");
        let prepared_pair_impl = &target_source[prepared_pair_impl_start..prepared_pair_impl_end];
        for forbidden in ["fn successor(", "fn target(", "fn into_parts("] {
            assert!(
                !prepared_pair_impl.contains(forbidden),
                "prepared equality pair has ungated authority accessor {forbidden}"
            );
        }
        let legacy_rebind = format!("fn {}(", "rebind_to_unconsumed_successor");
        assert!(!target_source.contains(&legacy_rebind));
    }

    #[test]
    fn joint_view_rejects_foreign_state_despite_equal_visible_coordinates() {
        let (family, context, plan) = plan_fixture("exact-session-foreign-state-private");
        let first = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            59,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        let second = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            59,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        assert_eq!(first.database_epoch(), second.database_epoch());
        assert_eq!(first.group_ordinal(), second.group_ordinal());
        assert_eq!(first.state_version(), second.state_version());
        assert_eq!(first.target_count(), second.target_count());
        assert!(!first.target_state.same_allocation(&second.target_state));
        assert_eq!(
            first.authenticate_target_state_allocation(&second.target_state),
            Err(GeneratedAffineResidualGroupExactSessionError::WrongTargetStateAllocation)
        );
        assert!(
            first
                .database
                .authenticate_target_state_binding(second.target_state.binding())
                .is_err()
        );
        assert!(
            second
                .database
                .authenticate_target_state_binding(first.target_state.binding())
                .is_err()
        );

        let source = production_row(&family, &context, &plan);
        let transaction = first
            .stage_replayed_row(&family, &context, &source)
            .unwrap();
        let expected_group = plan.authority().authenticated_group_view(&context).unwrap();
        let expected_ambient_arity = expected_group.ambient_arity();
        let expected_matrix = expected_group.compact_linear_coefficients();
        let expected_staged_live_prospective =
            transaction.staged.staged_live_prospective_retained_bytes();
        let expected_staged_live_observed =
            transaction.staged.staged_live_observed_retained_bytes();
        let expected_target_envelope = transaction
            .target_state
            .stats()
            .combined_retained_byte_envelope();
        let joint = first
            .authenticate_staged_new_pivot(&family, &context, &transaction)
            .unwrap();
        assert_eq!(joint.database_epoch(), 59);
        assert_eq!(joint.group_ordinal(), plan.group_ordinal());
        assert_eq!(joint.anchor_case_ordinal(), plan.anchor_case_ordinal());
        assert_eq!(joint.free_positions(), plan.free_positions());
        assert_eq!(joint.target_locators(), plan.targets());
        assert_eq!(joint.ambient_arity(), expected_ambient_arity);
        assert_eq!(joint.ambient_arity(), context.index_count());
        assert_eq!(
            joint.compact_affine_matrix().len(),
            joint.ambient_arity() * joint.free_positions().len()
        );
        assert!(std::ptr::eq(joint.compact_affine_matrix(), expected_matrix));
        assert_eq!(
            joint.staged_live_prospective_retained_bytes(),
            expected_staged_live_prospective
        );
        assert_eq!(
            joint.staged_live_observed_retained_bytes(),
            expected_staged_live_observed
        );
        assert_eq!(
            joint.target_state_combined_retained_byte_envelope(),
            expected_target_envelope
        );
        assert!(
            joint.staged_live_prospective_retained_bytes()
                <= joint.staged_live_observed_retained_bytes()
        );
        assert!(Arc::ptr_eq(joint.physical_frame(), plan.physical_frame()));
        assert_eq!(joint.target_ordinals().len(), plan.targets().len());
        assert!(
            joint
                .target_ordinals()
                .all(|ordinal| joint.is_target_unresolved(ordinal) == Ok(true))
        );
        assert_eq!(joint.pivot_ordinal(), 0);
        assert_eq!(joint.source_ordinal(), 0);
        assert!(joint.production_source().is_some());
        let source_weak = Arc::downgrade(&source);
        let source_recipe = first
            .database
            .retain_source_recipe_for_session(&first.database_capability, &transaction.staged)
            .unwrap();
        assert!(source_recipe.authenticates_production_source_allocation(&source));
        assert!(joint.terms().len() > 0);
        assert_eq!(joint.key(), joint.terms().next_back().unwrap().0);
        assert!(joint.guards().len() <= source.guard_count());
        assert!(joint.reductions().is_empty());
        assert!(!joint.normalization_divisor().is_zero());
        let first_target = joint.target_ordinals().next().unwrap();
        let retained_target = joint.retain_target(first_target).unwrap();
        assert_eq!(retained_target.solve_ordinal(), first_target);
        let joint_debug = format!("{joint:?}");
        assert!(joint_debug.contains("private_staged_pivot: \"<redacted>\""));
        assert!(joint_debug.contains("private_compact_affine_matrix: \"<redacted>\""));
        assert!(joint_debug.contains("private_target_state: \"<redacted>\""));
        assert!(!joint_debug.contains(plan.stable_manifest()));

        let foreign_transaction = first
            .stage_replayed_row(&family, &context, &source)
            .unwrap();
        let GeneratedAffineResidualGroupExactSessionStagedTransaction { staged, .. } =
            foreign_transaction;
        let forged = GeneratedAffineResidualGroupExactSessionStagedTransaction {
            staged,
            target_state: Arc::clone(&second.target_state),
        };
        assert!(matches!(
            first.authenticate_staged_new_pivot(&family, &context, &forged),
            Err(GeneratedAffineResidualGroupExactSessionError::WrongTargetStateAllocation)
        ));
        assert!(matches!(
            second.authenticate_staged_new_pivot(&family, &context, &forged),
            Err(GeneratedAffineResidualGroupExactSessionError::Database(_))
        ));

        drop(retained_target);
        drop(joint);
        drop(transaction);
        drop(forged);
        drop(source);

        // This is deliberately compositional production coverage: the
        // equality test separately proves the sealed suspension transition.
        // Here the private recipe alone keeps the exact authenticated row
        // allocation alive and capable of replay after every staging owner is
        // gone; it does not claim an end-to-end production equality fixture.
        let retained_source = source_weak
            .upgrade()
            .expect("the private production recipe must keep its row alive");
        assert!(source_recipe.authenticates_production_source_allocation(&retained_source));
        let replayed = first
            .database
            .stage_retained_source_recipe_for_session(
                &first.database_capability,
                &family,
                &context,
                &source_recipe,
            )
            .unwrap();
        drop(replayed);
        drop(retained_source);
        assert!(source_weak.upgrade().is_some());
        drop(source_recipe);
        assert!(source_weak.upgrade().is_none());
    }

    #[test]
    fn typed_dependent_commit_advances_database_and_targets_atomically() {
        let (family, context, plan) = plan_fixture("exact-session-unconsumed-commit-private");
        let mut session = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            61,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        let source = production_row(&family, &context, &plan);
        let initial_state = Arc::clone(&session.target_state);
        let initial_target_stats = session.target_state.stats();

        // Both transactions bind the exact same visible version and exact
        // initial state allocation. Committing either must stale the other.
        let accepted = session
            .stage_replayed_row(&family, &context, &source)
            .unwrap();
        let competing = session
            .stage_replayed_row(&family, &context, &source)
            .unwrap();
        let outcome = session
            .commit_unconsumed(&family, &context, accepted)
            .unwrap();
        assert_eq!(
            outcome,
            GeneratedAffineResidualGroupExactRowOutcome::NewPivot {
                source_ordinal: 0,
                pivot_ordinal: 0,
            }
        );
        assert_eq!(session.database.state_version(), 1);
        assert_eq!(session.target_state.state_version(), 1);
        assert_eq!(session.database.pivot_count(), 1);
        assert!(!session.target_state.same_allocation(&initial_state));
        assert_eq!(
            session.target_state.stats().dispositions(),
            initial_target_stats.dispositions()
        );
        assert_eq!(
            session.target_state.stats().unresolved(),
            initial_target_stats.unresolved()
        );
        assert_eq!(session.target_state.stats().consumed(), 0);
        session.replay(&family, &context).unwrap();

        let failure = session
            .commit_unconsumed(&family, &context, competing)
            .unwrap_err();
        assert_eq!(
            failure.error(),
            GeneratedAffineResidualGroupExactSessionError::WrongTargetStateAllocation
        );
        let competing = failure.into_transaction().unwrap();
        assert_eq!(session.database.state_version(), 1);
        assert_eq!(session.target_state.state_version(), 1);
        assert_eq!(session.database.pivot_count(), 1);
        assert_eq!(session.target_state.stats().consumed(), 0);
        session.replay(&family, &context).unwrap();

        // Even if an internal adversarial caller replaces only the retained
        // state Arc, the independently sealed database stage remains stale and
        // the recoverable preflight failure mutates neither owner.
        let GeneratedAffineResidualGroupExactSessionStagedTransaction { staged, .. } = competing;
        let forged = GeneratedAffineResidualGroupExactSessionStagedTransaction {
            staged,
            target_state: Arc::clone(&session.target_state),
        };
        let failure = session
            .commit_unconsumed(&family, &context, forged)
            .unwrap_err();
        assert_eq!(
            failure.error(),
            GeneratedAffineResidualGroupExactSessionError::Database(
                GeneratedAffineResidualGroupExactDatabaseError::StaleStagedRow
            )
        );
        drop(failure.into_transaction().unwrap());
        assert_eq!(session.database.state_version(), 1);
        assert_eq!(session.target_state.state_version(), 1);
        assert_eq!(session.database.pivot_count(), 1);
        assert_eq!(session.target_state.stats().consumed(), 0);
        session.replay(&family, &context).unwrap();

        // Replaying the identical production row now closes against pivot 0,
        // but the unconsumed transition still advances both state versions.
        let dependent = session
            .stage_replayed_row(&family, &context, &source)
            .unwrap();
        let classified = session.classify_dependent(dependent).unwrap();
        assert_eq!(classified.source_ordinal(), 1);
        assert_eq!(classified.reduction_count(), 1);
        let committed = session
            .commit_dependent(&family, &context, classified)
            .unwrap();
        assert_eq!(committed.source_ordinal(), 1);
        assert_eq!(committed.reductions().len(), 1);
        assert_eq!(session.database.state_version(), 2);
        assert_eq!(session.target_state.state_version(), 2);
        assert_eq!(session.database.pivot_count(), 1);
        assert_eq!(
            session.target_state.stats().unresolved(),
            initial_target_stats.unresolved()
        );
        assert_eq!(session.target_state.stats().consumed(), 0);
        assert!(!session.publishes_rule());
        assert!(!session.infers_master());
        session.replay(&family, &context).unwrap();

        // A target-successor resource failure occurs after the database has
        // authenticated/minted the future binding but before either owner is
        // mutated, and therefore returns the complete transaction.
        let mut limits = GeneratedAffineResidualGroupExactSessionLimits::default();
        limits.target_state.max_disposition_copies = 0;
        let mut limited = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            67,
            limits,
        )
        .unwrap();
        assert!(limited.target_count() > 0);
        let transaction = limited
            .stage_replayed_row(&family, &context, &source)
            .unwrap();
        let failure = limited
            .commit_unconsumed(&family, &context, transaction)
            .unwrap_err();
        assert!(matches!(
            failure.error(),
            GeneratedAffineResidualGroupExactSessionError::Target(
                GeneratedAffineResidualGroupExactTargetError::ResourceLimit {
                    resource: "exact target disposition copies",
                    requested,
                    limit: 0,
                }
            ) if requested == limited.target_count()
        ));
        let recovered = failure.into_transaction().unwrap();
        let recovered_view = limited
            .authenticate_staged_new_pivot(&family, &context, &recovered)
            .unwrap();
        assert_eq!(recovered_view.source_ordinal(), 0);
        drop(recovered_view);
        drop(recovered);
        assert_eq!(limited.database.state_version(), 0);
        assert_eq!(limited.target_state.state_version(), 0);
        assert_eq!(limited.database.pivot_count(), 0);
        assert_eq!(limited.target_state.stats().consumed(), 0);
        limited.replay(&family, &context).unwrap();
    }

    #[test]
    fn production_event_recipe_retains_exact_source_allocation_for_replay() {
        let (family, context, plan) = plan_fixture("exact-session-production-event-recipe");
        let mut session = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            67,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        let source = production_row(&family, &context, &plan);
        let source_weak = Arc::downgrade(&source);
        let transaction = session
            .stage_replayed_row(&family, &context, &source)
            .unwrap();
        drop(source);
        assert!(source_weak.upgrade().is_some());

        let outcome = session
            .commit_unconsumed(&family, &context, transaction)
            .unwrap();
        assert!(matches!(
            outcome,
            GeneratedAffineResidualGroupExactRowOutcome::NewPivot {
                source_ordinal: 0,
                pivot_ordinal: 0,
            }
        ));
        let event = session.events.last().unwrap();
        let retained_source = source_weak
            .upgrade()
            .expect("the chronological event must retain its production source");
        let GeneratedAffineResidualGroupExactSessionEventHead::Replayable { source_recipe, .. } =
            &event.head
        else {
            panic!("ordinary committed event lost replayable source")
        };
        assert!(source_recipe.authenticates_production_source_allocation(&retained_source));
        drop(retained_source);
        session.replay(&family, &context).unwrap();
        assert!(source_weak.upgrade().is_some());
        drop(session);
        assert!(source_weak.upgrade().is_none());
    }

    #[test]
    fn empty_replay_bound_includes_initial_database_and_target_owners() {
        let (family, context, plan) = plan_fixture("exact-session-empty-replay-owner-bound");
        let mut session = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            68,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        assert!(session.events.is_empty());
        session.limits.database.max_database_retained_bytes =
            session.database.stats().retained_database_bytes();
        session.limits.database.max_staged_live_retained_bytes = 0;
        session.limits.target_catalog.max_peak_staging_byte_envelope =
            session.catalog.stats().peak_staging_byte_envelope();
        session
            .limits
            .target_state
            .max_combined_retained_byte_envelope = session
            .target_state
            .stats()
            .combined_retained_byte_envelope();
        session
            .limits
            .target_state
            .max_successor_peak_retained_byte_envelope = 0;
        session.limits.events.max_ledger_replacement_peak_bytes =
            session.event_stats.ledger_replacement_peak_bytes();
        let authenticated_ledger = session.authenticate_event_ledger_census().unwrap();
        let exact_combined = session.replay_combined_retained_peak_bound(authenticated_ledger);
        let shared_plan_owner = plan
            .stats()
            .owner_retained_bytes()
            .checked_add(2 * size_of::<usize>())
            .unwrap();
        let expected_original = session_event_saturating_sum([
            size_of::<GeneratedAffineResidualGroupExactSession>(),
            session.limits.database.max_database_retained_bytes,
            session
                .limits
                .target_state
                .max_combined_retained_byte_envelope,
            authenticated_ledger,
        ]);
        let expected_shadow = session_event_saturating_sum([
            size_of::<GeneratedAffineResidualGroupExactSession>(),
            session
                .limits
                .database
                .max_database_retained_bytes
                .max(session.limits.database.max_staged_live_retained_bytes),
            session.limits.target_catalog.max_peak_staging_byte_envelope,
            session
                .limits
                .target_state
                .max_combined_retained_byte_envelope
                .max(
                    session
                        .limits
                        .target_state
                        .max_successor_peak_retained_byte_envelope,
                ),
            session.limits.events.max_ledger_replacement_peak_bytes,
        ]);
        assert_eq!(
            exact_combined,
            session_event_saturating_sum([shared_plan_owner, expected_original, expected_shadow,])
        );
        session.limits.events.max_replay_combined_retained_bytes = exact_combined;
        session.replay(&family, &context).unwrap();
        session.limits.events.max_replay_combined_retained_bytes = exact_combined - 1;
        assert_eq!(
            session.replay(&family, &context),
            Err(
                GeneratedAffineResidualGroupExactSessionError::EventResourceLimit {
                    resource: "exact session replay combined retained bytes",
                    requested: exact_combined,
                    limit: exact_combined - 1,
                },
            )
        );
    }

    #[test]
    fn event_ledger_replacement_limits_precede_reservation_and_observed_capacity_is_authenticated()
    {
        let (family, context, plan) =
            plan_fixture("exact-session-ledger-prospective-before-reserve");
        let values = [Integer::from(7), Integer::from(M - 1), Integer::from(M - 1)];
        let stage_first = |session: &GeneratedAffineResidualGroupExactSession| {
            session
                .stage_authenticated_terms_for_test(
                    &context,
                    vec![(physical_key(&plan, &values), context.one())],
                    Vec::new(),
                )
                .unwrap()
        };

        // First observe the allocator-selected capacity and exact accepted
        // ledger census for this deterministic one-event transition.
        let mut pilot = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            70,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        reset_event_ledger_replacement_reservations_for_test();
        assert!(matches!(
            pilot
                .commit_unconsumed(&family, &context, stage_first(&pilot))
                .unwrap(),
            GeneratedAffineResidualGroupExactRowOutcome::NewPivot {
                source_ordinal: 0,
                pivot_ordinal: 0,
            }
        ));
        assert_eq!(event_ledger_replacement_reservations_for_test(), 1);
        let exact_stats = pilot.event_stats;
        let individual_event_retained_bytes = pilot.events[0].retained_bytes;
        assert_eq!(
            session_event_outer_buffer_bytes(pilot.events.capacity()).unwrap(),
            exact_stats.ledger_outer_buffer_bytes()
        );
        assert_eq!(
            pilot.authenticate_event_ledger_census().unwrap(),
            exact_stats.ledger_retained_bytes()
        );

        let initial_ledger_retained_bytes = session_event_arc_retained_bytes::<
            GeneratedAffineResidualGroupExactSessionEventAuthority,
        >()
        .unwrap();
        let requested_outer_buffer_bytes =
            session_event_outer_buffer_bytes(1).expect("one event slot must be representable");
        let requested_ledger_retained_bytes = session_event_saturating_sum([
            initial_ledger_retained_bytes,
            requested_outer_buffer_bytes,
            individual_event_retained_bytes,
        ]);
        let requested_replacement_peak_bytes = requested_ledger_retained_bytes;
        assert!(requested_outer_buffer_bytes > 0);
        assert!(requested_ledger_retained_bytes > 0);
        assert!(requested_replacement_peak_bytes > 0);

        // Every requested-capacity one-below limit rejects before the ledger
        // reserve is attempted and before either retained owner commits.
        for (resource, requested, axis) in [
            (
                "exact session event-ledger outer buffer bytes",
                requested_outer_buffer_bytes,
                0usize,
            ),
            (
                "exact session event-ledger retained bytes",
                requested_ledger_retained_bytes,
                1usize,
            ),
            (
                "exact session event-ledger replacement peak bytes",
                requested_replacement_peak_bytes,
                2usize,
            ),
        ] {
            let mut limited = GeneratedAffineResidualGroupExactSession::try_new(
                &family,
                &context,
                Arc::clone(&plan),
                70,
                GeneratedAffineResidualGroupExactSessionLimits::default(),
            )
            .unwrap();
            let transaction = stage_first(&limited);
            match axis {
                0 => limited.limits.events.max_ledger_outer_buffer_bytes = requested - 1,
                1 => limited.limits.events.max_ledger_retained_bytes = requested - 1,
                2 => limited.limits.events.max_ledger_replacement_peak_bytes = requested - 1,
                _ => unreachable!(),
            }
            let before = session_state_snapshot(&limited);
            reset_event_ledger_replacement_reservations_for_test();
            let failure = limited
                .commit_unconsumed(&family, &context, transaction)
                .unwrap_err();
            assert_eq!(
                failure.error(),
                GeneratedAffineResidualGroupExactSessionError::EventResourceLimit {
                    resource,
                    requested,
                    limit: requested - 1,
                }
            );
            assert_eq!(
                event_ledger_replacement_reservations_for_test(),
                0,
                "{resource} must reject before replacement reservation"
            );
            assert_eq!(session_state_snapshot(&limited), before);
            drop(failure.into_transaction().unwrap());
        }

        // The allocator-observed exact limits accept the same transition; the
        // installed capacity and full payload census remain authoritative for
        // both live authentication and chronological shadow replay.
        let mut exact = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            70,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        exact.limits.events.max_ledger_outer_buffer_bytes = exact_stats.ledger_outer_buffer_bytes();
        exact.limits.events.max_ledger_retained_bytes = exact_stats.ledger_retained_bytes();
        exact.limits.events.max_ledger_replacement_peak_bytes =
            exact_stats.ledger_replacement_peak_bytes();
        reset_event_ledger_replacement_reservations_for_test();
        assert!(matches!(
            exact
                .commit_unconsumed(&family, &context, stage_first(&exact))
                .unwrap(),
            GeneratedAffineResidualGroupExactRowOutcome::NewPivot {
                source_ordinal: 0,
                pivot_ordinal: 0,
            }
        ));
        assert_eq!(event_ledger_replacement_reservations_for_test(), 1);
        assert_eq!(exact.event_stats, exact_stats);
        assert_eq!(
            session_event_outer_buffer_bytes(exact.events.capacity()).unwrap(),
            exact.event_stats.ledger_outer_buffer_bytes()
        );
        assert_eq!(
            exact.authenticate_event_ledger_census().unwrap(),
            exact.event_stats.ledger_retained_bytes()
        );
        reset_event_ledger_replacement_reservations_for_test();
        exact.replay(&family, &context).unwrap();
        assert_eq!(event_ledger_replacement_reservations_for_test(), 1);
    }

    #[test]
    fn replay_combined_owner_bound_and_ledger_census_are_exactly_admitted() {
        let (family, context, plan) = plan_fixture("exact-session-replay-owner-bound");
        let mut session = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            69,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        let values = [Integer::from(7), Integer::from(M - 1), Integer::from(M - 1)];
        let transaction = session
            .stage_authenticated_terms_for_test(
                &context,
                vec![(physical_key(&plan, &values), context.one())],
                Vec::new(),
            )
            .unwrap();
        assert!(matches!(
            session
                .commit_unconsumed(&family, &context, transaction)
                .unwrap(),
            GeneratedAffineResidualGroupExactRowOutcome::NewPivot {
                source_ordinal: 0,
                pivot_ordinal: 0,
            }
        ));

        // Tighten every child peak used by the combined replay bound to the
        // authenticated peak actually required by this one-event history.
        // The aggregate limit can then be exercised at its exact charged
        // value and one byte below without relying on an unbounded default.
        let database_stats = session.database.stats();
        session.limits.database.max_database_retained_bytes =
            database_stats.retained_database_bytes();
        session.limits.database.max_staged_live_retained_bytes =
            database_stats.peak_staged_live_retained_bytes();
        session.limits.target_catalog.max_peak_staging_byte_envelope =
            session.catalog.stats().peak_staging_byte_envelope();
        session
            .limits
            .target_state
            .max_combined_retained_byte_envelope = session
            .target_state
            .stats()
            .combined_retained_byte_envelope();
        session
            .limits
            .target_state
            .max_successor_peak_retained_byte_envelope = session
            .target_state
            .stats()
            .successor_peak_retained_byte_envelope();
        session.limits.events.max_ledger_replacement_peak_bytes =
            session.event_stats.ledger_replacement_peak_bytes();
        let authenticated_ledger = session.authenticate_event_ledger_census().unwrap();
        let exact_combined = session.replay_combined_retained_peak_bound(authenticated_ledger);
        assert!(exact_combined > 0);
        session.limits.events.max_replay_combined_retained_bytes = exact_combined;
        session.replay(&family, &context).unwrap();

        session.limits.events.max_replay_combined_retained_bytes = exact_combined - 1;
        assert_eq!(
            session.replay(&family, &context),
            Err(
                GeneratedAffineResidualGroupExactSessionError::EventResourceLimit {
                    resource: "exact session replay combined retained bytes",
                    requested: exact_combined,
                    limit: exact_combined - 1,
                },
            )
        );

        // A lowered persisted ledger scalar is rejected by the payload census
        // before it can influence shadow-memory admission.
        session.limits.events.max_replay_combined_retained_bytes = exact_combined;
        let exact_ledger = session.event_stats.ledger_retained_bytes;
        session.event_stats.ledger_retained_bytes = exact_ledger - 1;
        assert_eq!(
            session.replay(&family, &context),
            Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch)
        );
        session.event_stats.ledger_retained_bytes = exact_ledger;
        session.replay(&family, &context).unwrap();
    }

    #[test]
    fn dependent_classifier_rejects_new_pivot_and_returns_intact_transaction() {
        let (family, context, plan) = plan_fixture("exact-session-dependent-reject-new-pivot");
        let session = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            71,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        let source = production_row(&family, &context, &plan);
        let transaction = session
            .stage_replayed_row(&family, &context, &source)
            .unwrap();
        let failure = session.classify_dependent(transaction).unwrap_err();
        assert_eq!(
            failure.error(),
            GeneratedAffineResidualGroupExactSessionError::Database(
                GeneratedAffineResidualGroupExactDatabaseError::NewPivotStagedRow
            )
        );
        let failure_debug = format!("{failure:?}");
        assert!(failure_debug.contains("private_transaction: \"<redacted>\""));
        let recovered = failure.into_transaction();
        let joint = session
            .authenticate_staged_new_pivot(&family, &context, &recovered)
            .unwrap();
        assert_eq!(joint.source_ordinal(), 0);
        assert_eq!(joint.pivot_ordinal(), 0);
        drop(joint);
        drop(recovered);

        assert_eq!(session.database.state_version(), 0);
        assert_eq!(session.target_state.state_version(), 0);
        assert_eq!(session.database.pivot_count(), 0);
        assert_eq!(session.target_state.stats().consumed(), 0);
        session.replay(&family, &context).unwrap();
    }

    #[test]
    fn dependent_commit_stale_and_foreign_failures_preserve_transaction_authority() {
        let (family, context, plan) = plan_fixture("exact-session-dependent-stale-foreign");
        let mut session = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            73,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        let foreign = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            73,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        let source = production_row(&family, &context, &plan);

        // Seed exactly one pivot through the still-private new-pivot test
        // kernel, then classify two competing dependent stages at version 1.
        let first_pivot = session
            .stage_replayed_row(&family, &context, &source)
            .unwrap();
        assert!(matches!(
            session
                .commit_unconsumed(&family, &context, first_pivot)
                .unwrap(),
            GeneratedAffineResidualGroupExactRowOutcome::NewPivot {
                source_ordinal: 0,
                pivot_ordinal: 0
            }
        ));
        let accepted = session
            .stage_replayed_row(&family, &context, &source)
            .unwrap();
        let competing = session
            .stage_replayed_row(&family, &context, &source)
            .unwrap();
        let accepted = session.classify_dependent(accepted).unwrap();
        let competing = session.classify_dependent(competing).unwrap();
        assert_eq!(accepted.source_ordinal(), competing.source_ordinal());
        assert_eq!(accepted.reduction_count(), competing.reduction_count());

        session
            .commit_dependent(&family, &context, accepted)
            .unwrap();
        let failure = session
            .commit_dependent(&family, &context, competing)
            .unwrap_err();
        assert_eq!(
            failure.error(),
            GeneratedAffineResidualGroupExactSessionError::WrongTargetStateAllocation
        );
        let failure_debug = format!("{failure:?}");
        assert!(failure_debug.contains("private_classification: \"<redacted>\""));
        let recovered_classified = failure.into_classified().unwrap();
        assert_eq!(recovered_classified.source_ordinal(), 1);
        assert_eq!(recovered_classified.reduction_count(), 1);
        let recovered = recovered_classified.into_transaction();
        assert_eq!(session.database.state_version(), 2);
        assert_eq!(session.target_state.state_version(), 2);
        assert_eq!(session.database.pivot_count(), 1);
        session.replay(&family, &context).unwrap();

        // The original target-state allocation is stale and is returned
        // intact by classification. Replacing only that internal Arc proves
        // the independently sealed database transition is stale as well.
        let failure = session.classify_dependent(recovered).unwrap_err();
        assert_eq!(
            failure.error(),
            GeneratedAffineResidualGroupExactSessionError::WrongTargetStateAllocation
        );
        let recovered = failure.into_transaction();
        let GeneratedAffineResidualGroupExactSessionStagedTransaction { staged, .. } = recovered;
        let forged_live_target = GeneratedAffineResidualGroupExactSessionStagedTransaction {
            staged,
            target_state: Arc::clone(&session.target_state),
        };
        let failure = session.classify_dependent(forged_live_target).unwrap_err();
        assert_eq!(
            failure.error(),
            GeneratedAffineResidualGroupExactSessionError::Database(
                GeneratedAffineResidualGroupExactDatabaseError::StaleStagedRow
            )
        );
        let recovered = failure.into_transaction();

        // Finally pair the same untouched database stage with the foreign
        // session's live target allocation. Target authentication succeeds in
        // that owner, but the hidden database allocation rejects the stage.
        let GeneratedAffineResidualGroupExactSessionStagedTransaction { staged, .. } = recovered;
        let forged_foreign = GeneratedAffineResidualGroupExactSessionStagedTransaction {
            staged,
            target_state: Arc::clone(&foreign.target_state),
        };
        let failure = foreign.classify_dependent(forged_foreign).unwrap_err();
        assert_eq!(
            failure.error(),
            GeneratedAffineResidualGroupExactSessionError::Database(
                GeneratedAffineResidualGroupExactDatabaseError::WrongDatabaseAllocation
            )
        );
        drop(failure.into_transaction());
        assert_eq!(foreign.database.state_version(), 0);
        assert_eq!(foreign.target_state.state_version(), 0);
        assert_eq!(foreign.database.pivot_count(), 0);
        foreign.replay(&family, &context).unwrap();
    }
}
