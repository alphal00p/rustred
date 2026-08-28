//! Exact elimination adapter for the frozen four-loop next-shell parent rows.
//!
//! This module is intentionally only an adapter.  The reusable exact sparse
//! arithmetic lives in [`rustred::exact_sparse_elimination`], while the typed
//! four-loop source matrix is owned by [`FourLoopNextClosedRows`].  The
//! adapter authenticates that immutable boundary, requires agreement of the
//! three frozen modular discovery images, proves the result again over the
//! exact coefficient field, and projects the indexed result back to
//! [`FourLoopCornerColumnId`].
//!
//! A completed certificate covers exactly the 1,968 frozen parent rows and
//! their 1,734-column catalog.  Its free columns are unresolved coordinates
//! of this finite shell, not unrestricted four-loop masters.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use crate::four_loop_corner_shell::FourLoopCornerColumnId;
use crate::four_loop_next_closed_rows::{
    FOUR_LOOP_NEXT_CLOSED_ROWS, FOUR_LOOP_NEXT_CLOSED_ROWS_CHECKSUM,
    FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRIES, FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_MAX_ROW_WIDTH, FOUR_LOOP_NEXT_CLOSED_ROWS_ZERO_ROWS,
    FourLoopNextClosedRows, FourLoopNextClosedRowsError,
};
use crate::four_loop_next_manifest::FourLoopNextRawRowId;
use crate::four_loop_next_modular_rank::{
    FOUR_LOOP_NEXT_MODULAR_DISCOVERY_IMAGES, FourLoopNextModularRankConfig,
    FourLoopNextModularRankError, FourLoopNextModularRankReport,
    discover_four_loop_next_modular_rank_at_images,
};
use rustred::SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT;
use rustred::coefficient::{Coefficient, CoefficientContext, CoefficientProjectionError};
use rustred::exact_sparse_elimination::{
    ExactSparseElimination, ExactSparseEliminationConfig, ExactSparseEliminationError,
    ExactSparseRow,
};

const FNV1A64_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV1A64_PRIME: u64 = 0x0000_0100_0000_01b3;
const FOUR_LOOP_NEXT_ELIMINATION_SCHEMA: &str =
    "rustred-equal-mass-euclidean-four-loop-next-fixed-seed-elimination-v1";

/// Frozen exact regression facts from the accepted composed production replay.
///
/// Unlike the modular-discovery constants, these values were retained only
/// after exact construction and [`FourLoopNextElimination::replay`] both
/// succeeded for the authenticated fixed-seed matrix.  They remain facts about
/// this finite shell, not an unrestricted four-loop master census.
pub const FOUR_LOOP_NEXT_ELIMINATION_SOURCE_ROWS: usize = 1_968;
pub const FOUR_LOOP_NEXT_ELIMINATION_COLUMNS: usize = 1_734;
pub const FOUR_LOOP_NEXT_ELIMINATION_INPUT_ENTRIES: usize = 22_424;
pub const FOUR_LOOP_NEXT_ELIMINATION_MAXIMUM_INPUT_ROW_WIDTH: usize = 45;
pub const FOUR_LOOP_NEXT_ELIMINATION_MODULAR_IMAGES: usize = 3;
pub const FOUR_LOOP_NEXT_ELIMINATION_MODULAR_CANDIDATE_RANK: usize = 1_588;
pub const FOUR_LOOP_NEXT_ELIMINATION_RANK: usize = 1_588;
pub const FOUR_LOOP_NEXT_ELIMINATION_PIVOT_RULES: usize = 1_588;
pub const FOUR_LOOP_NEXT_ELIMINATION_FREE_UNRESOLVED_COLUMNS: usize = 146;
pub const FOUR_LOOP_NEXT_ELIMINATION_PROJECTED_RHS_ENTRIES: usize = 15_461;
pub const FOUR_LOOP_NEXT_ELIMINATION_TRACE_REDUCTIONS: usize = 3_646;
pub const FOUR_LOOP_NEXT_ELIMINATION_MAXIMUM_TRACE_REDUCTIONS: usize = 169;
pub const FOUR_LOOP_NEXT_ELIMINATION_PROJECTED_COEFFICIENT_TERMS: usize = 47_780;
pub const FOUR_LOOP_NEXT_ELIMINATION_PROJECTED_COEFFICIENT_BYTES: usize = 94_202;
pub const FOUR_LOOP_NEXT_ELIMINATION_EXACT_PIVOT_REDUCTIONS: usize = 3_646;
pub const FOUR_LOOP_NEXT_ELIMINATION_EXACT_VERIFICATION_REDUCTIONS: usize = 23_993;
pub const FOUR_LOOP_NEXT_ELIMINATION_EXACT_ARITHMETIC_UPDATES: usize = 46_580;
pub const FOUR_LOOP_NEXT_ELIMINATION_EXACT_RETAINED_ENTRIES: usize = 22_283;
pub const FOUR_LOOP_NEXT_ELIMINATION_EXACT_RETAINED_COEFFICIENT_TERMS: usize = 50_956;
pub const FOUR_LOOP_NEXT_ELIMINATION_EXACT_RETAINED_COEFFICIENT_BYTES: usize = 95_067;
pub const FOUR_LOOP_NEXT_ELIMINATION_EXACT_MAXIMUM_ROW_WIDTH: usize = 173;
pub const FOUR_LOOP_NEXT_ELIMINATION_EXACT_MAXIMUM_COEFFICIENT_DEGREE: usize = 5;
pub const FOUR_LOOP_NEXT_ELIMINATION_EXACT_REPLAY_REDUCTIONS: usize = 27_639;
pub const FOUR_LOOP_NEXT_ELIMINATION_EXACT_REPLAY_UPDATES: usize = 232_446;
pub const FOUR_LOOP_NEXT_ELIMINATION_CONSERVATIVE_CONDITION_SLOTS: usize = 45_087;

pub const FOUR_LOOP_NEXT_ELIMINATION_PARENT_ROW_SCALE_SLOTS: usize = 1_968;
pub const FOUR_LOOP_NEXT_ELIMINATION_PARENT_COEFFICIENT_DENOMINATOR_SLOTS: usize = 22_424;
pub const FOUR_LOOP_NEXT_ELIMINATION_TRACE_DIVISOR_SLOTS: usize = 1_588;
pub const FOUR_LOOP_NEXT_ELIMINATION_TRACE_FACTOR_DENOMINATOR_SLOTS: usize = 3_646;
pub const FOUR_LOOP_NEXT_ELIMINATION_RULE_RHS_DENOMINATOR_SLOTS: usize = 15_461;

pub const FOUR_LOOP_NEXT_ELIMINATION_PROJECTED_SOURCE_CHECKSUM: u64 = 0x8900_8a25_3f62_89fa;
pub const FOUR_LOOP_NEXT_ELIMINATION_EXACT_CHECKSUM: u64 = 0x97c0_89ef_cd1b_808d;
pub const FOUR_LOOP_NEXT_ELIMINATION_CHECKSUM: u64 = 0x2e72_3cec_8b36_c8de;

const FOUR_LOOP_NEXT_PROJECTED_TRIANGULAR_ENTRY_BOUND: usize =
    FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS * (FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS - 1) / 2;

/// Independent resource envelopes for discovery and exact proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopNextEliminationConfig {
    pub modular: FourLoopNextModularRankConfig,
    pub exact: ExactSparseEliminationConfig,
    /// Typed pivot records retained by this four-loop projection layer.
    pub max_projected_pivots: usize,
    /// Total typed right-hand-side map entries across all projected rules.
    pub max_projected_rhs_entries: usize,
    /// Total recursive trace edges retained across all projected rules.
    pub max_projected_trace_reductions: usize,
    /// Sum of numerator and denominator terms in projected coefficients.
    pub max_projected_coefficient_terms: usize,
    /// Total bounded display bytes of all projected coefficients.
    pub max_projected_coefficient_bytes: usize,
}

impl Default for FourLoopNextEliminationConfig {
    fn default() -> Self {
        let exact = ExactSparseEliminationConfig::default();
        Self {
            modular: FourLoopNextModularRankConfig::default(),
            exact,
            max_projected_pivots: FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS,
            max_projected_rhs_entries: FOUR_LOOP_NEXT_PROJECTED_TRIANGULAR_ENTRY_BOUND,
            max_projected_trace_reductions: FOUR_LOOP_NEXT_PROJECTED_TRIANGULAR_ENTRY_BOUND,
            max_projected_coefficient_terms: exact.max_retained_coefficient_terms,
            max_projected_coefficient_bytes: exact.max_retained_coefficient_bytes,
        }
    }
}

/// Scope of the completed exact certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FourLoopNextEliminationStatus {
    /// Exact over `Q(d)` for the authenticated fixed-seed matrix only.
    CompleteFixedSeedShell,
}

/// One recursive provenance edge in an exact unit pivot row.
///
/// If `T` is the stored trace for a rule, the generic engine proves
///
/// ```text
/// unit_row(T) =
///   (closed_row[T.base_source_row_index]
///    - sum reduction.factor * prior_unit_row[reduction.prior_pivot_ordinal])
///   / T.divisor.
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopNextEliminationTraceReduction {
    prior_pivot_ordinal: usize,
    prior_pivot: FourLoopCornerColumnId,
    factor: Coefficient,
}

