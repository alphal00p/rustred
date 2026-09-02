use crate::foundry::completion::UncoveredPartition;
use crate::foundry::completion::frame::admission::{
    ExactOwnerCoverObstructionKind, ExactOwnerCoverStatus,
};
use crate::foundry::completion::source_discovery::CampaignModularProbe;
use crate::foundry::completion::source_discovery::boundary_simplex::{
    BoundarySimplexPlan, BoundarySimplexScopePartition, BoundarySimplexTask,
    try_plan_boundary_simplex_samples,
};
use crate::foundry::completion::source_discovery::cover_delta::{
    CanonicalExactOwnerLedger, ExactOwnerCoverSnapshot, ExactOwnerLedgerCoverStatus,
    ExactOwnerLedgerSnapshotIdentity,
};
use crate::foundry::completion::source_discovery::leader_walk::{
    RequestedDomainPlan, RequestedDomainTask,
};
use crate::sector::Mask;

use super::super::ProbeCampaignAdapter;
use super::chronology::DiscoveryTaskChronology;
use super::compact::{
    CompactTaskAction, CompactTaskCommit, operational_reason, search_refinement_reason,
    try_increment, try_reserve_evaluated_task, validate_live_effect,
};
use super::schedule::try_build_class_schedule;
use super::{
    BoundaryProbeCoordinator, ProbeCoordinatorCensus, ProbeCoordinatorFailure,
    ProbeCoordinatorFailureStop, ProbeCoordinatorFairCursor, ProbeCoordinatorNeedsRefinement,
    ProbeCoordinatorNeedsRefinementReason, ProbeCoordinatorOperationalReason,
    ProbeCoordinatorOperationalStop, ProbeCoordinatorOwnerSetChanged, ProbeCoordinatorStop,
    ProbeCoordinatorTaskLocation, ProbeCoordinatorTaskLocationKind, RequestedProbeCoordinatorStop,
};

const EPOCHS: &str = "epochs";
const PLANS: &str = "plans";
const TASK_REPORTS: &str = "task reports";
const INVALIDATED_TICKETS: &str = "invalidated tickets";
const CENSUS: &str = "scalar census";
const PROBES: &str = "fixed task-relative probes";
const PROBE_COORDINATES: &str = "fixed task-relative probe coordinate cells";
const PLANNER_SCOPE_KEY: &str = "internal planner scope key bytes";
const PLANNER_SCOPE_PREFIX: &str = "rustred.boundary-probe-coordinator.scope.v1:";

mod sealed_task {
    pub trait Sealed {}
}

/// Planner-authenticated task kinds admitted by the compact coordinator.
///
/// This view supplies coefficient-sample geometry only. It cannot mint source
/// rows, proposal support, exact owners, or closure authority; those remain in
/// the shared regenerated-source replay and exact ledger compiler.
pub(super) trait CoordinatedProbeTask:
    super::super::ProbeCampaignPlannedTask + sealed_task::Sealed
{
    fn try_chart_coordinate(
        &self,
        position: usize,
        offset: u64,
        probe_ordinal: usize,
    ) -> Result<u64, ProbeCoordinatorFailure>;
}

impl sealed_task::Sealed for BoundarySimplexTask {}

impl CoordinatedProbeTask for BoundarySimplexTask {
    fn try_chart_coordinate(
        &self,
        position: usize,
        offset: u64,
        probe_ordinal: usize,
    ) -> Result<u64, ProbeCoordinatorFailure> {
        if self
            .key()
            .remaining_axes()
            .binary_search(&position)
            .is_err()
        {
            if offset != 0 {
                return Err(ProbeCoordinatorFailure::NonzeroRestrictedProbeOffset {
                    probe_ordinal,
                    coordinate: position,
                    offset,
                });
            }
            return Ok(0);
        }
        1_u64
            .checked_add(offset)
            .ok_or(ProbeCoordinatorFailure::ProbeChartCoordinateOverflow {
                probe_ordinal,
                coordinate: position,
            })
    }
}

impl sealed_task::Sealed for RequestedDomainTask {}

impl CoordinatedProbeTask for RequestedDomainTask {
    fn try_chart_coordinate(
        &self,
        position: usize,
        offset: u64,
        probe_ordinal: usize,
    ) -> Result<u64, ProbeCoordinatorFailure> {
        if self.key().symbolic_axes().binary_search(&position).is_err() {
            if offset != 0 {
                return Err(ProbeCoordinatorFailure::NonzeroRestrictedProbeOffset {
                    probe_ordinal,
                    coordinate: position,
                    offset,
                });
            }
            return Ok(0);
        }

        let leader = self.leader()[position];
        match self.key().residual_domain_upper()[position] {
            Some(upper) => {
                let available =
                    upper
                        .checked_sub(leader)
                        .ok_or(ProbeCoordinatorFailure::Invariant {
                            detail: "requested-domain residual upper endpoint precedes its leader",
                        })?;
                let origin = u64::from(available != 0);
                // The chart sample is task-relative: zero is the sector
                // corner and the finite residual width is its hard ceiling.
                // Clamp rather than sampling beyond the authenticated
                // residual rectangle.
                Ok(origin + offset.min(available - origin))
            }
            None => 1_u64.checked_add(offset).ok_or(
                ProbeCoordinatorFailure::ProbeChartCoordinateOverflow {
                    probe_ordinal,
                    coordinate: position,
                },
            ),
        }
    }
}

