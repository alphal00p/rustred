//! Bounded lazy residual paths over authenticated sector-coverage MTBDDs.
//!
//! This is the first scalable hand-off from reduced coverage to downstream
//! conditional derivation.  The cursor owns one shared reference to the exact
//! stage-1 coverage certificate and performs deterministic false/nonzero-first
//! depth-first traversal directly over its persisted rooted view.  It retains
//! only the current root-to-reference frontier; it never constructs a visited
//! set, a leaf partition, or a collection of all root-to-terminal paths.

use std::fmt;
use std::mem::size_of;
use std::sync::Arc;

use crate::coverage_decision_dag::{CoverageDecisionPersistedRef, CoverageDecisionTerminalId};
use crate::parametric_sector_mtbdd::{
    ParametricSectorMtbddDisposition, ParametricSectorMtbddTerminalPayload,
};
use crate::parametric_sector_mtbdd_certificate::{
    PARAMETRIC_SECTOR_MTBDD_COVERAGE_V5_STAGE1_SCHEMA, ParametricSectorMtbddCoverageCertificate,
    ParametricSectorMtbddCoverageError,
};
use crate::{IntegralFamily, ParametricCoefficientContext, ParametricPolynomial};

pub(crate) const PARAMETRIC_SECTOR_RESIDUAL_PATH_CURSOR_V1_SCHEMA: &str =
    "rustred-parametric-sector-residual-path-cursor-v1";
pub(crate) const PARAMETRIC_SECTOR_RESIDUAL_PATH_V1_SCHEMA: &str =
    "rustred-parametric-sector-residual-path-v1";
pub(crate) const PARAMETRIC_SECTOR_RESIDUAL_PATH_BRANCH_ORDER_V1: &str =
    "rustred-parametric-sector-residual-path-nonzero-before-equal-zero-v1";

/// Residual terminal requested by one lazy lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ParametricSectorResidualPathRequest {
    AnyResidual,
    Uncovered,
    Unsupported,
}

impl ParametricSectorResidualPathRequest {
    const fn accepts(self, terminal: ParametricSectorResidualPathTerminalKind) -> bool {
        matches!(self, Self::AnyResidual)
            || matches!(
                (self, terminal),
                (
                    Self::Uncovered,
                    ParametricSectorResidualPathTerminalKind::Uncovered
                ) | (
                    Self::Unsupported,
                    ParametricSectorResidualPathTerminalKind::Unsupported
                )
            )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ParametricSectorResidualPathTerminalKind {
    Uncovered,
    Unsupported,
}

/// Exact truth value of one base structural locus on a residual path.
///
/// MTBDD false edges mean that the corresponding polynomial is nonzero;
/// true edges mean that it is equal to zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ParametricSectorResidualPathPolarity {
    NonZero,
    EqualZero,
}

/// Complete provenance for one branch on the retained path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ParametricSectorResidualPathDecision {
    node_ordinal: usize,
    atom_ordinal: usize,
    structural_locus_ordinal: usize,
    polarity: ParametricSectorResidualPathPolarity,
}

impl ParametricSectorResidualPathDecision {
    pub(crate) const fn node_ordinal(self) -> usize {
        self.node_ordinal
    }

    pub(crate) const fn atom_ordinal(self) -> usize {
        self.atom_ordinal
    }

    pub(crate) const fn structural_locus_ordinal(self) -> usize {
        self.structural_locus_ordinal
    }

    pub(crate) const fn polarity(self) -> ParametricSectorResidualPathPolarity {
        self.polarity
    }
}

/// Independent resource envelope for traversal and the one retained path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ParametricSectorResidualPathLimits {
    pub(crate) max_reference_visits: usize,
    pub(crate) max_node_visits: usize,
    pub(crate) max_terminal_visits: usize,
    pub(crate) max_branch_traversals: usize,
    pub(crate) max_backtracks: usize,
    pub(crate) max_depth: usize,
    pub(crate) max_frontier_capacity_entries: usize,
    pub(crate) max_cursor_retained_bytes: usize,
    pub(crate) max_descending_terminals_skipped: usize,
    pub(crate) max_filtered_residual_terminals: usize,
    pub(crate) max_paths_yielded: usize,
    pub(crate) max_total_path_decisions_copied: usize,
    pub(crate) max_path_decisions: usize,
    pub(crate) max_path_retained_bytes: usize,
    pub(crate) max_unsupported_candidate_references: usize,
}

impl Default for ParametricSectorResidualPathLimits {
    fn default() -> Self {
        Self {
            max_reference_visits: 256_000_000,
            max_node_visits: 256_000_000,
            max_terminal_visits: 256_000_000,
            max_branch_traversals: 256_000_000,
            max_backtracks: 256_000_000,
            max_depth: 16_000_000,
            max_frontier_capacity_entries: 16_000_000,
            max_cursor_retained_bytes: 1024 * 1024 * 1024,
            max_descending_terminals_skipped: 16_000_000,
            max_filtered_residual_terminals: 16_000_000,
            max_paths_yielded: 16_000_000,
            max_total_path_decisions_copied: 256_000_000,
            max_path_decisions: 16_000_000,
            max_path_retained_bytes: 1024 * 1024 * 1024,
            max_unsupported_candidate_references: 16_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ParametricSectorResidualPathCursorStats {
    source_references: usize,
    reference_visits: usize,
    node_visits: usize,
    terminal_visits: usize,
    branch_traversals: usize,
    backtracks: usize,
    maximum_depth: usize,
    peak_frontier_capacity_entries: usize,
    peak_cursor_retained_bytes: usize,
    descending_terminals_skipped: usize,
    uncovered_terminals_visited: usize,
    unsupported_terminals_visited: usize,
    filtered_residual_terminals: usize,
    paths_yielded: usize,
    total_path_decisions_copied: usize,
    structural_locus_translations: usize,
}

macro_rules! cursor_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl ParametricSectorResidualPathCursorStats {
    cursor_stats_getters!(
        source_references,
        reference_visits,
        node_visits,
        terminal_visits,
        branch_traversals,
        backtracks,
        maximum_depth,
        peak_frontier_capacity_entries,
        peak_cursor_retained_bytes,
        descending_terminals_skipped,
        uncovered_terminals_visited,
        unsupported_terminals_visited,
        filtered_residual_terminals,
        paths_yielded,
        total_path_decisions_copied,
        structural_locus_translations,
    );
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ParametricSectorResidualPathStats {
    decisions: usize,
    nonzero_decisions: usize,
    equal_zero_decisions: usize,
    unsupported_candidate_references: usize,
    retained_path_bytes: usize,
}

impl ParametricSectorResidualPathStats {
    pub(crate) const fn decisions(self) -> usize {
        self.decisions
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
pub(crate) enum ParametricSectorResidualPathError {
    SourceSchemaMismatch,
    SourceReplay(ParametricSectorMtbddCoverageError),
    MalformedRootCount {
        actual: usize,
    },
    NodeOutOfRange {
        ordinal: usize,
        node_count: usize,
    },
    TerminalOutOfRange {
        ordinal: usize,
        terminal_count: usize,
    },
    AtomOutOfRange {
        ordinal: usize,
        atom_count: usize,
    },
    StructuralLocusOutOfRange {
        ordinal: usize,
        locus_count: usize,
    },
    SourceShapeMismatch,
    BooleanTerminalReachable,
    TerminalDispositionMismatch,
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

impl fmt::Display for ParametricSectorResidualPathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "parametric sector residual-path error: {self:?}")
    }
}

impl std::error::Error for ParametricSectorResidualPathError {}

/// One exact residual conjunction bound to the same source allocation as the
/// cursor that produced it.
///
/// This V1 identity is deliberately process-local and non-durable. `Arc`
/// allocation identity is sufficient for the first in-process hand-off, while
/// a serialized V6 owner will need its own exact count-delimited source
/// identity. No digest or debug rendering is treated as proof material here.
#[derive(Clone)]
pub(crate) struct ParametricSectorResidualPathCertificate {
    schema: &'static str,
    branch_order_schema: &'static str,
    source: Arc<ParametricSectorMtbddCoverageCertificate>,
    source_root: CoverageDecisionPersistedRef,
    request: ParametricSectorResidualPathRequest,
    yield_ordinal: usize,
    decisions: Box<[ParametricSectorResidualPathDecision]>,
    terminal_ordinal: usize,
    terminal_kind: ParametricSectorResidualPathTerminalKind,
    limits: ParametricSectorResidualPathLimits,
    stats: ParametricSectorResidualPathStats,
}

impl fmt::Debug for ParametricSectorResidualPathCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ParametricSectorResidualPathCertificate")
            .field("schema", &self.schema)
            .field("branch_order_schema", &self.branch_order_schema)
            .field("request", &self.request)
            .field("yield_ordinal", &self.yield_ordinal)
            .field("decisions", &self.decisions)
            .field("terminal_ordinal", &self.terminal_ordinal)
            .field("terminal_kind", &self.terminal_kind)
            .field("stats", &self.stats)
            .field("source", &"<shared authenticated MTBDD coverage>")
            .finish()
    }
}

impl ParametricSectorResidualPathCertificate {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) const fn branch_order_schema(&self) -> &'static str {
        self.branch_order_schema
    }