impl FourLoopNextEliminationTraceReduction {
    pub const fn prior_pivot_ordinal(&self) -> usize {
        self.prior_pivot_ordinal
    }

    pub const fn prior_pivot(&self) -> &FourLoopCornerColumnId {
        &self.prior_pivot
    }

    pub const fn factor(&self) -> &Coefficient {
        &self.factor
    }
}

/// Typed source-row trace for one exact pivot rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopNextEliminationTrace {
    base_source_row_index: usize,
    base_source_raw_id: FourLoopNextRawRowId,
    reductions: Vec<FourLoopNextEliminationTraceReduction>,
    divisor: Coefficient,
}

impl FourLoopNextEliminationTrace {
    pub const fn base_source_row_index(&self) -> usize {
        self.base_source_row_index
    }

    pub const fn base_source_raw_id(&self) -> FourLoopNextRawRowId {
        self.base_source_raw_id
    }

    pub fn reductions(&self) -> &[FourLoopNextEliminationTraceReduction] {
        &self.reductions
    }

    pub const fn divisor(&self) -> &Coefficient {
        &self.divisor
    }
}

/// A typed, strictly triangular exact rule `pivot = rhs`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopNextEliminationPivotRule {
    ordinal: usize,
    pivot: FourLoopCornerColumnId,
    rhs: BTreeMap<FourLoopCornerColumnId, Coefficient>,
    trace: FourLoopNextEliminationTrace,
}

impl FourLoopNextEliminationPivotRule {
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub const fn pivot(&self) -> &FourLoopCornerColumnId {
        &self.pivot
    }

    pub const fn rhs(&self) -> &BTreeMap<FourLoopCornerColumnId, Coefficient> {
        &self.rhs
    }

    pub const fn source_row_index(&self) -> usize {
        self.trace.base_source_row_index
    }

    pub const fn source_raw_id(&self) -> FourLoopNextRawRowId {
        self.trace.base_source_raw_id
    }

    /// Recursive exact provenance rooted in one authenticated closed row.
    pub const fn trace(&self) -> &FourLoopNextEliminationTrace {
        &self.trace
    }
}

/// Honesty boundary for the current exceptional-condition surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FourLoopNextEliminationConditionStatus {
    /// Exact use counts are retained, but polynomials are neither factored nor
    /// advertised as a complete exceptional-dimension classification.
    ConservativeUnfactoredInversionSlotCensusOnly,
}

/// Conservative census of places relevant to fixed-`d` specialization.
///
/// These counts make omitted condition work visible without publishing a
/// misleading list of roots.  They include trivial/unit slots and may
/// overcount repeated factors.  In particular, this is not a complete
/// upstream condition inventory and cannot be used to specialize the generic
/// certificate at a numerical value of `d`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FourLoopNextEliminationConditions {
    parent_row_scale_slots: usize,
    parent_coefficient_denominator_slots: usize,
    trace_divisor_slots: usize,
    trace_factor_denominator_slots: usize,
    rule_rhs_denominator_slots: usize,
    total_slots: usize,
}

impl FourLoopNextEliminationConditions {
    pub const fn status(self) -> FourLoopNextEliminationConditionStatus {
        FourLoopNextEliminationConditionStatus::ConservativeUnfactoredInversionSlotCensusOnly
    }

    pub const fn is_complete_exceptional_dimension_inventory(self) -> bool {
        false
    }

    pub const fn parent_row_scale_slots(self) -> usize {
        self.parent_row_scale_slots
    }

    pub const fn parent_coefficient_denominator_slots(self) -> usize {
        self.parent_coefficient_denominator_slots
    }

    pub const fn trace_divisor_slots(self) -> usize {
        self.trace_divisor_slots
    }

    pub const fn trace_factor_denominator_slots(self) -> usize {
        self.trace_factor_denominator_slots
    }

    pub const fn rule_rhs_denominator_slots(self) -> usize {
        self.rule_rhs_denominator_slots
    }

    pub const fn total_slots(self) -> usize {
        self.total_slots
    }
}

/// Adapter-level census.  Detailed arithmetic counters remain available from
/// the generic exact-engine certificate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FourLoopNextEliminationStats {
    source_rows: usize,
    columns: usize,
    input_entries: usize,
    maximum_input_row_width: usize,
    modular_images: usize,
    modular_candidate_rank: usize,
    exact_rank: usize,
    pivot_rules: usize,
    free_unresolved_columns: usize,
    projected_rhs_entries: usize,
    trace_reductions: usize,
    maximum_trace_reductions: usize,
    projected_coefficient_terms: usize,
    projected_coefficient_bytes: usize,
    exact_pivot_reductions: usize,
    exact_verification_reductions: usize,
    exact_arithmetic_updates: usize,
    exact_retained_entries: usize,
    exact_retained_coefficient_terms: usize,
    exact_retained_coefficient_bytes: usize,
    exact_maximum_row_width: usize,
    exact_maximum_coefficient_degree: usize,
    exact_replay_reductions: usize,
    exact_replay_updates: usize,
    conservative_condition_slots: usize,
}

impl FourLoopNextEliminationStats {
    pub const fn source_rows(self) -> usize {
        self.source_rows
    }

    pub const fn columns(self) -> usize {
        self.columns
    }

    pub const fn input_entries(self) -> usize {
        self.input_entries
    }

    pub const fn maximum_input_row_width(self) -> usize {
        self.maximum_input_row_width
    }

    pub const fn modular_images(self) -> usize {
        self.modular_images
    }

    /// Advisory rank agreed by the three finite-field images.
    pub const fn modular_candidate_rank(self) -> usize {
        self.modular_candidate_rank
    }

    /// Rank proved independently by exact elimination.
    pub const fn exact_rank(self) -> usize {
        self.exact_rank
    }

    pub const fn pivot_rules(self) -> usize {
        self.pivot_rules
    }

    pub const fn free_unresolved_columns(self) -> usize {
        self.free_unresolved_columns
    }

    pub const fn projected_rhs_entries(self) -> usize {
        self.projected_rhs_entries
    }

    pub const fn trace_reductions(self) -> usize {
        self.trace_reductions
    }

    pub const fn maximum_trace_reductions(self) -> usize {
        self.maximum_trace_reductions
    }

    pub const fn projected_coefficient_terms(self) -> usize {
        self.projected_coefficient_terms
    }

    pub const fn projected_coefficient_bytes(self) -> usize {
        self.projected_coefficient_bytes
    }

    pub const fn exact_pivot_reductions(self) -> usize {
        self.exact_pivot_reductions
    }

    pub const fn exact_verification_reductions(self) -> usize {
        self.exact_verification_reductions
    }

    pub const fn exact_arithmetic_updates(self) -> usize {
        self.exact_arithmetic_updates
    }

    pub const fn exact_retained_entries(self) -> usize {
        self.exact_retained_entries
    }

    pub const fn exact_retained_coefficient_terms(self) -> usize {
        self.exact_retained_coefficient_terms
    }

    pub const fn exact_retained_coefficient_bytes(self) -> usize {
        self.exact_retained_coefficient_bytes
    }

    pub const fn exact_maximum_row_width(self) -> usize {
        self.exact_maximum_row_width
    }

    pub const fn exact_maximum_coefficient_degree(self) -> usize {
        self.exact_maximum_coefficient_degree
    }

    pub const fn exact_replay_reductions(self) -> usize {
        self.exact_replay_reductions
    }

    pub const fn exact_replay_updates(self) -> usize {
        self.exact_replay_updates
    }

    pub const fn conservative_condition_slots(self) -> usize {
        self.conservative_condition_slots
    }
}

/// Fully typed exact proof for the frozen four-loop next-shell matrix.
pub struct FourLoopNextElimination<'closed, 'sources, 'transport, 'inventory> {
    closed: &'closed FourLoopNextClosedRows<'sources, 'transport, 'inventory>,
    config: FourLoopNextEliminationConfig,
    coefficient_context: CoefficientContext,
    modular_discovery: FourLoopNextModularRankReport,
    exact: ExactSparseElimination,
    pivots: Vec<FourLoopNextEliminationPivotRule>,
    free_unresolved_columns: Vec<FourLoopCornerColumnId>,
    conditions: FourLoopNextEliminationConditions,
    stats: FourLoopNextEliminationStats,
    checksum: u64,
}