#[derive(Debug)]
pub(super) enum ProbeCoordinatorDriveStop {
    CompilerClosed {
        census: ProbeCoordinatorCensus,
        exact: ExactOwnerCoverSnapshot,
    },
    OwnerSetChanged(ProbeCoordinatorOwnerSetChanged),
    NeedsRefinement(ProbeCoordinatorNeedsRefinement),
    OperationallyBounded(ProbeCoordinatorOperationalStop),
    Failed(ProbeCoordinatorFailureStop),
    StableProgramCompleted {
        census: ProbeCoordinatorCensus,
        ledger_revision: u64,
        completed_classes: usize,
        completed_tasks: usize,
    },
}

impl ProbeCoordinatorDriveStop {
    pub(super) const fn census(&self) -> ProbeCoordinatorCensus {
        match self {
            Self::CompilerClosed { census, .. } | Self::StableProgramCompleted { census, .. } => {
                *census
            }
            Self::OwnerSetChanged(stop) => stop.census,
            Self::NeedsRefinement(stop) => stop.census,
            Self::OperationallyBounded(stop) => stop.census,
            Self::Failed(stop) => stop.census,
        }
    }
}

impl<'inputs, 'sources, 'family> BoundaryProbeCoordinator<'inputs, 'sources, 'family> {
    /// Bind one fixed probe program, semantic adapter, and concrete exact
    /// ledger authority. The retained snapshot identity contributes only its
    /// process-local ledger nonce; later revisions of that same ledger remain
    /// valid replanning epochs.
    pub(crate) fn try_new(
        config: super::ProbeCoordinatorConfig,
        adapter: ProbeCampaignAdapter<'inputs, 'sources, 'family>,
        ledger: &CanonicalExactOwnerLedger,
    ) -> Result<Self, ProbeCoordinatorFailure> {
        adapter.validate_ledger_scope(ledger)?;
        validate_probe_program(&config, &adapter)?;
        if let Some(priority) = config.discovery_coordinate_priority()
            && priority.arity() != adapter.probe_chart_arity()
        {
            return Err(
                ProbeCoordinatorFailure::WrongDiscoveryCoordinatePriorityArity {
                    expected: adapter.probe_chart_arity(),
                    actual: priority.arity(),
                },
            );
        }
        let planner_scope_key = try_build_planner_scope_key(&config, ledger)?;
        Ok(Self {
            config,
            adapter,
            bound_ledger: ledger.snapshot_identity(),
            planner_scope_key,
            census: Self::empty_census(),
            fair_cursor: ProbeCoordinatorFairCursor::default(),
        })
    }

    /// Run one immutable ledger epoch in canonical order.
    ///
    /// A changed owner set ends this call immediately. No plan or ticket is
    /// retained; calling this method again necessarily clones the new exact
    /// partition and creates fresh opaque planner identities.
    pub(crate) fn try_run_boundary_epoch(
        &mut self,
        ledger: &mut CanonicalExactOwnerLedger,
    ) -> ProbeCoordinatorStop {
        if !self
            .bound_ledger
            .same_ledger_as(&ledger.snapshot_identity())
        {
            return failed_public(
                self.census,
                crate::foundry::completion::source_discovery::cover_delta::ExactOwnerCoverDeltaError::ForeignLedgerSnapshotIdentity.into(),
            );
        }
        let initial = ledger.snapshot();
        if initial.status().is_compiler_closed() {
            let ledger_snapshot = ledger.snapshot_identity();
            if let Err(error) = ledger.try_require_current_snapshot(&ledger_snapshot) {
                return failed_public(self.census, error.into());
            }
            return match try_increment(&mut self.census.compiler_closed, CENSUS) {
                Ok(()) => ProbeCoordinatorStop::CompilerClosed {
                    census: self.census,
                    ledger_snapshot,
                    exact: initial,
                },
                Err(failure) => failed_public(self.census, failure),
            };
        }
        let partition = match ledger.try_clone_uncovered_partition() {
            Ok(partition) => partition,
            Err(error) => return failed_public(self.census, error.into()),
        };
        let snapshot_identity = ledger.snapshot_identity();
        let sector = ledger.sector().clone();
        let revision = initial.revision().get();
        let expected_probes_per_task = self.config.probes_per_task();

        let config = &self.config;
        let adapter = &self.adapter;
        let planner_scope_key = self.planner_scope_key.as_ref();

        let drive = try_drive_partition(
            config,
            planner_scope_key,
            &mut self.census,
            &mut self.fair_cursor,
            revision,
            &sector,
            &partition,
            |plan, task, baseline_census, requested_report, invalidated_tickets| {
                try_execute_coordinated_task(
                    config,
                    adapter,
                    plan,
                    task,
                    ledger,
                    &snapshot_identity,
                    baseline_census,
                    requested_report,
                    invalidated_tickets,
                    expected_probes_per_task,
                )
            },
        );
        upgrade_drive_stop(drive, ledger, &snapshot_identity)
    }