    pub(crate) const fn request(&self) -> ParametricSectorResidualPathRequest {
        self.request
    }

    /// One-based ordinal among paths accepted by this cursor request.
    pub(crate) const fn yield_ordinal(&self) -> usize {
        self.yield_ordinal
    }

    pub(crate) fn decisions(&self) -> &[ParametricSectorResidualPathDecision] {
        &self.decisions
    }

    pub(crate) const fn terminal_ordinal(&self) -> usize {
        self.terminal_ordinal
    }

    pub(crate) const fn terminal_kind(&self) -> ParametricSectorResidualPathTerminalKind {
        self.terminal_kind
    }

    pub(crate) const fn limits(&self) -> ParametricSectorResidualPathLimits {
        self.limits
    }

    pub(crate) const fn stats(&self) -> ParametricSectorResidualPathStats {
        self.stats
    }

    pub(crate) fn same_source_allocation(
        &self,
        source: &Arc<ParametricSectorMtbddCoverageCertificate>,
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
            .structural_locus_ordinal;
        self.source.normalized().base_structural_loci().get(locus)
    }

    pub(crate) fn terminal_disposition(
        &self,
    ) -> Result<&ParametricSectorMtbddDisposition, ParametricSectorResidualPathError> {
        terminal_disposition(self.source.as_ref(), self.terminal_ordinal)
    }

    /// Reauthenticate the complete source and reproduce the exact selected
    /// path by replaying the same request and false/nonzero-first traversal up
    /// to this certificate's one-based yield ordinal.
    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), ParametricSectorResidualPathError> {
        self.source
            .replay(family, context)
            .map_err(ParametricSectorResidualPathError::SourceReplay)?;
        if self.yield_ordinal == 0 {
            return Err(ParametricSectorResidualPathError::PathReplayMismatch);
        }
        let mut cursor = ParametricSectorResidualPathCursor::from_replayed_source(
            Arc::clone(&self.source),
            self.request,
            self.limits,
        )?;
        let mut replayed = None;
        for _ in 0..self.yield_ordinal {
            replayed = cursor.next_path()?;
            if replayed.is_none() {
                return Err(ParametricSectorResidualPathError::PathReplayMismatch);
            }
        }
        if replayed
            .as_ref()
            .is_some_and(|candidate| self.payload_eq(candidate))
        {
            Ok(())
        } else {
            Err(ParametricSectorResidualPathError::PathReplayMismatch)
        }
    }

    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.branch_order_schema == other.branch_order_schema
            && Arc::ptr_eq(&self.source, &other.source)
            && self.source_root == other.source_root
            && self.request == other.request
            && self.yield_ordinal == other.yield_ordinal
            && self.decisions == other.decisions
            && self.terminal_ordinal == other.terminal_ordinal
            && self.terminal_kind == other.terminal_kind
            && self.limits == other.limits
            && self.stats == other.stats
    }
}

/// Resumable deterministic DFS. Resource failures poison only this cursor; the
/// retained source allocation and any previously yielded path remain valid.
pub(crate) struct ParametricSectorResidualPathCursor {
    schema: &'static str,
    source: Arc<ParametricSectorMtbddCoverageCertificate>,
    source_root: CoverageDecisionPersistedRef,
    request: ParametricSectorResidualPathRequest,
    current: Option<CoverageDecisionPersistedRef>,
    frontier: Vec<ParametricSectorResidualPathDecision>,
    resume_after_terminal: bool,
    exhausted: bool,
    poisoned: bool,
    limits: ParametricSectorResidualPathLimits,
    stats: ParametricSectorResidualPathCursorStats,
}

impl fmt::Debug for ParametricSectorResidualPathCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ParametricSectorResidualPathCursor")
            .field("schema", &self.schema)
            .field("request", &self.request)
            .field("current", &self.current)
            .field("frontier_depth", &self.frontier.len())
            .field("resume_after_terminal", &self.resume_after_terminal)
            .field("exhausted", &self.exhausted)
            .field("poisoned", &self.poisoned)
            .field("stats", &self.stats)
            .field("source", &"<shared authenticated MTBDD coverage>")
            .finish()
    }
}

impl ParametricSectorResidualPathCursor {
    pub(crate) fn try_new(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source: Arc<ParametricSectorMtbddCoverageCertificate>,
        request: ParametricSectorResidualPathRequest,
        limits: ParametricSectorResidualPathLimits,
    ) -> Result<Self, ParametricSectorResidualPathError> {
        source
            .replay(family, context)
            .map_err(ParametricSectorResidualPathError::SourceReplay)?;
        Self::from_replayed_source(source, request, limits)
    }