impl<'closed, 'sources, 'transport, 'inventory>
    FourLoopNextElimination<'closed, 'sources, 'transport, 'inventory>
{
    pub const SCHEMA: &'static str = FOUR_LOOP_NEXT_ELIMINATION_SCHEMA;

    /// Reject resource envelopes which cannot admit the immutable source
    /// census before any coefficient projection or finite-field work starts.
    pub fn preflight_config(
        config: FourLoopNextEliminationConfig,
    ) -> Result<(), FourLoopNextEliminationError> {
        preflight_config(config)
    }

    /// Authenticate, discover, and prove the frozen fixed-seed shell.
    ///
    /// Finite-field agreement is checked before exact work only as a pivot
    /// proposal.  `CompleteFixedSeedShell` is returned only after the generic
    /// exact engine has proved the same rank and skeleton over `Q(d)`.
    pub fn build(
        closed: &'closed FourLoopNextClosedRows<'sources, 'transport, 'inventory>,
        config: FourLoopNextEliminationConfig,
    ) -> Result<Self, FourLoopNextEliminationError> {
        preflight_config(config)?;
        let coefficient_context = CoefficientContext::new(["d"]);
        let source = authenticate_and_index_source(closed, &coefficient_context, config)?;
        let modular_discovery = discover_four_loop_next_modular_rank_at_images(
            closed,
            &FOUR_LOOP_NEXT_MODULAR_DISCOVERY_IMAGES,
            config.modular,
        )?;
        let (modular_rank, pivot_skeleton) =
            authenticate_modular_discovery(closed, &modular_discovery)?;
        preflight_projected_rank(config, modular_rank)?;

        // This is the proof boundary: no exact rank claim exists before this
        // call returns a replayable exact certificate.
        let exact = ExactSparseElimination::build(
            &coefficient_context,
            &source.rows,
            closed.columns().len(),
            &pivot_skeleton,
            config.exact,
        )?;
        authenticate_exact_result(&exact, modular_rank, &pivot_skeleton)?;

        let projected = project_exact_result(closed, &coefficient_context, &exact, config)?;
        let conditions = condition_census(closed, &projected.pivots)?;
        let stats = adapter_stats(
            &source,
            &modular_discovery,
            modular_rank,
            &exact,
            &projected,
            conditions,
        )?;
        let checksum = certificate_checksum(
            closed,
            &config,
            &modular_discovery,
            &exact,
            &projected.pivots,
            &projected.free_unresolved_columns,
            conditions,
            stats,
        )?;

        Ok(Self {
            closed,
            config,
            coefficient_context,
            modular_discovery,
            exact,
            pivots: projected.pivots,
            free_unresolved_columns: projected.free_unresolved_columns,
            conditions,
            stats,
            checksum,
        })
    }

    pub const fn status(&self) -> FourLoopNextEliminationStatus {
        FourLoopNextEliminationStatus::CompleteFixedSeedShell
    }

    /// The exact proof domain.  Literal `m2` absence is re-authenticated at
    /// the adapter boundary before the generic engine is called.
    pub const fn proof_domain(&self) -> &'static str {
        "Q(d)"
    }

    /// Exact one-variable context used by every retained rule and trace.
    pub const fn coefficient_context(&self) -> &CoefficientContext {
        &self.coefficient_context
    }

    pub const fn closed_rows(&self) -> &FourLoopNextClosedRows<'sources, 'transport, 'inventory> {
        self.closed
    }

    pub const fn config(&self) -> &FourLoopNextEliminationConfig {
        &self.config
    }

    /// Advisory finite-field evidence retained verbatim for audit.
    pub const fn modular_discovery(&self) -> &FourLoopNextModularRankReport {
        &self.modular_discovery
    }

    /// Generic engine certificate, with indexed columns.
    pub const fn exact_engine(&self) -> &ExactSparseElimination {
        &self.exact
    }

    pub fn rank(&self) -> usize {
        self.pivots.len()
    }

    pub fn columns(&self) -> &[FourLoopCornerColumnId] {
        self.closed.columns()
    }

    pub fn pivots(&self) -> &[FourLoopNextEliminationPivotRule] {
        &self.pivots
    }

    /// Ordered complement of the exact pivot set in this finite catalog.
    /// These coordinates are intentionally not called masters.
    pub fn free_unresolved_columns(&self) -> &[FourLoopCornerColumnId] {
        &self.free_unresolved_columns
    }

    pub const fn conditions(&self) -> FourLoopNextEliminationConditions {
        self.conditions
    }

    pub const fn stats(&self) -> FourLoopNextEliminationStats {
        self.stats
    }

    pub const fn checksum(&self) -> u64 {
        self.checksum
    }

    pub const fn source_checksum(&self) -> u64 {
        self.closed.checksum()
    }

    pub fn pivot_rule(
        &self,
        column: &FourLoopCornerColumnId,
    ) -> Option<&FourLoopNextEliminationPivotRule> {
        self.pivots.iter().find(|rule| rule.pivot() == column)
    }

    /// Replay the composed proof without widening its scope.
    ///
    /// This first replays the native parent-row certificate, rediscovers all
    /// three modular images, replays the exact indexed certificate against a
    /// freshly projected source matrix, and finally reprojects and hashes all
    /// typed adapter metadata.
    pub fn replay(&self) -> Result<(), FourLoopNextEliminationError> {
        preflight_config(self.config)?;
        if self.coefficient_context.parameter_names() != ["d"] {
            return Err(FourLoopNextEliminationError::ReplayMismatch {
                component: "exact Q(d) coefficient context",
            });
        }
        let source =
            authenticate_and_index_source(self.closed, &self.coefficient_context, self.config)?;
        self.closed.replay()?;

        let modular_discovery = discover_four_loop_next_modular_rank_at_images(
            self.closed,
            &FOUR_LOOP_NEXT_MODULAR_DISCOVERY_IMAGES,
            self.config.modular,
        )?;
        let (modular_rank, pivot_skeleton) =
            authenticate_modular_discovery(self.closed, &modular_discovery)?;
        preflight_projected_rank(self.config, modular_rank)?;
        if modular_discovery != self.modular_discovery {
            return Err(FourLoopNextEliminationError::ReplayMismatch {
                component: "modular discovery report",
            });
        }

        self.exact.replay(&self.coefficient_context, &source.rows)?;
        authenticate_exact_result(&self.exact, modular_rank, &pivot_skeleton)?;
        let projected = project_exact_result(
            self.closed,
            &self.coefficient_context,
            &self.exact,
            self.config,
        )?;
        if projected.pivots != self.pivots
            || projected.free_unresolved_columns != self.free_unresolved_columns
        {
            return Err(FourLoopNextEliminationError::ReplayMismatch {
                component: "typed pivot projection",
            });
        }

        let conditions = condition_census(self.closed, &projected.pivots)?;
        let stats = adapter_stats(
            &source,
            &modular_discovery,
            modular_rank,
            &self.exact,
            &projected,
            conditions,
        )?;
        let checksum = certificate_checksum(
            self.closed,
            &self.config,
            &modular_discovery,
            &self.exact,
            &projected.pivots,
            &projected.free_unresolved_columns,
            conditions,
            stats,
        )?;
        if conditions != self.conditions || stats != self.stats || checksum != self.checksum {
            return Err(FourLoopNextEliminationError::ReplayMismatch {
                component: "adapter conditions, statistics, or checksum",
            });
        }
        Ok(())
    }
}

impl fmt::Display for FourLoopNextElimination<'_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} status={:?} domain={} rows={} columns={} rank={} free_unresolved={} modular_images={} source_checksum=0x{:016x} exact_checksum=0x{:016x} checksum=0x{:016x}; fixed-seed shell only; conditions={:?}",
            Self::SCHEMA,
            self.status(),
            self.proof_domain(),
            self.stats.source_rows,
            self.stats.columns,
            self.rank(),
            self.free_unresolved_columns.len(),
            self.stats.modular_images,
            self.closed.checksum(),
            self.exact.checksum(),
            self.checksum,
            self.conditions.status(),
        )
    }
}

