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
//! a consuming typed continuation; equality may commit only into a one-way
//! refined-epoch suspension. A replay-proven IdenticallyBad derivation from
//! Ready may instead distill and commit a same-database rejected-candidate
//! continuation: its pivot is retained, its target stays unresolved, and it
//! publishes no rule or master claim. A private unconsumed-commit kernel proves
//! every atomic database/target-state transition, and no raw successor
//! transition is exposed outside this module. Dropping an unconsumed staged
//! transaction or recenter outcome leaves both retained owners unchanged.

use std::fmt;
use std::mem::size_of;
use std::ops::Range;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::prelude::Integer;

use super::database::{
    GeneratedAffineResidualGroupAuthenticatedStagedNewPivotView,
    GeneratedAffineResidualGroupExactDatabase, GeneratedAffineResidualGroupExactDatabaseError,
    GeneratedAffineResidualGroupExactDatabaseLimits,
    GeneratedAffineResidualGroupExactReductionStep,
    GeneratedAffineResidualGroupPreparedExactRowCommit,
    GeneratedAffineResidualGroupRetainedExactDependentReductions,
    GeneratedAffineResidualGroupRetainedExactSourceRecipe,
    GeneratedAffineResidualGroupRetainedExactUnitPivot, GeneratedAffineResidualGroupStagedExactRow,
};
use super::physical_key::{
    GeneratedAffineResidualGroupLatticeShift, GeneratedAffineResidualGroupPhysicalFrame,
    GeneratedAffineResidualGroupPhysicalKey, GeneratedAffineResidualGroupPhysicalKeyError,
};
use super::physical_row::GeneratedAffineResidualGroupExactPhysicalRow;
use super::plan::{
    GeneratedAffineResidualGroupSolvePlan, GeneratedAffineResidualGroupSolveTargetLocator,
};
use super::recenter::{
    ExactRecenterKernelError, ExactRecenterKernelLimits, ExactRecenterKernelStats,
    ExactRecenteredApplicationRow, ExactRecenteredRow, ExactRecenteredTerm, ExactTargetOffset,
    admit_inert_owner, bounded_add, checked_add, exact_offsets_equal, execute_target_offset,
    observe_inert_owner, preflight_exact_geometry, translate_centered_row,
    verify_target_offset_census,
};
use super::targets::{
    GeneratedAffineResidualGroupExactTargetCatalog,
    GeneratedAffineResidualGroupExactTargetCatalogLimits,
    GeneratedAffineResidualGroupExactTargetCatalogStats,
    GeneratedAffineResidualGroupExactTargetError, GeneratedAffineResidualGroupExactTargetState,
    GeneratedAffineResidualGroupExactTargetStateLimits,
    GeneratedAffineResidualGroupExactTargetStateView,
    GeneratedAffineResidualGroupRetainedEqualityRefinementExactTarget,
    GeneratedAffineResidualGroupRetainedExactTarget,
    GeneratedAffineResidualGroupRetainedReadyExactTarget,
};
use super::telemetry::NativeSparseScalingSnapshot;
use crate::generated_affine_residual_case_premises::GeneratedAffineResidualCaseEqualityRefinementCertificate;
use crate::generated_residual_affine_when_bad::{
    AffineWhenBadArbitraryRelativeCase, AffineWhenBadArbitraryRelativePredicate,
};
use crate::parametric_coefficient::symbolica_sparse::SymbolicaPersistentSparseShallowCapacitySnapshot;
use crate::solver::closure::case_inventory::{
    GeneratedAffineResidualCaseAuthoritySourceKind, GeneratedAffineResidualCaseSourceRowLimits,
    GeneratedAffineResidualCaseSourceRowView, GeneratedAffineResidualInventoryGroupSourceView,
};
use crate::solver::closure::post_ready::{
    GeneratedAffineResidualGroupExactConditionPlanCompiler,
    GeneratedAffineResidualGroupExactWhenBadMaterializationCompiler,
    GeneratedAffineResidualGroupExactWhenBadPartitionCompilation,
    GeneratedAffineResidualGroupExactWhenBadPartitionCompiler,
    GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBadReason,
    GeneratedAffineResidualGroupExactWhenBadRejectedCandidate,
    GeneratedAffineResidualGroupExactWhenBadRejectedCandidateReplayRecipe,
    GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler,
    GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome, PreparedPublication,
    PublicationLeaf, PublicationLeafDisposition, PublicationPayload, PublicationStats,
};
use crate::{
    GuardOrigin, IntegralFamily, IntegralOrderingPolicy, ParametricCoefficient,
    ParametricCoefficientContext, ParametricNonZeroCondition, ParametricPolynomial, SectorMask,
    SymbolicPolynomialPredicateKind,
};