    /// Construct only after the caller has authenticated this exact source in
    /// the current control flow. Kept private so a bare `Arc` is never exposed
    /// as replay authority.
    fn from_replayed_source(
        source: Arc<ParametricSectorMtbddCoverageCertificate>,
        request: ParametricSectorResidualPathRequest,
        limits: ParametricSectorResidualPathLimits,
    ) -> Result<Self, ParametricSectorResidualPathError> {
        validate_source_header(source.as_ref())?;
        let source_root = single_root(source.as_ref())?;
        let retained = size_of::<Self>();
        check_limit(
            "residual-path cursor retained bytes",
            retained,
            limits.max_cursor_retained_bytes,
        )?;
        Ok(Self {
            schema: PARAMETRIC_SECTOR_RESIDUAL_PATH_CURSOR_V1_SCHEMA,
            source,
            source_root,
            request,
            current: Some(source_root),
            frontier: Vec::new(),
            resume_after_terminal: false,
            exhausted: false,
            poisoned: false,
            limits,
            stats: ParametricSectorResidualPathCursorStats {
                source_references: 1,
                peak_cursor_retained_bytes: retained,
                ..ParametricSectorResidualPathCursorStats::default()
            },
        })
    }

    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) const fn branch_order_schema(&self) -> &'static str {
        PARAMETRIC_SECTOR_RESIDUAL_PATH_BRANCH_ORDER_V1
    }

    pub(crate) const fn limits(&self) -> ParametricSectorResidualPathLimits {
        self.limits
    }

    pub(crate) const fn request(&self) -> ParametricSectorResidualPathRequest {
        self.request
    }

    pub(crate) const fn stats(&self) -> ParametricSectorResidualPathCursorStats {
        self.stats
    }

    pub(crate) fn same_source_allocation(
        &self,
        source: &Arc<ParametricSectorMtbddCoverageCertificate>,
    ) -> bool {
        Arc::ptr_eq(&self.source, source)
    }

    pub(crate) fn frontier(&self) -> &[ParametricSectorResidualPathDecision] {
        &self.frontier
    }

    pub(crate) const fn is_exhausted(&self) -> bool {
        self.exhausted
    }

    pub(crate) const fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    pub(crate) fn retained_bytes(&self) -> Result<usize, ParametricSectorResidualPathError> {
        cursor_retained_bytes(self.frontier.capacity())
    }

    /// Return the next path matching the request bound at construction.
    /// Descending-rule terminals are always skipped. Calling this repeatedly
    /// resumes after the last yielded terminal rather than restarting at root.
    pub(crate) fn next_path(
        &mut self,
    ) -> Result<Option<ParametricSectorResidualPathCertificate>, ParametricSectorResidualPathError>
    {
        if self.poisoned {
            return Err(ParametricSectorResidualPathError::CursorPoisoned);
        }
        if self.exhausted {
            return Ok(None);
        }
        let result = self.next_path_inner();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn next_path_inner(
        &mut self,
    ) -> Result<Option<ParametricSectorResidualPathCertificate>, ParametricSectorResidualPathError>
    {
        loop {
            if self.resume_after_terminal {
                self.resume_after_terminal = false;
                if !self.advance_after_terminal()? {
                    return Ok(None);
                }
            }
            let Some(current) = self.current else {
                self.exhausted = true;
                return Ok(None);
            };
            match current {
                CoverageDecisionPersistedRef::Node(node) => self.descend_nonzero(node.ordinal())?,
                CoverageDecisionPersistedRef::Terminal(terminal) => {
                    charge_next(
                        "residual-path reference visits",
                        &mut self.stats.reference_visits,
                        self.limits.max_reference_visits,
                    )?;
                    charge_next(
                        "residual-path terminal visits",
                        &mut self.stats.terminal_visits,
                        self.limits.max_terminal_visits,
                    )?;
                    let payload = terminal_payload(self.source.as_ref(), terminal)?;
                    match payload {
                        ParametricSectorMtbddTerminalPayload::BooleanFalse
                        | ParametricSectorMtbddTerminalPayload::BooleanTrue => {
                            return Err(
                                ParametricSectorResidualPathError::BooleanTerminalReachable,
                            );
                        }
                        ParametricSectorMtbddTerminalPayload::Disposition(
                            ParametricSectorMtbddDisposition::DescendingRule { .. },
                        ) => {
                            charge_next(
                                "residual-path descending terminals skipped",
                                &mut self.stats.descending_terminals_skipped,
                                self.limits.max_descending_terminals_skipped,
                            )?;
                            self.resume_after_terminal = true;
                        }
                        ParametricSectorMtbddTerminalPayload::Disposition(disposition) => {
                            let (kind, unsupported_references) =
                                residual_terminal_census(disposition)?;
                            match kind {
                                ParametricSectorResidualPathTerminalKind::Uncovered => charge_next(
                                    "residual-path uncovered terminals visited",
                                    &mut self.stats.uncovered_terminals_visited,
                                    self.limits.max_terminal_visits,
                                )?,
                                ParametricSectorResidualPathTerminalKind::Unsupported => {
                                    charge_next(
                                        "residual-path unsupported terminals visited",
                                        &mut self.stats.unsupported_terminals_visited,
                                        self.limits.max_terminal_visits,
                                    )?
                                }
                            }
                            self.resume_after_terminal = true;
                            if !self.request.accepts(kind) {
                                charge_next(
                                    "residual-path filtered residual terminals",
                                    &mut self.stats.filtered_residual_terminals,
                                    self.limits.max_filtered_residual_terminals,
                                )?;
                                continue;
                            }
                            let next_paths =
                                checked_add("residual paths yielded", self.stats.paths_yielded, 1)?;
                            check_limit(
                                "residual paths yielded",
                                next_paths,
                                self.limits.max_paths_yielded,
                            )?;
                            let certificate = self.build_certificate(
                                next_paths,
                                terminal.ordinal(),
                                kind,
                                unsupported_references,
                            )?;
                            self.stats.paths_yielded = next_paths;
                            return Ok(Some(certificate));
                        }
                    }
                }
            }
        }
    }

    fn descend_nonzero(
        &mut self,
        node_ordinal: usize,
    ) -> Result<(), ParametricSectorResidualPathError> {
        let rooted = self.source.decision().rooted();
        let node = rooted.nodes().get(node_ordinal).copied().ok_or(
            ParametricSectorResidualPathError::NodeOutOfRange {
                ordinal: node_ordinal,
                node_count: rooted.nodes().len(),
            },
        )?;
        let atom_ordinal = node.atom().ordinal();
        let atom = self.source.decision().atoms().get(atom_ordinal).ok_or(
            ParametricSectorResidualPathError::AtomOutOfRange {
                ordinal: atom_ordinal,
                atom_count: self.source.decision().atoms().len(),
            },
        )?;
        let structural_locus_ordinal = atom.structural_locus_ordinal();
        if self
            .source
            .normalized()
            .base_structural_loci()
            .get(structural_locus_ordinal)
            .is_none()
        {
            return Err(
                ParametricSectorResidualPathError::StructuralLocusOutOfRange {
                    ordinal: structural_locus_ordinal,
                    locus_count: self.source.normalized().base_structural_loci().len(),
                },
            );
        }
        let next_depth = checked_add("residual-path depth", self.frontier.len(), 1)?;
        check_limit("residual-path depth", next_depth, self.limits.max_depth)?;
        // Preflight the logical next entry. Geometric growth below preserves
        // amortized O(depth) frontier-copy work; the allocator's observed
        // capacity is still checked independently after reservation.
        check_limit(
            "residual-path frontier capacity entries",
            next_depth,
            self.limits.max_frontier_capacity_entries,
        )?;
        check_limit(
            "residual-path cursor retained bytes",
            cursor_retained_bytes(next_depth)?,
            self.limits.max_cursor_retained_bytes,
        )?;
        let next_visits = checked_add(
            "residual-path reference visits",
            self.stats.reference_visits,
            1,
        )?;
        check_limit(
            "residual-path reference visits",
            next_visits,
            self.limits.max_reference_visits,
        )?;
        let next_node_visits = checked_add("residual-path node visits", self.stats.node_visits, 1)?;
        check_limit(
            "residual-path node visits",
            next_node_visits,
            self.limits.max_node_visits,
        )?;
        let next_branches = checked_add(
            "residual-path branch traversals",
            self.stats.branch_traversals,
            1,
        )?;
        check_limit(
            "residual-path branch traversals",
            next_branches,
            self.limits.max_branch_traversals,
        )?;
        self.reserve_frontier_for_depth(next_depth)?;
        self.frontier.push(ParametricSectorResidualPathDecision {
            node_ordinal,
            atom_ordinal,
            structural_locus_ordinal,
            polarity: ParametricSectorResidualPathPolarity::NonZero,
        });
        self.stats.reference_visits = next_visits;
        self.stats.node_visits = next_node_visits;
        self.stats.branch_traversals = next_branches;
        // There is exactly one translation per admitted node visit.
        self.stats.structural_locus_translations = next_node_visits;
        self.stats.maximum_depth = self.stats.maximum_depth.max(next_depth);
        self.current = Some(node.when_false());
        Ok(())
    }

    fn advance_after_terminal(&mut self) -> Result<bool, ParametricSectorResidualPathError> {
        loop {
            let Some(last) = self.frontier.last().copied() else {
                self.current = None;
                self.exhausted = true;
                return Ok(false);
            };
            let next_backtracks =
                checked_add("residual-path backtracks", self.stats.backtracks, 1)?;
            check_limit(
                "residual-path backtracks",
                next_backtracks,
                self.limits.max_backtracks,
            )?;
            if last.polarity == ParametricSectorResidualPathPolarity::NonZero {
                let node = self
                    .source
                    .decision()
                    .rooted()
                    .nodes()
                    .get(last.node_ordinal)
                    .copied()
                    .ok_or(ParametricSectorResidualPathError::NodeOutOfRange {
                        ordinal: last.node_ordinal,
                        node_count: self.source.decision().rooted().nodes().len(),
                    })?;
                if node.atom().ordinal() != last.atom_ordinal {
                    return Err(ParametricSectorResidualPathError::PathReplayMismatch);
                }
                let next_branches = checked_add(
                    "residual-path branch traversals",
                    self.stats.branch_traversals,
                    1,
                )?;
                check_limit(
                    "residual-path branch traversals",
                    next_branches,
                    self.limits.max_branch_traversals,
                )?;
                let Some(last_mut) = self.frontier.last_mut() else {
                    return Err(ParametricSectorResidualPathError::PathReplayMismatch);
                };
                last_mut.polarity = ParametricSectorResidualPathPolarity::EqualZero;
                self.stats.backtracks = next_backtracks;
                self.stats.branch_traversals = next_branches;
                self.current = Some(node.when_true());
                return Ok(true);
            }
            self.stats.backtracks = next_backtracks;
            self.frontier.pop();
        }
    }

    fn observe_frontier_capacity(&mut self) -> Result<(), ParametricSectorResidualPathError> {
        let capacity = self.frontier.capacity();
        check_limit(
            "residual-path frontier capacity entries",
            capacity,
            self.limits.max_frontier_capacity_entries,
        )?;
        let retained = cursor_retained_bytes(capacity)?;
        check_limit(
            "residual-path cursor retained bytes",
            retained,
            self.limits.max_cursor_retained_bytes,
        )?;
        self.stats.peak_frontier_capacity_entries =
            self.stats.peak_frontier_capacity_entries.max(capacity);
        self.stats.peak_cursor_retained_bytes = self.stats.peak_cursor_retained_bytes.max(retained);
        Ok(())
    }

    fn reserve_frontier_for_depth(
        &mut self,
        next_depth: usize,
    ) -> Result<(), ParametricSectorResidualPathError> {
        if self.frontier.capacity() >= next_depth {
            return Ok(());
        }
        let doubled = if self.frontier.capacity() == 0 {
            1
        } else {
            checked_mul(
                "residual-path frontier growth entries",
                self.frontier.capacity(),
                2,
            )?
        };
        let target = doubled.max(next_depth);
        check_limit(
            "residual-path frontier capacity entries",
            target,
            self.limits.max_frontier_capacity_entries,
        )?;
        check_limit(
            "residual-path cursor retained bytes",
            cursor_retained_bytes(target)?,
            self.limits.max_cursor_retained_bytes,
        )?;
        let additional = target.checked_sub(self.frontier.len()).ok_or(
            ParametricSectorResidualPathError::ResourceCountOverflow {
                resource: "residual-path frontier growth entries",
            },
        )?;
        self.frontier.try_reserve_exact(additional).map_err(|_| {
            ParametricSectorResidualPathError::AllocationFailure {
                resource: "residual-path frontier entries",
                requested: target,
            }
        })?;
        self.observe_frontier_capacity()
    }

    fn build_certificate(
        &mut self,
        yield_ordinal: usize,
        terminal_ordinal: usize,
        terminal_kind: ParametricSectorResidualPathTerminalKind,
        unsupported_references: usize,
    ) -> Result<ParametricSectorResidualPathCertificate, ParametricSectorResidualPathError> {
        let next_total_path_decisions_copied = checked_add(
            "residual-path total decisions copied",
            self.stats.total_path_decisions_copied,
            self.frontier.len(),
        )?;
        check_limit(
            "residual-path total decisions copied",
            next_total_path_decisions_copied,
            self.limits.max_total_path_decisions_copied,
        )?;
        check_limit(
            "residual-path retained decisions",
            self.frontier.len(),
            self.limits.max_path_decisions,
        )?;
        check_limit(
            "residual-path unsupported candidate references",
            unsupported_references,
            self.limits.max_unsupported_candidate_references,
        )?;
        let logical_path_bytes = checked_add(
            "residual-path retained bytes",
            size_of::<ParametricSectorResidualPathCertificate>(),
            checked_mul(
                "residual-path retained bytes",
                self.frontier.len(),
                size_of::<ParametricSectorResidualPathDecision>(),
            )?,
        )?;
        check_limit(
            "residual-path retained bytes",
            logical_path_bytes,
            self.limits.max_path_retained_bytes,
        )?;
        let mut decisions = Vec::new();
        decisions
            .try_reserve_exact(self.frontier.len())
            .map_err(|_| ParametricSectorResidualPathError::AllocationFailure {
                resource: "residual-path retained decisions",
                requested: self.frontier.len(),
            })?;
        decisions.extend_from_slice(&self.frontier);
        let decisions = decisions.into_boxed_slice();
        let stats = path_stats(&decisions, unsupported_references, self.limits)?;
        self.stats.total_path_decisions_copied = next_total_path_decisions_copied;
        Ok(ParametricSectorResidualPathCertificate {
            schema: PARAMETRIC_SECTOR_RESIDUAL_PATH_V1_SCHEMA,
            branch_order_schema: PARAMETRIC_SECTOR_RESIDUAL_PATH_BRANCH_ORDER_V1,
            source: Arc::clone(&self.source),
            source_root: self.source_root,
            request: self.request,
            yield_ordinal,
            decisions,
            terminal_ordinal,
            terminal_kind,
            limits: self.limits,
            stats,
        })
    }
}