/// Typed failures at the four-loop adapter boundary.
#[derive(Debug)]
pub enum FourLoopNextEliminationError {
    SourceCensusMismatch {
        resource: &'static str,
        expected: usize,
        actual: usize,
    },
    SourceChecksumMismatch {
        expected: u64,
        actual: u64,
    },
    SourceCoefficientContextMismatch,
    SourceColumnOrderMismatch,
    SourceRowMismatch {
        row_index: usize,
        reason: &'static str,
    },
    CoefficientProjection {
        row_index: usize,
        column_index: usize,
        source: CoefficientProjectionError,
    },
    Modular(FourLoopNextModularRankError),
    ModularImageSetMismatch,
    ModularEvidenceDisagrees {
        ranks: bool,
        pivot_columns: bool,
        source_row_skeleton: bool,
    },
    VacuousModularCandidate,
    InvalidModularSkeleton {
        reason: &'static str,
    },
    Exact(ExactSparseEliminationError),
    ExactShapeMismatch {
        rows: usize,
        columns: usize,
    },
    ExactRankMismatch {
        modular_candidate: usize,
        exact: usize,
    },
    ExactSkeletonMismatch {
        ordinal: usize,
        expected_column: usize,
        actual_column: usize,
        expected_source_row: usize,
        actual_source_row: usize,
    },
    InvalidExactRule {
        ordinal: usize,
        reason: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    ArithmeticOverflow {
        resource: &'static str,
    },
    ParentReplay(FourLoopNextClosedRowsError),
    ReplayMismatch {
        component: &'static str,
    },
}

impl fmt::Display for FourLoopNextEliminationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceCensusMismatch {
                resource,
                expected,
                actual,
            } => write!(
                formatter,
                "frozen parent {resource} mismatch: expected {expected}, found {actual}"
            ),
            Self::SourceChecksumMismatch { expected, actual } => write!(
                formatter,
                "frozen parent checksum mismatch: expected 0x{expected:016x}, found 0x{actual:016x}"
            ),
            Self::SourceCoefficientContextMismatch => formatter.write_str(
                "four-loop exact elimination requires the canonical [d,m2] coefficient map with literal m2-free entries",
            ),
            Self::SourceColumnOrderMismatch => formatter.write_str(
                "four-loop exact elimination requires the frozen strictly ordered column catalog",
            ),
            Self::SourceRowMismatch { row_index, reason } => {
                write!(formatter, "frozen parent row {row_index} is invalid: {reason}")
            }
            Self::CoefficientProjection {
                row_index,
                column_index,
                source,
            } => write!(
                formatter,
                "could not project parent row {row_index}, column {column_index} into Q(d): {source}"
            ),
            Self::Modular(error) => write!(formatter, "modular discovery failed: {error}"),
            Self::ModularImageSetMismatch => formatter.write_str(
                "modular discovery did not retain exactly the three frozen images in order",
            ),
            Self::ModularEvidenceDisagrees {
                ranks,
                pivot_columns,
                source_row_skeleton,
            } => write!(
                formatter,
                "the three modular images do not agree: ranks={ranks}, pivot_columns={pivot_columns}, source_row_skeleton={source_row_skeleton}"
            ),
            Self::VacuousModularCandidate => formatter.write_str(
                "the three-image modular candidate must contain at least one exact-proof proposal",
            ),
            Self::InvalidModularSkeleton { reason } => {
                write!(formatter, "invalid modular pivot skeleton: {reason}")
            }
            Self::Exact(error) => write!(formatter, "exact sparse elimination failed: {error}"),
            Self::ExactShapeMismatch { rows, columns } => write!(
                formatter,
                "exact certificate shape mismatch: found {rows} rows and {columns} columns"
            ),
            Self::ExactRankMismatch {
                modular_candidate,
                exact,
            } => write!(
                formatter,
                "the exact rank {exact} differs from the advisory modular candidate {modular_candidate}"
            ),
            Self::ExactSkeletonMismatch {
                ordinal,
                expected_column,
                actual_column,
                expected_source_row,
                actual_source_row,
            } => write!(
                formatter,
                "exact pivot {ordinal} differs from the authenticated modular proposal: column {actual_column} (expected {expected_column}), source row {actual_source_row} (expected {expected_source_row})"
            ),
            Self::InvalidExactRule { ordinal, reason } => {
                write!(formatter, "exact pivot rule {ordinal} is invalid: {reason}")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "four-loop elimination {resource} requested {requested}, limit is {limit}"
            ),
            Self::ArithmeticOverflow { resource } => {
                write!(formatter, "arithmetic overflow while counting {resource}")
            }
            Self::ParentReplay(error) => write!(formatter, "parent-row replay failed: {error}"),
            Self::ReplayMismatch { component } => {
                write!(formatter, "four-loop elimination replay mismatch in {component}")
            }
        }
    }
}

impl Error for FourLoopNextEliminationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Modular(error) => Some(error),
            Self::Exact(error) => Some(error),
            Self::ParentReplay(error) => Some(error),
            Self::CoefficientProjection { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<FourLoopNextModularRankError> for FourLoopNextEliminationError {
    fn from(error: FourLoopNextModularRankError) -> Self {
        Self::Modular(error)
    }
}

impl From<ExactSparseEliminationError> for FourLoopNextEliminationError {
    fn from(error: ExactSparseEliminationError) -> Self {
        Self::Exact(error)
    }
}

impl From<FourLoopNextClosedRowsError> for FourLoopNextEliminationError {
    fn from(error: FourLoopNextClosedRowsError) -> Self {
        Self::ParentReplay(error)
    }
}

fn preflight_config(
    config: FourLoopNextEliminationConfig,
) -> Result<(), FourLoopNextEliminationError> {
    check_resource(
        "configured exact source rows",
        FOUR_LOOP_NEXT_CLOSED_ROWS,
        config.exact.max_rows,
    )?;
    check_resource(
        "configured exact columns",
        FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS,
        config.exact.max_columns,
    )?;
    check_resource(
        "configured exact input entries",
        FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRIES,
        config.exact.max_input_entries,
    )?;
    // Every retained nonzero input coefficient has a nonempty textual
    // representation.  This lower bound is deliberately cheap and prevents
    // projecting the full matrix only to discover an impossible byte cap.
    check_resource(
        "configured exact input coefficient bytes",
        FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRIES,
        config.exact.max_input_coefficient_bytes,
    )?;
    check_resource(
        "configured modular images",
        FOUR_LOOP_NEXT_MODULAR_DISCOVERY_IMAGES.len(),
        config.modular.max_images,
    )?;
    check_resource(
        "configured modular input entries",
        FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRIES,
        config.modular.max_initial_nonzeros,
    )?;
    if config.exact.max_coefficient_degree as u128 > SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT {
        return Err(FourLoopNextEliminationError::ResourceLimit {
            resource: "configured exact coefficient exponent degree",
            requested: config.exact.max_coefficient_degree as u128,
            limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
        });
    }

    // The modular candidate is required to be non-vacuous.  Consequently an
    // exact certificate must retain at least one unit pivot and its divisor,
    // and the all-source proof must perform at least one reduction/update for
    // each of the 1,968 nonzero source rows.
    for (resource, requested, limit) in [
        (
            "configured exact arithmetic updates",
            1,
            config.exact.max_updates,
        ),
        (
            "configured exact retained entries",
            2,
            config.exact.max_retained_entries,
        ),
        (
            "configured exact retained coefficient terms",
            4,
            config.exact.max_retained_coefficient_terms,
        ),
        (
            "configured exact retained coefficient bytes",
            2,
            config.exact.max_retained_coefficient_bytes,
        ),
        (
            "configured exact coefficient operation terms",
            1,
            config.exact.max_coefficient_operation_terms,
        ),
        (
            "configured exact coefficient dense terms",
            1,
            config.exact.max_coefficient_dense_terms,
        ),
        (
            "configured exact replay reductions",
            FOUR_LOOP_NEXT_CLOSED_ROWS,
            config.exact.max_replay_reductions,
        ),
        (
            "configured exact replay updates",
            FOUR_LOOP_NEXT_CLOSED_ROWS,
            config.exact.max_replay_updates,
        ),
        (
            "configured projected pivots",
            1,
            config.max_projected_pivots,
        ),
        (
            "configured projected coefficient terms",
            2,
            config.max_projected_coefficient_terms,
        ),
        (
            "configured projected coefficient bytes",
            1,
            config.max_projected_coefficient_bytes,
        ),
    ] {
        check_resource(resource, requested, limit)?;
    }
    Ok(())
}

fn preflight_projected_rank(
    config: FourLoopNextEliminationConfig,
    rank: usize,
) -> Result<(), FourLoopNextEliminationError> {
    check_resource("projected pivots", rank, config.max_projected_pivots)?;
    let minimum_terms =
        rank.checked_mul(2)
            .ok_or(FourLoopNextEliminationError::ArithmeticOverflow {
                resource: "minimum projected coefficient terms",
            })?;
    check_resource(
        "minimum projected coefficient terms",
        minimum_terms,
        config.max_projected_coefficient_terms,
    )?;
    check_resource(
        "minimum projected coefficient bytes",
        rank,
        config.max_projected_coefficient_bytes,
    )
}

fn check_resource(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), FourLoopNextEliminationError> {
    if requested > limit {
        Err(FourLoopNextEliminationError::ResourceLimit {
            resource,
            requested: requested as u128,
            limit: limit as u128,
        })
    } else {
        Ok(())
    }
}

struct IndexedSource {
    rows: Vec<ExactSparseRow>,
    entries: usize,
    maximum_row_width: usize,
}

fn authenticate_and_index_source(
    closed: &FourLoopNextClosedRows<'_, '_, '_>,
    target_context: &CoefficientContext,
    config: FourLoopNextEliminationConfig,
) -> Result<IndexedSource, FourLoopNextEliminationError> {
    check_source_census("row count", FOUR_LOOP_NEXT_CLOSED_ROWS, closed.rows().len())?;
    check_source_census(
        "column count",
        FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS,
        closed.columns().len(),
    )?;
    if closed.checksum() != FOUR_LOOP_NEXT_CLOSED_ROWS_CHECKSUM {
        return Err(FourLoopNextEliminationError::SourceChecksumMismatch {
            expected: FOUR_LOOP_NEXT_CLOSED_ROWS_CHECKSUM,
            actual: closed.checksum(),
        });
    }
    if closed.coefficient_context().parameter_names() != ["d", "m2"] {
        return Err(FourLoopNextEliminationError::SourceCoefficientContextMismatch);
    }
    if target_context.parameter_names() != ["d"] {
        return Err(FourLoopNextEliminationError::SourceCoefficientContextMismatch);
    }
    if !closed.columns().windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(FourLoopNextEliminationError::SourceColumnOrderMismatch);
    }

    let mut rows = Vec::new();
    rows.try_reserve_exact(closed.rows().len()).map_err(|_| {
        FourLoopNextEliminationError::ResourceLimit {
            resource: "projected exact source rows",
            requested: closed.rows().len() as u128,
            limit: config.exact.max_rows as u128,
        }
    })?;
    let mut entries = 0_usize;
    let mut maximum_row_width = 0_usize;
    let mut projected_coefficient_bytes = 0_usize;
    let mut used_columns = BTreeSet::new();
    let mut raw_ids = BTreeSet::new();

    for (row_index, source_row) in closed.rows().iter().enumerate() {
        if !raw_ids.insert(source_row.raw_id()) {
            return Err(FourLoopNextEliminationError::SourceRowMismatch {
                row_index,
                reason: "duplicate raw-row provenance",
            });
        }
        if source_row.entries().is_empty() {
            return Err(FourLoopNextEliminationError::SourceRowMismatch {
                row_index,
                reason: "zero row",
            });
        }
        maximum_row_width = maximum_row_width.max(source_row.entries().len());
        entries = entries.checked_add(source_row.entries().len()).ok_or(
            FourLoopNextEliminationError::ArithmeticOverflow {
                resource: "source entries",
            },
        )?;
        check_resource(
            "projected exact input entries",
            entries,
            config.exact.max_input_entries,
        )?;

        let mut indexed = ExactSparseRow::new();
        for (column, coefficient) in source_row.entries() {
            let column_index = closed.columns().binary_search(column).map_err(|_| {
                FourLoopNextEliminationError::SourceRowMismatch {
                    row_index,
                    reason: "column is outside the frozen catalog",
                }
            })?;
            if coefficient.is_zero() {
                return Err(FourLoopNextEliminationError::SourceRowMismatch {
                    row_index,
                    reason: "explicit zero coefficient",
                });
            }
            let projected = closed
                .coefficient_context()
                .project_parameter_free(coefficient, "m2", target_context)
                .map_err(
                    |source| FourLoopNextEliminationError::CoefficientProjection {
                        row_index,
                        column_index,
                        source,
                    },
                )?;
            if projected.is_zero() {
                return Err(FourLoopNextEliminationError::SourceRowMismatch {
                    row_index,
                    reason: "a nonzero source coefficient projected to zero",
                });
            }
            let serialized_bytes = bounded_display_len(
                &projected,
                projected_coefficient_bytes,
                config.exact.max_input_coefficient_bytes,
                "projected exact input coefficient bytes",
            )?;
            projected_coefficient_bytes = projected_coefficient_bytes
                .checked_add(serialized_bytes)
                .ok_or(FourLoopNextEliminationError::ArithmeticOverflow {
                    resource: "projected exact input coefficient bytes",
                })?;
            used_columns.insert(column_index);
            indexed.insert(column_index, projected);
        }

        let actual_pivot = indexed.last_key_value().map(|(&column, _)| column);
        if source_row
            .pivot_column_index()
            .map(|column| column as usize)
            != actual_pivot
        {
            return Err(FourLoopNextEliminationError::SourceRowMismatch {
                row_index,
                reason: "stored pivot index does not name the hardest retained column",
            });
        }
        rows.push(indexed);
    }

    check_source_census(
        "retained entry count",
        FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRIES,
        entries,
    )?;
    check_source_census(
        "maximum row width",
        FOUR_LOOP_NEXT_CLOSED_ROWS_MAX_ROW_WIDTH,
        maximum_row_width,
    )?;
    check_source_census(
        "zero-row count",
        FOUR_LOOP_NEXT_CLOSED_ROWS_ZERO_ROWS,
        closed
            .rows()
            .iter()
            .filter(|row| row.entries().is_empty())
            .count(),
    )?;
    check_source_census(
        "used column count",
        FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS,
        used_columns.len(),
    )?;

    let stats = closed.stats();
    check_source_census(
        "reported row count",
        FOUR_LOOP_NEXT_CLOSED_ROWS,
        stats.rows(),
    )?;
    check_source_census(
        "reported column count",
        FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS,
        stats.global_columns(),
    )?;
    check_source_census(
        "reported retained entry count",
        FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRIES,
        stats.collected_entries(),
    )?;
    check_source_census(
        "reported maximum row width",
        FOUR_LOOP_NEXT_CLOSED_ROWS_MAX_ROW_WIDTH,
        stats.max_row_width(),
    )?;
    check_source_census(
        "reported zero-row count",
        FOUR_LOOP_NEXT_CLOSED_ROWS_ZERO_ROWS,
        stats.zero_rows(),
    )?;

    Ok(IndexedSource {
        rows,
        entries,
        maximum_row_width,
    })
}

