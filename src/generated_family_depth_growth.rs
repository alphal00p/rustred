//! Replayable, bounded depth growth above a generated family rule system.
//!
//! LiteRed's `SolvejSector` grows the `preparepoints` stencil when the
//! current symbolic rule set leaves cases behind.  This module implements the
//! corresponding topology-independent scheduling step for RustRed.  It does
//! not infer masters and it does not claim that a smaller residual census is
//! a proof of closure.  Every round compiles fresh generated IBP/LI material
//! only for selected residual sectors, in the family inventory's certified
//! subsector-first order, while reusing the family's one immutable generated
//! row span.
//!
//! Solved-subsector substitution back into supersector elimination is a
//! separate future layer.  The concrete provider in this module nevertheless
//! installs the latest successful material from all sectors together, so
//! ordinary recursive reduction can consume subsector rules before returning
//! to a supersector target.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::generated_family_rule_system::{discovery_error_is_resource, queue_error_is_resource};
use crate::reduction_engine::{ConcreteRuleDecision, ConcreteRuleProvider};
use crate::{
    CertifiedZeroSectorRuleProvider, CertifiedZeroSectorRuleProviderError, ConcreteIntegralKey,
    GeneratedFamilyRuleSystemCertificate, GeneratedFamilyRuleSystemError,
    GeneratedFamilyRuleSystemProviderLimits, GeneratedFamilySectorFailure,
    GeneratedFamilySectorResource, GeneratedFamilySectorStatus,
    GeneratedSectorConditionalRuleProvider, GeneratedSectorConditionalRuleProviderError,
    GeneratedSectorDiscoveryCertificate, GeneratedSectorDiscoveryCompiler,
    GeneratedSectorLiveLeafOutcome, GeneratedSectorLiveLeafQueueCertificate,
    GeneratedSectorLiveLeafQueueCompiler, GeneratedSymbolicRowSpanCertificate, IntegralFamily,
    MasterPolicyError, MasterPolicyProvider, MasterPolicyTerminal, ParametricCoefficientContext,
    ParametricSectorLeafDisposition, ParametricSectorRuleProvider,
    ParametricSectorRuleProviderError, SectorMask, SymbolicPolynomialPredicate,
};

pub const GENERATED_FAMILY_DEPTH_GROWTH_V1_SCHEMA: &str =
    "rustred.generated-family-depth-growth.v1";
pub const GENERATED_FAMILY_DEPTH_GROWTH_PROVIDER_V1_SCHEMA: &str =
    "rustred.generated-family-depth-growth-provider.v1";

/// Certificate-bound, topology-independent sector scheduling policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratedFamilyDepthGrowthSelectionPolicy {
    /// Visit every residual sector at a depth, in certified subsector-first
    /// order.
    AllResidualSubsectorFirst,
    /// Visit only the first `max_sectors_per_round` residual sectors at each
    /// depth.  This is a checked resource policy, not a topology selector.
    ResidualSubsectorFirstPrefix { max_sectors_per_round: usize },
}

impl Default for GeneratedFamilyDepthGrowthSelectionPolicy {
    fn default() -> Self {
        Self::AllResidualSubsectorFirst
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedFamilyDepthGrowthConfig {
    /// The depth already retained by the base family certificate.
    pub initial_depth: usize,
    /// Inclusive maximum cumulative diamond depth.
    pub maximum_depth: usize,
    pub selection: GeneratedFamilyDepthGrowthSelectionPolicy,
    /// LiteRed-faithful default is `false`: a completed plateau remains in
    /// the queue and depth continues to grow through `maximum_depth`.
    pub stop_on_no_strict_improvement: bool,
}

impl Default for GeneratedFamilyDepthGrowthConfig {
    fn default() -> Self {
        Self {
            initial_depth: 0,
            maximum_depth: 2,
            selection: GeneratedFamilyDepthGrowthSelectionPolicy::AllResidualSubsectorFirst,
            stop_on_no_strict_improvement: false,
        }
    }
}

/// Outer transcript limits.  Nested algebra/search limits are inherited
/// exactly from the base family certificate; only adaptive depth changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedFamilyDepthGrowthLimits {
    pub max_rounds: usize,
    pub max_sector_attempts: usize,
    pub max_final_sector_statuses: usize,
    pub max_retained_residual_leaves: usize,
    pub max_retained_residual_predicates: usize,
}

impl Default for GeneratedFamilyDepthGrowthLimits {
    fn default() -> Self {
        Self {
            max_rounds: 1_000_000,
            max_sector_attempts: 16_000_000,
            max_final_sector_statuses: 1_000_000,
            max_retained_residual_leaves: 32_000_000,
            max_retained_residual_predicates: 256_000_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedResidualLeafKind {
    Uncovered,
    Unsupported { candidate_ordinals: Box<[usize]> },
}

/// Exact structural identity of one residual cell.  This is retained so a
/// replay can distinguish an unchanged residual set from merely equal counts.
/// It is not used as an algebraic subset proof between different partitions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedResidualLeafIdentity {
    predicates: Box<[SymbolicPolynomialPredicate]>,
    kind: GeneratedResidualLeafKind,
}

impl GeneratedResidualLeafIdentity {
    pub fn predicates(&self) -> &[SymbolicPolynomialPredicate] {
        &self.predicates
    }
    pub const fn kind(&self) -> &GeneratedResidualLeafKind {
        &self.kind
    }
}

/// Explicit lexicographic heuristic used to decide whether another round may
/// be worthwhile.  A decrease is called *strict residual improvement*, never
/// convergence or a proof of set inclusion.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedResidualMeasure {
    unsupported_leaves: usize,
    uncovered_leaves: usize,
    preserved_without_assignment_leaves: usize,
    preserved_index_boundary_leaves: usize,
    empty_partial_systems: usize,
    residual_unresolved_predicates: usize,
    root_residual_leaves: usize,
    certified_partial_reeliminations: usize,
}

impl GeneratedResidualMeasure {
    pub const fn unsupported_leaves(self) -> usize {
        self.unsupported_leaves
    }
    pub const fn uncovered_leaves(self) -> usize {
        self.uncovered_leaves
    }
    pub const fn preserved_without_assignment_leaves(self) -> usize {
        self.preserved_without_assignment_leaves
    }
    pub const fn preserved_index_boundary_leaves(self) -> usize {
        self.preserved_index_boundary_leaves
    }
    pub const fn empty_partial_systems(self) -> usize {
        self.empty_partial_systems
    }
    pub const fn residual_unresolved_predicates(self) -> usize {
        self.residual_unresolved_predicates
    }
    pub const fn root_residual_leaves(self) -> usize {
        self.root_residual_leaves
    }
    pub const fn certified_partial_reeliminations(self) -> usize {
        self.certified_partial_reeliminations
    }

    pub const fn is_empty(self) -> bool {
        self.root_residual_leaves == 0
    }

    fn key(self) -> [usize; 8] {
        [
            self.unsupported_leaves,
            self.uncovered_leaves,
            self.preserved_without_assignment_leaves,
            self.preserved_index_boundary_leaves,
            self.empty_partial_systems,
            self.residual_unresolved_predicates,
            self.root_residual_leaves,
            self.certified_partial_reeliminations,
        ]
    }

