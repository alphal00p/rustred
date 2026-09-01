use std::num::NonZeroUsize;

use crate::foundry::completion::source_discovery::CampaignModularProbe;
use crate::foundry::completion::source_discovery::cover_delta::{
    ExactOwnerCoverSnapshot, ExactOwnerLedgerCoverStatus, ExactOwnerLedgerSnapshotIdentity,
};

use super::super::super::boundary_simplex::BoundarySimplexSamplingProfile;
use super::{ProbeCoordinatorFailure, ProbeCoordinatorLimits};

const CAMPAIGN_KEY: &str = "declared campaign-key bytes";
const PROBES: &str = "declared task probes";

/// Immutable semantics of one bounded boundary probe program.
///
/// The caller-declared key must bind every external choice not stored here,
/// notably the probe program, family/context/predecessor scope, sector, and
/// ordering. It is an audit label, not a digest or authority token. Opaque
/// ledger and planner identities remain the actual delayed-work authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProbeCoordinatorConfig {
    declared_campaign_key: Box<str>,
    declared_probes_per_task: NonZeroUsize,
    interior_margin: u64,
    polynomial_degree_ceiling: usize,
    limits: ProbeCoordinatorLimits,
}

impl ProbeCoordinatorConfig {
    pub(crate) fn try_new(
        declared_campaign_key: &str,
        declared_probes_per_task: NonZeroUsize,
        interior_margin: u64,
        polynomial_degree_ceiling: usize,
        limits: ProbeCoordinatorLimits,
    ) -> Result<Self, ProbeCoordinatorFailure> {
        if declared_campaign_key.is_empty() {
            return Err(ProbeCoordinatorFailure::EmptyDeclaredCampaignKey);
        }
        if interior_margin == 0 {
            return Err(ProbeCoordinatorFailure::ZeroInteriorMargin);
        }
        if declared_probes_per_task.get() > limits.max_probes_per_task {
            return Err(ProbeCoordinatorFailure::ResourceLimit {
                resource: PROBES,
                requested: declared_probes_per_task.get(),
                limit: limits.max_probes_per_task,
            });
        }
        if declared_campaign_key.len() > limits.max_declared_campaign_key_bytes {
            return Err(ProbeCoordinatorFailure::ResourceLimit {
                resource: CAMPAIGN_KEY,
                requested: declared_campaign_key.len(),
                limit: limits.max_declared_campaign_key_bytes,
            });
        }
        let mut retained = String::new();
        retained
            .try_reserve_exact(declared_campaign_key.len())
            .map_err(|_| ProbeCoordinatorFailure::AllocationFailure {
                resource: CAMPAIGN_KEY,
                requested: declared_campaign_key.len(),
            })?;
        retained.push_str(declared_campaign_key);
        Ok(Self {
            declared_campaign_key: retained.into_boxed_str(),
            declared_probes_per_task,
            interior_margin,
            polynomial_degree_ceiling,
            limits,
        })
    }

    pub(crate) fn declared_campaign_key(&self) -> &str {
        &self.declared_campaign_key
    }

    pub(crate) const fn interior_margin(&self) -> u64 {
        self.interior_margin
    }

    pub(crate) const fn declared_probes_per_task(&self) -> NonZeroUsize {
        self.declared_probes_per_task
    }

    pub(crate) const fn polynomial_degree_ceiling(&self) -> usize {
        self.polynomial_degree_ceiling
    }

    pub(crate) const fn limits(&self) -> ProbeCoordinatorLimits {
        self.limits
    }
}

/// Nonempty bounded probe program for one canonical task.
///
/// Construction freezes the declared probe count before evaluation. The
/// coordinator later requires the complete scheduler outcome census to have
/// exactly this cardinality before any stable-program result can be upgraded
/// to ExhaustedAtConfig.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProbeCoordinatorProbeBatch {
    probes: Box<[CampaignModularProbe]>,
}

impl ProbeCoordinatorProbeBatch {
    pub(crate) fn try_new(
        probes: impl IntoIterator<Item = CampaignModularProbe>,
        config: &ProbeCoordinatorConfig,
    ) -> Result<Self, ProbeCoordinatorFailure> {
        let expected = config.declared_probes_per_task().get();
        let mut retained = Vec::new();
        retained.try_reserve_exact(expected).map_err(|_| {
            ProbeCoordinatorFailure::AllocationFailure {
                resource: PROBES,
                requested: expected,
            }
        })?;
        for probe in probes {
            let requested = retained
                .len()
                .checked_add(1)
                .ok_or(ProbeCoordinatorFailure::ResourceCountOverflow { resource: PROBES })?;
            if requested > expected {
                return Err(ProbeCoordinatorFailure::ProbeCountMismatch {
                    expected,
                    actual: requested,
                });
            }
            retained.push(probe);
        }
        if retained.is_empty() {
            return Err(ProbeCoordinatorFailure::EmptyProbeBatch);
        }
        if retained.len() != expected {
            return Err(ProbeCoordinatorFailure::ProbeCountMismatch {
                expected,
                actual: retained.len(),
            });
        }
        Ok(Self {
            probes: retained.into_boxed_slice(),
        })
    }

