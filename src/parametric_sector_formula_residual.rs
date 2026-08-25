//! Bounded lazy residual paths over normalized sector-coverage formulas.
//!
//! This backend walks the authenticated backend-neutral formula IR directly.
//! It does not construct an MTBDD, a visited set, or a materialized partition
//! of all residual cubes.  The only search state is one dense three-valued
//! assignment table and the current root-to-leaf DFS frontier.

use std::fmt;
use std::mem::size_of;
use std::sync::Arc;

use crate::direct_bad_formula::{
    DirectBadFormulaClause, DirectBadFormulaRoute, DirectBadFormulaTruth, route_direct_bad_formula,
};
use crate::parametric_sector_formula_ir::{
    NormalizedBadFormulaBody, NormalizedBadLiteral, NormalizedBadLiteralPolarity,
    NormalizedCoverageAttempt, NormalizedCoverageIr, PARAMETRIC_SECTOR_FORMULA_IR_V1_SCHEMA,
};
use crate::parametric_sector_normalized_source::{
    PARAMETRIC_SECTOR_NORMALIZED_COVERAGE_SOURCE_V2_SCHEMA,
    ParametricSectorNormalizedCoverageSource, ParametricSectorNormalizedCoverageSourceError,
};
use crate::{IntegralFamily, ParametricCoefficientContext, ParametricPolynomial};

pub(crate) const PARAMETRIC_SECTOR_FORMULA_RESIDUAL_CURSOR_V1_SCHEMA: &str =
    "rustred-parametric-sector-formula-residual-cursor-v1";
pub(crate) const PARAMETRIC_SECTOR_FORMULA_RESIDUAL_PATH_V1_SCHEMA: &str =
    "rustred-parametric-sector-formula-residual-path-v1";
pub(crate) const PARAMETRIC_SECTOR_FORMULA_RESIDUAL_BRANCH_ORDER_V1: &str = "rustred-parametric-sector-formula-residual-earliest-attempt-clause-left-literal-nonzero-before-equal-zero-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ParametricSectorFormulaResidualRequest {
    AnyResidual,
    Uncovered,
    Unsupported,
}

