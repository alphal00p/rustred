use crate::foundry::completion::source_discovery::leader_walk::{
    LeaderWalkPlanError, LeaderWalkScopePartition, try_plan_maximal_orthant_leader_walk,
};
use crate::foundry::completion::source_discovery::{
    CanonicalExactOwnerLedger, ExactOwnerCoverDeltaKind,
};

use super::target::{try_order_wave, try_scope_key, try_target_key};
use super::{
    SpiredFixedPointCensus, SpiredFixedPointConfig, SpiredFixedPointError,
    SpiredFixedPointIncompleteReason, SpiredFixedPointReport, SpiredFixedPointStop,
    SpiredFixedPointTarget, SpiredFixedPointTargetRunner, SpiredTargetPortfolioBudget,
    SpiredTargetPortfolioCensus, SpiredTargetPortfolioDisposition,
    SpiredTargetPortfolioIncompleteReason,
};

const EPOCHS: &str = "epochs";
const PLANS: &str = "leader plans";
const PLANNED_TARGETS: &str = "planned targets";
const TARGETS_INSPECTED: &str = "targets inspected";
const TARGET_PORTFOLIOS: &str = "target portfolios";
const OWNER_MUTATIONS: &str = "owner mutations";

/// Drive one exact owner ledger using a deterministic window-one chronology.
///
/// The injected runner may inspect the ledger but cannot mutate it. Every
/// owner result is rejoined to the exact opaque snapshot from which its task
/// was planned. One nonduplicate mutation invalidates the whole plan and
/// triggers immediate replanning.
pub(crate) fn try_drive_spired_fixed_point<Runner: SpiredFixedPointTargetRunner>(
    ledger: &mut CanonicalExactOwnerLedger,
    runner: &mut Runner,
    config: &SpiredFixedPointConfig,
) -> Result<SpiredFixedPointReport, SpiredFixedPointError> {
    if let Some(priority) = config.discovery_coordinate_priority.as_ref()
        && priority.arity() != ledger.sector().arity()
    {
        return Err(SpiredFixedPointError::WrongCoordinatePriorityArity {
            expected: ledger.sector().arity(),
            actual: priority.arity(),
        });
    }

    let mut census = SpiredFixedPointCensus::default();
    'epochs: loop {
        if ledger.snapshot().status().is_compiler_closed() {
            return finish(ledger, census, None);
        }

        let Some(next_epochs) = try_next_bounded(census.epochs, config.limits.max_epochs, EPOCHS)?
        else {
            return finish(
                ledger,
                census,
                Some(limit_reason(
                    EPOCHS,
                    census.epochs,
                    config.limits.max_epochs,
                )?),
            );
        };
        census.epochs = next_epochs;

        // Reserve the aggregate plan slot before cloning or freezing any
        // geometry. A zero plan budget must not execute one whole planner and
        // only reject its result afterwards.
        let Some(next_plans) = try_next_bounded(census.plans, config.limits.max_plans, PLANS)?
        else {
            return finish(
                ledger,
                census,
                Some(limit_reason(PLANS, census.plans, config.limits.max_plans)?),
            );
        };
        let remaining_planned_targets = config
            .limits
            .max_planned_targets
            .checked_sub(census.planned_targets)
            .ok_or(SpiredFixedPointError::Invariant {
                detail: "planned-target census exceeded its aggregate limit",
            })?;
        if remaining_planned_targets == 0 {
            return finish(
                ledger,
                census,
                Some(limit_reason(
                    PLANNED_TARGETS,
                    census.planned_targets,
                    config.limits.max_planned_targets,
                )?),
            );
        }
        census.plans = next_plans;

        let epoch_identity = ledger.snapshot_identity();
        let partition = ledger.try_clone_uncovered_partition()?;
        let scope_key = try_scope_key(ledger)?;
        let mut leader_walk_limits = config.leader_walk;
        leader_walk_limits.max_tasks = leader_walk_limits.max_tasks.min(remaining_planned_targets);
        let plan = match try_plan_maximal_orthant_leader_walk(
            ledger.revision().get(),
            [LeaderWalkScopePartition::new(
                &scope_key,
                ledger.sector(),
                &partition,
            )],
            leader_walk_limits,
        ) {
            Ok(plan) => plan,
            Err(LeaderWalkPlanError::NoUnboundedGeometry) => {
                let snapshot = ledger.snapshot();
                return finish(
                    ledger,
                    census,
                    Some(
                        SpiredFixedPointIncompleteReason::FiniteResidualRequiresExactWork {
                            uncovered_boxes: snapshot.uncovered_box_count(),
                            missing_terminals: snapshot.missing_terminal_count(),
                            guard_incomplete_owners: snapshot.guard_incomplete_owner_count(),
                        },
                    ),
                );
            }
            Err(LeaderWalkPlanError::ResourceLimit {
                resource,
                requested,
                limit,
            }) => {
                return finish(
                    ledger,
                    census,
                    Some(SpiredFixedPointIncompleteReason::ResourceLimit {
                        resource,
                        requested,
                        limit,
                    }),
                );
            }
            Err(error) => return Err(error.into()),
        };

        let next_planned_targets = checked_add(
            PLANNED_TARGETS,
            census.planned_targets,
            plan.planned_task_count(),
        )?;
        if next_planned_targets > config.limits.max_planned_targets {
            return finish(
                ledger,
                census,
                Some(SpiredFixedPointIncompleteReason::ResourceLimit {
                    resource: PLANNED_TARGETS,
                    requested: next_planned_targets,
                    limit: config.limits.max_planned_targets,
                }),
            );
        }
        census.planned_targets = next_planned_targets;

        let mut first_target_reason = None;
        for (wave_ordinal, wave) in plan.waves().into_iter().enumerate() {
            match wave_ordinal {
                0 | 1 => {}
                _ => {
                    return Err(SpiredFixedPointError::Invariant {
                        detail: "a leader plan exposed more than two waves",
                    });
                }
            }
            let task_order = try_order_wave(wave, config.discovery_coordinate_priority.as_ref())?;

            for task_ordinal in task_order {
                let task =
                    wave.tasks()
                        .get(task_ordinal)
                        .ok_or(SpiredFixedPointError::Invariant {
                            detail: "ordered leader-task index escaped its wave",
                        })?;
                plan.validate_task(task)?;

                let Some(next_inspected) = try_next_bounded(
                    census.targets_inspected,
                    config.limits.max_targets_inspected,
                    TARGETS_INSPECTED,
                )?
                else {
                    return finish(
                        ledger,
                        census,
                        Some(limit_reason(
                            TARGETS_INSPECTED,
                            census.targets_inspected,
                            config.limits.max_targets_inspected,
                        )?),
                    );
                };
                census.targets_inspected = next_inspected;

                let target_key = try_target_key(ledger, task)?;
                if ledger.has_explicit_terminal(&target_key) {
                    census.terminal_targets_skipped = checked_add(
                        "terminal targets skipped",
                        census.terminal_targets_skipped,
                        1,
                    )?;
                    continue;
                }

                let Some(next_portfolios) = try_next_bounded(
                    census.target_portfolios,
                    config.limits.max_target_portfolios,
                    TARGET_PORTFOLIOS,
                )?
                else {
                    return finish(
                        ledger,
                        census,
                        Some(limit_reason(
                            TARGET_PORTFOLIOS,
                            census.target_portfolios,
                            config.limits.max_target_portfolios,
                        )?),
                    );
                };
                let budget = remaining_portfolio_budget(&census, config)?;
                census.target_portfolios = next_portfolios;
                let report = runner
                    .try_run_target_portfolio(SpiredFixedPointTarget::new(task, ledger), budget)
                    .map_err(SpiredFixedPointError::Runner)?;
                let work = report.census();
                work.fits(budget)?;
                charge_target_work(&mut census.target_work, work)?;
                match report.into_disposition() {
                    SpiredTargetPortfolioDisposition::Incomplete(reason) => {
                        census.incomplete_target_portfolios = checked_add(
                            "incomplete target portfolios",
                            census.incomplete_target_portfolios,
                            1,
                        )?;
                        if first_target_reason.is_none() {
                            first_target_reason = Some(reason);
                        }
                    }
                    SpiredTargetPortfolioDisposition::CompiledOwner(owner) => {
                        // Reserve mutation capacity before applying an owner:
                        // `try_apply_owner` is transactional but intentionally
                        // mutates on either semantic or geometric progress.
                        if census.semantic_owner_mutations >= config.limits.max_owner_mutations {
                            return finish(
                                ledger,
                                census,
                                Some(limit_reason(
                                    OWNER_MUTATIONS,
                                    census.semantic_owner_mutations,
                                    config.limits.max_owner_mutations,
                                )?),
                            );
                        }
                        ledger.try_require_current_snapshot(&epoch_identity)?;
                        let delta = ledger.try_apply_owner(owner)?;
                        match delta.kind() {
                            ExactOwnerCoverDeltaKind::Duplicate => {
                                census.duplicate_owner_proposals = checked_add(
                                    "duplicate owner proposals",
                                    census.duplicate_owner_proposals,
                                    1,
                                )?;
                            }
                            ExactOwnerCoverDeltaKind::ChangedWithoutGeometricShrink => {
                                census.semantic_owner_mutations = checked_add(
                                    OWNER_MUTATIONS,
                                    census.semantic_owner_mutations,
                                    1,
                                )?;
                                if delta.updated().status().is_compiler_closed() {
                                    return finish(ledger, census, None);
                                }
                                continue 'epochs;
                            }
                            ExactOwnerCoverDeltaKind::StrictGeometricShrink => {
                                census.semantic_owner_mutations = checked_add(
                                    OWNER_MUTATIONS,
                                    census.semantic_owner_mutations,
                                    1,
                                )?;
                                census.strict_geometric_shrinks = checked_add(
                                    "strict geometric shrinks",
                                    census.strict_geometric_shrinks,
                                    1,
                                )?;
                                if delta.updated().status().is_compiler_closed() {
                                    return finish(ledger, census, None);
                                }
                                continue 'epochs;
                            }
                        }
                    }
                }
            }
        }

        // No owner changed the revision during this complete finite program.
        // Rejoin the exact epoch before classifying the bounded stop.
        ledger.try_require_current_snapshot(&epoch_identity)?;
        return finish(
            ledger,
            census,
            Some(SpiredFixedPointIncompleteReason::StableProgramExhausted {
                first_target_reason,
            }),
        );
    }
}