fn check_source_census(
    resource: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), FourLoopNextEliminationError> {
    if expected == actual {
        Ok(())
    } else {
        Err(FourLoopNextEliminationError::SourceCensusMismatch {
            resource,
            expected,
            actual,
        })
    }
}

fn authenticate_modular_discovery(
    closed: &FourLoopNextClosedRows<'_, '_, '_>,
    report: &FourLoopNextModularRankReport,
) -> Result<(usize, Vec<(usize, usize)>), FourLoopNextEliminationError> {
    if report.source_checksum() != closed.checksum()
        || report.images().len() != FOUR_LOOP_NEXT_MODULAR_DISCOVERY_IMAGES.len()
        || !report
            .images()
            .iter()
            .map(|image| image.image())
            .eq(FOUR_LOOP_NEXT_MODULAR_DISCOVERY_IMAGES)
    {
        return Err(FourLoopNextEliminationError::ModularImageSetMismatch);
    }
    if !report.ranks_agree() || !report.pivot_columns_agree() || !report.pivot_skeletons_agree() {
        return Err(FourLoopNextEliminationError::ModularEvidenceDisagrees {
            ranks: report.ranks_agree(),
            pivot_columns: report.pivot_columns_agree(),
            source_row_skeleton: report.pivot_skeletons_agree(),
        });
    }

    let rank = report
        .common_modular_rank()
        .filter(|rank| *rank != 0)
        .ok_or(FourLoopNextEliminationError::VacuousModularCandidate)?;
    let first = report
        .images()
        .first()
        .ok_or(FourLoopNextEliminationError::VacuousModularCandidate)?;
    let skeleton = first
        .pivots()
        .iter()
        .map(|pivot| (pivot.source_row_index(), pivot.column_index()))
        .collect::<Vec<_>>();
    validate_candidate_skeleton(rank, &skeleton, closed.rows().len(), closed.columns().len())?;
    for image in report.images() {
        if image.rank() != rank
            || !image.pivots().iter().enumerate().all(|(ordinal, pivot)| {
                pivot.step() == ordinal
                    && skeleton.get(ordinal)
                        == Some(&(pivot.source_row_index(), pivot.column_index()))
            })
        {
            return Err(FourLoopNextEliminationError::ModularEvidenceDisagrees {
                ranks: false,
                pivot_columns: false,
                source_row_skeleton: false,
            });
        }
    }
    Ok((rank, skeleton))
}

fn validate_candidate_skeleton(
    rank: usize,
    skeleton: &[(usize, usize)],
    row_count: usize,
    column_count: usize,
) -> Result<(), FourLoopNextEliminationError> {
    if rank == 0 || skeleton.len() != rank {
        return Err(FourLoopNextEliminationError::InvalidModularSkeleton {
            reason: "the skeleton length must equal a nonzero candidate rank",
        });
    }
    if skeleton
        .iter()
        .any(|&(row, column)| column >= column_count || row >= row_count)
    {
        return Err(FourLoopNextEliminationError::InvalidModularSkeleton {
            reason: "a pivot column or source row is out of range",
        });
    }
    if !skeleton.windows(2).all(|pair| pair[0].1 > pair[1].1) {
        return Err(FourLoopNextEliminationError::InvalidModularSkeleton {
            reason: "pivot columns are not strictly hardest-first",
        });
    }
    let unique_rows = skeleton
        .iter()
        .map(|&(row, _)| row)
        .collect::<BTreeSet<_>>();
    if unique_rows.len() != skeleton.len() {
        return Err(FourLoopNextEliminationError::InvalidModularSkeleton {
            reason: "a source-row slot is reused by more than one pivot",
        });
    }
    Ok(())
}

fn authenticate_exact_result(
    exact: &ExactSparseElimination,
    modular_rank: usize,
    pivot_skeleton: &[(usize, usize)],
) -> Result<(), FourLoopNextEliminationError> {
    if exact.source_row_count() != FOUR_LOOP_NEXT_CLOSED_ROWS
        || exact.column_count() != FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS
    {
        return Err(FourLoopNextEliminationError::ExactShapeMismatch {
            rows: exact.source_row_count(),
            columns: exact.column_count(),
        });
    }
    if exact.rank() != modular_rank {
        return Err(FourLoopNextEliminationError::ExactRankMismatch {
            modular_candidate: modular_rank,
            exact: exact.rank(),
        });
    }
    if exact.pivot_rules().len() != pivot_skeleton.len() {
        return Err(FourLoopNextEliminationError::ExactRankMismatch {
            modular_candidate: pivot_skeleton.len(),
            exact: exact.pivot_rules().len(),
        });
    }
    for (ordinal, (rule, &(expected_source_row, expected_column))) in
        exact.pivot_rules().iter().zip(pivot_skeleton).enumerate()
    {
        if rule.ordinal() != ordinal
            || rule.pivot_column() != expected_column
            || rule.source_row_index() != expected_source_row
        {
            return Err(FourLoopNextEliminationError::ExactSkeletonMismatch {
                ordinal,
                expected_column,
                actual_column: rule.pivot_column(),
                expected_source_row,
                actual_source_row: rule.source_row_index(),
            });
        }
    }
    Ok(())
}

