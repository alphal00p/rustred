//! Sealed LiteRed-style top reduction for one exact affine group.
//!
//! This owner is deliberately narrower than public rule discovery. It binds
//! one exact solve-plan/frame allocation and keeps algebraic unit pivots for
//! the lifetime of that group. For every submitted raw row, Symbolica's sparse
//! reducer owns the ordered forward-elimination transcript over the complete
//! physical-key catalog. RustRed replays that transcript through the guarded
//! provenance path before accepting it. A known hardest key is substituted;
//! the first unknown hardest key is normalized and stored immediately. In
//! particular, known easier keys in that new pivot's tail are not rewritten.
//!
//! Recentring, target matching, `WhenBad`, rule publication, master inference,
//! and adaptive scheduling are intentionally outside this V1 algebraic seam.
//! Exact GMP key-comparison bit-work and `Vec` move-work metering remain the
//! next isolated resource slice; the present limits bound their cardinalities
//! but do not yet charge operand bit length or insertion distance.
//! Coefficient-work limits bound the algebraic staging work. There is
//! intentionally no byte-valued native-temporary claim in V1: the current
//! coefficient ledger exposes no sound pre-Symbolica peak-memory preflight.

use std::fmt;
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{
    Arc, Weak,
    atomic::{AtomicU64, Ordering},
};

#[cfg(test)]
use std::cell::Cell;

use super::physical_key::{
    GeneratedAffineResidualGroupPhysicalFrame, GeneratedAffineResidualGroupPhysicalKey,
};
use super::physical_row::{
    GeneratedAffineResidualGroupExactPhysicalRow,
    GeneratedAffineResidualGroupReplayedExactPhysicalRow,
};
use super::plan::{
    GeneratedAffineResidualGroupSolvePlan, GeneratedAffineResidualGroupSolvePlanReplayLimits,
};
use super::session::GeneratedAffineResidualGroupExactSessionDatabaseCapability;
use crate::parametric_coefficient::insert_parametric_condition;
use crate::parametric_coefficient::symbolica_sparse::{
    SymbolicaParametricSparseError, SymbolicaParametricSparseInputEntry,
    SymbolicaParametricSparseInputRow, SymbolicaPersistentSparseLimits,
    SymbolicaPersistentSparseOutcome, SymbolicaPersistentSparseReducer,
    SymbolicaPersistentSparseShallowCapacitySnapshot, SymbolicaPersistentSparseStats,
};
#[cfg(test)]
use crate::parametric_coefficient::symbolica_sparse::{
    SymbolicaParametricSparseLimits, SymbolicaParametricSparseOutcome, forward_reduce_last_row,
};
use crate::parametric_elimination::{
    ParametricCoefficientWorkLedger, ParametricCoefficientWorkLedgerLimits,
    ParametricCoefficientWorkPhase,
};
use crate::solver::closure::case_inventory::GeneratedAffineResidualCaseAuthoritySourceKind;
use crate::{
    GuardOrigin, IntegralFamily, ParametricCoefficient, ParametricCoefficientContext,
    ParametricNonZeroCondition,
};

pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_DATABASE_V1_SCHEMA: &str =
    "rustred-generated-affine-residual-group-exact-database-v1";
pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_DATABASE_V3_SCHEMA: &str =
    "rustred-generated-affine-residual-group-exact-database-v3";

const fn exact_database_schema_for_source(
    source_kind: GeneratedAffineResidualCaseAuthoritySourceKind,
) -> &'static str {
    match source_kind {
        GeneratedAffineResidualCaseAuthoritySourceKind::InitialInventory => {
            GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_DATABASE_V1_SCHEMA
        }
        GeneratedAffineResidualCaseAuthoritySourceKind::CommittedExceptionalSingleton => {
            GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_DATABASE_V3_SCHEMA
        }
    }
}

/// Process-unique identity source for sealed staged-row tokens. The counter
/// never wraps: exhausting it is reported before a database is constructed.
static NEXT_EXACT_DATABASE_NONCE: AtomicU64 = AtomicU64::new(1);

/// Process-unique identity source for staged database transitions. Zero is
/// reserved for the pristine database state, and this counter never wraps.
static NEXT_EXACT_DATABASE_TRANSITION_NONCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ExactDatabaseTransitionIdentity(u64);

impl ExactDatabaseTransitionIdentity {
    const PRISTINE: Self = Self(0);
}

#[cfg(test)]
thread_local! {
    static FAIL_NEXT_LOOKUP_REPLACEMENT_ALLOCATION: Cell<bool> = const { Cell::new(false) };
}

#[cfg(test)]
fn fail_next_lookup_replacement_allocation_for_test() {
    FAIL_NEXT_LOOKUP_REPLACEMENT_ALLOCATION.with(|fail| fail.set(true));
}

#[cfg(test)]
fn take_fail_next_lookup_replacement_allocation_for_test() -> bool {
    FAIL_NEXT_LOOKUP_REPLACEMENT_ALLOCATION.with(|fail| fail.replace(false))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactDatabaseLimits {
    /// Independent native-algebra envelope for one retained Symbolica sparse
    /// fork. This is deliberately separate from `coefficient_work`:
    /// the latter meters the provenance/guard replay that alone may enter the
    /// retained database, while this budget meters Symbolica's differential
    /// authority transcript.
    pub(crate) symbolica_sparse: SymbolicaPersistentSparseLimits,
    pub(crate) coefficient_work: ParametricCoefficientWorkLedgerLimits,
    /// Caller-owned budget for authenticating the retained solve plan before
    /// this database accepts its allocation identity. Keeping this inside the
    /// database limits makes the replay authority persistent rather than
    /// silently substituting a library default at construction time.
    pub(crate) solve_plan_replay: GeneratedAffineResidualGroupSolvePlanReplayLimits,
    pub(crate) max_pivots: usize,
    pub(crate) max_terms_per_row: usize,
    pub(crate) max_guards_per_row: usize,
    pub(crate) max_guard_origins: usize,
    pub(crate) max_reductions_per_row: usize,
    /// Allocation-free borrowed-input census admitted before the database
    /// allocates its ingress term/guard buffers or deep-copies coefficients and
    /// guards. This is a visible Rust-owned staging bound, not a claim about
    /// Symbolica's internal arithmetic workspace.
    pub(crate) max_ingress_retained_bytes: usize,
    /// Pre-commit retained payload admitted for one pivot that is about to
    /// become persistent database state. This deliberately does not claim to
    /// bound the earlier top-reduction scratch peak.
    pub(crate) max_candidate_retained_bytes: usize,
    /// Cumulative charged retained payload of this database owner.
    pub(crate) max_database_retained_bytes: usize,
    /// Coexistence bound for this live database and one returned sealed stage.
    /// It charges the token inline (including shared `Arc` handles), its owned
    /// vectors/deep coefficient payload, and empty replacement-buffer
    /// capacities. The complete uniquely retained source pipeline is charged
    /// once even when stage/recipe handles clone its outer `Arc`; plan/frame
    /// ancestry is pointer-deduplicated. Symbolica's opaque retained reducer
    /// heap (including the staged successor) and earlier arithmetic scratch
    /// remain outside this byte bound; native entry counts are bounded by
    /// `symbolica_sparse` instead.
    pub(crate) max_staged_live_retained_bytes: usize,
}

impl Default for GeneratedAffineResidualGroupExactDatabaseLimits {
    fn default() -> Self {
        const LARGE_BYTES: usize = 256 * 1024 * 1024 * 1024;
        const MAX_PIVOTS: usize = 16_000_000;
        let mut symbolica_sparse = SymbolicaPersistentSparseLimits::default();
        symbolica_sparse.max_independent_rows_after = MAX_PIVOTS;
        Self {
            symbolica_sparse,
            coefficient_work: ParametricCoefficientWorkLedgerLimits::default(),
            solve_plan_replay: GeneratedAffineResidualGroupSolvePlanReplayLimits::default(),
            max_pivots: MAX_PIVOTS,
            max_terms_per_row: 16_000_000,
            max_guards_per_row: 16_000_000,
            max_guard_origins: 64_000_000,
            max_reductions_per_row: 16_000_000,
            max_ingress_retained_bytes: LARGE_BYTES,
            max_candidate_retained_bytes: LARGE_BYTES,
            max_database_retained_bytes: 2 * LARGE_BYTES,
            max_staged_live_retained_bytes: 4 * LARGE_BYTES,
        }
    }
}

/// Fixed-size observational census for one successfully validated native
/// sparse stage. It owns no reducer state; its metric values affect neither
/// algebraic admission nor any database identity. Its fixed inline footprint
/// remains honestly included in owner byte accounting.
///
/// Coefficient-work counters cover the adapter's checked input copies, native
/// field callbacks, and checked returned-row copies. They exclude pre-native
/// row validation and the database's separate guarded provenance replay.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactNativeSparseStageStats {
    rows: usize,
    physical_columns: usize,
    input_entries: usize,
    prospective_native_output_entries: usize,
    observed_native_output_entries: usize,
    native_u_entries: usize,
    native_l_entries: usize,
    returned_trace_entries: usize,
    coefficient_algebra_work: usize,
    coefficient_exponent_entry_work: usize,
    coefficient_integer_bit_work: usize,
}

impl GeneratedAffineResidualGroupExactNativeSparseStageStats {
    fn from_adapter(stats: SymbolicaPersistentSparseStats) -> Self {
        let coefficient_work = stats.coefficient_work();
        Self {
            rows: stats.independent_rows_before().saturating_add(1),
            physical_columns: stats.physical_columns_after(),
            input_entries: stats.candidate_input_entries(),
            prospective_native_output_entries: stats.prospective_native_output_entries(),
            observed_native_output_entries: stats.observed_native_output_entries(),
            native_u_entries: stats.trial_native_u_entries_after(),
            native_l_entries: stats.trial_native_l_entries_after(),
            returned_trace_entries: stats.returned_trace_entries(),
            coefficient_algebra_work: coefficient_work.algebra_work(),
            coefficient_exponent_entry_work: coefficient_work.exponent_entry_work(),
            coefficient_integer_bit_work: coefficient_work.integer_bit_work(),
        }
    }

    fn componentwise_max(self, other: Self) -> Self {
        Self {
            rows: self.rows.max(other.rows),
            physical_columns: self.physical_columns.max(other.physical_columns),
            input_entries: self.input_entries.max(other.input_entries),
            prospective_native_output_entries: self
                .prospective_native_output_entries
                .max(other.prospective_native_output_entries),
            observed_native_output_entries: self
                .observed_native_output_entries
                .max(other.observed_native_output_entries),
            native_u_entries: self.native_u_entries.max(other.native_u_entries),
            native_l_entries: self.native_l_entries.max(other.native_l_entries),
            returned_trace_entries: self
                .returned_trace_entries
                .max(other.returned_trace_entries),
            coefficient_algebra_work: self
                .coefficient_algebra_work
                .max(other.coefficient_algebra_work),
            coefficient_exponent_entry_work: self
                .coefficient_exponent_entry_work
                .max(other.coefficient_exponent_entry_work),
            coefficient_integer_bit_work: self
                .coefficient_integer_bit_work
                .max(other.coefficient_integer_bit_work),
        }
    }

    fn saturating_componentwise_add(self, other: Self) -> (Self, bool) {
        let mut saturated = false;
        let mut add = |left: usize, right: usize| match left.checked_add(right) {
            Some(value) => value,
            None => {
                saturated = true;
                usize::MAX
            }
        };
        let sum = Self {
            rows: add(self.rows, other.rows),
            physical_columns: add(self.physical_columns, other.physical_columns),
            input_entries: add(self.input_entries, other.input_entries),
            prospective_native_output_entries: add(
                self.prospective_native_output_entries,
                other.prospective_native_output_entries,
            ),
            observed_native_output_entries: add(
                self.observed_native_output_entries,
                other.observed_native_output_entries,
            ),
            native_u_entries: add(self.native_u_entries, other.native_u_entries),
            native_l_entries: add(self.native_l_entries, other.native_l_entries),
            returned_trace_entries: add(self.returned_trace_entries, other.returned_trace_entries),
            coefficient_algebra_work: add(
                self.coefficient_algebra_work,
                other.coefficient_algebra_work,
            ),
            coefficient_exponent_entry_work: add(
                self.coefficient_exponent_entry_work,
                other.coefficient_exponent_entry_work,
            ),
            coefficient_integer_bit_work: add(
                self.coefficient_integer_bit_work,
                other.coefficient_integer_bit_work,
            ),
        };
        (sum, saturated)
    }

    pub(crate) const fn rows(self) -> usize {
        self.rows
    }

    pub(crate) const fn physical_columns(self) -> usize {
        self.physical_columns
    }

    pub(crate) const fn input_entries(self) -> usize {
        self.input_entries
    }

    pub(crate) const fn prospective_native_output_entries(self) -> usize {
        self.prospective_native_output_entries
    }

    pub(crate) const fn observed_native_output_entries(self) -> usize {
        self.observed_native_output_entries
    }

    pub(crate) const fn native_u_entries(self) -> usize {
        self.native_u_entries
    }

    pub(crate) const fn native_l_entries(self) -> usize {
        self.native_l_entries
    }

    pub(crate) const fn returned_trace_entries(self) -> usize {
        self.returned_trace_entries
    }

    pub(crate) const fn coefficient_algebra_work(self) -> usize {
        self.coefficient_algebra_work
    }

    pub(crate) const fn coefficient_exponent_entry_work(self) -> usize {
        self.coefficient_exponent_entry_work
    }

    pub(crate) const fn coefficient_integer_bit_work(self) -> usize {
        self.coefficient_integer_bit_work
    }
}

/// Saturating, allocation-free scaling telemetry sealed into successor stages.
/// A live database contains only committed events; a staged row exposes the
/// aggregate that would result from committing it.
///
/// `componentwise_peak` is a census of independent maxima, not necessarily one
/// realizable event. In particular its observed-output maximum need not equal
/// the sum of its independently maximized U and L components.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactNativeSparseScalingStats {
    event_count: usize,
    cumulative_saturated: bool,
    last: GeneratedAffineResidualGroupExactNativeSparseStageStats,
    componentwise_peak: GeneratedAffineResidualGroupExactNativeSparseStageStats,
    cumulative: GeneratedAffineResidualGroupExactNativeSparseStageStats,
}

impl GeneratedAffineResidualGroupExactNativeSparseScalingStats {
    fn with_event(self, event: GeneratedAffineResidualGroupExactNativeSparseStageStats) -> Self {
        let (event_count, count_saturated) = match self.event_count.checked_add(1) {
            Some(value) => (value, false),
            None => (usize::MAX, true),
        };
        let (cumulative, cumulative_saturated) =
            self.cumulative.saturating_componentwise_add(event);
        Self {
            event_count,
            cumulative_saturated: self.cumulative_saturated
                || count_saturated
                || cumulative_saturated,
            last: event,
            componentwise_peak: self.componentwise_peak.componentwise_max(event),
            cumulative,
        }
    }

    pub(crate) const fn event_count(self) -> usize {
        self.event_count
    }

    pub(crate) const fn cumulative_saturated(self) -> bool {
        self.cumulative_saturated
    }

    pub(crate) const fn last(self) -> GeneratedAffineResidualGroupExactNativeSparseStageStats {
        self.last
    }

    pub(crate) const fn componentwise_peak(
        self,
    ) -> GeneratedAffineResidualGroupExactNativeSparseStageStats {
        self.componentwise_peak
    }

    pub(crate) const fn cumulative(
        self,
    ) -> GeneratedAffineResidualGroupExactNativeSparseStageStats {
        self.cumulative
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactDatabaseStats {
    retained_database_bytes: usize,
    last_ingress_prospective_retained_bytes: usize,
    last_ingress_observed_retained_bytes: usize,
    peak_ingress_retained_bytes: usize,
    last_candidate_prospective_retained_bytes: usize,
    last_candidate_observed_retained_bytes: usize,
    peak_candidate_retained_bytes: usize,
    last_staged_live_prospective_retained_bytes: usize,
    last_staged_live_observed_retained_bytes: usize,
    peak_staged_live_retained_bytes: usize,
    native_sparse_scaling: GeneratedAffineResidualGroupExactNativeSparseScalingStats,
}

impl GeneratedAffineResidualGroupExactDatabaseStats {
    pub(crate) const fn retained_database_bytes(self) -> usize {
        self.retained_database_bytes
    }

    pub(crate) const fn last_ingress_prospective_retained_bytes(self) -> usize {
        self.last_ingress_prospective_retained_bytes
    }

    pub(crate) const fn last_ingress_observed_retained_bytes(self) -> usize {
        self.last_ingress_observed_retained_bytes
    }

    pub(crate) const fn peak_ingress_retained_bytes(self) -> usize {
        self.peak_ingress_retained_bytes
    }

    pub(crate) const fn last_candidate_prospective_retained_bytes(self) -> usize {
        self.last_candidate_prospective_retained_bytes
    }

    pub(crate) const fn last_candidate_observed_retained_bytes(self) -> usize {
        self.last_candidate_observed_retained_bytes
    }

    pub(crate) const fn peak_candidate_retained_bytes(self) -> usize {
        self.peak_candidate_retained_bytes
    }

    pub(crate) const fn last_staged_live_prospective_retained_bytes(self) -> usize {
        self.last_staged_live_prospective_retained_bytes
    }

    pub(crate) const fn last_staged_live_observed_retained_bytes(self) -> usize {
        self.last_staged_live_observed_retained_bytes
    }

    pub(crate) const fn peak_staged_live_retained_bytes(self) -> usize {
        self.peak_staged_live_retained_bytes
    }

    pub(crate) const fn native_sparse_scaling(
        self,
    ) -> GeneratedAffineResidualGroupExactNativeSparseScalingStats {
        self.native_sparse_scaling
    }

    /// Compare the replay-relevant resource state while deliberately ignoring
    /// observational native scaling counters. Their values depend on the
    /// Symbolica implementation and diagnostic census definition, not only on
    /// the algebraic database state being replayed.
    pub(crate) fn replay_semantically_equal(mut self, mut other: Self) -> bool {
        self.native_sparse_scaling =
            GeneratedAffineResidualGroupExactNativeSparseScalingStats::default();
        other.native_sparse_scaling =
            GeneratedAffineResidualGroupExactNativeSparseScalingStats::default();
        self == other
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactDatabaseError {
    SourceProfileMismatch,
    WrongDatabaseAllocation,
    WrongTargetStateBinding,
    DatabaseIdentityExhaustion,
    TransitionIdentityExhaustion,
    WrongPlanAllocation,
    WrongFrameAllocation,
    WrongDatabaseEpoch,
    WrongGroup,
    PlanReplay,
    RowReplay,
    PhysicalKey,
    CoefficientWork,
    InvalidTermOrder,
    InvalidUnitPivot,
    InvalidStagedRow,
    DependentStagedRow,
    NewPivotStagedRow,
    StaleStagedRow,
    WrongSourceOrder,
    SourceOrderOverflow,
    StateVersionOverflow,
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
    },
    SymbolicaPanic,
    SymbolicaTranscriptMismatch,
}

impl GeneratedAffineResidualGroupExactDatabaseError {
    const fn kind(self) -> &'static str {
        match self {
            Self::SourceProfileMismatch => "SourceProfileMismatch",
            Self::WrongDatabaseAllocation => "WrongDatabaseAllocation",
            Self::WrongTargetStateBinding => "WrongTargetStateBinding",
            Self::DatabaseIdentityExhaustion => "DatabaseIdentityExhaustion",
            Self::TransitionIdentityExhaustion => "TransitionIdentityExhaustion",
            Self::WrongPlanAllocation => "WrongPlanAllocation",
            Self::WrongFrameAllocation => "WrongFrameAllocation",
            Self::WrongDatabaseEpoch => "WrongDatabaseEpoch",
            Self::WrongGroup => "WrongGroup",
            Self::PlanReplay => "PlanReplay",
            Self::RowReplay => "RowReplay",
            Self::PhysicalKey => "PhysicalKey",
            Self::CoefficientWork => "CoefficientWork",
            Self::InvalidTermOrder => "InvalidTermOrder",
            Self::InvalidUnitPivot => "InvalidUnitPivot",
            Self::InvalidStagedRow => "InvalidStagedRow",
            Self::DependentStagedRow => "DependentStagedRow",
            Self::NewPivotStagedRow => "NewPivotStagedRow",
            Self::StaleStagedRow => "StaleStagedRow",
            Self::WrongSourceOrder => "WrongSourceOrder",
            Self::SourceOrderOverflow => "SourceOrderOverflow",
            Self::StateVersionOverflow => "StateVersionOverflow",
            Self::ResourceLimit { .. } => "ResourceLimit",
            Self::ResourceCountOverflow { .. } => "ResourceCountOverflow",
            Self::AllocationFailure { .. } => "AllocationFailure",
            Self::SymbolicaPanic => "SymbolicaPanic",
            Self::SymbolicaTranscriptMismatch => "SymbolicaTranscriptMismatch",
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactDatabaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactDatabaseError")
            .field("kind", &self.kind())
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualGroupExactDatabaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "generated affine exact-group database {}",
            self.kind()
        )
    }
}

impl std::error::Error for GeneratedAffineResidualGroupExactDatabaseError {}

#[derive(Clone, PartialEq, Eq)]
struct ExactDatabaseTerm {
    key: GeneratedAffineResidualGroupPhysicalKey,
    coefficient: ParametricCoefficient,
}

/// Allocation-free deep-payload census for one borrowed ingress row.
///
/// The scalar payload is combined first with logical vector lengths and then,
/// immediately after fallible reservation, with the actual retained
/// capacities. Physical-key payload is charged conservatively even though the
/// ingress clone shares its `Arc`s with the authenticated source row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct BorrowedIngressRetainedCensus {
    terms: usize,
    guards: usize,
    deep_payload_bytes: usize,
    prospective_retained_bytes: usize,
}

impl BorrowedIngressRetainedCensus {
    fn observed_retained_bytes(
        self,
        term_capacity: usize,
        guard_capacity: usize,
    ) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
        if term_capacity < self.terms || guard_capacity < self.guards {
            return Err(
                GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
                    resource: "exact-group borrowed ingress buffers",
                },
            );
        }
        ingress_retained_bytes(term_capacity, guard_capacity, self.deep_payload_bytes)
    }
}