    /// Execute one already planned requested-domain phase in canonical task
    /// order against the coordinator's live exact ledger.
    ///
    /// The caller must rebuild the plan after every owner mutation. Stable
    /// completion is returned as [`RequestedProbeCoordinatorStop::PhaseCompleted`]
    /// and is deliberately not upgraded to boundary-program exhaustion: a
    /// composite campaign must next service the generic boundary complement.
    pub(crate) fn try_run_requested_plan(
        &mut self,
        plan: &RequestedDomainPlan,
        ledger: &mut CanonicalExactOwnerLedger,
    ) -> RequestedProbeCoordinatorStop {
        if !self
            .bound_ledger
            .same_ledger_as(&ledger.snapshot_identity())
        {
            return requested_failed_public(
                self.census,
                crate::foundry::completion::source_discovery::cover_delta::ExactOwnerCoverDeltaError::ForeignLedgerSnapshotIdentity.into(),
            );
        }
        let initial = ledger.snapshot();
        if initial.status().is_compiler_closed() {
            let live_identity = ledger.snapshot_identity();
            if let Err(error) = ledger.try_require_current_snapshot(&live_identity) {
                return requested_failed_public(self.census, error.into());
            }
            return match try_increment(&mut self.census.compiler_closed, CENSUS) {
                Ok(()) => RequestedProbeCoordinatorStop::CompilerClosed {
                    census: self.census,
                    ledger_snapshot: live_identity,
                    exact: initial,
                },
                Err(failure) => requested_failed_public(self.census, failure),
            };
        }

        let snapshot_identity = ledger.snapshot_identity();
        let expected_probes_per_task = self.config.probes_per_task();
        let config = &self.config;
        let adapter = &self.adapter;
        let drive = try_drive_requested_plan(
            config,
            &mut self.census,
            ledger.revision().get(),
            plan,
            |plan, task, baseline_census, requested_report, invalidated_tickets| {
                try_execute_coordinated_task(
                    config,
                    adapter,
                    plan,
                    task,
                    ledger,
                    &snapshot_identity,
                    baseline_census,
                    requested_report,
                    invalidated_tickets,
                    expected_probes_per_task,
                )
            },
        );
        upgrade_requested_drive_stop(drive, ledger, &snapshot_identity)
    }
}

#[allow(clippy::too_many_arguments)]
fn try_execute_coordinated_task<Task: CoordinatedProbeTask>(
    config: &super::ProbeCoordinatorConfig,
    adapter: &ProbeCampaignAdapter<'_, '_, '_>,
    plan: &Task::Plan,
    task: &Task,
    ledger: &mut CanonicalExactOwnerLedger,
    epoch_identity: &ExactOwnerLedgerSnapshotIdentity,
    baseline_census: ProbeCoordinatorCensus,
    requested_report: usize,
    invalidated_tickets: usize,
    expected_probes_per_task: usize,
) -> Result<CompactTaskCommit, ProbeCoordinatorFailure> {
    ledger.try_require_current_snapshot(epoch_identity)?;
    let before = ledger.snapshot();
    let binding = adapter.try_bind_task(plan, task, ledger)?;
    let probes = try_materialize_probes(config, adapter, task)?;
    let evaluated = adapter.try_evaluate_task(binding, ledger, probes)?;
    // Every fallible replay/census join and every possible scalar counter
    // update is checked while the exact ledger is still immutable. Serial
    // application below is the transaction's sole mutation boundary.
    let reservation = try_reserve_evaluated_task(
        &evaluated,
        baseline_census,
        requested_report,
        invalidated_tickets,
    )?;
    if reservation.declared_probes() != expected_probes_per_task {
        return Err(ProbeCoordinatorFailure::Invariant {
            detail: "scheduler outcome total differed from the fixed task probes",
        });
    }
    let report = adapter.try_apply_evaluated_task(evaluated, ledger)?;
    let commit = reservation.finish_report(&report);
    let after = ledger.snapshot();
    debug_assert!(validate_live_effect(before, after, commit.compact.action).is_ok());
    drop(report);
    Ok(commit)
}

fn validate_probe_program(
    config: &super::ProbeCoordinatorConfig,
    adapter: &ProbeCampaignAdapter<'_, '_, '_>,
) -> Result<(), ProbeCoordinatorFailure> {
    let scheduler = adapter.limits().replay.scheduler;
    let probe_count = config.probes_per_task();
    for (resource, limit) in [
        (PROBES, scheduler.max_probes),
        (
            "retained fixed probe outcomes",
            scheduler.max_retained_outcomes,
        ),
    ] {
        if probe_count > limit {
            return Err(ProbeCoordinatorFailure::ResourceLimit {
                resource,
                requested: probe_count,
                limit,
            });
        }
    }

    let expected_base = adapter.probe_base_parameter_count();
    let expected_chart = adapter.probe_chart_arity();
    let mut coordinate_cells = 0usize;
    for (probe_ordinal, probe) in config.probes().iter().enumerate() {
        if probe.base_parameters().len() != expected_base {
            return Err(ProbeCoordinatorFailure::WrongProbeBaseParameterArity {
                probe_ordinal,
                expected: expected_base,
                actual: probe.base_parameters().len(),
            });
        }
        if probe.chart_offsets().len() != expected_chart {
            return Err(ProbeCoordinatorFailure::WrongProbeChartOffsetArity {
                probe_ordinal,
                expected: expected_chart,
                actual: probe.chart_offsets().len(),
            });
        }
        let probe_cells = probe
            .base_parameters()
            .len()
            .checked_add(probe.chart_offsets().len())
            .ok_or(ProbeCoordinatorFailure::ResourceCountOverflow {
                resource: PROBE_COORDINATES,
            })?;
        if probe_cells > scheduler.campaign.max_retained_probe_coordinates {
            return Err(ProbeCoordinatorFailure::ResourceLimit {
                resource: PROBE_COORDINATES,
                requested: probe_cells,
                limit: scheduler.campaign.max_retained_probe_coordinates,
            });
        }
        coordinate_cells = coordinate_cells.checked_add(probe_cells).ok_or(
            ProbeCoordinatorFailure::ResourceCountOverflow {
                resource: PROBE_COORDINATES,
            },
        )?;
    }
    if coordinate_cells > scheduler.max_retained_probe_coordinate_cells {
        return Err(ProbeCoordinatorFailure::ResourceLimit {
            resource: PROBE_COORDINATES,
            requested: coordinate_cells,
            limit: scheduler.max_retained_probe_coordinate_cells,
        });
    }
    Ok(())
}

