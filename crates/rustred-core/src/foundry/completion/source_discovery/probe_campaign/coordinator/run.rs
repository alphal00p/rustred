use crate::foundry::completion::UncoveredPartition;
use crate::foundry::completion::frame::admission::{
    ExactOwnerCoverObstructionKind, ExactOwnerCoverStatus,
};
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
    CompactTaskAction, CompactTaskResult, operational_reason, search_refinement_reason,
    try_compact_report, try_increment, try_record_compact, validate_live_effect,
};
use super::schedule::try_build_class_schedule;
use super::{
    BoundaryProbeCoordinator, ProbeCoordinatorCensus, ProbeCoordinatorFailure,
    ProbeCoordinatorFailureStop, ProbeCoordinatorNeedsRefinement,
    ProbeCoordinatorNeedsRefinementReason, ProbeCoordinatorOperationalReason,
    ProbeCoordinatorOperationalStop, ProbeCoordinatorOwnerSetChanged, ProbeCoordinatorProbeBatch,
    ProbeCoordinatorStop, ProbeCoordinatorTaskLocation,
};

const EPOCHS: &str = "epochs";
const PLANS: &str = "plans";
const TASK_REPORTS: &str = "task reports";
const INVALIDATED_TICKETS: &str = "invalidated tickets";
const CENSUS: &str = "scalar census";

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

impl BoundaryProbeCoordinator {
    /// Run one immutable ledger epoch in canonical order.
    ///
    /// A changed owner set ends this call immediately. No plan or ticket is
    /// retained; calling this method again necessarily clones the new exact
    /// partition and creates fresh opaque planner identities.
    pub(crate) fn try_run_boundary_epoch<F>(
        &mut self,
        adapter: &ProbeCampaignAdapter<'_, '_, '_>,
        ledger: &mut CanonicalExactOwnerLedger,
        probes_for_task: &mut F,
    ) -> ProbeCoordinatorStop
    where
        F: FnMut(
            &BoundarySimplexTask,
        ) -> Result<ProbeCoordinatorProbeBatch, ProbeCoordinatorFailure>,
    {
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
        let expected_probes_per_task = self.config.declared_probes_per_task().get();

        let drive = try_drive_partition(self, revision, &sector, &partition, |plan, task| {
            ledger.try_require_current_snapshot(&snapshot_identity)?;
            let before = ledger.snapshot();
            let binding = adapter.try_bind_task(plan, task, ledger)?;
            let probes = probes_for_task(task)?;
            let declared_probe_count = probes.declared_count();
            if declared_probe_count != expected_probes_per_task {
                return Err(ProbeCoordinatorFailure::ProbeCountMismatch {
                    expected: expected_probes_per_task,
                    actual: declared_probe_count,
                });
            }
            let evaluated = adapter.try_evaluate_task(binding, ledger, probes.into_probes())?;
            let report = adapter.try_apply_evaluated_task(evaluated, ledger)?;
            let compact = try_compact_report(&report)?;
            if compact.evidence.declared_probes != declared_probe_count {
                return Err(ProbeCoordinatorFailure::Invariant {
                    detail: "scheduler outcome total differed from declared task probes",
                });
            }
            let after = ledger.snapshot();
            validate_live_effect(before, after, compact.action)?;
            drop(report);
            Ok(compact)
        });
        upgrade_drive_stop(drive, ledger, &snapshot_identity)
    }
}