impl fmt::Debug for ExactDatabaseTerm {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactDatabaseTerm")
            .field("private_key", &"<redacted>")
            .field("private_coefficient", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactReductionStep {
    pivot_ordinal: usize,
    factor: ParametricCoefficient,
}

impl GeneratedAffineResidualGroupExactReductionStep {
    pub(crate) const fn pivot_ordinal(&self) -> usize {
        self.pivot_ordinal
    }

    pub(crate) const fn factor(&self) -> &ParametricCoefficient {
        &self.factor
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactReductionStep {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactReductionStep")
            .field("pivot_ordinal", &self.pivot_ordinal)
            .field("private_factor", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
struct ExactUnitPivot {
    ordinal: usize,
    source_ordinal: usize,
    terms: Vec<ExactDatabaseTerm>,
    guards: Vec<ParametricNonZeroCondition>,
    reductions: Arc<Vec<GeneratedAffineResidualGroupExactReductionStep>>,
    normalization_divisor: ParametricCoefficient,
}

/// Symbolica's owned algebra transcript plus normalized-key validation clones.
/// An independent stage also owns the complete successor catalog that may move
/// into the live database; a no-growth stage continues to borrow the live
/// catalog and therefore carries no successor allocation.
struct ExactDatabaseSymbolicaTranscript {
    outcome: SymbolicaPersistentSparseOutcome,
    normalized_keys_hardest_first: Vec<GeneratedAffineResidualGroupPhysicalKey>,
    successor_catalog_easiest_first: Option<Vec<GeneratedAffineResidualGroupPhysicalKey>>,
}

impl fmt::Debug for ExactUnitPivot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactUnitPivot")
            .field("ordinal", &self.ordinal)
            .field("source_ordinal", &self.source_ordinal)
            .field("term_count", &self.terms.len())
            .field("guard_count", &self.guards.len())
            .field("reduction_count", &self.reductions.len())
            .field("private_normalization_divisor", &"<redacted>")
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
struct ExactPivotLookupEntry {
    key: GeneratedAffineResidualGroupPhysicalKey,
    pivot_ordinal: usize,
}

/// Borrowed, read-only view of one chronologically retained algebraic pivot.
pub(crate) struct GeneratedAffineResidualGroupExactUnitPivotView<'a> {
    pivot: &'a ExactUnitPivot,
}

impl<'a> GeneratedAffineResidualGroupExactUnitPivotView<'a> {
    pub(crate) fn ordinal(&self) -> usize {
        self.pivot.ordinal
    }

    pub(crate) fn source_ordinal(&self) -> usize {
        self.pivot.source_ordinal
    }

    pub(crate) fn key(&self) -> &'a GeneratedAffineResidualGroupPhysicalKey {
        &self
            .pivot
            .terms
            .last()
            .expect("an authenticated unit pivot is nonempty")
            .key
    }

    pub(crate) fn terms(
        &self,
    ) -> impl ExactSizeIterator<
        Item = (
            &'a GeneratedAffineResidualGroupPhysicalKey,
            &'a ParametricCoefficient,
        ),
    > + DoubleEndedIterator
    + 'a {
        self.pivot
            .terms
            .iter()
            .map(|term| (&term.key, &term.coefficient))
    }

    pub(crate) fn guards(&self) -> &'a [ParametricNonZeroCondition] {
        &self.pivot.guards
    }

    pub(crate) fn reductions(&self) -> &'a [GeneratedAffineResidualGroupExactReductionStep] {
        self.pivot.reductions.as_slice()
    }

    /// Exact pre-normalization leader retained for future event replay.
    pub(crate) fn normalization_divisor(&self) -> &'a ParametricCoefficient {
        &self.pivot.normalization_divisor
    }
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactRowOutcome {
    Dependent {
        source_ordinal: usize,
        reductions: Vec<GeneratedAffineResidualGroupExactReductionStep>,
    },
    NewPivot {
        source_ordinal: usize,
        pivot_ordinal: usize,
    },
}

#[derive(Clone)]
enum ExactStagedSource {
    Production {
        source: Arc<GeneratedAffineResidualGroupExactPhysicalRow>,
        allocation: Weak<GeneratedAffineResidualGroupExactPhysicalRow>,
    },
    #[cfg(test)]
    Synthetic {
        source: Arc<ExactSyntheticSourceRecipe>,
        allocation: Weak<ExactSyntheticSourceRecipe>,
    },
}

#[cfg(test)]
struct ExactSyntheticSourceRecipe {
    terms: Vec<(
        GeneratedAffineResidualGroupPhysicalKey,
        ParametricCoefficient,
    )>,
    guards: Vec<ParametricNonZeroCondition>,
    retained_bytes: usize,
}

#[cfg(test)]
impl fmt::Debug for ExactSyntheticSourceRecipe {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactSyntheticSourceRecipe")
            .field("term_count", &self.terms.len())
            .field("guard_count", &self.guards.len())
            .field("retained_bytes", &self.retained_bytes)
            .field("private_terms", &"<redacted>")
            .field("private_guards", &"<redacted>")
            .finish()
    }
}

impl ExactStagedSource {
    fn production(source: &Arc<GeneratedAffineResidualGroupExactPhysicalRow>) -> Self {
        Self::Production {
            source: Arc::clone(source),
            allocation: Arc::downgrade(source),
        }
    }

    #[cfg(test)]
    fn synthetic(source: Arc<ExactSyntheticSourceRecipe>) -> Self {
        let allocation = Arc::downgrade(&source);
        Self::Synthetic { source, allocation }
    }

    fn authenticates_own_allocation(&self) -> bool {
        match self {
            Self::Production { source, allocation } => {
                Weak::ptr_eq(allocation, &Arc::downgrade(source))
            }
            #[cfg(test)]
            Self::Synthetic { source, allocation } => {
                Weak::ptr_eq(allocation, &Arc::downgrade(source))
            }
        }
    }

    fn same_allocation(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Production { source: left, .. }, Self::Production { source: right, .. }) => {
                Arc::ptr_eq(left, right)
            }
            #[cfg(test)]
            (Self::Synthetic { source: left, .. }, Self::Synthetic { source: right, .. }) => {
                Arc::ptr_eq(left, right)
            }
            #[cfg(test)]
            _ => false,
        }
    }

    fn unique_retained_bytes(
        &self,
    ) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
        match self {
            // The stage/recipe may become the sole owner of the complete
            // frozen source pipeline after every caller handle is released.
            // The physical-row authority performs the descendant census and
            // exact pointer deduplication against its shared plan/frame
            // ancestry. The enclosing session ledger separately deduplicates
            // repeated handles to this same authenticated row allocation.
            Self::Production { source, .. } => {
                source.unique_retained_source_graph_byte_bound().ok_or(
                    GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                        resource: "exact retained production source graph bytes",
                    },
                )
            }
            #[cfg(test)]
            Self::Synthetic { source, .. } => Ok(source.retained_bytes),
        }
    }
}

impl fmt::Debug for ExactStagedSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Production { .. } => "Production(<redacted>)",
            #[cfg(test)]
            Self::Synthetic { .. } => "Synthetic(<redacted>)",
        })
    }
}

/// Opaque allocation-bound recipe for replaying the exact raw source row that
/// produced one authenticated database stage.
///
/// Only the database can mint this owner, and only after the staged token has
/// authenticated against the live database. The retained plan/frame/source
/// allocations are intentionally private; callers may replay the recipe only
/// through the capability-gated database ingress below.
pub(crate) struct GeneratedAffineResidualGroupRetainedExactSourceRecipe {
    plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
    frame: Arc<GeneratedAffineResidualGroupPhysicalFrame>,
    database_epoch: usize,
    group_ordinal: usize,
    source: ExactStagedSource,
}

impl GeneratedAffineResidualGroupRetainedExactSourceRecipe {
    fn retained_copy(&self) -> Self {
        Self {
            plan: Arc::clone(&self.plan),
            frame: Arc::clone(&self.frame),
            database_epoch: self.database_epoch,
            group_ordinal: self.group_ordinal,
            source: self.source.clone(),
        }
    }

    pub(crate) const fn database_epoch(&self) -> usize {
        self.database_epoch
    }

    pub(crate) const fn group_ordinal(&self) -> usize {
        self.group_ordinal
    }

    pub(crate) const fn has_production_source(&self) -> bool {
        matches!(self.source, ExactStagedSource::Production { .. })
    }

    pub(crate) fn same_source_allocation(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.plan, &other.plan)
            && Arc::ptr_eq(&self.frame, &other.frame)
            && self.database_epoch == other.database_epoch
            && self.group_ordinal == other.group_ordinal
            && self.source.same_allocation(&other.source)
    }

    pub(crate) fn retained_byte_bound(
        &self,
    ) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
        checked_add(
            "exact retained source-recipe bytes",
            size_of::<Self>(),
            self.source.unique_retained_bytes()?,
        )
    }

    pub(crate) fn authenticates_production_source_allocation(
        &self,
        source: &Arc<GeneratedAffineResidualGroupExactPhysicalRow>,
    ) -> bool {
        match &self.source {
            ExactStagedSource::Production {
                source: retained, ..
            } => Arc::ptr_eq(retained, source),
            #[cfg(test)]
            ExactStagedSource::Synthetic { .. } => false,
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupRetainedExactSourceRecipe {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupRetainedExactSourceRecipe")
            .field("database_epoch", &self.database_epoch)
            .field("group_ordinal", &self.group_ordinal)
            .field("has_production_source", &self.has_production_source())
            .field("private_plan", &"<redacted>")
            .field("private_frame", &"<redacted>")
            .field("private_source", &"<redacted>")
            .finish()
    }
}

/// Immutable shared owner of one authenticated dependent reduction trace.
/// The allocation is exactly the allocation carried by the staged row.
pub(crate) struct GeneratedAffineResidualGroupRetainedExactDependentReductions {
    reductions: Arc<Vec<GeneratedAffineResidualGroupExactReductionStep>>,
}

impl GeneratedAffineResidualGroupRetainedExactDependentReductions {
    fn retained_copy(&self) -> Self {
        Self {
            reductions: Arc::clone(&self.reductions),
        }
    }

    pub(crate) fn reductions(&self) -> &[GeneratedAffineResidualGroupExactReductionStep] {
        self.reductions.as_slice()
    }

    pub(crate) fn same_allocation(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.reductions, &other.reductions)
    }

    pub(crate) fn structurally_equal(&self, other: &Self) -> bool {
        self.reductions == other.reductions
    }

    pub(crate) fn retained_byte_bound(
        &self,
    ) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
        shared_reduction_trace_retained_bytes(&self.reductions)
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupRetainedExactDependentReductions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupRetainedExactDependentReductions")
            .field("reduction_count", &self.reductions.len())
            .field("private_reductions", &"<redacted>")
            .finish()
    }
}

/// Immutable shared owner of one exact unit pivot. A committed new-pivot
/// transition installs this same allocation in the chronological database.
pub(crate) struct GeneratedAffineResidualGroupRetainedExactUnitPivot {
    pivot: Arc<ExactUnitPivot>,
}

impl GeneratedAffineResidualGroupRetainedExactUnitPivot {
    fn retained_copy(&self) -> Self {
        Self {
            pivot: Arc::clone(&self.pivot),
        }
    }

    pub(crate) fn ordinal(&self) -> usize {
        self.pivot.ordinal
    }

    pub(crate) fn source_ordinal(&self) -> usize {
        self.pivot.source_ordinal
    }

    pub(crate) fn key(&self) -> &GeneratedAffineResidualGroupPhysicalKey {
        &self
            .pivot
            .terms
            .last()
            .expect("an authenticated retained exact pivot is nonempty")
            .key
    }

    pub(crate) fn terms(
        &self,
    ) -> impl ExactSizeIterator<
        Item = (
            &GeneratedAffineResidualGroupPhysicalKey,
            &ParametricCoefficient,
        ),
    > + DoubleEndedIterator {
        self.pivot
            .terms
            .iter()
            .map(|term| (&term.key, &term.coefficient))
    }

    pub(crate) fn guards(&self) -> &[ParametricNonZeroCondition] {
        &self.pivot.guards
    }

    pub(crate) fn reductions(&self) -> &[GeneratedAffineResidualGroupExactReductionStep] {
        self.pivot.reductions.as_slice()
    }

    pub(crate) fn normalization_divisor(&self) -> &ParametricCoefficient {
        &self.pivot.normalization_divisor
    }

    pub(crate) fn same_allocation(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.pivot, &other.pivot)
    }

    pub(crate) fn structurally_equal(&self, other: &Self) -> bool {
        self.pivot == other.pivot
    }

    pub(crate) fn retained_byte_bound(
        &self,
    ) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
        exact_unit_pivot_owner_retained_bytes(&self.pivot)
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupRetainedExactUnitPivot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupRetainedExactUnitPivot")
            .field("ordinal", &self.ordinal())
            .field("source_ordinal", &self.source_ordinal())
            .field("term_count", &self.pivot.terms.len())
            .field("guard_count", &self.pivot.guards.len())
            .field("reduction_count", &self.pivot.reductions.len())
            .field("private_pivot", &"<redacted>")
            .finish()
    }
}

enum ExactStagedRowPayload {
    Dependent {
        reductions: Arc<Vec<GeneratedAffineResidualGroupExactReductionStep>>,
        committed_stats: GeneratedAffineResidualGroupExactDatabaseStats,
    },
    NewPivot {
        pivot: Arc<ExactUnitPivot>,
        pivot_key: GeneratedAffineResidualGroupPhysicalKey,
        lookup_insertion: usize,
        successor_reducer: SymbolicaPersistentSparseReducer,
        successor_catalog_easiest_first: Option<Vec<GeneratedAffineResidualGroupPhysicalKey>>,
        committed_pivots: Vec<Arc<ExactUnitPivot>>,
        committed_lookup: Vec<ExactPivotLookupEntry>,
        committed_stats: GeneratedAffineResidualGroupExactDatabaseStats,
    },
}

/// Sealed, consume-once result of exact hardest-only row reduction.
///
/// This value is intentionally non-`Clone`. Dropping it commits nothing. An
/// exact session recenterer may request a database-authenticated staged-pivot
/// authority view, but only this module can construct the token or consume it
/// into database state.
pub(crate) struct GeneratedAffineResidualGroupStagedExactRow {
    database_nonce: u64,
    next_transition_identity: ExactDatabaseTransitionIdentity,
    database_epoch: usize,
    group_ordinal: usize,
    state_version: usize,
    next_state_version: usize,
    source_ordinal: usize,
    next_source_ordinal: usize,
    pivot_count: usize,
    lookup_len: usize,
    catalog_len: usize,
    source: ExactStagedSource,
    payload: ExactStagedRowPayload,
}