fn try_build_planner_scope_key(
    config: &super::ProbeCoordinatorConfig,
    ledger: &CanonicalExactOwnerLedger,
) -> Result<Box<str>, ProbeCoordinatorFailure> {
    let predecessor = ledger.predecessor_snapshot().id().as_str();
    let ordering = ledger.ordering().stable_id();
    let predecessor_length = predecessor.len().to_string();
    let ordering_length = ordering.len().to_string();
    let pieces = [
        PLANNER_SCOPE_PREFIX,
        predecessor_length.as_str(),
        "#",
        predecessor,
        ":",
        ordering_length.as_str(),
        "#",
        ordering.as_str(),
    ];
    let mut requested = 0usize;
    for piece in pieces {
        requested = requested.checked_add(piece.len()).ok_or(
            ProbeCoordinatorFailure::ResourceCountOverflow {
                resource: PLANNER_SCOPE_KEY,
            },
        )?;
    }
    let limit = config.limits().boundary_plan.max_aggregate_scope_key_bytes;
    if requested > limit {
        return Err(ProbeCoordinatorFailure::ResourceLimit {
            resource: PLANNER_SCOPE_KEY,
            requested,
            limit,
        });
    }
    let mut key = String::new();
    key.try_reserve_exact(requested)
        .map_err(|_| ProbeCoordinatorFailure::AllocationFailure {
            resource: PLANNER_SCOPE_KEY,
            requested,
        })?;
    for piece in pieces {
        key.push_str(piece);
    }
    debug_assert_eq!(key.len(), requested);
    Ok(key.into_boxed_str())
}

pub(super) fn try_materialize_probes<Task: CoordinatedProbeTask>(
    config: &super::ProbeCoordinatorConfig,
    adapter: &ProbeCampaignAdapter<'_, '_, '_>,
    task: &Task,
) -> Result<Vec<CampaignModularProbe>, ProbeCoordinatorFailure> {
    let probe_count = config.probes_per_task();
    let mut probes = Vec::new();
    probes.try_reserve_exact(probe_count).map_err(|_| {
        ProbeCoordinatorFailure::AllocationFailure {
            resource: PROBES,
            requested: probe_count,
        }
    })?;
    for (probe_ordinal, template) in config.probes().iter().enumerate() {
        let mut chart_coordinates = Vec::new();
        chart_coordinates
            .try_reserve_exact(task.lattice_target().len())
            .map_err(|_| ProbeCoordinatorFailure::AllocationFailure {
                resource: PROBE_COORDINATES,
                requested: task.lattice_target().len(),
            })?;
        for (coordinate, &offset) in template.chart_offsets().iter().enumerate() {
            chart_coordinates.push(task.try_chart_coordinate(coordinate, offset, probe_ordinal)?);
        }
        probes.push(CampaignModularProbe::try_new(
            template.modulus(),
            template.base_parameters().iter().copied(),
            chart_coordinates,
            adapter.limits().replay.scheduler.campaign,
        )?);
    }
    Ok(probes)
}

/// Drive one immutable requested-domain plan in its planner-defined canonical
/// order. Unlike boundary discovery, explicit request chronology is semantic
/// and must not be rewritten by [`DiscoveryTaskChronology`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RequestedLocalObstruction {
    NeedsRefinement {
        location: ProbeCoordinatorTaskLocation,
        reason: ProbeCoordinatorNeedsRefinementReason,
    },
    OperationallyBounded {
        location: ProbeCoordinatorTaskLocation,
        reason: ProbeCoordinatorOperationalReason,
    },
}

impl RequestedLocalObstruction {
    fn into_stop(self, census: ProbeCoordinatorCensus) -> ProbeCoordinatorDriveStop {
        match self {
            Self::NeedsRefinement { location, reason } => {
                needs_refinement(census, Some(location), reason)
            }
            Self::OperationallyBounded { location, reason } => {
                operationally_bounded(census, Some(location), reason)
            }
        }
    }
}

fn requested_local_obstruction(
    compact: super::compact::CompactTaskResult,
    location: ProbeCoordinatorTaskLocation,
) -> Option<RequestedLocalObstruction> {
    if let Some(reason) = operational_reason(compact) {
        return Some(RequestedLocalObstruction::OperationallyBounded { location, reason });
    }
    if let Some(reason) = search_refinement_reason(compact) {
        return Some(RequestedLocalObstruction::NeedsRefinement { location, reason });
    }
    match compact.action {
        CompactTaskAction::NoProposal | CompactTaskAction::Duplicate
            if compact.evidence.exact_obstructions != 0 =>
        {
            Some(RequestedLocalObstruction::NeedsRefinement {
                location,
                reason: ProbeCoordinatorNeedsRefinementReason::DiagnosticExactObstructions {
                    count: compact.evidence.exact_obstructions,
                },
            })
        }
        CompactTaskAction::IncompleteProposal => Some(RequestedLocalObstruction::NeedsRefinement {
            location,
            reason: ProbeCoordinatorNeedsRefinementReason::IncompleteProposal {
                exact_obstructions: compact.evidence.exact_obstructions,
            },
        }),
        CompactTaskAction::NoProposal
        | CompactTaskAction::Duplicate
        | CompactTaskAction::OwnerSetChanged { .. }
        | CompactTaskAction::CompilerClosed { .. } => None,
    }
}