struct ProjectedResult {
    pivots: Vec<FourLoopNextEliminationPivotRule>,
    free_unresolved_columns: Vec<FourLoopCornerColumnId>,
    retention: ProjectedRetentionCensus,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ProjectedRetentionCensus {
    pivots: usize,
    rhs_entries: usize,
    trace_reductions: usize,
    coefficient_terms: usize,
    coefficient_bytes: usize,
}

struct ProjectedRetentionCharge {
    config: FourLoopNextEliminationConfig,
    census: ProjectedRetentionCensus,
}

impl ProjectedRetentionCharge {
    fn new(config: FourLoopNextEliminationConfig) -> Self {
        Self {
            config,
            census: ProjectedRetentionCensus::default(),
        }
    }

    fn charge_pivot(&mut self) -> Result<(), FourLoopNextEliminationError> {
        let requested = self.census.pivots.checked_add(1).ok_or(
            FourLoopNextEliminationError::ArithmeticOverflow {
                resource: "projected pivots",
            },
        )?;
        check_resource(
            "projected pivots",
            requested,
            self.config.max_projected_pivots,
        )?;
        self.census.pivots = requested;
        Ok(())
    }

    fn charge_rhs_entry(
        &mut self,
        coefficient: &Coefficient,
    ) -> Result<(), FourLoopNextEliminationError> {
        let requested = self.census.rhs_entries.checked_add(1).ok_or(
            FourLoopNextEliminationError::ArithmeticOverflow {
                resource: "projected right-hand-side entries",
            },
        )?;
        check_resource(
            "projected right-hand-side entries",
            requested,
            self.config.max_projected_rhs_entries,
        )?;
        self.charge_coefficient(coefficient)?;
        self.census.rhs_entries = requested;
        Ok(())
    }

    fn charge_trace_reduction(
        &mut self,
        coefficient: &Coefficient,
    ) -> Result<(), FourLoopNextEliminationError> {
        let requested = self.census.trace_reductions.checked_add(1).ok_or(
            FourLoopNextEliminationError::ArithmeticOverflow {
                resource: "projected trace reductions",
            },
        )?;
        check_resource(
            "projected trace reductions",
            requested,
            self.config.max_projected_trace_reductions,
        )?;
        self.charge_coefficient(coefficient)?;
        self.census.trace_reductions = requested;
        Ok(())
    }

    fn charge_divisor(
        &mut self,
        coefficient: &Coefficient,
    ) -> Result<(), FourLoopNextEliminationError> {
        self.charge_coefficient(coefficient)
    }

    fn charge_coefficient(
        &mut self,
        coefficient: &Coefficient,
    ) -> Result<(), FourLoopNextEliminationError> {
        let coefficient_terms = coefficient
            .numerator
            .nterms()
            .checked_add(coefficient.denominator.nterms())
            .ok_or(FourLoopNextEliminationError::ArithmeticOverflow {
                resource: "projected coefficient terms",
            })?;
        let requested_terms = self
            .census
            .coefficient_terms
            .checked_add(coefficient_terms)
            .ok_or(FourLoopNextEliminationError::ArithmeticOverflow {
                resource: "projected coefficient terms",
            })?;
        check_resource(
            "projected coefficient terms",
            requested_terms,
            self.config.max_projected_coefficient_terms,
        )?;
        let coefficient_bytes = bounded_display_len(
            coefficient,
            self.census.coefficient_bytes,
            self.config.max_projected_coefficient_bytes,
            "projected coefficient bytes",
        )?;
        let requested_bytes = self
            .census
            .coefficient_bytes
            .checked_add(coefficient_bytes)
            .ok_or(FourLoopNextEliminationError::ArithmeticOverflow {
                resource: "projected coefficient bytes",
            })?;
        check_resource(
            "projected coefficient bytes",
            requested_bytes,
            self.config.max_projected_coefficient_bytes,
        )?;
        self.census.coefficient_terms = requested_terms;
        self.census.coefficient_bytes = requested_bytes;
        Ok(())
    }
}

fn project_exact_result(
    closed: &FourLoopNextClosedRows<'_, '_, '_>,
    coefficient_context: &CoefficientContext,
    exact: &ExactSparseElimination,
    config: FourLoopNextEliminationConfig,
) -> Result<ProjectedResult, FourLoopNextEliminationError> {
    let one = coefficient_context.one();
    let mut retention = ProjectedRetentionCharge::new(config);
    let mut pivots = Vec::new();
    pivots
        .try_reserve_exact(exact.pivot_rules().len())
        .map_err(|_| FourLoopNextEliminationError::ResourceLimit {
            resource: "projected pivot storage",
            requested: exact.pivot_rules().len() as u128,
            limit: config.max_projected_pivots as u128,
        })?;
    for (ordinal, rule) in exact.pivot_rules().iter().enumerate() {
        if rule.ordinal() != ordinal {
            return Err(FourLoopNextEliminationError::InvalidExactRule {
                ordinal,
                reason: "stored ordinal does not match rule order",
            });
        }
        retention.charge_pivot()?;
        let pivot_index = rule.pivot_column();
        let pivot = closed.columns().get(pivot_index).ok_or(
            FourLoopNextEliminationError::InvalidExactRule {
                ordinal,
                reason: "pivot column is outside the typed catalog",
            },
        )?;
        if rule.row().get(&pivot_index) != Some(&one) {
            return Err(FourLoopNextEliminationError::InvalidExactRule {
                ordinal,
                reason: "unit pivot coefficient is not exactly one",
            });
        }

        let mut rhs = BTreeMap::new();
        for (&column_index, coefficient) in rule.row() {
            if column_index == pivot_index {
                continue;
            }
            if column_index >= pivot_index {
                return Err(FourLoopNextEliminationError::InvalidExactRule {
                    ordinal,
                    reason: "right-hand side is not strictly easier than the pivot",
                });
            }
            let column = closed.columns().get(column_index).cloned().ok_or(
                FourLoopNextEliminationError::InvalidExactRule {
                    ordinal,
                    reason: "right-hand-side column is outside the typed catalog",
                },
            )?;
            let coefficient = -coefficient.clone();
            if coefficient.is_zero() {
                return Err(FourLoopNextEliminationError::InvalidExactRule {
                    ordinal,
                    reason: "right-hand side contains a zero typed coefficient",
                });
            }
            retention.charge_rhs_entry(&coefficient)?;
            if rhs.insert(column, coefficient).is_some() {
                return Err(FourLoopNextEliminationError::InvalidExactRule {
                    ordinal,
                    reason: "right-hand side contains a duplicate typed column",
                });
            }
        }

        let trace = rule.trace();
        if trace.base_source_row_index() != rule.source_row_index()
            || trace.base_source_row_index() >= closed.rows().len()
        {
            return Err(FourLoopNextEliminationError::InvalidExactRule {
                ordinal,
                reason: "trace base does not identify the pivot source-row slot",
            });
        }
        if trace.divisor().is_zero() {
            return Err(FourLoopNextEliminationError::InvalidExactRule {
                ordinal,
                reason: "trace divisor is zero",
            });
        }
        let mut reductions = Vec::new();
        let requested_trace_reductions = retention
            .census
            .trace_reductions
            .checked_add(trace.reductions().len())
            .ok_or(FourLoopNextEliminationError::ArithmeticOverflow {
                resource: "projected trace reductions",
            })?;
        check_resource(
            "projected trace reductions",
            requested_trace_reductions,
            config.max_projected_trace_reductions,
        )?;
        reductions
            .try_reserve_exact(trace.reductions().len())
            .map_err(|_| FourLoopNextEliminationError::ResourceLimit {
                resource: "projected trace reduction storage",
                requested: trace.reductions().len() as u128,
                limit: config.max_projected_trace_reductions as u128,
            })?;
        for reduction in trace.reductions() {
            let prior_ordinal = reduction.prior_pivot_ordinal();
            if prior_ordinal >= ordinal {
                return Err(FourLoopNextEliminationError::InvalidExactRule {
                    ordinal,
                    reason: "trace references a non-prior pivot",
                });
            }
            if reduction.factor().is_zero() {
                return Err(FourLoopNextEliminationError::InvalidExactRule {
                    ordinal,
                    reason: "trace retains a zero reduction factor",
                });
            }
            let prior_pivot = pivots
                .get(prior_ordinal)
                .map(|rule: &FourLoopNextEliminationPivotRule| rule.pivot.clone())
                .ok_or(FourLoopNextEliminationError::InvalidExactRule {
                    ordinal,
                    reason: "trace prior-pivot ordinal is missing",
                })?;
            retention.charge_trace_reduction(reduction.factor())?;
            reductions.push(FourLoopNextEliminationTraceReduction {
                prior_pivot_ordinal: prior_ordinal,
                prior_pivot,
                factor: reduction.factor().clone(),
            });
        }
        retention.charge_divisor(trace.divisor())?;
        pivots.push(FourLoopNextEliminationPivotRule {
            ordinal,
            pivot: pivot.clone(),
            rhs,
            trace: FourLoopNextEliminationTrace {
                base_source_row_index: trace.base_source_row_index(),
                base_source_raw_id: closed.rows()[trace.base_source_row_index()].raw_id(),
                reductions,
                divisor: trace.divisor().clone(),
            },
        });
    }

    if exact
        .free_columns()
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(FourLoopNextEliminationError::InvalidExactRule {
            ordinal: exact.pivot_rules().len(),
            reason: "free-column indices are not strictly ordered",
        });
    }
    let mut free_unresolved_columns = Vec::new();
    free_unresolved_columns
        .try_reserve_exact(exact.free_columns().len())
        .map_err(|_| FourLoopNextEliminationError::ResourceLimit {
            resource: "projected free-column storage",
            requested: exact.free_columns().len() as u128,
            limit: FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS as u128,
        })?;
    for &column in exact.free_columns() {
        let column = closed.columns().get(column).cloned().ok_or(
            FourLoopNextEliminationError::InvalidExactRule {
                ordinal: exact.pivot_rules().len(),
                reason: "free column is outside the typed catalog",
            },
        )?;
        free_unresolved_columns.push(column);
    }