    pub fn is_strict_improvement_over(self, previous: Self) -> bool {
        self.key() < previous.key()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedSectorResidualSummary {
    leaves: Box<[GeneratedResidualLeafIdentity]>,
    measure: GeneratedResidualMeasure,
}

impl GeneratedSectorResidualSummary {
    pub fn leaves(&self) -> &[GeneratedResidualLeafIdentity] {
        &self.leaves
    }
    pub const fn measure(&self) -> GeneratedResidualMeasure {
        self.measure
    }
    pub const fn is_empty(&self) -> bool {
        self.measure.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratedFamilyDepthGrowthStage {
    Discovery,
    LiveLeafQueue,
}

#[derive(Clone, Debug)]
pub enum GeneratedFamilyDepthGrowthAttemptOutcome {
    /// The configured lexicographic measure decreased strictly.  This is not
    /// a closure proof.
    StrictResidualImprovement {
        exact_residual_identity_unchanged: bool,
        after: GeneratedSectorResidualSummary,
        discovery: GeneratedSectorDiscoveryCertificate,
        live_leaf_queue: GeneratedSectorLiveLeafQueueCertificate,
    },
    /// The deeper search completed, but its residual measure did not decrease.
    StalledNoStrictResidualImprovement {
        exact_residual_identity_unchanged: bool,
        after: GeneratedSectorResidualSummary,
        discovery: GeneratedSectorDiscoveryCertificate,
        live_leaf_queue: GeneratedSectorLiveLeafQueueCertificate,
    },
    ResourceLimited {
        stage: GeneratedFamilyDepthGrowthStage,
        completed_discovery: Option<GeneratedSectorDiscoveryCertificate>,
        resource: GeneratedFamilySectorResource,
    },
    Failed {
        stage: GeneratedFamilyDepthGrowthStage,
        completed_discovery: Option<GeneratedSectorDiscoveryCertificate>,
        failure: GeneratedFamilySectorFailure,
    },
}

impl GeneratedFamilyDepthGrowthAttemptOutcome {
    pub const fn successful_material(
        &self,
    ) -> Option<(
        &GeneratedSectorDiscoveryCertificate,
        &GeneratedSectorLiveLeafQueueCertificate,
    )> {
        match self {
            Self::StrictResidualImprovement {
                discovery,
                live_leaf_queue,
                ..
            }
            | Self::StalledNoStrictResidualImprovement {
                discovery,
                live_leaf_queue,
                ..
            } => Some((discovery, live_leaf_queue)),
            Self::ResourceLimited { .. } | Self::Failed { .. } => None,
        }
    }

    pub const fn residual_after(&self) -> Option<&GeneratedSectorResidualSummary> {
        match self {
            Self::StrictResidualImprovement { after, .. }
            | Self::StalledNoStrictResidualImprovement { after, .. } => Some(after),
            Self::ResourceLimited { .. } | Self::Failed { .. } => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GeneratedFamilyDepthGrowthSectorAttempt {
    ordinal: usize,
    sector: SectorMask,
    solve_ordinal: usize,
    previous_successful_depth: usize,
    before: GeneratedSectorResidualSummary,
    outcome: GeneratedFamilyDepthGrowthAttemptOutcome,
}

impl GeneratedFamilyDepthGrowthSectorAttempt {
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }
    pub const fn sector(&self) -> &SectorMask {
        &self.sector
    }
    pub const fn solve_ordinal(&self) -> usize {
        self.solve_ordinal
    }
    pub const fn previous_successful_depth(&self) -> usize {
        self.previous_successful_depth
    }
    pub const fn before(&self) -> &GeneratedSectorResidualSummary {
        &self.before
    }
    pub const fn outcome(&self) -> &GeneratedFamilyDepthGrowthAttemptOutcome {
        &self.outcome
    }
}

#[derive(Clone, Debug)]
pub struct GeneratedFamilyDepthGrowthRound {
    ordinal: usize,
    depth: usize,
    attempts: Box<[GeneratedFamilyDepthGrowthSectorAttempt]>,
}

impl GeneratedFamilyDepthGrowthRound {
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }
    pub const fn depth(&self) -> usize {
        self.depth
    }
    pub fn attempts(&self) -> &[GeneratedFamilyDepthGrowthSectorAttempt] {
        &self.attempts
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedFamilyDepthGrowthFinalStatus {
    CoveredByGeneratedRules {
        depth: usize,
    },
    ExhaustedAtMaxDepth {
        latest_successful_depth: usize,
        residual: GeneratedSectorResidualSummary,
    },
    StalledNoStrictResidualImprovement {
        depth: usize,
        residual: GeneratedSectorResidualSummary,
    },
    NotSelectedByPolicyBound {
        latest_successful_depth: usize,
        residual: GeneratedSectorResidualSummary,
    },
    ResourceLimited {
        attempted_depth: usize,
        stage: GeneratedFamilyDepthGrowthStage,
        resource: GeneratedFamilySectorResource,
    },
    Failed {
        attempted_depth: usize,
        stage: GeneratedFamilyDepthGrowthStage,
        failure: GeneratedFamilySectorFailure,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedFamilyDepthGrowthSectorStatus {
    sector: SectorMask,
    solve_ordinal: usize,
    status: GeneratedFamilyDepthGrowthFinalStatus,
}

impl GeneratedFamilyDepthGrowthSectorStatus {
    pub const fn sector(&self) -> &SectorMask {
        &self.sector
    }
    pub const fn solve_ordinal(&self) -> usize {
        self.solve_ordinal
    }
    pub const fn status(&self) -> &GeneratedFamilyDepthGrowthFinalStatus {
        &self.status
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedFamilyDepthGrowthStats {
    rounds: usize,
    sector_attempts: usize,
    strict_improvements: usize,
    stalled: usize,
    resource_limited: usize,
    failed: usize,
    shared_row_span_sector_reuses: usize,
    retained_residual_leaves: usize,
    retained_residual_predicates: usize,
}

impl GeneratedFamilyDepthGrowthStats {
    pub const fn rounds(self) -> usize {
        self.rounds
    }
    pub const fn sector_attempts(self) -> usize {
        self.sector_attempts
    }
    pub const fn strict_improvements(self) -> usize {
        self.strict_improvements
    }
    pub const fn stalled(self) -> usize {
        self.stalled
    }
    pub const fn resource_limited(self) -> usize {
        self.resource_limited
    }
    pub const fn failed(self) -> usize {
        self.failed
    }
    pub const fn shared_row_span_sector_reuses(self) -> usize {
        self.shared_row_span_sector_reuses
    }
    pub const fn retained_residual_leaves(self) -> usize {
        self.retained_residual_leaves
    }
    pub const fn retained_residual_predicates(self) -> usize {
        self.retained_residual_predicates
    }
}

#[derive(Clone, Debug)]
pub struct GeneratedFamilyDepthGrowthCertificate {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    base: GeneratedFamilyRuleSystemCertificate,
    config: GeneratedFamilyDepthGrowthConfig,
    limits: GeneratedFamilyDepthGrowthLimits,
    rounds: Box<[GeneratedFamilyDepthGrowthRound]>,
    final_statuses: Box<[GeneratedFamilyDepthGrowthSectorStatus]>,
    stats: GeneratedFamilyDepthGrowthStats,
}

pub struct GeneratedFamilyDepthGrowthMaterialRef<'a> {
    sector: &'a SectorMask,
    depth: usize,
    discovery: &'a GeneratedSectorDiscoveryCertificate,
    live_leaf_queue: &'a GeneratedSectorLiveLeafQueueCertificate,
}

impl<'a> GeneratedFamilyDepthGrowthMaterialRef<'a> {
    pub const fn sector(&self) -> &SectorMask {
        self.sector
    }
    pub const fn depth(&self) -> usize {
        self.depth
    }
    pub const fn discovery(&self) -> &GeneratedSectorDiscoveryCertificate {
        self.discovery
    }
    pub const fn live_leaf_queue(&self) -> &GeneratedSectorLiveLeafQueueCertificate {
        self.live_leaf_queue
    }
}

impl GeneratedFamilyDepthGrowthCertificate {
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
    pub const fn config(&self) -> GeneratedFamilyDepthGrowthConfig {
        self.config
    }
    pub const fn limits(&self) -> GeneratedFamilyDepthGrowthLimits {
        self.limits
    }
    pub fn rounds(&self) -> &[GeneratedFamilyDepthGrowthRound] {
        &self.rounds
    }
    pub fn final_statuses(&self) -> &[GeneratedFamilyDepthGrowthSectorStatus] {
        &self.final_statuses
    }
    pub const fn stats(&self) -> GeneratedFamilyDepthGrowthStats {
        self.stats
    }

    pub fn final_status(
        &self,
        sector: &SectorMask,
    ) -> Option<&GeneratedFamilyDepthGrowthFinalStatus> {
        self.final_statuses
            .iter()
            .find(|entry| entry.sector() == sector)
            .map(GeneratedFamilyDepthGrowthSectorStatus::status)
    }

    /// Latest complete discovery/queue pair for each generated sector, in the
    /// base inventory's certified subsector-first order.
    pub fn latest_successful_materials(&self) -> Vec<GeneratedFamilyDepthGrowthMaterialRef<'_>> {
        let mut slots = std::iter::repeat_with(|| None)
            .take(self.base.solve_order().len())
            .collect::<Vec<Option<GeneratedFamilyDepthGrowthMaterialRef<'_>>>>();
        for (solve_ordinal, sector) in self.base.solve_order().iter().enumerate() {
            let Some(GeneratedFamilySectorStatus::Unresolved {
                discovery,
                live_leaf_queue,
                ..
            }) = self.base.status(sector)
            else {
                continue;
            };
            slots[solve_ordinal] = Some(GeneratedFamilyDepthGrowthMaterialRef {
                sector,
                depth: self.config.initial_depth,
                discovery,
                live_leaf_queue,
            });
        }
        for round in &self.rounds {
            for attempt in &round.attempts {
                let Some(Some(material)) = slots.get_mut(attempt.solve_ordinal) else {
                    // Malformed transcripts are rejected by replay; this
                    // public view remains fail-closed by omitting no entry and
                    // applying no out-of-range update.
                    continue;
                };
                if material.sector != &attempt.sector {
                    continue;
                }
                if let Some((discovery, live_leaf_queue)) = attempt.outcome.successful_material() {
                    material.depth = round.depth;
                    material.discovery = discovery;
                    material.live_leaf_queue = live_leaf_queue;
                }
            }
        }
        slots.into_iter().flatten().collect()
    }

    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedFamilyDepthGrowthError> {
        validate_scope(family, context, &self.base)?;
        if self.schema != GENERATED_FAMILY_DEPTH_GROWTH_V1_SCHEMA {
            return Err(GeneratedFamilyDepthGrowthError::SchemaMismatch);
        }
        if self.family_fingerprint.as_ref() != family.fingerprint() {
            return Err(GeneratedFamilyDepthGrowthError::WrongFamily);
        }
        if self.context_fingerprint.as_ref() != context.fingerprint() {
            return Err(GeneratedFamilyDepthGrowthError::WrongContext);
        }
        validate_config(self.config, self.limits)?;
        self.base.replay(family, context)?;
        let rebuilt = GeneratedFamilyDepthGrowthCompiler::compile_with_replayed_base(
            family,
            context,
            self.base.clone(),
            self.config,
            self.limits,
        )?;
        if certificate_payload_eq(self, &rebuilt) {
            Ok(())
        } else {
            Err(GeneratedFamilyDepthGrowthError::ReplayMismatch {
                detail: "depth-growth transcript differs",
            })
        }
    }
}

pub struct GeneratedFamilyDepthGrowthCompiler;

impl GeneratedFamilyDepthGrowthCompiler {
    pub fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        base: GeneratedFamilyRuleSystemCertificate,
        config: GeneratedFamilyDepthGrowthConfig,
        limits: GeneratedFamilyDepthGrowthLimits,
    ) -> Result<GeneratedFamilyDepthGrowthCertificate, GeneratedFamilyDepthGrowthError> {
        validate_scope(family, context, &base)?;
        validate_config(config, limits)?;
        base.replay(family, context)?;
        Self::compile_with_replayed_base(family, context, base, config, limits)
    }

    /// Compile depth growth and immediately install explicit selected masters
    /// without replaying the newly constructed depth searches a second time.
    ///
    /// This fused path is safe because the certificate fields are private and
    /// every nested discovery/queue compiler has already replayed its fresh
    /// proof against the family-shared row span.  A standalone certificate
    /// passed later to [`GeneratedFamilyDepthGrowthProvider::try_with_selected`]
    /// still receives full independent replay.
    #[allow(clippy::too_many_arguments)]
    pub fn compile_with_selected_provider<'family>(
        family: &'family IntegralFamily,
        context: &'family ParametricCoefficientContext,
        base: GeneratedFamilyRuleSystemCertificate,
        config: GeneratedFamilyDepthGrowthConfig,
        depth_limits: GeneratedFamilyDepthGrowthLimits,
        selected: impl IntoIterator<Item = ConcreteIntegralKey>,
        provider_limits: GeneratedFamilyRuleSystemProviderLimits,
    ) -> Result<GeneratedFamilyDepthGrowthProvider<'family>, GeneratedFamilyDepthGrowthProviderError>
    {
        let certificate = Self::compile(family, context, base, config, depth_limits)?;
        GeneratedFamilyDepthGrowthProvider::try_with_terminals_from_compiler(
            family,
            context,
            certificate,
            selected
                .into_iter()
                .map(|key| (key, MasterPolicyTerminal::Selected)),
            provider_limits,
        )
    }

    fn compile_with_replayed_base(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        base: GeneratedFamilyRuleSystemCertificate,
        config: GeneratedFamilyDepthGrowthConfig,
        limits: GeneratedFamilyDepthGrowthLimits,
    ) -> Result<GeneratedFamilyDepthGrowthCertificate, GeneratedFamilyDepthGrowthError> {
        validate_scope(family, context, &base)?;
        validate_config(config, limits)?;

        let row_span = base.row_span_arc().cloned();
        let mut working = Vec::new();
        for (solve_ordinal, sector) in base.solve_order().iter().enumerate() {
            let Some(status) = base.status(sector) else {
                return Err(GeneratedFamilyDepthGrowthError::ReplayMismatch {
                    detail: "base solve-order sector has no status",
                });
            };
            let GeneratedFamilySectorStatus::Unresolved {
                discovery,
                live_leaf_queue,
                ..
            } = status
            else {
                // Base interruptions remain fully represented by the base
                // certificate.  There is no complete material to deepen.
                continue;
            };
            if discovery.limits().adaptive.max_search_depth != config.initial_depth {
                return Err(GeneratedFamilyDepthGrowthError::InitialDepthMismatch {
                    sector: sector.clone(),
                    expected: config.initial_depth,
                    actual: discovery.limits().adaptive.max_search_depth,
                });
            }
            validate_shared_material(row_span.as_ref(), discovery, live_leaf_queue)?;
            let residual = residual_summary(discovery, live_leaf_queue, limits)?;
            let stop = if residual.is_empty() {
                WorkingStop::Covered
            } else {
                WorkingStop::Active
            };
            working.push(WorkingSector {
                sector: sector.clone(),
                solve_ordinal,
                latest_depth: config.initial_depth,
                residual,
                stop,
            });
        }
        if !working.is_empty() && row_span.is_none() {
            return Err(GeneratedFamilyDepthGrowthError::ReplayMismatch {
                detail: "base generated sectors have no shared row span",
            });
        }

        let mut rounds = Vec::new();
        let mut aggregate_attempts = 0usize;
        let first_growth_depth = if config.initial_depth < config.maximum_depth {
            Some(config.initial_depth.checked_add(1).ok_or(
                GeneratedFamilyDepthGrowthError::ResourceCountOverflow {
                    resource: "depth-growth first depth",
                },
            )?)
        } else {
            None
        };
        for depth in first_growth_depth
            .filter(|_| config.initial_depth < config.maximum_depth)
            .into_iter()
            .flat_map(|first| first..=config.maximum_depth)
        {
            let active = working
                .iter()
                .enumerate()
                .filter_map(|(position, sector)| {
                    matches!(sector.stop, WorkingStop::Active).then_some(position)
                })
                .collect::<Vec<_>>();
            let selected = select_active(&active, config.selection)?;
            if selected.is_empty() {
                break;
            }
            let requested_rounds = checked_add("depth-growth rounds", rounds.len(), 1)?;
            check_limit("depth-growth rounds", requested_rounds, limits.max_rounds)?;
            let requested_attempts = aggregate_attempts.checked_add(selected.len()).ok_or(
                GeneratedFamilyDepthGrowthError::ResourceCountOverflow {
                    resource: "depth-growth sector attempts",
                },
            )?;
            check_limit(
                "depth-growth sector attempts",
                requested_attempts,
                limits.max_sector_attempts,
            )?;

            let mut attempts = Vec::with_capacity(selected.len());
            for position in selected {
                let state = &mut working[position];
                let before = state.residual.clone();
                let previous_successful_depth = state.latest_depth;
                let mut discovery_limits = base.limits().discovery;
                discovery_limits.adaptive.max_search_depth = depth;
                let shared =
                    row_span
                        .clone()
                        .ok_or(GeneratedFamilyDepthGrowthError::ReplayMismatch {
                            detail: "selected residual sector has no shared row span",
                        })?;
                let outcome = match GeneratedSectorDiscoveryCompiler::compile_with_replayed_row_span(
                    family,
                    context,
                    state.sector.clone(),
                    base.ordering(),
                    shared.clone(),
                    discovery_limits,
                ) {
                    Ok(discovery) => {
                        validate_shared_discovery(&shared, &discovery)?;
                        match GeneratedSectorLiveLeafQueueCompiler::compile_with_replayed_row_span(
                            family,
                            context,
                            &discovery,
                            shared,
                            base.limits().live_leaf_queue,
                        ) {
                            Ok(queue) => {
                                validate_shared_material(row_span.as_ref(), &discovery, &queue)?;
                                let after = residual_summary(&discovery, &queue, limits)?;
                                let exact_residual_identity_unchanged =
                                    before.leaves == after.leaves;
                                state.latest_depth = depth;
                                state.residual = after.clone();
                                if after.is_empty() {
                                    state.stop = WorkingStop::Covered;
                                } else if after.measure.is_strict_improvement_over(before.measure) {
                                    state.stop = WorkingStop::Active;
                                } else if config.stop_on_no_strict_improvement {
                                    state.stop = WorkingStop::Stalled { depth };
                                } else {
                                    state.stop = WorkingStop::Active;
                                }
                                if !after.measure.is_strict_improvement_over(before.measure) {
                                    GeneratedFamilyDepthGrowthAttemptOutcome::StalledNoStrictResidualImprovement {
                                        exact_residual_identity_unchanged,
                                        after,
                                        discovery,
                                        live_leaf_queue: queue,
                                    }
                                } else {
                                    GeneratedFamilyDepthGrowthAttemptOutcome::StrictResidualImprovement {
                                        exact_residual_identity_unchanged,
                                        after,
                                        discovery,
                                        live_leaf_queue: queue,
                                    }
                                }
                            }
                            Err(error) if queue_error_is_resource(&error) => {
                                let resource = GeneratedFamilySectorResource::LiveLeafQueue(error);
                                state.stop = WorkingStop::ResourceLimited {
                                    depth,
                                    stage: GeneratedFamilyDepthGrowthStage::LiveLeafQueue,
                                    resource: resource.clone(),
                                };
                                GeneratedFamilyDepthGrowthAttemptOutcome::ResourceLimited {
                                    stage: GeneratedFamilyDepthGrowthStage::LiveLeafQueue,
                                    completed_discovery: Some(discovery),
                                    resource,
                                }
                            }
                            Err(error) => {
                                let failure = GeneratedFamilySectorFailure::LiveLeafQueue(error);
                                state.stop = WorkingStop::Failed {
                                    depth,
                                    stage: GeneratedFamilyDepthGrowthStage::LiveLeafQueue,
                                    failure: failure.clone(),
                                };
                                GeneratedFamilyDepthGrowthAttemptOutcome::Failed {
                                    stage: GeneratedFamilyDepthGrowthStage::LiveLeafQueue,
                                    completed_discovery: Some(discovery),
                                    failure,
                                }
                            }
                        }
                    }
                    Err(error) if discovery_error_is_resource(&error) => {
                        let resource = GeneratedFamilySectorResource::Discovery(error);
                        state.stop = WorkingStop::ResourceLimited {
                            depth,
                            stage: GeneratedFamilyDepthGrowthStage::Discovery,
                            resource: resource.clone(),
                        };
                        GeneratedFamilyDepthGrowthAttemptOutcome::ResourceLimited {
                            stage: GeneratedFamilyDepthGrowthStage::Discovery,
                            completed_discovery: None,
                            resource,
                        }
                    }
                    Err(error) => {
                        let failure = GeneratedFamilySectorFailure::Discovery(error);
                        state.stop = WorkingStop::Failed {
                            depth,
                            stage: GeneratedFamilyDepthGrowthStage::Discovery,
                            failure: failure.clone(),
                        };
                        GeneratedFamilyDepthGrowthAttemptOutcome::Failed {
                            stage: GeneratedFamilyDepthGrowthStage::Discovery,
                            completed_discovery: None,
                            failure,
                        }
                    }
                };
                attempts.push(GeneratedFamilyDepthGrowthSectorAttempt {
                    ordinal: checked_add(
                        "depth-growth sector attempt ordinal",
                        aggregate_attempts,
                        attempts.len(),
                    )?,
                    sector: state.sector.clone(),
                    solve_ordinal: state.solve_ordinal,
                    previous_successful_depth,
                    before,
                    outcome,
                });
            }
            aggregate_attempts = requested_attempts;
            rounds.push(GeneratedFamilyDepthGrowthRound {
                ordinal: rounds.len(),
                depth,
                attempts: attempts.into_boxed_slice(),
            });
        }

        check_limit(
            "depth-growth final sector statuses",
            working.len(),
            limits.max_final_sector_statuses,
        )?;
        let final_statuses = working
            .into_iter()
            .map(|state| {
                let status = match state.stop {
                    WorkingStop::Covered => {
                        GeneratedFamilyDepthGrowthFinalStatus::CoveredByGeneratedRules {
                            depth: state.latest_depth,
                        }
                    }
                    WorkingStop::Stalled { depth } => {
                        GeneratedFamilyDepthGrowthFinalStatus::StalledNoStrictResidualImprovement {
                            depth,
                            residual: state.residual,
                        }
                    }
                    WorkingStop::ResourceLimited {
                        depth,
                        stage,
                        resource,
                    } => GeneratedFamilyDepthGrowthFinalStatus::ResourceLimited {
                        attempted_depth: depth,
                        stage,
                        resource,
                    },
                    WorkingStop::Failed {
                        depth,
                        stage,
                        failure,
                    } => GeneratedFamilyDepthGrowthFinalStatus::Failed {
                        attempted_depth: depth,
                        stage,
                        failure,
                    },
                    WorkingStop::Active if state.latest_depth == config.maximum_depth => {
                        GeneratedFamilyDepthGrowthFinalStatus::ExhaustedAtMaxDepth {
                            latest_successful_depth: state.latest_depth,
                            residual: state.residual,
                        }
                    }
                    WorkingStop::Active => {
                        GeneratedFamilyDepthGrowthFinalStatus::NotSelectedByPolicyBound {
                            latest_successful_depth: state.latest_depth,
                            residual: state.residual,
                        }
                    }
                };
                GeneratedFamilyDepthGrowthSectorStatus {
                    sector: state.sector,
                    solve_ordinal: state.solve_ordinal,
                    status,
                }
            })
            .collect::<Vec<_>>();
        let stats = compute_stats(&rounds, &final_statuses, limits)?;
        Ok(GeneratedFamilyDepthGrowthCertificate {
            schema: GENERATED_FAMILY_DEPTH_GROWTH_V1_SCHEMA,
            family_fingerprint: family.fingerprint().into(),
            context_fingerprint: context.fingerprint().into(),
            base,
            config,
            limits,
            rounds: rounds.into_boxed_slice(),
            final_statuses: final_statuses.into_boxed_slice(),
            stats,
        })
    }
}

#[derive(Clone)]
struct WorkingSector {
    sector: SectorMask,
    solve_ordinal: usize,
    latest_depth: usize,
    residual: GeneratedSectorResidualSummary,
    stop: WorkingStop,
}

#[derive(Clone)]
enum WorkingStop {
    Active,
    Covered,
    Stalled {
        depth: usize,
    },
    ResourceLimited {
        depth: usize,
        stage: GeneratedFamilyDepthGrowthStage,
        resource: GeneratedFamilySectorResource,
    },
    Failed {
        depth: usize,
        stage: GeneratedFamilyDepthGrowthStage,
        failure: GeneratedFamilySectorFailure,
    },
}

fn residual_summary(
    discovery: &GeneratedSectorDiscoveryCertificate,
    queue: &GeneratedSectorLiveLeafQueueCertificate,
    limits: GeneratedFamilyDepthGrowthLimits,
) -> Result<GeneratedSectorResidualSummary, GeneratedFamilyDepthGrowthError> {
    let mut leaves = Vec::new();
    let mut measure = GeneratedResidualMeasure::default();
    let mut predicates = 0usize;
    for classification in discovery.coverage().classifications() {
        let kind = match classification.disposition() {
            ParametricSectorLeafDisposition::DescendingRule { .. }
            | ParametricSectorLeafDisposition::ProvedEmptyLocus { .. } => continue,
            ParametricSectorLeafDisposition::Uncovered => {
                measure.uncovered_leaves =
                    checked_add("depth-growth uncovered leaves", measure.uncovered_leaves, 1)?;
                GeneratedResidualLeafKind::Uncovered
            }
            ParametricSectorLeafDisposition::Unsupported { candidate_ordinals } => {
                measure.unsupported_leaves = checked_add(
                    "depth-growth unsupported leaves",
                    measure.unsupported_leaves,
                    1,
                )?;
                GeneratedResidualLeafKind::Unsupported {
                    candidate_ordinals: candidate_ordinals.clone(),
                }
            }
        };
        let case = discovery
            .coverage()
            .partition()
            .case(classification.case())
            .ok_or(GeneratedFamilyDepthGrowthError::ReplayMismatch {
                detail: "residual classification has no structural case",
            })?;
        predicates = checked_add(
            "depth-growth retained residual predicates",
            predicates,
            case.predicates().len(),
        )?;
        check_limit(
            "depth-growth retained residual predicates",
            predicates,
            limits.max_retained_residual_predicates,
        )?;
        check_limit(
            "depth-growth retained residual leaves",
            checked_add("depth-growth retained residual leaves", leaves.len(), 1)?,
            limits.max_retained_residual_leaves,
        )?;
        leaves.push(GeneratedResidualLeafIdentity {
            predicates: case.predicates().to_vec().into_boxed_slice(),
            kind,
        });
    }
    measure.root_residual_leaves = leaves.len();
    if queue.work_items().len() != leaves.len() {
        return Err(GeneratedFamilyDepthGrowthError::ReplayMismatch {
            detail: "live queue and structural residual leaf counts differ",
        });
    }
    for item in queue.work_items() {
        match item.outcome() {
            GeneratedSectorLiveLeafOutcome::CoordinateLeafProvedEmpty => {}
            GeneratedSectorLiveLeafOutcome::PreservedWithoutEqualityAssignment => {
                measure.preserved_without_assignment_leaves = checked_add(
                    "depth-growth preserved leaves",
                    measure.preserved_without_assignment_leaves,
                    1,
                )?;
            }
            GeneratedSectorLiveLeafOutcome::PreservedIndexBoundary {
                residual_unresolved_predicates,
                ..
            } => {
                measure.preserved_index_boundary_leaves = checked_add(
                    "depth-growth index-boundary leaves",
                    measure.preserved_index_boundary_leaves,
                    1,
                )?;
                measure.residual_unresolved_predicates = checked_add(
                    "depth-growth residual unresolved predicates",
                    measure.residual_unresolved_predicates,
                    *residual_unresolved_predicates,
                )?;
            }
            GeneratedSectorLiveLeafOutcome::PartialReelimination {
                residual_unresolved_predicates,
                compilation,
            } => {
                measure.residual_unresolved_predicates = checked_add(
                    "depth-growth residual unresolved predicates",
                    measure.residual_unresolved_predicates,
                    *residual_unresolved_predicates,
                )?;
                match compilation {
                    crate::GeneratedPartialReeliminationCompilation::Certified(_) => {
                        measure.certified_partial_reeliminations = checked_add(
                            "depth-growth certified partial re-eliminations",
                            measure.certified_partial_reeliminations,
                            1,
                        )?;
                    }
                    crate::GeneratedPartialReeliminationCompilation::EmptySystem(_) => {
                        measure.empty_partial_systems = checked_add(
                            "depth-growth empty partial systems",
                            measure.empty_partial_systems,
                            1,
                        )?;
                    }
                }
            }
        }
    }
    Ok(GeneratedSectorResidualSummary {
        leaves: leaves.into_boxed_slice(),
        measure,
    })
}

fn select_active(
    active: &[usize],
    policy: GeneratedFamilyDepthGrowthSelectionPolicy,
) -> Result<Vec<usize>, GeneratedFamilyDepthGrowthError> {
    match policy {
        GeneratedFamilyDepthGrowthSelectionPolicy::AllResidualSubsectorFirst => Ok(active.to_vec()),
        GeneratedFamilyDepthGrowthSelectionPolicy::ResidualSubsectorFirstPrefix {
            max_sectors_per_round,
        } => {
            if max_sectors_per_round == 0 {
                return Err(GeneratedFamilyDepthGrowthError::InvalidConfig {
                    detail: "residual-sector prefix must be nonzero",
                });
            }
            Ok(active.iter().take(max_sectors_per_round).copied().collect())
        }
    }
}

fn validate_shared_material(
    row_span: Option<&Arc<GeneratedSymbolicRowSpanCertificate>>,
    discovery: &GeneratedSectorDiscoveryCertificate,
    queue: &GeneratedSectorLiveLeafQueueCertificate,
) -> Result<(), GeneratedFamilyDepthGrowthError> {
    let row_span = row_span.ok_or(GeneratedFamilyDepthGrowthError::ReplayMismatch {
        detail: "generated material has no family row span",
    })?;
    validate_shared_discovery(row_span, discovery)?;
    validate_shared_discovery(row_span, queue.discovery())?;
    if queue.sector() != discovery.sector()
        || queue.ordering() != discovery.ordering()
        || queue.discovery().stats() != discovery.stats()
    {
        return Err(GeneratedFamilyDepthGrowthError::ReplayMismatch {
            detail: "depth-growth queue is not bound to its discovery",
        });
    }
    Ok(())
}

fn validate_shared_discovery(
    row_span: &Arc<GeneratedSymbolicRowSpanCertificate>,
    discovery: &GeneratedSectorDiscoveryCertificate,
) -> Result<(), GeneratedFamilyDepthGrowthError> {
    if !Arc::ptr_eq(discovery.row_span_arc(), row_span)
        || !Arc::ptr_eq(discovery.coverage().row_span_arc(), row_span)
        || !discovery
            .coverage()
            .candidate_attempts()
            .iter()
            .all(|attempt| {
                Arc::ptr_eq(
                    attempt.compilation().source_authentication().row_span_arc(),
                    row_span,
                )
            })
    {
        return Err(GeneratedFamilyDepthGrowthError::ReplayMismatch {
            detail: "depth-growth material does not reuse the family row-span allocation",
        });
    }
    Ok(())
}

fn validate_scope(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    base: &GeneratedFamilyRuleSystemCertificate,
) -> Result<(), GeneratedFamilyDepthGrowthError> {
    if base.family_fingerprint() != family.fingerprint() {
        return Err(GeneratedFamilyDepthGrowthError::WrongFamily);
    }
    if base.context_fingerprint() != context.fingerprint()
        || !family
            .coefficient_context()
            .has_same_variable_map(context.base())
    {
        return Err(GeneratedFamilyDepthGrowthError::WrongContext);
    }
    if family.denominator_count() != context.index_count() {
        return Err(GeneratedFamilyDepthGrowthError::WrongArity {
            expected: family.denominator_count(),
            actual: context.index_count(),
        });
    }
    Ok(())
}

fn validate_config(
    config: GeneratedFamilyDepthGrowthConfig,
    limits: GeneratedFamilyDepthGrowthLimits,
) -> Result<(), GeneratedFamilyDepthGrowthError> {
    if config.maximum_depth < config.initial_depth {
        return Err(GeneratedFamilyDepthGrowthError::InvalidConfig {
            detail: "maximum depth is below the base depth",
        });
    }
    if let GeneratedFamilyDepthGrowthSelectionPolicy::ResidualSubsectorFirstPrefix {
        max_sectors_per_round: 0,
    } = config.selection
    {
        return Err(GeneratedFamilyDepthGrowthError::InvalidConfig {
            detail: "residual-sector prefix must be nonzero",
        });
    }
    let requested_rounds = config.maximum_depth - config.initial_depth;
    check_limit("depth-growth rounds", requested_rounds, limits.max_rounds)
}

fn compute_stats(
    rounds: &[GeneratedFamilyDepthGrowthRound],
    final_statuses: &[GeneratedFamilyDepthGrowthSectorStatus],
    limits: GeneratedFamilyDepthGrowthLimits,
) -> Result<GeneratedFamilyDepthGrowthStats, GeneratedFamilyDepthGrowthError> {
    let mut stats = GeneratedFamilyDepthGrowthStats {
        rounds: rounds.len(),
        ..GeneratedFamilyDepthGrowthStats::default()
    };
    for round in rounds {
        stats.sector_attempts = checked_add(
            "depth-growth sector attempts",
            stats.sector_attempts,
            round.attempts.len(),
        )?;
        stats.shared_row_span_sector_reuses = checked_add(
            "depth-growth shared row-span sector reuses",
            stats.shared_row_span_sector_reuses,
            round.attempts.len(),
        )?;
        for attempt in &round.attempts {
            stats.retained_residual_leaves = checked_add(
                "depth-growth retained residual leaves",
                stats.retained_residual_leaves,
                attempt.before.leaves.len(),
            )?;
            stats.retained_residual_predicates = checked_add(
                "depth-growth retained residual predicates",
                stats.retained_residual_predicates,
                residual_predicate_count(&attempt.before)?,
            )?;
            match &attempt.outcome {
                GeneratedFamilyDepthGrowthAttemptOutcome::StrictResidualImprovement {
                    after,
                    ..
                } => {
                    stats.strict_improvements = checked_add(
                        "depth-growth strict improvements",
                        stats.strict_improvements,
                        1,
                    )?;
                    add_after_stats(&mut stats, after)?;
                }
                GeneratedFamilyDepthGrowthAttemptOutcome::StalledNoStrictResidualImprovement {
                    after,
                    ..
                } => {
                    stats.stalled = checked_add("depth-growth stalled attempts", stats.stalled, 1)?;
                    add_after_stats(&mut stats, after)?;
                }
                GeneratedFamilyDepthGrowthAttemptOutcome::ResourceLimited { .. } => {
                    stats.resource_limited = checked_add(
                        "depth-growth resource interruptions",
                        stats.resource_limited,
                        1,
                    )?;
                }
                GeneratedFamilyDepthGrowthAttemptOutcome::Failed { .. } => {
                    stats.failed = checked_add("depth-growth failed attempts", stats.failed, 1)?;
                }
            }
        }
    }
    for final_status in final_statuses {
        let residual = match final_status.status() {
            GeneratedFamilyDepthGrowthFinalStatus::ExhaustedAtMaxDepth { residual, .. }
            | GeneratedFamilyDepthGrowthFinalStatus::StalledNoStrictResidualImprovement {
                residual,
                ..
            }
            | GeneratedFamilyDepthGrowthFinalStatus::NotSelectedByPolicyBound {
                residual, ..
            } => Some(residual),
            GeneratedFamilyDepthGrowthFinalStatus::CoveredByGeneratedRules { .. }
            | GeneratedFamilyDepthGrowthFinalStatus::ResourceLimited { .. }
            | GeneratedFamilyDepthGrowthFinalStatus::Failed { .. } => None,
        };
        if let Some(residual) = residual {
            stats.retained_residual_leaves = checked_add(
                "depth-growth retained residual leaves",
                stats.retained_residual_leaves,
                residual.leaves.len(),
            )?;
            stats.retained_residual_predicates = checked_add(
                "depth-growth retained residual predicates",
                stats.retained_residual_predicates,
                residual_predicate_count(residual)?,
            )?;
        }
    }
    check_limit(
        "depth-growth sector attempts",
        stats.sector_attempts,
        limits.max_sector_attempts,
    )?;
    check_limit(
        "depth-growth final sector statuses",
        final_statuses.len(),
        limits.max_final_sector_statuses,
    )?;
    check_limit(
        "depth-growth retained residual leaves",
        stats.retained_residual_leaves,
        limits.max_retained_residual_leaves,
    )?;
    check_limit(
        "depth-growth retained residual predicates",
        stats.retained_residual_predicates,
        limits.max_retained_residual_predicates,
    )?;
    Ok(stats)
}

fn add_after_stats(
    stats: &mut GeneratedFamilyDepthGrowthStats,
    after: &GeneratedSectorResidualSummary,
) -> Result<(), GeneratedFamilyDepthGrowthError> {
    stats.retained_residual_leaves = checked_add(
        "depth-growth retained residual leaves",
        stats.retained_residual_leaves,
        after.leaves.len(),
    )?;
    stats.retained_residual_predicates = checked_add(
        "depth-growth retained residual predicates",
        stats.retained_residual_predicates,
        residual_predicate_count(after)?,
    )?;
    Ok(())
}

fn residual_predicate_count(
    residual: &GeneratedSectorResidualSummary,
) -> Result<usize, GeneratedFamilyDepthGrowthError> {
    let mut count = 0usize;
    for leaf in &residual.leaves {
        count = checked_add(
            "depth-growth retained residual predicates",
            count,
            leaf.predicates.len(),
        )?;
    }
    Ok(count)
}

fn certificate_payload_eq(
    left: &GeneratedFamilyDepthGrowthCertificate,
    right: &GeneratedFamilyDepthGrowthCertificate,
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
        && left.final_statuses == right.final_statuses
        && left.rounds.len() == right.rounds.len()
        && left
            .rounds
            .iter()
            .zip(right.rounds.iter())
            .all(round_payload_eq)
}

fn round_payload_eq(
    (left, right): (
        &GeneratedFamilyDepthGrowthRound,
        &GeneratedFamilyDepthGrowthRound,
    ),
) -> bool {
    left.ordinal == right.ordinal
        && left.depth == right.depth
        && left.attempts.len() == right.attempts.len()
        && left
            .attempts
            .iter()
            .zip(right.attempts.iter())
            .all(attempt_payload_eq)
}

fn attempt_payload_eq(
    (left, right): (
        &GeneratedFamilyDepthGrowthSectorAttempt,
        &GeneratedFamilyDepthGrowthSectorAttempt,
    ),
) -> bool {
    left.ordinal == right.ordinal
        && left.sector == right.sector
        && left.solve_ordinal == right.solve_ordinal
        && left.previous_successful_depth == right.previous_successful_depth
        && left.before == right.before
        && outcome_payload_eq(&left.outcome, &right.outcome)
}

fn outcome_payload_eq(
    left: &GeneratedFamilyDepthGrowthAttemptOutcome,
    right: &GeneratedFamilyDepthGrowthAttemptOutcome,
) -> bool {
    match (left, right) {
        (
            GeneratedFamilyDepthGrowthAttemptOutcome::StrictResidualImprovement {
                exact_residual_identity_unchanged: left_identity,
                after: left_after,
                discovery: left_discovery,
                live_leaf_queue: left_queue,
            },
            GeneratedFamilyDepthGrowthAttemptOutcome::StrictResidualImprovement {
                exact_residual_identity_unchanged: right_identity,
                after: right_after,
                discovery: right_discovery,
                live_leaf_queue: right_queue,
            },
        )
        | (
            GeneratedFamilyDepthGrowthAttemptOutcome::StalledNoStrictResidualImprovement {
                exact_residual_identity_unchanged: left_identity,
                after: left_after,
                discovery: left_discovery,
                live_leaf_queue: left_queue,
            },
            GeneratedFamilyDepthGrowthAttemptOutcome::StalledNoStrictResidualImprovement {
                exact_residual_identity_unchanged: right_identity,
                after: right_after,
                discovery: right_discovery,
                live_leaf_queue: right_queue,
            },
        ) => {
            left_identity == right_identity
                && left_after == right_after
                && left_discovery.payload_eq(right_discovery)
                && left_queue.payload_eq(right_queue)
        }
        (
            GeneratedFamilyDepthGrowthAttemptOutcome::ResourceLimited {
                stage: left_stage,
                completed_discovery: left_discovery,
                resource: left_resource,
            },
            GeneratedFamilyDepthGrowthAttemptOutcome::ResourceLimited {
                stage: right_stage,
                completed_discovery: right_discovery,
                resource: right_resource,
            },
        ) => {
            left_stage == right_stage
                && left_resource == right_resource
                && optional_discovery_payload_eq(left_discovery.as_ref(), right_discovery.as_ref())
        }
        (
            GeneratedFamilyDepthGrowthAttemptOutcome::Failed {
                stage: left_stage,
                completed_discovery: left_discovery,
                failure: left_failure,
            },
            GeneratedFamilyDepthGrowthAttemptOutcome::Failed {
                stage: right_stage,
                completed_discovery: right_discovery,
                failure: right_failure,
            },
        ) => {
            left_stage == right_stage
                && left_failure == right_failure
                && optional_discovery_payload_eq(left_discovery.as_ref(), right_discovery.as_ref())
        }
        _ => false,
    }
}

fn optional_discovery_payload_eq(
    left: Option<&GeneratedSectorDiscoveryCertificate>,
    right: Option<&GeneratedSectorDiscoveryCertificate>,
) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.payload_eq(right),
        (None, None) => true,
        _ => false,
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedFamilyDepthGrowthError> {
    left.checked_add(right)
        .ok_or(GeneratedFamilyDepthGrowthError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedFamilyDepthGrowthError> {
    if requested <= limit {
        Ok(())
    } else {
        Err(GeneratedFamilyDepthGrowthError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedFamilyDepthGrowthError {
    SchemaMismatch,
    ReplayMismatch {
        detail: &'static str,
    },
    WrongFamily,
    WrongContext,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    InitialDepthMismatch {
        sector: SectorMask,
        expected: usize,
        actual: usize,
    },
    InvalidConfig {
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
}

impl fmt::Display for GeneratedFamilyDepthGrowthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter.write_str("family depth-growth schema mismatch"),
            Self::ReplayMismatch { detail } => {
                write!(formatter, "family depth-growth replay mismatch: {detail}")
            }
            Self::WrongFamily => formatter.write_str("family depth-growth family mismatch"),
            Self::WrongContext => formatter.write_str("family depth-growth context mismatch"),
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "family depth-growth arity is {actual}, expected {expected}"
            ),
            Self::InitialDepthMismatch {
                sector,
                expected,
                actual,
            } => write!(
                formatter,
                "sector {} starts at depth {actual}, expected base depth {expected}",
                sector.to_bit_string()
            ),
            Self::InvalidConfig { detail } => {
                write!(
                    formatter,
                    "invalid family depth-growth configuration: {detail}"
                )
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
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::Base(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratedFamilyDepthGrowthError {}

impl From<GeneratedFamilyRuleSystemError> for GeneratedFamilyDepthGrowthError {
    fn from(value: GeneratedFamilyRuleSystemError) -> Self {
        Self::Base(value)
    }
}

// Concrete application -----------------------------------------------------

pub type GeneratedFamilyDepthGrowthConditionalError =
    GeneratedSectorConditionalRuleProviderError<ParametricSectorRuleProviderError>;
pub type GeneratedFamilyDepthGrowthMasterError =
    MasterPolicyError<GeneratedFamilyDepthGrowthConditionalError>;
pub type GeneratedFamilyDepthGrowthStackError =
    CertifiedZeroSectorRuleProviderError<GeneratedFamilyDepthGrowthMasterError>;

type DepthSectorProvider<'family> = ParametricSectorRuleProvider<'family>;
type DepthConditionalProvider<'family> =
    GeneratedSectorConditionalRuleProvider<'family, DepthSectorProvider<'family>>;
type DepthMasterProvider<'family> = MasterPolicyProvider<DepthConditionalProvider<'family>>;
type DepthProviderStack<'family> =
    CertifiedZeroSectorRuleProvider<'family, DepthMasterProvider<'family>>;

pub struct GeneratedFamilyDepthGrowthProvider<'family> {
    family: &'family IntegralFamily,
    context: &'family ParametricCoefficientContext,
    certificate: GeneratedFamilyDepthGrowthCertificate,
    stack: DepthProviderStack<'family>,
    limits: GeneratedFamilyRuleSystemProviderLimits,
}

impl<'family> GeneratedFamilyDepthGrowthProvider<'family> {
    pub const SCHEMA: &'static str = GENERATED_FAMILY_DEPTH_GROWTH_PROVIDER_V1_SCHEMA;

    pub fn try_new(
        family: &'family IntegralFamily,
        context: &'family ParametricCoefficientContext,
        certificate: GeneratedFamilyDepthGrowthCertificate,
        limits: GeneratedFamilyRuleSystemProviderLimits,
    ) -> Result<Self, GeneratedFamilyDepthGrowthProviderError> {
        Self::try_with_terminals(family, context, certificate, [], limits)
    }

    pub fn try_with_selected(
        family: &'family IntegralFamily,
        context: &'family ParametricCoefficientContext,
        certificate: GeneratedFamilyDepthGrowthCertificate,
        selected: impl IntoIterator<Item = ConcreteIntegralKey>,
        limits: GeneratedFamilyRuleSystemProviderLimits,
    ) -> Result<Self, GeneratedFamilyDepthGrowthProviderError> {
        Self::try_with_terminals(
            family,
            context,
            certificate,
            selected
                .into_iter()
                .map(|key| (key, MasterPolicyTerminal::Selected)),
            limits,
        )
    }

    pub fn try_with_terminals(
        family: &'family IntegralFamily,
        context: &'family ParametricCoefficientContext,
        certificate: GeneratedFamilyDepthGrowthCertificate,
        terminals: impl IntoIterator<Item = (ConcreteIntegralKey, MasterPolicyTerminal)>,
        limits: GeneratedFamilyRuleSystemProviderLimits,
    ) -> Result<Self, GeneratedFamilyDepthGrowthProviderError> {
        Self::try_with_terminals_impl(family, context, certificate, terminals, limits, true)
    }

    fn try_with_terminals_from_compiler(
        family: &'family IntegralFamily,
        context: &'family ParametricCoefficientContext,
        certificate: GeneratedFamilyDepthGrowthCertificate,
        terminals: impl IntoIterator<Item = (ConcreteIntegralKey, MasterPolicyTerminal)>,
        limits: GeneratedFamilyRuleSystemProviderLimits,
    ) -> Result<Self, GeneratedFamilyDepthGrowthProviderError> {
        Self::try_with_terminals_impl(family, context, certificate, terminals, limits, false)
    }

    fn try_with_terminals_impl(
        family: &'family IntegralFamily,
        context: &'family ParametricCoefficientContext,
        certificate: GeneratedFamilyDepthGrowthCertificate,
        terminals: impl IntoIterator<Item = (ConcreteIntegralKey, MasterPolicyTerminal)>,
        limits: GeneratedFamilyRuleSystemProviderLimits,
        replay_certificate: bool,
    ) -> Result<Self, GeneratedFamilyDepthGrowthProviderError> {
        let has_interruption = reject_provider_interruptions(&certificate).is_err();
        let materials = certificate.latest_successful_materials();
        if !has_interruption {
            preflight_provider_materials(&materials, limits)?;
            ParametricSectorRuleProvider::preflight_certificates(
                family,
                context,
                materials
                    .iter()
                    .map(|material| material.discovery().coverage()),
                limits.sector_rules,
            )
            .map_err(GeneratedFamilyDepthGrowthProviderError::Sector)?;
            GeneratedSectorConditionalRuleProvider::<DepthSectorProvider<'family>>::preflight_queues(
                family,
                context,
                materials
                    .iter()
                    .map(GeneratedFamilyDepthGrowthMaterialRef::live_leaf_queue),
                limits.conditional_rules,
            )
            .map_err(GeneratedFamilyDepthGrowthProviderError::Conditional)?;
        }
        if replay_certificate {
            certificate.replay(family, context)?;
        }
        reject_provider_interruptions(&certificate)?;
        let shared_row_span = certificate
            .base()
            .row_span_arc()
            .cloned()
            .ok_or(GeneratedFamilyDepthGrowthProviderError::MissingSharedRowSpan)?;
        let zero_limits = certificate.base().limits().inventory.zero_sectors;
        if limits.certified_rewrite.zero_sector != zero_limits {
            return Err(GeneratedFamilyDepthGrowthProviderError::ZeroAnalysisLimitsMismatch);
        }
        let coverages = materials
            .iter()
            .map(|material| material.discovery().coverage().clone())
            .collect::<Vec<_>>();
        let queues = materials
            .iter()
            .map(|material| material.live_leaf_queue().clone())
            .collect::<Vec<_>>();
        let sector = ParametricSectorRuleProvider::try_new_with_replayed_certificates(
            family,
            context,
            coverages,
            &shared_row_span,
            limits.sector_rules,
        )
        .map_err(GeneratedFamilyDepthGrowthProviderError::Sector)?;
        let conditional = GeneratedSectorConditionalRuleProvider::try_new_with_replayed_queues(
            family,
            context,
            queues,
            sector,
            &shared_row_span,
            limits.conditional_rules,
        )
        .map_err(GeneratedFamilyDepthGrowthProviderError::Conditional)?;
        let master = MasterPolicyProvider::try_new(conditional, terminals, limits.master_policy)
            .map_err(GeneratedFamilyDepthGrowthProviderError::Master)?;
        let stack = CertifiedZeroSectorRuleProvider::try_new(
            family,
            certificate.base().inventory_restrictions().clone(),
            certificate.base().inventory_power_shift_policy(),
            master,
            limits.certified_rewrite,
        )
        .map_err(GeneratedFamilyDepthGrowthProviderError::Stack)?;
        Ok(Self {
            family,
            context,
            certificate,
            stack,
            limits,
        })
    }

    pub const fn certificate(&self) -> &GeneratedFamilyDepthGrowthCertificate {
        &self.certificate
    }
    pub const fn limits(&self) -> GeneratedFamilyRuleSystemProviderLimits {
        self.limits
    }
    pub fn terminals(
        &self,
    ) -> &std::collections::BTreeMap<ConcreteIntegralKey, MasterPolicyTerminal> {
        self.stack.inner().terminals()
    }
    pub const fn sector_provider(&self) -> &DepthSectorProvider<'family> {
        self.stack.inner().inner().inner()
    }
    pub const fn conditional_provider(&self) -> &DepthConditionalProvider<'family> {
        self.stack.inner().inner()
    }

    pub fn insert_selected_master(
        &mut self,
        integral: ConcreteIntegralKey,
    ) -> Result<(), GeneratedFamilyDepthGrowthProviderError> {
        self.stack
            .inner_mut()
            .insert_terminal(integral, MasterPolicyTerminal::Selected)
            .map_err(GeneratedFamilyDepthGrowthProviderError::Master)
    }

    pub fn replay(&self) -> Result<(), GeneratedFamilyDepthGrowthProviderError> {
        self.certificate.replay(self.family, self.context)?;
        reject_provider_interruptions(&self.certificate)?;
        self.validate_material_binding()?;
        self.conditional_provider()
            .replay_with_replayed_queues()
            .map_err(GeneratedFamilyDepthGrowthProviderError::Conditional)?;
        Ok(())
    }

    fn validate_material_binding(&self) -> Result<(), GeneratedFamilyDepthGrowthProviderError> {
        let shared = self
            .certificate
            .base()
            .row_span_arc()
            .ok_or(GeneratedFamilyDepthGrowthProviderError::MissingSharedRowSpan)?;
        let materials = self.certificate.latest_successful_materials();
        let material_by_sector = materials
            .iter()
            .map(|material| (material.sector(), material))
            .collect::<BTreeMap<_, _>>();
        if self.sector_provider().certificates().len() != materials.len()
            || self.conditional_provider().queues().len() != materials.len()
        {
            return Err(GeneratedFamilyDepthGrowthProviderError::ReplayMismatch {
                detail: "installed sector set differs from latest depth-growth material",
            });
        }
        for material in &materials {
            if !Arc::ptr_eq(material.discovery().row_span_arc(), shared)
                || !Arc::ptr_eq(material.discovery().coverage().row_span_arc(), shared)
                || !Arc::ptr_eq(
                    material.live_leaf_queue().discovery().row_span_arc(),
                    shared,
                )
            {
                return Err(GeneratedFamilyDepthGrowthProviderError::ReplayMismatch {
                    detail: "latest depth-growth material lost the base shared row span",
                });
            }
            let installed_coverage = self
                .sector_provider()
                .certificates()
                .get(material.sector())
                .ok_or(GeneratedFamilyDepthGrowthProviderError::ReplayMismatch {
                    detail: "latest depth-growth coverage was not installed",
                })?;
            if !installed_coverage.payload_eq(material.discovery().coverage())
                || !Arc::ptr_eq(installed_coverage.row_span_arc(), shared)
            {
                return Err(GeneratedFamilyDepthGrowthProviderError::ReplayMismatch {
                    detail: "installed coverage differs from latest depth-growth material",
                });
            }
        }
        for queue in self.conditional_provider().queues() {
            let material = material_by_sector.get(queue.sector()).copied().ok_or(
                GeneratedFamilyDepthGrowthProviderError::ReplayMismatch {
                    detail: "installed conditional queue has no latest depth-growth material",
                },
            )?;
            if !queue.payload_eq(material.live_leaf_queue())
                || !Arc::ptr_eq(queue.discovery().row_span_arc(), shared)
            {
                return Err(GeneratedFamilyDepthGrowthProviderError::ReplayMismatch {
                    detail: "installed queue differs from latest depth-growth material",
                });
            }
        }
        Ok(())
    }
}

impl ConcreteRuleProvider for GeneratedFamilyDepthGrowthProvider<'_> {
    type Error = GeneratedFamilyDepthGrowthProviderError;

    fn index_arity(&self) -> usize {
        self.context.index_count()
    }

    fn decision_for(
        &mut self,
        integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, Self::Error> {
        self.stack
            .decision_for(integral)
            .map_err(GeneratedFamilyDepthGrowthProviderError::Stack)
    }
}

fn reject_provider_interruptions(
    certificate: &GeneratedFamilyDepthGrowthCertificate,
) -> Result<(), GeneratedFamilyDepthGrowthProviderError> {
    for transcript in certificate.base().sectors() {
        match transcript.status() {
            GeneratedFamilySectorStatus::ResourceLimited { resource, .. } => {
                return Err(
                    GeneratedFamilyDepthGrowthProviderError::BaseResourceLimited {
                        sector: transcript.sector().clone(),
                        resource: resource.clone(),
                    },
                );
            }
            GeneratedFamilySectorStatus::Failed { failure, .. } => {
                return Err(GeneratedFamilyDepthGrowthProviderError::BaseFailed {
                    sector: transcript.sector().clone(),
                    failure: failure.clone(),
                });
            }
            _ => {}
        }
    }
    for round in certificate.rounds() {
        for attempt in round.attempts() {
            match attempt.outcome() {
                GeneratedFamilyDepthGrowthAttemptOutcome::ResourceLimited {
                    stage,
                    resource,
                    ..
                } => {
                    return Err(
                        GeneratedFamilyDepthGrowthProviderError::RoundResourceLimited {
                            sector: attempt.sector().clone(),
                            depth: round.depth(),
                            stage: *stage,
                            resource: resource.clone(),
                        },
                    );
                }
                GeneratedFamilyDepthGrowthAttemptOutcome::Failed { stage, failure, .. } => {
                    return Err(GeneratedFamilyDepthGrowthProviderError::RoundFailed {
                        sector: attempt.sector().clone(),
                        depth: round.depth(),
                        stage: *stage,
                        failure: failure.clone(),
                    });
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn preflight_provider_materials(
    materials: &[GeneratedFamilyDepthGrowthMaterialRef<'_>],
    limits: GeneratedFamilyRuleSystemProviderLimits,
) -> Result<(), GeneratedFamilyDepthGrowthProviderError> {
    provider_check_limit(
        "depth-growth provider generated sectors",
        materials.len(),
        limits.max_retained_generated_sectors,
    )?;
    let mut candidates = 0usize;
    let mut leaves = 0usize;
    let mut work_items = 0usize;
    for material in materials {
        candidates = provider_checked_add(
            "depth-growth provider candidate attempts",
            candidates,
            material.discovery().stats().candidate_attempts(),
        )?;
        leaves = provider_checked_add(
            "depth-growth provider global leaves",
            leaves,
            material.discovery().stats().global_leaves(),
        )?;
        work_items = provider_checked_add(
            "depth-growth provider live-leaf work items",
            work_items,
            material.live_leaf_queue().work_items().len(),
        )?;
    }
    provider_check_limit(
        "depth-growth provider candidate attempts",
        candidates,
        limits.max_total_candidate_attempts,
    )?;
    provider_check_limit(
        "depth-growth provider global leaves",
        leaves,
        limits.max_total_global_leaves,
    )?;
    provider_check_limit(
        "depth-growth provider live-leaf work items",
        work_items,
        limits.max_total_live_leaf_work_items,
    )
}

fn provider_checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedFamilyDepthGrowthProviderError> {
    left.checked_add(right)
        .ok_or(GeneratedFamilyDepthGrowthProviderError::ResourceCountOverflow { resource })
}

fn provider_check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedFamilyDepthGrowthProviderError> {
    if requested <= limit {
        Ok(())
    } else {
        Err(GeneratedFamilyDepthGrowthProviderError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    }
}

#[derive(Debug)]
pub enum GeneratedFamilyDepthGrowthProviderError {
    Certificate(GeneratedFamilyDepthGrowthError),
    BaseResourceLimited {
        sector: SectorMask,
        resource: GeneratedFamilySectorResource,
    },
    BaseFailed {
        sector: SectorMask,
        failure: GeneratedFamilySectorFailure,
    },
    RoundResourceLimited {
        sector: SectorMask,
        depth: usize,
        stage: GeneratedFamilyDepthGrowthStage,
        resource: GeneratedFamilySectorResource,
    },
    RoundFailed {
        sector: SectorMask,
        depth: usize,
        stage: GeneratedFamilyDepthGrowthStage,
        failure: GeneratedFamilySectorFailure,
    },
    ZeroAnalysisLimitsMismatch,
    MissingSharedRowSpan,
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
    Sector(ParametricSectorRuleProviderError),
    Conditional(GeneratedFamilyDepthGrowthConditionalError),
    Master(GeneratedFamilyDepthGrowthMasterError),
    Stack(GeneratedFamilyDepthGrowthStackError),
}

impl fmt::Display for GeneratedFamilyDepthGrowthProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Certificate(error) => error.fmt(formatter),
            Self::BaseResourceLimited { sector, resource } => write!(
                formatter,
                "base family sector {} is resource-limited at {:?}",
                sector.to_bit_string(),
                resource.stage()
            ),
            Self::BaseFailed { sector, failure } => write!(
                formatter,
                "base family sector {} failed at {:?}",
                sector.to_bit_string(),
                failure.stage()
            ),
            Self::RoundResourceLimited {
                sector,
                depth,
                stage,
                ..
            } => write!(
                formatter,
                "depth-growth sector {} is resource-limited at depth {depth} in {stage:?}",
                sector.to_bit_string()
            ),
            Self::RoundFailed {
                sector,
                depth,
                stage,
                ..
            } => write!(
                formatter,
                "depth-growth sector {} failed at depth {depth} in {stage:?}",
                sector.to_bit_string()
            ),
            Self::ZeroAnalysisLimitsMismatch => formatter.write_str(
                "depth-growth provider zero-analysis limits differ from the base certificate",
            ),
            Self::MissingSharedRowSpan => formatter
                .write_str("depth-growth provider has generated material but no shared row span"),
            Self::ReplayMismatch { detail } => {
                write!(formatter, "depth-growth provider replay mismatch: {detail}")
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
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::Sector(error) => error.fmt(formatter),
            Self::Conditional(error) => error.fmt(formatter),
            Self::Master(error) => error.fmt(formatter),
            Self::Stack(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratedFamilyDepthGrowthProviderError {}

impl From<GeneratedFamilyDepthGrowthError> for GeneratedFamilyDepthGrowthProviderError {
    fn from(value: GeneratedFamilyDepthGrowthError) -> Self {
        Self::Certificate(value)
    }
}