fn validate_source_header(
    source: &ParametricSectorMtbddCoverageCertificate,
) -> Result<(), ParametricSectorResidualPathError> {
    if source.schema() != PARAMETRIC_SECTOR_MTBDD_COVERAGE_V5_STAGE1_SCHEMA {
        return Err(ParametricSectorResidualPathError::SourceSchemaMismatch);
    }
    if source.decision().base_structural_locus_count()
        != source.normalized().base_structural_loci().len()
        || source.normalized().ir().base_structural_locus_count()
            != source.normalized().base_structural_loci().len()
        || source.decision().formula_schema() != source.normalized().ir().schema()
    {
        return Err(ParametricSectorResidualPathError::SourceShapeMismatch);
    }
    Ok(())
}

fn single_root(
    source: &ParametricSectorMtbddCoverageCertificate,
) -> Result<CoverageDecisionPersistedRef, ParametricSectorResidualPathError> {
    let roots = source.decision().rooted().roots();
    if roots.len() != 1 {
        return Err(ParametricSectorResidualPathError::MalformedRootCount {
            actual: roots.len(),
        });
    }
    Ok(roots[0])
}

fn terminal_payload(
    source: &ParametricSectorMtbddCoverageCertificate,
    terminal: CoverageDecisionTerminalId,
) -> Result<&ParametricSectorMtbddTerminalPayload, ParametricSectorResidualPathError> {
    source
        .decision()
        .rooted()
        .terminal_payloads()
        .get(terminal.ordinal())
        .map(Arc::as_ref)
        .ok_or(ParametricSectorResidualPathError::TerminalOutOfRange {
            ordinal: terminal.ordinal(),
            terminal_count: source.decision().rooted().terminal_payloads().len(),
        })
}

fn terminal_disposition(
    source: &ParametricSectorMtbddCoverageCertificate,
    terminal_ordinal: usize,
) -> Result<&ParametricSectorMtbddDisposition, ParametricSectorResidualPathError> {
    let payload = source
        .decision()
        .rooted()
        .terminal_payloads()
        .get(terminal_ordinal)
        .map(Arc::as_ref)
        .ok_or(ParametricSectorResidualPathError::TerminalOutOfRange {
            ordinal: terminal_ordinal,
            terminal_count: source.decision().rooted().terminal_payloads().len(),
        })?;
    match payload {
        ParametricSectorMtbddTerminalPayload::Disposition(disposition) => Ok(disposition),
        ParametricSectorMtbddTerminalPayload::BooleanFalse
        | ParametricSectorMtbddTerminalPayload::BooleanTrue => {
            Err(ParametricSectorResidualPathError::BooleanTerminalReachable)
        }
    }
}

