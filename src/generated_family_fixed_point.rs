//! Replayable residual-anchor fixed point for generated family rules.
//!
//! This is the generic RustRed analogue of the residual-case loop in
//! LiteRed's `SolvejSector`.  A concrete search anchor is derived only from an
//! authenticated nonempty coordinate-equality leaf.  It is useful for
//! selecting a parametric candidate, but it never proves that the symbolic
//! parent leaf is closed.  Closure is decided only after composing the full
//! generated `WhenBad` domains and rebuilding the live-leaf queue.
//!
//! No topology name, loop count, recurrence, master count, FORM program, or
//! Mathematica expression is an input to this module.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use crate::generated_family_rule_system::{
    adaptive_error_is_resource, discovery_error_is_resource, generated_when_bad_error_is_resource,
    queue_error_is_resource, when_bad_error_is_resource,
};
use crate::{
    AdaptiveParametricRuleProvider, AdaptiveRuleSearchError, ConcreteIntegralKey,
    CoordinateEqualityLeafStatus, GeneratedFamilyRuleSystemCertificate,
    GeneratedFamilyRuleSystemError, GeneratedFamilySectorStatus,
    GeneratedSectorDiscoveryCertificate, GeneratedSectorDiscoveryCompiler,
    GeneratedSectorDiscoveryError, GeneratedSectorLiveLeafQueueCertificate,
    GeneratedSectorLiveLeafQueueCompiler, GeneratedSectorLiveLeafQueueError,
    GeneratedSectorQueuedSourceDisposition, GeneratedWhenBadCompilation, GeneratedWhenBadCompiler,
    GeneratedWhenBadError, IntegralFamily, ParametricCoefficientContext,
    ParametricSectorCoverageError, ParametricSectorLeafDisposition, SectorFoundationError,
    SectorMask, SymbolicSectorCaseId, WhenBadCompilerError, WhenBadLeafDisposition,
};

pub const GENERATED_FAMILY_FIXED_POINT_V1_SCHEMA: &str = "rustred.generated-family-fixed-point.v1";
pub const GENERATED_FAMILY_FIXED_POINT_PROVIDER_V1_SCHEMA: &str =
    "rustred.generated-family-fixed-point-provider.v1";

/// Generic sector scheduling.  A bounded prefix is a resource policy, never
/// a topology selector or a claim about sectors omitted by the bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratedFamilyFixedPointSelectionPolicy {
    AllResidualSubsectorFirst,
    /// Select a fixed proper-subsector-first prefix for the complete
    /// fixed-point schedule.  The same selected sectors may be revisited in
    /// later rounds; this is a total selection bound, not a rolling quota.
    ResidualSubsectorFirstPrefix {
        max_selected_sectors: usize,
    },
}

impl Default for GeneratedFamilyFixedPointSelectionPolicy {
    fn default() -> Self {
        Self::AllResidualSubsectorFirst
    }
}

/// Search depth and fixed-point policies.  Rounds and local stencil depth are
/// deliberately separate resource dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedFamilyFixedPointConfig {
    pub base_search_depth: usize,
    pub maximum_rounds: usize,
    /// Exact in-sector shells around the sector corner inspected when a
    /// residual leaf has no coordinate equality, or around the corner
    /// completion in the unassigned coordinates of a partial equality.
    /// Depth zero is retained because a residual round may need a deeper
    /// adaptive stencil at the same LiteRed-style `startp`.
    pub residual_frontier_depth: usize,
    pub residual_anchor_local_depth: usize,
    pub maximum_local_depth: usize,
    pub selection: GeneratedFamilyFixedPointSelectionPolicy,
    /// Optional heuristic stop.  `false` is the LiteRed-faithful default:
    /// unchanged residual work may be revisited at a deeper local stencil.
    pub stop_on_no_strict_improvement: bool,
}

impl Default for GeneratedFamilyFixedPointConfig {
    fn default() -> Self {
        Self {
            base_search_depth: 1,
            maximum_rounds: 2,
            residual_frontier_depth: 1,
            residual_anchor_local_depth: 1,
            maximum_local_depth: 2,
            selection: GeneratedFamilyFixedPointSelectionPolicy::AllResidualSubsectorFirst,
            stop_on_no_strict_improvement: false,
        }
    }
}

/// Aggregate transcript limits in addition to the nested limits owned by the
/// base family certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedFamilyFixedPointLimits {
    pub max_base_preparations: usize,
    pub max_rounds: usize,
    pub max_sector_attempts: usize,
    pub max_final_sector_statuses: usize,
    pub max_retained_anchor_searches: usize,
    pub max_retained_anchor_origins: usize,
    pub max_frontier_offsets: usize,
    /// Heap-iterator transitions, including partial assignments that do not
    /// emit a point. This bounds shell work independently of output size.
    pub max_frontier_enumeration_steps: usize,
    pub max_frontier_points: usize,
    pub max_frontier_components: usize,
    pub max_retained_assignment_entries: usize,
    pub max_visited_candidates: usize,
    pub max_accepted_candidate_references: usize,
    pub max_retained_visited_source_rows: usize,
    pub max_retained_visited_source_terms: usize,
    pub max_retained_visited_source_manifest_bytes: usize,
    pub max_retained_visited_candidate_binding_bytes: usize,
    pub max_retained_visited_condition_terms: usize,
    pub max_retained_visited_condition_bytes: usize,
    pub max_locator_references: usize,
    pub max_retained_material_locators: usize,
    pub max_retained_residual_leaves: usize,
    pub max_retained_residual_predicates: usize,
}

impl Default for GeneratedFamilyFixedPointLimits {
    fn default() -> Self {
        Self {
            max_base_preparations: 1_000_000,
            max_rounds: 1_000_000,
            max_sector_attempts: 16_000_000,
            max_final_sector_statuses: 1_000_000,
            max_retained_anchor_searches: 16_000_000,
            max_retained_anchor_origins: 32_000_000,
            max_frontier_offsets: 16_000_000,
            max_frontier_enumeration_steps: 100_000_000,
            max_frontier_points: 16_000_000,
            max_frontier_components: 256_000_000,
            max_retained_assignment_entries: 256_000_000,
            max_visited_candidates: 100_000_000,
            max_accepted_candidate_references: 16_000_000,
            max_retained_visited_source_rows: 1_000_000_000,
            max_retained_visited_source_terms: 10_000_000_000,
            max_retained_visited_source_manifest_bytes: 8 * 1024 * 1024 * 1024,
            max_retained_visited_candidate_binding_bytes: 8 * 1024 * 1024 * 1024,
            max_retained_visited_condition_terms: 10_000_000_000,
            max_retained_visited_condition_bytes: 8 * 1024 * 1024 * 1024,
            max_locator_references: 100_000_000,
            max_retained_material_locators: 100_000_000,
            max_retained_residual_leaves: 32_000_000,
            max_retained_residual_predicates: 256_000_000,
        }
    }
}

/// Stable reference to one retained discovery/live-queue pair.
///
/// The base family certificate owns `BaseRuleSystem` material.  A successful
/// phase-zero preparation or residual round owns the other two variants.
/// Every locator is resolved and payload-checked during replay; it is never a
/// caller-provided rule handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GeneratedFixedPointMaterialLocator {
    BaseRuleSystem {
        solve_ordinal: usize,
    },
    BasePreparation {
        preparation_ordinal: usize,
    },
    ResidualRound {
        round_ordinal: usize,
        sector_attempt_ordinal: usize,
    },
}

/// Exact source of one residual-anchor request.
///
/// The locator and work-item ordinal are sufficient: the referenced queue
/// owns the source disposition, coordinate extraction, assignment, complete
/// parent partition, and source case.  Search construction derives all of
/// those values again and rejects a locator that does not reproduce the
/// enclosing request anchor.  This avoids cloning large proof payloads into
/// every fan-in origin.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GeneratedResidualAnchorOrigin {
    CoordinateAssignment {
        material: GeneratedFixedPointMaterialLocator,
        work_item_ordinal: usize,
    },
    /// A bounded completion of coordinates that remain free after applying
    /// the queue leaf's authenticated equality assignment. Assigned
    /// coordinates stay fixed; the locator addresses an exact L1 shell in
    /// the remaining coordinate subspace.
    CoordinateCompletionFrontier {
        material: GeneratedFixedPointMaterialLocator,
        work_item_ordinal: usize,
        frontier_depth: usize,
        within_frontier_ordinal: usize,
    },
    ResidualFrontier {
        material: GeneratedFixedPointMaterialLocator,
        work_item_ordinal: usize,
        frontier_depth: usize,
        within_frontier_ordinal: usize,
    },
}

impl GeneratedResidualAnchorOrigin {
    pub const fn material(&self) -> GeneratedFixedPointMaterialLocator {
        match self {
            Self::CoordinateAssignment { material, .. }
            | Self::CoordinateCompletionFrontier { material, .. }
            | Self::ResidualFrontier { material, .. } => *material,
        }
    }
    pub const fn work_item_ordinal(&self) -> usize {
        match self {
            Self::CoordinateAssignment {
                work_item_ordinal, ..
            }
            | Self::CoordinateCompletionFrontier {
                work_item_ordinal, ..
            }
            | Self::ResidualFrontier {
                work_item_ordinal, ..
            } => *work_item_ordinal,
        }
    }
    pub const fn frontier_locator(&self) -> Option<(usize, usize)> {
        match self {
            Self::CoordinateCompletionFrontier {
                frontier_depth,
                within_frontier_ordinal,
                ..
            }
            | Self::ResidualFrontier {
                frontier_depth,
                within_frontier_ordinal,
                ..
            } => Some((*frontier_depth, *within_frontier_ordinal)),
            Self::CoordinateAssignment { .. } => None,
        }
    }
}

/// Stable locator in one deterministic adaptive candidate search.  Candidate
/// ordinals are local to the requested anchor and depth layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GeneratedResidualCandidateLocator {
    local_depth: usize,
    within_layer_ordinal: usize,
}

impl GeneratedResidualCandidateLocator {
    pub const fn local_depth(self) -> usize {
        self.local_depth
    }
    pub const fn within_layer_ordinal(self) -> usize {
        self.within_layer_ordinal
    }
}

/// Exact outcome of authenticating one visited locator and evaluating its
/// `WhenBad` partition at the queue-derived request anchor.
#[derive(Clone, Debug)]
pub enum GeneratedResidualCandidateOutcome {
    Unsupported {
        compilation: GeneratedWhenBadCompilation,
    },
    CertifiedNotCoveringRequestAnchor {
        compilation: GeneratedWhenBadCompilation,
    },
    CertifiedCoveredRequestAnchor {
        compilation: GeneratedWhenBadCompilation,
    },
}

impl GeneratedResidualCandidateOutcome {
    pub const fn compilation(&self) -> &GeneratedWhenBadCompilation {
        match self {
            Self::Unsupported { compilation }
            | Self::CertifiedNotCoveringRequestAnchor { compilation }
            | Self::CertifiedCoveredRequestAnchor { compilation } => compilation,
        }
    }

    pub const fn covers_request_anchor(&self) -> bool {
        matches!(self, Self::CertifiedCoveredRequestAnchor { .. })
    }
}

#[derive(Clone, Debug)]
pub struct GeneratedResidualCandidateVisit {
    locator: GeneratedResidualCandidateLocator,
    outcome: GeneratedResidualCandidateOutcome,
}

impl GeneratedResidualCandidateVisit {
    pub const fn locator(&self) -> GeneratedResidualCandidateLocator {
        self.locator
    }
    pub const fn outcome(&self) -> &GeneratedResidualCandidateOutcome {
        &self.outcome
    }
}

/// One deterministic request.  The visited list is a prefix through the first
/// covering candidate, or the complete requested layers when none covers.
#[derive(Clone, Debug)]
pub struct GeneratedResidualAnchorSearch {
    request_anchor: ConcreteIntegralKey,
    requested_local_depth: usize,
    origins: Box<[GeneratedResidualAnchorOrigin]>,
    visited: Box<[GeneratedResidualCandidateVisit]>,
    selected_visit_ordinal: Option<usize>,
}

impl GeneratedResidualAnchorSearch {
    pub const fn request_anchor(&self) -> &ConcreteIntegralKey {
        &self.request_anchor
    }
    pub const fn requested_local_depth(&self) -> usize {
        self.requested_local_depth
    }
    pub fn origins(&self) -> &[GeneratedResidualAnchorOrigin] {
        &self.origins
    }
    pub fn visited(&self) -> &[GeneratedResidualCandidateVisit] {
        &self.visited
    }
    pub const fn selected_visit_ordinal(&self) -> Option<usize> {
        self.selected_visit_ordinal
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedAcceptedCandidateOrigin {
    /// Candidate already retained by the exact input base-sector material.
    /// These always remain first so phase zero is coverage-monotone even when
    /// its fresh search depth is shallower than the base compiler's depth.
    BaseInputCandidateOrdinal {
        solve_ordinal: usize,
        source_candidate_ordinal: usize,
    },
    /// Genuinely new useful candidate selected from the phase-zero fresh
    /// search after excluding payloads already present in the base material.
    BaseDescendingOrdinal {
        preparation_ordinal: usize,
        source_candidate_ordinal: usize,
    },
    ResidualSelection {
        round_ordinal: usize,
        sector_attempt_ordinal: usize,
        anchor_search_ordinal: usize,
        visit_ordinal: usize,
    },
}

/// Small provenance reference for one candidate in the exact V5 composition
/// order.  The full compilation is owned once, by its base discovery or
/// residual visit, and is payload-compared with the corresponding V5 coverage
/// attempt when the material is built and replayed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedAcceptedCandidateReference {
    composed_candidate_ordinal: usize,
    origin: GeneratedAcceptedCandidateOrigin,
}

impl GeneratedAcceptedCandidateReference {
    pub const fn composed_candidate_ordinal(&self) -> usize {
        self.composed_candidate_ordinal
    }
    pub const fn origin(&self) -> &GeneratedAcceptedCandidateOrigin {
        &self.origin
    }
}

/// One exact ordered residual-cell reference in a retained material.
///
/// `source_case` identifies the complete predicate conjunction in the owning
/// discovery partition.  `work_item_ordinal` binds the same cell to the live
/// queue and `source_disposition` preserves `Uncovered` versus the exact
/// unsupported-candidate ordinal set.  The predicates themselves therefore
/// remain owned once by the material instead of being cloned into every
/// before/after summary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedFixedPointResidualLeafReference {
    work_item_ordinal: usize,
    source_case: SymbolicSectorCaseId,
    source_disposition: GeneratedSectorQueuedSourceDisposition,
}

