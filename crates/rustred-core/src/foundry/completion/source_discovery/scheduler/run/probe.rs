//! One independent probe-local obstruction campaign.

mod outcome;
mod proposal;

use crate::foundry::completion::frame::exact::{ExactCircuitLift, try_lift_exact_circuit};
use crate::foundry::completion::frame::modular::ModularTargetQuery;
use crate::foundry::completion::stratum::{ImmutableOwnerSnapshot, MaximalStratumAnchor};
use crate::identity::{
    CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator, TranslatedSourceRequest,
};
use crate::sector::OrderingPolicy;

use super::super::super::{
    AccumulatedSourceRequests, CampaignModularProbe, CampaignRequestMerge, GrowingTaskEpochState,
    OrdinarySourceIncidenceIndex, SampledDeclaredModuleDual, SourceDiscoveryError,
};
use super::super::{
    ProbeLocalBudgetCause, ProbeLocalBudgetScope, ProbeLocalBudgetStop,
    ProbeLocalIterationDisposition, ProbeLocalOutcome, ProbeLocalProbeReport, ProbeLocalRejection,
    ProbeLocalSchedulerError, ProbeLocalSchedulerLimits, ProbeLocalStage, ProbeLocalStall,
    ProbeLocalStopContext,
};
use super::budget::{
    AGGREGATE_MATERIALIZED_SOURCE_TERMS, AGGREGATE_RESIDUAL_SOURCE_TERM_WORK, ITERATION_RECORDS,
    RunBudget, check_outer, checked_budget_add,
};
use outcome::{
    campaign_stop_or_rejection, finish_probe, sampled_dual_stop_or_rejection,
    source_stop_or_rejection,
};
use proposal::try_rank_residual_proposals;

const ITERATIONS_PER_PROBE: &str = "probe-local iterations per probe";
const REQUESTS_PER_PROBE: &str = "probe-local requests per probe";
const REQUEST_COORDINATES_PER_PROBE: &str = "probe-local request coordinate cells per probe";
const EPOCH_ORDINAL: &str = "probe-local epoch ordinal";