impl GeneratedAffineResidualGroupStagedExactRow {
    pub(crate) fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }

    pub(crate) fn reductions(&self) -> &[GeneratedAffineResidualGroupExactReductionStep] {
        match &self.payload {
            ExactStagedRowPayload::Dependent { reductions, .. } => reductions.as_slice(),
            ExactStagedRowPayload::NewPivot { pivot, .. } => pivot.reductions.as_slice(),
        }
    }

    pub(crate) const fn staged_live_prospective_retained_bytes(&self) -> usize {
        match &self.payload {
            ExactStagedRowPayload::Dependent {
                committed_stats, ..
            }
            | ExactStagedRowPayload::NewPivot {
                committed_stats, ..
            } => committed_stats.last_staged_live_prospective_retained_bytes,
        }
    }

    pub(crate) const fn staged_live_observed_retained_bytes(&self) -> usize {
        match &self.payload {
            ExactStagedRowPayload::Dependent {
                committed_stats, ..
            }
            | ExactStagedRowPayload::NewPivot {
                committed_stats, ..
            } => committed_stats.last_staged_live_observed_retained_bytes,
        }
    }

    /// Successor telemetry sealed into this stage. Dropping the stage leaves
    /// the live database's aggregate unchanged. These metrics cover native
    /// stage shape/fill and native coefficient work, not catalog sorting,
    /// key-comparison work, Rust metadata allocation, wall time, or RSS.
    pub(crate) const fn native_sparse_scaling_stats(
        &self,
    ) -> GeneratedAffineResidualGroupExactNativeSparseScalingStats {
        match &self.payload {
            ExactStagedRowPayload::Dependent {
                committed_stats, ..
            }
            | ExactStagedRowPayload::NewPivot {
                committed_stats, ..
            } => committed_stats.native_sparse_scaling,
        }
    }

    /// Retained authenticated raw-row recipe. Synthetic rows exist only in
    /// this module's tests and return `None`.
    pub(crate) fn production_source(
        &self,
    ) -> Option<&Arc<GeneratedAffineResidualGroupExactPhysicalRow>> {
        match &self.source {
            ExactStagedSource::Production { source, .. } => Some(source),
            #[cfg(test)]
            ExactStagedSource::Synthetic { .. } => None,
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupStagedExactRow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupStagedExactRow")
            .field("database_epoch", &self.database_epoch)
            .field("group_ordinal", &self.group_ordinal)
            .field("state_version", &self.state_version)
            .field("source_ordinal", &self.source_ordinal)
            .field("pivot_count", &self.pivot_count)
            .field("reduction_count", &self.reductions().len())
            .field(
                "is_new_pivot",
                &matches!(&self.payload, ExactStagedRowPayload::NewPivot { .. }),
            )
            .field("private_database_nonce", &"<redacted>")
            .field("private_next_transition_identity", &"<redacted>")
            .field("private_source", &"<redacted>")
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

enum PreparedExactRowEvidence {
    Dependent(GeneratedAffineResidualGroupRetainedExactDependentReductions),
    NewPivot(GeneratedAffineResidualGroupRetainedExactUnitPivot),
}

/// Owning proof that fallible database preparation for one staged transition
/// has completed. It is non-Clone and carries the exact staged token plus the
/// predecessor transition identity into an infallible move-only commit tail.
/// The tail performs one allocation-free fail-stop invariant assertion against
/// the live database immediately before its first mutation.
pub(crate) struct GeneratedAffineResidualGroupPreparedExactRowCommit {
    predecessor_transition_identity: ExactDatabaseTransitionIdentity,
    staged: GeneratedAffineResidualGroupStagedExactRow,
    source_recipe: GeneratedAffineResidualGroupRetainedExactSourceRecipe,
    evidence: PreparedExactRowEvidence,
}

impl GeneratedAffineResidualGroupPreparedExactRowCommit {
    pub(crate) const fn source_ordinal(&self) -> usize {
        self.staged.source_ordinal
    }

    pub(crate) fn retain_source_recipe_for_session(
        &self,
        _capability: &GeneratedAffineResidualGroupExactSessionDatabaseCapability,
    ) -> GeneratedAffineResidualGroupRetainedExactSourceRecipe {
        self.source_recipe.retained_copy()
    }

    pub(crate) fn retain_dependent_evidence_for_session(
        &self,
        _capability: &GeneratedAffineResidualGroupExactSessionDatabaseCapability,
    ) -> Option<GeneratedAffineResidualGroupRetainedExactDependentReductions> {
        match &self.evidence {
            PreparedExactRowEvidence::Dependent(evidence) => Some(evidence.retained_copy()),
            PreparedExactRowEvidence::NewPivot(_) => None,
        }
    }

    pub(crate) fn retain_new_pivot_evidence_for_session(
        &self,
        _capability: &GeneratedAffineResidualGroupExactSessionDatabaseCapability,
    ) -> Option<GeneratedAffineResidualGroupRetainedExactUnitPivot> {
        match &self.evidence {
            PreparedExactRowEvidence::Dependent(_) => None,
            PreparedExactRowEvidence::NewPivot(evidence) => Some(evidence.retained_copy()),
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupPreparedExactRowCommit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupPreparedExactRowCommit")
            .field("source_ordinal", &self.source_ordinal())
            .field(
                "kind",
                &match self.evidence {
                    PreparedExactRowEvidence::Dependent(_) => "Dependent",
                    PreparedExactRowEvidence::NewPivot(_) => "NewPivot",
                },
            )
            .field("private_predecessor_transition_identity", &"<redacted>")
            .field("private_staged", &"<redacted>")
            .field("private_source_recipe", &"<redacted>")
            .field("private_evidence", &"<redacted>")
            .finish()
    }
}

/// Preparation failure that returns the exact consume-once staged token.
pub(crate) struct GeneratedAffineResidualGroupPrepareExactRowCommitFailure {
    error: GeneratedAffineResidualGroupExactDatabaseError,
    staged: GeneratedAffineResidualGroupStagedExactRow,
}

impl GeneratedAffineResidualGroupPrepareExactRowCommitFailure {
    pub(crate) const fn error(&self) -> GeneratedAffineResidualGroupExactDatabaseError {
        self.error
    }

    pub(crate) fn into_staged(self) -> GeneratedAffineResidualGroupStagedExactRow {
        self.staged
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupPrepareExactRowCommitFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupPrepareExactRowCommitFailure")
            .field("error", &self.error)
            .field("private_staged", &"<redacted>")
            .finish()
    }
}

/// Shared evidence returned by the infallible prepared database commit.
pub(crate) enum GeneratedAffineResidualGroupPreparedExactRowOutcome {
    Dependent {
        source_ordinal: usize,
        evidence: GeneratedAffineResidualGroupRetainedExactDependentReductions,
    },
    NewPivot {
        source_ordinal: usize,
        pivot_ordinal: usize,
        evidence: GeneratedAffineResidualGroupRetainedExactUnitPivot,
    },
}

impl GeneratedAffineResidualGroupPreparedExactRowOutcome {
    pub(crate) const fn source_ordinal(&self) -> usize {
        match self {
            Self::Dependent { source_ordinal, .. } | Self::NewPivot { source_ordinal, .. } => {
                *source_ordinal
            }
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupPreparedExactRowOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dependent {
                source_ordinal,
                evidence,
            } => formatter
                .debug_struct("Dependent")
                .field("source_ordinal", source_ordinal)
                .field("reduction_count", &evidence.reductions().len())
                .field("private_evidence", &"<redacted>")
                .finish(),
            Self::NewPivot {
                source_ordinal,
                pivot_ordinal,
                ..
            } => formatter
                .debug_struct("NewPivot")
                .field("source_ordinal", source_ordinal)
                .field("pivot_ordinal", pivot_ordinal)
                .field("private_evidence", &"<redacted>")
                .finish(),
        }
    }
}

/// Observable live resource components for one exact database allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactDatabaseResidentResourceSnapshot {
    physical_columns: usize,
    independent_rows: usize,
    native_u_rows: usize,
    native_u_columns: usize,
    native_l_rows: usize,
    native_l_columns: usize,
    native_u_stored_entries: usize,
    native_l_stored_entries: usize,
    native_shallow_capacity_slots: SymbolicaPersistentSparseShallowCapacitySnapshot,
    retained_database_bytes: usize,
}

impl GeneratedAffineResidualGroupExactDatabaseResidentResourceSnapshot {
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

    pub(crate) const fn retained_database_bytes(self) -> usize {
        self.retained_database_bytes
    }
}

/// Persistent algebraic database for one exact solve-plan allocation.
pub(crate) struct GeneratedAffineResidualGroupExactDatabase {
    schema: &'static str,
    source_kind: GeneratedAffineResidualCaseAuthoritySourceKind,
    database_nonce: u64,
    transition_identity: ExactDatabaseTransitionIdentity,
    plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
    frame: Arc<GeneratedAffineResidualGroupPhysicalFrame>,
    database_epoch: usize,
    group_ordinal: usize,
    state_version: usize,
    next_source_ordinal: usize,
    pivots: Vec<Arc<ExactUnitPivot>>,
    lookup: Vec<ExactPivotLookupEntry>,
    symbolica_reducer: SymbolicaPersistentSparseReducer,
    symbolica_catalog_easiest_first: Vec<GeneratedAffineResidualGroupPhysicalKey>,
    limits: GeneratedAffineResidualGroupExactDatabaseLimits,
    stats: GeneratedAffineResidualGroupExactDatabaseStats,
}

/// Opaque allocation/transition authority for the exact target state paired
/// with this database.
///
/// Callers can move this value into the target-state owner, but cannot forge
/// one from public epoch/group/version scalars: the database nonce and parent
/// allocations and exact transition identity remain private to this module.
/// The successor form is minted only after the corresponding staged row has
/// passed every database check; competing rows at the same numeric version
/// carry distinct transition identities.
/// The joint session wrapper presents this binding back to the same database
/// before it may pair a target with a staged pivot.
pub(crate) struct GeneratedAffineResidualGroupExactTargetStateBinding {
    database_nonce: u64,
    transition_identity: ExactDatabaseTransitionIdentity,
    predecessor_transition_identity: Option<ExactDatabaseTransitionIdentity>,
    plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
    frame: Arc<GeneratedAffineResidualGroupPhysicalFrame>,
    database_epoch: usize,
    group_ordinal: usize,
    state_version: usize,
}

impl GeneratedAffineResidualGroupExactTargetStateBinding {
    /// Compare the hidden database allocation identity while keeping its nonce
    /// private.  Target-state successor preparation uses this to reject a
    /// transition minted by a sibling database with otherwise identical
    /// visible coordinates.
    pub(crate) fn same_database_allocation(&self, other: &Self) -> bool {
        self.database_nonce == other.database_nonce
            && Arc::ptr_eq(&self.plan, &other.plan)
            && Arc::ptr_eq(&self.frame, &other.frame)
            && self.database_epoch == other.database_epoch
            && self.group_ordinal == other.group_ordinal
    }

    /// Verify sealed direct ancestry rather than merely comparing adjacent
    /// numeric versions. Competing staged rows from the same live database
    /// state receive distinct transition identities, so a successor minted
    /// from one sibling cannot advance a target state retained from another.
    pub(crate) fn is_direct_successor_of(&self, predecessor: &Self) -> bool {
        self.same_database_allocation(predecessor)
            && predecessor.state_version.checked_add(1) == Some(self.state_version)
            && self.predecessor_transition_identity == Some(predecessor.transition_identity)
    }

    pub(crate) fn same_plan_allocation(
        &self,
        plan: &Arc<GeneratedAffineResidualGroupSolvePlan>,
    ) -> bool {
        Arc::ptr_eq(&self.plan, plan)
    }

    pub(crate) fn same_frame_allocation(
        &self,
        frame: &Arc<GeneratedAffineResidualGroupPhysicalFrame>,
    ) -> bool {
        Arc::ptr_eq(&self.frame, frame)
    }

    pub(crate) const fn database_epoch(&self) -> usize {
        self.database_epoch
    }

    pub(crate) const fn group_ordinal(&self) -> usize {
        self.group_ordinal
    }

    pub(crate) const fn state_version(&self) -> usize {
        self.state_version
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactTargetStateBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactTargetStateBinding")
            .field("database_epoch", &self.database_epoch)
            .field("group_ordinal", &self.group_ordinal)
            .field("state_version", &self.state_version)
            .field("private_database_nonce", &"<redacted>")
            .field("private_transition_identity", &"<redacted>")
            .field("private_predecessor_transition_identity", &"<redacted>")
            .field("private_plan", &"<redacted>")
            .field("private_frame", &"<redacted>")
            .finish()
    }
}

/// Sealed authority for one staged new pivot authenticated against one live
/// exact database state.
///
/// The simultaneous borrows keep the database allocation/state and the exact
/// staged token inseparable for the lifetime of this view. Its private
/// database reference retains the nonce binding without exposing the nonce;
/// the remaining accessors expose only the plan/frame allocations, public
/// transaction coordinates, retained source recipe, and read-only unit-pivot
/// payload needed by exact recentering.
pub(crate) struct GeneratedAffineResidualGroupAuthenticatedStagedNewPivotView<'a> {
    database: &'a GeneratedAffineResidualGroupExactDatabase,
    staged: &'a GeneratedAffineResidualGroupStagedExactRow,
    pivot: &'a ExactUnitPivot,
}

impl<'a> GeneratedAffineResidualGroupAuthenticatedStagedNewPivotView<'a> {
    /// Raw plan access is restricted to the allocation-sealed session. The
    /// session immediately narrows this owner to authenticated geometry and
    /// immutable locator slices before returning its own joint view.
    pub(crate) fn plan_for_session(
        &self,
        _capability: &GeneratedAffineResidualGroupExactSessionDatabaseCapability,
    ) -> &'a Arc<GeneratedAffineResidualGroupSolvePlan> {
        &self.database.plan
    }

    /// Explicit unit-test adapter for database-local authority assertions.
    #[cfg(test)]
    fn plan_for_test(&self) -> &'a Arc<GeneratedAffineResidualGroupSolvePlan> {
        &self.database.plan
    }

    pub(crate) fn frame(&self) -> &'a Arc<GeneratedAffineResidualGroupPhysicalFrame> {
        &self.database.frame
    }

    pub(crate) const fn database_epoch(&self) -> usize {
        self.database.database_epoch
    }

    pub(crate) const fn group_ordinal(&self) -> usize {
        self.database.group_ordinal
    }

    pub(crate) const fn state_version(&self) -> usize {
        self.database.state_version
    }

    pub(crate) const fn source_ordinal(&self) -> usize {
        self.pivot.source_ordinal
    }

    pub(crate) const fn pivot_ordinal(&self) -> usize {
        self.pivot.ordinal
    }

    pub(crate) fn production_source(
        &self,
    ) -> Option<&'a Arc<GeneratedAffineResidualGroupExactPhysicalRow>> {
        match &self.staged.source {
            ExactStagedSource::Production { source, .. } => Some(source),
            #[cfg(test)]
            ExactStagedSource::Synthetic { .. } => None,
        }
    }

    pub(crate) fn key(&self) -> &'a GeneratedAffineResidualGroupPhysicalKey {
        &self
            .pivot
            .terms
            .last()
            .expect("an authenticated staged unit pivot is nonempty")
            .key
    }

    pub(crate) fn terms(
        &self,
    ) -> impl ExactSizeIterator<
        Item = (
            &'a GeneratedAffineResidualGroupPhysicalKey,
            &'a ParametricCoefficient,
        ),
    > + DoubleEndedIterator
    + 'a {
        self.pivot
            .terms
            .iter()
            .map(|term| (&term.key, &term.coefficient))
    }

    pub(crate) fn guards(&self) -> &'a [ParametricNonZeroCondition] {
        &self.pivot.guards
    }

    pub(crate) fn reductions(&self) -> &'a [GeneratedAffineResidualGroupExactReductionStep] {
        self.pivot.reductions.as_slice()
    }

    pub(crate) const fn normalization_divisor(&self) -> &'a ParametricCoefficient {
        &self.pivot.normalization_divisor
    }

    pub(crate) fn successor_target_state_binding_for_session(
        &self,
        _capability: &GeneratedAffineResidualGroupExactSessionDatabaseCapability,
    ) -> GeneratedAffineResidualGroupExactTargetStateBinding {
        self.database.target_state_binding_at(
            self.staged.next_state_version,
            self.staged.next_transition_identity,
            Some(self.database.transition_identity),
        )
    }

    /// Retained coexistence envelope admitted before this sealed stage was
    /// returned.  Recenter accounting receives only the scalar, never the
    /// staged token or database statistics owner from which it was derived.
    pub(crate) const fn staged_live_prospective_retained_bytes(&self) -> usize {
        self.staged.staged_live_prospective_retained_bytes()
    }

    /// Allocator-observed counterpart of
    /// [`Self::staged_live_prospective_retained_bytes`].
    pub(crate) const fn staged_live_observed_retained_bytes(&self) -> usize {
        self.staged.staged_live_observed_retained_bytes()
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupAuthenticatedStagedNewPivotView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupAuthenticatedStagedNewPivotView")
            .field("database_epoch", &self.database_epoch())
            .field("group_ordinal", &self.group_ordinal())
            .field("state_version", &self.state_version())
            .field("source_ordinal", &self.source_ordinal())
            .field("pivot_ordinal", &self.pivot_ordinal())
            .field("term_count", &self.pivot.terms.len())
            .field("guard_count", &self.pivot.guards.len())
            .field("reduction_count", &self.pivot.reductions.len())
            .field("has_production_source", &self.production_source().is_some())
            .field("private_database_nonce", &"<redacted>")
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

/// Sealed proof that one consume-once stage is both live for this database
/// allocation and algebraically dependent on its retained pivots.
///
/// The proof is borrowed so it cannot outlive either authenticated input, and
/// it exposes no database nonce, transition identity, staged token, or
/// payload owner.  The session layer turns this proof into an owning,
/// non-`Clone` dependent classification before any crate caller may commit.
pub(crate) struct GeneratedAffineResidualGroupAuthenticatedStagedDependentView<'a> {
    database: &'a GeneratedAffineResidualGroupExactDatabase,
    staged: &'a GeneratedAffineResidualGroupStagedExactRow,
    reductions: &'a Arc<Vec<GeneratedAffineResidualGroupExactReductionStep>>,
}

impl<'a> GeneratedAffineResidualGroupAuthenticatedStagedDependentView<'a> {
    pub(crate) const fn source_ordinal(&self) -> usize {
        self.staged.source_ordinal
    }

    pub(crate) fn reductions(&self) -> &'a [GeneratedAffineResidualGroupExactReductionStep] {
        self.reductions.as_slice()
    }

    pub(crate) fn retain_exact_reduction_evidence_for_session(
        &self,
        _capability: &GeneratedAffineResidualGroupExactSessionDatabaseCapability,
    ) -> GeneratedAffineResidualGroupRetainedExactDependentReductions {
        GeneratedAffineResidualGroupRetainedExactDependentReductions {
            reductions: Arc::clone(self.reductions),
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupAuthenticatedStagedDependentView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupAuthenticatedStagedDependentView")
            .field("database_epoch", &self.database.database_epoch)
            .field("state_version", &self.database.state_version)
            .field("source_ordinal", &self.source_ordinal())
            .field("reduction_count", &self.reductions.len())
            .field("private_database_authority", &"<redacted>")
            .field("private_staged_token", &"<redacted>")
            .finish()
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactDatabase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactDatabase")
            .field("schema", &self.schema)
            .field("database_epoch", &self.database_epoch)
            .field("group_ordinal", &self.group_ordinal)
            .field("state_version", &self.state_version)
            .field("next_source_ordinal", &self.next_source_ordinal)
            .field("pivot_count", &self.pivots.len())
            .field("stats", &self.stats)
            .field("private_transition_identity", &"<redacted>")
            .field("private_plan", &"<redacted>")
            .field("private_frame", &"<redacted>")
            .field("private_payload", &"<redacted>")
            .field("publishes_rule", &false)
            .field("infers_master", &false)
            .finish()
    }
}