impl GeneratedFixedPointResidualLeafReference {
    pub const fn work_item_ordinal(&self) -> usize {
        self.work_item_ordinal
    }
    pub const fn source_case(&self) -> SymbolicSectorCaseId {
        self.source_case
    }
    pub const fn source_disposition(&self) -> &GeneratedSectorQueuedSourceDisposition {
        &self.source_disposition
    }
}

/// Exact ordered residual identity relative to its enclosing material
/// locator.  Replay resolves every case/work-item reference against that
/// material and recounts the predicate instances; equal counts alone never
/// establish identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedFixedPointResidualSummary {
    leaves: Box<[GeneratedFixedPointResidualLeafReference]>,
    predicate_instances: usize,
}

impl GeneratedFixedPointResidualSummary {
    pub fn leaves(&self) -> &[GeneratedFixedPointResidualLeafReference] {
        &self.leaves
    }
    pub fn root_leaves(&self) -> usize {
        self.leaves.len()
    }
    pub const fn predicate_instances(&self) -> usize {
        self.predicate_instances
    }
    pub fn queue_work_items(&self) -> usize {
        self.leaves.len()
    }
    pub fn is_empty(&self) -> bool {
        self.leaves.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratedFamilyFixedPointStage {
    BaseDiscovery,
    BaseLiveLeafQueue,
    ResidualSearch,
    GlobalComposition,
    LiveLeafQueue,
}

/// Exact typed interruption at a replayable scheduler location.  Residual
/// variants refer into the enclosing attempt's `anchor_searches`; the
/// candidate locator identifies the first candidate that could not complete
/// authentication or point evaluation after the retained visited prefix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedFamilyFixedPointInterruption {
    BaseDiscovery {
        error: GeneratedSectorDiscoveryError,
    },
    BaseLiveLeafQueue {
        error: GeneratedSectorLiveLeafQueueError,
    },
    ResidualAdaptiveSearch {
        anchor_search_ordinal: usize,
        error: AdaptiveRuleSearchError,
    },
    ResidualCandidateAuthentication {
        anchor_search_ordinal: usize,
        locator: GeneratedResidualCandidateLocator,
        error: GeneratedWhenBadError,
    },
    ResidualPointEvaluation {
        anchor_search_ordinal: usize,
        locator: GeneratedResidualCandidateLocator,
        error: WhenBadCompilerError,
    },
    GlobalComposition {
        error: GeneratedSectorDiscoveryError,
    },
    LiveLeafQueue {
        error: GeneratedSectorLiveLeafQueueError,
    },
}

impl GeneratedFamilyFixedPointInterruption {
    pub const fn stage(&self) -> GeneratedFamilyFixedPointStage {
        match self {
            Self::BaseDiscovery { .. } => GeneratedFamilyFixedPointStage::BaseDiscovery,
            Self::BaseLiveLeafQueue { .. } => GeneratedFamilyFixedPointStage::BaseLiveLeafQueue,
            Self::ResidualAdaptiveSearch { .. }
            | Self::ResidualCandidateAuthentication { .. }
            | Self::ResidualPointEvaluation { .. } => {
                GeneratedFamilyFixedPointStage::ResidualSearch
            }
            Self::GlobalComposition { .. } => GeneratedFamilyFixedPointStage::GlobalComposition,
            Self::LiveLeafQueue { .. } => GeneratedFamilyFixedPointStage::LiveLeafQueue,
        }
    }

    pub const fn anchor_search_ordinal(&self) -> Option<usize> {
        match self {
            Self::ResidualAdaptiveSearch {
                anchor_search_ordinal,
                ..
            }
            | Self::ResidualCandidateAuthentication {
                anchor_search_ordinal,
                ..
            }
            | Self::ResidualPointEvaluation {
                anchor_search_ordinal,
                ..
            } => Some(*anchor_search_ordinal),
            _ => None,
        }
    }

    pub const fn candidate_locator(&self) -> Option<GeneratedResidualCandidateLocator> {
        match self {
            Self::ResidualCandidateAuthentication { locator, .. }
            | Self::ResidualPointEvaluation { locator, .. } => Some(*locator),
            _ => None,
        }
    }
}

/// Phase-zero replacement for the old depth-growth corner pass.  Only sectors
/// selected by the generic policy receive one preparation.  Other sectors
/// continue to reference their original base-rule-system material.
#[derive(Clone, Debug)]
pub enum GeneratedFamilyFixedPointBasePreparationOutcome {
    Prepared {
        /// Search-backed corner stencil from which the exact source candidate
        /// ordinals below are selected.  The separately retained
        /// composition-only `discovery` is the material installed for later
        /// residual work; keeping both prevents a V5 composed ordinal from
        /// being mistaken for its phase-zero search locator.
        search_discovery: GeneratedSectorDiscoveryCertificate,
        after: GeneratedFixedPointResidualSummary,
        discovery: GeneratedSectorDiscoveryCertificate,
        live_leaf_queue: GeneratedSectorLiveLeafQueueCertificate,
        accepted_candidates: Box<[GeneratedAcceptedCandidateReference]>,
    },
    ResourceLimited {
        interruption: GeneratedFamilyFixedPointInterruption,
        completed_discovery: Option<GeneratedSectorDiscoveryCertificate>,
    },
    Failed {
        interruption: GeneratedFamilyFixedPointInterruption,
        completed_discovery: Option<GeneratedSectorDiscoveryCertificate>,
    },
}

impl GeneratedFamilyFixedPointBasePreparationOutcome {
    pub const fn search_discovery(&self) -> Option<&GeneratedSectorDiscoveryCertificate> {
        match self {
            Self::Prepared {
                search_discovery, ..
            } => Some(search_discovery),
            Self::ResourceLimited { .. } | Self::Failed { .. } => None,
        }
    }

    pub fn accepted_candidates(&self) -> &[GeneratedAcceptedCandidateReference] {
        match self {
            Self::Prepared {
                accepted_candidates,
                ..
            } => accepted_candidates,
            Self::ResourceLimited { .. } | Self::Failed { .. } => &[],
        }
    }