fn residual_terminal_census(
    disposition: &ParametricSectorMtbddDisposition,
) -> Result<(ParametricSectorResidualPathTerminalKind, usize), ParametricSectorResidualPathError> {
    match disposition {
        ParametricSectorMtbddDisposition::DescendingRule { .. } => {
            Err(ParametricSectorResidualPathError::TerminalDispositionMismatch)
        }
        ParametricSectorMtbddDisposition::Uncovered => {
            Ok((ParametricSectorResidualPathTerminalKind::Uncovered, 0))
        }
        ParametricSectorMtbddDisposition::Unsupported { candidate_ordinals } => Ok((
            ParametricSectorResidualPathTerminalKind::Unsupported,
            candidate_ordinals.len(),
        )),
    }
}

fn path_stats(
    decisions: &[ParametricSectorResidualPathDecision],
    unsupported_candidate_references: usize,
    limits: ParametricSectorResidualPathLimits,
) -> Result<ParametricSectorResidualPathStats, ParametricSectorResidualPathError> {
    check_limit("residual-path depth", decisions.len(), limits.max_depth)?;
    check_limit(
        "residual-path retained decisions",
        decisions.len(),
        limits.max_path_decisions,
    )?;
    check_limit(
        "residual-path unsupported candidate references",
        unsupported_candidate_references,
        limits.max_unsupported_candidate_references,
    )?;
    let retained_path_bytes = checked_add(
        "residual-path retained bytes",
        size_of::<ParametricSectorResidualPathCertificate>(),
        checked_mul(
            "residual-path retained bytes",
            decisions.len(),
            size_of::<ParametricSectorResidualPathDecision>(),
        )?,
    )?;
    check_limit(
        "residual-path retained bytes",
        retained_path_bytes,
        limits.max_path_retained_bytes,
    )?;
    let nonzero_decisions = decisions
        .iter()
        .filter(|decision| decision.polarity == ParametricSectorResidualPathPolarity::NonZero)
        .count();
    let equal_zero_decisions = decisions.len() - nonzero_decisions;
    Ok(ParametricSectorResidualPathStats {
        decisions: decisions.len(),
        nonzero_decisions,
        equal_zero_decisions,
        unsupported_candidate_references,
        retained_path_bytes,
    })
}

fn cursor_retained_bytes(capacity: usize) -> Result<usize, ParametricSectorResidualPathError> {
    checked_add(
        "residual-path cursor retained bytes",
        size_of::<ParametricSectorResidualPathCursor>(),
        checked_mul(
            "residual-path cursor retained bytes",
            capacity,
            size_of::<ParametricSectorResidualPathDecision>(),
        )?,
    )
}