pub(super) fn try_drive_requested_plan<F>(
    config: &super::ProbeCoordinatorConfig,
    census: &mut ProbeCoordinatorCensus,
    ledger_revision: u64,
    plan: &RequestedDomainPlan,
    mut execute: F,
) -> ProbeCoordinatorDriveStop
where
    F: FnMut(
        &RequestedDomainPlan,
        &RequestedDomainTask,
        ProbeCoordinatorCensus,
        usize,
        usize,
    ) -> Result<CompactTaskCommit, ProbeCoordinatorFailure>,
{
    if plan.epoch_ordinal() != ledger_revision {
        return failed(
            *census,
            super::super::ProbeCampaignError::StaleLedgerRevision {
                planned: plan.epoch_ordinal(),
                current: ledger_revision,
            }
            .into(),
        );
    }

    let limits = config.limits();
    let requested_epoch = match census.epochs_started.checked_add(1) {
        Some(requested) => requested,
        None => {
            return failed(
                *census,
                ProbeCoordinatorFailure::ResourceCountOverflow { resource: EPOCHS },
            );
        }
    };
    if requested_epoch > limits.max_epochs {
        return operationally_bounded(
            *census,
            None,
            ProbeCoordinatorOperationalReason::EpochLimit {
                requested: requested_epoch,
                limit: limits.max_epochs,
            },
        );
    }
    let requested_plan = match census.plans_built.checked_add(1) {
        Some(requested) => requested,
        None => {
            return failed(
                *census,
                ProbeCoordinatorFailure::ResourceCountOverflow { resource: PLANS },
            );
        }
    };
    if requested_plan > limits.max_plans {
        return operationally_bounded(
            *census,
            None,
            ProbeCoordinatorOperationalReason::PlanLimit {
                requested: requested_plan,
                limit: limits.max_plans,
            },
        );
    }
    census.epochs_started = requested_epoch;
    census.plans_built = requested_plan;

    let task_count = plan.tasks().len();
    let mut completed_tasks = 0usize;
    let mut first_local_obstruction = None;
    for (execution_rank, task) in plan.tasks().iter().enumerate() {
        if task.canonical_ordinal() != execution_rank {
            return failed(
                *census,
                ProbeCoordinatorFailure::Invariant {
                    detail: "requested-domain plan tasks are not in canonical ordinal order",
                },
            );
        }
        if let Err(error) = plan.validate_task(task) {
            return failed(
                *census,
                super::super::ProbeCampaignError::LeaderPlan(error).into(),
            );
        }
        let symbolic_dimension = task.key().symbolic_axes().len();
        let location = ProbeCoordinatorTaskLocation {
            kind: ProbeCoordinatorTaskLocationKind::RequestedDomain {
                requested_ordinal: task.key().requested_ordinal(),
            },
            ledger_revision,
            // Diagnostic display projection only. The typed kind above keeps
            // this from becoming boundary-class or closure authority.
            class_ordinal: 0,
            effective_dimension: symbolic_dimension,
            parent_free_dimension: symbolic_dimension,
            boundary_codimension: 0,
            task_ordinal: task.canonical_ordinal(),
        };
        let requested_report = match census.task_reports.checked_add(1) {
            Some(requested) => requested,
            None => {
                return failed(
                    *census,
                    ProbeCoordinatorFailure::ResourceCountOverflow {
                        resource: TASK_REPORTS,
                    },
                );
            }
        };
        if requested_report > limits.max_task_reports {
            return operationally_bounded(
                *census,
                Some(location),
                ProbeCoordinatorOperationalReason::TaskReportLimit {
                    requested: requested_report,
                    limit: limits.max_task_reports,
                },
            );
        }
        let next_execution_rank = match execution_rank.checked_add(1) {
            Some(next) => next,
            None => {
                return failed(
                    *census,
                    ProbeCoordinatorFailure::ResourceCountOverflow {
                        resource: INVALIDATED_TICKETS,
                    },
                );
            }
        };
        let invalidated_tickets = match task_count.checked_sub(next_execution_rank) {
            Some(count) => count,
            None => {
                return failed(
                    *census,
                    ProbeCoordinatorFailure::Invariant {
                        detail: "requested-domain execution rank exceeded its owning plan",
                    },
                );
            }
        };
        let requested_completed_tasks = match completed_tasks.checked_add(1) {
            Some(requested) => requested,
            None => {
                return failed(
                    *census,
                    ProbeCoordinatorFailure::ResourceCountOverflow {
                        resource: TASK_REPORTS,
                    },
                );
            }
        };

        let commit = match execute(plan, task, *census, requested_report, invalidated_tickets) {
            Ok(commit) => commit,
            Err(error) => return failed(*census, error),
        };
        *census = commit.census;
        completed_tasks = requested_completed_tasks;
        let compact = commit.compact;

        if first_local_obstruction.is_none()
            && !matches!(
                compact.action,
                CompactTaskAction::OwnerSetChanged { .. }
                    | CompactTaskAction::CompilerClosed { .. }
            )
        {
            first_local_obstruction = requested_local_obstruction(compact, location);
        }

        match compact.action {
            // Requested domains are proposal chronology only. A local miss,
            // typed scheduler stop, rejected query, or unretained obstruction
            // cannot block later requests or the generic boundary complement.
            // Their exact scalar evidence and the first typed local
            // obstruction remain retained while later requests run. Global
            // epoch/plan/report limits were enforced above before the task
            // ran. Exact mutation or closure wins immediately; otherwise the
            // first obstruction is returned after the complete request plan.
            CompactTaskAction::NoProposal
            | CompactTaskAction::Duplicate
            | CompactTaskAction::IncompleteProposal => {}
            CompactTaskAction::OwnerSetChanged {
                mutation,
                before_revision,
                after_revision,
            } => {
                return ProbeCoordinatorDriveStop::OwnerSetChanged(
                    ProbeCoordinatorOwnerSetChanged {
                        census: *census,
                        location,
                        mutation,
                        before_revision,
                        after_revision,
                        invalidated_tickets,
                    },
                );
            }
            CompactTaskAction::CompilerClosed { exact } => {
                return ProbeCoordinatorDriveStop::CompilerClosed {
                    census: *census,
                    exact,
                };
            }
        }
    }

    if let Some(obstruction) = first_local_obstruction {
        return obstruction.into_stop(*census);
    }

    ProbeCoordinatorDriveStop::StableProgramCompleted {
        census: *census,
        ledger_revision,
        completed_classes: 0,
        completed_tasks,
    }
}