fn finish(
    ledger: &CanonicalExactOwnerLedger,
    census: SpiredFixedPointCensus,
    incomplete: Option<SpiredFixedPointIncompleteReason>,
) -> Result<SpiredFixedPointReport, SpiredFixedPointError> {
    let identity = ledger.snapshot_identity();
    ledger.try_require_current_snapshot(&identity)?;
    let snapshot = ledger.snapshot();
    let stop = if snapshot.status().is_compiler_closed() {
        SpiredFixedPointStop::CompilerClosed {
            authority: identity,
            snapshot,
        }
    } else {
        SpiredFixedPointStop::Incomplete {
            authority: identity,
            snapshot,
            reason: incomplete.ok_or(SpiredFixedPointError::Invariant {
                detail: "a nonclosed fixed-point stop omitted its incompleteness reason",
            })?,
        }
    };
    Ok(SpiredFixedPointReport::new(census, stop))
}

fn remaining_portfolio_budget(
    census: &SpiredFixedPointCensus,
    config: &SpiredFixedPointConfig,
) -> Result<SpiredTargetPortfolioBudget, SpiredFixedPointError> {
    let used = census.target_work;
    let limits = config.limits;
    Ok(SpiredTargetPortfolioBudget {
        target_lane_runs: remaining(
            "target lane runs",
            limits.max_target_lane_runs,
            used.target_lane_runs,
        )?,
        probe_attempts: remaining(
            "probe attempts",
            limits.max_probe_attempts,
            used.probe_attempts,
        )?,
        scheduled_requests: remaining(
            "scheduled requests",
            limits.max_scheduled_requests,
            used.scheduled_requests,
        )?,
        streamed_rows: remaining(
            "streamed rows",
            limits.max_streamed_rows,
            used.streamed_rows,
        )?,
        modular_hits: remaining("modular hits", limits.max_modular_hits, used.modular_hits)?,
        exact_lift_attempts: remaining(
            "exact lift attempts",
            limits.max_exact_lift_attempts,
            used.exact_lift_attempts,
        )?,
        rejected_hits: remaining(
            "rejected hits",
            limits.max_rejected_hits,
            used.rejected_hits,
        )?,
        alternative_nodes: remaining(
            "alternative nodes",
            limits.max_alternative_nodes,
            used.alternative_nodes,
        )?,
        excluded_requests: remaining(
            "excluded requests",
            limits.max_excluded_requests,
            used.excluded_requests,
        )?,
        excluded_request_coordinate_cells: remaining(
            "excluded request coordinate cells",
            limits.max_excluded_request_coordinate_cells,
            used.excluded_request_coordinate_cells,
        )?,
        guard_predicates: remaining(
            "guard predicates",
            limits.max_guard_predicates,
            used.guard_predicates,
        )?,
        guard_obligations: remaining(
            "guard obligations",
            limits.max_guard_obligations,
            used.guard_obligations,
        )?,
        owner_compile_attempts: remaining(
            "owner compile attempts",
            limits.max_owner_compile_attempts,
            used.owner_compile_attempts,
        )?,
    })
}