    pub fn material(
        &self,
    ) -> Option<(
        &GeneratedSectorDiscoveryCertificate,
        &GeneratedSectorLiveLeafQueueCertificate,
    )> {
        match self {
            Self::Prepared {
                discovery,
                live_leaf_queue,
                ..
            } => Some((discovery, live_leaf_queue)),
            Self::ResourceLimited { .. } | Self::Failed { .. } => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GeneratedFamilyFixedPointBasePreparation {
    ordinal: usize,
    sector: SectorMask,
    solve_ordinal: usize,
    input_material: GeneratedFixedPointMaterialLocator,
    before: GeneratedFixedPointResidualSummary,
    outcome: GeneratedFamilyFixedPointBasePreparationOutcome,
}

impl GeneratedFamilyFixedPointBasePreparation {
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }
    pub const fn sector(&self) -> &SectorMask {
        &self.sector
    }
    pub const fn solve_ordinal(&self) -> usize {
        self.solve_ordinal
    }
    pub const fn input_material(&self) -> GeneratedFixedPointMaterialLocator {
        self.input_material
    }
    pub const fn before(&self) -> &GeneratedFixedPointResidualSummary {
        &self.before
    }
    pub const fn outcome(&self) -> &GeneratedFamilyFixedPointBasePreparationOutcome {
        &self.outcome
    }
}

#[derive(Clone, Debug)]
pub enum GeneratedFamilyFixedPointAttemptOutcome {
    Completed {
        after: GeneratedFixedPointResidualSummary,
        strict_improvement: bool,
        discovery: GeneratedSectorDiscoveryCertificate,
        live_leaf_queue: GeneratedSectorLiveLeafQueueCertificate,
    },
    /// Eligible authenticated coordinate anchors were searched completely,
    /// but no candidate covered any request point.  The input material stays
    /// current and the same anchors may reappear at a deeper local depth.
    NoCandidateCoveredRequestAnchors {
        after: GeneratedFixedPointResidualSummary,
    },
    /// The bounded request scheduler found no new concrete witness in the
    /// configured coordinate completion/frontier.  This is not a proof that
    /// the residual integer locus has no point, and never declares a master.
    AnchorWitnessSearchExhaustedWithinConfiguredBounds {
        after: GeneratedFixedPointResidualSummary,
    },
    ResourceLimited {
        interruption: GeneratedFamilyFixedPointInterruption,
        completed_discovery: Option<GeneratedSectorDiscoveryCertificate>,
    },
    Failed {
        interruption: GeneratedFamilyFixedPointInterruption,
        completed_discovery: Option<GeneratedSectorDiscoveryCertificate>,
    },
}

impl GeneratedFamilyFixedPointAttemptOutcome {
    pub fn material(
        &self,
    ) -> Option<(
        &GeneratedSectorDiscoveryCertificate,
        &GeneratedSectorLiveLeafQueueCertificate,
    )> {
        match self {
            Self::Completed {
                discovery,
                live_leaf_queue,
                ..
            } => Some((discovery, live_leaf_queue)),
            Self::NoCandidateCoveredRequestAnchors { .. }
            | Self::AnchorWitnessSearchExhaustedWithinConfiguredBounds { .. }
            | Self::ResourceLimited { .. }
            | Self::Failed { .. } => None,
        }
    }
}

/// Exact reason a residual sector stopped without an algebraic closure proof.
/// Every variant is a bounded scheduler outcome, never irreducibility or a
/// master-integral certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratedAnchorWitnessSearchExhaustionReason {
    /// No as-yet-unsearched point in the configured coordinate completion and
    /// exact frontier shells classified into a retained residual case.
    NoNewWitnessWithinConfiguredFrontier,
    /// All scheduled request anchors exhausted `maximum_local_depth` without
    /// an authenticated candidate covering them.
    MaximumLocalSearchDepthExhausted,
    /// The caller explicitly enabled the heuristic early-stop policy.
    HeuristicStopOnNoStrictImprovement,
}

#[derive(Clone, Debug)]
pub struct GeneratedFamilyFixedPointSectorAttempt {
    ordinal: usize,
    sector: SectorMask,
    solve_ordinal: usize,
    input_material: GeneratedFixedPointMaterialLocator,
    before: GeneratedFixedPointResidualSummary,
    anchor_searches: Box<[GeneratedResidualAnchorSearch]>,
    newly_accepted_candidates: Box<[GeneratedAcceptedCandidateReference]>,
    outcome: GeneratedFamilyFixedPointAttemptOutcome,
}

impl GeneratedFamilyFixedPointSectorAttempt {
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }
    pub const fn sector(&self) -> &SectorMask {
        &self.sector
    }
    pub const fn solve_ordinal(&self) -> usize {
        self.solve_ordinal
    }
    pub const fn input_material(&self) -> GeneratedFixedPointMaterialLocator {
        self.input_material
    }
    pub const fn before(&self) -> &GeneratedFixedPointResidualSummary {
        &self.before
    }
    pub fn anchor_searches(&self) -> &[GeneratedResidualAnchorSearch] {
        &self.anchor_searches
    }
    pub fn newly_accepted_candidates(&self) -> &[GeneratedAcceptedCandidateReference] {
        &self.newly_accepted_candidates
    }
    pub const fn outcome(&self) -> &GeneratedFamilyFixedPointAttemptOutcome {
        &self.outcome
    }
}

#[derive(Clone, Debug)]
pub struct GeneratedFamilyFixedPointRound {
    ordinal: usize,
    attempts: Box<[GeneratedFamilyFixedPointSectorAttempt]>,
}

impl GeneratedFamilyFixedPointRound {
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }
    pub fn attempts(&self) -> &[GeneratedFamilyFixedPointSectorAttempt] {
        &self.attempts
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedFamilyFixedPointFinalStatus {
    CoveredByGeneratedRules,
    AnchorWitnessSearchExhaustedWithinConfiguredBounds {
        residual: GeneratedFixedPointResidualSummary,
        reason: GeneratedAnchorWitnessSearchExhaustionReason,
    },
    ExhaustedAtMaximumRounds {
        residual: GeneratedFixedPointResidualSummary,
    },
    StalledNoStrictResidualImprovement {
        residual: GeneratedFixedPointResidualSummary,
    },
    NotSelectedByPolicyBound {
        residual: GeneratedFixedPointResidualSummary,
    },
    ResourceLimited {
        interruption: GeneratedFamilyFixedPointInterruption,
    },
    Failed {
        interruption: GeneratedFamilyFixedPointInterruption,
    },
}

#[derive(Clone, Debug)]
pub struct GeneratedFamilyFixedPointSectorStatus {
    sector: SectorMask,
    solve_ordinal: usize,
    latest_material: GeneratedFixedPointMaterialLocator,
    cumulative_accepted_candidates: Box<[GeneratedAcceptedCandidateReference]>,
    status: GeneratedFamilyFixedPointFinalStatus,
}

impl GeneratedFamilyFixedPointSectorStatus {
    pub const fn sector(&self) -> &SectorMask {
        &self.sector
    }
    pub const fn solve_ordinal(&self) -> usize {
        self.solve_ordinal
    }
    pub const fn latest_material(&self) -> GeneratedFixedPointMaterialLocator {
        self.latest_material
    }
    pub fn cumulative_accepted_candidates(&self) -> &[GeneratedAcceptedCandidateReference] {
        &self.cumulative_accepted_candidates
    }
    pub const fn status(&self) -> &GeneratedFamilyFixedPointFinalStatus {
        &self.status
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedFamilyFixedPointStats {
    base_preparations: usize,
    completed_base_preparations: usize,
    rounds: usize,
    sector_attempts: usize,
    anchor_searches: usize,
    anchor_origins: usize,
    assignment_entries_referenced: usize,
    visited_candidates: usize,
    retained_visited_source_rows: usize,
    retained_visited_source_terms: usize,
    retained_visited_source_manifest_bytes: usize,
    retained_visited_candidate_binding_bytes: usize,
    retained_visited_condition_terms: usize,
    retained_visited_condition_bytes: usize,
    accepted_candidate_references: usize,
    material_locator_references: usize,
    locator_references: usize,
    residual_leaf_references: usize,
    residual_predicate_references: usize,
    resource_limited: usize,
    failed: usize,
    shared_row_span_material_reuses: usize,
}

impl GeneratedFamilyFixedPointStats {
    pub const fn base_preparations(self) -> usize {
        self.base_preparations
    }
    pub const fn completed_base_preparations(self) -> usize {
        self.completed_base_preparations
    }
    pub const fn rounds(self) -> usize {
        self.rounds
    }
    pub const fn sector_attempts(self) -> usize {
        self.sector_attempts
    }
    pub const fn anchor_searches(self) -> usize {
        self.anchor_searches
    }
    pub const fn anchor_origins(self) -> usize {
        self.anchor_origins
    }
    pub const fn assignment_entries_referenced(self) -> usize {
        self.assignment_entries_referenced
    }
    pub const fn visited_candidates(self) -> usize {
        self.visited_candidates
    }
    pub const fn retained_visited_source_rows(self) -> usize {
        self.retained_visited_source_rows
    }
    pub const fn retained_visited_source_terms(self) -> usize {
        self.retained_visited_source_terms
    }
    pub const fn retained_visited_source_manifest_bytes(self) -> usize {
        self.retained_visited_source_manifest_bytes
    }
    pub const fn retained_visited_candidate_binding_bytes(self) -> usize {
        self.retained_visited_candidate_binding_bytes
    }
    pub const fn retained_visited_condition_terms(self) -> usize {
        self.retained_visited_condition_terms
    }
    pub const fn retained_visited_condition_bytes(self) -> usize {
        self.retained_visited_condition_bytes
    }
    pub const fn accepted_candidate_references(self) -> usize {
        self.accepted_candidate_references
    }
    pub const fn material_locator_references(self) -> usize {
        self.material_locator_references
    }
    pub const fn locator_references(self) -> usize {
        self.locator_references
    }
    pub const fn residual_leaf_references(self) -> usize {
        self.residual_leaf_references
    }
    pub const fn residual_predicate_references(self) -> usize {
        self.residual_predicate_references
    }
    pub const fn resource_limited(self) -> usize {
        self.resource_limited
    }
    pub const fn failed(self) -> usize {
        self.failed
    }
    pub const fn shared_row_span_material_reuses(self) -> usize {
        self.shared_row_span_material_reuses
    }
}

/// Borrowed resolution of one material locator.  The owned discovery and
/// queue stay in exactly one history node; providers and origin replay use
/// this view instead of cloning them into final statuses.
pub struct GeneratedFixedPointMaterialRef<'certificate> {
    locator: GeneratedFixedPointMaterialLocator,
    sector: &'certificate SectorMask,
    discovery: &'certificate GeneratedSectorDiscoveryCertificate,
    live_leaf_queue: &'certificate GeneratedSectorLiveLeafQueueCertificate,
}

impl<'certificate> GeneratedFixedPointMaterialRef<'certificate> {
    pub const fn locator(&self) -> GeneratedFixedPointMaterialLocator {
        self.locator
    }
    pub const fn sector(&self) -> &SectorMask {
        self.sector
    }
    pub const fn discovery(&self) -> &GeneratedSectorDiscoveryCertificate {
        self.discovery
    }
    pub const fn live_leaf_queue(&self) -> &GeneratedSectorLiveLeafQueueCertificate {
        self.live_leaf_queue
    }
}

/// Immutable proof transcript.  Construction remains private to the compiler;
/// replay must rebuild the entire schedule rather than trust retained V5
/// compilations alone.
#[derive(Clone, Debug)]
pub struct GeneratedFamilyFixedPointCertificate {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    base: GeneratedFamilyRuleSystemCertificate,
    config: GeneratedFamilyFixedPointConfig,
    limits: GeneratedFamilyFixedPointLimits,
    base_preparations: Box<[GeneratedFamilyFixedPointBasePreparation]>,
    rounds: Box<[GeneratedFamilyFixedPointRound]>,
    final_statuses: Box<[GeneratedFamilyFixedPointSectorStatus]>,
    stats: GeneratedFamilyFixedPointStats,
}

impl GeneratedFamilyFixedPointCertificate {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }
    pub const fn base(&self) -> &GeneratedFamilyRuleSystemCertificate {
        &self.base
    }
    pub const fn config(&self) -> GeneratedFamilyFixedPointConfig {
        self.config
    }
    pub const fn limits(&self) -> GeneratedFamilyFixedPointLimits {
        self.limits
    }
    pub fn base_preparations(&self) -> &[GeneratedFamilyFixedPointBasePreparation] {
        &self.base_preparations
    }
    pub fn rounds(&self) -> &[GeneratedFamilyFixedPointRound] {
        &self.rounds
    }
    pub fn final_statuses(&self) -> &[GeneratedFamilyFixedPointSectorStatus] {
        &self.final_statuses
    }
    pub const fn stats(&self) -> GeneratedFamilyFixedPointStats {
        self.stats
    }

    /// Resolve a history locator without duplicating its proof payload.
    /// Malformed/out-of-range locators return `None`; compiler and replay paths
    /// treat that as an exact transcript mismatch.
    pub fn material(
        &self,
        locator: GeneratedFixedPointMaterialLocator,
    ) -> Option<GeneratedFixedPointMaterialRef<'_>> {
        match locator {
            GeneratedFixedPointMaterialLocator::BaseRuleSystem { solve_ordinal } => {
                let sector = self.base.solve_order().get(solve_ordinal)?;
                let GeneratedFamilySectorStatus::Unresolved {
                    discovery,
                    live_leaf_queue,
                    ..
                } = self.base.status(sector)?
                else {
                    return None;
                };
                Some(GeneratedFixedPointMaterialRef {
                    locator,
                    sector,
                    discovery,
                    live_leaf_queue,
                })
            }
            GeneratedFixedPointMaterialLocator::BasePreparation {
                preparation_ordinal,
            } => {
                let preparation = self
                    .base_preparations
                    .iter()
                    .find(|preparation| preparation.ordinal == preparation_ordinal)?;
                let (discovery, live_leaf_queue) = preparation.outcome.material()?;
                Some(GeneratedFixedPointMaterialRef {
                    locator,
                    sector: &preparation.sector,
                    discovery,
                    live_leaf_queue,
                })
            }
            GeneratedFixedPointMaterialLocator::ResidualRound {
                round_ordinal,
                sector_attempt_ordinal,
            } => {
                let round = self
                    .rounds
                    .iter()
                    .find(|round| round.ordinal == round_ordinal)?;
                let attempt = round
                    .attempts
                    .iter()
                    .find(|attempt| attempt.ordinal == sector_attempt_ordinal)?;
                let (discovery, live_leaf_queue) = attempt.outcome.material()?;
                Some(GeneratedFixedPointMaterialRef {
                    locator,
                    sector: &attempt.sector,
                    discovery,
                    live_leaf_queue,
                })
            }
        }
    }

    pub fn final_status(
        &self,
        sector: &SectorMask,
    ) -> Option<&GeneratedFamilyFixedPointSectorStatus> {
        self.final_statuses
            .iter()
            .find(|status| status.sector() == sector)
    }

    /// Resolve the exact latest material for every retained final status in
    /// final-status order.  A malformed private transcript is fail-closed by
    /// returning `None` rather than silently falling back to older material.
    pub fn latest_materials(&self) -> Option<Vec<GeneratedFixedPointMaterialRef<'_>>> {
        self.final_statuses
            .iter()
            .map(|status| self.material(status.latest_material))
            .collect()
    }
}

/// Compiler marker.  The implementation is intentionally added separately
/// from the proof vocabulary so review can freeze the scheduler contract.
pub struct GeneratedFamilyFixedPointCompiler;

impl GeneratedFamilyFixedPointCertificate {
    /// Replay the base family proof and rebuild the complete deterministic
    /// fixed-point schedule.  Candidate discovery, request-point usefulness,
    /// global `WhenBad` composition, and every history locator are compared;
    /// retained material is never trusted merely because it can replay in
    /// isolation.
    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedFamilyFixedPointError> {
        validate_fixed_point_scope(family, context, &self.base)?;
        if self.schema != GENERATED_FAMILY_FIXED_POINT_V1_SCHEMA {
            return Err(GeneratedFamilyFixedPointError::SchemaMismatch);
        }
        if self.family_fingerprint.as_ref() != family.fingerprint() {
            return Err(GeneratedFamilyFixedPointError::WrongFamily);
        }
        if self.context_fingerprint.as_ref() != context.fingerprint() {
            return Err(GeneratedFamilyFixedPointError::WrongContext);
        }
        validate_fixed_point_config(self.config, self.limits)?;
        self.base.replay(family, context)?;
        let rebuilt = GeneratedFamilyFixedPointCompiler::compile_with_replayed_base(
            family,
            context,
            self.base.clone(),
            self.config,
            self.limits,
        )?;
        if fixed_point_certificate_payload_eq(self, &rebuilt) {
            Ok(())
        } else {
            Err(GeneratedFamilyFixedPointError::ReplayMismatch {
                detail: "fixed-point transcript differs",
            })
        }
    }
}