#[cfg(test)]
use super::database::GeneratedAffineResidualGroupExactRowOutcome;

pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_SESSION_V1_SCHEMA: &str =
    "rustred-generated-affine-residual-group-exact-session-v1";
pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_SESSION_V3_SCHEMA: &str =
    "rustred-generated-affine-residual-group-exact-session-v3";
const GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_SESSION_EVENT_V1_SCHEMA: &str =
    "rustred-generated-affine-residual-group-exact-session-event-v1";
const GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_SESSION_EVENT_V3_SCHEMA: &str =
    "rustred-generated-affine-residual-group-exact-session-event-v3";

const fn exact_session_schema_for_source(
    source_kind: GeneratedAffineResidualCaseAuthoritySourceKind,
) -> &'static str {
    match source_kind {
        GeneratedAffineResidualCaseAuthoritySourceKind::InitialInventory => {
            GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_SESSION_V1_SCHEMA
        }
        GeneratedAffineResidualCaseAuthoritySourceKind::CommittedExceptionalSingleton => {
            GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_SESSION_V3_SCHEMA
        }
    }
}

const fn exact_session_event_schema_for_source(
    source_kind: GeneratedAffineResidualCaseAuthoritySourceKind,
) -> &'static str {
    match source_kind {
        GeneratedAffineResidualCaseAuthoritySourceKind::InitialInventory => {
            GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_SESSION_EVENT_V1_SCHEMA
        }
        GeneratedAffineResidualCaseAuthoritySourceKind::CommittedExceptionalSingleton => {
            GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_SESSION_EVENT_V3_SCHEMA
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
/// The type is visible through the exact-session facade solely so the database,
/// target, and rejected-candidate seams can name it in protected signatures.
/// Its seal and constructor remain private here, it is neither `Clone` nor
/// `Default`, and the owning session never returns a borrow. Consequently
/// another production sibling may name the type but cannot produce a value
/// with which to stage, authenticate, or commit a database transition.
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
    RejectedCandidate {
        target_offset: Arc<ExactTargetOffset>,
        locator: GeneratedAffineResidualGroupSolveTargetLocator,
        replay_recipe: GeneratedAffineResidualGroupExactWhenBadRejectedCandidateReplayRecipe,
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
    RejectedCandidate {
        target_offset: &'a ExactTargetOffset,
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
            Self::RejectedCandidate { target_offset, .. } => {
                GeneratedAffineResidualGroupExactSessionEventDispositionView::RejectedCandidate {
                    target_offset,
                }
            }
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
            Self::RejectedCandidate {
                locator,
                replay_recipe,
                stats,
                ..
            } => formatter
                .debug_struct("RejectedCandidate")
                .field("locator", locator)
                .field("reason", &replay_recipe.reason())
                .field("stats", stats)
                .field("private_target_offset", &"<redacted>")
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
            GeneratedAffineResidualGroupExactSessionEventDisposition::RejectedCandidate {
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
            (
                GeneratedAffineResidualGroupExactSessionEventDisposition::RejectedCandidate {
                    target_offset: left_offset,
                    locator: left_locator,
                    replay_recipe: left_recipe,
                    stats: left_stats,
                },
                GeneratedAffineResidualGroupExactSessionEventDisposition::RejectedCandidate {
                    target_offset: right_offset,
                    locator: right_locator,
                    replay_recipe: right_recipe,
                    stats: right_stats,
                },
            ) => {
                left_offset.values() == right_offset.values()
                    && left_locator == right_locator
                    && left_recipe == right_recipe
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

    /// True only when both handles share the exact immutable event
    /// allocation. This is process-local ownership provenance, never a
    /// mathematical or durable identity.
    pub(crate) fn same_event_allocation(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.event, &other.event)
    }

    /// Process-local identity used only by the frozen publication-handoff
    /// compiler to reject two owners of the same committed event.  This value
    /// is never a mathematical, durable, or ordering identity.
    pub(crate) fn event_allocation_identity_for_handoff(&self) -> usize {
        Arc::as_ptr(&self.event) as usize
    }

    /// Process-local identity of the exact session authority behind this
    /// event.  The handoff compiler uses it only to detect conflicting stable
    /// lane keys; canonical ordering continues to use caller-supplied stable
    /// lane metadata and visible event coordinates.
    pub(crate) fn session_authority_allocation_identity_for_handoff(&self) -> usize {
        Arc::as_ptr(&self.event.authority) as usize
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

    /// Owner-visible retained size of this immutable event allocation and its
    /// event-local payload.  This deliberately excludes the separate event
    /// authority and its parent plan/catalog ancestry; campaign lineage
    /// accounting must retain or conservatively charge that shared graph too.
    pub(crate) const fn retained_event_bytes(self) -> usize {
        self.event.retained_bytes
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

    /// Absolute affine constants of the retained Ready target.  These are
    /// borrowed from the target authority; `target_offset()` remains the
    /// distinct parent-frame-relative coordinate used by recentering.
    pub(crate) fn target_constants(self) -> &'session [Integer] {
        self.event
            .authority
            .catalog
            .ready_target_constants(self.target_locator())
            .expect("committed publication lost its immutable Ready-target constants")
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

    /// Stable value identity of the exact generic-source plan inherited by a
    /// narrowed exceptional child.  Allocation ancestry remains bound by the
    /// owning event handle; this string is used only for deterministic child
    /// manifests.
    pub(crate) fn retained_parent_plan_manifest(self) -> &'session str {
        self.event.authority.plan.stable_manifest()
    }

    /// Source representation whose exact value is bound by the retained
    /// parent plan manifest. Legacy V1 plans predate full source-row value
    /// identities, so committed children additionally serialize those rows.
    pub(crate) fn retained_parent_source_kind(
        self,
    ) -> GeneratedAffineResidualCaseAuthoritySourceKind {
        self.event.authority.plan.source_kind()
    }

    /// Number of generic IBP/LI source rows retained by this event's exact
    /// source authority.  A fresh exceptional lane replays these rows; it does
    /// not treat the publication relation as a generated source row.
    pub(crate) fn retained_parent_source_row_count(self) -> usize {
        self.event.authority.plan.authority().source_row_count()
    }

    pub(crate) fn authenticated_retained_parent_source_row(
        self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source_row_ordinal: usize,
        limits: GeneratedAffineResidualCaseSourceRowLimits,
    ) -> Result<
        GeneratedAffineResidualCaseSourceRowView<'session>,
        crate::solver::closure::case_inventory::GeneratedAffineResidualCaseAuthorityError,
    > {
        self.event
            .authority
            .plan
            .authority()
            .authenticated_source_row_view(family, context, source_row_ordinal, limits)
    }

    pub(crate) fn replay_retained_parent_source_authority(
        self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), crate::solver::closure::case_inventory::GeneratedAffineResidualCaseAuthorityError>
    {
        self.event
            .authority
            .plan
            .authority()
            .replay(family, context)
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

    pub(crate) fn event_allocation_identity_for_handoff(&self) -> usize {
        self.event.event_allocation_identity_for_handoff()
    }

    pub(crate) fn session_authority_allocation_identity_for_handoff(&self) -> usize {
        self.event
            .session_authority_allocation_identity_for_handoff()
    }

    #[cfg(test)]
    pub(crate) fn duplicate_for_handoff_test(&self) -> Self {
        Self {
            event_ordinal: self.event_ordinal,
            source_ordinal: self.source_ordinal,
            pivot_ordinal: self.pivot_ordinal,
            retained_event_bytes: self.retained_event_bytes,
            stats: self.stats,
            event: self.event.clone(),
        }
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

/// Typed successful result of committing a pivot whose complete current
/// target domain was proven unusable by exact `WhenBad` analysis.
pub(crate) struct GeneratedAffineResidualGroupExactSessionCommittedRejectedCandidate {
    session: GeneratedAffineResidualGroupExactSession,
    event: Arc<GeneratedAffineResidualGroupExactSessionEvent>,
    source_ordinal: usize,
    pivot_ordinal: usize,
    locator: GeneratedAffineResidualGroupSolveTargetLocator,
    reason: GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBadReason,
    stats: GeneratedAffineResidualGroupExactSessionRecenterStats,
}

impl GeneratedAffineResidualGroupExactSessionCommittedRejectedCandidate {
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

    pub(crate) const fn target_locator(&self) -> GeneratedAffineResidualGroupSolveTargetLocator {
        self.locator
    }

    pub(crate) const fn reason(
        &self,
    ) -> GeneratedAffineResidualGroupExactWhenBadPartitionIdenticallyBadReason {
        self.reason
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

    pub(crate) const fn emits_residual(&self) -> bool {
        false
    }

    /// Continue the same exact database after the rejected candidate's pivot
    /// has been retained and its matched target left unresolved.
    pub(crate) fn into_session(self) -> GeneratedAffineResidualGroupExactSession {
        self.session
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionCommittedRejectedCandidate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionCommittedRejectedCandidate")
            .field("database_epoch", &self.database_epoch())
            .field("group_ordinal", &self.group_ordinal())
            .field("state_version", &self.state_version())
            .field("source_ordinal", &self.source_ordinal)
            .field("pivot_ordinal", &self.pivot_ordinal)
            .field("locator", &self.locator)
            .field("reason", &self.reason)
            .field("stats", &self.stats)
            .field("targets_consumed", &0)
            .field("publishes_rule", &false)
            .field("infers_master", &false)
            .field("emits_residual", &false)
            .field("private_session", &"<redacted>")
            .field("private_event", &"<redacted>")
            .finish()
    }
}

/// Transactional failure of the rejected-candidate session transition.
/// Both the running session and the complete distilled token are returned.
pub(crate) enum GeneratedAffineResidualGroupExactSessionCommitRejectedCandidateFailure {
    Preflight {
        error: GeneratedAffineResidualGroupExactSessionError,
        session: GeneratedAffineResidualGroupExactSession,
        candidate: GeneratedAffineResidualGroupExactWhenBadRejectedCandidate,
    },
}

impl GeneratedAffineResidualGroupExactSessionCommitRejectedCandidateFailure {
    pub(crate) const fn error(&self) -> GeneratedAffineResidualGroupExactSessionError {
        match self {
            Self::Preflight { error, .. } => *error,
        }
    }

    pub(crate) fn into_recovery(
        self,
    ) -> (
        GeneratedAffineResidualGroupExactSession,
        GeneratedAffineResidualGroupExactWhenBadRejectedCandidate,
    ) {
        match self {
            Self::Preflight {
                session, candidate, ..
            } => (session, candidate),
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactSessionCommitRejectedCandidateFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactSessionCommitRejectedCandidateFailure")
            .field("phase", &"preflight")
            .field("error", &self.error())
            .field("private_session", &"<redacted>")
            .field("private_candidate", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualGroupExactSessionCommitRejectedCandidateFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("exact rejected-candidate transition failed before commit")
    }
}

impl std::error::Error for GeneratedAffineResidualGroupExactSessionCommitRejectedCandidateFailure {}

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

/// Live retained-resource components observable from one exact-session owner.
///
/// Native dimensions and entry counts are direct observations of Symbolica's
/// currently committed sparse reducer; entry counts mean stored CSR entries,
/// not semantic nonzeros. The nested capacity snapshot contains public U/L CSR
/// and pivot `Vec` slot capacities only. The byte fields are existing RustRed
/// component envelopes. They deliberately remain separate: the solve plan is a
/// shared campaign authority, some `Arc` payloads overlap, and Symbolica's
/// native heap and private per-thread workspaces require external calibration.
/// This is neither an exhaustive Rust-owner envelope nor a native/RSS census.
/// The campaign layer separately charges the session shell and supplies one
/// deduplicated baseline charge for the shared plan `Arc` and other shared
/// authorities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactSessionResidentResourceSnapshot {
    physical_columns: usize,
    independent_rows: usize,
    native_u_rows: usize,
    native_u_columns: usize,
    native_l_rows: usize,
    native_l_columns: usize,
    native_u_stored_entries: usize,
    native_l_stored_entries: usize,
    native_shallow_capacity_slots: SymbolicaPersistentSparseShallowCapacitySnapshot,
    shared_plan_owner_retained_bytes: usize,
    database_retained_bytes: usize,
    target_state_combined_retained_byte_envelope: usize,
    event_ledger_retained_bytes: usize,
}

impl GeneratedAffineResidualGroupExactSessionResidentResourceSnapshot {
    pub(crate) const fn physical_columns(self) -> usize {
        self.physical_columns
    }

    pub(crate) const fn independent_rows(self) -> usize {
        self.independent_rows
    }

    pub(crate) const fn native_u_rows(self) -> usize {
        self.native_u_rows
    }

    pub(crate) const fn native_u_columns(self) -> usize {
        self.native_u_columns
    }

    pub(crate) const fn native_l_rows(self) -> usize {
        self.native_l_rows
    }

    pub(crate) const fn native_l_columns(self) -> usize {
        self.native_l_columns
    }

    pub(crate) const fn native_u_stored_entries(self) -> usize {
        self.native_u_stored_entries
    }

    pub(crate) const fn native_l_stored_entries(self) -> usize {
        self.native_l_stored_entries
    }

    pub(crate) const fn native_shallow_capacity_slots(
        self,
    ) -> SymbolicaPersistentSparseShallowCapacitySnapshot {
        self.native_shallow_capacity_slots
    }

    pub(crate) const fn shared_plan_owner_retained_bytes(self) -> usize {
        self.shared_plan_owner_retained_bytes
    }

    pub(crate) const fn database_retained_bytes(self) -> usize {
        self.database_retained_bytes
    }

    pub(crate) const fn target_state_combined_retained_byte_envelope(self) -> usize {
        self.target_state_combined_retained_byte_envelope
    }

    pub(crate) const fn event_ledger_retained_bytes(self) -> usize {
        self.event_ledger_retained_bytes
    }
}

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

    pub(crate) fn physical_frame_schema(&self) -> &'static str {
        self.plan.physical_frame().schema()
    }

    pub(crate) fn solve_plan_schema(&self) -> &'static str {
        self.plan.schema()
    }

    pub(crate) fn target_catalog_schema(&self) -> &'static str {
        self.catalog.schema()
    }

    pub(crate) fn target_catalog_stats(
        &self,
    ) -> GeneratedAffineResidualGroupExactTargetCatalogStats {
        self.catalog.stats()
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
    pub(crate) fn native_sparse_scaling_stats(&self) -> NativeSparseScalingSnapshot {
        self.database.stats().native_sparse_scaling().into()
    }

    /// Observe current retained shape without inferring native peak bytes.
    /// Historical stage telemetry may contain a discarded dependent trial's
    /// extra L row, so campaign estimators must use these live components for
    /// the resident successor baseline.
    pub(crate) fn resident_resource_snapshot(
        &self,
    ) -> GeneratedAffineResidualGroupExactSessionResidentResourceSnapshot {
        let database = self.database.resident_resource_snapshot();
        GeneratedAffineResidualGroupExactSessionResidentResourceSnapshot {
            physical_columns: database.physical_columns(),
            independent_rows: database.independent_rows(),
            native_u_rows: database.native_u_rows(),
            native_u_columns: database.native_u_columns(),
            native_l_rows: database.native_l_rows(),
            native_l_columns: database.native_l_columns(),
            native_u_stored_entries: database.native_u_stored_entries(),
            native_l_stored_entries: database.native_l_stored_entries(),
            native_shallow_capacity_slots: database.native_shallow_capacity_slots(),
            shared_plan_owner_retained_bytes: self.plan.stats().owner_retained_bytes(),
            database_retained_bytes: database.retained_database_bytes(),
            target_state_combined_retained_byte_envelope: self
                .target_state
                .stats()
                .combined_retained_byte_envelope(),
            event_ledger_retained_bytes: self.event_stats.ledger_retained_bytes(),
        }
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
                | GeneratedAffineResidualGroupExactSessionEventDisposition::RequiresAffineEqualityRefinement { .. }
                | GeneratedAffineResidualGroupExactSessionEventDisposition::RejectedCandidate { .. },
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
                                | GeneratedAffineResidualGroupExactSessionEventDisposition::RequiresAffineEqualityRefinement { .. }
                                | GeneratedAffineResidualGroupExactSessionEventDisposition::RejectedCandidate { .. },
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
                GeneratedAffineResidualGroupExactSessionEventDisposition::RejectedCandidate {
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
        // Post-Ready rejection replay reruns four nested compilers. Each
        // recorded recipe accounts for the raw recentered Ready row plus the
        // complete retained A + C + P owner graph (P already contains M), then
        // adds the largest phase-local duplicate scratch peak. Replay processes
        // events serially, so only the largest such event recipe coexists with
        // the shadow.
        let rejected_candidate_rederivation_peak = self
            .events
            .iter()
            .filter_map(|event| match &event.disposition {
                GeneratedAffineResidualGroupExactSessionEventDisposition::RejectedCandidate {
                    replay_recipe,
                    ..
                } => Some(replay_recipe.rederivation_owned_logical_peak_upper_bound()),
                _ => None,
            })
            .max()
            .unwrap_or(0);
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
            rejected_candidate_rederivation_peak,
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
                GeneratedAffineResidualGroupExactSessionEventDisposition::RejectedCandidate {
                    replay_recipe,
                    ..
                } => {
                    if !matches!(
                        database_evidence,
                        GeneratedAffineResidualGroupExactSessionEventDatabaseEvidence::NewPivot(_)
                    ) {
                        return Err(
                            GeneratedAffineResidualGroupExactSessionError::ReplayMismatch,
                        );
                    }
                    let outcome = shadow
                        .recenter_staged_new_pivot(family, context, transaction)
                        .map_err(|_| {
                            GeneratedAffineResidualGroupExactSessionError::ReplayMismatch
                        })?;
                    let GeneratedAffineResidualGroupExactSessionRecenterOutcome::Ready(ready) =
                        outcome
                    else {
                        return Err(
                            GeneratedAffineResidualGroupExactSessionError::ReplayMismatch,
                        );
                    };
                    let candidate = shadow
                        .rederive_rejected_candidate(
                            family,
                            context,
                            ready,
                            *replay_recipe,
                        )?;
                    let committed = shadow
                        .commit_rejected_candidate(family, context, candidate)
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
                GeneratedAffineResidualGroupExactSessionEventDispositionView::RejectedCandidate {
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
            | GeneratedAffineResidualGroupExactSessionEventDisposition::RequiresAffineEqualityRefinement { .. }
            | GeneratedAffineResidualGroupExactSessionEventDisposition::RejectedCandidate { .. } => database_commit
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

    /// Commit one replay-authenticated exact `WhenBad` rejection while
    /// retaining its staged pivot and leaving the matched solve target
    /// unresolved. The transition publishes no rule, infers no master, and
    /// emits no residual. Success is the sole same-database continuation;
    /// every preflight failure returns both consumed owners unchanged.
    pub(crate) fn commit_rejected_candidate(
        mut self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        candidate: GeneratedAffineResidualGroupExactWhenBadRejectedCandidate,
    ) -> Result<
        GeneratedAffineResidualGroupExactSessionCommittedRejectedCandidate,
        GeneratedAffineResidualGroupExactSessionCommitRejectedCandidateFailure,
    > {
        let successor = match catch_unwind(AssertUnwindSafe(|| {
            self.prepare_rejected_candidate_successor(family, context, &candidate)
        })) {
            Ok(Ok(successor)) => successor,
            Ok(Err(error)) => {
                return Err(
                    GeneratedAffineResidualGroupExactSessionCommitRejectedCandidateFailure::Preflight {
                        error,
                        session: self,
                        candidate,
                    },
                );
            }
            Err(_) => {
                return Err(
                    GeneratedAffineResidualGroupExactSessionCommitRejectedCandidateFailure::Preflight {
                        error: GeneratedAffineResidualGroupExactSessionError::SymbolicaPanic,
                        session: self,
                        candidate,
                    },
                );
            }
        };
        let (ready, replay_recipe) = candidate.into_parts_for_session();
        let reason = replay_recipe.reason();
        let GeneratedAffineResidualGroupExactSessionRecenterReady {
            transaction,
            target,
            target_offset,
            recentered,
            source_ordinal,
            pivot_ordinal,
            stats,
        } = ready;
        let locator = *target.locator();
        let prepared = match self.prepare_session_event_transition(
            transaction,
            successor,
            GeneratedAffineResidualGroupExactSessionEventDisposition::RejectedCandidate {
                target_offset: Arc::clone(&target_offset),
                locator,
                replay_recipe,
                stats,
            },
            source_ordinal,
            ExpectedSessionEventDatabaseOutcome::NewPivot { pivot_ordinal },
        ) {
            Ok(prepared) => prepared,
            Err(failure) => {
                let ready = GeneratedAffineResidualGroupExactSessionRecenterReady {
                    transaction: failure.transaction,
                    target,
                    target_offset,
                    recentered,
                    source_ordinal,
                    pivot_ordinal,
                    stats,
                };
                let candidate =
                    GeneratedAffineResidualGroupExactWhenBadRejectedCandidate::from_parts_for_session(
                        &self.database_capability,
                        ready,
                        replay_recipe,
                    );
                return Err(
                    GeneratedAffineResidualGroupExactSessionCommitRejectedCandidateFailure::Preflight {
                        error: failure.error,
                        session: self,
                        candidate,
                    },
                );
            }
        };
        let event = self.commit_prepared_unconsumed(prepared);
        drop(target);
        drop(target_offset);
        drop(recentered);
        Ok(
            GeneratedAffineResidualGroupExactSessionCommittedRejectedCandidate {
                session: self,
                event,
                source_ordinal,
                pivot_ordinal,
                locator,
                reason,
                stats,
            },
        )
    }

    fn prepare_rejected_candidate_successor(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        candidate: &GeneratedAffineResidualGroupExactWhenBadRejectedCandidate,
    ) -> Result<
        Arc<GeneratedAffineResidualGroupExactTargetState>,
        GeneratedAffineResidualGroupExactSessionError,
    > {
        let ready = candidate.ready();
        let geometry = self.authenticated_ready_geometry(family, context, ready)?;
        if geometry.locator() != *candidate.target_locator()
            || ready.source_ordinal() != candidate.source_ordinal()
            || ready.pivot_ordinal() != candidate.pivot_ordinal()
        {
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
        }
        drop(geometry);
        self.prepare_unchanged_target_successor(family, context, &ready.transaction)
    }

    /// Rebuild the complete post-Ready derivation during chronological replay.
    /// A recorded compact reason is never accepted as proof by itself: every
    /// compiler is rerun under the exact limits retained by the live commit,
    /// and the resulting terminal recipe must compare exactly.
    fn rederive_rejected_candidate(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        ready: GeneratedAffineResidualGroupExactSessionRecenterReady,
        expected: GeneratedAffineResidualGroupExactWhenBadRejectedCandidateReplayRecipe,
    ) -> Result<
        GeneratedAffineResidualGroupExactWhenBadRejectedCandidate,
        GeneratedAffineResidualGroupExactSessionError,
    > {
        let analyzed = GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler::analyze(
            family,
            context,
            self,
            ready,
            expected.ready_analysis_limits(),
        )
        .map_err(|_| GeneratedAffineResidualGroupExactSessionError::ReplayMismatch)?;
        let GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome::ReadyForConditions(ready) =
            analyzed
        else {
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
        };
        let plan = GeneratedAffineResidualGroupExactConditionPlanCompiler::compile(
            family,
            context,
            self,
            ready,
            expected.condition_plan_limits(),
        )
        .map_err(|_| GeneratedAffineResidualGroupExactSessionError::ReplayMismatch)?;
        let materialization =
            GeneratedAffineResidualGroupExactWhenBadMaterializationCompiler::compile(
                family,
                context,
                self,
                plan,
                expected.materialization_limits(),
            )
            .map_err(|_| GeneratedAffineResidualGroupExactSessionError::ReplayMismatch)?;
        let partition = GeneratedAffineResidualGroupExactWhenBadPartitionCompiler::compile(
            family,
            context,
            self,
            materialization,
            expected.partition_limits(),
        )
        .map_err(|_| GeneratedAffineResidualGroupExactSessionError::ReplayMismatch)?;
        let GeneratedAffineResidualGroupExactWhenBadPartitionCompilation::IdenticallyBad(owner) =
            partition
        else {
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
        };
        let candidate = owner
            .into_rejected_candidate(family, context, self)
            .map_err(|_| GeneratedAffineResidualGroupExactSessionError::ReplayMismatch)?;
        if candidate.replay_recipe() != expected {
            return Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch);
        }
        Ok(candidate)
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
pub(super) mod tests {
    use super::*;

    use super::super::physical_key::{
        GeneratedAffineResidualGroupPhysicalFrame, GeneratedAffineResidualGroupPhysicalKeyLimits,
    };
    use super::super::physical_row::{
        GeneratedAffineResidualGroupExactPhysicalRowCompiler,
        GeneratedAffineResidualGroupExactPhysicalRowLimits,
    };
    use super::super::plan::GeneratedAffineResidualGroupSolvePlanLimits;
    use super::super::targets::GeneratedAffineResidualGroupExactTargetStateStats;
    use crate::generated_affine_parametric_ordering::{
        GeneratedAffineParametricOrderingCertificate, GeneratedAffineParametricOrderingLimits,
    };
    use crate::generated_affine_prepare_point_schedule::{
        GeneratedAffinePreparePointScheduleCertificate, GeneratedAffinePreparePointScheduleLimits,
    };
    use crate::generated_affine_residual_boolean_cover::{
        GeneratedAffineResidualBooleanCoverCompiler, GeneratedAffineResidualBooleanCoverLimits,
    };
    use crate::generated_affine_residual_case_bound_relation::{
        GeneratedAffineResidualCaseBoundRelationCompilation,
        GeneratedAffineResidualCaseBoundRelationCompiler,
        GeneratedAffineResidualCaseBoundRelationLimits,
    };
    use crate::generated_affine_residual_case_completed_bound_row::{
        GeneratedAffineResidualCaseCompletedBoundRowCompiler,
        GeneratedAffineResidualCaseCompletedBoundRowLimits,
    };
    use crate::generated_affine_residual_case_premises::{
        GeneratedAffineResidualCasePremisesLimits, GeneratedAffineResidualCasePremisesOutcome,
        compile_generated_affine_residual_case_premises,
    };
    use crate::generated_affine_residual_source_authority::GeneratedAffineResidualSourceAuthority;
    use crate::solver::closure::case_inventory::{
        GeneratedAffineResidualCaseAuthority, GeneratedAffineResidualCaseAuthorityLimits,
        GeneratedAffineResidualCaseInventoryCompiler, GeneratedAffineResidualCaseInventoryLimits,
    };
    use crate::{
        AffineDenominator, GeneratedSectorDiscoveryCompiler, GeneratedSectorDiscoveryLimits,
        GeneratedSectorLiveLeafQueueCompiler, GeneratedSectorLiveLeafQueueLimits,
        IntegralOrderingPolicy, ParametricIbpGenerator, SectorMask, algebra::CoefficientContext,
    };

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
            for layer in schedule.layers() {
                for point_ordinal in 0..layer.point_count() {
                    for source_row_ordinal in 0..authority.source_row_count() {
                        let point = schedule
                            .point_handle(layer.depth(), point_ordinal)
                            .expect("a scheduled point must have an authenticated handle");
                        let compilation =
                            GeneratedAffineResidualCaseBoundRelationCompiler::compile(
                                family,
                                context,
                                Arc::clone(&authority),
                                Arc::clone(&ordering),
                                Arc::clone(&schedule),
                                Arc::clone(&premises),
                                source_row_ordinal,
                                point,
                                GeneratedAffineResidualCaseBoundRelationLimits::default(),
                            )
                            .unwrap();
                        let GeneratedAffineResidualCaseBoundRelationCompilation::Retained(bound) =
                            compilation
                        else {
                            continue;
                        };
                        let completed = Arc::new(
                            GeneratedAffineResidualCaseCompletedBoundRowCompiler::compile(
                                family,
                                context,
                                Arc::clone(&authority),
                                Arc::clone(&ordering),
                                Arc::clone(&schedule),
                                Arc::clone(&premises),
                                Arc::new(bound),
                                GeneratedAffineResidualCaseCompletedBoundRowLimits::default(),
                            )
                            .unwrap(),
                        );
                        return Arc::new(
                            GeneratedAffineResidualGroupExactPhysicalRowCompiler::compile(
                                family,
                                context,
                                completed,
                                Arc::clone(frame),
                                GeneratedAffineResidualGroupExactPhysicalRowLimits::default(),
                            )
                            .unwrap(),
                        );
                    }
                }
            }
        }
        panic!("the generated-affine fixture produced no authenticated physical row")
    }

    #[test]
    fn sentinel_exact_transaction_rollback_preserves_real_replayed_row() {
        let (family, context, plan) = plan_fixture("exact-session-rollback-sentinel");
        let session = GeneratedAffineResidualGroupExactSession::try_new(
            &family,
            &context,
            Arc::clone(&plan),
            71,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        let source = production_row(&family, &context, &plan);
        let before = session_state_snapshot(&session);
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
        let recovered = failure.into_transaction();
        let joint = session
            .authenticate_staged_new_pivot(&family, &context, &recovered)
            .unwrap();
        assert_eq!(joint.source_ordinal(), 0);
        assert_eq!(joint.pivot_ordinal(), 0);
        drop(joint);
        drop(recovered);

        assert_eq!(session_state_snapshot(&session), before);
        session.replay(&family, &context).unwrap();
    }
}
