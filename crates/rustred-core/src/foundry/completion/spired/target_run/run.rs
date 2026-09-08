use crate::foundry::completion::guard::ExactGuardProbeWitness;
use crate::foundry::completion::source_discovery::CampaignModularProbe;
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};

use super::super::{
    SignedL1ShellScheduler, SpiredCompactLift, SpiredExecutionCase, SpiredStreamingDiscovery,
    try_lift_spired_compact_support, try_promote_spired_replayed_rule_cell,
};
use super::{
    SpiredTargetRunCensus, SpiredTargetRunError, SpiredTargetRunErrorCause, SpiredTargetRunLimits,
    SpiredTargetRunOutcome, SpiredTargetRunReport, SpiredTargetRunStage,
};

const CHUNKS: &str = "target-run materialized chunks";
const SHELLS: &str = "target-run opened or completed shells";
const REQUESTS: &str = "target-run scheduled requests";
const REQUEST_COORDINATES: &str = "target-run scheduled request coordinate cells";
const DEPTH: &str = "target-run next signed-L1 depth";

/// Run one deterministic signed-L1 source lane through ordinary RuleCell
/// promotion, stopping at its first modular target-rank gain.
pub(crate) fn try_run_spired_target(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    case: &SpiredExecutionCase,
    probe: &CampaignModularProbe,
    limits: SpiredTargetRunLimits,
) -> Result<SpiredTargetRunReport, SpiredTargetRunError> {
    try_run_spired_target_inner(generator, completed, case, probe, None, limits)
}

/// Run one guarded target only after binding its exact branch predicates to
/// the retained raw probe. The witness is checked before any streaming state
/// or modular row is constructed.
pub(crate) fn try_run_spired_guarded_target(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    case: &SpiredExecutionCase,
    probe: &CampaignModularProbe,
    witness: &ExactGuardProbeWitness,
    limits: SpiredTargetRunLimits,
) -> Result<SpiredTargetRunReport, SpiredTargetRunError> {
    try_run_spired_target_inner(generator, completed, case, probe, Some(witness), limits)
}