impl GeneratedFamilyFixedPointCompiler {
    pub fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        base: GeneratedFamilyRuleSystemCertificate,
        config: GeneratedFamilyFixedPointConfig,
        limits: GeneratedFamilyFixedPointLimits,
    ) -> Result<GeneratedFamilyFixedPointCertificate, GeneratedFamilyFixedPointError> {
        validate_fixed_point_scope(family, context, &base)?;
        validate_fixed_point_config(config, limits)?;
        base.replay(family, context)?;
        Self::compile_with_replayed_base(family, context, base, config, limits)
    }

    fn compile_with_replayed_base(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        base: GeneratedFamilyRuleSystemCertificate,
        config: GeneratedFamilyFixedPointConfig,
        limits: GeneratedFamilyFixedPointLimits,
    ) -> Result<GeneratedFamilyFixedPointCertificate, GeneratedFamilyFixedPointError> {
        validate_fixed_point_scope(family, context, &base)?;
        validate_fixed_point_config(config, limits)?;
        let shared = base.row_span_arc().cloned();
        let selected_solves = selected_solve_ordinals(&base, config.selection)?;
        let selected_set = selected_solves.iter().copied().collect::<BTreeSet<_>>();

        let mut working = Vec::<WorkingFixedPointSector>::new();
        for (solve_ordinal, sector) in base.solve_order().iter().enumerate() {
            let Some(GeneratedFamilySectorStatus::Unresolved {
                discovery,
                live_leaf_queue,
                ..
            }) = base.status(sector)
            else {
                continue;
            };
            let locator = GeneratedFixedPointMaterialLocator::BaseRuleSystem { solve_ordinal };
            let residual = fixed_point_residual_summary(discovery, live_leaf_queue, limits)?;
            working.push(WorkingFixedPointSector {
                sector: sector.clone(),
                solve_ordinal,
                selected: selected_set.contains(&solve_ordinal),
                latest_material: locator,
                residual,
                cumulative_accepted: Vec::new(),
                search_misses: BTreeMap::new(),
                selected_anchors: BTreeSet::new(),
                stop: if live_leaf_queue.work_items().is_empty() {
                    WorkingFixedPointStop::Covered
                } else {
                    WorkingFixedPointStop::Active
                },
            });
        }
        if !working.is_empty() && shared.is_none() {
            return Err(GeneratedFamilyFixedPointError::ReplayMismatch {
                detail: "generated fixed-point sectors have no shared row span",
            });
        }

        let mut base_preparations = Vec::new();
        for state in working.iter_mut().filter(|state| state.selected) {
            fixed_point_check_limit(
                "fixed-point base preparations",
                fixed_point_checked_add(
                    "fixed-point base preparations",
                    base_preparations.len(),
                    1,
                )?,
                limits.max_base_preparations,
            )?;
            let preparation_ordinal = base_preparations.len();
            let before = state.residual.clone();
            let input_material = state.latest_material;
            let mut discovery_limits = base.limits().discovery;
            discovery_limits.adaptive.max_search_depth = config.base_search_depth;
            let shared = shared
                .clone()
                .ok_or(GeneratedFamilyFixedPointError::ReplayMismatch {
                    detail: "selected base preparation has no shared row span",
                })?;
            let search_discovery =
                match GeneratedSectorDiscoveryCompiler::compile_with_replayed_row_span(
                    family,
                    context,
                    state.sector.clone(),
                    base.ordering(),
                    shared.clone(),
                    discovery_limits,
                ) {
                    Ok(discovery) => discovery,
                    Err(error) => {
                        let interruption = GeneratedFamilyFixedPointInterruption::BaseDiscovery {
                            error: error.clone(),
                        };
                        state.stop = if discovery_error_is_resource(&error) {
                            WorkingFixedPointStop::ResourceLimited(interruption.clone())
                        } else {
                            WorkingFixedPointStop::Failed(interruption.clone())
                        };
                        let outcome = if discovery_error_is_resource(&error) {
                            GeneratedFamilyFixedPointBasePreparationOutcome::ResourceLimited {
                                interruption,
                                completed_discovery: None,
                            }
                        } else {
                            GeneratedFamilyFixedPointBasePreparationOutcome::Failed {
                                interruption,
                                completed_discovery: None,
                            }
                        };
                        base_preparations.push(GeneratedFamilyFixedPointBasePreparation {
                            ordinal: preparation_ordinal,
                            sector: state.sector.clone(),
                            solve_ordinal: state.solve_ordinal,
                            input_material,
                            before,
                            outcome,
                        });
                        continue;
                    }
                };
            let selected_source_ordinals = useful_base_candidate_ordinals(&search_discovery)?;
            let base_discovery = match base.status(&state.sector) {
                Some(GeneratedFamilySectorStatus::Unresolved { discovery, .. }) => discovery,
                _ => {
                    return Err(GeneratedFamilyFixedPointError::ReplayMismatch {
                        detail: "phase-zero input material is not an unresolved base sector",
                    });
                }
            };
            let base_attempts = base_discovery.coverage().candidate_attempts();
            fixed_point_check_limit(
                "fixed-point accepted references",
                base_attempts.len(),
                limits.max_accepted_candidate_references,
            )?;
            // Preserve every input attempt first.  Fresh phase-zero attempts
            // are charged only after payload de-duplication: preflighting the
            // worst-case `base + selected` length would reject a valid replay
            // whenever a low reference limit exactly fits the base material
            // and every selected search result is already present there.
            let mut compilations = Vec::with_capacity(base_attempts.len());
            let mut accepted = Vec::with_capacity(base_attempts.len());
            for (source_candidate_ordinal, attempt) in base_attempts.iter().enumerate() {
                let composed_candidate_ordinal = compilations.len();
                compilations.push(attempt.compilation().clone());
                accepted.push(GeneratedAcceptedCandidateReference {
                    composed_candidate_ordinal,
                    origin: GeneratedAcceptedCandidateOrigin::BaseInputCandidateOrdinal {
                        solve_ordinal: state.solve_ordinal,
                        source_candidate_ordinal,
                    },
                });
            }
            for source_candidate_ordinal in selected_source_ordinals {
                let compilation = search_discovery
                    .coverage()
                    .candidate_attempts()
                    .get(source_candidate_ordinal)
                    .ok_or(GeneratedFamilyFixedPointError::ReplayMismatch {
                        detail: "phase-zero candidate ordinal is out of range",
                    })?
                    .compilation()
                    .clone();
                if compilations
                    .iter()
                    .any(|retained| retained.payload_eq(&compilation))
                {
                    continue;
                }
                fixed_point_check_limit(
                    "fixed-point accepted references",
                    fixed_point_checked_add("fixed-point accepted references", accepted.len(), 1)?,
                    limits.max_accepted_candidate_references,
                )?;
                let composed_candidate_ordinal = compilations.len();
                compilations.push(compilation);
                accepted.push(GeneratedAcceptedCandidateReference {
                    composed_candidate_ordinal,
                    origin: GeneratedAcceptedCandidateOrigin::BaseDescendingOrdinal {
                        preparation_ordinal,
                        source_candidate_ordinal,
                    },
                });
            }
            let discovery =
                match GeneratedSectorDiscoveryCompiler::compose_accepted_with_replayed_row_span(
                    family,
                    context,
                    state.sector.clone(),
                    base.ordering(),
                    compilations,
                    shared.clone(),
                    discovery_limits,
                ) {
                    Ok(discovery) => discovery,
                    Err(error) => {
                        let interruption = GeneratedFamilyFixedPointInterruption::BaseDiscovery {
                            error: error.clone(),
                        };
                        state.stop = if discovery_error_is_resource(&error) {
                            WorkingFixedPointStop::ResourceLimited(interruption.clone())
                        } else {
                            WorkingFixedPointStop::Failed(interruption.clone())
                        };
                        let outcome = if discovery_error_is_resource(&error) {
                            GeneratedFamilyFixedPointBasePreparationOutcome::ResourceLimited {
                                interruption,
                                completed_discovery: Some(search_discovery),
                            }
                        } else {
                            GeneratedFamilyFixedPointBasePreparationOutcome::Failed {
                                interruption,
                                completed_discovery: Some(search_discovery),
                            }
                        };
                        base_preparations.push(GeneratedFamilyFixedPointBasePreparation {
                            ordinal: preparation_ordinal,
                            sector: state.sector.clone(),
                            solve_ordinal: state.solve_ordinal,
                            input_material,
                            before,
                            outcome,
                        });
                        continue;
                    }
                };
            let live_leaf_queue =
                match GeneratedSectorLiveLeafQueueCompiler::compile_with_replayed_row_span(
                    family,
                    context,
                    &discovery,
                    shared,
                    base.limits().live_leaf_queue,
                ) {
                    Ok(queue) => queue,
                    Err(error) => {
                        let interruption =
                            GeneratedFamilyFixedPointInterruption::BaseLiveLeafQueue {
                                error: error.clone(),
                            };
                        state.stop = if queue_error_is_resource(&error) {
                            WorkingFixedPointStop::ResourceLimited(interruption.clone())
                        } else {
                            WorkingFixedPointStop::Failed(interruption.clone())
                        };
                        let outcome = if queue_error_is_resource(&error) {
                            GeneratedFamilyFixedPointBasePreparationOutcome::ResourceLimited {
                                interruption,
                                completed_discovery: Some(discovery),
                            }
                        } else {
                            GeneratedFamilyFixedPointBasePreparationOutcome::Failed {
                                interruption,
                                completed_discovery: Some(discovery),
                            }
                        };
                        base_preparations.push(GeneratedFamilyFixedPointBasePreparation {
                            ordinal: preparation_ordinal,
                            sector: state.sector.clone(),
                            solve_ordinal: state.solve_ordinal,
                            input_material,
                            before,
                            outcome,
                        });
                        continue;
                    }
                };
            validate_fixed_point_material(&shared_or_base(&base)?, &discovery, &live_leaf_queue)?;
            let after = fixed_point_residual_summary(&discovery, &live_leaf_queue, limits)?;
            let locator = GeneratedFixedPointMaterialLocator::BasePreparation {
                preparation_ordinal,
            };
            state.latest_material = locator;
            state.residual = after.clone();
            state.cumulative_accepted = accepted.clone();
            state.stop = if after.is_empty() {
                WorkingFixedPointStop::Covered
            } else {
                WorkingFixedPointStop::Active
            };
            base_preparations.push(GeneratedFamilyFixedPointBasePreparation {
                ordinal: preparation_ordinal,
                sector: state.sector.clone(),
                solve_ordinal: state.solve_ordinal,
                input_material,
                before,
                outcome: GeneratedFamilyFixedPointBasePreparationOutcome::Prepared {
                    search_discovery,
                    after,
                    discovery,
                    live_leaf_queue,
                    accepted_candidates: accepted.into_boxed_slice(),
                },
            });
        }

        let mut rounds = Vec::new();
        let mut aggregate_attempts = 0usize;
        for round_ordinal in 0..config.maximum_rounds {
            let active_positions = working
                .iter()
                .enumerate()
                .filter_map(|(position, state)| {
                    (state.selected && matches!(state.stop, WorkingFixedPointStop::Active))
                        .then_some(position)
                })
                .collect::<Vec<_>>();
            if active_positions.is_empty() {
                break;
            }
            fixed_point_check_limit(
                "fixed-point rounds",
                fixed_point_checked_add("fixed-point rounds", rounds.len(), 1)?,
                limits.max_rounds,
            )?;
            fixed_point_check_limit(
                "fixed-point sector attempts",
                fixed_point_checked_add(
                    "fixed-point sector attempts",
                    aggregate_attempts,
                    active_positions.len(),
                )?,
                limits.max_sector_attempts,
            )?;
            let mut attempts = Vec::with_capacity(active_positions.len());
            for position in active_positions {
                let state = &mut working[position];
                let attempt_ordinal = aggregate_attempts + attempts.len();
                let input_material = state.latest_material;
                let before = state.residual.clone();
                let input =
                    resolve_working_material(&base, &base_preparations, &rounds, input_material)?;
                let mut grouped = derive_residual_anchor_requests(
                    context,
                    base.ordering(),
                    state.sector.clone(),
                    input_material,
                    input.1,
                    config,
                    limits,
                )?;
                grouped
                    .sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
                let grouped = grouped
                    .into_iter()
                    .filter(|(_, anchor, _)| !state.selected_anchors.contains(anchor))
                    .collect::<Vec<_>>();
                if grouped.is_empty() {
                    state.stop = WorkingFixedPointStop::AnchorWitnessSearchExhausted(
                        GeneratedAnchorWitnessSearchExhaustionReason::NoNewWitnessWithinConfiguredFrontier,
                    );
                    attempts.push(GeneratedFamilyFixedPointSectorAttempt {
                        ordinal: attempt_ordinal,
                        sector: state.sector.clone(),
                        solve_ordinal: state.solve_ordinal,
                        input_material,
                        before: before.clone(),
                        anchor_searches: Box::new([]),
                        newly_accepted_candidates: Box::new([]),
                        outcome: GeneratedFamilyFixedPointAttemptOutcome::AnchorWitnessSearchExhaustedWithinConfiguredBounds {
                            after: before,
                        },
                    });
                    continue;
                }

                let mut searches = Vec::new();
                let mut newly_accepted_compilations = Vec::new();
                let mut newly_accepted_references = Vec::new();
                let mut interruption = None;
                for (_, request_anchor, origins) in grouped {
                    fixed_point_check_limit(
                        "fixed-point retained anchor searches",
                        fixed_point_checked_add(
                            "fixed-point retained anchor searches",
                            searches.len(),
                            1,
                        )?,
                        limits.max_retained_anchor_searches,
                    )?;
                    let miss_count = state
                        .search_misses
                        .get(&request_anchor)
                        .copied()
                        .unwrap_or(0);
                    let requested_depth = config
                        .residual_anchor_local_depth
                        .saturating_add(miss_count)
                        .min(config.maximum_local_depth);
                    let search_ordinal = searches.len();
                    let mut adaptive_limits = base.limits().discovery.adaptive;
                    adaptive_limits.max_search_depth = requested_depth;
                    let adaptive_row_span = shared_or_base(&base)?;
                    let mut adaptive = match AdaptiveParametricRuleProvider::try_new(
                        context,
                        adaptive_row_span.rows(),
                        base.ordering(),
                        adaptive_limits,
                    ) {
                        Ok(provider) => provider,
                        Err(error) => {
                            interruption = Some(
                                GeneratedFamilyFixedPointInterruption::ResidualAdaptiveSearch {
                                    anchor_search_ordinal: search_ordinal,
                                    error,
                                },
                            );
                            break;
                        }
                    };
                    let layers = match adaptive.candidate_layers_for_quotient(&request_anchor) {
                        Ok(layers) => layers,
                        Err(error) => {
                            interruption = Some(
                                GeneratedFamilyFixedPointInterruption::ResidualAdaptiveSearch {
                                    anchor_search_ordinal: search_ordinal,
                                    error,
                                },
                            );
                            break;
                        }
                    };
                    let mut visited = Vec::new();
                    let mut selected_visit_ordinal = None;
                    'layers: for (local_depth, layer) in layers.into_iter().enumerate() {
                        for (within_layer_ordinal, candidate) in layer.into_iter().enumerate() {
                            fixed_point_check_limit(
                                "fixed-point visited candidates",
                                fixed_point_checked_add(
                                    "fixed-point visited candidates",
                                    visited.len(),
                                    1,
                                )?,
                                limits.max_visited_candidates,
                            )?;
                            let locator = GeneratedResidualCandidateLocator {
                                local_depth,
                                within_layer_ordinal,
                            };
                            let compilation =
                                match GeneratedWhenBadCompiler::compile_with_replayed_row_span(
                                    family,
                                    context,
                                    &candidate,
                                    shared_or_base(&base)?,
                                    base.limits().discovery.coverage.generated_when_bad,
                                ) {
                                    Ok(compilation) => compilation,
                                    Err(error) => {
                                        interruption = Some(
                                        GeneratedFamilyFixedPointInterruption::ResidualCandidateAuthentication {
                                            anchor_search_ordinal: search_ordinal,
                                            locator,
                                            error,
                                        },
                                    );
                                        break 'layers;
                                    }
                                };
                            let outcome = match compilation {
                                GeneratedWhenBadCompilation::Unsupported(_) => {
                                    GeneratedResidualCandidateOutcome::Unsupported { compilation }
                                }
                                GeneratedWhenBadCompilation::Certified(ref certificate) => {
                                    let point = match certificate
                                        .admissibility()
                                        .classification_for_indices(
                                            context,
                                            request_anchor.powers(),
                                        ) {
                                        Ok(point) => point,
                                        Err(error) => {
                                            interruption = Some(
                                                GeneratedFamilyFixedPointInterruption::ResidualPointEvaluation {
                                                    anchor_search_ordinal: search_ordinal,
                                                    locator,
                                                    error,
                                                },
                                            );
                                            break 'layers;
                                        }
                                    };
                                    if point.is_some_and(|classification| {
                                        matches!(
                                            classification.disposition(),
                                            WhenBadLeafDisposition::CoveredByCandidate
                                        )
                                    }) {
                                        GeneratedResidualCandidateOutcome::CertifiedCoveredRequestAnchor {
                                            compilation,
                                        }
                                    } else {
                                        GeneratedResidualCandidateOutcome::CertifiedNotCoveringRequestAnchor {
                                            compilation,
                                        }
                                    }
                                }
                            };
                            let covers = outcome.covers_request_anchor();
                            visited.push(GeneratedResidualCandidateVisit { locator, outcome });
                            if covers {
                                let visit_ordinal = visited.len() - 1;
                                selected_visit_ordinal = Some(visit_ordinal);
                                let compilation =
                                    visited[visit_ordinal].outcome.compilation().clone();
                                let composed_candidate_ordinal = state
                                    .cumulative_accepted
                                    .len()
                                    .checked_add(newly_accepted_compilations.len())
                                    .ok_or(
                                        GeneratedFamilyFixedPointError::ResourceCountOverflow {
                                            resource: "fixed-point composed candidate ordinal",
                                        },
                                    )?;
                                fixed_point_check_limit(
                                    "fixed-point accepted references",
                                    fixed_point_checked_add(
                                        "fixed-point accepted references",
                                        composed_candidate_ordinal,
                                        1,
                                    )?,
                                    limits.max_accepted_candidate_references,
                                )?;
                                newly_accepted_compilations.push(compilation);
                                newly_accepted_references.push(
                                    GeneratedAcceptedCandidateReference {
                                        composed_candidate_ordinal,
                                        origin:
                                            GeneratedAcceptedCandidateOrigin::ResidualSelection {
                                                round_ordinal,
                                                sector_attempt_ordinal: attempt_ordinal,
                                                anchor_search_ordinal: search_ordinal,
                                                visit_ordinal,
                                            },
                                    },
                                );
                                state.selected_anchors.insert(request_anchor.clone());
                                break 'layers;
                            }
                        }
                    }
                    if selected_visit_ordinal.is_none() {
                        let misses = state
                            .search_misses
                            .entry(request_anchor.clone())
                            .or_insert(0);
                        *misses = fixed_point_checked_add(
                            "fixed-point anchor search misses",
                            *misses,
                            1,
                        )?;
                    }
                    searches.push(GeneratedResidualAnchorSearch {
                        request_anchor,
                        requested_local_depth: requested_depth,
                        origins: origins.into_boxed_slice(),
                        visited: visited.into_boxed_slice(),
                        selected_visit_ordinal,
                    });
                    if interruption.is_some() {
                        break;
                    }
                }

                if let Some(interruption) = interruption {
                    let is_resource = fixed_point_interruption_is_resource(&interruption);
                    state.stop = if is_resource {
                        WorkingFixedPointStop::ResourceLimited(interruption.clone())
                    } else {
                        WorkingFixedPointStop::Failed(interruption.clone())
                    };
                    let outcome = if is_resource {
                        GeneratedFamilyFixedPointAttemptOutcome::ResourceLimited {
                            interruption,
                            completed_discovery: None,
                        }
                    } else {
                        GeneratedFamilyFixedPointAttemptOutcome::Failed {
                            interruption,
                            completed_discovery: None,
                        }
                    };
                    attempts.push(GeneratedFamilyFixedPointSectorAttempt {
                        ordinal: attempt_ordinal,
                        sector: state.sector.clone(),
                        solve_ordinal: state.solve_ordinal,
                        input_material,
                        before,
                        anchor_searches: searches.into_boxed_slice(),
                        newly_accepted_candidates: newly_accepted_references.into_boxed_slice(),
                        outcome,
                    });
                    continue;
                }

                if newly_accepted_compilations.is_empty() {
                    let maximum_local_depth_exhausted =
                        state.search_misses.values().all(|misses| {
                            config.residual_anchor_local_depth.saturating_add(*misses)
                                > config.maximum_local_depth
                        });
                    if config.stop_on_no_strict_improvement {
                        state.stop = WorkingFixedPointStop::AnchorWitnessSearchExhausted(
                            GeneratedAnchorWitnessSearchExhaustionReason::HeuristicStopOnNoStrictImprovement,
                        );
                    } else if maximum_local_depth_exhausted {
                        state.stop = WorkingFixedPointStop::AnchorWitnessSearchExhausted(
                            GeneratedAnchorWitnessSearchExhaustionReason::MaximumLocalSearchDepthExhausted,
                        );
                    }
                    attempts.push(GeneratedFamilyFixedPointSectorAttempt {
                        ordinal: attempt_ordinal,
                        sector: state.sector.clone(),
                        solve_ordinal: state.solve_ordinal,
                        input_material,
                        before: before.clone(),
                        anchor_searches: searches.into_boxed_slice(),
                        newly_accepted_candidates: Box::new([]),
                        outcome: GeneratedFamilyFixedPointAttemptOutcome::NoCandidateCoveredRequestAnchors {
                            after: before,
                        },
                    });
                    continue;
                }

                let mut all_compilations = input
                    .0
                    .coverage()
                    .candidate_attempts()
                    .iter()
                    .map(|attempt| attempt.compilation().clone())
                    .collect::<Vec<_>>();
                all_compilations.extend(newly_accepted_compilations);
                let discovery =
                    match GeneratedSectorDiscoveryCompiler::compose_accepted_with_replayed_row_span(
                        family,
                        context,
                        state.sector.clone(),
                        base.ordering(),
                        all_compilations,
                        shared_or_base(&base)?,
                        base.limits().discovery,
                    ) {
                        Ok(discovery) => discovery,
                        Err(error) => {
                            let interruption =
                                GeneratedFamilyFixedPointInterruption::GlobalComposition {
                                    error: error.clone(),
                                };
                            let is_resource = discovery_error_is_resource(&error);
                            state.stop = if is_resource {
                                WorkingFixedPointStop::ResourceLimited(interruption.clone())
                            } else {
                                WorkingFixedPointStop::Failed(interruption.clone())
                            };
                            let outcome = if is_resource {
                                GeneratedFamilyFixedPointAttemptOutcome::ResourceLimited {
                                    interruption,
                                    completed_discovery: None,
                                }
                            } else {
                                GeneratedFamilyFixedPointAttemptOutcome::Failed {
                                    interruption,
                                    completed_discovery: None,
                                }
                            };
                            attempts.push(GeneratedFamilyFixedPointSectorAttempt {
                                ordinal: attempt_ordinal,
                                sector: state.sector.clone(),
                                solve_ordinal: state.solve_ordinal,
                                input_material,
                                before,
                                anchor_searches: searches.into_boxed_slice(),
                                newly_accepted_candidates: newly_accepted_references
                                    .into_boxed_slice(),
                                outcome,
                            });
                            continue;
                        }
                    };
                let live_leaf_queue =
                    match GeneratedSectorLiveLeafQueueCompiler::compile_with_replayed_row_span(
                        family,
                        context,
                        &discovery,
                        shared_or_base(&base)?,
                        base.limits().live_leaf_queue,
                    ) {
                        Ok(queue) => queue,
                        Err(error) => {
                            let interruption =
                                GeneratedFamilyFixedPointInterruption::LiveLeafQueue {
                                    error: error.clone(),
                                };
                            let is_resource = queue_error_is_resource(&error);
                            state.stop = if is_resource {
                                WorkingFixedPointStop::ResourceLimited(interruption.clone())
                            } else {
                                WorkingFixedPointStop::Failed(interruption.clone())
                            };
                            let outcome = if is_resource {
                                GeneratedFamilyFixedPointAttemptOutcome::ResourceLimited {
                                    interruption,
                                    completed_discovery: Some(discovery),
                                }
                            } else {
                                GeneratedFamilyFixedPointAttemptOutcome::Failed {
                                    interruption,
                                    completed_discovery: Some(discovery),
                                }
                            };
                            attempts.push(GeneratedFamilyFixedPointSectorAttempt {
                                ordinal: attempt_ordinal,
                                sector: state.sector.clone(),
                                solve_ordinal: state.solve_ordinal,
                                input_material,
                                before,
                                anchor_searches: searches.into_boxed_slice(),
                                newly_accepted_candidates: newly_accepted_references
                                    .into_boxed_slice(),
                                outcome,
                            });
                            continue;
                        }
                    };
                let after = fixed_point_residual_summary(&discovery, &live_leaf_queue, limits)?;
                // Candidate composition is monotone: the complete previous
                // priority list remains first and newly authenticated
                // candidates are appended.  Residual leaf and predicate
                // counts are therefore not a semantic progress measure--an
                // exact bad locus can be split into more structural cells
                // while becoming strictly smaller.  Instead retain a direct
                // integer-domain witness: every selected request anchor must
                // now replay as descending in the newly composed material.
                // A concrete integer point matching `ProvedEmptyLocus` would
                // contradict that structural proof and must fail replay.
                let strict_improvement =
                    fixed_point_selected_anchors_are_closed(context, &discovery, &searches)?;
                let locator = GeneratedFixedPointMaterialLocator::ResidualRound {
                    round_ordinal,
                    sector_attempt_ordinal: attempt_ordinal,
                };
                state.latest_material = locator;
                state.residual = after.clone();
                state
                    .cumulative_accepted
                    .extend(newly_accepted_references.clone());
                state.stop = if after.is_empty() {
                    WorkingFixedPointStop::Covered
                } else if !strict_improvement && config.stop_on_no_strict_improvement {
                    WorkingFixedPointStop::Stalled
                } else {
                    WorkingFixedPointStop::Active
                };
                attempts.push(GeneratedFamilyFixedPointSectorAttempt {
                    ordinal: attempt_ordinal,
                    sector: state.sector.clone(),
                    solve_ordinal: state.solve_ordinal,
                    input_material,
                    before,
                    anchor_searches: searches.into_boxed_slice(),
                    newly_accepted_candidates: newly_accepted_references.into_boxed_slice(),
                    outcome: GeneratedFamilyFixedPointAttemptOutcome::Completed {
                        after,
                        strict_improvement,
                        discovery,
                        live_leaf_queue,
                    },
                });
            }
            aggregate_attempts = fixed_point_checked_add(
                "fixed-point sector attempts",
                aggregate_attempts,
                attempts.len(),
            )?;
            rounds.push(GeneratedFamilyFixedPointRound {
                ordinal: round_ordinal,
                attempts: attempts.into_boxed_slice(),
            });
        }

        fixed_point_check_limit(
            "fixed-point final sector statuses",
            working.len(),
            limits.max_final_sector_statuses,
        )?;
        let final_statuses = working
            .into_iter()
            .map(|state| {
                let status = match state.stop {
                    WorkingFixedPointStop::Covered => {
                        GeneratedFamilyFixedPointFinalStatus::CoveredByGeneratedRules
                    }
                    WorkingFixedPointStop::AnchorWitnessSearchExhausted(reason) => {
                        GeneratedFamilyFixedPointFinalStatus::AnchorWitnessSearchExhaustedWithinConfiguredBounds {
                            residual: state.residual.clone(),
                            reason,
                        }
                    }
                    WorkingFixedPointStop::Stalled => {
                        GeneratedFamilyFixedPointFinalStatus::StalledNoStrictResidualImprovement {
                            residual: state.residual.clone(),
                        }
                    }
                    WorkingFixedPointStop::ResourceLimited(interruption) => {
                        GeneratedFamilyFixedPointFinalStatus::ResourceLimited { interruption }
                    }
                    WorkingFixedPointStop::Failed(interruption) => {
                        GeneratedFamilyFixedPointFinalStatus::Failed { interruption }
                    }
                    WorkingFixedPointStop::Active if !state.selected => {
                        GeneratedFamilyFixedPointFinalStatus::NotSelectedByPolicyBound {
                            residual: state.residual.clone(),
                        }
                    }
                    WorkingFixedPointStop::Active => {
                        GeneratedFamilyFixedPointFinalStatus::ExhaustedAtMaximumRounds {
                            residual: state.residual.clone(),
                        }
                    }
                };
                GeneratedFamilyFixedPointSectorStatus {
                    sector: state.sector,
                    solve_ordinal: state.solve_ordinal,
                    latest_material: state.latest_material,
                    cumulative_accepted_candidates: state
                        .cumulative_accepted
                        .into_boxed_slice(),
                    status,
                }
            })
            .collect::<Vec<_>>();
        let stats =
            compute_fixed_point_stats(&base_preparations, &rounds, &final_statuses, limits)?;
        Ok(GeneratedFamilyFixedPointCertificate {
            schema: GENERATED_FAMILY_FIXED_POINT_V1_SCHEMA,
            family_fingerprint: family.fingerprint().into(),
            context_fingerprint: context.fingerprint().into(),
            base,
            config,
            limits,
            base_preparations: base_preparations.into_boxed_slice(),
            rounds: rounds.into_boxed_slice(),
            final_statuses: final_statuses.into_boxed_slice(),
            stats,
        })
    }
}

