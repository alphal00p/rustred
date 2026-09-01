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
use crate::sector::Mask;

use super::super::ProbeCampaignAdapter;
use super::compact::{
    CompactTaskAction, CompactTaskCommit, operational_reason, search_refinement_reason,
    try_increment, try_reserve_evaluated_task, validate_live_effect,
};
use super::schedule::try_build_class_schedule;
use super::{
    BoundaryProbeCoordinator, ProbeCoordinatorCensus, ProbeCoordinatorFailure,
    ProbeCoordinatorFailureStop, ProbeCoordinatorNeedsRefinement,
    ProbeCoordinatorNeedsRefinementReason, ProbeCoordinatorOperationalReason,
    ProbeCoordinatorOperationalStop, ProbeCoordinatorOwnerSetChanged, ProbeCoordinatorStop,
    ProbeCoordinatorTaskLocation,
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
        let planner_scope_key = try_build_planner_scope_key(&config, ledger)?;
        Ok(Self {
            config,
            adapter,
            bound_ledger: ledger.snapshot_identity(),
            planner_scope_key,
            census: Self::empty_census(),
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
            revision,
            &sector,
            &partition,
            |plan, task, baseline_census, requested_report, invalidated_tickets| {
                ledger.try_require_current_snapshot(&snapshot_identity)?;
                let before = ledger.snapshot();
                let binding = adapter.try_bind_task(plan, task, ledger)?;
                let probes = try_materialize_probes(config, adapter, task)?;
                let evaluated = adapter.try_evaluate_task(binding, ledger, probes)?;
                // Every fallible replay/census join and every possible scalar
                // counter update is checked while the exact ledger is still
                // immutable. Serial application below is the transaction's
                // sole mutation boundary.
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
            },
        );
        upgrade_drive_stop(drive, ledger, &snapshot_identity)
    }
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
        ordering,
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

fn try_materialize_probes(
    config: &super::ProbeCoordinatorConfig,
    adapter: &ProbeCampaignAdapter<'_, '_, '_>,
    task: &BoundarySimplexTask,
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
        for (coordinate, (&target, &offset)) in task
            .lattice_target()
            .iter()
            .zip(template.chart_offsets())
            .enumerate()
        {
            chart_coordinates.push(target.checked_add(offset).ok_or(
                ProbeCoordinatorFailure::ProbeChartCoordinateOverflow {
                    probe_ordinal,
                    coordinate,
                },
            )?);
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

pub(super) fn try_drive_partition<F>(
    config: &super::ProbeCoordinatorConfig,
    planner_scope_key: &str,
    census: &mut ProbeCoordinatorCensus,
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

    for class in schedule.classes() {
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

        for task in plan.tasks() {
            let location = ProbeCoordinatorTaskLocation {
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
            let next_task_ordinal = match task.canonical_ordinal().checked_add(1) {
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
            let invalidated_tickets = match plan.tasks().len().checked_sub(next_task_ordinal) {
                Some(count) => count,
                None => {
                    return failed(
                        *census,
                        ProbeCoordinatorFailure::Invariant {
                            detail: "canonical task ordinal exceeded its owning plan",
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