fn charge_next(
    resource: &'static str,
    counter: &mut usize,
    limit: usize,
) -> Result<(), ParametricSectorResidualPathError> {
    let requested = checked_add(resource, *counter, 1)?;
    check_limit(resource, requested, limit)?;
    *counter = requested;
    Ok(())
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ParametricSectorResidualPathError> {
    if requested > limit {
        Err(ParametricSectorResidualPathError::ResourceLimit {
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
) -> Result<usize, ParametricSectorResidualPathError> {
    left.checked_add(right)
        .ok_or(ParametricSectorResidualPathError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ParametricSectorResidualPathError> {
    left.checked_mul(right)
        .ok_or(ParametricSectorResidualPathError::ResourceCountOverflow { resource })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adaptive_rules::{AdaptiveParametricRuleProvider, AdaptiveRuleSearchLimits};
    use crate::parametric_sector_mtbdd_certificate::{
        ParametricSectorMtbddCoverageCompiler, ParametricSectorMtbddCoverageLimits,
    };
    use crate::{
        AffineDenominator, CoefficientContext, ConcreteIntegralKey,
        GeneratedSectorDiscoveryCompiler, GeneratedSectorDiscoveryLimits,
        GeneratedSymbolicRowSpanCompiler, GeneratedWhenBadCompilation, GeneratedWhenBadCompiler,
        GeneratedWhenBadLimits, IntegralOrderingPolicy, ParametricElimination,
        ParametricEliminationLimits, ParametricEliminationOrdering, ParametricIbpGenerator,
        ParametricReductionRuleCandidate, ParametricRuleLimits, SectorMask,
    };

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

    fn massive_sunset(name: &str) -> IntegralFamily {
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

    fn sunset_source(
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<ParametricSectorMtbddCoverageCertificate>,
    ) {
        let family = massive_sunset(name);
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
        discovery_limits.adaptive.max_search_depth = 0;
        discovery_limits
            .coverage
            .max_materialized_product_zero_support_terms = 0;
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_from_bit_string("111").unwrap(),
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
            ParametricSectorMtbddCoverageCompiler::compile_authenticated(
                &family,
                &context,
                discovery.sector().clone(),
                compilations,
                ParametricSectorMtbddCoverageLimits::default(),
            )
            .unwrap(),
        );
        (family, context, source)
    }

    fn one_loop_source(
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<ParametricSectorMtbddCoverageCertificate>,
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
            ParametricSectorMtbddCoverageCompiler::compile_authenticated(
                &family,
                &context,
                discovery.sector().clone(),
                compilations,
                ParametricSectorMtbddCoverageLimits::default(),
            )
            .unwrap(),
        );
        (family, context, source)
    }

    fn unsupported_one_loop_source() -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<ParametricSectorMtbddCoverageCertificate>,
    ) {
        let family = massive_tadpole("residual-path-unsupported-one-loop");
        let generated = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .generate()
            .unwrap();
        let context = generated.context().clone();
        let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
        assert_eq!(rows.len(), 1);
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
            ParametricSectorMtbddCoverageCompiler::compile_authenticated(
                &family,
                &context,
                sector,
                vec![compilation],
                ParametricSectorMtbddCoverageLimits::default(),
            )
            .unwrap(),
        );
        (family, context, source)
    }

    #[test]
    fn owning_cursor_skips_descending_and_replays_exact_locus_path() {
        let (family, context, source) = one_loop_source("residual-path-owning-tadpole");
        let mut cursor = ParametricSectorResidualPathCursor::try_new(
            &family,
            &context,
            Arc::clone(&source),
            ParametricSectorResidualPathRequest::AnyResidual,
            ParametricSectorResidualPathLimits::default(),
        )
        .unwrap();
        let path = cursor
            .next_path()
            .unwrap()
            .expect("the active tadpole has one exceptional residual locus");

        assert_eq!(
            cursor.schema(),
            PARAMETRIC_SECTOR_RESIDUAL_PATH_CURSOR_V1_SCHEMA
        );
        assert_eq!(
            cursor.branch_order_schema(),
            PARAMETRIC_SECTOR_RESIDUAL_PATH_BRANCH_ORDER_V1
        );
        assert_eq!(path.schema(), PARAMETRIC_SECTOR_RESIDUAL_PATH_V1_SCHEMA);
        assert_eq!(path.yield_ordinal(), 1);
        assert!(path.same_source_allocation(&source));
        assert_eq!(
            path.terminal_kind(),
            ParametricSectorResidualPathTerminalKind::Uncovered
        );
        assert_eq!(path.decisions().len(), 1);
        assert_eq!(
            path.decisions()[0].polarity(),
            ParametricSectorResidualPathPolarity::EqualZero
        );
        assert_eq!(path.decisions()[0].atom_ordinal(), 0);
        assert_eq!(path.decisions()[0].structural_locus_ordinal(), 0);
        assert!(path.structural_locus(0).is_some());
        assert!(matches!(
            path.terminal_disposition().unwrap(),
            ParametricSectorMtbddDisposition::Uncovered
        ));
        assert_eq!(cursor.stats().descending_terminals_skipped(), 1);
        assert_eq!(cursor.stats().reference_visits(), 3);
        assert_eq!(cursor.stats().node_visits(), 1);
        assert_eq!(cursor.stats().terminal_visits(), 2);
        assert_eq!(cursor.stats().branch_traversals(), 2);
        assert_eq!(cursor.stats().backtracks(), 1);
        assert_eq!(cursor.stats().maximum_depth(), 1);
        assert_eq!(cursor.stats().paths_yielded(), 1);
        assert_eq!(cursor.frontier(), path.decisions());
        assert_eq!(
            cursor.retained_bytes().unwrap(),
            cursor.stats().peak_cursor_retained_bytes()
        );

        path.replay(&family, &context).unwrap();
        let (_, _, foreign) = one_loop_source("residual-path-owning-tadpole-foreign");
        assert!(!path.same_source_allocation(&foreign));
        assert!(format!("{path:?}").contains("<shared authenticated MTBDD coverage>"));

        assert!(cursor.next_path().unwrap().is_none());
        assert!(cursor.is_exhausted());
    }

    #[test]
    fn root_residual_is_an_empty_path_and_kind_filter_is_lazy() {
        let (family, context, nonempty) = one_loop_source("residual-path-empty-owner");
        let source = Arc::new(
            ParametricSectorMtbddCoverageCompiler::compile_authenticated(
                &family,
                &context,
                nonempty.sector().clone(),
                Vec::new(),
                ParametricSectorMtbddCoverageLimits::default(),
            )
            .unwrap(),
        );
        let mut filtered_limits = ParametricSectorResidualPathLimits::default();
        filtered_limits.max_filtered_residual_terminals = 1;
        let mut filtered = ParametricSectorResidualPathCursor::try_new(
            &family,
            &context,
            Arc::clone(&source),
            ParametricSectorResidualPathRequest::Unsupported,
            filtered_limits,
        )
        .unwrap();
        assert!(filtered.next_path().unwrap().is_none());
        assert!(filtered.is_exhausted());
        assert_eq!(filtered.stats().reference_visits(), 1);
        assert_eq!(filtered.stats().terminal_visits(), 1);
        assert_eq!(filtered.stats().filtered_residual_terminals(), 1);
        assert_eq!(filtered.stats().maximum_depth(), 0);
        assert_eq!(filtered.stats().peak_frontier_capacity_entries(), 0);

        let mut one_below = filtered_limits;
        one_below.max_filtered_residual_terminals = 0;
        let mut rejected = ParametricSectorResidualPathCursor::try_new(
            &family,
            &context,
            Arc::clone(&source),
            ParametricSectorResidualPathRequest::Unsupported,
            one_below,
        )
        .unwrap();
        assert!(matches!(
            rejected.next_path(),
            Err(ParametricSectorResidualPathError::ResourceLimit {
                resource: "residual-path filtered residual terminals",
                requested: 1,
                limit: 0,
            })
        ));

        let mut cursor = ParametricSectorResidualPathCursor::try_new(
            &family,
            &context,
            Arc::clone(&source),
            ParametricSectorResidualPathRequest::Uncovered,
            ParametricSectorResidualPathLimits::default(),
        )
        .unwrap();
        let path = cursor.next_path().unwrap().unwrap();
        assert!(path.decisions().is_empty());
        assert_eq!(path.stats().decisions(), 0);
        assert_eq!(path.yield_ordinal(), 1);
        path.replay(&family, &context).unwrap();
    }

    #[test]
    fn second_yield_replays_selection_and_shared_suffix_exactly() {
        let (family, context, source) = sunset_source("residual-path-shared-suffix-sunset");
        assert_eq!(source.normalized().base_structural_loci().len(), 10);
        assert_eq!(source.decision().atoms().len(), 10);
        assert_eq!(source.decision().rooted().nodes().len(), 7);
        assert_eq!(source.decision().rooted().terminal_payloads().len(), 6);
        let mut cursor = ParametricSectorResidualPathCursor::try_new(
            &family,
            &context,
            Arc::clone(&source),
            ParametricSectorResidualPathRequest::AnyResidual,
            ParametricSectorResidualPathLimits::default(),
        )
        .unwrap();
        let first = cursor.next_path().unwrap().unwrap();
        let second = cursor.next_path().unwrap().unwrap();
        let third = cursor.next_path().unwrap().unwrap();
        let fourth = cursor.next_path().unwrap().unwrap();

        assert_eq!(first.yield_ordinal(), 1);
        assert_eq!(second.yield_ordinal(), 2);
        assert_eq!(third.yield_ordinal(), 3);
        assert_eq!(fourth.yield_ordinal(), 4);
        assert_eq!(second.terminal_ordinal(), 5);
        assert_eq!(
            second.terminal_kind(),
            ParametricSectorResidualPathTerminalKind::Uncovered
        );
        second.replay(&family, &context).unwrap();

        // The first and fourth paths diverge above node 2 and then enter the
        // same persisted suffix 2 -> 1 -> 0 -> terminal 5. A global visited
        // set would incorrectly suppress the fourth path.
        assert_ne!(&first.decisions()[..2], &fourth.decisions()[..3]);
        assert_eq!(&first.decisions()[2..], &fourth.decisions()[3..]);
        assert_eq!(first.terminal_ordinal(), fourth.terminal_ordinal());

        let mut wrong_yield = second.clone();
        wrong_yield.yield_ordinal = 1;
        assert_eq!(
            wrong_yield.replay(&family, &context),
            Err(ParametricSectorResidualPathError::PathReplayMismatch)
        );
        let mut wrong_request = second;
        wrong_request.request = ParametricSectorResidualPathRequest::Unsupported;
        assert_eq!(
            wrong_request.replay(&family, &context),
            Err(ParametricSectorResidualPathError::PathReplayMismatch)
        );
    }

    #[test]
    fn real_unsupported_terminal_preserves_ordered_references_and_exact_limit() {
        let (family, context, source) = unsupported_one_loop_source();
        let mut exact = ParametricSectorResidualPathLimits::default();
        exact.max_unsupported_candidate_references = 1;
        let mut cursor = ParametricSectorResidualPathCursor::try_new(
            &family,
            &context,
            Arc::clone(&source),
            ParametricSectorResidualPathRequest::Unsupported,
            exact,
        )
        .unwrap();
        let path = cursor.next_path().unwrap().unwrap();
        assert_eq!(path.yield_ordinal(), 1);
        assert_eq!(
            path.terminal_kind(),
            ParametricSectorResidualPathTerminalKind::Unsupported
        );
        assert!(path.decisions().is_empty());
        assert_eq!(path.stats().unsupported_candidate_references(), 1);
        assert!(matches!(
            path.terminal_disposition().unwrap(),
            ParametricSectorMtbddDisposition::Unsupported { candidate_ordinals }
                if candidate_ordinals.as_ref() == [0]
        ));
        path.replay(&family, &context).unwrap();

        let mut one_below = exact;
        one_below.max_unsupported_candidate_references = 0;
        let mut rejected = ParametricSectorResidualPathCursor::try_new(
            &family,
            &context,
            Arc::clone(&source),
            ParametricSectorResidualPathRequest::Unsupported,
            one_below,
        )
        .unwrap();
        assert!(matches!(
            rejected.next_path(),
            Err(ParametricSectorResidualPathError::ResourceLimit {
                resource: "residual-path unsupported candidate references",
                requested: 1,
                limit: 0,
            })
        ));
        assert!(rejected.is_poisoned());

        let mut recovered = ParametricSectorResidualPathCursor::try_new(
            &family,
            &context,
            Arc::clone(&source),
            ParametricSectorResidualPathRequest::Unsupported,
            exact,
        )
        .unwrap();
        assert!(recovered.next_path().unwrap().is_some());
    }

    fn exact_one_loop_limits(
        cursor: ParametricSectorResidualPathCursorStats,
        path: ParametricSectorResidualPathStats,
    ) -> ParametricSectorResidualPathLimits {
        ParametricSectorResidualPathLimits {
            max_reference_visits: cursor.reference_visits(),
            max_node_visits: cursor.node_visits(),
            max_terminal_visits: cursor.terminal_visits(),
            max_branch_traversals: cursor.branch_traversals(),
            max_backtracks: cursor.backtracks(),
            max_depth: cursor.maximum_depth(),
            max_frontier_capacity_entries: cursor.peak_frontier_capacity_entries(),
            max_cursor_retained_bytes: cursor.peak_cursor_retained_bytes(),
            max_descending_terminals_skipped: cursor.descending_terminals_skipped(),
            max_filtered_residual_terminals: 0,
            max_paths_yielded: cursor.paths_yielded(),
            max_total_path_decisions_copied: cursor.total_path_decisions_copied(),
            max_path_decisions: path.decisions(),
            max_path_retained_bytes: path.retained_path_bytes(),
            max_unsupported_candidate_references: 0,
        }
    }

    fn run_one_loop_path(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source: &Arc<ParametricSectorMtbddCoverageCertificate>,
        limits: ParametricSectorResidualPathLimits,
    ) -> Result<ParametricSectorResidualPathCertificate, ParametricSectorResidualPathError> {
        let mut cursor = ParametricSectorResidualPathCursor::try_new(
            family,
            context,
            Arc::clone(source),
            ParametricSectorResidualPathRequest::AnyResidual,
            limits,
        )?;
        cursor
            .next_path()?
            .ok_or(ParametricSectorResidualPathError::PathReplayMismatch)
    }

    #[test]
    fn exact_and_one_below_limits_are_typed_and_leave_source_reusable() {
        let (family, context, source) = one_loop_source("residual-path-exact-limits");
        let mut baseline = ParametricSectorResidualPathCursor::try_new(
            &family,
            &context,
            Arc::clone(&source),
            ParametricSectorResidualPathRequest::AnyResidual,
            ParametricSectorResidualPathLimits::default(),
        )
        .unwrap();
        let baseline_path = baseline.next_path().unwrap().unwrap();
        let exact = exact_one_loop_limits(baseline.stats(), baseline_path.stats());
        let exact_path = run_one_loop_path(&family, &context, &source, exact).unwrap();
        exact_path.replay(&family, &context).unwrap();
        assert_eq!(exact_path.stats(), baseline_path.stats());

        macro_rules! one_below {
            ($field:ident, $value:expr, $resource:literal) => {{
                let expected = $value;
                assert!(expected > 0);
                let mut limits = exact;
                limits.$field = expected - 1;
                let error = run_one_loop_path(&family, &context, &source, limits).unwrap_err();
                assert!(matches!(
                    error,
                    ParametricSectorResidualPathError::ResourceLimit {
                        resource,
                        requested,
                        limit,
                    } if resource == $resource
                        && requested == expected
                        && limit == expected - 1
                ));
                run_one_loop_path(&family, &context, &source, exact).unwrap();
            }};
        }

        one_below!(
            max_reference_visits,
            baseline.stats().reference_visits(),
            "residual-path reference visits"
        );
        one_below!(
            max_node_visits,
            baseline.stats().node_visits(),
            "residual-path node visits"
        );
        one_below!(
            max_terminal_visits,
            baseline.stats().terminal_visits(),
            "residual-path terminal visits"
        );
        one_below!(
            max_branch_traversals,
            baseline.stats().branch_traversals(),
            "residual-path branch traversals"
        );
        one_below!(
            max_backtracks,
            baseline.stats().backtracks(),
            "residual-path backtracks"
        );
        one_below!(
            max_depth,
            baseline.stats().maximum_depth(),
            "residual-path depth"
        );
        one_below!(
            max_frontier_capacity_entries,
            baseline.stats().peak_frontier_capacity_entries(),
            "residual-path frontier capacity entries"
        );
        one_below!(
            max_cursor_retained_bytes,
            baseline.stats().peak_cursor_retained_bytes(),
            "residual-path cursor retained bytes"
        );
        one_below!(
            max_descending_terminals_skipped,
            baseline.stats().descending_terminals_skipped(),
            "residual-path descending terminals skipped"
        );
        one_below!(
            max_paths_yielded,
            baseline.stats().paths_yielded(),
            "residual paths yielded"
        );
        one_below!(
            max_total_path_decisions_copied,
            baseline.stats().total_path_decisions_copied(),
            "residual-path total decisions copied"
        );
        one_below!(
            max_path_decisions,
            baseline_path.stats().decisions(),
            "residual-path retained decisions"
        );
        one_below!(
            max_path_retained_bytes,
            baseline_path.stats().retained_path_bytes(),
            "residual-path retained bytes"
        );

        let mut poisoned_limits = exact;
        poisoned_limits.max_depth = 0;
        let mut poisoned = ParametricSectorResidualPathCursor::try_new(
            &family,
            &context,
            Arc::clone(&source),
            ParametricSectorResidualPathRequest::AnyResidual,
            poisoned_limits,
        )
        .unwrap();
        assert!(matches!(
            poisoned.next_path(),
            Err(ParametricSectorResidualPathError::ResourceLimit {
                resource: "residual-path depth",
                requested: 1,
                limit: 0,
            })
        ));
        assert!(poisoned.is_poisoned());
        assert!(poisoned.same_source_allocation(&source));
        assert!(matches!(
            poisoned.next_path(),
            Err(ParametricSectorResidualPathError::CursorPoisoned)
        ));
        run_one_loop_path(&family, &context, &source, exact).unwrap();
    }

    fn six_loop_unit_mass_coordinate_basis(name: &str) -> IntegralFamily {
        const LOOPS: usize = 6;
        const ARITY: usize = LOOPS * (LOOPS + 1) / 2;
        let coefficients = CoefficientContext::new(["d"]);
        let zero = coefficients.zero();
        let one = coefficients.one();
        let denominators = (0..ARITY)
            .map(|row| {
                AffineDenominator::new(
                    coefficients.integer(-1),
                    (0..ARITY)
                        .map(|column| {
                            if row == column {
                                one.clone()
                            } else {
                                zero.clone()
                            }
                        })
                        .collect(),
                )
            })
            .collect();
        IntegralFamily::new(
            name,
            (0..LOOPS).map(|loop_| format!("k{}", loop_ + 1)).collect(),
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            denominators,
            Vec::new(),
            vec![zero; ARITY],
        )
        .unwrap()
    }

    /// This fixture enters through generation, adaptive candidate derivation,
    /// GeneratedWhenBad authentication, normalization, and the existing MTBDD
    /// compiler. It deliberately never invokes legacy V4 coverage.
    fn six_loop_k21_source() -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<ParametricSectorMtbddCoverageCertificate>,
    ) {
        const ARITY: usize = 21;
        let family = six_loop_unit_mass_coordinate_basis("residual-path-six-loop-k21");
        let generator = ParametricIbpGenerator::try_new(&family).unwrap();
        let context = generator.context().clone();
        let coverage_limits = ParametricSectorMtbddCoverageLimits::default();
        let row_span = Arc::new(
            GeneratedSymbolicRowSpanCompiler::compile(
                &family,
                &context,
                coverage_limits.coverage.generated_when_bad.ibp,
                coverage_limits.coverage.generated_when_bad.row_span,
            )
            .unwrap(),
        );
        row_span.replay(&family, &context).unwrap();
        assert_eq!(row_span.rows().len(), 36);
        let mut adaptive_limits = AdaptiveRuleSearchLimits::default();
        adaptive_limits.max_search_depth = 0;
        let mut adaptive = AdaptiveParametricRuleProvider::try_new(
            &context,
            row_span.rows(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            adaptive_limits,
        )
        .unwrap();
        let corner = ConcreteIntegralKey::try_new(vec![0; ARITY]).unwrap();
        let candidates = adaptive.candidates_for_quotient(&corner).unwrap();
        assert_eq!(candidates.len(), 36);
        let compilations = candidates
            .iter()
            .map(|candidate| {
                GeneratedWhenBadCompiler::compile_with_replayed_row_span(
                    &family,
                    &context,
                    candidate,
                    Arc::clone(&row_span),
                    coverage_limits.coverage.generated_when_bad,
                )
                .unwrap()
            })
            .collect();
        let source = Arc::new(
            ParametricSectorMtbddCoverageCompiler::compile_authenticated(
                &family,
                &context,
                SectorMask::try_new([false; ARITY]).unwrap(),
                compilations,
                coverage_limits,
            )
            .unwrap(),
        );
        (family, context, source)
    }

    #[test]
    #[ignore = "full all-36 L=6/K=21 MTBDD construction is an explicit scaling stress"]
    fn full_six_loop_k21_source_finds_first_residual_with_bounded_cursor_memory() {
        const PATH_DEPTH: usize = 43;
        const EXPLICIT_ATOM_ASSIGNMENTS: usize = 1 << 49;
        let (family, context, source) = six_loop_k21_source();

        // This census is intentionally the real all-36 generated source, not
        // a manufactured 21-rule chain. It is expensive enough to stay out of
        // the routine contract suite, but records why lazy traversal alone is
        // not a complete high-loop construction strategy.
        assert_eq!(source.normalized().base_structural_loci().len(), 49);
        assert_eq!(source.decision().atoms().len(), 49);
        assert_eq!(source.decision().rooted().nodes().len(), 268_427);
        assert_eq!(source.decision().rooted().terminal_payloads().len(), 18);
        let certified_candidates = [1, 2, 3, 4, 5, 8, 9, 10, 11, 15, 16, 17, 22, 23, 29];
        assert_eq!(
            (2..=16)
                .map(
                    |terminal| match terminal_disposition(source.as_ref(), terminal).unwrap() {
                        ParametricSectorMtbddDisposition::DescendingRule { candidate_ordinal } => {
                            *candidate_ordinal
                        }
                        _ => panic!("terminal {terminal} must select a descending rule"),
                    }
                )
                .collect::<Vec<_>>(),
            certified_candidates
        );
        let unsupported_candidates = [
            0, 6, 7, 12, 13, 14, 18, 19, 20, 21, 24, 25, 26, 27, 28, 30, 31, 32, 33, 34, 35,
        ];
        assert!(matches!(
            terminal_disposition(source.as_ref(), 17).unwrap(),
            ParametricSectorMtbddDisposition::Unsupported { candidate_ordinals }
                if candidate_ordinals.as_ref() == unsupported_candidates
        ));

        let mut cursor = ParametricSectorResidualPathCursor::try_new(
            &family,
            &context,
            Arc::clone(&source),
            ParametricSectorResidualPathRequest::AnyResidual,
            ParametricSectorResidualPathLimits::default(),
        )
        .unwrap();
        let path = cursor
            .next_path()
            .unwrap()
            .expect("the all-boundary conjunction remains residual");

        assert_eq!(
            cursor.request(),
            ParametricSectorResidualPathRequest::AnyResidual
        );
        assert_eq!(
            path.request(),
            ParametricSectorResidualPathRequest::AnyResidual
        );
        assert_eq!(path.yield_ordinal(), 1);
        assert_eq!(
            path.terminal_kind(),
            ParametricSectorResidualPathTerminalKind::Unsupported
        );
        assert_eq!(path.terminal_ordinal(), 17);
        assert_eq!(path.decisions().len(), PATH_DEPTH);
        let expected_loci = (0..=27).chain(31..=45).collect::<Vec<_>>();
        assert_eq!(
            path.decisions()
                .iter()
                .map(|decision| decision.atom_ordinal())
                .collect::<Vec<_>>(),
            expected_loci
        );
        assert_eq!(
            path.decisions()
                .iter()
                .map(|decision| decision.structural_locus_ordinal())
                .collect::<Vec<_>>(),
            expected_loci
        );
        let equal_zero_loci = path
            .decisions()
            .iter()
            .filter(|decision| {
                decision.polarity() == ParametricSectorResidualPathPolarity::EqualZero
            })
            .map(|decision| decision.structural_locus_ordinal())
            .collect::<Vec<_>>();
        assert_eq!(equal_zero_loci, [12, 36, 41, 45]);

        let stats = cursor.stats();
        assert_eq!(stats.source_references(), 1);
        assert_eq!(stats.reference_visits(), 48);
        assert_eq!(stats.node_visits(), PATH_DEPTH);
        assert_eq!(stats.terminal_visits(), 5);
        assert_eq!(stats.branch_traversals(), 47);
        assert_eq!(stats.backtracks(), 4);
        assert_eq!(stats.maximum_depth(), PATH_DEPTH);
        let frontier_capacity = stats.peak_frontier_capacity_entries();
        assert!(frontier_capacity >= PATH_DEPTH);
        assert_eq!(
            stats.peak_cursor_retained_bytes(),
            cursor_retained_bytes(frontier_capacity).unwrap()
        );
        assert_eq!(stats.descending_terminals_skipped(), 4);
        assert_eq!(stats.uncovered_terminals_visited(), 0);
        assert_eq!(stats.unsupported_terminals_visited(), 1);
        assert_eq!(stats.filtered_residual_terminals(), 0);
        assert_eq!(stats.paths_yielded(), 1);
        assert_eq!(stats.total_path_decisions_copied(), PATH_DEPTH);
        assert_eq!(stats.structural_locus_translations(), PATH_DEPTH);
        assert_eq!(path.stats().decisions(), PATH_DEPTH);
        assert_eq!(path.stats().nonzero_decisions(), 39);
        assert_eq!(path.stats().equal_zero_decisions(), 4);
        assert_eq!(path.stats().unsupported_candidate_references(), 21);
        assert_eq!(
            path.stats().retained_path_bytes(),
            size_of::<ParametricSectorResidualPathCertificate>()
                + PATH_DEPTH * size_of::<ParametricSectorResidualPathDecision>()
        );
        assert!(cursor.stats().reference_visits() < EXPLICIT_ATOM_ASSIGNMENTS);
        assert!(cursor.stats().peak_frontier_capacity_entries() < EXPLICIT_ATOM_ASSIGNMENTS);

        let exact = ParametricSectorResidualPathLimits {
            max_reference_visits: cursor.stats().reference_visits(),
            max_node_visits: cursor.stats().node_visits(),
            max_terminal_visits: cursor.stats().terminal_visits(),
            max_branch_traversals: cursor.stats().branch_traversals(),
            max_backtracks: cursor.stats().backtracks(),
            max_depth: PATH_DEPTH,
            max_frontier_capacity_entries: cursor.stats().peak_frontier_capacity_entries(),
            max_cursor_retained_bytes: cursor.stats().peak_cursor_retained_bytes(),
            max_descending_terminals_skipped: cursor.stats().descending_terminals_skipped(),
            max_filtered_residual_terminals: 0,
            max_paths_yielded: 1,
            max_total_path_decisions_copied: PATH_DEPTH,
            max_path_decisions: PATH_DEPTH,
            max_path_retained_bytes: path.stats().retained_path_bytes(),
            max_unsupported_candidate_references: 21,
        };
        let mut exact_cursor = ParametricSectorResidualPathCursor::from_replayed_source(
            Arc::clone(&source),
            ParametricSectorResidualPathRequest::AnyResidual,
            exact,
        )
        .unwrap();
        let exact_path = exact_cursor.next_path().unwrap().unwrap();
        assert_eq!(exact_path.stats(), path.stats());
        assert_eq!(exact_path.decisions(), path.decisions());
        let mut one_below = exact;
        one_below.max_depth = PATH_DEPTH - 1;
        let mut rejected = ParametricSectorResidualPathCursor::from_replayed_source(
            Arc::clone(&source),
            ParametricSectorResidualPathRequest::AnyResidual,
            one_below,
        )
        .unwrap();
        let error = rejected.next_path().unwrap_err();
        assert_eq!(
            error,
            ParametricSectorResidualPathError::ResourceLimit {
                resource: "residual-path depth",
                requested: PATH_DEPTH,
                limit: PATH_DEPTH - 1,
            }
        );
        // The failed cursor retained only its bounded in-process Arc; a fresh
        // exact traversal over the same authenticated owner remains valid.
        let mut recovered = ParametricSectorResidualPathCursor::from_replayed_source(
            Arc::clone(&source),
            ParametricSectorResidualPathRequest::AnyResidual,
            exact,
        )
        .unwrap();
        assert!(recovered.next_path().unwrap().is_some());

        // This is bounded-depth evidence for finding the first residual path;
        // neither exhaustive path enumeration nor global MTBDD construction
        // is claimed to be linear.
    }
}
