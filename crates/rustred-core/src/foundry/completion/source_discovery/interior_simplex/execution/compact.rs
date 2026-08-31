use crate::foundry::completion::source_discovery::scheduler::{
    ProbeLocalOutcome, ProbeLocalRunCensus, ProbeLocalSchedulerReport,
};

use super::InteriorSimplexExecutionError;
use super::limits::InteriorSimplexExecutionLimits;
use super::model::{
    InteriorSimplexIterationTelemetry, InteriorSimplexOutcomeTelemetry,
    InteriorSimplexProbeTelemetry, InteriorSimplexReplayRetention,
};
use super::resource::{check_limit, checked_add, try_reserve_exact};

/// Aggregate budget for compact iteration records retained across tasks.
///
/// It is updated only after one complete scheduler report has been compacted,
/// so a failed compaction never publishes a partial task report.
#[derive(Debug, Default)]
pub(super) struct RetainedIterationBudget {
    records: usize,
}

impl RetainedIterationBudget {
    pub(super) const fn records(&self) -> usize {
        self.records
    }
}

/// Consume one scheduler report and detach only plan-independent telemetry.
///
/// Fresh epochs, physical frames, request accumulators, sampled duals, and
/// exact circuits are dropped at this boundary. In particular,
/// `ExactTargetCircuit` still owns a physical-plan identity; canonical replay
/// must detach it through its dedicated admission seam before it can be kept.
pub(super) fn try_compact_scheduler_report(
    report: ProbeLocalSchedulerReport,
    limits: InteriorSimplexExecutionLimits,
    budget: &mut RetainedIterationBudget,
) -> Result<(Vec<InteriorSimplexProbeTelemetry>, ProbeLocalRunCensus), InteriorSimplexExecutionError>
{
    let census = report.census();
    let retained_iterations = report.probes().iter().try_fold(0usize, |count, probe| {
        checked_add(
            "retained compact iteration records",
            count,
            probe.iterations().len(),
        )
    })?;
    let aggregate_iterations = checked_add(
        "retained compact iteration records",
        budget.records,
        retained_iterations,
    )?;
    check_limit(
        "retained compact iteration records",
        aggregate_iterations,
        limits.max_retained_iteration_records,
    )?;

    let probes = report.into_probes();
    let mut compact = Vec::new();
    try_reserve_exact(&mut compact, probes.len(), "retained compact probe reports")?;
    for report in probes {
        let mut iterations = Vec::new();
        try_reserve_exact(
            &mut iterations,
            report.iterations().len(),
            "retained compact iteration records",
        )?;
        iterations.extend(report.iterations().iter().map(|record| {
            InteriorSimplexIterationTelemetry::new(
                record.epoch_ordinal(),
                record.request_count(),
                record.physical_rows(),
                record.physical_columns(),
                record.physical_entries(),
                record.allowed_columns(),
                record.forbidden_columns(),
                record.forbidden_rank(),
                record.augmented_rank(),
                record.disposition(),
            )
        }));
        let outcome = summarize_outcome(report.outcome());
        compact.push(InteriorSimplexProbeTelemetry::new(
            report.probe_ordinal(),
            report.probe().clone(),
            iterations,
            outcome,
        ));
    }

    budget.records = aggregate_iterations;
    Ok((compact, census))
}

fn summarize_outcome(outcome: &ProbeLocalOutcome) -> InteriorSimplexOutcomeTelemetry {
    match outcome {
        ProbeLocalOutcome::Replayed { epoch, circuit } => {
            InteriorSimplexOutcomeTelemetry::Replayed {
                exact_support: InteriorSimplexReplayRetention::UnsupportedEpochBoundCircuit,
                final_requests: epoch.requests().requests().len(),
                selected_sources: circuit.source_combination().len(),
                residual_terms: circuit.residual_terms().len(),
                pivot_guards: circuit.pivot_guards().len(),
                nonzero_guards: circuit.nonzero_guards().len(),
            }
        }
        ProbeLocalOutcome::SupportDidNotLift {
            epoch,
            inconclusive,
        } => InteriorSimplexOutcomeTelemetry::SupportDidNotLift {
            final_requests: epoch.requests().requests().len(),
            selected_sources: inconclusive.selected_source_instances().len(),
            exact_forbidden_rank: inconclusive.exact_forbidden_rank(),
            exact_augmented_rank: inconclusive.exact_augmented_rank(),
        },
        ProbeLocalOutcome::ExactLiftError { epoch, .. } => {
            InteriorSimplexOutcomeTelemetry::ExactLiftError {
                final_requests: epoch.requests().requests().len(),
            }
        }
        ProbeLocalOutcome::SampledDual(evidence) => {
            let census = evidence.census();
            InteriorSimplexOutcomeTelemetry::SampledDual {
                final_requests: evidence.final_requests().len(),
                obstruction_entries: evidence.obstruction().len(),
                structurally_incident_rows: census.structurally_incident_rows(),
                evaluated_unseen_rows: census.evaluated_unseen_rows(),
                evaluated_source_terms: census.evaluated_source_terms(),
                paired_source_terms: census.paired_source_terms(),
            }
        }
        ProbeLocalOutcome::BudgetStop { context, stop } => {
            InteriorSimplexOutcomeTelemetry::BudgetStop {
                final_requests: context.requests().map(|requests| requests.requests().len()),
                stage: stop.stage(),
                resource: stop.cause().resource(),
            }
        }
        ProbeLocalOutcome::Rejected { context, stage, .. } => {
            InteriorSimplexOutcomeTelemetry::Rejected {
                final_requests: context.requests().map(|requests| requests.requests().len()),
                stage: *stage,
            }
        }
        ProbeLocalOutcome::Stalled { epoch, stall } => InteriorSimplexOutcomeTelemetry::Stalled {
            final_requests: epoch.requests().requests().len(),
            nonzero_residual_requests: stall.nonzero_residual_requests(),
        },
    }
}