impl GeneratedAffineResidualGroupExactDatabase {
    fn authenticate_source_profile(
        &self,
    ) -> Result<(), GeneratedAffineResidualGroupExactDatabaseError> {
        if self.schema != exact_database_schema_for_source(self.source_kind)
            || self.source_kind != self.plan.source_kind()
            || self.group_ordinal != self.plan.group_ordinal()
            || !Arc::ptr_eq(self.plan.physical_frame(), &self.frame)
        {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::SourceProfileMismatch);
        }
        Ok(())
    }

    pub(crate) fn try_new(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
        frame: Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        database_epoch: usize,
        limits: GeneratedAffineResidualGroupExactDatabaseLimits,
    ) -> Result<Self, GeneratedAffineResidualGroupExactDatabaseError> {
        catch_unwind(AssertUnwindSafe(|| {
            check_limit(
                "exact-group database retained bytes",
                size_of::<Self>(),
                limits.max_database_retained_bytes,
            )?;
            if !Arc::ptr_eq(plan.physical_frame(), &frame) {
                return Err(GeneratedAffineResidualGroupExactDatabaseError::WrongFrameAllocation);
            }
            plan.replay_retained_source(family, context, limits.solve_plan_replay)
                .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::PlanReplay)?;
            let symbolica_reducer = SymbolicaPersistentSparseReducer::try_new(
                Arc::new(context.clone()),
                0,
                limits.symbolica_sparse,
            )
            .map_err(map_symbolica_sparse_error)?;
            let database_nonce = next_exact_database_nonce()?;
            let source_kind = plan.source_kind();
            Ok(Self {
                schema: exact_database_schema_for_source(source_kind),
                source_kind,
                database_nonce,
                transition_identity: ExactDatabaseTransitionIdentity::PRISTINE,
                group_ordinal: plan.group_ordinal(),
                plan,
                frame,
                database_epoch,
                state_version: 0,
                next_source_ordinal: 0,
                pivots: Vec::new(),
                lookup: Vec::new(),
                symbolica_reducer,
                symbolica_catalog_easiest_first: Vec::new(),
                limits,
                stats: GeneratedAffineResidualGroupExactDatabaseStats {
                    retained_database_bytes: size_of::<Self>(),
                    ..GeneratedAffineResidualGroupExactDatabaseStats::default()
                },
            })
        }))
        .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::SymbolicaPanic)?
    }

    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) const fn source_kind(&self) -> GeneratedAffineResidualCaseAuthoritySourceKind {
        self.source_kind
    }

    pub(crate) const fn database_epoch(&self) -> usize {
        self.database_epoch
    }

    pub(crate) const fn group_ordinal(&self) -> usize {
        self.group_ordinal
    }

    pub(crate) const fn state_version(&self) -> usize {
        self.state_version
    }

    /// Mint an admissible authority for an initial exact target state.
    ///
    /// The returned value is deliberately non-`Clone` and non-constructible
    /// outside this module. Repeated calls while the database is pristine may
    /// mint equivalent authorities; uniqueness of a target-state owner is a
    /// separate session-layer responsibility.
    pub(crate) fn initial_target_state_binding_for_session(
        &self,
        _capability: &GeneratedAffineResidualGroupExactSessionDatabaseCapability,
    ) -> Result<
        GeneratedAffineResidualGroupExactTargetStateBinding,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        self.initial_target_state_binding_inner()
    }

    /// Explicit test-only adapter for database/target-state unit tests.
    #[cfg(test)]
    pub(crate) fn initial_target_state_binding_for_test(
        &self,
    ) -> Result<
        GeneratedAffineResidualGroupExactTargetStateBinding,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        self.initial_target_state_binding_inner()
    }

    fn initial_target_state_binding_inner(
        &self,
    ) -> Result<
        GeneratedAffineResidualGroupExactTargetStateBinding,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        self.authenticate_source_profile()?;
        if self.state_version != 0
            || self.next_source_ordinal != 0
            || !self.pivots.is_empty()
            || !self.lookup.is_empty()
            || self.transition_identity != ExactDatabaseTransitionIdentity::PRISTINE
        {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::WrongTargetStateBinding);
        }
        Ok(self.target_state_binding_at(self.state_version, self.transition_identity, None))
    }

    /// Pre-authenticate one staged row and mint the authority for the target
    /// state that must coexist with the database after that row is committed.
    /// Dropping either value still mutates nothing.
    pub(crate) fn successor_target_state_binding_for_session(
        &self,
        _capability: &GeneratedAffineResidualGroupExactSessionDatabaseCapability,
        staged: &GeneratedAffineResidualGroupStagedExactRow,
    ) -> Result<
        GeneratedAffineResidualGroupExactTargetStateBinding,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        self.successor_target_state_binding_inner(staged)
    }

    /// Explicit test-only adapter for database/target-state unit tests.
    #[cfg(test)]
    pub(crate) fn successor_target_state_binding_for_test(
        &self,
        staged: &GeneratedAffineResidualGroupStagedExactRow,
    ) -> Result<
        GeneratedAffineResidualGroupExactTargetStateBinding,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        self.successor_target_state_binding_inner(staged)
    }

    fn successor_target_state_binding_inner(
        &self,
        staged: &GeneratedAffineResidualGroupStagedExactRow,
    ) -> Result<
        GeneratedAffineResidualGroupExactTargetStateBinding,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        self.authenticate_staged_row(staged)?;
        Ok(self.target_state_binding_at(
            staged.next_state_version,
            staged.next_transition_identity,
            Some(self.transition_identity),
        ))
    }

    /// Authenticate a target-state authority against this exact live database
    /// allocation and exact transition. This check includes the hidden
    /// database and transition identities; equality of the visible
    /// plan/group/epoch/version coordinates is not sufficient.
    pub(crate) fn authenticate_target_state_binding(
        &self,
        binding: &GeneratedAffineResidualGroupExactTargetStateBinding,
    ) -> Result<(), GeneratedAffineResidualGroupExactDatabaseError> {
        self.authenticate_source_profile()?;
        if binding.database_nonce != self.database_nonce
            || binding.transition_identity != self.transition_identity
            || !Arc::ptr_eq(&binding.plan, &self.plan)
            || !Arc::ptr_eq(&binding.frame, &self.frame)
            || binding.database_epoch != self.database_epoch
            || binding.group_ordinal != self.group_ordinal
            || binding.state_version != self.state_version
            || (binding.state_version == 0) != binding.predecessor_transition_identity.is_none()
        {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::WrongTargetStateBinding);
        }
        Ok(())
    }

    fn target_state_binding_at(
        &self,
        state_version: usize,
        transition_identity: ExactDatabaseTransitionIdentity,
        predecessor_transition_identity: Option<ExactDatabaseTransitionIdentity>,
    ) -> GeneratedAffineResidualGroupExactTargetStateBinding {
        GeneratedAffineResidualGroupExactTargetStateBinding {
            database_nonce: self.database_nonce,
            transition_identity,
            predecessor_transition_identity,
            plan: Arc::clone(&self.plan),
            frame: Arc::clone(&self.frame),
            database_epoch: self.database_epoch,
            group_ordinal: self.group_ordinal,
            state_version,
        }
    }

    pub(crate) fn pivot_count(&self) -> usize {
        self.pivots.len()
    }

    pub(crate) const fn stats(&self) -> GeneratedAffineResidualGroupExactDatabaseStats {
        self.stats
    }

    /// Current retained native shape plus the database's Rust-side retained
    /// byte envelope. Unlike historical per-stage scaling telemetry, the U/L
    /// stored-entry counts and shallow public vector-capacity slots here
    /// describe the live committed reducer after discarded dependent trials
    /// have been dropped.
    pub(crate) fn resident_resource_snapshot(
        &self,
    ) -> GeneratedAffineResidualGroupExactDatabaseResidentResourceSnapshot {
        GeneratedAffineResidualGroupExactDatabaseResidentResourceSnapshot {
            physical_columns: self.symbolica_reducer.physical_columns(),
            independent_rows: self.symbolica_reducer.independent_rows(),
            native_u_rows: self.symbolica_reducer.native_u_rows(),
            native_u_columns: self.symbolica_reducer.native_u_columns(),
            native_l_rows: self.symbolica_reducer.native_l_rows(),
            native_l_columns: self.symbolica_reducer.native_l_columns(),
            native_u_stored_entries: self.symbolica_reducer.native_u_entries(),
            native_l_stored_entries: self.symbolica_reducer.native_l_entries(),
            native_shallow_capacity_slots: self.symbolica_reducer.shallow_capacity_snapshot(),
            retained_database_bytes: self.stats.retained_database_bytes(),
        }
    }

    pub(crate) const fn publishes_rule(&self) -> bool {
        false
    }

    pub(crate) const fn infers_master(&self) -> bool {
        false
    }

    pub(crate) fn pivot(
        &self,
        ordinal: usize,
    ) -> Option<GeneratedAffineResidualGroupExactUnitPivotView<'_>> {
        self.pivots
            .get(ordinal)
            .map(|pivot| GeneratedAffineResidualGroupExactUnitPivotView { pivot })
    }

    #[cfg(test)]
    fn lookup_pivot(
        &self,
        key: &GeneratedAffineResidualGroupPhysicalKey,
    ) -> Option<GeneratedAffineResidualGroupExactUnitPivotView<'_>> {
        let position = self
            .lookup
            .binary_search_by(|entry| entry.key.cmp(key))
            .ok()?;
        self.pivot(self.lookup[position].pivot_ordinal)
    }

    fn authenticate_binding(
        &self,
        plan: &Arc<GeneratedAffineResidualGroupSolvePlan>,
        frame: &Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        database_epoch: usize,
    ) -> Result<(), GeneratedAffineResidualGroupExactDatabaseError> {
        self.authenticate_source_profile()?;
        if !Arc::ptr_eq(&self.plan, plan) {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::WrongPlanAllocation);
        }
        if !Arc::ptr_eq(&self.frame, frame) {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::WrongFrameAllocation);
        }
        if self.database_epoch != database_epoch {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::WrongDatabaseEpoch);
        }
        Ok(())
    }

    /// Authenticate and stage one raw physical row without mutating this
    /// database. Dropping the returned consume-once token commits nothing.
    pub(crate) fn stage_replayed_row_for_session(
        &self,
        _capability: &GeneratedAffineResidualGroupExactSessionDatabaseCapability,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        plan: &Arc<GeneratedAffineResidualGroupSolvePlan>,
        frame: &Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        database_epoch: usize,
        source: &Arc<GeneratedAffineResidualGroupExactPhysicalRow>,
    ) -> Result<
        GeneratedAffineResidualGroupStagedExactRow,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            self.stage_replayed_row_inner(family, context, plan, frame, database_epoch, source)
        }))
        .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::SymbolicaPanic)?
    }

    /// Retain the exact raw-row recipe carried by one live authenticated stage.
    /// No source allocation is reconstructed or compared by value.
    pub(crate) fn retain_source_recipe_for_session(
        &self,
        _capability: &GeneratedAffineResidualGroupExactSessionDatabaseCapability,
        staged: &GeneratedAffineResidualGroupStagedExactRow,
    ) -> Result<
        GeneratedAffineResidualGroupRetainedExactSourceRecipe,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        self.authenticate_staged_row(staged)?;
        self.retained_source_recipe(staged)
    }

    /// Restage an opaque retained recipe through the same exact raw-row ingress.
    /// This is the chronological replay seam; the recipe remains bound to the
    /// original plan/frame allocation, group, and database epoch while a fresh
    /// database allocation may replay it.
    pub(crate) fn stage_retained_source_recipe_for_session(
        &self,
        _capability: &GeneratedAffineResidualGroupExactSessionDatabaseCapability,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        recipe: &GeneratedAffineResidualGroupRetainedExactSourceRecipe,
    ) -> Result<
        GeneratedAffineResidualGroupStagedExactRow,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            self.stage_retained_source_recipe_inner(family, context, recipe)
        }))
        .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::SymbolicaPanic)?
    }

    /// Explicit test-only adapter for replaying the same opaque recipe ingress
    /// without making the session capability forgeable in database tests.
    #[cfg(test)]
    fn stage_retained_source_recipe_for_test(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        recipe: &GeneratedAffineResidualGroupRetainedExactSourceRecipe,
    ) -> Result<
        GeneratedAffineResidualGroupStagedExactRow,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            self.stage_retained_source_recipe_inner(family, context, recipe)
        }))
        .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::SymbolicaPanic)?
    }

    fn stage_retained_source_recipe_inner(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        recipe: &GeneratedAffineResidualGroupRetainedExactSourceRecipe,
    ) -> Result<
        GeneratedAffineResidualGroupStagedExactRow,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        self.authenticate_source_profile()?;
        if !Arc::ptr_eq(&recipe.plan, &self.plan)
            || !Arc::ptr_eq(&recipe.frame, &self.frame)
            || recipe.database_epoch != self.database_epoch
            || recipe.group_ordinal != self.group_ordinal
            || !recipe.source.authenticates_own_allocation()
        {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::RowReplay);
        }
        match &recipe.source {
            ExactStagedSource::Production { source, .. } => self.stage_replayed_row_inner(
                family,
                context,
                &recipe.plan,
                &recipe.frame,
                recipe.database_epoch,
                source,
            ),
            #[cfg(test)]
            ExactStagedSource::Synthetic { source, .. } => {
                self.stage_synthetic_source_recipe(context, Arc::clone(source))
            }
        }
    }

    fn retained_source_recipe(
        &self,
        staged: &GeneratedAffineResidualGroupStagedExactRow,
    ) -> Result<
        GeneratedAffineResidualGroupRetainedExactSourceRecipe,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        if !staged.source.authenticates_own_allocation() {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::InvalidStagedRow);
        }
        Ok(GeneratedAffineResidualGroupRetainedExactSourceRecipe {
            plan: Arc::clone(&self.plan),
            frame: Arc::clone(&self.frame),
            database_epoch: self.database_epoch,
            group_ordinal: self.group_ordinal,
            source: staged.source.clone(),
        })
    }

    /// Explicit test-only adapter for algebraic database and target-state
    /// transaction tests. Production callers must use the capability-gated
    /// session entry point above.
    #[cfg(test)]
    pub(crate) fn stage_replayed_row_for_test(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        plan: &Arc<GeneratedAffineResidualGroupSolvePlan>,
        frame: &Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        database_epoch: usize,
        source: &Arc<GeneratedAffineResidualGroupExactPhysicalRow>,
    ) -> Result<
        GeneratedAffineResidualGroupStagedExactRow,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            self.stage_replayed_row_inner(family, context, plan, frame, database_epoch, source)
        }))
        .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::SymbolicaPanic)?
    }

    fn stage_replayed_row_inner(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        plan: &Arc<GeneratedAffineResidualGroupSolvePlan>,
        frame: &Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        database_epoch: usize,
        source: &Arc<GeneratedAffineResidualGroupExactPhysicalRow>,
    ) -> Result<
        GeneratedAffineResidualGroupStagedExactRow,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        self.authenticate_binding(plan, frame, database_epoch)?;
        let next_source_ordinal = self.preflight_next_source_ordinal()?;
        let next_state_version = self.preflight_next_state_version()?;
        let view = source
            .replay_for_database(family, context, frame)
            .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::RowReplay)?;
        if view.group_ordinal() != self.group_ordinal {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::WrongGroup);
        }
        self.stage_view(
            context,
            view,
            next_source_ordinal,
            next_state_version,
            ExactStagedSource::production(source),
        )
    }

    /// Explicit test-only compatibility adapter. Production transaction
    /// owners must keep the capability-gated session token through
    /// recentering and `WhenBad`, then commit through a typed session path.
    #[cfg(test)]
    pub(crate) fn ingest_replayed_row_for_test(
        &mut self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        plan: &Arc<GeneratedAffineResidualGroupSolvePlan>,
        frame: &Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        database_epoch: usize,
        source: &Arc<GeneratedAffineResidualGroupExactPhysicalRow>,
    ) -> Result<
        GeneratedAffineResidualGroupExactRowOutcome,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        let staged =
            self.stage_replayed_row_for_test(family, context, plan, frame, database_epoch, source)?;
        self.commit_staged_row_for_test(staged)
    }

    fn stage_view(
        &self,
        context: &ParametricCoefficientContext,
        view: GeneratedAffineResidualGroupReplayedExactPhysicalRow<'_>,
        next_source_ordinal: usize,
        next_state_version: usize,
        source: ExactStagedSource,
    ) -> Result<
        GeneratedAffineResidualGroupStagedExactRow,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        let mut ledger = ParametricCoefficientWorkLedger::new(
            ParametricCoefficientWorkPhase::Construction,
            self.limits.coefficient_work,
        );
        check_limit(
            "terms in one exact top-reduction row",
            view.term_count(),
            self.limits.max_terms_per_row,
        )?;
        let ingress = preflight_borrowed_ingress(
            context,
            &self.frame,
            view.terms(),
            view.term_count(),
            view.guards(),
            self.limits,
        )?;
        check_limit(
            "exact-group borrowed ingress prospective retained bytes",
            ingress.prospective_retained_bytes,
            self.limits.max_ingress_retained_bytes,
        )?;
        let mut terms = try_terms_with_capacity(view.term_count())?;
        let mut guards = try_guards_with_capacity(view.guard_count())?;
        let observed_ingress_retained_bytes =
            ingress.observed_retained_bytes(terms.capacity(), guards.capacity())?;
        check_limit(
            "exact-group borrowed ingress observed retained bytes",
            observed_ingress_retained_bytes,
            self.limits.max_ingress_retained_bytes,
        )?;
        for (key, coefficient) in view.terms() {
            terms.push(ExactDatabaseTerm {
                key: key.clone(),
                coefficient: ledger
                    .try_copy_authenticated(coefficient)
                    .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?,
            });
        }
        guards.extend(view.guards().iter().cloned());
        self.finish_stage(
            context,
            terms,
            guards,
            ledger,
            next_source_ordinal,
            next_state_version,
            ingress.prospective_retained_bytes,
            observed_ingress_retained_bytes,
            source,
        )
    }

    fn finish_stage(
        &self,
        context: &ParametricCoefficientContext,
        mut terms: Vec<ExactDatabaseTerm>,
        mut guards: Vec<ParametricNonZeroCondition>,
        mut ledger: ParametricCoefficientWorkLedger,
        next_source_ordinal: usize,
        next_state_version: usize,
        ingress_prospective_retained_bytes: usize,
        ingress_observed_retained_bytes: usize,
        source: ExactStagedSource,
    ) -> Result<
        GeneratedAffineResidualGroupStagedExactRow,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        let source_ordinal = self.next_source_ordinal;
        debug_assert_eq!(source_ordinal.checked_add(1), Some(next_source_ordinal));
        debug_assert_eq!(self.state_version.checked_add(1), Some(next_state_version));
        let candidate_was_empty = terms.is_empty();
        let native_transcript = symbolica_sparse_transcript(
            context,
            &self.symbolica_reducer,
            &self.symbolica_catalog_easiest_first,
            &terms,
            self.limits.symbolica_sparse,
        )?;
        check_limit(
            "top-reduction steps in one row",
            native_transcript.outcome.reductions().len(),
            self.limits.max_reductions_per_row,
        )?;
        let mut reductions = Vec::new();

        for native_reduction in native_transcript.outcome.reductions() {
            let requested = reductions.len().checked_add(1).ok_or(
                GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                    resource: "top-reduction steps in one row",
                },
            )?;
            check_limit(
                "top-reduction steps in one row",
                requested,
                self.limits.max_reductions_per_row,
            )?;
            reductions.try_reserve_exact(1).map_err(|_| {
                GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
                    resource: "top-reduction steps",
                }
            })?;
            let hardest = terms.last().ok_or(
                GeneratedAffineResidualGroupExactDatabaseError::SymbolicaTranscriptMismatch,
            )?;
            let pivot_ordinal = native_reduction.pivot_row();
            if self.lookup_ordinal(&hardest.key) != Some(pivot_ordinal)
                || native_reduction.factor() != &hardest.coefficient
            {
                return Err(
                    GeneratedAffineResidualGroupExactDatabaseError::SymbolicaTranscriptMismatch,
                );
            }
            let factor = ledger
                .try_copy_authenticated(native_reduction.factor())
                .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?;
            let pivot = self.pivots.get(pivot_ordinal).ok_or(
                GeneratedAffineResidualGroupExactDatabaseError::SymbolicaTranscriptMismatch,
            )?;
            if pivot.terms.last().map(|term| &term.key) != Some(&hardest.key) {
                return Err(
                    GeneratedAffineResidualGroupExactDatabaseError::SymbolicaTranscriptMismatch,
                );
            }
            terms.pop();
            merge_guards(
                context,
                &mut guards,
                &pivot.guards,
                self.limits.max_guards_per_row,
                self.limits.max_guard_origins,
                self.limits.coefficient_work.arithmetic.max_guard_origins,
            )?;
            for (term_ordinal, pivot_term) in pivot
                .terms
                .iter()
                .take(pivot.terms.len().saturating_sub(1))
                .enumerate()
            {
                let scaled = ledger
                    .try_mul(context, &factor, &pivot_term.coefficient)
                    .and_then(|value| ledger.try_neg(context, &value))
                    .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?;
                add_sorted_term(
                    context,
                    &mut ledger,
                    &mut terms,
                    pivot_term.key.clone(),
                    scaled,
                    self.limits.max_terms_per_row,
                )?;
                if let Ok(position) = terms.binary_search_by(|term| term.key.cmp(&pivot_term.key)) {
                    let origin =
                        GuardOrigin::GeneratedAffineGroupTopReductionCoefficientDenominator {
                            solve_group_ordinal: self.group_ordinal,
                            database_epoch: self.database_epoch,
                            event_ordinal: source_ordinal,
                            operation_ordinal: reductions.len(),
                            term_ordinal,
                            pivot_normalization: false,
                        };
                    insert_denominator_guard(
                        context,
                        &mut ledger,
                        &mut guards,
                        &terms[position].coefficient,
                        origin,
                        self.limits,
                    )?;
                }
            }
            reductions.push(GeneratedAffineResidualGroupExactReductionStep {
                pivot_ordinal,
                factor,
            });
        }

        match &native_transcript.outcome {
            SymbolicaPersistentSparseOutcome::Dependent {
                canonical_zero_input,
                ..
            } => {
                if !terms.is_empty() || *canonical_zero_input != candidate_was_empty {
                    return Err(
                        GeneratedAffineResidualGroupExactDatabaseError::SymbolicaTranscriptMismatch,
                    );
                }
                let native_sparse_event =
                    GeneratedAffineResidualGroupExactNativeSparseStageStats::from_adapter(
                        native_transcript.outcome.stats(),
                    );
                drop(native_transcript);
                let prospective_staged_live_retained_bytes = dependent_staged_live_retained_bytes(
                    self.stats.retained_database_bytes,
                    &reductions,
                    reductions.len(),
                    source.unique_retained_bytes()?,
                )?;
                check_limit(
                    "exact-group staged live retained bytes",
                    prospective_staged_live_retained_bytes,
                    self.limits.max_staged_live_retained_bytes,
                )?;
                let observed_staged_live_retained_bytes = dependent_staged_live_retained_bytes(
                    self.stats.retained_database_bytes,
                    &reductions,
                    reductions.capacity(),
                    source.unique_retained_bytes()?,
                )?;
                check_limit(
                    "exact-group staged live retained bytes",
                    observed_staged_live_retained_bytes,
                    self.limits.max_staged_live_retained_bytes,
                )?;
                let ingress_stats = self.stats_with_ingress(
                    ingress_prospective_retained_bytes,
                    ingress_observed_retained_bytes,
                );
                let ingress_stats =
                    self.stats_with_native_sparse_event(ingress_stats, native_sparse_event);
                let committed_stats = self.stats_with_staged_live(
                    ingress_stats,
                    prospective_staged_live_retained_bytes,
                    observed_staged_live_retained_bytes,
                );
                let next_transition_identity = next_exact_database_transition_identity()?;
                let reductions = Arc::new(reductions);
                return Ok(GeneratedAffineResidualGroupStagedExactRow {
                    database_nonce: self.database_nonce,
                    next_transition_identity,
                    database_epoch: self.database_epoch,
                    group_ordinal: self.group_ordinal,
                    state_version: self.state_version,
                    next_state_version,
                    source_ordinal,
                    next_source_ordinal,
                    pivot_count: self.pivots.len(),
                    lookup_len: self.lookup.len(),
                    catalog_len: self.symbolica_catalog_easiest_first.len(),
                    source,
                    payload: ExactStagedRowPayload::Dependent {
                        reductions,
                        committed_stats,
                    },
                });
            }
            SymbolicaPersistentSparseOutcome::Independent {
                normalization_divisor,
                ..
            } => {
                let hardest = terms.last().ok_or(
                    GeneratedAffineResidualGroupExactDatabaseError::SymbolicaTranscriptMismatch,
                )?;
                if native_transcript.normalized_keys_hardest_first.first() != Some(&hardest.key)
                    || normalization_divisor != &hardest.coefficient
                {
                    return Err(
                        GeneratedAffineResidualGroupExactDatabaseError::SymbolicaTranscriptMismatch,
                    );
                }
            }
        }

        let pivot_ordinal = self.pivots.len();
        let requested = pivot_ordinal.checked_add(1).ok_or(
            GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                resource: "exact group pivots",
            },
        )?;
        check_limit("exact group pivots", requested, self.limits.max_pivots)?;
        let normalization_divisor = normalize_unknown_leader(
            context,
            &mut ledger,
            &mut terms,
            &mut guards,
            self.group_ordinal,
            self.database_epoch,
            source_ordinal,
            self.limits,
        )?;
        let SymbolicaPersistentSparseOutcome::Independent {
            normalized_row,
            normalization_divisor: native_normalization_divisor,
            ..
        } = &native_transcript.outcome
        else {
            return Err(
                GeneratedAffineResidualGroupExactDatabaseError::SymbolicaTranscriptMismatch,
            );
        };
        if native_normalization_divisor != &normalization_divisor
            || normalized_row.entries().len() != terms.len()
            || native_transcript.normalized_keys_hardest_first.len() != terms.len()
        {
            return Err(
                GeneratedAffineResidualGroupExactDatabaseError::SymbolicaTranscriptMismatch,
            );
        }
        for ((native_entry, native_key), term) in normalized_row
            .entries()
            .iter()
            .zip(&native_transcript.normalized_keys_hardest_first)
            .zip(terms.iter().rev())
        {
            if native_key != &term.key || native_entry.coefficient() != &term.coefficient {
                return Err(
                    GeneratedAffineResidualGroupExactDatabaseError::SymbolicaTranscriptMismatch,
                );
            }
        }
        let native_sparse_event =
            GeneratedAffineResidualGroupExactNativeSparseStageStats::from_adapter(
                native_transcript.outcome.stats(),
            );
        let ExactDatabaseSymbolicaTranscript {
            outcome,
            normalized_keys_hardest_first: _,
            successor_catalog_easiest_first,
        } = native_transcript;
        let SymbolicaPersistentSparseOutcome::Independent {
            successor: successor_reducer,
            ..
        } = outcome
        else {
            return Err(
                GeneratedAffineResidualGroupExactDatabaseError::SymbolicaTranscriptMismatch,
            );
        };
        let pivot_key = terms
            .last()
            .ok_or(GeneratedAffineResidualGroupExactDatabaseError::InvalidUnitPivot)?
            .key
            .clone();
        let insertion = self
            .lookup
            .binary_search_by(|entry| entry.key.cmp(&pivot_key))
            .map_or_else(|position| position, |position| position);
        if self
            .lookup
            .get(insertion)
            .is_some_and(|entry| entry.key == pivot_key)
        {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::InvalidUnitPivot);
        }
        let prospective_retained_bytes =
            pivot_retained_bytes(&terms, &guards, &reductions, &normalization_divisor, false)?;
        check_limit(
            "exact-group pivot prospective retained bytes",
            prospective_retained_bytes,
            self.limits.max_candidate_retained_bytes,
        )?;
        let reductions = Arc::new(reductions);
        let pivot = Arc::new(ExactUnitPivot {
            ordinal: pivot_ordinal,
            source_ordinal,
            terms,
            guards,
            reductions,
            normalization_divisor,
        });
        let observed_retained_bytes = exact_unit_pivot_retained_bytes(&pivot)?;
        check_limit(
            "exact-group pivot observed retained bytes",
            observed_retained_bytes,
            self.limits.max_candidate_retained_bytes,
        )?;
        let prospective_catalog_replacement_slots =
            successor_catalog_easiest_first.as_ref().map_or(0, Vec::len);
        let prospective_database_catalog_slots = successor_catalog_easiest_first
            .as_ref()
            .map_or(self.symbolica_catalog_easiest_first.capacity(), Vec::len);
        let prospective_staged_live_retained_bytes = new_pivot_staged_live_retained_bytes(
            self.stats.retained_database_bytes,
            &pivot,
            requested,
            requested,
            prospective_catalog_replacement_slots,
            source.unique_retained_bytes()?,
        )?;
        check_limit(
            "exact-group staged live retained bytes",
            prospective_staged_live_retained_bytes,
            self.limits.max_staged_live_retained_bytes,
        )?;
        let prospective_database_retained_bytes = database_retained_bytes_with_candidate(
            &self.pivots,
            requested,
            requested,
            prospective_database_catalog_slots,
            Some(&pivot),
        )?;
        check_limit(
            "exact-group database retained bytes",
            prospective_database_retained_bytes,
            self.limits.max_database_retained_bytes,
        )?;

        // Allocate both complete replacement buffers before touching either
        // retained index. If the second allocation fails, the first remains a
        // local and is dropped while the database stays byte-for-byte intact.
        let committed_pivots = try_pivot_replacement_with_capacity(requested)?;
        #[cfg(test)]
        if take_fail_next_lookup_replacement_allocation_for_test() {
            return Err(
                GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
                    resource: "sorted exact-pivot lookup replacement",
                },
            );
        }
        let committed_lookup = try_lookup_replacement_with_capacity(requested)?;
        let observed_catalog_replacement_slots = successor_catalog_easiest_first
            .as_ref()
            .map_or(0, Vec::capacity);
        let observed_database_catalog_slots = successor_catalog_easiest_first.as_ref().map_or(
            self.symbolica_catalog_easiest_first.capacity(),
            Vec::capacity,
        );
        let observed_staged_live_retained_bytes = new_pivot_staged_live_retained_bytes(
            self.stats.retained_database_bytes,
            &pivot,
            committed_pivots.capacity(),
            committed_lookup.capacity(),
            observed_catalog_replacement_slots,
            source.unique_retained_bytes()?,
        )?;
        check_limit(
            "exact-group staged live retained bytes",
            observed_staged_live_retained_bytes,
            self.limits.max_staged_live_retained_bytes,
        )?;
        let retained_database_bytes = database_retained_bytes_with_candidate(
            &self.pivots,
            committed_pivots.capacity(),
            committed_lookup.capacity(),
            observed_database_catalog_slots,
            Some(&pivot),
        )?;
        check_limit(
            "exact-group database retained bytes",
            retained_database_bytes,
            self.limits.max_database_retained_bytes,
        )?;
        let ingress_stats = self.stats_with_ingress(
            ingress_prospective_retained_bytes,
            ingress_observed_retained_bytes,
        );
        let ingress_stats = self.stats_with_native_sparse_event(ingress_stats, native_sparse_event);
        let candidate_stats = GeneratedAffineResidualGroupExactDatabaseStats {
            retained_database_bytes,
            last_candidate_prospective_retained_bytes: prospective_retained_bytes,
            last_candidate_observed_retained_bytes: observed_retained_bytes,
            peak_candidate_retained_bytes: ingress_stats
                .peak_candidate_retained_bytes
                .max(observed_retained_bytes),
            ..ingress_stats
        };
        let committed_stats = self.stats_with_staged_live(
            candidate_stats,
            prospective_staged_live_retained_bytes,
            observed_staged_live_retained_bytes,
        );
        let next_transition_identity = next_exact_database_transition_identity()?;

        Ok(GeneratedAffineResidualGroupStagedExactRow {
            database_nonce: self.database_nonce,
            next_transition_identity,
            database_epoch: self.database_epoch,
            group_ordinal: self.group_ordinal,
            state_version: self.state_version,
            next_state_version,
            source_ordinal,
            next_source_ordinal,
            pivot_count: self.pivots.len(),
            lookup_len: self.lookup.len(),
            catalog_len: self.symbolica_catalog_easiest_first.len(),
            source,
            payload: ExactStagedRowPayload::NewPivot {
                pivot,
                pivot_key,
                lookup_insertion: insertion,
                successor_reducer,
                successor_catalog_easiest_first,
                committed_pivots,
                committed_lookup,
                committed_stats,
            },
        })
    }

    /// Authenticate and borrow one staged new pivot together with every
    /// allocation and transaction coordinate that authorizes its use.
    pub(crate) fn authenticate_staged_new_pivot_for_session<'a>(
        &'a self,
        _capability: &GeneratedAffineResidualGroupExactSessionDatabaseCapability,
        staged: &'a GeneratedAffineResidualGroupStagedExactRow,
    ) -> Result<
        GeneratedAffineResidualGroupAuthenticatedStagedNewPivotView<'a>,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        self.authenticate_staged_new_pivot_inner(staged)
    }

    /// Explicit test-only adapter for database-local staged-pivot assertions.
    #[cfg(test)]
    fn authenticate_staged_new_pivot_for_test<'a>(
        &'a self,
        staged: &'a GeneratedAffineResidualGroupStagedExactRow,
    ) -> Result<
        GeneratedAffineResidualGroupAuthenticatedStagedNewPivotView<'a>,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        self.authenticate_staged_new_pivot_inner(staged)
    }

    fn authenticate_staged_new_pivot_inner<'a>(
        &'a self,
        staged: &'a GeneratedAffineResidualGroupStagedExactRow,
    ) -> Result<
        GeneratedAffineResidualGroupAuthenticatedStagedNewPivotView<'a>,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        self.authenticate_staged_row(staged)?;
        let ExactStagedRowPayload::NewPivot { pivot, .. } = &staged.payload else {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::DependentStagedRow);
        };
        Ok(
            GeneratedAffineResidualGroupAuthenticatedStagedNewPivotView {
                database: self,
                staged,
                pivot: pivot.as_ref(),
            },
        )
    }

    /// Authenticate and classify one live staged row as dependent without
    /// exposing its consume-once token or the database authority used for the
    /// classification.  New pivots are rejected symmetrically with
    /// [`Self::authenticate_staged_new_pivot_for_session`]'s dependent-row
    /// rejection.
    pub(crate) fn authenticate_staged_dependent_for_session<'a>(
        &'a self,
        _capability: &GeneratedAffineResidualGroupExactSessionDatabaseCapability,
        staged: &'a GeneratedAffineResidualGroupStagedExactRow,
    ) -> Result<
        GeneratedAffineResidualGroupAuthenticatedStagedDependentView<'a>,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        self.authenticate_staged_row(staged)?;
        let ExactStagedRowPayload::Dependent { reductions, .. } = &staged.payload else {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::NewPivotStagedRow);
        };
        Ok(
            GeneratedAffineResidualGroupAuthenticatedStagedDependentView {
                database: self,
                staged,
                reductions,
            },
        )
    }

    /// Consume and completely authenticate a stage before any database
    /// mutation. Failure returns the exact staged token to the session.
    pub(crate) fn prepare_staged_row_commit_for_session(
        &self,
        _capability: &GeneratedAffineResidualGroupExactSessionDatabaseCapability,
        staged: GeneratedAffineResidualGroupStagedExactRow,
    ) -> Result<
        GeneratedAffineResidualGroupPreparedExactRowCommit,
        GeneratedAffineResidualGroupPrepareExactRowCommitFailure,
    > {
        self.prepare_staged_row_commit_inner(staged)
    }

    /// Recover the original stage when a fallible outer-owner preparation
    /// fails after database preparation but before the final move-only tail.
    pub(crate) fn abort_prepared_staged_row_commit_for_session(
        &self,
        _capability: &GeneratedAffineResidualGroupExactSessionDatabaseCapability,
        prepared: GeneratedAffineResidualGroupPreparedExactRowCommit,
    ) -> GeneratedAffineResidualGroupStagedExactRow {
        prepared.staged
    }

    /// Infallible database-local tail for a prepared owning token. Immediately
    /// before mutation, a local allocation-free assertion verifies the live
    /// database/predecessor identity and all move-only capacity assumptions;
    /// invariant violation is fail-stop rather than a recoverable post-prepare
    /// branch. Safe production code can reach this only through the
    /// allocation-sealed session capability and an exclusive database borrow.
    pub(crate) fn commit_prepared_staged_row_for_session(
        &mut self,
        _capability: &GeneratedAffineResidualGroupExactSessionDatabaseCapability,
        prepared: GeneratedAffineResidualGroupPreparedExactRowCommit,
    ) {
        drop(self.commit_prepared_staged_row_inner(prepared));
    }

    /// Final publication-only database move. The exclusive session
    /// authenticated this exact staged new pivot immediately before all
    /// recoverable preparation, so the tail moves it directly without
    /// repackaging duplicate recipe or evidence owners.
    pub(crate) fn commit_current_staged_new_pivot_for_session(
        &mut self,
        _capability: &GeneratedAffineResidualGroupExactSessionDatabaseCapability,
        staged: GeneratedAffineResidualGroupStagedExactRow,
    ) {
        debug_assert_eq!(self.database_nonce, staged.database_nonce);
        debug_assert_eq!(self.state_version, staged.state_version);
        debug_assert!(matches!(
            &staged.payload,
            ExactStagedRowPayload::NewPivot { .. }
        ));
        let _ = self.commit_staged_new_pivot_move_inner(staged);
    }

    /// Explicit test-only adapters exercise the owning prepare/abort/commit
    /// protocol while leaving the production capability unforgeable.
    #[cfg(test)]
    fn prepare_staged_row_commit_for_test(
        &self,
        staged: GeneratedAffineResidualGroupStagedExactRow,
    ) -> Result<
        GeneratedAffineResidualGroupPreparedExactRowCommit,
        GeneratedAffineResidualGroupPrepareExactRowCommitFailure,
    > {
        self.prepare_staged_row_commit_inner(staged)
    }

    #[cfg(test)]
    fn abort_prepared_staged_row_commit_for_test(
        &self,
        prepared: GeneratedAffineResidualGroupPreparedExactRowCommit,
    ) -> GeneratedAffineResidualGroupStagedExactRow {
        prepared.staged
    }

    #[cfg(test)]
    fn commit_prepared_staged_row_for_test(
        &mut self,
        prepared: GeneratedAffineResidualGroupPreparedExactRowCommit,
    ) {
        drop(self.commit_prepared_staged_row_inner(prepared));
    }

    /// Test-only compatibility adapter returning the historical owned-vector
    /// outcome used by algebraic transaction tests. Production retains shared
    /// evidence through the owning prepare/commit protocol instead.
    #[cfg(test)]
    pub(crate) fn commit_staged_row_for_test(
        &mut self,
        staged: GeneratedAffineResidualGroupStagedExactRow,
    ) -> Result<
        GeneratedAffineResidualGroupExactRowOutcome,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        self.commit_staged_row_inner(staged)
    }

    #[cfg(test)]
    fn commit_staged_row_inner(
        &mut self,
        staged: GeneratedAffineResidualGroupStagedExactRow,
    ) -> Result<
        GeneratedAffineResidualGroupExactRowOutcome,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        let prepared = self
            .prepare_staged_row_commit_inner(staged)
            .map_err(|failure| failure.error)?;
        Ok(match self.commit_prepared_staged_row_inner(prepared) {
            GeneratedAffineResidualGroupPreparedExactRowOutcome::Dependent {
                source_ordinal,
                evidence,
            } => {
                let reductions = Arc::try_unwrap(evidence.reductions)
                    .unwrap_or_else(|shared| shared.as_ref().clone());
                GeneratedAffineResidualGroupExactRowOutcome::Dependent {
                    source_ordinal,
                    reductions,
                }
            }
            GeneratedAffineResidualGroupPreparedExactRowOutcome::NewPivot {
                source_ordinal,
                pivot_ordinal,
                ..
            } => GeneratedAffineResidualGroupExactRowOutcome::NewPivot {
                source_ordinal,
                pivot_ordinal,
            },
        })
    }

    fn prepare_staged_row_commit_inner(
        &self,
        staged: GeneratedAffineResidualGroupStagedExactRow,
    ) -> Result<
        GeneratedAffineResidualGroupPreparedExactRowCommit,
        GeneratedAffineResidualGroupPrepareExactRowCommitFailure,
    > {
        let prepared = (|| {
            self.authenticate_staged_row(&staged)?;
            let source_recipe = self.retained_source_recipe(&staged)?;
            let evidence = match &staged.payload {
                ExactStagedRowPayload::Dependent { reductions, .. } => {
                    PreparedExactRowEvidence::Dependent(
                        GeneratedAffineResidualGroupRetainedExactDependentReductions {
                            reductions: Arc::clone(reductions),
                        },
                    )
                }
                ExactStagedRowPayload::NewPivot { pivot, .. } => {
                    PreparedExactRowEvidence::NewPivot(
                        GeneratedAffineResidualGroupRetainedExactUnitPivot {
                            pivot: Arc::clone(pivot),
                        },
                    )
                }
            };
            Ok((source_recipe, evidence))
        })();
        match prepared {
            Ok((source_recipe, evidence)) => {
                Ok(GeneratedAffineResidualGroupPreparedExactRowCommit {
                    predecessor_transition_identity: self.transition_identity,
                    staged,
                    source_recipe,
                    evidence,
                })
            }
            Err(error) => {
                Err(GeneratedAffineResidualGroupPrepareExactRowCommitFailure { error, staged })
            }
        }
    }

    fn commit_prepared_staged_row_inner(
        &mut self,
        prepared: GeneratedAffineResidualGroupPreparedExactRowCommit,
    ) -> GeneratedAffineResidualGroupPreparedExactRowOutcome {
        assert!(
            self.prepared_commit_invariants_hold(&prepared),
            "prepared exact database commit invariant violated"
        );
        self.commit_prepared_staged_row_move_inner(prepared)
    }

    fn commit_prepared_staged_row_move_inner(
        &mut self,
        prepared: GeneratedAffineResidualGroupPreparedExactRowCommit,
    ) -> GeneratedAffineResidualGroupPreparedExactRowOutcome {
        let GeneratedAffineResidualGroupPreparedExactRowCommit {
            predecessor_transition_identity: _,
            staged,
            source_recipe: _,
            evidence,
        } = prepared;
        match evidence {
            PreparedExactRowEvidence::Dependent(evidence) => {
                let source_ordinal = self.commit_staged_dependent_move_inner(staged);
                GeneratedAffineResidualGroupPreparedExactRowOutcome::Dependent {
                    source_ordinal,
                    evidence,
                }
            }
            PreparedExactRowEvidence::NewPivot(evidence) => {
                let (source_ordinal, pivot_ordinal) =
                    self.commit_staged_new_pivot_move_inner(staged);
                GeneratedAffineResidualGroupPreparedExactRowOutcome::NewPivot {
                    source_ordinal,
                    pivot_ordinal,
                    evidence,
                }
            }
        }
    }

    fn commit_staged_dependent_move_inner(
        &mut self,
        staged: GeneratedAffineResidualGroupStagedExactRow,
    ) -> usize {
        let GeneratedAffineResidualGroupStagedExactRow {
            next_transition_identity,
            next_state_version,
            source_ordinal,
            next_source_ordinal,
            payload,
            ..
        } = staged;
        let ExactStagedRowPayload::Dependent {
            reductions,
            committed_stats,
        } = payload
        else {
            unreachable!("prepared dependent evidence changed staged outcome")
        };
        self.stats = committed_stats;
        self.next_source_ordinal = next_source_ordinal;
        self.state_version = next_state_version;
        self.transition_identity = next_transition_identity;
        drop(reductions);
        source_ordinal
    }

    fn commit_staged_new_pivot_move_inner(
        &mut self,
        staged: GeneratedAffineResidualGroupStagedExactRow,
    ) -> (usize, usize) {
        let GeneratedAffineResidualGroupStagedExactRow {
            next_transition_identity,
            next_state_version,
            source_ordinal,
            next_source_ordinal,
            payload,
            ..
        } = staged;
        let ExactStagedRowPayload::NewPivot {
            pivot,
            pivot_key,
            lookup_insertion,
            successor_reducer,
            successor_catalog_easiest_first,
            mut committed_pivots,
            mut committed_lookup,
            committed_stats,
        } = payload
        else {
            unreachable!("prepared new-pivot evidence changed staged outcome")
        };
        let pivot_ordinal = pivot.ordinal;
        let mut prior_pivots = std::mem::take(&mut self.pivots);
        let mut prior_lookup = std::mem::take(&mut self.lookup);
        committed_pivots.append(&mut prior_pivots);
        committed_lookup.append(&mut prior_lookup);
        committed_lookup.insert(
            lookup_insertion,
            ExactPivotLookupEntry {
                key: pivot_key,
                pivot_ordinal,
            },
        );
        committed_pivots.push(pivot);
        self.pivots = committed_pivots;
        self.lookup = committed_lookup;
        self.symbolica_reducer = successor_reducer;
        if let Some(successor_catalog_easiest_first) = successor_catalog_easiest_first {
            self.symbolica_catalog_easiest_first = successor_catalog_easiest_first;
        }
        self.stats = committed_stats;
        self.next_source_ordinal = next_source_ordinal;
        self.state_version = next_state_version;
        self.transition_identity = next_transition_identity;
        (source_ordinal, pivot_ordinal)
    }

    /// Allocation-free local assertion predicate for the final prepared tail.
    /// This deliberately returns a boolean rather than a recoverable error:
    /// once an outer session starts its infallible commit tail, a violated
    /// sealed-token invariant must stop before the first database mutation.
    fn prepared_commit_invariants_hold(
        &self,
        prepared: &GeneratedAffineResidualGroupPreparedExactRowCommit,
    ) -> bool {
        let staged = &prepared.staged;
        if self.authenticate_source_profile().is_err()
            || self.database_nonce != staged.database_nonce
            || self.database_epoch != staged.database_epoch
            || self.group_ordinal != staged.group_ordinal
            || self.state_version != staged.state_version
            || self.next_source_ordinal != staged.source_ordinal
            || self.pivots.len() != staged.pivot_count
            || self.lookup.len() != staged.lookup_len
            || self.symbolica_catalog_easiest_first.len() != staged.catalog_len
            || self.pivots.len() != self.lookup.len()
            || !self.live_symbolica_shape_is_valid()
            || self.transition_identity != prepared.predecessor_transition_identity
            || staged.next_transition_identity == ExactDatabaseTransitionIdentity::PRISTINE
            || staged.next_transition_identity == prepared.predecessor_transition_identity
            || staged.source_ordinal.checked_add(1) != Some(staged.next_source_ordinal)
            || staged.state_version.checked_add(1) != Some(staged.next_state_version)
            || !staged.source.authenticates_own_allocation()
            || !prepared.source_recipe.source.authenticates_own_allocation()
            || !prepared
                .source_recipe
                .source
                .same_allocation(&staged.source)
            || !Arc::ptr_eq(&prepared.source_recipe.plan, &self.plan)
            || !Arc::ptr_eq(&prepared.source_recipe.frame, &self.frame)
            || prepared.source_recipe.database_epoch != self.database_epoch
            || prepared.source_recipe.group_ordinal != self.group_ordinal
        {
            return false;
        }

        match (&staged.payload, &prepared.evidence) {
            (
                ExactStagedRowPayload::Dependent { reductions, .. },
                PreparedExactRowEvidence::Dependent(evidence),
            ) => Arc::ptr_eq(reductions, &evidence.reductions),
            (
                ExactStagedRowPayload::NewPivot {
                    pivot,
                    pivot_key,
                    lookup_insertion,
                    successor_reducer,
                    successor_catalog_easiest_first,
                    committed_pivots,
                    committed_lookup,
                    ..
                },
                PreparedExactRowEvidence::NewPivot(evidence),
            ) => {
                let Some(requested_pivots) = self.pivots.len().checked_add(1) else {
                    return false;
                };
                let Some(requested_lookup) = self.lookup.len().checked_add(1) else {
                    return false;
                };
                Arc::ptr_eq(pivot, &evidence.pivot)
                    && pivot.ordinal == self.pivots.len()
                    && pivot.source_ordinal == staged.source_ordinal
                    && pivot.terms.last().map(|term| &term.key) == Some(pivot_key)
                    && committed_pivots.is_empty()
                    && committed_lookup.is_empty()
                    && committed_pivots.capacity() >= requested_pivots
                    && committed_lookup.capacity() >= requested_lookup
                    && self.staged_symbolica_successor_is_valid(
                        pivot,
                        successor_reducer,
                        successor_catalog_easiest_first.as_deref(),
                    )
                    && *lookup_insertion <= self.lookup.len()
                    && self
                        .lookup
                        .binary_search_by(|entry| entry.key.cmp(pivot_key))
                        == Err(*lookup_insertion)
            }
            _ => false,
        }
    }

    fn authenticate_staged_row(
        &self,
        staged: &GeneratedAffineResidualGroupStagedExactRow,
    ) -> Result<(), GeneratedAffineResidualGroupExactDatabaseError> {
        self.authenticate_source_profile()?;
        if !staged.source.authenticates_own_allocation() {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::InvalidStagedRow);
        }
        if self.database_nonce != staged.database_nonce {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::WrongDatabaseAllocation);
        }
        if self.database_epoch != staged.database_epoch {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::WrongDatabaseEpoch);
        }
        if self.group_ordinal != staged.group_ordinal {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::WrongGroup);
        }
        if self.pivots.len() != self.lookup.len() || !self.live_symbolica_shape_is_valid() {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::InvalidStagedRow);
        }
        if self.state_version != staged.state_version
            || self.pivots.len() != staged.pivot_count
            || self.lookup.len() != staged.lookup_len
            || self.symbolica_catalog_easiest_first.len() != staged.catalog_len
        {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::StaleStagedRow);
        }
        if self.next_source_ordinal != staged.source_ordinal {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::WrongSourceOrder);
        }
        if staged.next_transition_identity == ExactDatabaseTransitionIdentity::PRISTINE
            || staged.next_transition_identity == self.transition_identity
            || staged.next_source_ordinal
                != staged
                    .source_ordinal
                    .checked_add(1)
                    .ok_or(GeneratedAffineResidualGroupExactDatabaseError::InvalidStagedRow)?
            || staged.next_state_version
                != staged
                    .state_version
                    .checked_add(1)
                    .ok_or(GeneratedAffineResidualGroupExactDatabaseError::InvalidStagedRow)?
        {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::InvalidStagedRow);
        }
        if let ExactStagedRowPayload::NewPivot {
            pivot,
            pivot_key,
            lookup_insertion,
            successor_reducer,
            successor_catalog_easiest_first,
            committed_pivots,
            committed_lookup,
            ..
        } = &staged.payload
        {
            let requested_pivots = self
                .pivots
                .len()
                .checked_add(1)
                .ok_or(GeneratedAffineResidualGroupExactDatabaseError::InvalidStagedRow)?;
            let requested_lookup = self
                .lookup
                .len()
                .checked_add(1)
                .ok_or(GeneratedAffineResidualGroupExactDatabaseError::InvalidStagedRow)?;
            if pivot.ordinal != self.pivots.len()
                || pivot.source_ordinal != staged.source_ordinal
                || pivot.terms.last().map(|term| &term.key) != Some(pivot_key)
                || !committed_pivots.is_empty()
                || !committed_lookup.is_empty()
                || committed_pivots.capacity() < requested_pivots
                || committed_lookup.capacity() < requested_lookup
                || !self.staged_symbolica_successor_is_valid(
                    pivot,
                    successor_reducer,
                    successor_catalog_easiest_first.as_deref(),
                )
                || *lookup_insertion > self.lookup.len()
                || self
                    .lookup
                    .binary_search_by(|entry| entry.key.cmp(pivot_key))
                    != Err(*lookup_insertion)
            {
                return Err(GeneratedAffineResidualGroupExactDatabaseError::InvalidStagedRow);
            }
        }
        Ok(())
    }

    fn live_symbolica_shape_is_valid(&self) -> bool {
        self.symbolica_reducer.independent_rows() == self.pivots.len()
            && self.symbolica_reducer.physical_columns()
                == self.symbolica_catalog_easiest_first.len()
            && self
                .symbolica_catalog_easiest_first
                .windows(2)
                .all(|pair| pair[0] < pair[1])
    }

    fn staged_symbolica_successor_is_valid(
        &self,
        pivot: &ExactUnitPivot,
        successor: &SymbolicaPersistentSparseReducer,
        successor_catalog: Option<&[GeneratedAffineResidualGroupPhysicalKey]>,
    ) -> bool {
        let Some(expected_rows) = self.pivots.len().checked_add(1) else {
            return false;
        };
        if successor.independent_rows() != expected_rows
            || successor.context_fingerprint() != self.symbolica_reducer.context_fingerprint()
        {
            return false;
        }

        let missing_keys = pivot
            .terms
            .iter()
            .filter(|term| {
                self.symbolica_catalog_easiest_first
                    .binary_search(&term.key)
                    .is_err()
            })
            .count();
        let catalog = match (missing_keys, successor_catalog) {
            (0, None) => self.symbolica_catalog_easiest_first.as_slice(),
            (0, Some(_)) | (_, None) => return false,
            (_, Some(catalog)) => {
                if catalog.len()
                    != self
                        .symbolica_catalog_easiest_first
                        .len()
                        .checked_add(missing_keys)
                        .unwrap_or(usize::MAX)
                    || catalog.windows(2).any(|pair| pair[0] >= pair[1])
                    || self
                        .symbolica_catalog_easiest_first
                        .iter()
                        .any(|key| catalog.binary_search(key).is_err())
                    || pivot
                        .terms
                        .iter()
                        .any(|term| catalog.binary_search(&term.key).is_err())
                {
                    return false;
                }
                catalog
            }
        };
        let Some(pivot_key) = pivot.terms.last().map(|term| &term.key) else {
            return false;
        };
        let Ok(easiest_index) = catalog.binary_search(pivot_key) else {
            return false;
        };
        let Some(native_column) = easiest_index
            .checked_add(1)
            .and_then(|offset| catalog.len().checked_sub(offset))
        else {
            return false;
        };
        if successor.physical_columns() != catalog.len()
            || successor.pivot_row_for_physical_column(native_column) != Some(pivot.ordinal)
        {
            return false;
        }

        let expected_native_row = pivot.terms.iter().rev().filter_map(|term| {
            let easiest_index = catalog.binary_search(&term.key).ok()?;
            let native_column = easiest_index
                .checked_add(1)
                .and_then(|offset| catalog.len().checked_sub(offset))?;
            Some((native_column, &term.coefficient))
        });
        successor.normalized_u_row_matches(pivot.ordinal, pivot.terms.len(), expected_native_row)
    }

    fn lookup_ordinal(&self, key: &GeneratedAffineResidualGroupPhysicalKey) -> Option<usize> {
        let position = self
            .lookup
            .binary_search_by(|entry| entry.key.cmp(key))
            .ok()?;
        Some(self.lookup[position].pivot_ordinal)
    }

    fn stats_with_ingress(
        &self,
        prospective_retained_bytes: usize,
        observed_retained_bytes: usize,
    ) -> GeneratedAffineResidualGroupExactDatabaseStats {
        GeneratedAffineResidualGroupExactDatabaseStats {
            last_ingress_prospective_retained_bytes: prospective_retained_bytes,
            last_ingress_observed_retained_bytes: observed_retained_bytes,
            peak_ingress_retained_bytes: self
                .stats
                .peak_ingress_retained_bytes
                .max(observed_retained_bytes),
            ..self.stats
        }
    }

    fn stats_with_staged_live(
        &self,
        stats: GeneratedAffineResidualGroupExactDatabaseStats,
        prospective_retained_bytes: usize,
        observed_retained_bytes: usize,
    ) -> GeneratedAffineResidualGroupExactDatabaseStats {
        GeneratedAffineResidualGroupExactDatabaseStats {
            last_staged_live_prospective_retained_bytes: prospective_retained_bytes,
            last_staged_live_observed_retained_bytes: observed_retained_bytes,
            peak_staged_live_retained_bytes: self
                .stats
                .peak_staged_live_retained_bytes
                .max(observed_retained_bytes),
            ..stats
        }
    }

    fn stats_with_native_sparse_event(
        &self,
        stats: GeneratedAffineResidualGroupExactDatabaseStats,
        event: GeneratedAffineResidualGroupExactNativeSparseStageStats,
    ) -> GeneratedAffineResidualGroupExactDatabaseStats {
        GeneratedAffineResidualGroupExactDatabaseStats {
            native_sparse_scaling: stats.native_sparse_scaling.with_event(event),
            ..stats
        }
    }

    fn preflight_next_source_ordinal(
        &self,
    ) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
        self.next_source_ordinal
            .checked_add(1)
            .ok_or(GeneratedAffineResidualGroupExactDatabaseError::SourceOrderOverflow)
    }

    fn preflight_next_state_version(
        &self,
    ) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
        self.state_version
            .checked_add(1)
            .ok_or(GeneratedAffineResidualGroupExactDatabaseError::StateVersionOverflow)
    }

    /// Narrow session-capability adapter for sibling recentering tests that
    /// already hold authenticated physical keys, coefficients, and guards.
    /// It is absent from production builds and still cannot be called without
    /// the session module's unforgeable database capability.
    #[cfg(test)]
    pub(crate) fn stage_authenticated_terms_for_session(
        &self,
        _capability: &GeneratedAffineResidualGroupExactSessionDatabaseCapability,
        context: &ParametricCoefficientContext,
        terms: Vec<(
            GeneratedAffineResidualGroupPhysicalKey,
            ParametricCoefficient,
        )>,
        guards: Vec<ParametricNonZeroCondition>,
    ) -> Result<
        GeneratedAffineResidualGroupStagedExactRow,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            self.stage_test_terms(context, terms, guards)
        }))
        .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::SymbolicaPanic)?
    }

    #[cfg(test)]
    fn stage_test_terms(
        &self,
        context: &ParametricCoefficientContext,
        terms: Vec<(
            GeneratedAffineResidualGroupPhysicalKey,
            ParametricCoefficient,
        )>,
        guards: Vec<ParametricNonZeroCondition>,
    ) -> Result<
        GeneratedAffineResidualGroupStagedExactRow,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        let retained_bytes = synthetic_source_recipe_retained_bytes(&terms, &guards)?;
        let source = Arc::new(ExactSyntheticSourceRecipe {
            terms,
            guards,
            retained_bytes,
        });
        self.stage_synthetic_source_recipe(context, source)
    }

    #[cfg(test)]
    fn stage_synthetic_source_recipe(
        &self,
        context: &ParametricCoefficientContext,
        source: Arc<ExactSyntheticSourceRecipe>,
    ) -> Result<
        GeneratedAffineResidualGroupStagedExactRow,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        let next_source_ordinal = self.preflight_next_source_ordinal()?;
        let next_state_version = self.preflight_next_state_version()?;
        check_limit(
            "terms in one exact top-reduction row",
            source.terms.len(),
            self.limits.max_terms_per_row,
        )?;
        let mut ledger = ParametricCoefficientWorkLedger::new(
            ParametricCoefficientWorkPhase::Construction,
            self.limits.coefficient_work,
        );
        let ingress = preflight_borrowed_ingress(
            context,
            &self.frame,
            source
                .terms
                .iter()
                .map(|(key, coefficient)| (key, coefficient)),
            source.terms.len(),
            &source.guards,
            self.limits,
        )?;
        check_limit(
            "exact-group borrowed ingress prospective retained bytes",
            ingress.prospective_retained_bytes,
            self.limits.max_ingress_retained_bytes,
        )?;
        let mut retained = try_terms_with_capacity(source.terms.len())?;
        let mut retained_guards = try_guards_with_capacity(source.guards.len())?;
        let observed_ingress_retained_bytes =
            ingress.observed_retained_bytes(retained.capacity(), retained_guards.capacity())?;
        check_limit(
            "exact-group borrowed ingress observed retained bytes",
            observed_ingress_retained_bytes,
            self.limits.max_ingress_retained_bytes,
        )?;
        for (key, coefficient) in &source.terms {
            retained.push(ExactDatabaseTerm {
                key: key.clone(),
                coefficient: ledger
                    .try_copy_authenticated(coefficient)
                    .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?,
            });
        }
        retained_guards.extend(source.guards.iter().cloned());
        self.finish_stage(
            context,
            retained,
            retained_guards,
            ledger,
            next_source_ordinal,
            next_state_version,
            ingress.prospective_retained_bytes,
            observed_ingress_retained_bytes,
            ExactStagedSource::synthetic(source),
        )
    }

    #[cfg(test)]
    fn ingest_test_terms(
        &mut self,
        context: &ParametricCoefficientContext,
        terms: Vec<(
            GeneratedAffineResidualGroupPhysicalKey,
            ParametricCoefficient,
        )>,
        guards: Vec<ParametricNonZeroCondition>,
    ) -> Result<
        GeneratedAffineResidualGroupExactRowOutcome,
        GeneratedAffineResidualGroupExactDatabaseError,
    > {
        let staged = self.stage_test_terms(context, terms, guards)?;
        self.commit_staged_row_for_test(staged)
    }
}