    pub(crate) const fn declared_count(&self) -> usize {
        self.probes.len()
    }

    pub(super) fn into_probes(self) -> impl Iterator<Item = CampaignModularProbe> {
        self.probes.into_vec().into_iter()
    }
}

/// One canonical effective-dimension and parent-dimension service class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProbeCoordinatorClass {
    canonical_ordinal: usize,
    effective_dimension: usize,
    parent_free_dimension: usize,
    boundary_codimension: usize,
    profile: BoundarySimplexSamplingProfile,
}

impl ProbeCoordinatorClass {
    pub(crate) const fn canonical_ordinal(self) -> usize {
        self.canonical_ordinal
    }
    pub(crate) const fn effective_dimension(self) -> usize {
        self.effective_dimension
    }
    pub(crate) const fn parent_free_dimension(self) -> usize {
        self.parent_free_dimension
    }
    pub(crate) const fn boundary_codimension(self) -> usize {
        self.boundary_codimension
    }
    pub(crate) const fn profile(self) -> BoundarySimplexSamplingProfile {
        self.profile
    }

    pub(super) const fn new(
        canonical_ordinal: usize,
        effective_dimension: usize,
        parent_free_dimension: usize,
        profile: BoundarySimplexSamplingProfile,
    ) -> Self {
        Self {
            canonical_ordinal,
            effective_dimension,
            parent_free_dimension,
            boundary_codimension: parent_free_dimension - effective_dimension,
            profile,
        }
    }
}

/// Pure canonical class design for one exact partition snapshot.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ProbeCoordinatorClassSchedule {
    present_parent_dimensions: Box<[usize]>,
    classes: Box<[ProbeCoordinatorClass]>,
}

impl ProbeCoordinatorClassSchedule {
    pub(crate) fn present_parent_dimensions(&self) -> &[usize] {
        &self.present_parent_dimensions
    }
    pub(crate) fn classes(&self) -> &[ProbeCoordinatorClass] {
        &self.classes
    }

    pub(super) fn new(
        present_parent_dimensions: Vec<usize>,
        classes: Vec<ProbeCoordinatorClass>,
    ) -> Self {
        Self {
            present_parent_dimensions: present_parent_dimensions.into_boxed_slice(),
            classes: classes.into_boxed_slice(),
        }
    }
}

/// Allocation-free aggregate telemetry. No task, plan, replay, circuit,
/// proposal, owner, or opaque identity is retained here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProbeCoordinatorCensus {
    pub(super) epochs_started: usize,
    pub(super) plans_built: usize,
    pub(super) classes_completed: usize,
    pub(super) task_reports: usize,
    pub(super) no_proposal: usize,
    pub(super) duplicate: usize,
    pub(super) incomplete_proposal: usize,
    pub(super) changed_without_geometric_shrink: usize,
    pub(super) strict_geometric_shrink: usize,
    pub(super) compiler_closed: usize,
    pub(super) invalidated_tickets: usize,
    pub(super) scheduler_budget_stops: usize,
    pub(super) scheduler_rejections: usize,
    pub(super) scheduler_stalls: usize,
    pub(super) scheduler_exact_lift_errors: usize,
    pub(super) canonical_replayed: usize,
    pub(super) canonical_no_modular_hit: usize,
    pub(super) canonical_query_rejections: usize,
    pub(super) canonical_support_did_not_lift: usize,
    pub(super) exact_obstructions: usize,
    pub(super) declared_probes: usize,
    pub(super) scheduler_replayed: usize,
    pub(super) scheduler_support_did_not_lift: usize,
    pub(super) scheduler_sampled_dual: usize,
}

macro_rules! census_accessors {
    ($($name:ident),* $(,)?) => {$(
        pub(crate) const fn $name(self) -> usize { self.$name }
    )*};
}