pub(super) fn try_drive_partition<F>(
    config: &super::ProbeCoordinatorConfig,
    planner_scope_key: &str,
    census: &mut ProbeCoordinatorCensus,
    fair_cursor: &mut ProbeCoordinatorFairCursor,
    ledger_revision: u64,
    sector: &Mask,
    partition: &UncoveredPartition,
    mut execute: F,
) -> ProbeCoordinatorDriveStop
where
    F: FnMut(
        &BoundarySimplexPlan,
        &BoundarySimplexTask,
        ProbeCoordinatorCensus,
        usize,
        usize,
    ) -> Result<CompactTaskCommit, ProbeCoordinatorFailure>,
{
    let limits = config.limits();
    let requested_epoch = match census.epochs_started.checked_add(1) {
        Some(requested) => requested,
        None => {
            return failed(
                *census,
                ProbeCoordinatorFailure::ResourceCountOverflow { resource: EPOCHS },
            );
        }
    };
    if requested_epoch > limits.max_epochs {
        return operationally_bounded(
            *census,
            None,
            ProbeCoordinatorOperationalReason::EpochLimit {
                requested: requested_epoch,
                limit: limits.max_epochs,
            },
        );
    }

    let schedule = match try_build_class_schedule(partition, sector.arity(), config) {
        Ok(schedule) => schedule,
        Err(error) => return failed(*census, error),
    };
    census.epochs_started = requested_epoch;
    let mut epoch_completed_tasks = 0usize;
    let class_count = schedule.classes().len();
    if class_count == 0 {
        return failed(
            *census,
            ProbeCoordinatorFailure::Invariant {
                detail: "nonempty uncovered partition produced no boundary service class",
            },
        );
    }
    let class_start = fair_cursor.class_start(class_count);

    for class_offset in 0..class_count {
        let class_ordinal = (class_start + class_offset) % class_count;
        let class = &schedule.classes()[class_ordinal];
        let requested_plan = match census.plans_built.checked_add(1) {
            Some(requested) => requested,
            None => {
                return failed(
                    *census,
                    ProbeCoordinatorFailure::ResourceCountOverflow { resource: PLANS },
                );
            }
        };
        if requested_plan > limits.max_plans {
            return operationally_bounded(
                *census,
                None,
                ProbeCoordinatorOperationalReason::PlanLimit {
                    requested: requested_plan,
                    limit: limits.max_plans,
                },
            );
        }
        let plan = match try_plan_boundary_simplex_samples(
            ledger_revision,
            [BoundarySimplexScopePartition::new(
                planner_scope_key,
                sector,
                partition,
            )],
            class.parent_free_dimension(),
            class.boundary_codimension(),
            class.profile(),
            limits.boundary_plan,
        ) {
            Ok(plan) => plan,
            Err(error) => return failed(*census, error.into()),
        };
        if plan.parent_free_dimension() != class.parent_free_dimension()
            || plan.face_dimension() != class.effective_dimension()
            || plan.boundary_codimension() != class.boundary_codimension()
            || plan.profile() != class.profile()
        {
            return failed(
                *census,
                ProbeCoordinatorFailure::Invariant {
                    detail: "boundary plan changed its canonical class semantics",
                },
            );
        }
        census.plans_built = requested_plan;
        let task_count = plan.tasks().len();
        if task_count == 0 {
            return failed(
                *census,
                ProbeCoordinatorFailure::Invariant {
                    detail: "boundary service class produced an empty task plan",
                },
            );
        }
        let chronology =
            match DiscoveryTaskChronology::try_new(&plan, config.discovery_coordinate_priority()) {
                Ok(chronology) => chronology,
                Err(error) => return failed(*census, error),
            };
        let task_start = if class_offset == 0 {
            fair_cursor.task_start(task_count)
        } else {
            0
        };

        for task_offset in 0..task_count {
            let execution_rank = (task_start + task_offset) % task_count;
            let Some(task_index) = chronology.canonical_task_index(execution_rank) else {
                return failed(
                    *census,
                    ProbeCoordinatorFailure::Invariant {
                        detail: "discovery chronology omitted an execution rank",
                    },
                );
            };
            let task = &plan.tasks()[task_index];
            let location = ProbeCoordinatorTaskLocation {
                kind: ProbeCoordinatorTaskLocationKind::BoundarySimplex,
                ledger_revision,
                class_ordinal: class.canonical_ordinal(),
                effective_dimension: class.effective_dimension(),
                parent_free_dimension: class.parent_free_dimension(),
                boundary_codimension: class.boundary_codimension(),
                task_ordinal: task.canonical_ordinal(),
            };
            let requested_report = match census.task_reports.checked_add(1) {
                Some(requested) => requested,
                None => {
                    return failed(
                        *census,
                        ProbeCoordinatorFailure::ResourceCountOverflow {
                            resource: TASK_REPORTS,
                        },
                    );
                }
            };
            if requested_report > limits.max_task_reports {
                return operationally_bounded(
                    *census,
                    Some(location),
                    ProbeCoordinatorOperationalReason::TaskReportLimit {
                        requested: requested_report,
                        limit: limits.max_task_reports,
                    },
                );
            }
            let requested_epoch_completed = match epoch_completed_tasks.checked_add(1) {
                Some(requested) => requested,
                None => {
                    return failed(
                        *census,
                        ProbeCoordinatorFailure::ResourceCountOverflow {
                            resource: TASK_REPORTS,
                        },
                    );
                }
            };

            // The possible plan suffix is computed before task evaluation so
            // its cumulative counter can be reserved before any owner
            // application. It is committed only for a non-closing mutation.
            let next_task_offset = match task_offset.checked_add(1) {
                Some(ordinal) => ordinal,
                None => {
                    return failed(
                        *census,
                        ProbeCoordinatorFailure::ResourceCountOverflow {
                            resource: INVALIDATED_TICKETS,
                        },
                    );
                }
            };
            let invalidated_tickets = match task_count.checked_sub(next_task_offset) {
                Some(count) => count,
                None => {
                    return failed(
                        *census,
                        ProbeCoordinatorFailure::Invariant {
                            detail: "task execution rank exceeded its owning plan",
                        },
                    );
                }
            };

            let commit = match execute(&plan, task, *census, requested_report, invalidated_tickets)
            {
                Ok(commit) => commit,
                Err(error) => return failed(*census, error),
            };
            *census = commit.census;
            let compact = commit.compact;
            epoch_completed_tasks = requested_epoch_completed;

            if !matches!(
                compact.action,
                CompactTaskAction::OwnerSetChanged { .. }
                    | CompactTaskAction::CompilerClosed { .. }
            ) {
                if let Some(reason) = operational_reason(compact) {
                    return operationally_bounded(*census, Some(location), reason);
                }
                if let Some(reason) = search_refinement_reason(compact) {
                    return needs_refinement(*census, Some(location), reason);
                }
            }

            match compact.action {
                CompactTaskAction::NoProposal | CompactTaskAction::Duplicate => {
                    if compact.evidence.exact_obstructions != 0 {
                        return needs_refinement(
                            *census,
                            Some(location),
                            ProbeCoordinatorNeedsRefinementReason::DiagnosticExactObstructions {
                                count: compact.evidence.exact_obstructions,
                            },
                        );
                    }
                }
                CompactTaskAction::IncompleteProposal => {
                    return needs_refinement(
                        *census,
                        Some(location),
                        ProbeCoordinatorNeedsRefinementReason::IncompleteProposal {
                            exact_obstructions: compact.evidence.exact_obstructions,
                        },
                    );
                }
                CompactTaskAction::OwnerSetChanged {
                    mutation,
                    before_revision,
                    after_revision,
                } => {
                    fair_cursor.advance_after(
                        class_ordinal,
                        execution_rank,
                        task_count,
                        class_count,
                    );
                    return ProbeCoordinatorDriveStop::OwnerSetChanged(
                        ProbeCoordinatorOwnerSetChanged {
                            census: *census,
                            location,
                            mutation,
                            before_revision,
                            after_revision,
                            invalidated_tickets,
                        },
                    );
                }
                CompactTaskAction::CompilerClosed { exact } => {
                    return ProbeCoordinatorDriveStop::CompilerClosed {
                        census: *census,
                        exact,
                    };
                }
            }
        }
        if let Err(error) = try_increment(&mut census.classes_completed, CENSUS) {
            return failed(*census, error);
        }
    }

    ProbeCoordinatorDriveStop::StableProgramCompleted {
        census: *census,
        ledger_revision,
        completed_classes: schedule.classes().len(),
        completed_tasks: epoch_completed_tasks,
    }
}