fn map_symbolica_sparse_error(
    error: SymbolicaParametricSparseError,
) -> GeneratedAffineResidualGroupExactDatabaseError {
    match error {
        SymbolicaParametricSparseError::ResourceLimit {
            resource,
            requested,
            limit,
        } => GeneratedAffineResidualGroupExactDatabaseError::ResourceLimit {
            resource,
            requested,
            limit,
        },
        SymbolicaParametricSparseError::ResourceCountOverflow { resource } => {
            GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow { resource }
        }
        SymbolicaParametricSparseError::AllocationFailure { resource } => {
            GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure { resource }
        }
        SymbolicaParametricSparseError::Coefficient(_)
        | SymbolicaParametricSparseError::CoefficientWork(_) => {
            GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork
        }
        SymbolicaParametricSparseError::NativePanic { .. } => {
            GeneratedAffineResidualGroupExactDatabaseError::SymbolicaPanic
        }
        SymbolicaParametricSparseError::DimensionOverflow => {
            GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                resource: "Symbolica parametric sparse dimensions",
            }
        }
        SymbolicaParametricSparseError::EmptyPriorRow { .. }
        | SymbolicaParametricSparseError::ColumnOutOfRange { .. }
        | SymbolicaParametricSparseError::NonIncreasingColumns { .. }
        | SymbolicaParametricSparseError::DecreasingColumnInsertions { .. }
        | SymbolicaParametricSparseError::ColumnInsertionOutOfRange { .. }
        | SymbolicaParametricSparseError::MissingInsertedColumnCandidateEntry { .. }
        | SymbolicaParametricSparseError::ExplicitZero { .. }
        | SymbolicaParametricSparseError::DependentPriorRow { .. }
        | SymbolicaParametricSparseError::PriorRowReplayMismatch { .. }
        | SymbolicaParametricSparseError::UnexpectedFieldOperation { .. }
        | SymbolicaParametricSparseError::NativeTranscriptMismatch { .. }
        | SymbolicaParametricSparseError::NewColumnDependentCandidate => {
            GeneratedAffineResidualGroupExactDatabaseError::SymbolicaTranscriptMismatch
        }
    }
}