#[allow(clippy::too_many_arguments)]
fn try_run_spired_target_inner(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    case: &SpiredExecutionCase,
    probe: &CampaignModularProbe,
    guard_witness: Option<&ExactGuardProbeWitness>,
    limits: SpiredTargetRunLimits,
) -> Result<SpiredTargetRunReport, SpiredTargetRunError> {
    let mut census = SpiredTargetRunCensus::default();
    if limits.request_chunk_size == 0 {
        return Err(component_error(
            SpiredTargetRunStage::Scheduling,
            census,
            SpiredTargetRunErrorCause::Scheduling(
                super::super::SpiredFoundationError::EmptyRequestChunk,
            ),
        ));
    }
    let scheduler = SignedL1ShellScheduler::try_new(
        case.arity(),
        completed.source_row_count(),
        limits.schedule,
    )
    .map_err(|error| {
        component_error(
            SpiredTargetRunStage::Scheduling,
            census,
            SpiredTargetRunErrorCause::Scheduling(error),
        )
    })?;
    let discovery = match guard_witness {
        Some(witness) => SpiredStreamingDiscovery::try_new_guarded(
            generator.context(),
            completed,
            case,
            probe,
            witness,
            limits.streaming,
        ),
        None => SpiredStreamingDiscovery::try_new(
            generator.context(),
            completed,
            case,
            probe,
            limits.streaming,
        ),
    };
    let mut discovery = discovery.map_err(|error| {
        component_error(
            SpiredTargetRunStage::Streaming,
            census,
            SpiredTargetRunErrorCause::Streaming(error),
        )
    })?;

    let mut depth = 0usize;
    loop {
        census.set_current_depth(depth);
        let mut shell = scheduler.try_depth_shell(depth).map_err(|error| {
            component_error(
                SpiredTargetRunStage::Scheduling,
                census,
                SpiredTargetRunErrorCause::Scheduling(error),
            )
        })?;
        census.set_shells_opened(checked_add(SHELLS, census.shells_opened(), 1, census)?);

        loop {
            let remaining = shell.remaining_request_count();
            if remaining == 0 {
                let exhausted =
                    shell
                        .try_next_chunk(limits.request_chunk_size)
                        .map_err(|error| {
                            component_error(
                                SpiredTargetRunStage::Scheduling,
                                census,
                                SpiredTargetRunErrorCause::Scheduling(error),
                            )
                        })?;
                if exhausted.is_some() {
                    return Err(invariant_error(
                        census,
                        "an exhausted signed-L1 shell emitted another chunk",
                    ));
                }
                census.set_shells_completed(checked_add(
                    SHELLS,
                    census.shells_completed(),
                    1,
                    census,
                )?);
                break;
            }

            let residual_request_budget = limits
                .max_total_scheduled_requests
                .checked_sub(census.scheduled_requests())
                .ok_or_else(|| {
                    invariant_error(
                        census,
                        "scheduled-request census exceeded its aggregate limit",
                    )
                })?;
            let residual_coordinate_budget = limits
                .max_total_request_coordinate_cells
                .checked_sub(census.scheduled_request_coordinate_cells())
                .ok_or_else(|| {
                    invariant_error(
                        census,
                        "scheduled-coordinate census exceeded its aggregate limit",
                    )
                })?;
            let chunk_capacity = remaining
                .min(limits.request_chunk_size)
                .min(residual_request_budget)
                .min(residual_coordinate_budget / case.arity());
            if chunk_capacity == 0 {
                return Err(exhausted_schedule_budget_error(
                    case.arity(),
                    residual_request_budget,
                    census,
                    limits,
                ));
            }
            let next_chunks = checked_add(CHUNKS, census.chunks_materialized(), 1, census)?;
            check_limit(CHUNKS, next_chunks, limits.max_chunks, census)?;
            let next_requests = checked_add(
                REQUESTS,
                census.scheduled_requests(),
                chunk_capacity,
                census,
            )?;
            check_limit(
                REQUESTS,
                next_requests,
                limits.max_total_scheduled_requests,
                census,
            )?;
            let chunk_coordinates =
                checked_mul(REQUEST_COORDINATES, chunk_capacity, case.arity(), census)?;
            let next_coordinates = checked_add(
                REQUEST_COORDINATES,
                census.scheduled_request_coordinate_cells(),
                chunk_coordinates,
                census,
            )?;
            check_limit(
                REQUEST_COORDINATES,
                next_coordinates,
                limits.max_total_request_coordinate_cells,
                census,
            )?;

            let chunk = shell
                .try_next_chunk(chunk_capacity)
                .map_err(|error| {
                    component_error(
                        SpiredTargetRunStage::Scheduling,
                        census,
                        SpiredTargetRunErrorCause::Scheduling(error),
                    )
                })?
                .ok_or_else(|| {
                    invariant_error(
                        census,
                        "a nonempty signed-L1 shell failed to emit its next chunk",
                    )
                })?;
            if chunk.len() != chunk_capacity {
                return Err(invariant_error(
                    census,
                    "a signed-L1 chunk differed from its exact preflight capacity",
                ));
            }
            let chunk_ordinal = census.chunks_materialized();
            census.set_chunks_materialized(next_chunks);
            census.set_scheduled_requests(next_requests);
            census.set_scheduled_request_coordinate_cells(next_coordinates);

            for (request_ordinal_in_chunk, request) in chunk.requests().iter().enumerate() {
                let rows_before = discovery.rows_consumed();
                // Structural pre-registration is intentionally scoped to one
                // row. A later request in this already materialized window
                // must neither add columns nor consume preparation limits
                // before an earlier row has had the chance to hit.
                let hit = discovery
                    .try_consume_chunk(std::slice::from_ref(request))
                    .map_err(|error| {
                        census.set_streamed_rows(discovery.rows_consumed());
                        component_error(
                            SpiredTargetRunStage::Streaming,
                            census,
                            SpiredTargetRunErrorCause::Streaming(error),
                        )
                    })?;
                census.set_streamed_rows(discovery.rows_consumed());
                let consumed = discovery
                    .rows_consumed()
                    .checked_sub(rows_before)
                    .ok_or_else(|| {
                        streaming_invariant_error(
                            census,
                            "streaming row count moved backwards within a chunk",
                        )
                    })?;
                if consumed != 1 {
                    return Err(streaming_invariant_error(
                        census,
                        "one admitted request did not consume exactly one modular row",
                    ));
                }
                let Some(hit) = hit else {
                    continue;
                };

                let shell_request_ordinal = chunk
                    .first_request_ordinal()
                    .checked_add(request_ordinal_in_chunk)
                    .ok_or_else(|| {
                        count_overflow_error(REQUESTS, SpiredTargetRunStage::Streaming, census)
                    })?;
                census.record_hit(
                    depth,
                    chunk_ordinal,
                    shell_request_ordinal,
                    request_ordinal_in_chunk,
                    hit.support().len(),
                );

                let lift = try_lift_spired_compact_support(
                    case,
                    generator,
                    completed,
                    &hit,
                    discovery.probe(),
                    limits.compact_lift,
                )
                .map_err(|error| {
                    component_error(
                        SpiredTargetRunStage::CompactLift,
                        census,
                        SpiredTargetRunErrorCause::CompactLift(error),
                    )
                })?;
                let outcome = match lift {
                    SpiredCompactLift::Replayed(replayed) => {
                        let promoted = try_promote_spired_replayed_rule_cell(
                            generator.context(),
                            replayed,
                            limits.promotion,
                        )
                        .map_err(|error| {
                            component_error(
                                SpiredTargetRunStage::RuleCellPromotion,
                                census,
                                SpiredTargetRunErrorCause::RuleCellPromotion(error),
                            )
                        })?;
                        SpiredTargetRunOutcome::RuleCell(promoted)
                    }
                    inconclusive @ (SpiredCompactLift::FreshFrameDidNotHit { .. }
                    | SpiredCompactLift::ExactSupportDidNotLift { .. }) => {
                        SpiredTargetRunOutcome::CompactInconclusive(inconclusive)
                    }
                };
                return Ok(SpiredTargetRunReport::new(census, outcome));
            }
        }

        if depth == limits.max_depth_inclusive {
            return Ok(SpiredTargetRunReport::new(
                census,
                SpiredTargetRunOutcome::SearchDepthExhausted,
            ));
        }
        depth = depth
            .checked_add(1)
            .ok_or_else(|| count_overflow_error(DEPTH, SpiredTargetRunStage::Scheduling, census))?;
    }
}