type FixedPointBorrowedMaterial<'a> = (
    &'a GeneratedSectorDiscoveryCertificate,
    &'a GeneratedSectorLiveLeafQueueCertificate,
);

#[derive(Clone)]
struct WorkingFixedPointSector {
    sector: SectorMask,
    solve_ordinal: usize,
    selected: bool,
    latest_material: GeneratedFixedPointMaterialLocator,
    residual: GeneratedFixedPointResidualSummary,
    cumulative_accepted: Vec<GeneratedAcceptedCandidateReference>,
    search_misses: BTreeMap<ConcreteIntegralKey, usize>,
    selected_anchors: BTreeSet<ConcreteIntegralKey>,
    stop: WorkingFixedPointStop,
}

#[derive(Clone)]
enum WorkingFixedPointStop {
    Active,
    Covered,
    AnchorWitnessSearchExhausted(GeneratedAnchorWitnessSearchExhaustionReason),
    Stalled,
    ResourceLimited(GeneratedFamilyFixedPointInterruption),
    Failed(GeneratedFamilyFixedPointInterruption),
}

fn validate_fixed_point_scope(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    base: &GeneratedFamilyRuleSystemCertificate,
) -> Result<(), GeneratedFamilyFixedPointError> {
    if base.family_fingerprint() != family.fingerprint() {
        return Err(GeneratedFamilyFixedPointError::WrongFamily);
    }
    if base.context_fingerprint() != context.fingerprint()
        || !family
            .coefficient_context()
            .has_same_variable_map(context.base())
    {
        return Err(GeneratedFamilyFixedPointError::WrongContext);
    }
    if family.denominator_count() != context.index_count() {
        return Err(GeneratedFamilyFixedPointError::WrongArity {
            expected: family.denominator_count(),
            actual: context.index_count(),
        });
    }
    Ok(())
}