fn needs_refinement(
    census: ProbeCoordinatorCensus,
    location: Option<ProbeCoordinatorTaskLocation>,
    reason: ProbeCoordinatorNeedsRefinementReason,
) -> ProbeCoordinatorDriveStop {
    ProbeCoordinatorDriveStop::NeedsRefinement(ProbeCoordinatorNeedsRefinement {
        census,
        location,
        reason,
    })
}

fn operationally_bounded(
    census: ProbeCoordinatorCensus,
    location: Option<ProbeCoordinatorTaskLocation>,
    reason: ProbeCoordinatorOperationalReason,
) -> ProbeCoordinatorDriveStop {
    ProbeCoordinatorDriveStop::OperationallyBounded(ProbeCoordinatorOperationalStop {
        census,
        location,
        reason,
    })
}

fn failed(
    census: ProbeCoordinatorCensus,
    failure: ProbeCoordinatorFailure,
) -> ProbeCoordinatorDriveStop {
    ProbeCoordinatorDriveStop::Failed(ProbeCoordinatorFailureStop { census, failure })
}

fn failed_public(
    census: ProbeCoordinatorCensus,
    failure: ProbeCoordinatorFailure,
) -> ProbeCoordinatorStop {
    ProbeCoordinatorStop::Failed(ProbeCoordinatorFailureStop { census, failure })
}

fn requested_failed_public(
    census: ProbeCoordinatorCensus,
    failure: ProbeCoordinatorFailure,
) -> RequestedProbeCoordinatorStop {
    RequestedProbeCoordinatorStop::Failed(ProbeCoordinatorFailureStop { census, failure })
}

