use crate::foundry::completion::source_discovery::cover_delta::{
    ExactOwnerCoverSnapshot, ExactOwnerLedgerCoverStatus, ExactOwnerLedgerSnapshotIdentity,
};
use crate::foundry::completion::source_discovery::scheduler::ProbeLocalRejectionSummary;
use crate::foundry::completion::source_discovery::{CampaignLimits, CampaignModularProbe};
use crate::sector::CoordinatePriority;
use symbolica::prelude::Integer;

use super::super::super::boundary_simplex::BoundarySimplexSamplingProfile;
use super::{ProbeCoordinatorFailure, ProbeCoordinatorLimits};

const PROBES: &str = "fixed task-relative probes";

/// Immutable semantics of one bounded boundary probe program.
///
/// Probe templates are retained in exact scheduling order.  Family, source,
/// predecessor, sector, ordering, and concrete-ledger scope are bound later by
/// [`BoundaryProbeCoordinator::try_new`]; no caller-authored string stands in
/// for those typed values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProbeCoordinatorConfig {
    probes: Box<[TaskRelativeModularProbe]>,
    interior_margin: u64,
    polynomial_degree_ceiling: usize,
    discovery_coordinate_priority: Option<CoordinatePriority>,
    limits: ProbeCoordinatorLimits,
}

impl ProbeCoordinatorConfig {
    pub(crate) fn try_new(
        probes: impl IntoIterator<Item = TaskRelativeModularProbe>,
        interior_margin: u64,
        polynomial_degree_ceiling: usize,
        limits: ProbeCoordinatorLimits,
    ) -> Result<Self, ProbeCoordinatorFailure> {
        if interior_margin == 0 {
            return Err(ProbeCoordinatorFailure::ZeroInteriorMargin);
        }
        let mut retained = Vec::new();
        for probe in probes {
            let requested = retained
                .len()
                .checked_add(1)
                .ok_or(ProbeCoordinatorFailure::ResourceCountOverflow { resource: PROBES })?;
            if requested > limits.max_probes_per_task {
                return Err(ProbeCoordinatorFailure::ResourceLimit {
                    resource: PROBES,
                    requested,
                    limit: limits.max_probes_per_task,
                });
            }
            retained
                .try_reserve(1)
                .map_err(|_| ProbeCoordinatorFailure::AllocationFailure {
                    resource: PROBES,
                    requested,
                })?;
            retained.push(probe);
        }
        if retained.is_empty() {
            return Err(ProbeCoordinatorFailure::EmptyProbeProgram);
        }
        Ok(Self {
            probes: retained.into_boxed_slice(),
            interior_margin,
            polynomial_degree_ceiling,
            discovery_coordinate_priority: None,
            limits,
        })
    }

    /// Add a proposal-only coordinate chronology.
    ///
    /// Natural priority is normalized to the allocation-free canonical path.
    /// A nonnatural priority is validated against the bound family arity when
    /// the coordinator is constructed.
    pub(crate) fn with_discovery_coordinate_priority(
        mut self,
        priority: CoordinatePriority,
    ) -> Self {
        self.discovery_coordinate_priority = (!priority.is_natural()).then_some(priority);
        self
    }

    pub(crate) fn probes(&self) -> &[TaskRelativeModularProbe] {
        &self.probes
    }

    pub(crate) const fn interior_margin(&self) -> u64 {
        self.interior_margin
    }

    pub(crate) fn probes_per_task(&self) -> usize {
        self.probes.len()
    }

    pub(crate) const fn polynomial_degree_ceiling(&self) -> usize {
        self.polynomial_degree_ceiling
    }

    pub(crate) const fn discovery_coordinate_priority(&self) -> Option<&CoordinatePriority> {
        self.discovery_coordinate_priority.as_ref()
    }

    pub(crate) const fn limits(&self) -> ProbeCoordinatorLimits {
        self.limits
    }
}

/// One immutable modular sample relative to every canonical task.
///
/// `chart_offsets` are nonnegative chart-space displacements from the task's
/// canonical base-index sample. On a restricted boundary face, fixed axes use
/// chart zero and require zero offset; only remaining symbolic axes use the
/// first interior chart point plus the supplied offset. No topology name,
/// loop count, sector, or family-specific dispatch enters this value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TaskRelativeModularProbe {
    template: CampaignModularProbe,
}

impl TaskRelativeModularProbe {
    pub(crate) fn try_new(
        modulus: u64,
        base_parameters: impl IntoIterator<Item = i64>,
        chart_offsets: impl IntoIterator<Item = u64>,
        limits: CampaignLimits,
    ) -> Result<Self, ProbeCoordinatorFailure> {
        if modulus.is_multiple_of(2) {
            return Err(ProbeCoordinatorFailure::UnsupportedEvenModulus { modulus });
        }
        // Symbolica's deterministic u64 primality path is the canonical
        // finite-field admission check used by the downstream scheduler.
        if modulus == u64::MAX || !Integer::from(modulus).is_prime(0) {
            return Err(ProbeCoordinatorFailure::NonPrimeModulus { modulus });
        }
        Ok(Self {
            template: CampaignModularProbe::try_new(
                modulus,
                base_parameters,
                chart_offsets,
                limits,
            )
            .map_err(ProbeCoordinatorFailure::Probe)?,
        })
    }

    pub(crate) const fn modulus(&self) -> u64 {
        self.template.modulus()
    }

    pub(crate) fn base_parameters(&self) -> &[i64] {
        self.template.base_parameters()
    }