fn validate_fixed_point_config(
    config: GeneratedFamilyFixedPointConfig,
    limits: GeneratedFamilyFixedPointLimits,
) -> Result<(), GeneratedFamilyFixedPointError> {
    if config.residual_anchor_local_depth > config.maximum_local_depth {
        return Err(GeneratedFamilyFixedPointError::InvalidConfig {
            detail: "initial residual-anchor depth exceeds its maximum",
        });
    }
    if let GeneratedFamilyFixedPointSelectionPolicy::ResidualSubsectorFirstPrefix {
        max_selected_sectors: 0,
    } = config.selection
    {
        return Err(GeneratedFamilyFixedPointError::InvalidConfig {
            detail: "residual-sector prefix must be nonzero",
        });
    }
    fixed_point_check_limit(
        "fixed-point rounds",
        config.maximum_rounds,
        limits.max_rounds,
    )
}

fn selected_solve_ordinals(
    base: &GeneratedFamilyRuleSystemCertificate,
    policy: GeneratedFamilyFixedPointSelectionPolicy,
) -> Result<Vec<usize>, GeneratedFamilyFixedPointError> {
    let all = base
        .solve_order()
        .iter()
        .enumerate()
        .filter_map(|(ordinal, sector)| {
            matches!(
                base.status(sector),
                Some(GeneratedFamilySectorStatus::Unresolved {
                    live_leaf_queue,
                    ..
                }) if !live_leaf_queue.work_items().is_empty()
            )
            .then_some(ordinal)
        })
        .collect::<Vec<_>>();
    match policy {
        GeneratedFamilyFixedPointSelectionPolicy::AllResidualSubsectorFirst => Ok(all),
        GeneratedFamilyFixedPointSelectionPolicy::ResidualSubsectorFirstPrefix {
            max_selected_sectors,
        } => Ok(all.into_iter().take(max_selected_sectors).collect()),
    }
}

fn useful_base_candidate_ordinals(
    discovery: &GeneratedSectorDiscoveryCertificate,
) -> Result<Vec<usize>, GeneratedFamilyFixedPointError> {
    let mut ordinals = BTreeSet::new();
    for classification in discovery.coverage().classifications() {
        if let ParametricSectorLeafDisposition::DescendingRule { candidate_ordinal } =
            classification.disposition()
        {
            if *candidate_ordinal >= discovery.coverage().candidate_attempts().len() {
                return Err(GeneratedFamilyFixedPointError::ReplayMismatch {
                    detail: "descending phase-zero candidate ordinal is out of range",
                });
            }
            ordinals.insert(*candidate_ordinal);
        }
    }
    Ok(ordinals.into_iter().collect())
}

fn fixed_point_residual_summary(
    discovery: &GeneratedSectorDiscoveryCertificate,
    queue: &GeneratedSectorLiveLeafQueueCertificate,
    limits: GeneratedFamilyFixedPointLimits,
) -> Result<GeneratedFixedPointResidualSummary, GeneratedFamilyFixedPointError> {
    if discovery.sector() != queue.sector()
        || discovery.ordering() != queue.ordering()
        || !discovery.payload_eq(queue.discovery())
    {
        return Err(GeneratedFamilyFixedPointError::ReplayMismatch {
            detail: "fixed-point queue is not bound to its discovery",
        });
    }
    let mut leaves = Vec::with_capacity(queue.work_items().len());
    let mut predicates = 0usize;
    for (ordinal, item) in queue.work_items().iter().enumerate() {
        if item.ordinal() != ordinal {
            return Err(GeneratedFamilyFixedPointError::ReplayMismatch {
                detail: "fixed-point queue ordinal differs",
            });
        }
        let case = discovery
            .coverage()
            .partition()
            .case(item.source_case())
            .ok_or(GeneratedFamilyFixedPointError::ReplayMismatch {
                detail: "fixed-point residual case is absent from the partition",
            })?;
        predicates = fixed_point_checked_add(
            "fixed-point retained residual predicates",
            predicates,
            case.predicates().len(),
        )?;
        fixed_point_check_limit(
            "fixed-point retained residual predicates",
            predicates,
            limits.max_retained_residual_predicates,
        )?;
        leaves.push(GeneratedFixedPointResidualLeafReference {
            work_item_ordinal: ordinal,
            source_case: item.source_case(),
            source_disposition: item.source_disposition().clone(),
        });
        fixed_point_check_limit(
            "fixed-point retained residual leaves",
            leaves.len(),
            limits.max_retained_residual_leaves,
        )?;
    }
    Ok(GeneratedFixedPointResidualSummary {
        leaves: leaves.into_boxed_slice(),
        predicate_instances: predicates,
    })
}

fn fixed_point_selected_anchors_are_closed(
    context: &ParametricCoefficientContext,
    discovery: &GeneratedSectorDiscoveryCertificate,
    searches: &[GeneratedResidualAnchorSearch],
) -> Result<bool, GeneratedFamilyFixedPointError> {
    let mut selected = 0usize;
    for search in searches
        .iter()
        .filter(|search| search.selected_visit_ordinal().is_some())
    {
        selected =
            fixed_point_checked_add("fixed-point semantic improvement witnesses", selected, 1)?;
        let classification = discovery
            .coverage()
            .classification_for_indices(context, search.request_anchor().powers())?
            .ok_or(GeneratedFamilyFixedPointError::ReplayMismatch {
                detail: "selected residual anchor left the composed sector partition",
            })?;
        match classification.disposition() {
            ParametricSectorLeafDisposition::DescendingRule { .. } => {}
            ParametricSectorLeafDisposition::ProvedEmptyLocus { .. } => {
                return Err(GeneratedFamilyFixedPointError::ReplayMismatch {
                    detail: "concrete selected residual anchor matched a proved-empty locus",
                });
            }
            ParametricSectorLeafDisposition::Uncovered
            | ParametricSectorLeafDisposition::Unsupported { .. } => {
                return Err(GeneratedFamilyFixedPointError::ReplayMismatch {
                    detail: "selected residual anchor is not closed by the composed material",
                });
            }
        }
    }
    Ok(selected != 0)
}

fn derive_residual_anchor_requests(
    context: &ParametricCoefficientContext,
    ordering: crate::IntegralOrderingPolicy,
    sector: SectorMask,
    material: GeneratedFixedPointMaterialLocator,
    queue: &GeneratedSectorLiveLeafQueueCertificate,
    config: GeneratedFamilyFixedPointConfig,
    limits: GeneratedFamilyFixedPointLimits,
) -> Result<
    Vec<(
        crate::IntegralComplexityKey,
        ConcreteIntegralKey,
        Vec<GeneratedResidualAnchorOrigin>,
    )>,
    GeneratedFamilyFixedPointError,