pub(super) fn upgrade_requested_drive_stop(
    drive: ProbeCoordinatorDriveStop,
    ledger: &CanonicalExactOwnerLedger,
    epoch_identity: &ExactOwnerLedgerSnapshotIdentity,
) -> RequestedProbeCoordinatorStop {
    match drive {
        ProbeCoordinatorDriveStop::CompilerClosed { census, exact } => {
            let live_identity = ledger.snapshot_identity();
            if let Err(error) = ledger.try_require_current_snapshot(&live_identity) {
                return requested_failed_public(census, error.into());
            }
            let live = ledger.snapshot();
            if live != exact || !live.status().is_compiler_closed() {
                return requested_failed_public(
                    census,
                    ProbeCoordinatorFailure::Invariant {
                        detail: "requested compiler-closed stop did not match the live exact ledger",
                    },
                );
            }
            RequestedProbeCoordinatorStop::CompilerClosed {
                census,
                ledger_snapshot: live_identity,
                exact: live,
            }
        }
        ProbeCoordinatorDriveStop::OwnerSetChanged(stop) => {
            RequestedProbeCoordinatorStop::OwnerSetChanged(stop)
        }
        ProbeCoordinatorDriveStop::NeedsRefinement(stop) => {
            RequestedProbeCoordinatorStop::NeedsRefinement(stop)
        }
        ProbeCoordinatorDriveStop::OperationallyBounded(stop) => {
            RequestedProbeCoordinatorStop::OperationallyBounded(stop)
        }
        ProbeCoordinatorDriveStop::Failed(stop) => RequestedProbeCoordinatorStop::Failed(stop),
        ProbeCoordinatorDriveStop::StableProgramCompleted {
            census,
            ledger_revision,
            completed_tasks,
            ..
        } => {
            if let Err(error) = ledger.try_require_current_snapshot(epoch_identity) {
                return requested_failed_public(census, error.into());
            }
            if ledger.revision().get() != ledger_revision {
                return requested_failed_public(
                    census,
                    ProbeCoordinatorFailure::Invariant {
                        detail: "requested phase completed at a different live ledger revision",
                    },
                );
            }
            RequestedProbeCoordinatorStop::PhaseCompleted {
                census,
                ledger_snapshot: epoch_identity.clone(),
                ledger_revision,
                completed_tasks,
            }
        }
    }
}

pub(super) fn upgrade_drive_stop(
    drive: ProbeCoordinatorDriveStop,
    ledger: &CanonicalExactOwnerLedger,
    epoch_identity: &ExactOwnerLedgerSnapshotIdentity,
) -> ProbeCoordinatorStop {
    match drive {
        ProbeCoordinatorDriveStop::CompilerClosed { census, exact } => {
            let live_identity = ledger.snapshot_identity();
            if let Err(error) = ledger.try_require_current_snapshot(&live_identity) {
                return failed_public(census, error.into());
            }
            let live = ledger.snapshot();
            if live != exact || !live.status().is_compiler_closed() {
                return failed_public(
                    census,
                    ProbeCoordinatorFailure::Invariant {
                        detail: "compiler-closed drive stop did not match the live exact ledger",
                    },
                );
            }
            ProbeCoordinatorStop::CompilerClosed {
                census,
                ledger_snapshot: live_identity,
                exact: live,
            }
        }
        ProbeCoordinatorDriveStop::OwnerSetChanged(stop) => {
            ProbeCoordinatorStop::OwnerSetChanged(stop)
        }
        ProbeCoordinatorDriveStop::NeedsRefinement(stop) => {
            ProbeCoordinatorStop::NeedsRefinement(stop)
        }
        ProbeCoordinatorDriveStop::OperationallyBounded(stop) => {
            ProbeCoordinatorStop::OperationallyBounded(stop)
        }
        ProbeCoordinatorDriveStop::Failed(stop) => ProbeCoordinatorStop::Failed(stop),
        ProbeCoordinatorDriveStop::StableProgramCompleted {
            census,
            ledger_revision,
            completed_classes,
            completed_tasks,
        } => {
            if let Err(error) = ledger.try_require_current_snapshot(epoch_identity) {
                return failed_public(census, error.into());
            }
            let live = ledger.snapshot();
            if live.revision().get() != ledger_revision {
                return failed_public(
                    census,
                    ProbeCoordinatorFailure::Invariant {
                        detail: "stable program completed at a different live ledger revision",
                    },
                );
            }
            let exhaustible = matches!(
                live.status(),
                ExactOwnerLedgerCoverStatus::Compiled(ExactOwnerCoverStatus::Incomplete(
                    ExactOwnerCoverObstructionKind::NonFinite,
                ))
            ) && !live.uncovered_is_finite()
                && live.missing_terminal_count() == 0
                && live.guard_incomplete_owner_count() == 0;
            if !exhaustible {
                return ProbeCoordinatorStop::NeedsRefinement(ProbeCoordinatorNeedsRefinement {
                    census,
                    location: None,
                    reason: ProbeCoordinatorNeedsRefinementReason::ExactCompilerState {
                        status: live.status(),
                        uncovered_is_finite: live.uncovered_is_finite(),
                        missing_terminal_count: live.missing_terminal_count(),
                        guard_incomplete_owner_count: live.guard_incomplete_owner_count(),
                    },
                });
            }
            ProbeCoordinatorStop::ExhaustedAtConfig {
                census,
                ledger_snapshot: epoch_identity.clone(),
                exact: live,
                ledger_revision,
                completed_classes,
                completed_tasks,
            }
        }
    }
}