fn charge_target_work(
    cumulative: &mut SpiredTargetPortfolioCensus,
    delta: SpiredTargetPortfolioCensus,
) -> Result<(), SpiredFixedPointError> {
    cumulative.target_lane_runs = checked_add(
        "target lane runs",
        cumulative.target_lane_runs,
        delta.target_lane_runs,
    )?;
    cumulative.probe_attempts = checked_add(
        "probe attempts",
        cumulative.probe_attempts,
        delta.probe_attempts,
    )?;
    cumulative.scheduled_requests = checked_add(
        "scheduled requests",
        cumulative.scheduled_requests,
        delta.scheduled_requests,
    )?;
    cumulative.streamed_rows = checked_add(
        "streamed rows",
        cumulative.streamed_rows,
        delta.streamed_rows,
    )?;
    cumulative.modular_hits =
        checked_add("modular hits", cumulative.modular_hits, delta.modular_hits)?;
    cumulative.exact_lift_attempts = checked_add(
        "exact lift attempts",
        cumulative.exact_lift_attempts,
        delta.exact_lift_attempts,
    )?;
    cumulative.rejected_hits = checked_add(
        "rejected hits",
        cumulative.rejected_hits,
        delta.rejected_hits,
    )?;
    cumulative.alternative_nodes = checked_add(
        "alternative nodes",
        cumulative.alternative_nodes,
        delta.alternative_nodes,
    )?;
    cumulative.excluded_requests = checked_add(
        "excluded requests",
        cumulative.excluded_requests,
        delta.excluded_requests,
    )?;
    cumulative.excluded_request_coordinate_cells = checked_add(
        "excluded request coordinate cells",
        cumulative.excluded_request_coordinate_cells,
        delta.excluded_request_coordinate_cells,
    )?;
    cumulative.guard_predicates = checked_add(
        "guard predicates",
        cumulative.guard_predicates,
        delta.guard_predicates,
    )?;
    cumulative.guard_obligations = checked_add(
        "guard obligations",
        cumulative.guard_obligations,
        delta.guard_obligations,
    )?;
    cumulative.owner_compile_attempts = checked_add(
        "owner compile attempts",
        cumulative.owner_compile_attempts,
        delta.owner_compile_attempts,
    )?;
    Ok(())
}