> {
    let mut grouped = BTreeMap::<ConcreteIntegralKey, Vec<GeneratedResidualAnchorOrigin>>::new();
    let mut retained_origins = 0usize;
    for item in queue.work_items() {
        if item.extraction().status() != &CoordinateEqualityLeafStatus::NotProvedEmpty {
            continue;
        }
        if !item.extraction().assignment().is_empty() {
            let mut completion = sector.corner_indices();
            let mut assigned = vec![false; context.index_count()];
            for &(position, value) in item.extraction().assignment().entries() {
                *completion.get_mut(position).ok_or(
                    GeneratedFamilyFixedPointError::ReplayMismatch {
                        detail: "coordinate assignment exceeds fixed-point arity",
                    },
                )? = value;
                *assigned.get_mut(position).ok_or(
                    GeneratedFamilyFixedPointError::ReplayMismatch {
                        detail: "coordinate assignment exceeds fixed-point arity",
                    },
                )? = true;
            }
            if !sector.contains_indices(&completion)? {
                return Err(GeneratedFamilyFixedPointError::ReplayMismatch {
                    detail: "coordinate assignment generated an out-of-sector anchor",
                });
            }
            let free_positions = assigned
                .iter()
                .enumerate()
                .filter_map(|(position, &is_assigned)| (!is_assigned).then_some(position))
                .collect::<Vec<_>>();
            for depth in 0..=config.residual_frontier_depth {
                let offsets = if free_positions.is_empty() {
                    if depth == 0 {
                        vec![Vec::new()]
                    } else {
                        Vec::new()
                    }
                } else {
                    fixed_point_exact_shell_offsets(
                        free_positions.len(),
                        depth,
                        limits.max_frontier_offsets,
                        limits.max_frontier_enumeration_steps,
                        limits.max_frontier_components,
                    )?
                };
                for (within_frontier_ordinal, offset) in offsets.into_iter().enumerate() {
                    let mut point = completion.clone();
                    for (&position, delta) in free_positions.iter().zip(offset) {
                        point[position] = point[position].checked_add(delta).ok_or(
                            GeneratedFamilyFixedPointError::ReplayMismatch {
                                detail: "coordinate-completion frontier index overflow",
                            },
                        )?;
                    }
                    if !sector.contains_indices(&point)? {
                        continue;
                    }
                    // Equalities alone do not prove membership in this
                    // residual cell. Bind every completion to the exact input
                    // case before it may select a candidate or witness
                    // progress.
                    let classification = queue
                        .discovery()
                        .coverage()
                        .classification_for_indices(context, &point)?;
                    if classification
                        .is_none_or(|classification| classification.case() != item.source_case())
                    {
                        continue;
                    }
                    let key = ConcreteIntegralKey::try_new(point)?;
                    retained_origins = fixed_point_checked_add(
                        "fixed-point retained anchor origins",
                        retained_origins,
                        1,
                    )?;
                    fixed_point_check_limit(
                        "fixed-point retained anchor origins",
                        retained_origins,
                        limits.max_retained_anchor_origins,
                    )?;
                    let origin = if depth == 0 {
                        GeneratedResidualAnchorOrigin::CoordinateAssignment {
                            material,
                            work_item_ordinal: item.ordinal(),
                        }
                    } else {
                        GeneratedResidualAnchorOrigin::CoordinateCompletionFrontier {
                            material,
                            work_item_ordinal: item.ordinal(),
                            frontier_depth: depth,
                            within_frontier_ordinal,
                        }
                    };
                    grouped.entry(key).or_default().push(origin);
                    fixed_point_check_limit(
                        "fixed-point retained anchor searches",
                        grouped.len(),
                        limits.max_retained_anchor_searches,
                    )?;
                    fixed_point_check_limit(
                        "fixed-point retained frontier points",
                        grouped.len(),
                        limits.max_frontier_points,
                    )?;
                }
            }
            continue;
        }

        // An empty coordinate assignment is still a real symbolic residual
        // cell.  Enumerate deterministic exact L1 shells around the sector
        // corner and retain only points belonging to this exact parent case.
        // This is the generic counterpart of LiteRed rebuilding `startp` and
        // calling `preparepoints`; no topology-specific numerator point is
        // supplied by the caller.
        let corner = sector.corner_indices();
        for depth in 0..=config.residual_frontier_depth {
            let offsets = fixed_point_exact_shell_offsets(
                context.index_count(),
                depth,
                limits.max_frontier_offsets,
                limits.max_frontier_enumeration_steps,
                limits.max_frontier_components,
            )?;
            for (within_frontier_ordinal, offset) in offsets.into_iter().enumerate() {
                let point = corner
                    .iter()
                    .zip(offset)
                    .map(|(&value, delta)| {
                        value.checked_add(delta).ok_or(
                            GeneratedFamilyFixedPointError::ReplayMismatch {
                                detail: "residual-frontier index overflow",
                            },
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if !sector.contains_indices(&point)? {
                    continue;
                }
                let classification = queue
                    .discovery()
                    .coverage()
                    .classification_for_indices(context, &point)?;
                if classification
                    .is_none_or(|classification| classification.case() != item.source_case())
                {
                    continue;
                }
                let key = ConcreteIntegralKey::try_new(point)?;
                retained_origins = fixed_point_checked_add(
                    "fixed-point retained anchor origins",
                    retained_origins,
                    1,
                )?;
                fixed_point_check_limit(
                    "fixed-point retained anchor origins",
                    retained_origins,
                    limits.max_retained_anchor_origins,
                )?;
                grouped.entry(key).or_default().push(
                    GeneratedResidualAnchorOrigin::ResidualFrontier {
                        material,
                        work_item_ordinal: item.ordinal(),
                        frontier_depth: depth,
                        within_frontier_ordinal,
                    },
                );
                fixed_point_check_limit(
                    "fixed-point retained anchor searches",
                    grouped.len(),
                    limits.max_retained_anchor_searches,
                )?;
                fixed_point_check_limit(
                    "fixed-point retained frontier points",
                    grouped.len(),
                    limits.max_frontier_points,
                )?;
            }
        }
    }
    let components = grouped.len().checked_mul(context.index_count()).ok_or(
        GeneratedFamilyFixedPointError::ResourceCountOverflow {
            resource: "fixed-point frontier components",
        },
    )?;
    fixed_point_check_limit(
        "fixed-point frontier components",
        components,
        limits.max_frontier_components,
    )?;
    grouped
        .into_iter()
        .map(|(anchor, mut origins)| {
            origins.sort();
            origins.dedup();
            fixed_point_check_limit(
                "fixed-point retained anchor origins",
                origins.len(),
                limits.max_retained_anchor_origins,
            )?;
            Ok((ordering.complexity_key(anchor.powers())?, anchor, origins))
        })
        .collect()
}

fn fixed_point_exact_shell_offsets(
    arity: usize,
    depth: usize,
    limit: usize,
    step_limit: usize,
    component_limit: usize,
) -> Result<Vec<Vec<i64>>, GeneratedFamilyFixedPointError> {
    if arity == 0 {
        return Err(GeneratedFamilyFixedPointError::WrongArity {
            expected: 1,
            actual: 0,
        });
    }
    fixed_point_check_limit("fixed-point frontier components", arity, component_limit)?;
    let depth_i64 = i64::try_from(depth).map_err(|_| {
        GeneratedFamilyFixedPointError::ResourceCountOverflow {
            resource: "fixed-point frontier offset magnitude",
        }
    })?;
    #[derive(Clone, Copy)]
    struct Frame {
        position: usize,
        remaining: i64,
        next_value: i64,
    }
    let mut output = Vec::new();
    let mut current = vec![0i64; arity];
    let mut stack = vec![Frame {
        position: 0,
        remaining: depth_i64,
        next_value: -depth_i64,
    }];
    let mut steps = 0usize;
    while let Some(frame) = stack.last().copied() {
        steps = fixed_point_checked_add("fixed-point frontier enumeration steps", steps, 1)?;
        fixed_point_check_limit("fixed-point frontier enumeration steps", steps, step_limit)?;
        if frame.position == arity {
            if frame.remaining == 0 {
                let components = fixed_point_checked_mul(
                    "fixed-point frontier components",
                    fixed_point_checked_add("fixed-point frontier offsets", output.len(), 1)?,
                    arity,
                )?;
                fixed_point_check_limit(
                    "fixed-point frontier components",
                    components,
                    component_limit,
                )?;
                fixed_point_check_limit(
                    "fixed-point frontier offsets",
                    fixed_point_checked_add("fixed-point frontier offsets", output.len(), 1)?,
                    limit,
                )?;
                output.push(current.clone());
            }
            stack.pop();
            continue;
        }
        if frame.next_value > frame.remaining {
            stack.pop();
            continue;
        }
        let value = frame.next_value;
        stack
            .last_mut()
            .expect("the copied fixed-point frontier frame remains present")
            .next_value = frame.next_value.checked_add(1).ok_or(
            GeneratedFamilyFixedPointError::ResourceCountOverflow {
                resource: "fixed-point frontier offset enumeration",
            },
        )?;
        let remaining = frame.remaining.checked_sub(value.abs()).ok_or(
            GeneratedFamilyFixedPointError::ResourceCountOverflow {
                resource: "fixed-point frontier remaining magnitude",
            },
        )?;
        current[frame.position] = value;
        stack.push(Frame {
            position: frame.position + 1,
            remaining,
            next_value: -remaining,
        });
    }
    output.sort();
    Ok(output)
}

fn resolve_working_material<'a>(
    base: &'a GeneratedFamilyRuleSystemCertificate,
    preparations: &'a [GeneratedFamilyFixedPointBasePreparation],
    rounds: &'a [GeneratedFamilyFixedPointRound],
    locator: GeneratedFixedPointMaterialLocator,
) -> Result<FixedPointBorrowedMaterial<'a>, GeneratedFamilyFixedPointError> {
    match locator {
        GeneratedFixedPointMaterialLocator::BaseRuleSystem { solve_ordinal } => {
            let sector = base.solve_order().get(solve_ordinal).ok_or(
                GeneratedFamilyFixedPointError::ReplayMismatch {
                    detail: "base material solve ordinal is out of range",
                },
            )?;
            let Some(GeneratedFamilySectorStatus::Unresolved {
                discovery,
                live_leaf_queue,
                ..
            }) = base.status(sector)
            else {
                return Err(GeneratedFamilyFixedPointError::ReplayMismatch {
                    detail: "base material does not resolve to an unresolved sector",
                });
            };
            Ok((discovery, live_leaf_queue))
        }
        GeneratedFixedPointMaterialLocator::BasePreparation {
            preparation_ordinal,
        } => preparations
            .iter()
            .find(|preparation| preparation.ordinal == preparation_ordinal)
            .and_then(|preparation| preparation.outcome.material())
            .ok_or(GeneratedFamilyFixedPointError::ReplayMismatch {
                detail: "base-preparation material locator is unresolved",
            }),
        GeneratedFixedPointMaterialLocator::ResidualRound {
            round_ordinal,
            sector_attempt_ordinal,
        } => rounds
            .iter()
            .find(|round| round.ordinal == round_ordinal)
            .and_then(|round| {
                round
                    .attempts
                    .iter()
                    .find(|attempt| attempt.ordinal == sector_attempt_ordinal)
            })
            .and_then(|attempt| attempt.outcome.material())
            .ok_or(GeneratedFamilyFixedPointError::ReplayMismatch {
                detail: "residual-round material locator is unresolved",
            }),
    }
}

fn shared_or_base(
    base: &GeneratedFamilyRuleSystemCertificate,
) -> Result<Arc<crate::GeneratedSymbolicRowSpanCertificate>, GeneratedFamilyFixedPointError> {
    base.row_span_arc()
        .cloned()
        .ok_or(GeneratedFamilyFixedPointError::ReplayMismatch {
            detail: "fixed-point generated work has no shared row span",
        })
}

fn validate_fixed_point_material(
    shared: &Arc<crate::GeneratedSymbolicRowSpanCertificate>,
    discovery: &GeneratedSectorDiscoveryCertificate,
    queue: &GeneratedSectorLiveLeafQueueCertificate,
) -> Result<(), GeneratedFamilyFixedPointError> {
    if !Arc::ptr_eq(shared, discovery.row_span_arc())
        || !Arc::ptr_eq(shared, discovery.coverage().row_span_arc())
        || !Arc::ptr_eq(shared, queue.discovery().row_span_arc())
        || !discovery.payload_eq(queue.discovery())
    {
        return Err(GeneratedFamilyFixedPointError::ReplayMismatch {
            detail: "fixed-point material lost its exact shared row-span or queue binding",
        });
    }
    Ok(())
}

fn fixed_point_interruption_is_resource(
    interruption: &GeneratedFamilyFixedPointInterruption,
) -> bool {
    match interruption {
        GeneratedFamilyFixedPointInterruption::BaseDiscovery { error }
        | GeneratedFamilyFixedPointInterruption::GlobalComposition { error } => {
            discovery_error_is_resource(error)
        }
        GeneratedFamilyFixedPointInterruption::BaseLiveLeafQueue { error }
        | GeneratedFamilyFixedPointInterruption::LiveLeafQueue { error } => {
            queue_error_is_resource(error)
        }
        GeneratedFamilyFixedPointInterruption::ResidualAdaptiveSearch { error, .. } => {
            adaptive_error_is_resource(error)
        }
        GeneratedFamilyFixedPointInterruption::ResidualCandidateAuthentication {
            error, ..
        } => generated_when_bad_error_is_resource(error),
        GeneratedFamilyFixedPointInterruption::ResidualPointEvaluation { error, .. } => {
            when_bad_error_is_resource(error)
        }
    }
}

fn compute_fixed_point_stats(
    preparations: &[GeneratedFamilyFixedPointBasePreparation],
    rounds: &[GeneratedFamilyFixedPointRound],
    final_statuses: &[GeneratedFamilyFixedPointSectorStatus],
    limits: GeneratedFamilyFixedPointLimits,
) -> Result<GeneratedFamilyFixedPointStats, GeneratedFamilyFixedPointError> {
    let mut stats = GeneratedFamilyFixedPointStats {
        base_preparations: preparations.len(),
        rounds: rounds.len(),
        ..GeneratedFamilyFixedPointStats::default()
    };
    for preparation in preparations {
        add_residual_stats(&mut stats, &preparation.before)?;
        match &preparation.outcome {
            GeneratedFamilyFixedPointBasePreparationOutcome::Prepared {
                after,
                search_discovery,
                accepted_candidates,
                ..
            } => {
                stats.completed_base_preparations = fixed_point_checked_add(
                    "fixed-point completed base preparations",
                    stats.completed_base_preparations,
                    1,
                )?;
                stats.shared_row_span_material_reuses = fixed_point_checked_add(
                    "fixed-point shared row-span material reuses",
                    stats.shared_row_span_material_reuses,
                    2,
                )?;
                stats.accepted_candidate_references = fixed_point_checked_add(
                    "fixed-point accepted references",
                    stats.accepted_candidate_references,
                    accepted_candidates.len(),
                )?;
                stats.locator_references = fixed_point_checked_add(
                    "fixed-point locator references",
                    stats.locator_references,
                    accepted_candidates.len(),
                )?;
                let _ = search_discovery;
                add_residual_stats(&mut stats, after)?;
            }
            GeneratedFamilyFixedPointBasePreparationOutcome::ResourceLimited { .. } => {
                stats.resource_limited = fixed_point_checked_add(
                    "fixed-point resource interruptions",
                    stats.resource_limited,
                    1,
                )?;
            }
            GeneratedFamilyFixedPointBasePreparationOutcome::Failed { .. } => {
                stats.failed = fixed_point_checked_add("fixed-point failures", stats.failed, 1)?;
            }
        }
    }
    for round in rounds {
        stats.sector_attempts = fixed_point_checked_add(
            "fixed-point sector attempts",
            stats.sector_attempts,
            round.attempts.len(),
        )?;
        for attempt in &round.attempts {
            add_residual_stats(&mut stats, &attempt.before)?;
            stats.anchor_searches = fixed_point_checked_add(
                "fixed-point anchor searches",
                stats.anchor_searches,
                attempt.anchor_searches.len(),
            )?;
            for search in &attempt.anchor_searches {
                stats.anchor_origins = fixed_point_checked_add(
                    "fixed-point anchor origins",
                    stats.anchor_origins,
                    search.origins.len(),
                )?;
                stats.material_locator_references = fixed_point_checked_add(
                    "fixed-point material locator references",
                    stats.material_locator_references,
                    search.origins.len(),
                )?;
                stats.assignment_entries_referenced = fixed_point_checked_add(
                    "fixed-point assignment entries referenced",
                    stats.assignment_entries_referenced,
                    search.request_anchor.powers().len(),
                )?;
                stats.visited_candidates = fixed_point_checked_add(
                    "fixed-point visited candidates",
                    stats.visited_candidates,
                    search.visited.len(),
                )?;
                for visit in &search.visited {
                    let source = visit.outcome.compilation().source_authentication().stats();
                    stats.retained_visited_source_rows = fixed_point_checked_add(
                        "fixed-point retained visited source rows",
                        stats.retained_visited_source_rows,
                        source.retained_rows(),
                    )?;
                    stats.retained_visited_source_terms = fixed_point_checked_add(
                        "fixed-point retained visited source terms",
                        stats.retained_visited_source_terms,
                        source.retained_terms(),
                    )?;
                    stats.retained_visited_source_manifest_bytes = fixed_point_checked_add(
                        "fixed-point retained visited source manifest bytes",
                        stats.retained_visited_source_manifest_bytes,
                        source.source_manifest_bytes(),
                    )?;
                    let (binding_bytes, condition_terms, condition_bytes) = match &visit.outcome {
                        GeneratedResidualCandidateOutcome::Unsupported { compilation }
                        | GeneratedResidualCandidateOutcome::CertifiedNotCoveringRequestAnchor {
                            compilation,
                        }
                        | GeneratedResidualCandidateOutcome::CertifiedCoveredRequestAnchor {
                            compilation,
                        } => match compilation {
                            GeneratedWhenBadCompilation::Certified(certificate) => (
                                certificate.admissibility().binding().retained_bytes(),
                                certificate
                                    .admissibility()
                                    .stats()
                                    .retained_condition_terms(),
                                certificate
                                    .admissibility()
                                    .stats()
                                    .retained_condition_bytes(),
                            ),
                            GeneratedWhenBadCompilation::Unsupported(unsupported) => {
                                (unsupported.admissibility().binding().retained_bytes(), 0, 0)
                            }
                        },
                    };
                    stats.retained_visited_candidate_binding_bytes = fixed_point_checked_add(
                        "fixed-point retained visited candidate binding bytes",
                        stats.retained_visited_candidate_binding_bytes,
                        binding_bytes,
                    )?;
                    stats.retained_visited_condition_terms = fixed_point_checked_add(
                        "fixed-point retained visited condition terms",
                        stats.retained_visited_condition_terms,
                        condition_terms,
                    )?;
                    stats.retained_visited_condition_bytes = fixed_point_checked_add(
                        "fixed-point retained visited condition bytes",
                        stats.retained_visited_condition_bytes,
                        condition_bytes,
                    )?;
                }
            }
            stats.accepted_candidate_references = fixed_point_checked_add(
                "fixed-point accepted references",
                stats.accepted_candidate_references,
                attempt.newly_accepted_candidates.len(),
            )?;
            stats.locator_references = fixed_point_checked_add(
                "fixed-point locator references",
                stats.locator_references,
                attempt.newly_accepted_candidates.len(),
            )?;
            match &attempt.outcome {
                GeneratedFamilyFixedPointAttemptOutcome::Completed { after, .. }
                | GeneratedFamilyFixedPointAttemptOutcome::NoCandidateCoveredRequestAnchors {
                    after,
                }
                | GeneratedFamilyFixedPointAttemptOutcome::AnchorWitnessSearchExhaustedWithinConfiguredBounds {
                    after,
                } => add_residual_stats(&mut stats, after)?,
                GeneratedFamilyFixedPointAttemptOutcome::ResourceLimited { .. } => {
                    stats.resource_limited = fixed_point_checked_add(
                        "fixed-point resource interruptions",
                        stats.resource_limited,
                        1,
                    )?;
                }
                GeneratedFamilyFixedPointAttemptOutcome::Failed { .. } => {
                    stats.failed =
                        fixed_point_checked_add("fixed-point failures", stats.failed, 1)?;
                }
            }
        }
    }
    for final_status in final_statuses {
        stats.material_locator_references = fixed_point_checked_add(
            "fixed-point material locator references",
            stats.material_locator_references,
            1,
        )?;
        stats.accepted_candidate_references = fixed_point_checked_add(
            "fixed-point accepted references",
            stats.accepted_candidate_references,
            final_status.cumulative_accepted_candidates.len(),
        )?;
        if let Some(residual) = final_status_residual(&final_status.status) {
            add_residual_stats(&mut stats, residual)?;
        }
    }
    for (resource, requested, limit) in [
        (
            "fixed-point anchor searches",
            stats.anchor_searches,
            limits.max_retained_anchor_searches,
        ),
        (
            "fixed-point anchor origins",
            stats.anchor_origins,
            limits.max_retained_anchor_origins,
        ),
        (
            "fixed-point assignment entries referenced",
            stats.assignment_entries_referenced,
            limits.max_retained_assignment_entries,
        ),
        (
            "fixed-point visited candidates",
            stats.visited_candidates,
            limits.max_visited_candidates,
        ),
        (
            "fixed-point accepted references",
            stats.accepted_candidate_references,
            limits.max_accepted_candidate_references,
        ),
        (
            "fixed-point retained visited source rows",
            stats.retained_visited_source_rows,
            limits.max_retained_visited_source_rows,
        ),
        (
            "fixed-point retained visited source terms",
            stats.retained_visited_source_terms,
            limits.max_retained_visited_source_terms,
        ),
        (
            "fixed-point retained visited source manifest bytes",
            stats.retained_visited_source_manifest_bytes,
            limits.max_retained_visited_source_manifest_bytes,
        ),
        (
            "fixed-point retained visited candidate binding bytes",
            stats.retained_visited_candidate_binding_bytes,
            limits.max_retained_visited_candidate_binding_bytes,
        ),
        (
            "fixed-point retained visited condition terms",
            stats.retained_visited_condition_terms,
            limits.max_retained_visited_condition_terms,
        ),
        (
            "fixed-point retained visited condition bytes",
            stats.retained_visited_condition_bytes,
            limits.max_retained_visited_condition_bytes,
        ),
        (
            "fixed-point locator references",
            stats.locator_references,
            limits.max_locator_references,
        ),
        (
            "fixed-point material locator references",
            stats.material_locator_references,
            limits.max_retained_material_locators,
        ),
        (
            "fixed-point residual leaves",
            stats.residual_leaf_references,
            limits.max_retained_residual_leaves,
        ),
        (
            "fixed-point residual predicates",
            stats.residual_predicate_references,
            limits.max_retained_residual_predicates,
        ),
    ] {
        fixed_point_check_limit(resource, requested, limit)?;
    }
    Ok(stats)
}

fn add_residual_stats(
    stats: &mut GeneratedFamilyFixedPointStats,
    residual: &GeneratedFixedPointResidualSummary,
) -> Result<(), GeneratedFamilyFixedPointError> {
    stats.residual_leaf_references = fixed_point_checked_add(
        "fixed-point residual leaves",
        stats.residual_leaf_references,
        residual.leaves.len(),
    )?;
    stats.residual_predicate_references = fixed_point_checked_add(
        "fixed-point residual predicates",
        stats.residual_predicate_references,
        residual.predicate_instances,
    )?;
    Ok(())
}

fn final_status_residual(
    status: &GeneratedFamilyFixedPointFinalStatus,
) -> Option<&GeneratedFixedPointResidualSummary> {
    match status {
        GeneratedFamilyFixedPointFinalStatus::AnchorWitnessSearchExhaustedWithinConfiguredBounds {
            residual,
            ..
        }
        | GeneratedFamilyFixedPointFinalStatus::ExhaustedAtMaximumRounds { residual }
        | GeneratedFamilyFixedPointFinalStatus::StalledNoStrictResidualImprovement { residual }
        | GeneratedFamilyFixedPointFinalStatus::NotSelectedByPolicyBound { residual } => {
            Some(residual)
        }
        GeneratedFamilyFixedPointFinalStatus::CoveredByGeneratedRules
        | GeneratedFamilyFixedPointFinalStatus::ResourceLimited { .. }
        | GeneratedFamilyFixedPointFinalStatus::Failed { .. } => None,
    }
}

fn fixed_point_checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedFamilyFixedPointError> {
    left.checked_add(right)
        .ok_or(GeneratedFamilyFixedPointError::ResourceCountOverflow { resource })
}

fn fixed_point_checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedFamilyFixedPointError> {
    left.checked_mul(right)
        .ok_or(GeneratedFamilyFixedPointError::ResourceCountOverflow { resource })
}

fn fixed_point_check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedFamilyFixedPointError> {
    if requested <= limit {
        Ok(())
    } else {
        Err(GeneratedFamilyFixedPointError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    }
}

fn fixed_point_certificate_payload_eq(
    left: &GeneratedFamilyFixedPointCertificate,
    right: &GeneratedFamilyFixedPointCertificate,
) -> bool {
    left.schema == right.schema
        && left.family_fingerprint == right.family_fingerprint
        && left.context_fingerprint == right.context_fingerprint
        && left.base.family_fingerprint() == right.base.family_fingerprint()
        && left.base.context_fingerprint() == right.base.context_fingerprint()
        && left.base.config() == right.base.config()
        && left.base.limits() == right.base.limits()
        && left.base.ordering() == right.base.ordering()
        && left.base.solve_order() == right.base.solve_order()
        && left.config == right.config
        && left.limits == right.limits
        && left.stats == right.stats
        && fixed_point_preparations_payload_eq(&left.base_preparations, &right.base_preparations)
        && fixed_point_rounds_payload_eq(&left.rounds, &right.rounds)
        && fixed_point_final_statuses_payload_eq(&left.final_statuses, &right.final_statuses)
}

fn fixed_point_preparations_payload_eq(
    left: &[GeneratedFamilyFixedPointBasePreparation],
    right: &[GeneratedFamilyFixedPointBasePreparation],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.ordinal == right.ordinal
                && left.sector == right.sector
                && left.solve_ordinal == right.solve_ordinal
                && left.input_material == right.input_material
                && left.before == right.before
                && fixed_point_preparation_outcome_payload_eq(&left.outcome, &right.outcome)
        })
}