pub(super) fn unexecuted_suffix_report(
    probe_ordinal: usize,
    probe: CampaignModularProbe,
    triggering_probe_ordinal: usize,
    resource: &'static str,
) -> ProbeLocalProbeReport {
    outcome::unexecuted_suffix_report(probe_ordinal, probe, triggering_probe_ordinal, resource)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_single_probe(
    probe_ordinal: usize,
    probe: CampaignModularProbe,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    incidence: &OrdinarySourceIncidenceIndex<'_>,
    target_shift: &IntegralShift,
    stratum: &MaximalStratumAnchor,
    owners: &ImmutableOwnerSnapshot,
    ordering: OrderingPolicy,
    limits: ProbeLocalSchedulerLimits,
    budget: &mut RunBudget,
) -> Result<ProbeLocalProbeReport, ProbeLocalSchedulerError> {
    // Recompute this structural bootstrap for every probe. Even though its raw
    // identities are deterministic, no request accumulator crosses this
    // boundary.
    let bootstrap = match incidence.try_nominate_target_unit(target_shift, limits.source_discovery)
    {
        Ok(bootstrap) => bootstrap,
        Err(error) => {
            let outcome = source_stop_or_rejection(
                probe_ordinal,
                0,
                ProbeLocalStage::BootstrapNomination,
                ProbeLocalStopContext::BeforeBootstrap,
                error,
            );
            return Ok(finish_probe(probe_ordinal, probe, Vec::new(), outcome));
        }
    };
    let mut requests = match AccumulatedSourceRequests::try_new(
        incidence.arity(),
        bootstrap.requests().iter().cloned(),
        limits.campaign,
    ) {
        Ok(requests) => requests,
        Err(error) => {
            let outcome = campaign_stop_or_rejection(
                probe_ordinal,
                0,
                ProbeLocalStage::BootstrapAccumulation,
                ProbeLocalStopContext::BeforeBootstrap,
                error,
            );
            return Ok(finish_probe(probe_ordinal, probe, Vec::new(), outcome));
        }
    };
    if let Err(cause) = verify_probe_requests(&requests, limits) {
        let stop = ProbeLocalBudgetStop::new(
            probe_ordinal,
            0,
            ProbeLocalStage::BootstrapAccumulation,
            cause,
        );
        return Ok(finish_probe(
            probe_ordinal,
            probe,
            Vec::new(),
            ProbeLocalOutcome::BudgetStop {
                context: ProbeLocalStopContext::Requests(requests),
                stop,
            },
        ));
    }
    let mut epochs = GrowingTaskEpochState::new(
        target_shift.clone(),
        stratum.clone(),
        owners.clone(),
        ordering,
    );

    let mut records = Vec::new();
    loop {
        let epoch_ordinal = epochs.next_epoch_ordinal();
        let requested_iteration = match epoch_ordinal.checked_add(1) {
            Some(value) => value,
            None => {
                let stop = ProbeLocalBudgetStop::new(
                    probe_ordinal,
                    epoch_ordinal,
                    ProbeLocalStage::EpochAdmission,
                    ProbeLocalBudgetCause::CountOverflow {
                        scope: ProbeLocalBudgetScope::Probe,
                        resource: EPOCH_ORDINAL,
                    },
                );
                return Ok(finish_probe(
                    probe_ordinal,
                    probe,
                    records,
                    ProbeLocalOutcome::BudgetStop {
                        context: ProbeLocalStopContext::Requests(requests),
                        stop,
                    },
                ));
            }
        };
        if requested_iteration > limits.max_iterations_per_probe {
            let stop = ProbeLocalBudgetStop::new(
                probe_ordinal,
                epoch_ordinal,
                ProbeLocalStage::EpochAdmission,
                ProbeLocalBudgetCause::Outer {
                    scope: ProbeLocalBudgetScope::Probe,
                    resource: ITERATIONS_PER_PROBE,
                    requested: requested_iteration,
                    limit: limits.max_iterations_per_probe,
                },
            );
            return Ok(finish_probe(
                probe_ordinal,
                probe,
                records,
                ProbeLocalOutcome::BudgetStop {
                    context: ProbeLocalStopContext::Requests(requests),
                    stop,
                },
            ));
        }
        let source_term_work = match translated_source_term_work(
            incidence,
            requests.requests(),
            AGGREGATE_MATERIALIZED_SOURCE_TERMS,
        ) {
            Ok(work) => work,
            Err(RequestSourceTermWorkError::Budget(cause)) => {
                let stop = ProbeLocalBudgetStop::new(
                    probe_ordinal,
                    epoch_ordinal,
                    ProbeLocalStage::EpochAdmission,
                    cause,
                );
                return Ok(finish_probe(
                    probe_ordinal,
                    probe,
                    records,
                    ProbeLocalOutcome::BudgetStop {
                        context: ProbeLocalStopContext::Requests(requests),
                        stop,
                    },
                ));
            }
            Err(RequestSourceTermWorkError::InvalidSourceOrdinal) => {
                return Ok(finish_probe(
                    probe_ordinal,
                    probe,
                    records,
                    ProbeLocalOutcome::Rejected {
                        context: ProbeLocalStopContext::Requests(requests),
                        stage: ProbeLocalStage::EpochAdmission,
                        error: ProbeLocalRejection::SourceDiscovery(
                            SourceDiscoveryError::Invariant {
                                detail: "probe-local accumulator names a source outside its sealed incidence module",
                            },
                        ),
                    },
                ));
            }
        };
        if let Err(cause) = budget.try_admit_epoch(requests.len(), source_term_work, limits) {
            let stop = ProbeLocalBudgetStop::new(
                probe_ordinal,
                epoch_ordinal,
                ProbeLocalStage::EpochAdmission,
                cause,
            );
            return Ok(finish_probe(
                probe_ordinal,
                probe,
                records,
                ProbeLocalOutcome::BudgetStop {
                    context: ProbeLocalStopContext::Requests(requests),
                    stop,
                },
            ));
        }

        let epoch = match epochs.try_next(generator, completed, requests.clone(), limits.campaign) {
            Ok(epoch) => epoch,
            Err(error) => {
                let outcome = campaign_stop_or_rejection(
                    probe_ordinal,
                    epoch_ordinal,
                    ProbeLocalStage::EpochBuild,
                    ProbeLocalStopContext::Requests(requests),
                    error,
                );
                return Ok(finish_probe(probe_ordinal, probe, records, outcome));
            }
        };
        drop(requests);
        if let Err(cause) =
            budget.try_admit_modular_work(epoch.telemetry().physical_entries(), limits)
        {
            let stop = ProbeLocalBudgetStop::new(
                probe_ordinal,
                epoch_ordinal,
                ProbeLocalStage::ModularQuery,
                cause,
            );
            return Ok(finish_probe(
                probe_ordinal,
                probe,
                records,
                ProbeLocalOutcome::BudgetStop {
                    context: ProbeLocalStopContext::Epoch(epoch),
                    stop,
                },
            ));
        }
        let query = match epoch.try_query(generator.context(), &probe, limits.campaign) {
            Ok(query) => query,
            Err(error) => {
                let outcome = campaign_stop_or_rejection(
                    probe_ordinal,
                    epoch_ordinal,
                    ProbeLocalStage::ModularQuery,
                    ProbeLocalStopContext::Epoch(epoch),
                    error,
                );
                return Ok(finish_probe(probe_ordinal, probe, records, outcome));
            }
        };
        if records.try_reserve(1).is_err() {
            drop(query);
            let requested = records.len().saturating_add(1);
            let stop = ProbeLocalBudgetStop::new(
                probe_ordinal,
                epoch_ordinal,
                ProbeLocalStage::EpochAdmission,
                ProbeLocalBudgetCause::AllocationFailure {
                    scope: ProbeLocalBudgetScope::Probe,
                    resource: ITERATION_RECORDS,
                    requested,
                },
            );
            return Ok(finish_probe(
                probe_ordinal,
                probe,
                records,
                ProbeLocalOutcome::BudgetStop {
                    context: ProbeLocalStopContext::Epoch(epoch),
                    stop,
                },
            ));
        }
        if let Err(cause) = budget.try_admit_iteration_record(limits) {
            drop(query);
            let stop = ProbeLocalBudgetStop::new(
                probe_ordinal,
                epoch_ordinal,
                ProbeLocalStage::EpochAdmission,
                cause,
            );
            return Ok(finish_probe(
                probe_ordinal,
                probe,
                records,
                ProbeLocalOutcome::BudgetStop {
                    context: ProbeLocalStopContext::Epoch(epoch),
                    stop,
                },
            ));
        }

        match query.query() {
            ModularTargetQuery::Hit(hit) => {
                records.push(outcome::iteration_record(
                    &epoch,
                    &query,
                    ProbeLocalIterationDisposition::ModularHit,
                ));
                if let Err(cause) = budget.try_admit_exact(limits) {
                    drop(query);
                    let stop = ProbeLocalBudgetStop::new(
                        probe_ordinal,
                        epoch_ordinal,
                        ProbeLocalStage::ExactLift,
                        cause,
                    );
                    return Ok(finish_probe(
                        probe_ordinal,
                        probe,
                        records,
                        ProbeLocalOutcome::BudgetStop {
                            context: ProbeLocalStopContext::Epoch(epoch),
                            stop,
                        },
                    ));
                }
                // The live query binds the modular hit and exact partition to
                // this epoch. No hit or ordinal is retained past this call.
                let exact = try_lift_exact_circuit(
                    generator.context(),
                    hit,
                    query.partition(),
                    limits.exact_circuit,
                );
                drop(query);
                let outcome = match exact {
                    Ok(ExactCircuitLift::Replayed(circuit)) => {
                        ProbeLocalOutcome::Replayed { epoch, circuit }
                    }
                    Ok(ExactCircuitLift::ModularSupportDidNotLift(inconclusive)) => {
                        ProbeLocalOutcome::SupportDidNotLift {
                            epoch,
                            inconclusive,
                        }
                    }
                    Err(error) => ProbeLocalOutcome::ExactLiftError { epoch, error },
                };
                return Ok(finish_probe(probe_ordinal, probe, records, outcome));
            }
            ModularTargetQuery::NoHitWithObstruction(obstruction) => {
                let nominations = match incidence
                    .try_nominate_obstruction(obstruction, limits.source_discovery)
                {
                    Ok(nominations) => nominations,
                    Err(error) => {
                        records.push(outcome::iteration_record(
                            &epoch,
                            &query,
                            ProbeLocalIterationDisposition::NoHitStopped {
                                stage: ProbeLocalStage::ObstructionNomination,
                            },
                        ));
                        drop(query);
                        let outcome = source_stop_or_rejection(
                            probe_ordinal,
                            epoch_ordinal,
                            ProbeLocalStage::ObstructionNomination,
                            ProbeLocalStopContext::Epoch(epoch),
                            error,
                        );
                        return Ok(finish_probe(probe_ordinal, probe, records, outcome));
                    }
                };
                let residual_source_term_work = match translated_source_term_work(
                    incidence,
                    nominations.requests(),
                    AGGREGATE_RESIDUAL_SOURCE_TERM_WORK,
                ) {
                    Ok(work) => work,
                    Err(RequestSourceTermWorkError::Budget(cause)) => {
                        records.push(outcome::iteration_record(
                            &epoch,
                            &query,
                            ProbeLocalIterationDisposition::NoHitStopped {
                                stage: ProbeLocalStage::ResidualEvaluation,
                            },
                        ));
                        drop(query);
                        let stop = ProbeLocalBudgetStop::new(
                            probe_ordinal,
                            epoch_ordinal,
                            ProbeLocalStage::ResidualEvaluation,
                            cause,
                        );
                        return Ok(finish_probe(
                            probe_ordinal,
                            probe,
                            records,
                            ProbeLocalOutcome::BudgetStop {
                                context: ProbeLocalStopContext::Epoch(epoch),
                                stop,
                            },
                        ));
                    }
                    Err(RequestSourceTermWorkError::InvalidSourceOrdinal) => {
                        records.push(outcome::iteration_record(
                            &epoch,
                            &query,
                            ProbeLocalIterationDisposition::NoHitStopped {
                                stage: ProbeLocalStage::ResidualEvaluation,
                            },
                        ));
                        drop(query);
                        return Ok(finish_probe(
                            probe_ordinal,
                            probe,
                            records,
                            ProbeLocalOutcome::Rejected {
                                context: ProbeLocalStopContext::Epoch(epoch),
                                stage: ProbeLocalStage::ResidualEvaluation,
                                error: ProbeLocalRejection::SourceDiscovery(
                                    SourceDiscoveryError::Invariant {
                                        detail: "residual nomination names a source outside its sealed incidence module",
                                    },
                                ),
                            },
                        ));
                    }
                };
                if let Err(cause) = budget.try_admit_residual_work(
                    nominations.requests().len(),
                    residual_source_term_work,
                    limits,
                ) {
                    records.push(outcome::iteration_record(
                        &epoch,
                        &query,
                        ProbeLocalIterationDisposition::NoHitStopped {
                            stage: ProbeLocalStage::ResidualEvaluation,
                        },
                    ));
                    drop(query);
                    let stop = ProbeLocalBudgetStop::new(
                        probe_ordinal,
                        epoch_ordinal,
                        ProbeLocalStage::ResidualEvaluation,
                        cause,
                    );
                    return Ok(finish_probe(
                        probe_ordinal,
                        probe,
                        records,
                        ProbeLocalOutcome::BudgetStop {
                            context: ProbeLocalStopContext::Epoch(epoch),
                            stop,
                        },
                    ));
                }
                let residuals = match incidence.try_retain_nonzero_residuals_for_partition(
                    generator,
                    completed,
                    &nominations,
                    query.sampled(),
                    obstruction,
                    query.partition(),
                    limits.source_discovery,
                ) {
                    Ok(residuals) => residuals,
                    Err(error) => {
                        records.push(outcome::iteration_record(
                            &epoch,
                            &query,
                            ProbeLocalIterationDisposition::NoHitStopped {
                                stage: ProbeLocalStage::ResidualEvaluation,
                            },
                        ));
                        drop(query);
                        let outcome = source_stop_or_rejection(
                            probe_ordinal,
                            epoch_ordinal,
                            ProbeLocalStage::ResidualEvaluation,
                            ProbeLocalStopContext::Epoch(epoch),
                            error,
                        );
                        return Ok(finish_probe(probe_ordinal, probe, records, outcome));
                    }
                };
                if residuals.requests().is_empty() {
                    records.push(outcome::iteration_record(
                        &epoch,
                        &query,
                        ProbeLocalIterationDisposition::NoHitEmptyResidual {
                            nominated_requests: nominations.requests().len(),
                        },
                    ));
                    let dual = SampledDeclaredModuleDual::try_new(
                        incidence,
                        &epoch,
                        &query,
                        &nominations,
                        &residuals,
                        limits.source_discovery,
                    );
                    drop(query);
                    return Ok(finish_probe(
                        probe_ordinal,
                        probe,
                        records,
                        match dual {
                            Ok(evidence) => ProbeLocalOutcome::SampledDual(evidence),
                            Err(error) => sampled_dual_stop_or_rejection(
                                probe_ordinal,
                                epoch_ordinal,
                                ProbeLocalStopContext::Epoch(epoch),
                                error,
                            ),
                        },
                    ));
                }

                let nonzero_residual_requests = residuals.requests().len();
                // The residual census above is deliberately exhaustive: only
                // its empty result can authorize a sampled dual.  Frame growth
                // is a separate, non-authoritative proposal policy.  Admit a
                // deterministic frontier-ranked prefix so one obstruction
                // cannot inflate the next exact frame by thousands of
                // translations.
                // Requests not selected here are not forbidden or forgotten;
                // a fresh obstruction may nominate them again in a later
                // epoch, while already admitted requests are excluded by the
                // incidence boundary.
                let proposals = match try_rank_residual_proposals(
                    &residuals,
                    limits.max_residual_proposals_per_iteration,
                ) {
                    Ok(proposals) => proposals,
                    Err(ProbeLocalSchedulerError::AllocationFailure {
                        resource,
                        requested,
                    }) => {
                        records.push(outcome::iteration_record(
                            &epoch,
                            &query,
                            ProbeLocalIterationDisposition::NoHitStopped {
                                stage: ProbeLocalStage::RequestMerge,
                            },
                        ));
                        drop(query);
                        let stop = ProbeLocalBudgetStop::new(
                            probe_ordinal,
                            epoch_ordinal,
                            ProbeLocalStage::RequestMerge,
                            ProbeLocalBudgetCause::AllocationFailure {
                                scope: ProbeLocalBudgetScope::Probe,
                                resource,
                                requested,
                            },
                        );
                        return Ok(finish_probe(
                            probe_ordinal,
                            probe,
                            records,
                            ProbeLocalOutcome::BudgetStop {
                                context: ProbeLocalStopContext::Epoch(epoch),
                                stop,
                            },
                        ));
                    }
                    Err(error) => return Err(error),
                };
                let proposed_residual_requests = proposals.len();
                if let Err(cause) = budget.try_admit_merge_work(
                    epoch.requests().len(),
                    proposed_residual_requests,
                    limits,
                ) {
                    records.push(outcome::iteration_record(
                        &epoch,
                        &query,
                        ProbeLocalIterationDisposition::NoHitStopped {
                            stage: ProbeLocalStage::RequestMerge,
                        },
                    ));
                    drop(query);
                    let stop = ProbeLocalBudgetStop::new(
                        probe_ordinal,
                        epoch_ordinal,
                        ProbeLocalStage::RequestMerge,
                        cause,
                    );
                    return Ok(finish_probe(
                        probe_ordinal,
                        probe,
                        records,
                        ProbeLocalOutcome::BudgetStop {
                            context: ProbeLocalStopContext::Epoch(epoch),
                            stop,
                        },
                    ));
                }
                let merged = epoch
                    .requests()
                    .try_merge_candidates(proposals, limits.campaign);
                match merged {
                    Ok(CampaignRequestMerge::Augmented {
                        requests: augmented,
                        telemetry,
                    }) => {
                        records.push(outcome::iteration_record(
                            &epoch,
                            &query,
                            ProbeLocalIterationDisposition::NoHitAugmented {
                                nominated_requests: nominations.requests().len(),
                                nonzero_residual_requests,
                                added_requests: telemetry.added_requests(),
                            },
                        ));
                        drop(query);
                        if let Err(cause) = verify_probe_requests(&augmented, limits) {
                            let stop = ProbeLocalBudgetStop::new(
                                probe_ordinal,
                                epoch_ordinal,
                                ProbeLocalStage::RequestMerge,
                                cause,
                            );
                            return Ok(finish_probe(
                                probe_ordinal,
                                probe,
                                records,
                                ProbeLocalOutcome::BudgetStop {
                                    context: ProbeLocalStopContext::Requests(augmented),
                                    stop,
                                },
                            ));
                        }
                        requests = augmented;
                    }
                    Ok(CampaignRequestMerge::CandidateBatchExhausted(exhaustion)) => {
                        records.push(outcome::iteration_record(
                            &epoch,
                            &query,
                            ProbeLocalIterationDisposition::NoHitStalled {
                                nominated_requests: nominations.requests().len(),
                                nonzero_residual_requests,
                            },
                        ));
                        drop(query);
                        let stall = ProbeLocalStall::new(
                            probe_ordinal,
                            epoch_ordinal,
                            nonzero_residual_requests,
                            exhaustion,
                        );
                        return Ok(finish_probe(
                            probe_ordinal,
                            probe,
                            records,
                            ProbeLocalOutcome::Stalled { epoch, stall },
                        ));
                    }
                    Err(error) => {
                        records.push(outcome::iteration_record(
                            &epoch,
                            &query,
                            ProbeLocalIterationDisposition::NoHitStopped {
                                stage: ProbeLocalStage::RequestMerge,
                            },
                        ));
                        drop(query);
                        let outcome = campaign_stop_or_rejection(
                            probe_ordinal,
                            epoch_ordinal,
                            ProbeLocalStage::RequestMerge,
                            ProbeLocalStopContext::Epoch(epoch),
                            error,
                        );
                        return Ok(finish_probe(probe_ordinal, probe, records, outcome));
                    }
                }
            }
        }
    }
}

/// Exact translated-term work for one request set, computed before a fresh
/// epoch materializes any selected row. This is both tighter and safer than
/// using `request_count * total module terms` as an aggregate proxy.
enum RequestSourceTermWorkError {
    Budget(ProbeLocalBudgetCause),
    InvalidSourceOrdinal,
}

fn translated_source_term_work(
    incidence: &OrdinarySourceIncidenceIndex<'_>,
    requests: &[TranslatedSourceRequest],
    resource: &'static str,
) -> Result<usize, RequestSourceTermWorkError> {
    let mut work = 0usize;
    for request in requests {
        let terms = incidence
            .sources()
            .get(request.source_ordinal())
            .map(|source| source.terms().len())
            .ok_or(RequestSourceTermWorkError::InvalidSourceOrdinal)?;
        work = checked_budget_add(ProbeLocalBudgetScope::Aggregate, resource, work, terms)
            .map_err(RequestSourceTermWorkError::Budget)?;
    }
    Ok(work)
}

fn verify_probe_requests(
    requests: &AccumulatedSourceRequests,
    limits: ProbeLocalSchedulerLimits,
) -> Result<(), ProbeLocalBudgetCause> {
    check_outer(
        ProbeLocalBudgetScope::Probe,
        REQUESTS_PER_PROBE,
        requests.len(),
        limits.max_requests_per_probe,
    )?;
    let cells = requests.arity().checked_mul(requests.len()).ok_or(
        ProbeLocalBudgetCause::CountOverflow {
            scope: ProbeLocalBudgetScope::Probe,
            resource: REQUEST_COORDINATES_PER_PROBE,
        },
    )?;
    check_outer(
        ProbeLocalBudgetScope::Probe,
        REQUEST_COORDINATES_PER_PROBE,
        cells,
        limits.max_request_coordinate_cells_per_probe,
    )
}