    let pivot_indices = exact
        .pivot_rules()
        .iter()
        .map(|rule| rule.pivot_column())
        .collect::<BTreeSet<_>>();
    let free_indices = exact
        .free_columns()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let expected_free = (0..closed.columns().len())
        .filter(|column| !pivot_indices.contains(column))
        .collect::<BTreeSet<_>>();
    if pivot_indices.len() != exact.rank()
        || !pivot_indices.is_disjoint(&free_indices)
        || free_indices != expected_free
    {
        return Err(FourLoopNextEliminationError::InvalidExactRule {
            ordinal: exact.pivot_rules().len(),
            reason: "pivot and free columns do not partition the typed catalog",
        });
    }

    Ok(ProjectedResult {
        pivots,
        free_unresolved_columns,
        retention: retention.census,
    })
}

fn condition_census(
    closed: &FourLoopNextClosedRows<'_, '_, '_>,
    pivots: &[FourLoopNextEliminationPivotRule],
) -> Result<FourLoopNextEliminationConditions, FourLoopNextEliminationError> {
    let parent_row_scale_slots = closed.rows().len();
    let parent_coefficient_denominator_slots = closed
        .rows()
        .iter()
        .try_fold(0_usize, |sum, row| sum.checked_add(row.entries().len()))
        .ok_or(FourLoopNextEliminationError::ArithmeticOverflow {
            resource: "parent coefficient denominator slots",
        })?;
    let trace_divisor_slots = pivots.len();
    let trace_factor_denominator_slots = pivots
        .iter()
        .try_fold(0_usize, |sum, rule| {
            sum.checked_add(rule.trace.reductions.len())
        })
        .ok_or(FourLoopNextEliminationError::ArithmeticOverflow {
            resource: "trace factor denominator slots",
        })?;
    let rule_rhs_denominator_slots = pivots
        .iter()
        .try_fold(0_usize, |sum, rule| sum.checked_add(rule.rhs.len()))
        .ok_or(FourLoopNextEliminationError::ArithmeticOverflow {
            resource: "rule right-hand-side denominator slots",
        })?;
    let total_slots = [
        parent_row_scale_slots,
        parent_coefficient_denominator_slots,
        trace_divisor_slots,
        trace_factor_denominator_slots,
        rule_rhs_denominator_slots,
    ]
    .into_iter()
    .try_fold(0_usize, |sum, count| sum.checked_add(count))
    .ok_or(FourLoopNextEliminationError::ArithmeticOverflow {
        resource: "conservative condition slots",
    })?;
    Ok(FourLoopNextEliminationConditions {
        parent_row_scale_slots,
        parent_coefficient_denominator_slots,
        trace_divisor_slots,
        trace_factor_denominator_slots,
        rule_rhs_denominator_slots,
        total_slots,
    })
}

fn adapter_stats(
    source: &IndexedSource,
    modular: &FourLoopNextModularRankReport,
    modular_rank: usize,
    exact: &ExactSparseElimination,
    projected: &ProjectedResult,
    conditions: FourLoopNextEliminationConditions,
) -> Result<FourLoopNextEliminationStats, FourLoopNextEliminationError> {
    let trace_reductions = projected
        .pivots
        .iter()
        .try_fold(0_usize, |sum, rule| {
            sum.checked_add(rule.trace.reductions.len())
        })
        .ok_or(FourLoopNextEliminationError::ArithmeticOverflow {
            resource: "trace reductions",
        })?;
    let projected_rhs_entries = projected
        .pivots
        .iter()
        .try_fold(0_usize, |sum, rule| sum.checked_add(rule.rhs.len()))
        .ok_or(FourLoopNextEliminationError::ArithmeticOverflow {
            resource: "projected right-hand-side entries",
        })?;
    let maximum_trace_reductions = projected
        .pivots
        .iter()
        .map(|rule| rule.trace.reductions.len())
        .max()
        .unwrap_or(0);
    let exact_stats = exact.stats();
    if exact_stats.source_rows() != source.rows.len()
        || exact_stats.columns() != FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS
        || exact_stats.input_entries() != source.entries
        || exact_stats.rank() != projected.pivots.len()
        || exact_stats.free_columns() != projected.free_unresolved_columns.len()
        || projected.retention.pivots != projected.pivots.len()
        || projected.retention.rhs_entries != projected_rhs_entries
        || projected.retention.trace_reductions != trace_reductions
    {
        return Err(FourLoopNextEliminationError::ReplayMismatch {
            component: "exact-engine statistics",
        });
    }
    Ok(FourLoopNextEliminationStats {
        source_rows: source.rows.len(),
        columns: FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS,
        input_entries: source.entries,
        maximum_input_row_width: source.maximum_row_width,
        modular_images: modular.images().len(),
        modular_candidate_rank: modular_rank,
        exact_rank: projected.pivots.len(),
        pivot_rules: projected.pivots.len(),
        free_unresolved_columns: projected.free_unresolved_columns.len(),
        projected_rhs_entries,
        trace_reductions,
        maximum_trace_reductions,
        projected_coefficient_terms: projected.retention.coefficient_terms,
        projected_coefficient_bytes: projected.retention.coefficient_bytes,
        exact_pivot_reductions: exact_stats.pivot_reductions(),
        exact_verification_reductions: exact_stats.verification_reductions(),
        exact_arithmetic_updates: exact_stats.arithmetic_updates(),
        exact_retained_entries: exact_stats.retained_entries(),
        exact_retained_coefficient_terms: exact_stats.retained_coefficient_terms(),
        exact_retained_coefficient_bytes: exact_stats.retained_coefficient_bytes(),
        exact_maximum_row_width: exact_stats.maximum_row_width(),
        exact_maximum_coefficient_degree: exact_stats.maximum_coefficient_degree(),
        exact_replay_reductions: exact_stats.replay_reductions(),
        exact_replay_updates: exact_stats.replay_updates(),
        conservative_condition_slots: conditions.total_slots,
    })
}