    pub(crate) fn chart_offsets(&self) -> &[u64] {
        self.template.chart_coordinates()
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
    pub(super) first_scheduler_rejection: Option<ProbeLocalRejectionSummary>,
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

    pub(crate) const fn first_scheduler_rejection(self) -> Option<ProbeLocalRejectionSummary> {
        self.first_scheduler_rejection
    }
}

/// Semantic origin of one diagnostic task location.
///
/// Requested-domain execution deliberately does not impersonate a boundary
/// service class: its request ordinal remains typed here even though the
/// compact stop shares the boundary coordinator's scalar location envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProbeCoordinatorTaskLocationKind {
    BoundarySimplex,
    RequestedDomain { requested_ordinal: usize },
}

/// Detached diagnostic coordinates of the task that produced a compact stop.
///
/// For [`ProbeCoordinatorTaskLocationKind::RequestedDomain`], the class and
/// dimension fields are a deterministic display projection only. They carry
/// no boundary exhaustion or closure authority. Closure remains exclusively
/// the live exact ledger compiler's status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProbeCoordinatorTaskLocation {
    pub(super) kind: ProbeCoordinatorTaskLocationKind,
    pub(super) ledger_revision: u64,
    pub(super) class_ordinal: usize,
    pub(super) effective_dimension: usize,
    pub(super) parent_free_dimension: usize,
    pub(super) boundary_codimension: usize,
    pub(super) task_ordinal: usize,
}

/// Deterministic ordinal next-service position across replanning epochs.
///
/// This retains only canonical scalar ordinals, never a task, plan, geometry
/// identity, or ledger authority. Every epoch normalizes it against the newly
/// rebuilt class and task counts before use. Consequently it is fair for a
/// stable/reindex-preserving plan, but it cannot certify visitation of an
/// external fixed target itinerary when partition mutations renumber tasks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProbeCoordinatorFairCursor {
    class_ordinal: usize,
    task_ordinal: usize,
}

impl ProbeCoordinatorFairCursor {
    pub(crate) fn class_start(self, class_count: usize) -> usize {
        self.class_ordinal % class_count
    }

    pub(crate) fn task_start(self, task_count: usize) -> usize {
        self.task_ordinal % task_count
    }

    pub(crate) fn advance_after(
        &mut self,
        class_ordinal: usize,
        task_ordinal: usize,
        task_count: usize,
        class_count: usize,
    ) {
        debug_assert!(task_ordinal < task_count);
        debug_assert!(class_ordinal < class_count);
        if task_ordinal + 1 < task_count {
            self.class_ordinal = class_ordinal;
            self.task_ordinal = task_ordinal + 1;
        } else {
            self.class_ordinal = (class_ordinal + 1) % class_count;
            self.task_ordinal = 0;
        }
    }
}

impl ProbeCoordinatorTaskLocation {
    pub(crate) const fn kind(self) -> ProbeCoordinatorTaskLocationKind {
        self.kind
    }
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
        terminal_scheduler_rejection: Option<ProbeLocalRejectionSummary>,
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

/// Stop of the explicit requested-domain phase.
///
/// `PhaseCompleted` means only that every residual task in this one immutable
/// requested plan was serviced without changing the owner set. It is not
/// search exhaustion and carries no closure authority. A composite driver
/// must continue with freshly planned boundary service; only
/// `CompilerClosed` reflects the live exact compiler's closed status.
#[derive(Debug)]
pub(crate) enum RequestedProbeCoordinatorStop {
    CompilerClosed {
        census: ProbeCoordinatorCensus,
        ledger_snapshot: ExactOwnerLedgerSnapshotIdentity,
        exact: ExactOwnerCoverSnapshot,
    },
    OwnerSetChanged(ProbeCoordinatorOwnerSetChanged),
    NeedsRefinement(ProbeCoordinatorNeedsRefinement),
    OperationallyBounded(ProbeCoordinatorOperationalStop),
    Failed(ProbeCoordinatorFailureStop),
    PhaseCompleted {
        census: ProbeCoordinatorCensus,
        ledger_snapshot: ExactOwnerLedgerSnapshotIdentity,
        ledger_revision: u64,
        completed_tasks: usize,
    },
}

impl RequestedProbeCoordinatorStop {
    pub(crate) const fn census(&self) -> ProbeCoordinatorCensus {
        match self {
            Self::CompilerClosed { census, .. } | Self::PhaseCompleted { census, .. } => *census,
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
/// proposal, or owner between calls. It deliberately retains the immutable
/// adapter, fixed probe templates, and process-local nonce of its bound ledger.
#[derive(Debug)]
pub(crate) struct BoundaryProbeCoordinator<'inputs, 'sources, 'family> {
    pub(super) config: ProbeCoordinatorConfig,
    pub(super) adapter: super::super::ProbeCampaignAdapter<'inputs, 'sources, 'family>,
    pub(super) bound_ledger: ExactOwnerLedgerSnapshotIdentity,
    pub(super) planner_scope_key: Box<str>,
    pub(super) census: ProbeCoordinatorCensus,
    pub(super) fair_cursor: ProbeCoordinatorFairCursor,
}

impl BoundaryProbeCoordinator<'_, '_, '_> {
    pub(super) const fn empty_census() -> ProbeCoordinatorCensus {
        ProbeCoordinatorCensus {
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
            first_scheduler_rejection: None,
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
        }
    }

    pub(crate) const fn config(&self) -> &ProbeCoordinatorConfig {
        &self.config
    }

    pub(crate) const fn census(&self) -> ProbeCoordinatorCensus {
        self.census
    }
}