/// Fork the retained Symbolica basis and ask it for one authoritative forward
/// transcript. The persistent physical catalog follows RustRed's canonical
/// easiest-to-hardest order, while native columns use the reverse order so
/// Symbolica's leftmost pivot is RustRed's hardest term. New keys become
/// ordered native-column insertions; only an independent outcome may carry the
/// resulting catalog and reducer successor.
fn symbolica_sparse_transcript(
    context: &ParametricCoefficientContext,
    reducer: &SymbolicaPersistentSparseReducer,
    catalog_easiest_first: &[GeneratedAffineResidualGroupPhysicalKey],
    candidate: &[ExactDatabaseTerm],
    limits: SymbolicaPersistentSparseLimits,
) -> Result<ExactDatabaseSymbolicaTranscript, GeneratedAffineResidualGroupExactDatabaseError> {
    if reducer.context_fingerprint() != context.fingerprint()
        || reducer.physical_columns() != catalog_easiest_first.len()
        || catalog_easiest_first
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(GeneratedAffineResidualGroupExactDatabaseError::SymbolicaTranscriptMismatch);
    }
    check_limit(
        "persistent Symbolica sparse candidate input entries",
        candidate.len(),
        limits.max_candidate_input_entries,
    )?;

    let mut new_column_count = 0usize;
    for term in candidate {
        if catalog_easiest_first.binary_search(&term.key).is_err() {
            new_column_count = new_column_count.checked_add(1).ok_or(
                GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                    resource: "persistent Symbolica sparse inserted columns",
                },
            )?;
        }
    }
    check_limit(
        "persistent Symbolica sparse inserted columns",
        new_column_count,
        limits.max_new_columns,
    )?;
    let physical_columns_after = catalog_easiest_first
        .len()
        .checked_add(new_column_count)
        .ok_or(
            GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                resource: "persistent Symbolica sparse physical columns after insertion",
            },
        )?;
    check_limit(
        "persistent Symbolica sparse physical columns after insertion",
        physical_columns_after,
        limits.max_physical_columns_after,
    )?;

    let mut insertions = Vec::new();
    insertions
        .try_reserve_exact(new_column_count)
        .map_err(
            |_| GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
                resource: "Symbolica exact-group persistent column insertions",
            },
        )?;
    let mut new_keys_easiest_first = Vec::new();
    new_keys_easiest_first
        .try_reserve_exact(new_column_count)
        .map_err(
            |_| GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
                resource: "Symbolica exact-group new physical keys",
            },
        )?;
    for term in candidate {
        if catalog_easiest_first.binary_search(&term.key).is_err() {
            new_keys_easiest_first.push(&term.key);
        }
    }
    for key in new_keys_easiest_first.iter().rev() {
        let easiest_insertion = catalog_easiest_first.partition_point(|old| old < *key);
        insertions.push(catalog_easiest_first.len() - easiest_insertion);
    }

    let successor_catalog_easiest_first = if new_keys_easiest_first.is_empty() {
        None
    } else {
        let mut merged = Vec::new();
        merged
            .try_reserve_exact(physical_columns_after)
            .map_err(
                |_| GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
                    resource: "Symbolica exact-group successor physical-key catalog",
                },
            )?;
        let mut old_ordinal = 0usize;
        let mut new_ordinal = 0usize;
        while old_ordinal < catalog_easiest_first.len()
            && new_ordinal < new_keys_easiest_first.len()
        {
            if catalog_easiest_first[old_ordinal] < *new_keys_easiest_first[new_ordinal] {
                merged.push(catalog_easiest_first[old_ordinal].clone());
                old_ordinal += 1;
            } else {
                merged.push(new_keys_easiest_first[new_ordinal].clone());
                new_ordinal += 1;
            }
        }
        merged.extend(catalog_easiest_first[old_ordinal..].iter().cloned());
        merged.extend(
            new_keys_easiest_first[new_ordinal..]
                .iter()
                .map(|key| (*key).clone()),
        );
        if merged.len() != physical_columns_after
            || merged.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(
                GeneratedAffineResidualGroupExactDatabaseError::SymbolicaTranscriptMismatch,
            );
        }
        Some(merged)
    };
    let catalog_after = successor_catalog_easiest_first
        .as_deref()
        .unwrap_or(catalog_easiest_first);

    let mut candidate_entries = Vec::new();
    candidate_entries
        .try_reserve_exact(candidate.len())
        .map_err(
            |_| GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
                resource: "Symbolica exact-group candidate sparse-row entries",
            },
        )?;
    for term in candidate.iter().rev() {
        let easiest_index = catalog_after.binary_search(&term.key).map_err(|_| {
            GeneratedAffineResidualGroupExactDatabaseError::SymbolicaTranscriptMismatch
        })?;
        let column = catalog_after
            .len()
            .checked_sub(easiest_index.checked_add(1).ok_or(
                GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                    resource: "Symbolica exact-group native candidate column",
                },
            )?)
            .ok_or(GeneratedAffineResidualGroupExactDatabaseError::SymbolicaTranscriptMismatch)?;
        candidate_entries.push(SymbolicaParametricSparseInputEntry::new(
            column,
            &term.coefficient,
        ));
    }
    let candidate_row = SymbolicaParametricSparseInputRow::new(candidate_entries);

    let outcome = reducer
        .try_stage_row(&insertions, &candidate_row, limits)
        .map_err(map_symbolica_sparse_error)?;
    let mut normalized_keys_hardest_first = Vec::new();
    if let SymbolicaPersistentSparseOutcome::Independent {
        successor,
        pivot_column,
        normalized_row,
        ..
    } = &outcome
    {
        if successor.physical_columns() != catalog_after.len()
            || successor.independent_rows() != reducer.independent_rows().saturating_add(1)
            || successor.context_fingerprint() != reducer.context_fingerprint()
        {
            return Err(
                GeneratedAffineResidualGroupExactDatabaseError::SymbolicaTranscriptMismatch,
            );
        }
        normalized_keys_hardest_first
            .try_reserve_exact(normalized_row.entries().len())
            .map_err(
                |_| GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
                    resource: "Symbolica exact-group normalized physical-key transcript",
                },
            )?;
        for entry in normalized_row.entries() {
            let easiest_index = catalog_after
                .len()
                .checked_sub(entry.column().checked_add(1).ok_or(
                    GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                        resource: "Symbolica exact-group normalized native column",
                    },
                )?)
                .ok_or(
                    GeneratedAffineResidualGroupExactDatabaseError::SymbolicaTranscriptMismatch,
                )?;
            let key = catalog_after.get(easiest_index).ok_or(
                GeneratedAffineResidualGroupExactDatabaseError::SymbolicaTranscriptMismatch,
            )?;
            normalized_keys_hardest_first.push(key.clone());
        }
        let pivot_easiest_index = catalog_after
            .len()
            .checked_sub(pivot_column.checked_add(1).ok_or(
                GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                    resource: "Symbolica exact-group native pivot column",
                },
            )?)
            .ok_or(GeneratedAffineResidualGroupExactDatabaseError::SymbolicaTranscriptMismatch)?;
        if normalized_keys_hardest_first.first() != catalog_after.get(pivot_easiest_index) {
            return Err(
                GeneratedAffineResidualGroupExactDatabaseError::SymbolicaTranscriptMismatch,
            );
        }
    } else if successor_catalog_easiest_first.is_some() {
        return Err(GeneratedAffineResidualGroupExactDatabaseError::SymbolicaTranscriptMismatch);
    }
    Ok(ExactDatabaseSymbolicaTranscript {
        outcome,
        normalized_keys_hardest_first,
        successor_catalog_easiest_first,
    })
}