fn exhausted_schedule_budget_error(
    arity: usize,
    residual_request_budget: usize,
    census: SpiredTargetRunCensus,
    limits: SpiredTargetRunLimits,
) -> SpiredTargetRunError {
    let (resource, requested, limit) = if residual_request_budget == 0 {
        let Some(requested) = census.scheduled_requests().checked_add(1) else {
            return count_overflow_error(REQUESTS, SpiredTargetRunStage::Scheduling, census);
        };
        (REQUESTS, requested, limits.max_total_scheduled_requests)
    } else {
        let Some(requested) = census
            .scheduled_request_coordinate_cells()
            .checked_add(arity)
        else {
            return count_overflow_error(
                REQUEST_COORDINATES,
                SpiredTargetRunStage::Scheduling,
                census,
            );
        };
        (
            REQUEST_COORDINATES,
            requested,
            limits.max_total_request_coordinate_cells,
        )
    };
    SpiredTargetRunError::new(
        SpiredTargetRunStage::Scheduling,
        census,
        SpiredTargetRunErrorCause::ResourceLimit {
            resource,
            requested,
            limit,
        },
    )
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
    census: SpiredTargetRunCensus,
) -> Result<usize, SpiredTargetRunError> {
    left.checked_add(right)
        .ok_or_else(|| count_overflow_error(resource, SpiredTargetRunStage::Scheduling, census))
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
    census: SpiredTargetRunCensus,
) -> Result<usize, SpiredTargetRunError> {
    left.checked_mul(right)
        .ok_or_else(|| count_overflow_error(resource, SpiredTargetRunStage::Scheduling, census))
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
    census: SpiredTargetRunCensus,
) -> Result<(), SpiredTargetRunError> {
    if requested > limit {
        Err(SpiredTargetRunError::new(
            SpiredTargetRunStage::Scheduling,
            census,
            SpiredTargetRunErrorCause::ResourceLimit {
                resource,
                requested,
                limit,
            },
        ))
    } else {
        Ok(())
    }
}

fn count_overflow_error(
    resource: &'static str,
    stage: SpiredTargetRunStage,
    census: SpiredTargetRunCensus,
) -> SpiredTargetRunError {
    SpiredTargetRunError::new(
        stage,
        census,
        SpiredTargetRunErrorCause::ResourceCountOverflow { resource },
    )
}

fn invariant_error(census: SpiredTargetRunCensus, detail: &'static str) -> SpiredTargetRunError {
    component_error(
        SpiredTargetRunStage::Scheduling,
        census,
        SpiredTargetRunErrorCause::Scheduling(super::super::SpiredFoundationError::Invariant {
            detail,
        }),
    )
}

fn streaming_invariant_error(
    census: SpiredTargetRunCensus,
    detail: &'static str,
) -> SpiredTargetRunError {
    component_error(
        SpiredTargetRunStage::Streaming,
        census,
        SpiredTargetRunErrorCause::Streaming(super::super::SpiredStreamingError::Invariant {
            detail,
        }),
    )
}

const fn component_error(
    stage: SpiredTargetRunStage,
    census: SpiredTargetRunCensus,
    cause: SpiredTargetRunErrorCause,
) -> SpiredTargetRunError {
    SpiredTargetRunError::new(stage, census, cause)
}