impl ParametricSectorFormulaResidualRequest {
    const fn accepts(self, kind: ParametricSectorFormulaResidualKind) -> bool {
        matches!(self, Self::AnyResidual)
            || matches!(
                (self, kind),
                (
                    Self::Uncovered,
                    ParametricSectorFormulaResidualKind::Uncovered
                ) | (
                    Self::Unsupported,
                    ParametricSectorFormulaResidualKind::Unsupported
                )
            )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ParametricSectorFormulaResidualKind {
    Uncovered,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ParametricSectorFormulaResidualPolarity {
    NonZero,
    EqualZero,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ParametricSectorFormulaResidualSplitLocator {
    source_attempt_ordinal: usize,
    clause_ordinal: usize,
    literal_position: u8,
    structural_locus_ordinal: usize,
    bad_literal_polarity: NormalizedBadLiteralPolarity,
}

impl ParametricSectorFormulaResidualSplitLocator {
    pub(crate) const fn source_attempt_ordinal(self) -> usize {
        self.source_attempt_ordinal
    }

    pub(crate) const fn clause_ordinal(self) -> usize {
        self.clause_ordinal
    }

    pub(crate) const fn literal_position(self) -> u8 {
        self.literal_position
    }

    pub(crate) const fn structural_locus_ordinal(self) -> usize {
        self.structural_locus_ordinal
    }

    pub(crate) const fn bad_literal_polarity(self) -> NormalizedBadLiteralPolarity {
        self.bad_literal_polarity
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ParametricSectorFormulaResidualDecision {
    split: ParametricSectorFormulaResidualSplitLocator,
    polarity: ParametricSectorFormulaResidualPolarity,
}

impl ParametricSectorFormulaResidualDecision {
    pub(crate) const fn split(self) -> ParametricSectorFormulaResidualSplitLocator {
        self.split
    }

    pub(crate) const fn structural_locus_ordinal(self) -> usize {
        self.split.structural_locus_ordinal
    }

    pub(crate) const fn polarity(self) -> ParametricSectorFormulaResidualPolarity {
        self.polarity
    }
}

/// Independent resource envelope for direct routing and one retained path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ParametricSectorFormulaResidualLimits {
    pub(crate) max_base_structural_loci: usize,
    pub(crate) max_attempts: usize,
    pub(crate) max_certified_attempts: usize,
    pub(crate) max_unsupported_candidate_references: usize,
    pub(crate) max_assignment_capacity_entries: usize,
    pub(crate) max_state_classifications: usize,
    pub(crate) max_attempt_visits: usize,
    pub(crate) max_formula_evaluations: usize,
    pub(crate) max_formula_clause_charges: usize,
    pub(crate) max_literal_query_charges: usize,
    pub(crate) max_good_routes: usize,
    pub(crate) max_bad_routes: usize,
    pub(crate) max_split_routes: usize,
    pub(crate) max_later_good_prunes: usize,
    pub(crate) max_covered_prunes: usize,
    pub(crate) max_residual_terminal_visits: usize,
    pub(crate) max_filtered_residual_terminals: usize,
    pub(crate) max_branch_traversals: usize,
    pub(crate) max_backtracks: usize,
    pub(crate) max_depth: usize,
    pub(crate) max_frontier_capacity_entries: usize,
    pub(crate) max_cursor_retained_bytes: usize,
    pub(crate) max_paths_yielded: usize,
    pub(crate) max_total_path_decisions_copied: usize,
    pub(crate) max_path_decisions: usize,
    pub(crate) max_path_capacity_entries: usize,
    pub(crate) max_path_retained_bytes: usize,
}

impl Default for ParametricSectorFormulaResidualLimits {
    fn default() -> Self {
        Self {
            max_base_structural_loci: 16_000_000,
            max_attempts: 1_000_000,
            max_certified_attempts: 1_000_000,
            max_unsupported_candidate_references: 1_000_000,
            max_assignment_capacity_entries: 16_000_000,
            max_state_classifications: 256_000_000,
            max_attempt_visits: 1_000_000_000,
            max_formula_evaluations: 1_000_000_000,
            max_formula_clause_charges: 4_000_000_000,
            max_literal_query_charges: 8_000_000_000,
            max_good_routes: 256_000_000,
            max_bad_routes: 1_000_000_000,
            max_split_routes: 1_000_000_000,
            max_later_good_prunes: 256_000_000,
            max_covered_prunes: 256_000_000,
            max_residual_terminal_visits: 256_000_000,
            max_filtered_residual_terminals: 16_000_000,
            max_branch_traversals: 256_000_000,
            max_backtracks: 256_000_000,
            max_depth: 16_000_000,
            max_frontier_capacity_entries: 16_000_000,
            max_cursor_retained_bytes: 1024 * 1024 * 1024,
            max_paths_yielded: 16_000_000,
            max_total_path_decisions_copied: 256_000_000,
            max_path_decisions: 16_000_000,
            max_path_capacity_entries: 16_000_000,
            max_path_retained_bytes: 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ParametricSectorFormulaResidualCursorStats {
    source_references: usize,
    base_structural_loci: usize,
    attempts: usize,
    certified_attempts: usize,
    unsupported_attempts: usize,
    assignment_capacity_entries: usize,
    state_classifications: usize,
    attempt_visits: usize,
    formula_evaluations: usize,
    formula_clause_charges: usize,
    literal_query_charges: usize,
    good_routes: usize,
    bad_routes: usize,
    split_routes: usize,
    later_good_prunes: usize,
    covered_prunes: usize,
    residual_terminal_visits: usize,
    uncovered_terminals_visited: usize,
    unsupported_terminals_visited: usize,
    filtered_residual_terminals: usize,
    branch_traversals: usize,
    backtracks: usize,
    maximum_depth: usize,
    peak_frontier_capacity_entries: usize,
    peak_cursor_retained_bytes: usize,
    paths_yielded: usize,
    total_path_decisions_copied: usize,
}

macro_rules! cursor_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl ParametricSectorFormulaResidualCursorStats {
    cursor_stats_getters!(
        source_references,
        base_structural_loci,
        attempts,
        certified_attempts,
        unsupported_attempts,
        assignment_capacity_entries,
        state_classifications,
        attempt_visits,
        formula_evaluations,
        formula_clause_charges,
        literal_query_charges,
        good_routes,
        bad_routes,
        split_routes,
        later_good_prunes,
        covered_prunes,
        residual_terminal_visits,
        uncovered_terminals_visited,
        unsupported_terminals_visited,
        filtered_residual_terminals,
        branch_traversals,
        backtracks,
        maximum_depth,
        peak_frontier_capacity_entries,
        peak_cursor_retained_bytes,
        paths_yielded,
        total_path_decisions_copied,
    );
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ParametricSectorFormulaResidualPathStats {
    decisions: usize,
    decision_capacity_entries: usize,
    nonzero_decisions: usize,
    equal_zero_decisions: usize,
    unsupported_candidate_references: usize,
    retained_path_bytes: usize,
}

impl ParametricSectorFormulaResidualPathStats {
    pub(crate) const fn decisions(self) -> usize {
        self.decisions
    }

    pub(crate) const fn decision_capacity_entries(self) -> usize {
        self.decision_capacity_entries
    }

    pub(crate) const fn nonzero_decisions(self) -> usize {
        self.nonzero_decisions
    }

    pub(crate) const fn equal_zero_decisions(self) -> usize {
        self.equal_zero_decisions
    }

    pub(crate) const fn unsupported_candidate_references(self) -> usize {
        self.unsupported_candidate_references
    }

    pub(crate) const fn retained_path_bytes(self) -> usize {
        self.retained_path_bytes
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ParametricSectorFormulaResidualError {
    SourceSchemaMismatch,
    SourceReplay(ParametricSectorNormalizedCoverageSourceError),
    SourceShapeMismatch,
    StructuralLocusOutOfRange {
        ordinal: usize,
        locus_count: usize,
    },
    SplitClauseOutOfRange {
        attempt_ordinal: usize,
        clause_ordinal: usize,
        clause_count: usize,
    },
    SplitLiteralMismatch {
        attempt_ordinal: usize,
        clause_ordinal: usize,
    },
    SplitLocusAlreadyAssigned {
        ordinal: usize,
    },
    PathReplayMismatch,
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
    CursorPoisoned,
}

impl fmt::Display for ParametricSectorFormulaResidualError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "parametric sector formula-residual error: {self:?}"
        )
    }
}

impl std::error::Error for ParametricSectorFormulaResidualError {}

impl From<ParametricSectorNormalizedCoverageSourceError> for ParametricSectorFormulaResidualError {
    fn from(value: ParametricSectorNormalizedCoverageSourceError) -> Self {
        Self::SourceReplay(value)
    }
}

/// One process-local exact residual conjunction over the shared normalized
/// source. Unsupported attempt ordinals remain source-backed and are not
/// cloned into each path.
#[derive(Clone)]
pub(crate) struct ParametricSectorFormulaResidualPathCertificate {
    schema: &'static str,
    branch_order_schema: &'static str,
    source: Arc<ParametricSectorNormalizedCoverageSource>,
    request: ParametricSectorFormulaResidualRequest,
    yield_ordinal: usize,
    decisions: Vec<ParametricSectorFormulaResidualDecision>,
    terminal_kind: ParametricSectorFormulaResidualKind,
    limits: ParametricSectorFormulaResidualLimits,
    stats: ParametricSectorFormulaResidualPathStats,
}

impl fmt::Debug for ParametricSectorFormulaResidualPathCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ParametricSectorFormulaResidualPathCertificate")
            .field("schema", &self.schema)
            .field("branch_order_schema", &self.branch_order_schema)
            .field("request", &self.request)
            .field("yield_ordinal", &self.yield_ordinal)
            .field("decisions", &self.decisions)
            .field("terminal_kind", &self.terminal_kind)
            .field("stats", &self.stats)
            .field("source", &"<shared normalized sector coverage>")
            .finish()
    }
}

impl ParametricSectorFormulaResidualPathCertificate {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) const fn branch_order_schema(&self) -> &'static str {
        self.branch_order_schema
    }

    pub(crate) const fn request(&self) -> ParametricSectorFormulaResidualRequest {
        self.request
    }

    /// One-based ordinal among paths accepted by this request.
    pub(crate) const fn yield_ordinal(&self) -> usize {
        self.yield_ordinal
    }

    pub(crate) fn decisions(&self) -> &[ParametricSectorFormulaResidualDecision] {
        &self.decisions
    }

    pub(crate) const fn terminal_kind(&self) -> ParametricSectorFormulaResidualKind {
        self.terminal_kind
    }

    pub(crate) const fn limits(&self) -> ParametricSectorFormulaResidualLimits {
        self.limits
    }

    pub(crate) const fn stats(&self) -> ParametricSectorFormulaResidualPathStats {
        self.stats
    }

    pub(crate) const fn source_arc(&self) -> &Arc<ParametricSectorNormalizedCoverageSource> {
        &self.source
    }

    pub(crate) fn same_source_allocation(
        &self,
        source: &Arc<ParametricSectorNormalizedCoverageSource>,
    ) -> bool {
        Arc::ptr_eq(&self.source, source)
    }

    pub(crate) fn structural_locus(
        &self,
        decision_ordinal: usize,
    ) -> Option<&ParametricPolynomial> {
        let locus = self
            .decisions
            .get(decision_ordinal)?
            .structural_locus_ordinal();
        self.source.normalized().base_structural_loci().get(locus)
    }

    pub(crate) fn nonzero_locus_ordinals(&self) -> impl Iterator<Item = usize> + '_ {
        self.decisions.iter().filter_map(|decision| {
            (decision.polarity == ParametricSectorFormulaResidualPolarity::NonZero)
                .then_some(decision.structural_locus_ordinal())
        })
    }

    pub(crate) fn equal_zero_locus_ordinals(&self) -> impl Iterator<Item = usize> + '_ {
        self.decisions.iter().filter_map(|decision| {
            (decision.polarity == ParametricSectorFormulaResidualPolarity::EqualZero)
                .then_some(decision.structural_locus_ordinal())
        })
    }

    pub(crate) fn unsupported_candidate_ordinals(&self) -> impl Iterator<Item = usize> + '_ {
        self.source
            .normalized()
            .ir()
            .attempts()
            .iter()
            .filter_map(|attempt| match attempt {
                NormalizedCoverageAttempt::Unsupported {
                    source_attempt_ordinal,
                } => Some(*source_attempt_ordinal),
                NormalizedCoverageAttempt::Certified(_) => None,
            })
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), ParametricSectorFormulaResidualError> {
        if self.schema != PARAMETRIC_SECTOR_FORMULA_RESIDUAL_PATH_V1_SCHEMA
            || self.branch_order_schema != PARAMETRIC_SECTOR_FORMULA_RESIDUAL_BRANCH_ORDER_V1
            || self.yield_ordinal == 0
        {
            return Err(ParametricSectorFormulaResidualError::PathReplayMismatch);
        }
        let expected_stats = path_stats(
            &self.decisions,
            self.stats.decision_capacity_entries,
            self.unsupported_candidate_ordinals().count(),
            self.limits,
        )?;
        if self.decisions.capacity() > self.stats.decision_capacity_entries
            || expected_stats != self.stats
        {
            return Err(ParametricSectorFormulaResidualError::PathReplayMismatch);
        }
        self.source.replay(family, context)?;
        let mut cursor = ParametricSectorFormulaResidualCursor::from_replayed_source(
            Arc::clone(&self.source),
            self.request,
            self.limits,
        )?;
        let mut replayed = None;
        for _ in 0..self.yield_ordinal {
            replayed = cursor.next_path()?;
            if replayed.is_none() {
                return Err(ParametricSectorFormulaResidualError::PathReplayMismatch);
            }
        }
        if replayed
            .as_ref()
            .is_some_and(|candidate| self.payload_eq(candidate))
        {
            Ok(())
        } else {
            Err(ParametricSectorFormulaResidualError::PathReplayMismatch)
        }
    }

    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.branch_order_schema == other.branch_order_schema
            && Arc::ptr_eq(&self.source, &other.source)
            && self.request == other.request
            && self.yield_ordinal == other.yield_ordinal
            && self.decisions == other.decisions
            && self.terminal_kind == other.terminal_kind
            && self.limits == other.limits
            && self.stats == other.stats
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum FormulaResidualLocusAssignment {
    #[default]
    Unknown,
    NonZero,
    EqualZero,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FormulaResidualStateClassification {
    Covered,
    Split(ParametricSectorFormulaResidualSplitLocator),
    Residual(ParametricSectorFormulaResidualKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NormalizedFormulaRoute {
    Bad,
    Good,
    Split {
        clause_ordinal: usize,
        atom: NormalizedBadLiteral,
    },
}

/// Resumable nonzero-first DFS. A resource failure poisons only this cursor;
/// its source allocation and previously yielded paths remain reusable.
pub(crate) struct ParametricSectorFormulaResidualCursor {
    schema: &'static str,
    source: Arc<ParametricSectorNormalizedCoverageSource>,
    request: ParametricSectorFormulaResidualRequest,
    assignments: Vec<FormulaResidualLocusAssignment>,
    frontier: Vec<ParametricSectorFormulaResidualDecision>,
    resume_after_leaf: bool,
    exhausted: bool,
    poisoned: bool,
    limits: ParametricSectorFormulaResidualLimits,
    stats: ParametricSectorFormulaResidualCursorStats,
}

impl fmt::Debug for ParametricSectorFormulaResidualCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ParametricSectorFormulaResidualCursor")
            .field("schema", &self.schema)
            .field("request", &self.request)
            .field("frontier_depth", &self.frontier.len())
            .field("resume_after_leaf", &self.resume_after_leaf)
            .field("exhausted", &self.exhausted)
            .field("poisoned", &self.poisoned)
            .field("stats", &self.stats)
            .field("source", &"<shared normalized sector coverage>")
            .finish()
    }
}

impl ParametricSectorFormulaResidualCursor {
    pub(crate) fn try_new(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source: Arc<ParametricSectorNormalizedCoverageSource>,
        request: ParametricSectorFormulaResidualRequest,
        limits: ParametricSectorFormulaResidualLimits,
    ) -> Result<Self, ParametricSectorFormulaResidualError> {
        let census = validate_source_header_and_limits(source.as_ref(), limits)?;
        preflight_census_storage(census, limits)?;
        source.replay(family, context)?;
        Self::from_replayed_source_with_census(source, request, limits, census)
    }

    fn from_replayed_source(
        source: Arc<ParametricSectorNormalizedCoverageSource>,
        request: ParametricSectorFormulaResidualRequest,
        limits: ParametricSectorFormulaResidualLimits,
    ) -> Result<Self, ParametricSectorFormulaResidualError> {
        let census = validate_source_header_and_limits(source.as_ref(), limits)?;
        Self::from_replayed_source_with_census(source, request, limits, census)
    }

    fn from_replayed_source_with_census(
        source: Arc<ParametricSectorNormalizedCoverageSource>,
        request: ParametricSectorFormulaResidualRequest,
        limits: ParametricSectorFormulaResidualLimits,
        census: SourceCensus,
    ) -> Result<Self, ParametricSectorFormulaResidualError> {
        preflight_census_storage(census, limits)?;
        let mut assignments = Vec::new();
        assignments
            .try_reserve_exact(census.base_structural_loci)
            .map_err(
                |_| ParametricSectorFormulaResidualError::AllocationFailure {
                    resource: "formula-residual assignments",
                    requested: census.base_structural_loci,
                },
            )?;
        let assignment_capacity_entries = assignments.capacity();
        check_limit(
            "formula-residual assignment capacity entries",
            assignment_capacity_entries,
            limits.max_assignment_capacity_entries,
        )?;
        let retained = cursor_retained_bytes(assignment_capacity_entries, 0)?;
        check_limit(
            "formula-residual cursor retained bytes",
            retained,
            limits.max_cursor_retained_bytes,
        )?;
        assignments.resize(
            census.base_structural_loci,
            FormulaResidualLocusAssignment::Unknown,
        );
        Ok(Self {
            schema: PARAMETRIC_SECTOR_FORMULA_RESIDUAL_CURSOR_V1_SCHEMA,
            source,
            request,
            assignments,
            frontier: Vec::new(),
            resume_after_leaf: false,
            exhausted: false,
            poisoned: false,
            limits,
            stats: ParametricSectorFormulaResidualCursorStats {
                source_references: 1,
                base_structural_loci: census.base_structural_loci,
                attempts: census.attempts,
                certified_attempts: census.certified_attempts,
                unsupported_attempts: census.unsupported_attempts,
                assignment_capacity_entries,
                peak_cursor_retained_bytes: retained,
                ..ParametricSectorFormulaResidualCursorStats::default()
            },
        })
    }

    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) const fn branch_order_schema(&self) -> &'static str {
        PARAMETRIC_SECTOR_FORMULA_RESIDUAL_BRANCH_ORDER_V1
    }

    pub(crate) const fn request(&self) -> ParametricSectorFormulaResidualRequest {
        self.request
    }

    pub(crate) const fn limits(&self) -> ParametricSectorFormulaResidualLimits {
        self.limits
    }

    pub(crate) const fn stats(&self) -> ParametricSectorFormulaResidualCursorStats {
        self.stats
    }

    pub(crate) fn same_source_allocation(
        &self,
        source: &Arc<ParametricSectorNormalizedCoverageSource>,
    ) -> bool {
        Arc::ptr_eq(&self.source, source)
    }

    pub(crate) fn frontier(&self) -> &[ParametricSectorFormulaResidualDecision] {
        &self.frontier
    }

    pub(crate) const fn is_exhausted(&self) -> bool {
        self.exhausted
    }

    pub(crate) const fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    pub(crate) fn retained_bytes(&self) -> Result<usize, ParametricSectorFormulaResidualError> {
        cursor_retained_bytes(self.assignments.capacity(), self.frontier.capacity())
    }

    pub(crate) fn next_path(
        &mut self,
    ) -> Result<
        Option<ParametricSectorFormulaResidualPathCertificate>,
        ParametricSectorFormulaResidualError,
    > {
        if self.poisoned {
            return Err(ParametricSectorFormulaResidualError::CursorPoisoned);
        }
        let result = self.next_path_inner();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn next_path_inner(
        &mut self,
    ) -> Result<
        Option<ParametricSectorFormulaResidualPathCertificate>,
        ParametricSectorFormulaResidualError,
    > {
        if self.exhausted {
            return Ok(None);
        }
        loop {
            if self.resume_after_leaf {
                self.resume_after_leaf = false;
                if !self.advance_after_leaf()? {
                    return Ok(None);
                }
            }
            let classification = classify_partial_assignment(
                self.source.normalized().ir(),
                &self.assignments,
                self.limits,
                &mut self.stats,
            )?;
            match classification {
                FormulaResidualStateClassification::Covered => {
                    self.resume_after_leaf = true;
                }
                FormulaResidualStateClassification::Split(split) => {
                    self.descend_nonzero(split)?;
                }
                FormulaResidualStateClassification::Residual(kind) => {
                    charge_next(
                        "formula-residual terminal visits",
                        &mut self.stats.residual_terminal_visits,
                        self.limits.max_residual_terminal_visits,
                    )?;
                    match kind {
                        ParametricSectorFormulaResidualKind::Uncovered => {
                            self.stats.uncovered_terminals_visited = checked_add(
                                "formula-residual uncovered terminals",
                                self.stats.uncovered_terminals_visited,
                                1,
                            )?;
                        }
                        ParametricSectorFormulaResidualKind::Unsupported => {
                            self.stats.unsupported_terminals_visited = checked_add(
                                "formula-residual unsupported terminals",
                                self.stats.unsupported_terminals_visited,
                                1,
                            )?;
                        }
                    }
                    self.resume_after_leaf = true;
                    if !self.request.accepts(kind) {
                        charge_next(
                            "formula-residual filtered residual terminals",
                            &mut self.stats.filtered_residual_terminals,
                            self.limits.max_filtered_residual_terminals,
                        )?;
                        continue;
                    }
                    let next_yield = checked_add(
                        "formula-residual paths yielded",
                        self.stats.paths_yielded,
                        1,
                    )?;
                    check_limit(
                        "formula-residual paths yielded",
                        next_yield,
                        self.limits.max_paths_yielded,
                    )?;
                    let path = self.build_certificate(next_yield, kind)?;
                    self.stats.paths_yielded = next_yield;
                    return Ok(Some(path));
                }
            }
        }
    }

    fn descend_nonzero(
        &mut self,
        split: ParametricSectorFormulaResidualSplitLocator,
    ) -> Result<(), ParametricSectorFormulaResidualError> {
        let locus = split.structural_locus_ordinal;
        let assignment = self.assignments.get(locus).copied().ok_or(
            ParametricSectorFormulaResidualError::StructuralLocusOutOfRange {
                ordinal: locus,
                locus_count: self.assignments.len(),
            },
        )?;
        if assignment != FormulaResidualLocusAssignment::Unknown {
            return Err(
                ParametricSectorFormulaResidualError::SplitLocusAlreadyAssigned { ordinal: locus },
            );
        }
        let next_depth = checked_add("formula-residual depth", self.frontier.len(), 1)?;
        check_limit("formula-residual depth", next_depth, self.limits.max_depth)?;
        self.reserve_frontier_for_depth(next_depth)?;
        let next_branches = checked_add(
            "formula-residual branch traversals",
            self.stats.branch_traversals,
            1,
        )?;
        check_limit(
            "formula-residual branch traversals",
            next_branches,
            self.limits.max_branch_traversals,
        )?;
        self.assignments[locus] = FormulaResidualLocusAssignment::NonZero;
        self.frontier.push(ParametricSectorFormulaResidualDecision {
            split,
            polarity: ParametricSectorFormulaResidualPolarity::NonZero,
        });
        self.stats.branch_traversals = next_branches;
        self.stats.maximum_depth = self.stats.maximum_depth.max(next_depth);
        Ok(())
    }

    fn advance_after_leaf(&mut self) -> Result<bool, ParametricSectorFormulaResidualError> {
        loop {
            let Some(last) = self.frontier.last().copied() else {
                self.exhausted = true;
                return Ok(false);
            };
            let next_backtracks =
                checked_add("formula-residual backtracks", self.stats.backtracks, 1)?;
            check_limit(
                "formula-residual backtracks",
                next_backtracks,
                self.limits.max_backtracks,
            )?;
            let locus = last.structural_locus_ordinal();
            let locus_count = self.assignments.len();
            let assignment = self.assignments.get_mut(locus).ok_or(
                ParametricSectorFormulaResidualError::StructuralLocusOutOfRange {
                    ordinal: locus,
                    locus_count,
                },
            )?;
            match last.polarity {
                ParametricSectorFormulaResidualPolarity::NonZero => {
                    if *assignment != FormulaResidualLocusAssignment::NonZero {
                        return Err(ParametricSectorFormulaResidualError::PathReplayMismatch);
                    }
                    let next_branches = checked_add(
                        "formula-residual branch traversals",
                        self.stats.branch_traversals,
                        1,
                    )?;
                    check_limit(
                        "formula-residual branch traversals",
                        next_branches,
                        self.limits.max_branch_traversals,
                    )?;
                    *assignment = FormulaResidualLocusAssignment::EqualZero;
                    let Some(last_mut) = self.frontier.last_mut() else {
                        return Err(ParametricSectorFormulaResidualError::PathReplayMismatch);
                    };
                    last_mut.polarity = ParametricSectorFormulaResidualPolarity::EqualZero;
                    self.stats.backtracks = next_backtracks;
                    self.stats.branch_traversals = next_branches;
                    return Ok(true);
                }
                ParametricSectorFormulaResidualPolarity::EqualZero => {
                    if *assignment != FormulaResidualLocusAssignment::EqualZero {
                        return Err(ParametricSectorFormulaResidualError::PathReplayMismatch);
                    }
                    *assignment = FormulaResidualLocusAssignment::Unknown;
                    self.frontier.pop();
                    self.stats.backtracks = next_backtracks;
                }
            }
        }
    }

    fn reserve_frontier_for_depth(
        &mut self,
        next_depth: usize,
    ) -> Result<(), ParametricSectorFormulaResidualError> {
        if self.frontier.capacity() >= next_depth {
            return Ok(());
        }
        let doubled = if self.frontier.capacity() == 0 {
            1
        } else {
            checked_mul(
                "formula-residual frontier growth entries",
                self.frontier.capacity(),
                2,
            )?
        };
        let target = doubled.max(next_depth);
        check_limit(
            "formula-residual frontier capacity entries",
            target,
            self.limits.max_frontier_capacity_entries,
        )?;
        check_limit(
            "formula-residual cursor retained bytes",
            cursor_retained_bytes(self.assignments.capacity(), target)?,
            self.limits.max_cursor_retained_bytes,
        )?;
        let additional = target.checked_sub(self.frontier.len()).ok_or(
            ParametricSectorFormulaResidualError::ResourceCountOverflow {
                resource: "formula-residual frontier growth entries",
            },
        )?;
        self.frontier.try_reserve_exact(additional).map_err(|_| {
            ParametricSectorFormulaResidualError::AllocationFailure {
                resource: "formula-residual frontier entries",
                requested: target,
            }
        })?;
        let capacity = self.frontier.capacity();
        check_limit(
            "formula-residual frontier capacity entries",
            capacity,
            self.limits.max_frontier_capacity_entries,
        )?;
        let retained = cursor_retained_bytes(self.assignments.capacity(), capacity)?;
        check_limit(
            "formula-residual cursor retained bytes",
            retained,
            self.limits.max_cursor_retained_bytes,
        )?;
        self.stats.peak_frontier_capacity_entries =
            self.stats.peak_frontier_capacity_entries.max(capacity);
        self.stats.peak_cursor_retained_bytes = self.stats.peak_cursor_retained_bytes.max(retained);
        Ok(())
    }

    fn build_certificate(
        &mut self,
        yield_ordinal: usize,
        terminal_kind: ParametricSectorFormulaResidualKind,
    ) -> Result<ParametricSectorFormulaResidualPathCertificate, ParametricSectorFormulaResidualError>
    {
        let next_copied = checked_add(
            "formula-residual total path decisions copied",
            self.stats.total_path_decisions_copied,
            self.frontier.len(),
        )?;
        check_limit(
            "formula-residual total path decisions copied",
            next_copied,
            self.limits.max_total_path_decisions_copied,
        )?;
        check_limit(
            "formula-residual path decisions",
            self.frontier.len(),
            self.limits.max_path_decisions,
        )?;
        let mut decisions = Vec::new();
        decisions
            .try_reserve_exact(self.frontier.len())
            .map_err(
                |_| ParametricSectorFormulaResidualError::AllocationFailure {
                    resource: "formula-residual retained path decisions",
                    requested: self.frontier.len(),
                },
            )?;
        check_limit(
            "formula-residual path capacity entries",
            decisions.capacity(),
            self.limits.max_path_capacity_entries,
        )?;
        check_limit(
            "formula-residual path retained bytes",
            path_retained_bytes(decisions.capacity())?,
            self.limits.max_path_retained_bytes,
        )?;
        decisions.extend_from_slice(&self.frontier);
        let stats = path_stats(
            &decisions,
            decisions.capacity(),
            self.stats.unsupported_attempts,
            self.limits,
        )?;
        self.stats.total_path_decisions_copied = next_copied;
        Ok(ParametricSectorFormulaResidualPathCertificate {
            schema: PARAMETRIC_SECTOR_FORMULA_RESIDUAL_PATH_V1_SCHEMA,
            branch_order_schema: PARAMETRIC_SECTOR_FORMULA_RESIDUAL_BRANCH_ORDER_V1,
            source: Arc::clone(&self.source),
            request: self.request,
            yield_ordinal,
            decisions,
            terminal_kind,
            limits: self.limits,
            stats,
        })
    }
}

#[derive(Clone, Copy)]
struct SourceCensus {
    base_structural_loci: usize,
    attempts: usize,
    certified_attempts: usize,
    unsupported_attempts: usize,
}

fn preflight_census_storage(
    census: SourceCensus,
    limits: ParametricSectorFormulaResidualLimits,
) -> Result<(), ParametricSectorFormulaResidualError> {
    check_limit(
        "formula-residual assignment capacity entries",
        census.base_structural_loci,
        limits.max_assignment_capacity_entries,
    )?;
    check_limit(
        "formula-residual cursor retained bytes",
        cursor_retained_bytes(census.base_structural_loci, 0)?,
        limits.max_cursor_retained_bytes,
    )
}

fn validate_source_header_and_limits(
    source: &ParametricSectorNormalizedCoverageSource,
    limits: ParametricSectorFormulaResidualLimits,
) -> Result<SourceCensus, ParametricSectorFormulaResidualError> {
    if source.schema() != PARAMETRIC_SECTOR_NORMALIZED_COVERAGE_SOURCE_V2_SCHEMA {
        return Err(ParametricSectorFormulaResidualError::SourceSchemaMismatch);
    }
    if source.normalized().ir().schema() != PARAMETRIC_SECTOR_FORMULA_IR_V1_SCHEMA
        || source.normalized().family_fingerprint() != source.family_fingerprint()
        || source.normalized().context_fingerprint() != source.context_fingerprint()
        || source.normalized().sector() != source.sector()
        || source.normalized().ir().base_structural_locus_count()
            != source.normalized().base_structural_loci().len()
        || source.normalized().ir().attempts().len() != source.attempts().len()
    {
        return Err(ParametricSectorFormulaResidualError::SourceShapeMismatch);
    }
    let base_structural_loci = source.normalized().ir().base_structural_locus_count();
    let attempts = source.normalized().ir().attempts().len();
    let certified_attempts = source
        .normalized()
        .ir()
        .attempts()
        .iter()
        .filter(|attempt| matches!(attempt, NormalizedCoverageAttempt::Certified(_)))
        .count();
    let unsupported_attempts = attempts - certified_attempts;
    check_limit(
        "formula-residual base structural loci",
        base_structural_loci,
        limits.max_base_structural_loci,
    )?;
    check_limit("formula-residual attempts", attempts, limits.max_attempts)?;
    check_limit(
        "formula-residual certified attempts",
        certified_attempts,
        limits.max_certified_attempts,
    )?;
    check_limit(
        "formula-residual unsupported candidate references",
        unsupported_attempts,
        limits.max_unsupported_candidate_references,
    )?;
    Ok(SourceCensus {
        base_structural_loci,
        attempts,
        certified_attempts,
        unsupported_attempts,
    })
}

fn classify_partial_assignment(
    ir: &NormalizedCoverageIr,
    assignments: &[FormulaResidualLocusAssignment],
    limits: ParametricSectorFormulaResidualLimits,
    stats: &mut ParametricSectorFormulaResidualCursorStats,
) -> Result<FormulaResidualStateClassification, ParametricSectorFormulaResidualError> {
    if assignments.len() != ir.base_structural_locus_count() {
        return Err(ParametricSectorFormulaResidualError::SourceShapeMismatch);
    }
    charge_next(
        "formula-residual state classifications",
        &mut stats.state_classifications,
        limits.max_state_classifications,
    )?;
    let mut first_split = None;
    let mut has_unsupported = false;
    for attempt in ir.attempts() {
        charge_next(
            "formula-residual attempt visits",
            &mut stats.attempt_visits,
            limits.max_attempt_visits,
        )?;
        let NormalizedCoverageAttempt::Certified(formula) = attempt else {
            has_unsupported = true;
            continue;
        };
        let route = route_normalized_formula(formula.body(), assignments, limits, stats)?;
        match route {
            NormalizedFormulaRoute::Bad => charge_next(
                "formula-residual bad routes",
                &mut stats.bad_routes,
                limits.max_bad_routes,
            )?,
            NormalizedFormulaRoute::Good => {
                charge_next(
                    "formula-residual good routes",
                    &mut stats.good_routes,
                    limits.max_good_routes,
                )?;
                if first_split.is_some() {
                    charge_next(
                        "formula-residual later-Good prunes",
                        &mut stats.later_good_prunes,
                        limits.max_later_good_prunes,
                    )?;
                }
                charge_next(
                    "formula-residual covered prunes",
                    &mut stats.covered_prunes,
                    limits.max_covered_prunes,
                )?;
                return Ok(FormulaResidualStateClassification::Covered);
            }
            NormalizedFormulaRoute::Split {
                clause_ordinal,
                atom,
            } => {
                charge_next(
                    "formula-residual split routes",
                    &mut stats.split_routes,
                    limits.max_split_routes,
                )?;
                if first_split.is_none() {
                    first_split = Some(split_locator(
                        formula.source_attempt_ordinal(),
                        formula.body(),
                        clause_ordinal,
                        atom,
                    )?);
                }
            }
        }
    }
    Ok(match first_split {
        Some(split) => FormulaResidualStateClassification::Split(split),
        None if !has_unsupported => FormulaResidualStateClassification::Residual(
            ParametricSectorFormulaResidualKind::Uncovered,
        ),
        None => FormulaResidualStateClassification::Residual(
            ParametricSectorFormulaResidualKind::Unsupported,
        ),
    })
}

fn route_normalized_formula(
    body: &NormalizedBadFormulaBody,
    assignments: &[FormulaResidualLocusAssignment],
    limits: ParametricSectorFormulaResidualLimits,
    stats: &mut ParametricSectorFormulaResidualCursorStats,
) -> Result<NormalizedFormulaRoute, ParametricSectorFormulaResidualError> {
    charge_next(
        "formula-residual formula evaluations",
        &mut stats.formula_evaluations,
        limits.max_formula_evaluations,
    )?;
    let (clauses, literals) = formula_census(body)?;
    charge_amount(
        "formula-residual formula clause charges",
        &mut stats.formula_clause_charges,
        clauses,
        limits.max_formula_clause_charges,
    )?;
    charge_amount(
        "formula-residual literal query charges",
        &mut stats.literal_query_charges,
        literals,
        limits.max_literal_query_charges,
    )?;
    match body {
        NormalizedBadFormulaBody::False => Ok(NormalizedFormulaRoute::Good),
        NormalizedBadFormulaBody::True { .. } => Ok(NormalizedFormulaRoute::Bad),
        NormalizedBadFormulaBody::Dnf { clauses, .. } => {
            let route =
                route_direct_bad_formula(clauses.iter().map(|clause| clause.body()), |literal| {
                    literal_truth(assignments, literal)
                })?;
            Ok(match route {
                DirectBadFormulaRoute::Bad { .. } => NormalizedFormulaRoute::Bad,
                DirectBadFormulaRoute::Good => NormalizedFormulaRoute::Good,
                DirectBadFormulaRoute::Split {
                    clause_ordinal,
                    atom,
                } => NormalizedFormulaRoute::Split {
                    clause_ordinal,
                    atom,
                },
            })
        }
    }
}

fn formula_census(
    body: &NormalizedBadFormulaBody,
) -> Result<(usize, usize), ParametricSectorFormulaResidualError> {
    let NormalizedBadFormulaBody::Dnf { clauses, .. } = body else {
        return Ok((0, 0));
    };
    let mut literals = 0usize;
    for clause in clauses.iter() {
        literals = checked_add(
            "formula-residual literal query charges",
            literals,
            clause.body().atom_count(),
        )?;
    }
    Ok((clauses.len(), literals))
}

fn literal_truth(
    assignments: &[FormulaResidualLocusAssignment],
    literal: NormalizedBadLiteral,
) -> Result<DirectBadFormulaTruth, ParametricSectorFormulaResidualError> {
    let locus = literal.structural_locus_ordinal();
    let assignment = assignments.get(locus).copied().ok_or(
        ParametricSectorFormulaResidualError::StructuralLocusOutOfRange {
            ordinal: locus,
            locus_count: assignments.len(),
        },
    )?;
    Ok(match (assignment, literal.polarity()) {
        (FormulaResidualLocusAssignment::Unknown, _) => DirectBadFormulaTruth::Unknown,
        (FormulaResidualLocusAssignment::EqualZero, NormalizedBadLiteralPolarity::EqualZero)
        | (FormulaResidualLocusAssignment::NonZero, NormalizedBadLiteralPolarity::NonZero) => {
            DirectBadFormulaTruth::True
        }
        _ => DirectBadFormulaTruth::False,
    })
}

fn split_locator(
    attempt_ordinal: usize,
    body: &NormalizedBadFormulaBody,
    clause_ordinal: usize,
    atom: NormalizedBadLiteral,
) -> Result<ParametricSectorFormulaResidualSplitLocator, ParametricSectorFormulaResidualError> {
    let NormalizedBadFormulaBody::Dnf { clauses, .. } = body else {
        return Err(
            ParametricSectorFormulaResidualError::SplitClauseOutOfRange {
                attempt_ordinal,
                clause_ordinal,
                clause_count: 0,
            },
        );
    };
    let clause = clauses.get(clause_ordinal).ok_or(
        ParametricSectorFormulaResidualError::SplitClauseOutOfRange {
            attempt_ordinal,
            clause_ordinal,
            clause_count: clauses.len(),
        },
    )?;
    let literal_position = match clause.body() {
        DirectBadFormulaClause::Atom(candidate) if candidate == atom => 0,
        DirectBadFormulaClause::Conjunction(left, _) if left == atom => 0,
        DirectBadFormulaClause::Conjunction(_, right) if right == atom => 1,
        _ => {
            return Err(ParametricSectorFormulaResidualError::SplitLiteralMismatch {
                attempt_ordinal,
                clause_ordinal,
            });
        }
    };
    Ok(ParametricSectorFormulaResidualSplitLocator {
        source_attempt_ordinal: attempt_ordinal,
        clause_ordinal,
        literal_position,
        structural_locus_ordinal: atom.structural_locus_ordinal(),
        bad_literal_polarity: atom.polarity(),
    })
}

fn path_stats(
    decisions: &[ParametricSectorFormulaResidualDecision],
    decision_capacity_entries: usize,
    unsupported_candidate_references: usize,
    limits: ParametricSectorFormulaResidualLimits,
) -> Result<ParametricSectorFormulaResidualPathStats, ParametricSectorFormulaResidualError> {
    check_limit(
        "formula-residual path decisions",
        decisions.len(),
        limits.max_path_decisions,
    )?;
    check_limit(
        "formula-residual path capacity entries",
        decision_capacity_entries,
        limits.max_path_capacity_entries,
    )?;
    if decision_capacity_entries < decisions.len() {
        return Err(ParametricSectorFormulaResidualError::PathReplayMismatch);
    }
    check_limit(
        "formula-residual unsupported candidate references",
        unsupported_candidate_references,
        limits.max_unsupported_candidate_references,
    )?;
    let retained_path_bytes = path_retained_bytes(decision_capacity_entries)?;
    check_limit(
        "formula-residual path retained bytes",
        retained_path_bytes,
        limits.max_path_retained_bytes,
    )?;
    let nonzero_decisions = decisions
        .iter()
        .filter(|decision| decision.polarity == ParametricSectorFormulaResidualPolarity::NonZero)
        .count();
    Ok(ParametricSectorFormulaResidualPathStats {
        decisions: decisions.len(),
        decision_capacity_entries,
        nonzero_decisions,
        equal_zero_decisions: decisions.len() - nonzero_decisions,
        unsupported_candidate_references,
        retained_path_bytes,
    })
}

fn cursor_retained_bytes(
    assignment_capacity: usize,
    frontier_capacity: usize,
) -> Result<usize, ParametricSectorFormulaResidualError> {
    checked_add(
        "formula-residual cursor retained bytes",
        checked_add(
            "formula-residual cursor retained bytes",
            size_of::<ParametricSectorFormulaResidualCursor>(),
            checked_mul(
                "formula-residual cursor retained bytes",
                assignment_capacity,
                size_of::<FormulaResidualLocusAssignment>(),
            )?,
        )?,
        checked_mul(
            "formula-residual cursor retained bytes",
            frontier_capacity,
            size_of::<ParametricSectorFormulaResidualDecision>(),
        )?,
    )
}

fn path_retained_bytes(
    decision_capacity: usize,
) -> Result<usize, ParametricSectorFormulaResidualError> {
    checked_add(
        "formula-residual path retained bytes",
        size_of::<ParametricSectorFormulaResidualPathCertificate>(),
        checked_mul(
            "formula-residual path retained bytes",
            decision_capacity,
            size_of::<ParametricSectorFormulaResidualDecision>(),
        )?,
    )
}

fn charge_next(
    resource: &'static str,
    counter: &mut usize,
    limit: usize,
) -> Result<(), ParametricSectorFormulaResidualError> {
    charge_amount(resource, counter, 1, limit)
}

fn charge_amount(
    resource: &'static str,
    counter: &mut usize,
    amount: usize,
    limit: usize,
) -> Result<(), ParametricSectorFormulaResidualError> {
    let requested = checked_add(resource, *counter, amount)?;
    check_limit(resource, requested, limit)?;
    *counter = requested;
    Ok(())
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ParametricSectorFormulaResidualError> {
    if requested > limit {
        Err(ParametricSectorFormulaResidualError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ParametricSectorFormulaResidualError> {
    left.checked_add(right)
        .ok_or(ParametricSectorFormulaResidualError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ParametricSectorFormulaResidualError> {
    left.checked_mul(right)
        .ok_or(ParametricSectorFormulaResidualError::ResourceCountOverflow { resource })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated_when_bad::{
        replayed_row_span_authentication_calls, reset_replayed_row_span_authentication_calls,
    };
    use crate::parametric_sector_formula_ir::{
        NormalizedBadClause, NormalizedBadClauseRole, NormalizedBadClauseSource,
        NormalizedCandidateBadFormula, NormalizedFactorZeroSource,
    };
    use crate::parametric_sector_k21_test_support::compile_six_loop_k21_normalized_fixture;
    use crate::parametric_sector_mtbdd::{
        ParametricSectorMtbddCompiler, ParametricSectorMtbddDisposition,
        ParametricSectorMtbddLimits, reference_disposition_for_assignment,
    };
    use crate::parametric_sector_mtbdd_certificate::{
        ParametricSectorMtbddCoverageCompiler, ParametricSectorMtbddCoverageLimits,
    };
    use crate::parametric_sector_normalized_source::{
        ParametricSectorNormalizedCoverageSourceCompiler,
        ParametricSectorNormalizedCoverageSourceLimits,
    };
    use crate::{
        AffineDenominator, CoefficientContext, GeneratedSectorDiscoveryCompiler,
        GeneratedSectorDiscoveryLimits, GeneratedWhenBadCompilation, GeneratedWhenBadCompiler,
        GeneratedWhenBadLimits, IntegralOrderingPolicy, ParametricElimination,
        ParametricEliminationLimits, ParametricEliminationOrdering, ParametricIbpGenerator,
        ParametricReductionRuleCandidate, ParametricRuleLimits, SectorMask,
    };

    fn clause_sources(source_ordinal: usize) -> Box<[NormalizedBadClauseSource]> {
        vec![NormalizedBadClauseSource::LeakEvent {
            event_ordinal: source_ordinal,
        }]
        .into_boxed_slice()
    }

    fn literal(locus: usize, polarity: NormalizedBadLiteralPolarity) -> NormalizedBadLiteral {
        NormalizedBadLiteral::new(locus, polarity)
    }

    fn clause(
        body: DirectBadFormulaClause<NormalizedBadLiteral>,
        source_ordinal: usize,
        role: NormalizedBadClauseRole,
    ) -> NormalizedBadClause {
        NormalizedBadClause::new(body, clause_sources(source_ordinal), role)
    }

    fn certified(ordinal: usize, body: NormalizedBadFormulaBody) -> NormalizedCoverageAttempt {
        NormalizedCoverageAttempt::Certified(NormalizedCandidateBadFormula::new(ordinal, body))
    }

    fn dnf(
        ordinal: usize,
        clauses: Vec<NormalizedBadClause>,
        factors: Vec<NormalizedFactorZeroSource>,
    ) -> NormalizedCoverageAttempt {
        certified(
            ordinal,
            NormalizedBadFormulaBody::Dnf {
                clauses: clauses.into_boxed_slice(),
                atomic_equal_zero_factors: factors.into_boxed_slice(),
            },
        )
    }

    fn unsupported(ordinal: usize) -> NormalizedCoverageAttempt {
        NormalizedCoverageAttempt::Unsupported {
            source_attempt_ordinal: ordinal,
        }
    }

    fn mixed_synthetic_ir() -> NormalizedCoverageIr {
        let equal_zero_0 = literal(0, NormalizedBadLiteralPolarity::EqualZero);
        NormalizedCoverageIr::try_new(
            3,
            vec![
                dnf(
                    0,
                    vec![
                        clause(
                            DirectBadFormulaClause::Atom(equal_zero_0),
                            0,
                            NormalizedBadClauseRole::AtomicEqualZeroFactor,
                        ),
                        clause(
                            DirectBadFormulaClause::Atom(literal(
                                1,
                                NormalizedBadLiteralPolarity::NonZero,
                            )),
                            1,
                            NormalizedBadClauseRole::Ordinary,
                        ),
                        clause(
                            DirectBadFormulaClause::Conjunction(
                                literal(2, NormalizedBadLiteralPolarity::EqualZero),
                                literal(0, NormalizedBadLiteralPolarity::NonZero),
                            ),
                            2,
                            NormalizedBadClauseRole::Ordinary,
                        ),
                    ],
                    vec![NormalizedFactorZeroSource::new(0, 0)],
                ),
                unsupported(1),
                dnf(
                    2,
                    vec![clause(
                        DirectBadFormulaClause::Atom(literal(
                            2,
                            NormalizedBadLiteralPolarity::NonZero,
                        )),
                        3,
                        NormalizedBadClauseRole::Ordinary,
                    )],
                    Vec::new(),
                ),
                unsupported(3),
            ]
            .into_boxed_slice(),
        )
        .unwrap()
    }

    type SyntheticCube = (
        Vec<ParametricSectorFormulaResidualDecision>,
        ParametricSectorFormulaResidualKind,
    );

    fn collect_synthetic_cubes(
        ir: &NormalizedCoverageIr,
    ) -> (
        Vec<SyntheticCube>,
        ParametricSectorFormulaResidualCursorStats,
    ) {
        fn walk(
            ir: &NormalizedCoverageIr,
            assignments: &mut [FormulaResidualLocusAssignment],
            frontier: &mut Vec<ParametricSectorFormulaResidualDecision>,
            cubes: &mut Vec<SyntheticCube>,
            stats: &mut ParametricSectorFormulaResidualCursorStats,
        ) {
            match classify_partial_assignment(
                ir,
                assignments,
                ParametricSectorFormulaResidualLimits::default(),
                stats,
            )
            .unwrap()
            {
                FormulaResidualStateClassification::Covered => {}
                FormulaResidualStateClassification::Residual(kind) => {
                    cubes.push((frontier.clone(), kind));
                }
                FormulaResidualStateClassification::Split(split) => {
                    let locus = split.structural_locus_ordinal();
                    assert_eq!(assignments[locus], FormulaResidualLocusAssignment::Unknown);
                    assignments[locus] = FormulaResidualLocusAssignment::NonZero;
                    frontier.push(ParametricSectorFormulaResidualDecision {
                        split,
                        polarity: ParametricSectorFormulaResidualPolarity::NonZero,
                    });
                    walk(ir, assignments, frontier, cubes, stats);
                    assignments[locus] = FormulaResidualLocusAssignment::EqualZero;
                    frontier.last_mut().unwrap().polarity =
                        ParametricSectorFormulaResidualPolarity::EqualZero;
                    walk(ir, assignments, frontier, cubes, stats);
                    assignments[locus] = FormulaResidualLocusAssignment::Unknown;
                    frontier.pop();
                }
            }
        }

        let mut assignments =
            vec![FormulaResidualLocusAssignment::Unknown; ir.base_structural_locus_count()];
        let mut frontier = Vec::new();
        let mut cubes = Vec::new();
        let mut stats = ParametricSectorFormulaResidualCursorStats::default();
        walk(ir, &mut assignments, &mut frontier, &mut cubes, &mut stats);
        (cubes, stats)
    }

    fn cube_matches(decisions: &[ParametricSectorFormulaResidualDecision], zero: &[bool]) -> bool {
        decisions.iter().all(|decision| {
            zero[decision.structural_locus_ordinal()]
                == (decision.polarity() == ParametricSectorFormulaResidualPolarity::EqualZero)
        })
    }

    fn is_residual(disposition: &ParametricSectorMtbddDisposition) -> bool {
        matches!(
            disposition,
            ParametricSectorMtbddDisposition::Uncovered
                | ParametricSectorMtbddDisposition::Unsupported { .. }
        )
    }

    #[test]
    fn exhaustive_small_ir_residual_union_matches_reference_and_mtbdd() {
        let ir = mixed_synthetic_ir();
        let mtbdd =
            ParametricSectorMtbddCompiler::compile(&ir, ParametricSectorMtbddLimits::default())
                .unwrap();
        let (cubes, _) = collect_synthetic_cubes(&ir);
        let (repeated, _) = collect_synthetic_cubes(&ir);
        assert_eq!(cubes, repeated);
        assert!(!cubes.is_empty());

        for mask in 0usize..(1usize << ir.base_structural_locus_count()) {
            let zero = (0..ir.base_structural_locus_count())
                .map(|bit| mask & (1usize << bit) != 0)
                .collect::<Vec<_>>();
            let reference = reference_disposition_for_assignment(&ir, &zero).unwrap();
            assert_eq!(mtbdd.classify_assignment(&zero).unwrap(), &reference);
            let matching = cubes
                .iter()
                .filter(|(decisions, _)| cube_matches(decisions, &zero))
                .collect::<Vec<_>>();
            assert_eq!(
                matching.len(),
                usize::from(is_residual(&reference)),
                "assignment mask {mask:03b}"
            );
            if let Some((_, kind)) = matching.first() {
                assert_eq!(*kind, ParametricSectorFormulaResidualKind::Unsupported);
                assert!(matches!(
                    reference,
                    ParametricSectorMtbddDisposition::Unsupported { ref candidate_ordinals }
                        if candidate_ordinals.as_ref() == [1, 3]
                ));
            }
        }
    }

    #[test]
    fn later_good_prunes_an_earlier_split_and_locator_is_exact() {
        let ir = NormalizedCoverageIr::try_new(
            2,
            vec![
                dnf(
                    0,
                    vec![clause(
                        DirectBadFormulaClause::Conjunction(
                            literal(0, NormalizedBadLiteralPolarity::EqualZero),
                            literal(1, NormalizedBadLiteralPolarity::EqualZero),
                        ),
                        0,
                        NormalizedBadClauseRole::Ordinary,
                    )],
                    Vec::new(),
                ),
                dnf(
                    1,
                    vec![clause(
                        DirectBadFormulaClause::Atom(literal(
                            0,
                            NormalizedBadLiteralPolarity::NonZero,
                        )),
                        1,
                        NormalizedBadClauseRole::Ordinary,
                    )],
                    Vec::new(),
                ),
            ]
            .into_boxed_slice(),
        )
        .unwrap();
        let assignments = [
            FormulaResidualLocusAssignment::EqualZero,
            FormulaResidualLocusAssignment::Unknown,
        ];
        let mut stats = ParametricSectorFormulaResidualCursorStats::default();
        assert_eq!(
            classify_partial_assignment(
                &ir,
                &assignments,
                ParametricSectorFormulaResidualLimits::default(),
                &mut stats,
            )
            .unwrap(),
            FormulaResidualStateClassification::Covered
        );
        assert_eq!(stats.split_routes(), 1);
        assert_eq!(stats.good_routes(), 1);
        assert_eq!(stats.later_good_prunes(), 1);

        let first_only =
            NormalizedCoverageIr::try_new(2, vec![ir.attempts()[0].clone()].into_boxed_slice())
                .unwrap();
        let mut first_stats = ParametricSectorFormulaResidualCursorStats::default();
        let FormulaResidualStateClassification::Split(split) = classify_partial_assignment(
            &first_only,
            &assignments,
            ParametricSectorFormulaResidualLimits::default(),
            &mut first_stats,
        )
        .unwrap() else {
            panic!("the first formula alone must split")
        };
        assert_eq!(split.source_attempt_ordinal(), 0);
        assert_eq!(split.clause_ordinal(), 0);
        assert_eq!(split.literal_position(), 1);
        assert_eq!(split.structural_locus_ordinal(), 1);
        assert_eq!(
            split.bad_literal_polarity(),
            NormalizedBadLiteralPolarity::EqualZero
        );
    }

    #[test]
    fn constants_repeated_literals_and_opposite_polarities_route_exactly() {
        let constants = NormalizedCoverageIr::try_new(
            1,
            vec![
                certified(
                    0,
                    NormalizedBadFormulaBody::True {
                        sources: clause_sources(0),
                    },
                ),
                certified(1, NormalizedBadFormulaBody::False),
                unsupported(2),
            ]
            .into_boxed_slice(),
        )
        .unwrap();
        let mut constant_stats = ParametricSectorFormulaResidualCursorStats::default();
        assert_eq!(
            classify_partial_assignment(
                &constants,
                &[FormulaResidualLocusAssignment::Unknown],
                ParametricSectorFormulaResidualLimits::default(),
                &mut constant_stats,
            )
            .unwrap(),
            FormulaResidualStateClassification::Covered
        );
        assert_eq!(constant_stats.bad_routes(), 1);
        assert_eq!(constant_stats.good_routes(), 1);
        assert_eq!(constant_stats.attempt_visits(), 2);

        let repeated = NormalizedCoverageIr::try_new(
            1,
            vec![dnf(
                0,
                vec![clause(
                    DirectBadFormulaClause::Conjunction(
                        literal(0, NormalizedBadLiteralPolarity::EqualZero),
                        literal(0, NormalizedBadLiteralPolarity::EqualZero),
                    ),
                    0,
                    NormalizedBadClauseRole::Ordinary,
                )],
                Vec::new(),
            )]
            .into_boxed_slice(),
        )
        .unwrap();
        let mut repeated_stats = ParametricSectorFormulaResidualCursorStats::default();
        let FormulaResidualStateClassification::Split(split) = classify_partial_assignment(
            &repeated,
            &[FormulaResidualLocusAssignment::Unknown],
            ParametricSectorFormulaResidualLimits::default(),
            &mut repeated_stats,
        )
        .unwrap() else {
            panic!("the repeated unknown literal must split")
        };
        assert_eq!(split.literal_position(), 0);

        let opposite = NormalizedCoverageIr::try_new(
            1,
            vec![dnf(
                0,
                vec![clause(
                    DirectBadFormulaClause::Conjunction(
                        literal(0, NormalizedBadLiteralPolarity::EqualZero),
                        literal(0, NormalizedBadLiteralPolarity::NonZero),
                    ),
                    0,
                    NormalizedBadClauseRole::Ordinary,
                )],
                Vec::new(),
            )]
            .into_boxed_slice(),
        )
        .unwrap();
        let (cubes, _) = collect_synthetic_cubes(&opposite);
        assert!(
            cubes.is_empty(),
            "the contradictory bad clause is always Good"
        );
        for zero in [false, true] {
            assert!(matches!(
                reference_disposition_for_assignment(&opposite, &[zero]).unwrap(),
                ParametricSectorMtbddDisposition::DescendingRule {
                    candidate_ordinal: 0
                }
            ));
        }
    }

    #[test]
    fn atomic_factor_metadata_is_not_routed_a_second_time() {
        let ir = NormalizedCoverageIr::try_new(
            1,
            vec![dnf(
                0,
                vec![clause(
                    DirectBadFormulaClause::Atom(literal(
                        0,
                        NormalizedBadLiteralPolarity::EqualZero,
                    )),
                    0,
                    NormalizedBadClauseRole::AtomicEqualZeroFactor,
                )],
                vec![NormalizedFactorZeroSource::new(0, 0)],
            )]
            .into_boxed_slice(),
        )
        .unwrap();
        let mut stats = ParametricSectorFormulaResidualCursorStats::default();
        let FormulaResidualStateClassification::Split(split) = classify_partial_assignment(
            &ir,
            &[FormulaResidualLocusAssignment::Unknown],
            ParametricSectorFormulaResidualLimits::default(),
            &mut stats,
        )
        .unwrap() else {
            panic!("the factor clause must split once")
        };
        assert_eq!(split.structural_locus_ordinal(), 0);
        assert_eq!(stats.formula_clause_charges(), 1);
        assert_eq!(stats.literal_query_charges(), 1);
        let (cubes, _) = collect_synthetic_cubes(&ir);
        assert_eq!(cubes.len(), 1);
        assert_eq!(
            cubes[0].0[0].polarity(),
            ParametricSectorFormulaResidualPolarity::EqualZero
        );
    }

    fn massive_tadpole(name: &str) -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        IntegralFamily::new(
            name,
            vec!["k".into()],
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

    fn one_loop_source(
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<ParametricSectorNormalizedCoverageSource>,
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
        let compilations = discovery
            .coverage()
            .candidate_attempts()
            .iter()
            .map(|attempt| attempt.compilation().clone())
            .collect();
        let source = Arc::new(
            ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated(
                &family,
                &context,
                discovery.sector().clone(),
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                compilations,
                ParametricSectorNormalizedCoverageSourceLimits::default(),
            )
            .unwrap(),
        );
        (family, context, source)
    }

    fn unsupported_one_loop_source() -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<ParametricSectorNormalizedCoverageSource>,
    ) {
        let family = massive_tadpole("formula-residual-unsupported-one-loop");
        let generated = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .generate()
            .unwrap();
        let context = generated.context().clone();
        let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
        let sector = SectorMask::try_new([false]).unwrap();
        let elimination = ParametricElimination::build(
            &context,
            &rows,
            ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [0])
                .unwrap(),
            ParametricEliminationLimits::default(),
        )
        .unwrap();
        let candidate = ParametricReductionRuleCandidate::try_from_elimination_pivot(
            &context,
            &rows,
            &elimination,
            0,
            sector.clone(),
            ParametricRuleLimits::default(),
        )
        .unwrap();
        let compilation = GeneratedWhenBadCompiler::compile(
            &family,
            &context,
            &candidate,
            GeneratedWhenBadLimits::default(),
        )
        .unwrap();
        assert!(matches!(
            &compilation,
            GeneratedWhenBadCompilation::Unsupported(_)
        ));
        let source = Arc::new(
            ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated(
                &family,
                &context,
                sector,
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                vec![compilation.clone(), compilation],
                ParametricSectorNormalizedCoverageSourceLimits::default(),
            )
            .unwrap(),
        );
        (family, context, source)
    }

    fn collect_cursor_paths(
        cursor: &mut ParametricSectorFormulaResidualCursor,
    ) -> Vec<ParametricSectorFormulaResidualPathCertificate> {
        let mut paths = Vec::new();
        while let Some(path) = cursor.next_path().unwrap() {
            paths.push(path);
        }
        paths
    }

    #[test]
    fn authenticated_direct_cursor_matches_same_source_mtbdd_on_every_assignment() {
        let (family, context, source) = one_loop_source("formula-residual-direct-one-loop");
        let mut cursor = ParametricSectorFormulaResidualCursor::try_new(
            &family,
            &context,
            Arc::clone(&source),
            ParametricSectorFormulaResidualRequest::AnyResidual,
            ParametricSectorFormulaResidualLimits::default(),
        )
        .unwrap();
        let paths = collect_cursor_paths(&mut cursor);
        assert!(!paths.is_empty());
        assert!(cursor.is_exhausted());
        assert!(!cursor.is_poisoned());
        assert!(cursor.same_source_allocation(&source));
        assert_eq!(
            cursor.schema(),
            PARAMETRIC_SECTOR_FORMULA_RESIDUAL_CURSOR_V1_SCHEMA
        );
        assert_eq!(
            cursor.branch_order_schema(),
            PARAMETRIC_SECTOR_FORMULA_RESIDUAL_BRANCH_ORDER_V1
        );
        assert_eq!(
            cursor.retained_bytes().unwrap(),
            cursor.stats().peak_cursor_retained_bytes()
        );
        assert!(cursor.stats().covered_prunes() > 0);
        assert!(cursor.stats().backtracks() > 0);
        assert_eq!(
            paths[0].decisions().last().unwrap().polarity(),
            ParametricSectorFormulaResidualPolarity::EqualZero,
            "the covered nonzero branch is visited before the first residual equal-zero branch"
        );

        source.replay(&family, &context).unwrap();
        let mtbdd = ParametricSectorMtbddCoverageCompiler::compile_from_source(
            Arc::clone(&source),
            ParametricSectorMtbddCoverageLimits::default().mtbdd,
        )
        .unwrap();
        assert!(Arc::ptr_eq(mtbdd.source_arc(), &source));
        let locus_count = source.normalized().ir().base_structural_locus_count();
        for mask in 0usize..(1usize << locus_count) {
            let zero = (0..locus_count)
                .map(|bit| mask & (1usize << bit) != 0)
                .collect::<Vec<_>>();
            let reference =
                reference_disposition_for_assignment(source.normalized().ir(), &zero).unwrap();
            assert_eq!(mtbdd.classify_assignment(&zero).unwrap(), &reference);
            let matches = paths
                .iter()
                .filter(|path| cube_matches(path.decisions(), &zero))
                .count();
            assert_eq!(matches, usize::from(is_residual(&reference)));
        }
        for path in &paths {
            assert!(path.same_source_allocation(&source));
            assert_eq!(
                path.yield_ordinal(),
                1 + paths.iter().position(|p| p.payload_eq(path)).unwrap()
            );
            path.replay(&family, &context).unwrap();
        }
    }

    #[test]
    fn root_filters_and_source_backed_unsupported_order_are_exact() {
        let (family, context, nonempty) = one_loop_source("formula-residual-empty-source");
        let empty = Arc::new(
            ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated(
                &family,
                &context,
                nonempty.sector().clone(),
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                Vec::new(),
                ParametricSectorNormalizedCoverageSourceLimits::default(),
            )
            .unwrap(),
        );
        let mut filtered_limits = ParametricSectorFormulaResidualLimits::default();
        filtered_limits.max_filtered_residual_terminals = 1;
        let mut filtered = ParametricSectorFormulaResidualCursor::try_new(
            &family,
            &context,
            Arc::clone(&empty),
            ParametricSectorFormulaResidualRequest::Unsupported,
            filtered_limits,
        )
        .unwrap();
        assert!(filtered.next_path().unwrap().is_none());
        assert!(filtered.is_exhausted());
        assert_eq!(filtered.stats().filtered_residual_terminals(), 1);

        let mut below_filter = filtered_limits;
        below_filter.max_filtered_residual_terminals = 0;
        let mut rejected_filter = ParametricSectorFormulaResidualCursor::try_new(
            &family,
            &context,
            Arc::clone(&empty),
            ParametricSectorFormulaResidualRequest::Unsupported,
            below_filter,
        )
        .unwrap();
        assert!(matches!(
            rejected_filter.next_path(),
            Err(ParametricSectorFormulaResidualError::ResourceLimit {
                resource: "formula-residual filtered residual terminals",
                requested: 1,
                limit: 0,
            })
        ));
        assert!(rejected_filter.is_poisoned());

        let mut uncovered = ParametricSectorFormulaResidualCursor::try_new(
            &family,
            &context,
            Arc::clone(&empty),
            ParametricSectorFormulaResidualRequest::Uncovered,
            ParametricSectorFormulaResidualLimits::default(),
        )
        .unwrap();
        let empty_path = uncovered.next_path().unwrap().unwrap();
        assert!(empty_path.decisions().is_empty());
        assert_eq!(
            empty_path.terminal_kind(),
            ParametricSectorFormulaResidualKind::Uncovered
        );
        assert!(empty_path.unsupported_candidate_ordinals().next().is_none());
        empty_path.replay(&family, &context).unwrap();

        let (unsupported_family, unsupported_context, unsupported_source) =
            unsupported_one_loop_source();
        let mut unsupported_cursor = ParametricSectorFormulaResidualCursor::try_new(
            &unsupported_family,
            &unsupported_context,
            Arc::clone(&unsupported_source),
            ParametricSectorFormulaResidualRequest::Unsupported,
            ParametricSectorFormulaResidualLimits::default(),
        )
        .unwrap();
        let unsupported_path = unsupported_cursor.next_path().unwrap().unwrap();
        assert_eq!(
            unsupported_path.terminal_kind(),
            ParametricSectorFormulaResidualKind::Unsupported
        );
        assert_eq!(
            unsupported_path
                .unsupported_candidate_ordinals()
                .collect::<Vec<_>>(),
            [0, 1]
        );
        assert_eq!(
            unsupported_path.stats().unsupported_candidate_references(),
            2
        );
        unsupported_path
            .replay(&unsupported_family, &unsupported_context)
            .unwrap();

        let mut below_unsupported = ParametricSectorFormulaResidualLimits::default();
        below_unsupported.max_unsupported_candidate_references = 1;
        assert!(matches!(
            ParametricSectorFormulaResidualCursor::try_new(
                &unsupported_family,
                &unsupported_context,
                Arc::clone(&unsupported_source),
                ParametricSectorFormulaResidualRequest::Unsupported,
                below_unsupported,
            ),
            Err(ParametricSectorFormulaResidualError::ResourceLimit {
                resource: "formula-residual unsupported candidate references",
                requested: 2,
                limit: 1,
            })
        ));

        assert_eq!(
            mixed_synthetic_ir()
                .attempts()
                .iter()
                .filter_map(|attempt| match attempt {
                    NormalizedCoverageAttempt::Unsupported {
                        source_attempt_ordinal,
                    } => Some(*source_attempt_ordinal),
                    NormalizedCoverageAttempt::Certified(_) => None,
                })
                .collect::<Vec<_>>(),
            [1, 3]
        );
    }

    #[test]
    fn path_replay_rejects_provenance_request_decision_and_stats_tamper() {
        let (family, context, source) = one_loop_source("formula-residual-replay-tamper");
        let mut cursor = ParametricSectorFormulaResidualCursor::try_new(
            &family,
            &context,
            Arc::clone(&source),
            ParametricSectorFormulaResidualRequest::AnyResidual,
            ParametricSectorFormulaResidualLimits::default(),
        )
        .unwrap();
        let path = cursor.next_path().unwrap().unwrap();
        path.replay(&family, &context).unwrap();
        assert!(format!("{path:?}").contains("<shared normalized sector coverage>"));
        assert!(path.structural_locus(0).is_some());

        let mut wrong_schema = path.clone();
        wrong_schema.schema = "tampered-formula-residual-path";
        assert_eq!(
            wrong_schema.replay(&family, &context),
            Err(ParametricSectorFormulaResidualError::PathReplayMismatch)
        );
        let mut wrong_yield = path.clone();
        wrong_yield.yield_ordinal += 1;
        assert_eq!(
            wrong_yield.replay(&family, &context),
            Err(ParametricSectorFormulaResidualError::PathReplayMismatch)
        );
        let mut wrong_request = path.clone();
        wrong_request.request = ParametricSectorFormulaResidualRequest::Unsupported;
        assert_eq!(
            wrong_request.replay(&family, &context),
            Err(ParametricSectorFormulaResidualError::PathReplayMismatch)
        );
        let mut wrong_stats = path.clone();
        wrong_stats.stats.decisions += 1;
        assert_eq!(
            wrong_stats.replay(&family, &context),
            Err(ParametricSectorFormulaResidualError::PathReplayMismatch)
        );
        if !path.decisions.is_empty() {
            let mut wrong_locator = path.clone();
            wrong_locator.decisions[0].split.bad_literal_polarity = match wrong_locator.decisions[0]
                .split
                .bad_literal_polarity
            {
                NormalizedBadLiteralPolarity::EqualZero => NormalizedBadLiteralPolarity::NonZero,
                NormalizedBadLiteralPolarity::NonZero => NormalizedBadLiteralPolarity::EqualZero,
            };
            assert_eq!(
                wrong_locator.replay(&family, &context),
                Err(ParametricSectorFormulaResidualError::PathReplayMismatch)
            );
            let mut wrong_decision = path.clone();
            wrong_decision.decisions[0].polarity = match wrong_decision.decisions[0].polarity {
                ParametricSectorFormulaResidualPolarity::NonZero => {
                    ParametricSectorFormulaResidualPolarity::EqualZero
                }
                ParametricSectorFormulaResidualPolarity::EqualZero => {
                    ParametricSectorFormulaResidualPolarity::NonZero
                }
            };
            assert_eq!(
                wrong_decision.replay(&family, &context),
                Err(ParametricSectorFormulaResidualError::PathReplayMismatch)
            );
        }
    }

    fn exact_limits(
        cursor: ParametricSectorFormulaResidualCursorStats,
        path: ParametricSectorFormulaResidualPathStats,
    ) -> ParametricSectorFormulaResidualLimits {
        ParametricSectorFormulaResidualLimits {
            max_base_structural_loci: cursor.base_structural_loci(),
            max_attempts: cursor.attempts(),
            max_certified_attempts: cursor.certified_attempts(),
            max_unsupported_candidate_references: cursor.unsupported_attempts(),
            max_assignment_capacity_entries: cursor.assignment_capacity_entries(),
            max_state_classifications: cursor.state_classifications(),
            max_attempt_visits: cursor.attempt_visits(),
            max_formula_evaluations: cursor.formula_evaluations(),
            max_formula_clause_charges: cursor.formula_clause_charges(),
            max_literal_query_charges: cursor.literal_query_charges(),
            max_good_routes: cursor.good_routes(),
            max_bad_routes: cursor.bad_routes(),
            max_split_routes: cursor.split_routes(),
            max_later_good_prunes: cursor.later_good_prunes(),
            max_covered_prunes: cursor.covered_prunes(),
            max_residual_terminal_visits: cursor.residual_terminal_visits(),
            max_filtered_residual_terminals: cursor.filtered_residual_terminals(),
            max_branch_traversals: cursor.branch_traversals(),
            max_backtracks: cursor.backtracks(),
            max_depth: cursor.maximum_depth(),
            max_frontier_capacity_entries: cursor.peak_frontier_capacity_entries(),
            max_cursor_retained_bytes: cursor.peak_cursor_retained_bytes(),
            max_paths_yielded: cursor.paths_yielded(),
            max_total_path_decisions_copied: cursor.total_path_decisions_copied(),
            max_path_decisions: path.decisions(),
            max_path_capacity_entries: path.decision_capacity_entries(),
            max_path_retained_bytes: path.retained_path_bytes(),
        }
    }

    fn first_path_with_limits(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source: &Arc<ParametricSectorNormalizedCoverageSource>,
        limits: ParametricSectorFormulaResidualLimits,
    ) -> Result<ParametricSectorFormulaResidualPathCertificate, ParametricSectorFormulaResidualError>
    {
        let mut cursor = ParametricSectorFormulaResidualCursor::try_new(
            family,
            context,
            Arc::clone(source),
            ParametricSectorFormulaResidualRequest::AnyResidual,
            limits,
        )?;
        cursor
            .next_path()?
            .ok_or(ParametricSectorFormulaResidualError::PathReplayMismatch)
    }

    #[test]
    #[ignore = "honest all-36 L=6/K=21 direct normalized-formula stress"]
    fn full_six_loop_k21_normalized_source_finds_first_residual_without_mtbdd() {
        use std::time::Instant;

        const LOCI: usize = 49;
        const ATTEMPTS: usize = 36;
        const CERTIFIED: usize = 15;
        const UNSUPPORTED: usize = 21;
        const RETAINED_BYTE_LIMIT: usize = 1024 * 1024;
        const MAX_REFERENCE_COMPLETIONS: usize = 1 << 20;
        const CERTIFIED_ORDINALS: [usize; CERTIFIED] =
            [1, 2, 3, 4, 5, 8, 9, 10, 11, 15, 16, 17, 22, 23, 29];
        const UNSUPPORTED_ORDINALS: [usize; UNSUPPORTED] = [
            0, 6, 7, 12, 13, 14, 18, 19, 20, 21, 24, 25, 26, 27, 28, 30, 31, 32, 33, 34, 35,
        ];

        reset_replayed_row_span_authentication_calls();
        let fixture =
            compile_six_loop_k21_normalized_fixture("formula-residual-six-loop-k21-direct");
        let fixture_authentication_calls = replayed_row_span_authentication_calls();
        assert_eq!(fixture_authentication_calls, ATTEMPTS);
        let family = fixture.family;
        let context = fixture.context;
        let source = fixture.source;
        let build = fixture.timings;

        let coverage = source.stats().coverage();
        let normalization = source.stats().normalization();
        assert_eq!(source.row_span().rows().len(), ATTEMPTS);
        assert_eq!(source.attempts().len(), ATTEMPTS);
        assert_eq!(coverage.candidates(), ATTEMPTS);
        assert_eq!(coverage.certified_candidates(), CERTIFIED);
        assert_eq!(coverage.unsupported_candidates(), UNSUPPORTED);
        assert_eq!(normalization.attempts(), ATTEMPTS);
        assert_eq!(normalization.certified_attempts(), CERTIFIED);
        assert_eq!(normalization.unsupported_attempts(), UNSUPPORTED);
        assert_eq!(source.normalized().ir().base_structural_locus_count(), LOCI);
        assert_eq!(source.normalized().base_structural_loci().len(), LOCI);
        let certified = source
            .normalized()
            .ir()
            .attempts()
            .iter()
            .filter_map(|attempt| match attempt {
                NormalizedCoverageAttempt::Certified(formula) => {
                    Some(formula.source_attempt_ordinal())
                }
                NormalizedCoverageAttempt::Unsupported { .. } => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(certified, CERTIFIED_ORDINALS);
        let unsupported = source
            .normalized()
            .ir()
            .attempts()
            .iter()
            .filter_map(|attempt| match attempt {
                NormalizedCoverageAttempt::Unsupported {
                    source_attempt_ordinal,
                } => Some(*source_attempt_ordinal),
                NormalizedCoverageAttempt::Certified(_) => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(unsupported, UNSUPPORTED_ORDINALS);

        reset_replayed_row_span_authentication_calls();
        let started = Instant::now();
        source.replay(&family, &context).unwrap();
        let source_replay = started.elapsed();
        let source_replay_authentication_calls = replayed_row_span_authentication_calls();
        assert_eq!(source_replay_authentication_calls, ATTEMPTS);

        let mut limits = ParametricSectorFormulaResidualLimits::default();
        limits.max_base_structural_loci = LOCI;
        limits.max_attempts = ATTEMPTS;
        limits.max_certified_attempts = CERTIFIED;
        limits.max_unsupported_candidate_references = UNSUPPORTED;
        limits.max_assignment_capacity_entries = 128;
        limits.max_depth = LOCI;
        limits.max_frontier_capacity_entries = 128;
        limits.max_cursor_retained_bytes = RETAINED_BYTE_LIMIT;
        limits.max_paths_yielded = 1;
        limits.max_path_decisions = LOCI;
        limits.max_path_capacity_entries = 128;
        limits.max_path_retained_bytes = RETAINED_BYTE_LIMIT;

        let started = Instant::now();
        let mut cursor = ParametricSectorFormulaResidualCursor::from_replayed_source(
            Arc::clone(&source),
            ParametricSectorFormulaResidualRequest::AnyResidual,
            limits,
        )
        .unwrap();
        let cursor_initialization = started.elapsed();
        let started = Instant::now();
        let path = cursor
            .next_path()
            .unwrap()
            .expect("the honest all-36 source must retain a residual cube");
        let direct_first_residual = started.elapsed();

        // No MTBDD compiler, owner, or decision DAG is constructed in this
        // direct stress.  The later backend-free assignment evaluator is an
        // independent semantic oracle for the residual cube.
        assert!(cursor.same_source_allocation(&source));
        assert!(path.same_source_allocation(&source));
        assert_eq!(
            cursor.branch_order_schema(),
            PARAMETRIC_SECTOR_FORMULA_RESIDUAL_BRANCH_ORDER_V1
        );
        assert_eq!(
            path.branch_order_schema(),
            PARAMETRIC_SECTOR_FORMULA_RESIDUAL_BRANCH_ORDER_V1
        );
        assert_eq!(path.yield_ordinal(), 1);
        assert_eq!(
            path.terminal_kind(),
            ParametricSectorFormulaResidualKind::Unsupported
        );
        assert_eq!(
            path.unsupported_candidate_ordinals().collect::<Vec<_>>(),
            UNSUPPORTED_ORDINALS
        );
        assert!(path.decisions().len() <= LOCI);
        let mut seen = [false; LOCI];
        for decision in path.decisions() {
            let locus = decision.structural_locus_ordinal();
            assert!(locus < LOCI);
            assert!(!seen[locus], "structural locus {locus} was split twice");
            seen[locus] = true;
        }
        assert!(cursor.stats().maximum_depth() <= LOCI);
        assert!(cursor.stats().peak_cursor_retained_bytes() <= RETAINED_BYTE_LIMIT);
        assert!(path.stats().retained_path_bytes() <= RETAINED_BYTE_LIMIT);
        let free_locus_count = LOCI - path.decisions().len();
        eprintln!(
            "K21 direct core: family/context={:?}, row-span-compile={:?}, adaptive-candidates={:?}, candidate-to-normalized-source={:?} (authentications={fixture_authentication_calls}), source-replay={:?} (authentications={source_replay_authentication_calls}), cursor-init={:?}, first-residual={:?}, decisions={}, free-loci={free_locus_count}; cursor-stats={:?}; path-stats={:?}",
            build.family_and_context,
            build.row_span,
            build.adaptive_candidates,
            build.candidate_to_normalized_source,
            source_replay,
            cursor_initialization,
            direct_first_residual,
            path.decisions().len(),
            cursor.stats(),
            path.stats(),
        );

        let started = Instant::now();
        let mut zero_by_locus = vec![false; LOCI];
        for decision in path.decisions() {
            zero_by_locus[decision.structural_locus_ordinal()] = matches!(
                decision.polarity(),
                ParametricSectorFormulaResidualPolarity::EqualZero
            );
        }
        let free_loci = seen
            .iter()
            .enumerate()
            .filter_map(|(ordinal, assigned)| (!assigned).then_some(ordinal))
            .collect::<Vec<_>>();
        let completions = 1usize
            .checked_shl(u32::try_from(free_loci.len()).unwrap())
            .filter(|count| *count <= MAX_REFERENCE_COMPLETIONS)
            .unwrap_or_else(|| {
                panic!(
                    "direct residual leaves {} free loci, exceeding the explicit semantic-oracle cap of {MAX_REFERENCE_COMPLETIONS} completions",
                    free_loci.len()
                )
            });
        for mask in 0..completions {
            for (bit, locus) in free_loci.iter().copied().enumerate() {
                zero_by_locus[locus] = mask & (1usize << bit) != 0;
            }
            assert!(matches!(
                reference_disposition_for_assignment(source.normalized().ir(), &zero_by_locus)
                    .unwrap(),
                ParametricSectorMtbddDisposition::Unsupported { candidate_ordinals }
                    if candidate_ordinals.as_ref() == UNSUPPORTED_ORDINALS
            ));
        }
        let semantic_reference_completions = started.elapsed();

        let started = Instant::now();
        path.replay(&family, &context).unwrap();
        let path_replay = started.elapsed();

        let exact = exact_limits(cursor.stats(), path.stats());
        let started = Instant::now();
        let mut exact_cursor = ParametricSectorFormulaResidualCursor::from_replayed_source(
            Arc::clone(&source),
            ParametricSectorFormulaResidualRequest::AnyResidual,
            exact,
        )
        .unwrap();
        let exact_path = exact_cursor.next_path().unwrap().unwrap();
        assert_eq!(exact_path.decisions(), path.decisions());
        assert_eq!(exact_path.terminal_kind(), path.terminal_kind());
        assert_eq!(exact_path.stats(), path.stats());
        let exact_first_residual = started.elapsed();

        let mut one_below_checked = false;
        let started = Instant::now();
        if cursor.stats().maximum_depth() > 0 {
            let mut one_below = exact;
            one_below.max_depth = cursor.stats().maximum_depth() - 1;
            let mut rejected = ParametricSectorFormulaResidualCursor::from_replayed_source(
                Arc::clone(&source),
                ParametricSectorFormulaResidualRequest::AnyResidual,
                one_below,
            )
            .unwrap();
            assert!(matches!(
                rejected.next_path(),
                Err(ParametricSectorFormulaResidualError::ResourceLimit {
                    resource: "formula-residual depth",
                    requested,
                    limit,
                }) if requested == cursor.stats().maximum_depth()
                    && limit + 1 == requested
            ));
            assert!(rejected.is_poisoned());
            one_below_checked = true;
        }
        let one_below_depth = started.elapsed();

        eprintln!(
            "K21 direct phases: family/context={:?}, row-span-compile={:?}, adaptive-candidates={:?}, candidate-to-normalized-source={:?} (authentications={fixture_authentication_calls}), source-replay={:?} (authentications={source_replay_authentication_calls}), cursor-init={:?}, first-residual={:?}, semantic-completions({completions})={:?}, path-replay={:?}, exact-first-residual={:?}, one-below-depth({one_below_checked})={:?}; cursor-stats={:?}; path-stats={:?}",
            build.family_and_context,
            build.row_span,
            build.adaptive_candidates,
            build.candidate_to_normalized_source,
            source_replay,
            cursor_initialization,
            direct_first_residual,
            semantic_reference_completions,
            path_replay,
            exact_first_residual,
            one_below_depth,
            cursor.stats(),
            path.stats(),
        );
    }

    #[test]
    fn deterministic_storage_preflight_precedes_candidate_authentication() {
        let (family, context, source) = one_loop_source("formula-residual-storage-preflight-order");
        let base_structural_loci = source.normalized().ir().base_structural_locus_count();
        let minimum_cursor_retained_bytes = cursor_retained_bytes(base_structural_loci, 0).unwrap();
        let authentication_calls = source.attempts().len();
        assert!(base_structural_loci > 0);
        assert!(minimum_cursor_retained_bytes > 0);
        assert!(authentication_calls > 0);

        let mut below_assignment = ParametricSectorFormulaResidualLimits::default();
        below_assignment.max_assignment_capacity_entries = base_structural_loci - 1;
        reset_replayed_row_span_authentication_calls();
        assert!(matches!(
            ParametricSectorFormulaResidualCursor::try_new(
                &family,
                &context,
                Arc::clone(&source),
                ParametricSectorFormulaResidualRequest::AnyResidual,
                below_assignment,
            ),
            Err(ParametricSectorFormulaResidualError::ResourceLimit {
                resource: "formula-residual assignment capacity entries",
                requested,
                limit,
            }) if requested == base_structural_loci && limit == base_structural_loci - 1
        ));
        assert_eq!(replayed_row_span_authentication_calls(), 0);

        let mut below_cursor_bytes = ParametricSectorFormulaResidualLimits::default();
        below_cursor_bytes.max_cursor_retained_bytes = minimum_cursor_retained_bytes - 1;
        reset_replayed_row_span_authentication_calls();
        assert!(matches!(
            ParametricSectorFormulaResidualCursor::try_new(
                &family,
                &context,
                Arc::clone(&source),
                ParametricSectorFormulaResidualRequest::AnyResidual,
                below_cursor_bytes,
            ),
            Err(ParametricSectorFormulaResidualError::ResourceLimit {
                resource: "formula-residual cursor retained bytes",
                requested,
                limit,
            }) if requested == minimum_cursor_retained_bytes
                && limit == minimum_cursor_retained_bytes - 1
        ));
        assert_eq!(replayed_row_span_authentication_calls(), 0);

        let mut exact = ParametricSectorFormulaResidualLimits::default();
        exact.max_assignment_capacity_entries = base_structural_loci;
        exact.max_cursor_retained_bytes = minimum_cursor_retained_bytes;
        reset_replayed_row_span_authentication_calls();
        let cursor = ParametricSectorFormulaResidualCursor::try_new(
            &family,
            &context,
            source,
            ParametricSectorFormulaResidualRequest::AnyResidual,
            exact,
        )
        .unwrap();
        assert_eq!(
            cursor.stats().assignment_capacity_entries(),
            base_structural_loci
        );
        assert_eq!(
            cursor.stats().peak_cursor_retained_bytes(),
            minimum_cursor_retained_bytes
        );
        assert_eq!(
            replayed_row_span_authentication_calls(),
            authentication_calls
        );
    }

    #[test]
    fn exact_and_one_below_limits_are_typed_and_failure_poisons_only_cursor() {
        let (family, context, source) = one_loop_source("formula-residual-exact-limits");
        let mut baseline = ParametricSectorFormulaResidualCursor::try_new(
            &family,
            &context,
            Arc::clone(&source),
            ParametricSectorFormulaResidualRequest::AnyResidual,
            ParametricSectorFormulaResidualLimits::default(),
        )
        .unwrap();
        let baseline_path = baseline.next_path().unwrap().unwrap();
        let exact = exact_limits(baseline.stats(), baseline_path.stats());
        let exact_path = first_path_with_limits(&family, &context, &source, exact).unwrap();
        exact_path.replay(&family, &context).unwrap();

        macro_rules! one_below {
            ($field:ident, $value:expr, $resource:literal) => {{
                let value = $value;
                if value > 0 {
                    let mut limits = exact;
                    limits.$field = value - 1;
                    assert!(matches!(
                        first_path_with_limits(&family, &context, &source, limits),
                        Err(ParametricSectorFormulaResidualError::ResourceLimit {
                            resource: $resource,
                            requested,
                            limit,
                        }) if requested == value && limit == value - 1
                    ));
                }
            }};
        }

        let cursor = baseline.stats();
        let path = baseline_path.stats();
        one_below!(
            max_base_structural_loci,
            cursor.base_structural_loci(),
            "formula-residual base structural loci"
        );
        one_below!(max_attempts, cursor.attempts(), "formula-residual attempts");
        one_below!(
            max_certified_attempts,
            cursor.certified_attempts(),
            "formula-residual certified attempts"
        );
        one_below!(
            max_assignment_capacity_entries,
            cursor.assignment_capacity_entries(),
            "formula-residual assignment capacity entries"
        );
        one_below!(
            max_state_classifications,
            cursor.state_classifications(),
            "formula-residual state classifications"
        );
        one_below!(
            max_attempt_visits,
            cursor.attempt_visits(),
            "formula-residual attempt visits"
        );
        one_below!(
            max_formula_evaluations,
            cursor.formula_evaluations(),
            "formula-residual formula evaluations"
        );
        one_below!(
            max_formula_clause_charges,
            cursor.formula_clause_charges(),
            "formula-residual formula clause charges"
        );
        one_below!(
            max_literal_query_charges,
            cursor.literal_query_charges(),
            "formula-residual literal query charges"
        );
        one_below!(
            max_good_routes,
            cursor.good_routes(),
            "formula-residual good routes"
        );
        one_below!(
            max_bad_routes,
            cursor.bad_routes(),
            "formula-residual bad routes"
        );
        one_below!(
            max_split_routes,
            cursor.split_routes(),
            "formula-residual split routes"
        );
        one_below!(
            max_covered_prunes,
            cursor.covered_prunes(),
            "formula-residual covered prunes"
        );
        one_below!(
            max_residual_terminal_visits,
            cursor.residual_terminal_visits(),
            "formula-residual terminal visits"
        );
        one_below!(
            max_branch_traversals,
            cursor.branch_traversals(),
            "formula-residual branch traversals"
        );
        one_below!(
            max_backtracks,
            cursor.backtracks(),
            "formula-residual backtracks"
        );
        one_below!(max_depth, cursor.maximum_depth(), "formula-residual depth");
        one_below!(
            max_frontier_capacity_entries,
            cursor.peak_frontier_capacity_entries(),
            "formula-residual frontier capacity entries"
        );
        one_below!(
            max_cursor_retained_bytes,
            cursor.peak_cursor_retained_bytes(),
            "formula-residual cursor retained bytes"
        );
        one_below!(
            max_paths_yielded,
            cursor.paths_yielded(),
            "formula-residual paths yielded"
        );
        one_below!(
            max_total_path_decisions_copied,
            cursor.total_path_decisions_copied(),
            "formula-residual total path decisions copied"
        );
        one_below!(
            max_path_decisions,
            path.decisions(),
            "formula-residual path decisions"
        );
        one_below!(
            max_path_capacity_entries,
            path.decision_capacity_entries(),
            "formula-residual path capacity entries"
        );
        one_below!(
            max_path_retained_bytes,
            path.retained_path_bytes(),
            "formula-residual path retained bytes"
        );

        let mut poisoned_limits = ParametricSectorFormulaResidualLimits::default();
        poisoned_limits.max_state_classifications = 0;
        let mut poisoned = ParametricSectorFormulaResidualCursor::try_new(
            &family,
            &context,
            Arc::clone(&source),
            ParametricSectorFormulaResidualRequest::AnyResidual,
            poisoned_limits,
        )
        .unwrap();
        assert!(matches!(
            poisoned.next_path(),
            Err(ParametricSectorFormulaResidualError::ResourceLimit {
                resource: "formula-residual state classifications",
                requested: 1,
                limit: 0,
            })
        ));
        assert!(poisoned.is_poisoned());
        assert!(matches!(
            poisoned.next_path(),
            Err(ParametricSectorFormulaResidualError::CursorPoisoned)
        ));
        source.replay(&family, &context).unwrap();
        assert!(
            first_path_with_limits(
                &family,
                &context,
                &source,
                ParametricSectorFormulaResidualLimits::default()
            )
            .is_ok()
        );
    }
}