fn fixed_point_preparation_outcome_payload_eq(
    left: &GeneratedFamilyFixedPointBasePreparationOutcome,
    right: &GeneratedFamilyFixedPointBasePreparationOutcome,
) -> bool {
    match (left, right) {
        (
            GeneratedFamilyFixedPointBasePreparationOutcome::Prepared {
                search_discovery: ls,
                after: la,
                discovery: ld,
                live_leaf_queue: lq,
                accepted_candidates: lc,
            },
            GeneratedFamilyFixedPointBasePreparationOutcome::Prepared {
                search_discovery: rs,
                after: ra,
                discovery: rd,
                live_leaf_queue: rq,
                accepted_candidates: rc,
            },
        ) => ls.payload_eq(rs) && la == ra && ld.payload_eq(rd) && lq.payload_eq(rq) && lc == rc,
        (
            GeneratedFamilyFixedPointBasePreparationOutcome::ResourceLimited {
                interruption: li,
                completed_discovery: ld,
            },
            GeneratedFamilyFixedPointBasePreparationOutcome::ResourceLimited {
                interruption: ri,
                completed_discovery: rd,
            },
        )
        | (
            GeneratedFamilyFixedPointBasePreparationOutcome::Failed {
                interruption: li,
                completed_discovery: ld,
            },
            GeneratedFamilyFixedPointBasePreparationOutcome::Failed {
                interruption: ri,
                completed_discovery: rd,
            },
        ) => li == ri && optional_fixed_point_discovery_payload_eq(ld.as_ref(), rd.as_ref()),
        _ => false,
    }
}

fn fixed_point_rounds_payload_eq(
    left: &[GeneratedFamilyFixedPointRound],
    right: &[GeneratedFamilyFixedPointRound],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.ordinal == right.ordinal
                && left.attempts.len() == right.attempts.len()
                && left
                    .attempts
                    .iter()
                    .zip(right.attempts.iter())
                    .all(|(left, right)| {
                        left.ordinal == right.ordinal
                            && left.sector == right.sector
                            && left.solve_ordinal == right.solve_ordinal
                            && left.input_material == right.input_material
                            && left.before == right.before
                            && fixed_point_searches_payload_eq(
                                &left.anchor_searches,
                                &right.anchor_searches,
                            )
                            && left.newly_accepted_candidates == right.newly_accepted_candidates
                            && fixed_point_attempt_outcome_payload_eq(&left.outcome, &right.outcome)
                    })
        })
}

fn fixed_point_searches_payload_eq(
    left: &[GeneratedResidualAnchorSearch],
    right: &[GeneratedResidualAnchorSearch],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.request_anchor == right.request_anchor
                && left.requested_local_depth == right.requested_local_depth
                && left.origins == right.origins
                && left.selected_visit_ordinal == right.selected_visit_ordinal
                && left.visited.len() == right.visited.len()
                && left
                    .visited
                    .iter()
                    .zip(right.visited.iter())
                    .all(|(left, right)| {
                        left.locator == right.locator
                            && fixed_point_candidate_outcome_payload_eq(
                                &left.outcome,
                                &right.outcome,
                            )
                    })
        })
}

fn fixed_point_candidate_outcome_payload_eq(
    left: &GeneratedResidualCandidateOutcome,
    right: &GeneratedResidualCandidateOutcome,
) -> bool {
    matches!(
        (left, right),
        (
            GeneratedResidualCandidateOutcome::Unsupported { .. },
            GeneratedResidualCandidateOutcome::Unsupported { .. }
        ) | (
            GeneratedResidualCandidateOutcome::CertifiedNotCoveringRequestAnchor { .. },
            GeneratedResidualCandidateOutcome::CertifiedNotCoveringRequestAnchor { .. }
        ) | (
            GeneratedResidualCandidateOutcome::CertifiedCoveredRequestAnchor { .. },
            GeneratedResidualCandidateOutcome::CertifiedCoveredRequestAnchor { .. }
        )
    ) && left.compilation().payload_eq(right.compilation())
}

fn fixed_point_attempt_outcome_payload_eq(
    left: &GeneratedFamilyFixedPointAttemptOutcome,
    right: &GeneratedFamilyFixedPointAttemptOutcome,
) -> bool {
    match (left, right) {
        (
            GeneratedFamilyFixedPointAttemptOutcome::Completed {
                after: la,
                strict_improvement: li,
                discovery: ld,
                live_leaf_queue: lq,
            },
            GeneratedFamilyFixedPointAttemptOutcome::Completed {
                after: ra,
                strict_improvement: ri,
                discovery: rd,
                live_leaf_queue: rq,
            },
        ) => la == ra && li == ri && ld.payload_eq(rd) && lq.payload_eq(rq),
        (
            GeneratedFamilyFixedPointAttemptOutcome::NoCandidateCoveredRequestAnchors {
                after: left,
            },
            GeneratedFamilyFixedPointAttemptOutcome::NoCandidateCoveredRequestAnchors {
                after: right,
            },
        )
        | (
            GeneratedFamilyFixedPointAttemptOutcome::AnchorWitnessSearchExhaustedWithinConfiguredBounds {
                after: left,
            },
            GeneratedFamilyFixedPointAttemptOutcome::AnchorWitnessSearchExhaustedWithinConfiguredBounds {
                after: right,
            },
        ) => left == right,
        (
            GeneratedFamilyFixedPointAttemptOutcome::ResourceLimited {
                interruption: li,
                completed_discovery: ld,
            },
            GeneratedFamilyFixedPointAttemptOutcome::ResourceLimited {
                interruption: ri,
                completed_discovery: rd,
            },
        )
        | (
            GeneratedFamilyFixedPointAttemptOutcome::Failed {
                interruption: li,
                completed_discovery: ld,
            },
            GeneratedFamilyFixedPointAttemptOutcome::Failed {
                interruption: ri,
                completed_discovery: rd,
            },
        ) => li == ri && optional_fixed_point_discovery_payload_eq(ld.as_ref(), rd.as_ref()),
        _ => false,
    }
}

fn fixed_point_final_statuses_payload_eq(
    left: &[GeneratedFamilyFixedPointSectorStatus],
    right: &[GeneratedFamilyFixedPointSectorStatus],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.sector == right.sector
                && left.solve_ordinal == right.solve_ordinal
                && left.latest_material == right.latest_material
                && left.cumulative_accepted_candidates == right.cumulative_accepted_candidates
                && left.status == right.status
        })
}

fn optional_fixed_point_discovery_payload_eq(
    left: Option<&GeneratedSectorDiscoveryCertificate>,
    right: Option<&GeneratedSectorDiscoveryCertificate>,
) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.payload_eq(right),
        (None, None) => true,
        _ => false,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedFamilyFixedPointError {
    SchemaMismatch,
    WrongFamily,
    WrongContext,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    InvalidConfig {
        detail: &'static str,
    },
    ReplayMismatch {
        detail: &'static str,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    Base(GeneratedFamilyRuleSystemError),
    Sector(SectorFoundationError),
    Coverage(ParametricSectorCoverageError),
    Relation(crate::ParametricRelationError),
}

impl fmt::Display for GeneratedFamilyFixedPointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter.write_str("family fixed-point schema mismatch"),
            Self::WrongFamily => formatter.write_str("family fixed-point family mismatch"),
            Self::WrongContext => formatter.write_str("family fixed-point context mismatch"),
            Self::WrongArity { expected, actual } => {
                write!(
                    formatter,
                    "family fixed-point arity is {actual}, expected {expected}"
                )
            }
            Self::InvalidConfig { detail } => {
                write!(formatter, "invalid fixed-point config: {detail}")
            }
            Self::ReplayMismatch { detail } => {
                write!(formatter, "fixed-point replay mismatch: {detail}")
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
                "{resource} needs {requested} units, exceeding configured limit {limit}"
            ),
            Self::Base(error) => error.fmt(formatter),
            Self::Sector(error) => error.fmt(formatter),
            Self::Coverage(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratedFamilyFixedPointError {}

impl From<GeneratedFamilyRuleSystemError> for GeneratedFamilyFixedPointError {
    fn from(value: GeneratedFamilyRuleSystemError) -> Self {
        Self::Base(value)
    }
}

impl From<SectorFoundationError> for GeneratedFamilyFixedPointError {
    fn from(value: SectorFoundationError) -> Self {
        Self::Sector(value)
    }
}

impl From<ParametricSectorCoverageError> for GeneratedFamilyFixedPointError {
    fn from(value: ParametricSectorCoverageError) -> Self {
        Self::Coverage(value)
    }
}

impl From<crate::ParametricRelationError> for GeneratedFamilyFixedPointError {
    fn from(value: crate::ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}