impl ProbeCoordinatorCensus {
    census_accessors!(
        epochs_started,
        plans_built,
        classes_completed,
        task_reports,
        no_proposal,
        duplicate,
        incomplete_proposal,
        changed_without_geometric_shrink,
        strict_geometric_shrink,
        compiler_closed,
        invalidated_tickets,
        scheduler_budget_stops,
        scheduler_rejections,
        scheduler_stalls,
        scheduler_exact_lift_errors,
        canonical_replayed,
        canonical_no_modular_hit,
        canonical_query_rejections,
        canonical_support_did_not_lift,
        exact_obstructions,
        declared_probes,
        scheduler_replayed,
        scheduler_support_did_not_lift,
        scheduler_sampled_dual,
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProbeCoordinatorTaskLocation {
    pub(super) ledger_revision: u64,
    pub(super) class_ordinal: usize,
    pub(super) effective_dimension: usize,
    pub(super) parent_free_dimension: usize,
    pub(super) boundary_codimension: usize,
    pub(super) task_ordinal: usize,
}

impl ProbeCoordinatorTaskLocation {
    pub(crate) const fn ledger_revision(self) -> u64 {
        self.ledger_revision
    }
    pub(crate) const fn class_ordinal(self) -> usize {
        self.class_ordinal
    }
    pub(crate) const fn effective_dimension(self) -> usize {
        self.effective_dimension
    }
    pub(crate) const fn parent_free_dimension(self) -> usize {
        self.parent_free_dimension
    }
    pub(crate) const fn boundary_codimension(self) -> usize {
        self.boundary_codimension
    }
    pub(crate) const fn task_ordinal(self) -> usize {
        self.task_ordinal
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProbeCoordinatorOwnerMutation {
    ChangedWithoutGeometricShrink,
    StrictGeometricShrink,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProbeCoordinatorNeedsRefinementReason {
    /// Diagnostic only: the full proposal payload was intentionally dropped.
    /// Refinement must rerun this canonical task under a richer config.
    IncompleteProposal {
        exact_obstructions: usize,
    },
    ProbeStalled {
        scheduler_stalls: usize,
    },
    CanonicalQueryRejected {
        canonical_query_rejections: usize,
    },
    /// Diagnostic only: exact obstruction payloads are not retained by this
    /// compact coordinator. The task must be replayed by a refinement owner.
    DiagnosticExactObstructions {
        count: usize,
    },
    ExactCompilerState {
        status: ExactOwnerLedgerCoverStatus,
        uncovered_is_finite: bool,
        missing_terminal_count: usize,
        guard_incomplete_owner_count: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProbeCoordinatorNeedsRefinement {
    pub(super) census: ProbeCoordinatorCensus,
    pub(super) location: Option<ProbeCoordinatorTaskLocation>,
    pub(super) reason: ProbeCoordinatorNeedsRefinementReason,
}

impl ProbeCoordinatorNeedsRefinement {
    pub(crate) const fn census(self) -> ProbeCoordinatorCensus {
        self.census
    }
    pub(crate) const fn location(self) -> Option<ProbeCoordinatorTaskLocation> {
        self.location
    }
    pub(crate) const fn reason(self) -> ProbeCoordinatorNeedsRefinementReason {
        self.reason
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProbeCoordinatorOperationalReason {
    EpochLimit {
        requested: usize,
        limit: usize,
    },
    PlanLimit {
        requested: usize,
        limit: usize,
    },
    TaskReportLimit {
        requested: usize,
        limit: usize,
    },
    IncompleteProbeExecution {
        scheduler_budget_stops: usize,
        scheduler_rejections: usize,
        scheduler_exact_lift_errors: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProbeCoordinatorOperationalStop {
    pub(super) census: ProbeCoordinatorCensus,
    pub(super) location: Option<ProbeCoordinatorTaskLocation>,
    pub(super) reason: ProbeCoordinatorOperationalReason,
}

impl ProbeCoordinatorOperationalStop {
    pub(crate) const fn census(self) -> ProbeCoordinatorCensus {
        self.census
    }
    pub(crate) const fn location(self) -> Option<ProbeCoordinatorTaskLocation> {
        self.location
    }
    pub(crate) const fn reason(self) -> ProbeCoordinatorOperationalReason {
        self.reason
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProbeCoordinatorOwnerSetChanged {
    pub(super) census: ProbeCoordinatorCensus,
    pub(super) location: ProbeCoordinatorTaskLocation,
    pub(super) mutation: ProbeCoordinatorOwnerMutation,
    pub(super) before_revision: u64,
    pub(super) after_revision: u64,
    pub(super) invalidated_tickets: usize,
}

impl ProbeCoordinatorOwnerSetChanged {
    pub(crate) const fn census(self) -> ProbeCoordinatorCensus {
        self.census
    }
    pub(crate) const fn location(self) -> ProbeCoordinatorTaskLocation {
        self.location
    }
    pub(crate) const fn mutation(self) -> ProbeCoordinatorOwnerMutation {
        self.mutation
    }
    pub(crate) const fn before_revision(self) -> u64 {
        self.before_revision
    }
    pub(crate) const fn after_revision(self) -> u64 {
        self.after_revision
    }
    pub(crate) const fn invalidated_tickets(self) -> usize {
        self.invalidated_tickets
    }
}

#[derive(Debug)]
pub(crate) struct ProbeCoordinatorFailureStop {
    pub(super) census: ProbeCoordinatorCensus,
    pub(super) failure: ProbeCoordinatorFailure,
}

impl ProbeCoordinatorFailureStop {
    pub(crate) const fn census(&self) -> ProbeCoordinatorCensus {
        self.census
    }
    pub(crate) const fn failure(&self) -> &ProbeCoordinatorFailure {
        &self.failure
    }
}

/// Terminal boundary of one window-one coordinator invocation.
///
/// Deliberately no common is_closed method exists. CompilerClosed retains a
/// revalidated opaque ledger identity beside scalar exact-compiler telemetry.
/// The copied telemetry is not publication authority: sealing must still
/// consume the live ledger owners through the existing artifact boundary.
#[derive(Debug)]
pub(crate) enum ProbeCoordinatorStop {
    CompilerClosed {
        census: ProbeCoordinatorCensus,
        ledger_snapshot: ExactOwnerLedgerSnapshotIdentity,
        exact: ExactOwnerCoverSnapshot,
    },
    OwnerSetChanged(ProbeCoordinatorOwnerSetChanged),
    NeedsRefinement(ProbeCoordinatorNeedsRefinement),
    OperationallyBounded(ProbeCoordinatorOperationalStop),
    Failed(ProbeCoordinatorFailureStop),
    ExhaustedAtConfig {
        census: ProbeCoordinatorCensus,
        ledger_snapshot: ExactOwnerLedgerSnapshotIdentity,
        exact: ExactOwnerCoverSnapshot,
        ledger_revision: u64,
        completed_classes: usize,
        completed_tasks: usize,
    },
}

impl ProbeCoordinatorStop {
    pub(crate) const fn census(&self) -> ProbeCoordinatorCensus {
        match self {
            Self::CompilerClosed { census, .. } | Self::ExhaustedAtConfig { census, .. } => *census,
            Self::OwnerSetChanged(stop) => stop.census,
            Self::NeedsRefinement(stop) => stop.census,
            Self::OperationallyBounded(stop) => stop.census,
            Self::Failed(stop) => stop.census,
        }
    }
}

/// Stateful aggregate budget and telemetry owner. During a call, live memory
/// is O(partition + classes + largest materialized class plan + one evaluated
/// task). It retains no completed report history and no task, plan, circuit,
/// proposal, owner, or ledger identity between calls.
#[derive(Debug)]
pub(crate) struct BoundaryProbeCoordinator {
    pub(super) config: ProbeCoordinatorConfig,
    pub(super) census: ProbeCoordinatorCensus,
}

impl BoundaryProbeCoordinator {
    pub(crate) const fn new(config: ProbeCoordinatorConfig) -> Self {
        Self {
            config,
            census: ProbeCoordinatorCensus {
                epochs_started: 0,
                plans_built: 0,
                classes_completed: 0,
                task_reports: 0,
                no_proposal: 0,
                duplicate: 0,
                incomplete_proposal: 0,
                changed_without_geometric_shrink: 0,
                strict_geometric_shrink: 0,
                compiler_closed: 0,
                invalidated_tickets: 0,
                scheduler_budget_stops: 0,
                scheduler_rejections: 0,
                scheduler_stalls: 0,
                scheduler_exact_lift_errors: 0,
                canonical_replayed: 0,
                canonical_no_modular_hit: 0,
                canonical_query_rejections: 0,
                canonical_support_did_not_lift: 0,
                exact_obstructions: 0,
                declared_probes: 0,
                scheduler_replayed: 0,
                scheduler_support_did_not_lift: 0,
                scheduler_sampled_dual: 0,
            },
        }
    }

    pub(crate) const fn config(&self) -> &ProbeCoordinatorConfig {
        &self.config
    }

    pub(crate) const fn census(&self) -> ProbeCoordinatorCensus {
        self.census
    }
}