fn certificate_checksum(
    closed: &FourLoopNextClosedRows<'_, '_, '_>,
    config: &FourLoopNextEliminationConfig,
    modular: &FourLoopNextModularRankReport,
    exact: &ExactSparseElimination,
    pivots: &[FourLoopNextEliminationPivotRule],
    free: &[FourLoopCornerColumnId],
    conditions: FourLoopNextEliminationConditions,
    stats: FourLoopNextEliminationStats,
) -> Result<u64, FourLoopNextEliminationError> {
    let mut hash = FNV1A64_OFFSET;
    let mut coefficient_bytes = 0_usize;
    hash_bytes(&mut hash, FOUR_LOOP_NEXT_ELIMINATION_SCHEMA.as_bytes());
    hash_u64(&mut hash, closed.checksum());
    hash_u64(&mut hash, modular.column_catalog_checksum());
    hash_u64(&mut hash, modular.checksum());
    hash_u64(&mut hash, exact.checksum());
    for value in [
        config.modular.max_images,
        config.modular.max_initial_nonzeros,
        config.modular.max_live_nonzeros,
        config.modular.max_cumulative_fill_in,
        config.modular.max_elimination_updates,
        config.exact.max_rows,
        config.exact.max_columns,
        config.exact.max_input_entries,
        config.exact.max_input_coefficient_bytes,
        config.exact.max_reductions,
        config.exact.max_updates,
        config.exact.max_retained_entries,
        config.exact.max_retained_coefficient_terms,
        config.exact.max_retained_coefficient_bytes,
        config.exact.max_coefficient_degree,
        config.exact.max_coefficient_operation_terms,
        config.exact.max_coefficient_dense_terms,
        config.exact.max_replay_reductions,
        config.exact.max_replay_updates,
        config.max_projected_pivots,
        config.max_projected_rhs_entries,
        config.max_projected_trace_reductions,
        config.max_projected_coefficient_terms,
        config.max_projected_coefficient_bytes,
    ] {
        hash_usize(&mut hash, value);
    }
    hash_usize(&mut hash, modular.images().len());
    for image in modular.images() {
        hash_u64(&mut hash, image.image().prime());
        hash_u64(&mut hash, image.image().dimension());
    }
    for rule in pivots {
        hash_usize(&mut hash, rule.ordinal);
        hash_bytes(&mut hash, rule.pivot.stable_key().as_bytes());
        hash_usize(&mut hash, rule.trace.base_source_row_index);
        hash_bytes(
            &mut hash,
            rule.trace.base_source_raw_id.stable_key().as_bytes(),
        );
        for reduction in &rule.trace.reductions {
            hash_usize(&mut hash, reduction.prior_pivot_ordinal);
            hash_bytes(&mut hash, reduction.prior_pivot.stable_key().as_bytes());
            hash_display_bounded(
                &mut hash,
                &reduction.factor,
                &mut coefficient_bytes,
                config.max_projected_coefficient_bytes,
                "projected checksum coefficient bytes",
            )?;
        }
        hash_display_bounded(
            &mut hash,
            &rule.trace.divisor,
            &mut coefficient_bytes,
            config.max_projected_coefficient_bytes,
            "projected checksum coefficient bytes",
        )?;
        for (column, coefficient) in &rule.rhs {
            hash_bytes(&mut hash, column.stable_key().as_bytes());
            hash_display_bounded(
                &mut hash,
                coefficient,
                &mut coefficient_bytes,
                config.max_projected_coefficient_bytes,
                "projected checksum coefficient bytes",
            )?;
        }
        hash_u64(&mut hash, u64::MAX);
    }
    for column in free {
        hash_bytes(&mut hash, column.stable_key().as_bytes());
    }
    for value in [
        conditions.parent_row_scale_slots,
        conditions.parent_coefficient_denominator_slots,
        conditions.trace_divisor_slots,
        conditions.trace_factor_denominator_slots,
        conditions.rule_rhs_denominator_slots,
        conditions.total_slots,
        stats.source_rows,
        stats.columns,
        stats.input_entries,
        stats.maximum_input_row_width,
        stats.modular_images,
        stats.modular_candidate_rank,
        stats.exact_rank,
        stats.pivot_rules,
        stats.free_unresolved_columns,
        stats.projected_rhs_entries,
        stats.trace_reductions,
        stats.maximum_trace_reductions,
        stats.projected_coefficient_terms,
        stats.projected_coefficient_bytes,
        stats.exact_pivot_reductions,
        stats.exact_verification_reductions,
        stats.exact_arithmetic_updates,
        stats.exact_retained_entries,
        stats.exact_retained_coefficient_terms,
        stats.exact_retained_coefficient_bytes,
        stats.exact_maximum_row_width,
        stats.exact_maximum_coefficient_degree,
        stats.exact_replay_reductions,
        stats.exact_replay_updates,
        stats.conservative_condition_slots,
    ] {
        hash_usize(&mut hash, value);
    }
    if coefficient_bytes != stats.projected_coefficient_bytes {
        return Err(FourLoopNextEliminationError::ReplayMismatch {
            component: "projected checksum coefficient-byte census",
        });
    }
    Ok(hash)
}

struct BoundedLengthWriter {
    length: usize,
    limit: usize,
}

impl fmt::Write for BoundedLengthWriter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let requested = self.length.checked_add(value.len()).ok_or(fmt::Error)?;
        if requested > self.limit {
            return Err(fmt::Error);
        }
        self.length = requested;
        Ok(())
    }
}

fn bounded_display_len(
    value: &impl fmt::Display,
    used: usize,
    total_limit: usize,
    resource: &'static str,
) -> Result<usize, FourLoopNextEliminationError> {
    let remaining = total_limit.saturating_sub(used);
    let mut writer = BoundedLengthWriter {
        length: 0,
        limit: remaining,
    };
    write!(&mut writer, "{value}").map_err(|_| FourLoopNextEliminationError::ResourceLimit {
        resource,
        requested: total_limit as u128 + 1,
        limit: total_limit as u128,
    })?;
    Ok(writer.length)
}

struct BoundedHashWriter<'hash> {
    hash: &'hash mut u64,
    length: usize,
    limit: usize,
}

impl fmt::Write for BoundedHashWriter<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let requested = self.length.checked_add(value.len()).ok_or(fmt::Error)?;
        if requested > self.limit {
            return Err(fmt::Error);
        }
        hash_bytes(self.hash, value.as_bytes());
        self.length = requested;
        Ok(())
    }
}

fn hash_display_bounded(
    hash: &mut u64,
    value: &impl fmt::Display,
    used: &mut usize,
    total_limit: usize,
    resource: &'static str,
) -> Result<(), FourLoopNextEliminationError> {
    let remaining = total_limit.saturating_sub(*used);
    let mut writer = BoundedHashWriter {
        hash,
        length: 0,
        limit: remaining,
    };
    write!(&mut writer, "{value}").map_err(|_| FourLoopNextEliminationError::ResourceLimit {
        resource,
        requested: total_limit as u128 + 1,
        limit: total_limit as u128,
    })?;
    *used = (*used).checked_add(writer.length).ok_or(
        FourLoopNextEliminationError::ArithmeticOverflow {
            resource: "projected checksum coefficient bytes",
        },
    )?;
    hash_u64(writer.hash, u64::MAX);
    Ok(())
}

fn hash_usize(hash: &mut u64, value: usize) {
    hash_u64(hash, value as u64);
}

fn hash_u64(hash: &mut u64, value: u64) {
    hash_bytes(hash, &value.to_le_bytes());
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for &byte in bytes {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(FNV1A64_PRIME);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_skeleton_requires_nonempty_strict_hardest_first_unique_rows() {
        assert!(validate_candidate_skeleton(3, &[(2, 8), (0, 5), (3, 1)], 4, 9).is_ok());
        assert!(validate_candidate_skeleton(0, &[], 4, 9).is_err());
        assert!(validate_candidate_skeleton(2, &[(0, 5)], 4, 9).is_err());
        assert!(validate_candidate_skeleton(2, &[(0, 5), (1, 5)], 4, 9).is_err());
        assert!(validate_candidate_skeleton(2, &[(0, 5), (1, 6)], 4, 9).is_err());
        assert!(validate_candidate_skeleton(2, &[(0, 5), (0, 1)], 4, 9).is_err());
        assert!(validate_candidate_skeleton(1, &[(0, 9)], 4, 9).is_err());
        assert!(validate_candidate_skeleton(1, &[(4, 8)], 4, 9).is_err());
    }

    #[test]
    fn condition_inventory_never_claims_exceptional_dimension_completeness() {
        let conditions = FourLoopNextEliminationConditions::default();
        assert_eq!(
            conditions.status(),
            FourLoopNextEliminationConditionStatus::ConservativeUnfactoredInversionSlotCensusOnly
        );
        assert!(!conditions.is_complete_exceptional_dimension_inventory());
    }

    #[test]
    fn preflight_rejects_impossible_fixed_census_and_degree_caps() {
        let mut config = FourLoopNextEliminationConfig::default();
        config.exact.max_rows = FOUR_LOOP_NEXT_CLOSED_ROWS - 1;
        assert!(matches!(
            preflight_config(config),
            Err(FourLoopNextEliminationError::ResourceLimit {
                resource: "configured exact source rows",
                requested,
                limit,
            }) if requested == FOUR_LOOP_NEXT_CLOSED_ROWS as u128
                && limit == (FOUR_LOOP_NEXT_CLOSED_ROWS - 1) as u128
        ));

        let mut config = FourLoopNextEliminationConfig::default();
        config.exact.max_coefficient_degree =
            usize::try_from(SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT + 1).unwrap();
        assert!(matches!(
            preflight_config(config),
            Err(FourLoopNextEliminationError::ResourceLimit {
                resource: "configured exact coefficient exponent degree",
                ..
            })
        ));

        let mut config = FourLoopNextEliminationConfig::default();
        config.max_projected_pivots = 0;
        assert!(matches!(
            preflight_config(config),
            Err(FourLoopNextEliminationError::ResourceLimit {
                resource: "configured projected pivots",
                ..
            })
        ));
    }

    #[test]
    fn bounded_streaming_hash_never_allocates_a_display_string() {
        let coefficient = CoefficientContext::new(["d"]).one();
        assert_eq!(
            bounded_display_len(&coefficient, 0, 1, "test coefficient bytes").unwrap(),
            1
        );
        assert!(matches!(
            bounded_display_len(&coefficient, 0, 0, "test coefficient bytes"),
            Err(FourLoopNextEliminationError::ResourceLimit {
                resource: "test coefficient bytes",
                requested: 1,
                limit: 0,
            })
        ));

        let mut hash = FNV1A64_OFFSET;
        let mut used = 0_usize;
        hash_display_bounded(
            &mut hash,
            &coefficient,
            &mut used,
            1,
            "test checksum coefficient bytes",
        )
        .unwrap();
        assert_eq!(used, 1);

        let mut hash = FNV1A64_OFFSET;
        let mut used = 0_usize;
        assert!(matches!(
            hash_display_bounded(
                &mut hash,
                &coefficient,
                &mut used,
                0,
                "test checksum coefficient bytes",
            ),
            Err(FourLoopNextEliminationError::ResourceLimit {
                resource: "test checksum coefficient bytes",
                requested: 1,
                limit: 0,
            })
        ));
    }
}