pub(super) fn try_drive_partition<F>(
    coordinator: &mut BoundaryProbeCoordinator,
    ledger_revision: u64,
    sector: &Mask,
    partition: &UncoveredPartition,
    mut execute: F,
) -> ProbeCoordinatorDriveStop
where
    F: FnMut(
        &BoundarySimplexPlan,
        &BoundarySimplexTask,
    ) -> Result<CompactTaskResult, ProbeCoordinatorFailure>,
{
    let limits = coordinator.config.limits();
    let requested_epoch = match coordinator.census.epochs_started.checked_add(1) {
        Some(requested) => requested,
        None => {
            return failed(
                coordinator.census,
                ProbeCoordinatorFailure::ResourceCountOverflow { resource: EPOCHS },
            );
        }
    };
    if requested_epoch > limits.max_epochs {
        return operationally_bounded(
            coordinator.census,
            None,
            ProbeCoordinatorOperationalReason::EpochLimit {
                requested: requested_epoch,
                limit: limits.max_epochs,
            },
        );
    }

    let schedule = match try_build_class_schedule(partition, sector.arity(), &coordinator.config) {
        Ok(schedule) => schedule,
        Err(error) => return failed(coordinator.census, error),
    };
    coordinator.census.epochs_started = requested_epoch;
    let mut epoch_completed_tasks = 0usize;

    for class in schedule.classes() {
        let requested_plan = match coordinator.census.plans_built.checked_add(1) {
            Some(requested) => requested,
            None => {
                return failed(
                    coordinator.census,
                    ProbeCoordinatorFailure::ResourceCountOverflow { resource: PLANS },
                );
            }
        };
        if requested_plan > limits.max_plans {
            return operationally_bounded(
                coordinator.census,
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
                coordinator.config.declared_campaign_key(),
                sector,
                partition,
            )],
            class.parent_free_dimension(),
            class.boundary_codimension(),
            class.profile(),
            limits.boundary_plan,
        ) {
            Ok(plan) => plan,
            Err(error) => return failed(coordinator.census, error.into()),
        };
        if plan.parent_free_dimension() != class.parent_free_dimension()
            || plan.face_dimension() != class.effective_dimension()
            || plan.boundary_codimension() != class.boundary_codimension()
            || plan.profile() != class.profile()
        {
            return failed(
                coordinator.census,
                ProbeCoordinatorFailure::Invariant {
                    detail: "boundary plan changed its canonical class semantics",
                },
            );
        }
        coordinator.census.plans_built = requested_plan;

        for task in plan.tasks() {
            let location = ProbeCoordinatorTaskLocation {
                ledger_revision,
                class_ordinal: class.canonical_ordinal(),
                effective_dimension: class.effective_dimension(),
                parent_free_dimension: class.parent_free_dimension(),
                boundary_codimension: class.boundary_codimension(),
                task_ordinal: task.canonical_ordinal(),
            };
            let requested_report = match coordinator.census.task_reports.checked_add(1) {
                Some(requested) => requested,
                None => {
                    return failed(
                        coordinator.census,
                        ProbeCoordinatorFailure::ResourceCountOverflow {
                            resource: TASK_REPORTS,
                        },
                    );
                }
            };
            if requested_report > limits.max_task_reports {
                return operationally_bounded(
                    coordinator.census,
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
                        coordinator.census,
                        ProbeCoordinatorFailure::ResourceCountOverflow {
                            resource: TASK_REPORTS,
                        },
                    );
                }
            };

            let compact = match execute(&plan, task) {
                Ok(compact) => compact,
                Err(error) => return failed(coordinator.census, error),
            };
            if let Err(error) =
                try_record_compact(&mut coordinator.census, requested_report, compact)
            {
                return failed(coordinator.census, error);
            }
            epoch_completed_tasks = requested_epoch_completed;

            if !matches!(
                compact.action,
                CompactTaskAction::OwnerSetChanged { .. }
                    | CompactTaskAction::CompilerClosed { .. }
            ) {
                if let Some(reason) = operational_reason(compact) {
                    return operationally_bounded(coordinator.census, Some(location), reason);
                }
                if let Some(reason) = search_refinement_reason(compact) {
                    return needs_refinement(coordinator.census, Some(location), reason);
                }
            }

            match compact.action {
                CompactTaskAction::NoProposal | CompactTaskAction::Duplicate => {
                    if compact.evidence.exact_obstructions != 0 {
                        return needs_refinement(
                            coordinator.census,
                            Some(location),
                            ProbeCoordinatorNeedsRefinementReason::DiagnosticExactObstructions {
                                count: compact.evidence.exact_obstructions,
                            },
                        );
                    }
                }
                CompactTaskAction::IncompleteProposal => {
                    return needs_refinement(
                        coordinator.census,
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
                    let next_task_ordinal = match task.canonical_ordinal().checked_add(1) {
                        Some(ordinal) => ordinal,
                        None => {
                            return failed(
                                coordinator.census,
                                ProbeCoordinatorFailure::ResourceCountOverflow {
                                    resource: INVALIDATED_TICKETS,
                                },
                            );
                        }
                    };
                    let invalidated_tickets =
                        plan.tasks().len().checked_sub(next_task_ordinal).ok_or(
                            ProbeCoordinatorFailure::Invariant {
                                detail: "canonical task ordinal exceeded its owning plan",
                            },
                        );
                    let invalidated_tickets = match invalidated_tickets {
                        Ok(count) => count,
                        Err(error) => return failed(coordinator.census, error),
                    };
                    let updated_invalidated = match coordinator
                        .census
                        .invalidated_tickets
                        .checked_add(invalidated_tickets)
                    {
                        Some(count) => count,
                        None => {
                            return failed(
                                coordinator.census,
                                ProbeCoordinatorFailure::ResourceCountOverflow {
                                    resource: INVALIDATED_TICKETS,
                                },
                            );
                        }
                    };
                    coordinator.census.invalidated_tickets = updated_invalidated;
                    return ProbeCoordinatorDriveStop::OwnerSetChanged(
                        ProbeCoordinatorOwnerSetChanged {
                            census: coordinator.census,
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
                        census: coordinator.census,
                        exact,
                    };
                }
            }
        }
        if let Err(error) = try_increment(&mut coordinator.census.classes_completed, CENSUS) {
            return failed(coordinator.census, error);
        }
    }

    ProbeCoordinatorDriveStop::StableProgramCompleted {
        census: coordinator.census,
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