/// Test-only glue oracle retaining the former rebuild-every-stage path. It is
/// deliberately absent from production; integration tests compare only its
/// algebraic transcript with the persistent path, never its reconstruction
/// telemetry.
#[cfg(test)]
struct ExactDatabaseRebuildingSymbolicaTranscript {
    outcome: SymbolicaParametricSparseOutcome,
    normalized_keys_hardest_first: Vec<GeneratedAffineResidualGroupPhysicalKey>,
}

#[cfg(test)]
fn rebuilding_symbolica_sparse_transcript(
    context: &ParametricCoefficientContext,
    pivots: &[Arc<ExactUnitPivot>],
    candidate: &[ExactDatabaseTerm],
) -> Result<
    ExactDatabaseRebuildingSymbolicaTranscript,
    GeneratedAffineResidualGroupExactDatabaseError,
> {
    let mut catalog: Vec<&GeneratedAffineResidualGroupPhysicalKey> = pivots
        .iter()
        .flat_map(|pivot| pivot.terms.iter().map(|term| &term.key))
        .chain(candidate.iter().map(|term| &term.key))
        .collect();
    catalog.sort_unstable();
    catalog.dedup();

    let mut prior_rows = Vec::with_capacity(pivots.len());
    for pivot in pivots {
        let mut entries = Vec::with_capacity(pivot.terms.len());
        for term in pivot.terms.iter().rev() {
            let easiest_index = catalog
                .binary_search_by(|key| (*key).cmp(&term.key))
                .map_err(|_| {
                    GeneratedAffineResidualGroupExactDatabaseError::SymbolicaTranscriptMismatch
                })?;
            let column = catalog.len() - 1 - easiest_index;
            entries.push(SymbolicaParametricSparseInputEntry::new(
                column,
                &term.coefficient,
            ));
        }
        prior_rows.push(SymbolicaParametricSparseInputRow::new(entries));
    }

    let mut candidate_entries = Vec::with_capacity(candidate.len());
    for term in candidate.iter().rev() {
        let easiest_index = catalog
            .binary_search_by(|key| (*key).cmp(&term.key))
            .map_err(|_| {
                GeneratedAffineResidualGroupExactDatabaseError::SymbolicaTranscriptMismatch
            })?;
        candidate_entries.push(SymbolicaParametricSparseInputEntry::new(
            catalog.len() - 1 - easiest_index,
            &term.coefficient,
        ));
    }
    let candidate_row = SymbolicaParametricSparseInputRow::new(candidate_entries);
    let outcome = forward_reduce_last_row(
        context,
        catalog.len(),
        &prior_rows,
        &candidate_row,
        SymbolicaParametricSparseLimits::default(),
    )
    .map_err(map_symbolica_sparse_error)?;
    let mut normalized_keys_hardest_first = Vec::new();
    if let SymbolicaParametricSparseOutcome::Independent {
        pivot_column,
        normalized_row,
        ..
    } = &outcome
    {
        normalized_keys_hardest_first.reserve(normalized_row.entries().len());
        for entry in normalized_row.entries() {
            let easiest_index = catalog.len() - 1 - entry.column();
            normalized_keys_hardest_first.push(
                (*catalog.get(easiest_index).ok_or(
                    GeneratedAffineResidualGroupExactDatabaseError::SymbolicaTranscriptMismatch,
                )?)
                .clone(),
            );
        }
        let pivot_easiest_index = catalog.len() - 1 - pivot_column;
        if normalized_keys_hardest_first.first() != catalog.get(pivot_easiest_index).copied() {
            return Err(
                GeneratedAffineResidualGroupExactDatabaseError::SymbolicaTranscriptMismatch,
            );
        }
    }
    Ok(ExactDatabaseRebuildingSymbolicaTranscript {
        outcome,
        normalized_keys_hardest_first,
    })
}

fn normalize_unknown_leader(
    context: &ParametricCoefficientContext,
    ledger: &mut ParametricCoefficientWorkLedger,
    terms: &mut [ExactDatabaseTerm],
    guards: &mut Vec<ParametricNonZeroCondition>,
    group_ordinal: usize,
    database_epoch: usize,
    source_ordinal: usize,
    limits: GeneratedAffineResidualGroupExactDatabaseLimits,
) -> Result<ParametricCoefficient, GeneratedAffineResidualGroupExactDatabaseError> {
    let divisor = ledger
        .try_copy_authenticated(
            &terms
                .last()
                .ok_or(GeneratedAffineResidualGroupExactDatabaseError::InvalidUnitPivot)?
                .coefficient,
        )
        .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?;
    let one = ledger
        .try_one(context)
        .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?;
    let pending = ledger
        .try_guarded_division_pending(context, &one, &divisor)
        .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?;
    let inverse = ledger
        .try_finish_guarded_division(context, pending)
        .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?;
    merge_guards(
        context,
        guards,
        &inverse.nonzero,
        limits.max_guards_per_row,
        limits.max_guard_origins,
        limits.coefficient_work.arithmetic.max_guard_origins,
    )?;
    let leader_ordinal = terms
        .len()
        .checked_sub(1)
        .ok_or(GeneratedAffineResidualGroupExactDatabaseError::InvalidUnitPivot)?;
    for (term_ordinal, term) in terms.iter_mut().enumerate() {
        term.coefficient = ledger
            .try_mul(context, &term.coefficient, &inverse.value)
            .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?;
        insert_denominator_guard(
            context,
            ledger,
            guards,
            &term.coefficient,
            GuardOrigin::GeneratedAffineGroupTopReductionCoefficientDenominator {
                solve_group_ordinal: group_ordinal,
                database_epoch,
                event_ordinal: source_ordinal,
                operation_ordinal: 0,
                term_ordinal,
                pivot_normalization: term_ordinal == leader_ordinal,
            },
            limits,
        )?;
    }
    if terms.last().map(|term| &term.coefficient) != Some(&one) {
        return Err(GeneratedAffineResidualGroupExactDatabaseError::InvalidUnitPivot);
    }
    let pivot = &terms
        .last()
        .ok_or(GeneratedAffineResidualGroupExactDatabaseError::InvalidUnitPivot)?
        .key;
    if terms[..terms.len().saturating_sub(1)]
        .iter()
        .any(|term| term.key >= *pivot)
    {
        return Err(GeneratedAffineResidualGroupExactDatabaseError::InvalidUnitPivot);
    }
    Ok(divisor)
}

fn add_sorted_term(
    context: &ParametricCoefficientContext,
    ledger: &mut ParametricCoefficientWorkLedger,
    terms: &mut Vec<ExactDatabaseTerm>,
    key: GeneratedAffineResidualGroupPhysicalKey,
    coefficient: ParametricCoefficient,
    max_terms: usize,
) -> Result<(), GeneratedAffineResidualGroupExactDatabaseError> {
    if coefficient.is_zero() {
        return Ok(());
    }
    match terms.binary_search_by(|term| term.key.cmp(&key)) {
        Ok(position) => {
            let sum = ledger
                .try_add(context, &terms[position].coefficient, &coefficient)
                .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?;
            if sum.is_zero() {
                terms.remove(position);
            } else {
                terms[position].coefficient = sum;
            }
        }
        Err(position) => {
            let requested = terms.len().checked_add(1).ok_or(
                GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                    resource: "terms in one exact top-reduction row",
                },
            )?;
            check_limit("terms in one exact top-reduction row", requested, max_terms)?;
            terms.try_reserve_exact(1).map_err(|_| {
                GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
                    resource: "exact top-reduction row terms",
                }
            })?;
            terms.insert(position, ExactDatabaseTerm { key, coefficient });
        }
    }
    Ok(())
}

fn preflight_borrowed_ingress<'a>(
    context: &ParametricCoefficientContext,
    frame: &GeneratedAffineResidualGroupPhysicalFrame,
    terms: impl IntoIterator<
        Item = (
            &'a GeneratedAffineResidualGroupPhysicalKey,
            &'a ParametricCoefficient,
        ),
    >,
    expected_terms: usize,
    guards: &[ParametricNonZeroCondition],
    limits: GeneratedAffineResidualGroupExactDatabaseLimits,
) -> Result<BorrowedIngressRetainedCensus, GeneratedAffineResidualGroupExactDatabaseError> {
    check_limit(
        "terms in one exact top-reduction row",
        expected_terms,
        limits.max_terms_per_row,
    )?;
    check_limit(
        "guards in one exact top-reduction row",
        guards.len(),
        limits.max_guards_per_row,
    )?;

    let resource = "exact-group borrowed ingress retained bytes";
    let mut deep_payload_bytes = 0usize;
    let mut observed_terms = 0usize;
    let mut previous_key: Option<&GeneratedAffineResidualGroupPhysicalKey> = None;
    for (key, coefficient) in terms {
        observed_terms = checked_add(resource, observed_terms, 1)?;
        if coefficient.is_zero() || previous_key.is_some_and(|previous| previous >= key) {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::InvalidTermOrder);
        }
        context
            .validate_with_limits(
                coefficient,
                limits.coefficient_work.arithmetic.exact_algebra,
            )
            .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?;
        frame
            .replay_key(key)
            .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::PhysicalKey)?;
        deep_payload_bytes = checked_add(resource, deep_payload_bytes, key.retained_bytes())?;
        deep_payload_bytes = checked_add(
            resource,
            deep_payload_bytes,
            coefficient.owned_retained_byte_bound().ok_or(
                GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow { resource },
            )?,
        )?;
        previous_key = Some(key);
    }
    if observed_terms != expected_terms || observed_terms == 0 {
        return Err(GeneratedAffineResidualGroupExactDatabaseError::InvalidTermOrder);
    }

    let mut aggregate_origins = 0usize;
    for guard in guards {
        if guard.origins().is_empty() {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork);
        }
        context
            .validate_polynomial_with_limits(
                guard.polynomial(),
                limits.coefficient_work.arithmetic.exact_algebra,
            )
            .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?;
        check_limit(
            "guard origins in one exact top-reduction condition",
            guard.origins().len(),
            limits.coefficient_work.arithmetic.max_guard_origins,
        )?;
        aggregate_origins = checked_add(
            "guard origins in one exact top-reduction row",
            aggregate_origins,
            guard.origins().len(),
        )?;
        deep_payload_bytes = checked_add(
            resource,
            deep_payload_bytes,
            guard.owned_retained_byte_bound().ok_or(
                GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow { resource },
            )?,
        )?;
    }
    check_limit(
        "guard origins in one exact top-reduction row",
        aggregate_origins,
        limits.max_guard_origins,
    )?;
    let prospective_retained_bytes =
        ingress_retained_bytes(expected_terms, guards.len(), deep_payload_bytes)?;
    Ok(BorrowedIngressRetainedCensus {
        terms: expected_terms,
        guards: guards.len(),
        deep_payload_bytes,
        prospective_retained_bytes,
    })
}