fn remaining(
    resource: &'static str,
    limit: usize,
    used: usize,
) -> Result<usize, SpiredFixedPointError> {
    limit
        .checked_sub(used)
        .ok_or(SpiredFixedPointError::Invariant {
            detail: match resource {
                "target lane runs" => "target-lane census exceeded its aggregate limit",
                "probe attempts" => "probe-attempt census exceeded its aggregate limit",
                "scheduled requests" => "scheduled-request census exceeded its aggregate limit",
                "streamed rows" => "streamed-row census exceeded its aggregate limit",
                "modular hits" => "modular-hit census exceeded its aggregate limit",
                "exact lift attempts" => "exact-lift census exceeded its aggregate limit",
                "rejected hits" => "rejected-hit census exceeded its aggregate limit",
                "alternative nodes" => "alternative-node census exceeded its aggregate limit",
                "excluded requests" => "excluded-request census exceeded its aggregate limit",
                "excluded request coordinate cells" => {
                    "excluded-request coordinate census exceeded its aggregate limit"
                }
                "guard predicates" => "guard-predicate census exceeded its aggregate limit",
                "guard obligations" => "guard-obligation census exceeded its aggregate limit",
                "owner compile attempts" => "owner-compile census exceeded its aggregate limit",
                _ => "unknown target-work census exceeded its aggregate limit",
            },
        })
}

fn try_next_bounded(
    current: usize,
    limit: usize,
    resource: &'static str,
) -> Result<Option<usize>, SpiredFixedPointError> {
    let requested = checked_add(resource, current, 1)?;
    Ok((requested <= limit).then_some(requested))
}

fn limit_reason(
    resource: &'static str,
    current: usize,
    limit: usize,
) -> Result<SpiredFixedPointIncompleteReason, SpiredFixedPointError> {
    Ok(SpiredFixedPointIncompleteReason::ResourceLimit {
        resource,
        requested: checked_add(resource, current, 1)?,
        limit,
    })
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SpiredFixedPointError> {
    left.checked_add(right)
        .ok_or(SpiredFixedPointError::ResourceCountOverflow { resource })
}