fn ingress_retained_bytes(
    term_capacity: usize,
    guard_capacity: usize,
    deep_payload_bytes: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
    const RESOURCE: &str = "exact-group borrowed ingress retained bytes";
    checked_sum(
        RESOURCE,
        [
            size_of::<Vec<ExactDatabaseTerm>>(),
            size_of::<Vec<ParametricNonZeroCondition>>(),
            checked_mul(RESOURCE, term_capacity, size_of::<ExactDatabaseTerm>())?,
            checked_mul(
                RESOURCE,
                guard_capacity,
                size_of::<ParametricNonZeroCondition>(),
            )?,
            deep_payload_bytes,
        ],
    )
}

fn copy_guards(
    context: &ParametricCoefficientContext,
    source: &[ParametricNonZeroCondition],
    max_guards: usize,
    max_aggregate_origins: usize,
    max_origins_per_condition: usize,
) -> Result<Vec<ParametricNonZeroCondition>, GeneratedAffineResidualGroupExactDatabaseError> {
    check_limit(
        "guards in one exact top-reduction row",
        source.len(),
        max_guards,
    )?;
    let origins = source.iter().try_fold(0usize, |total, guard| {
        if !context.contains_nonzero_condition(guard) {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork);
        }
        check_limit(
            "guard origins in one exact top-reduction condition",
            guard.origins().len(),
            max_origins_per_condition,
        )?;
        total.checked_add(guard.origins().len()).ok_or(
            GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                resource: "guard origins in one exact top-reduction row",
            },
        )
    })?;
    check_limit(
        "guard origins in one exact top-reduction row",
        origins,
        max_aggregate_origins,
    )?;
    // The exact physical-row compiler already authenticated and bounded these
    // payloads. V1 reserves the outer vector fallibly before cloning; a fully
    // fallible inner condition clone remains a later safety hardening seam.
    let mut guards = Vec::new();
    guards.try_reserve_exact(source.len()).map_err(|_| {
        GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
            resource: "exact top-reduction row guards",
        }
    })?;
    guards.extend(source.iter().cloned());
    Ok(guards)
}

fn merge_guards(
    context: &ParametricCoefficientContext,
    target: &mut Vec<ParametricNonZeroCondition>,
    source: &[ParametricNonZeroCondition],
    max_guards: usize,
    max_aggregate_origins: usize,
    max_origins_per_condition: usize,
) -> Result<(), GeneratedAffineResidualGroupExactDatabaseError> {
    let mut trial = copy_guards(
        context,
        target,
        max_guards,
        max_aggregate_origins,
        max_origins_per_condition,
    )?;
    trial.try_reserve_exact(source.len()).map_err(|_| {
        GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
            resource: "exact top-reduction row guards",
        }
    })?;
    for guard in source {
        if !context.contains_nonzero_condition(guard) {
            return Err(GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork);
        }
        insert_parametric_condition(&mut trial, guard.clone(), max_origins_per_condition)
            .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?;
        check_guard_occurrence_limits(
            &trial,
            max_guards,
            max_aggregate_origins,
            max_origins_per_condition,
        )?;
    }
    *target = trial;
    Ok(())
}

fn insert_denominator_guard(
    context: &ParametricCoefficientContext,
    ledger: &mut ParametricCoefficientWorkLedger,
    guards: &mut Vec<ParametricNonZeroCondition>,
    coefficient: &ParametricCoefficient,
    origin: GuardOrigin,
    limits: GeneratedAffineResidualGroupExactDatabaseLimits,
) -> Result<(), GeneratedAffineResidualGroupExactDatabaseError> {
    let per_condition = limits.coefficient_work.arithmetic.max_guard_origins;
    let mut trial = copy_guards(
        context,
        guards,
        limits.max_guards_per_row,
        limits.max_guard_origins,
        per_condition,
    )?;
    ledger
        .try_insert_denominator_guard(context, &mut trial, coefficient, origin)
        .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::CoefficientWork)?;
    check_guard_occurrence_limits(
        &trial,
        limits.max_guards_per_row,
        limits.max_guard_origins,
        per_condition,
    )?;
    *guards = trial;
    Ok(())
}

fn check_guard_occurrence_limits(
    guards: &[ParametricNonZeroCondition],
    max_guards: usize,
    max_aggregate_origins: usize,
    max_origins_per_condition: usize,
) -> Result<(), GeneratedAffineResidualGroupExactDatabaseError> {
    check_limit(
        "guards in one exact top-reduction row",
        guards.len(),
        max_guards,
    )?;
    let aggregate = guards.iter().try_fold(0usize, |total, guard| {
        check_limit(
            "guard origins in one exact top-reduction condition",
            guard.origins().len(),
            max_origins_per_condition,
        )?;
        total.checked_add(guard.origins().len()).ok_or(
            GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                resource: "guard origins in one exact top-reduction row",
            },
        )
    })?;
    check_limit(
        "guard origins in one exact top-reduction row",
        aggregate,
        max_aggregate_origins,
    )
}

fn exact_unit_pivot_retained_bytes(
    pivot: &Arc<ExactUnitPivot>,
) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
    pivot_retained_bytes(
        &pivot.terms,
        &pivot.guards,
        pivot.reductions.as_ref(),
        &pivot.normalization_divisor,
        true,
    )
}

fn exact_unit_pivot_owner_retained_bytes(
    pivot: &Arc<ExactUnitPivot>,
) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
    exact_unit_pivot_retained_bytes(pivot)?
        .checked_sub(size_of::<ExactPivotLookupEntry>())
        .ok_or(
            GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                resource: "exact-group pivot retained bytes",
            },
        )
}

fn shared_reduction_trace_retained_bytes(
    reductions: &Arc<Vec<GeneratedAffineResidualGroupExactReductionStep>>,
) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
    shared_reduction_trace_retained_bytes_with_slots(reductions.as_slice(), reductions.capacity())
}

fn shared_reduction_trace_retained_bytes_with_slots(
    reductions: &[GeneratedAffineResidualGroupExactReductionStep],
    reduction_slots: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
    const RESOURCE: &str = "exact shared reduction-trace retained bytes";
    if reduction_slots < reductions.len() {
        return Err(
            GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
                resource: "shared exact reduction trace",
            },
        );
    }
    let mut bytes = checked_sum(
        RESOURCE,
        [
            checked_mul(RESOURCE, 2, size_of::<usize>())?,
            size_of::<Vec<GeneratedAffineResidualGroupExactReductionStep>>(),
            checked_mul(
                RESOURCE,
                reduction_slots,
                size_of::<GeneratedAffineResidualGroupExactReductionStep>(),
            )?,
        ],
    )?;
    for reduction in reductions {
        bytes = checked_add(
            RESOURCE,
            bytes,
            reduction.factor.owned_retained_byte_bound().ok_or(
                GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                    resource: RESOURCE,
                },
            )?,
        )?;
    }
    Ok(bytes)
}

#[cfg(test)]
fn synthetic_source_recipe_retained_bytes(
    terms: &Vec<(
        GeneratedAffineResidualGroupPhysicalKey,
        ParametricCoefficient,
    )>,
    guards: &Vec<ParametricNonZeroCondition>,
) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
    const RESOURCE: &str = "synthetic exact source-recipe retained bytes";
    let mut bytes = checked_sum(
        RESOURCE,
        [
            checked_mul(RESOURCE, 2, size_of::<usize>())?,
            size_of::<ExactSyntheticSourceRecipe>(),
            checked_mul(
                RESOURCE,
                terms.capacity(),
                size_of::<(
                    GeneratedAffineResidualGroupPhysicalKey,
                    ParametricCoefficient,
                )>(),
            )?,
            checked_mul(
                RESOURCE,
                guards.capacity(),
                size_of::<ParametricNonZeroCondition>(),
            )?,
        ],
    )?;
    for (key, coefficient) in terms {
        bytes = checked_add(RESOURCE, bytes, key.retained_bytes())?;
        bytes = checked_add(
            RESOURCE,
            bytes,
            coefficient.owned_retained_byte_bound().ok_or(
                GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                    resource: RESOURCE,
                },
            )?,
        )?;
    }
    for guard in guards {
        bytes = checked_add(
            RESOURCE,
            bytes,
            guard.owned_retained_byte_bound().ok_or(
                GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                    resource: RESOURCE,
                },
            )?,
        )?;
    }
    Ok(bytes)
}

fn exact_unit_pivot_arc_allocation_bytes()
-> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
    checked_sum(
        "exact-group pivot retained bytes",
        [
            checked_mul("exact-group pivot retained bytes", 2, size_of::<usize>())?,
            size_of::<ExactUnitPivot>(),
        ],
    )
}

/// Retained coexistence of the live database and a sealed dependent stage.
/// The token's inline size charges all handles. Its source argument is the
/// deduplicated complete source-pipeline graph (excluding shared plan/frame
/// ancestry); the reduction vector and coefficient payload are the remaining
/// additional unique owners.
fn dependent_staged_live_retained_bytes(
    database_retained_bytes: usize,
    reductions: &[GeneratedAffineResidualGroupExactReductionStep],
    reduction_slots: usize,
    source_unique_retained_bytes: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
    const RESOURCE: &str = "exact-group staged live retained bytes";
    if reduction_slots < reductions.len() {
        return Err(
            GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
                resource: "staged dependent reduction trace",
            },
        );
    }
    checked_sum(
        RESOURCE,
        [
            database_retained_bytes,
            size_of::<GeneratedAffineResidualGroupStagedExactRow>(),
            source_unique_retained_bytes,
            shared_reduction_trace_retained_bytes_with_slots(reductions, reduction_slots)?,
        ],
    )
}

/// Retained coexistence of the live database and a sealed new-pivot stage.
/// The candidate's inline pivot/key/handles live inside the token. Its deep
/// payload, the deduplicated unique source graph, and both still-empty
/// replacement allocations are charged here.
fn new_pivot_staged_live_retained_bytes(
    database_retained_bytes: usize,
    pivot: &Arc<ExactUnitPivot>,
    pivot_replacement_slots: usize,
    lookup_replacement_slots: usize,
    catalog_replacement_slots: usize,
    source_unique_retained_bytes: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
    const RESOURCE: &str = "exact-group staged live retained bytes";
    checked_sum(
        RESOURCE,
        [
            database_retained_bytes,
            size_of::<GeneratedAffineResidualGroupStagedExactRow>(),
            source_unique_retained_bytes,
            exact_unit_pivot_owner_retained_bytes(pivot)?,
            checked_mul(
                RESOURCE,
                pivot_replacement_slots,
                size_of::<Arc<ExactUnitPivot>>(),
            )?,
            checked_mul(
                RESOURCE,
                lookup_replacement_slots,
                size_of::<ExactPivotLookupEntry>(),
            )?,
            checked_mul(
                RESOURCE,
                catalog_replacement_slots,
                size_of::<GeneratedAffineResidualGroupPhysicalKey>(),
            )?,
        ],
    )
}

/// Rust-visible persistent database ownership under explicit outer-vector
/// capacities. Deep lookup/catalog-key payload is excluded because those keys
/// shallow-clone payload already charged by pivot terms. Symbolica's opaque
/// reducer heap is bounded separately by exact native-entry limits; its private
/// scratch capacities are not exposed by the public API and are not claimed by
/// this byte census.
fn database_retained_bytes_with_candidate(
    pivots: &[Arc<ExactUnitPivot>],
    pivot_capacity: usize,
    lookup_capacity: usize,
    catalog_capacity: usize,
    candidate: Option<&Arc<ExactUnitPivot>>,
) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
    const RESOURCE: &str = "exact-group database retained bytes";
    let required = checked_add(RESOURCE, pivots.len(), usize::from(candidate.is_some()))?;
    if pivot_capacity < required || lookup_capacity < required {
        return Err(
            GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
                resource: "exact-group database replacement vectors",
            },
        );
    }
    let mut bytes = checked_sum(
        RESOURCE,
        [
            size_of::<GeneratedAffineResidualGroupExactDatabase>(),
            checked_mul(RESOURCE, pivot_capacity, size_of::<Arc<ExactUnitPivot>>())?,
            checked_mul(
                RESOURCE,
                lookup_capacity,
                size_of::<ExactPivotLookupEntry>(),
            )?,
            checked_mul(
                RESOURCE,
                catalog_capacity,
                size_of::<GeneratedAffineResidualGroupPhysicalKey>(),
            )?,
        ],
    )?;
    for pivot in pivots.iter().chain(candidate) {
        bytes = checked_add(
            RESOURCE,
            bytes,
            exact_unit_pivot_owner_retained_bytes(pivot)?,
        )?;
    }
    Ok(bytes)
}

/// Conservative charged ownership of one pivot and its sorted lookup entry.
///
/// The lookup key is a shallow `Arc` clone of the pivot leader. Its inline
/// entry is charged here, while the shared deep key payload is charged exactly
/// once through `terms`. `observed_capacity` selects actual staged vector
/// capacities; the prospective pass uses exact logical lengths.
fn pivot_retained_bytes(
    terms: &Vec<ExactDatabaseTerm>,
    guards: &Vec<ParametricNonZeroCondition>,
    reductions: &Vec<GeneratedAffineResidualGroupExactReductionStep>,
    normalization_divisor: &ParametricCoefficient,
    observed_capacity: bool,
) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
    const RESOURCE: &str = "exact-group pivot retained bytes";
    let term_slots = if observed_capacity {
        terms.capacity()
    } else {
        terms.len()
    };
    let guard_slots = if observed_capacity {
        guards.capacity()
    } else {
        guards.len()
    };
    let reduction_slots = if observed_capacity {
        reductions.capacity()
    } else {
        reductions.len()
    };
    let mut bytes = checked_sum(
        RESOURCE,
        [
            exact_unit_pivot_arc_allocation_bytes()?,
            size_of::<ExactPivotLookupEntry>(),
            checked_mul(RESOURCE, term_slots, size_of::<ExactDatabaseTerm>())?,
            checked_mul(
                RESOURCE,
                guard_slots,
                size_of::<ParametricNonZeroCondition>(),
            )?,
            checked_mul(
                RESOURCE,
                reduction_slots,
                size_of::<GeneratedAffineResidualGroupExactReductionStep>(),
            )?,
            checked_mul(RESOURCE, 2, size_of::<usize>())?,
            size_of::<Vec<GeneratedAffineResidualGroupExactReductionStep>>(),
            normalization_divisor.owned_retained_byte_bound().ok_or(
                GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                    resource: RESOURCE,
                },
            )?,
        ],
    )?;
    for term in terms {
        bytes = checked_add(RESOURCE, bytes, term.key.retained_bytes())?;
        bytes = checked_add(
            RESOURCE,
            bytes,
            term.coefficient.owned_retained_byte_bound().ok_or(
                GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                    resource: RESOURCE,
                },
            )?,
        )?;
    }
    for guard in guards {
        bytes = checked_add(
            RESOURCE,
            bytes,
            guard.owned_retained_byte_bound().ok_or(
                GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                    resource: RESOURCE,
                },
            )?,
        )?;
    }
    for reduction in reductions {
        bytes = checked_add(
            RESOURCE,
            bytes,
            reduction.factor.owned_retained_byte_bound().ok_or(
                GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow {
                    resource: RESOURCE,
                },
            )?,
        )?;
    }
    Ok(bytes)
}

fn try_terms_with_capacity(
    capacity: usize,
) -> Result<Vec<ExactDatabaseTerm>, GeneratedAffineResidualGroupExactDatabaseError> {
    let mut terms = Vec::new();
    terms.try_reserve_exact(capacity).map_err(|_| {
        GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
            resource: "exact top-reduction row terms",
        }
    })?;
    Ok(terms)
}

fn try_pivot_replacement_with_capacity(
    capacity: usize,
) -> Result<Vec<Arc<ExactUnitPivot>>, GeneratedAffineResidualGroupExactDatabaseError> {
    let mut pivots = Vec::new();
    pivots.try_reserve_exact(capacity).map_err(|_| {
        GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
            resource: "chronological exact-pivot replacement",
        }
    })?;
    Ok(pivots)
}

fn try_lookup_replacement_with_capacity(
    capacity: usize,
) -> Result<Vec<ExactPivotLookupEntry>, GeneratedAffineResidualGroupExactDatabaseError> {
    let mut lookup = Vec::new();
    lookup.try_reserve_exact(capacity).map_err(|_| {
        GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
            resource: "sorted exact-pivot lookup replacement",
        }
    })?;
    Ok(lookup)
}

fn try_guards_with_capacity(
    capacity: usize,
) -> Result<Vec<ParametricNonZeroCondition>, GeneratedAffineResidualGroupExactDatabaseError> {
    let mut guards = Vec::new();
    guards.try_reserve_exact(capacity).map_err(|_| {
        GeneratedAffineResidualGroupExactDatabaseError::AllocationFailure {
            resource: "exact top-reduction row guards",
        }
    })?;
    Ok(guards)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineResidualGroupExactDatabaseError> {
    if requested > limit {
        Err(
            GeneratedAffineResidualGroupExactDatabaseError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        )
    } else {
        Ok(())
    }
}

fn next_exact_database_nonce() -> Result<u64, GeneratedAffineResidualGroupExactDatabaseError> {
    take_exact_database_nonce(&NEXT_EXACT_DATABASE_NONCE)
}

fn take_exact_database_nonce(
    source: &AtomicU64,
) -> Result<u64, GeneratedAffineResidualGroupExactDatabaseError> {
    source
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |nonce| {
            nonce.checked_add(1)
        })
        .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::DatabaseIdentityExhaustion)
}

fn next_exact_database_transition_identity()
-> Result<ExactDatabaseTransitionIdentity, GeneratedAffineResidualGroupExactDatabaseError> {
    take_exact_database_transition_identity(&NEXT_EXACT_DATABASE_TRANSITION_NONCE)
}

fn take_exact_database_transition_identity(
    source: &AtomicU64,
) -> Result<ExactDatabaseTransitionIdentity, GeneratedAffineResidualGroupExactDatabaseError> {
    source
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |nonce| {
            nonce.checked_add(1)
        })
        .map(ExactDatabaseTransitionIdentity)
        .map_err(|_| GeneratedAffineResidualGroupExactDatabaseError::TransitionIdentityExhaustion)
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
    left.checked_add(right)
        .ok_or(GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
    left.checked_mul(right)
        .ok_or(GeneratedAffineResidualGroupExactDatabaseError::ResourceCountOverflow { resource })
}

fn checked_sum(
    resource: &'static str,
    values: impl IntoIterator<Item = usize>,
) -> Result<usize, GeneratedAffineResidualGroupExactDatabaseError> {
    values
        .into_iter()
        .try_fold(0usize, |sum, value| checked_add(resource, sum, value))
}
