//! Input-driven shared finite-target inspection, not parametric family closure.
mod feedback;
mod input;
mod prepare;
pub use feedback::{RoutedFeedbackOptions, RoutedFeedbackRoundResult, RoutedFeedbackSession};
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use crate::AppError;
use rustred::reduction::ReductionLimits;
use rustred::solver::{
    CandidateRoutedCampaignFailure, CandidateRoutedCampaignSnapshot, CandidateRoutedFrontierReason,
    CandidateRoutedWork, RoutedCandidateLimits,
};
use serde_json::{Value, json};

/// Trusted-local immutable owner selection and concrete entries. Existing
/// candidate bundle scope/order authority is unchanged. No rule generation.
#[derive(Clone, Debug)]
pub struct RoutedCampaignRequest {
    pub selection_json: String,
    pub targets_csv: String,
    /// Relative owner paths are resolved here, not relative to the executable.
    pub owner_base: PathBuf,
    pub workers: usize,
    pub trace_limits: RoutedCandidateLimits,
    pub reduction_limits: ReductionLimits,
}
impl RoutedCampaignRequest {
    pub fn new(selection_json: String, targets_csv: String) -> Self {
        Self {
            selection_json,
            targets_csv,
            owner_base: PathBuf::from("."),
            workers: 1,
            trace_limits: Default::default(),
            reduction_limits: Default::default(),
        }
    }
}

/// Durable diagnostic summary; not a serialized work queue or resume token.
#[derive(Clone, Debug)]
pub struct RoutedCampaignResult {
    /// Every scheduled dependency completed and the finite frontier is empty.
    /// This never asserts arbitrary-positive-power rank-bounded closure.
    pub completed_finite_trace: bool,
    pub document: Value,
}

/// Load once, verify maps once, and trace all entries through one shared queue.
/// The observer is called synchronously outside native worker locks. Native
/// calls are cooperatively cancelled at operation boundaries, not interrupted.
/// Cancellation/budget failure returns an explicit incomplete result when core
/// tracing has started; preparation/admission errors return `AppError`.
pub fn routed_campaign_with_progress(
    request: RoutedCampaignRequest,
    cancellation: &AtomicBool,
    observer: impl Fn(Value),
) -> Result<RoutedCampaignResult, AppError> {
    if !(1..=50).contains(&request.workers) {
        return Err(AppError::input("workers must be in 1..=50"));
    }
    rustred::campaign::ParallelExecution::preflight_requested_core_budget(request.workers)
        .map_err(|e| AppError::input(e.to_string()))?;
    let (selection, n, limits) = input::Selection::parse(&request.selection_json)?;
    let targets = input::targets(
        &request.targets_csv,
        n,
        request.trace_limits.max_input_targets,
    )?;
    observer(
        json!({"event":"admitted", "arity":n, "targets":targets.len(), "workers":request.workers,
        "load_limits":format!("{limits:?}"), "trace_limits":format!("{:?}", request.trace_limits),
        "reduction_limits":format!("{:?}", request.reduction_limits),
        "shared_dependency_queue":true, "family_closure_claim":false, "ibp_generation":false}),
    );
    macro_rules! dispatch { ($($n:literal),*) => { match n {
        $($n => run::<$n>(&request, &selection, limits, targets, cancellation, &observer),)*
        _ => unreachable!("admitted arity"),
    }} }
    dispatch!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)
}

fn run<const N: usize>(
    request: &RoutedCampaignRequest,
    selection: &input::Selection,
    limits: crate::CandidateOwnerLoadLimits,
    targets: Vec<rustred::family::IntegralKey>,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<RoutedCampaignResult, AppError> {
    let started = Instant::now();
    let Some(reducer) = prepare::prepare::<N>(request, selection, limits, cancellation, observer)?
    else {
        let document = json!({"schema":"rustred.routed-campaign.json.v1", "event":"finished",
            "status":"cancelled_during_preparation", "completed_finite_trace":false,
            "family_closure_claim":false, "ibp_generation":false, "work_checkpoint":false,
            "elapsed_seconds":started.elapsed().as_secs_f64()});
        observer(document.clone());
        return Ok(RoutedCampaignResult {
            completed_finite_trace: false,
            document,
        });
    };
    let result = reducer.trace_targets_parallel_with_observer(
        targets,
        request.workers,
        cancellation,
        |snapshot| observer(snapshot_json(snapshot)),
    );
    let (report, error) = match &result {
        Ok(report) => (report, None),
        Err(error) => (
            error.partial_report(),
            Some(format!("{:?}", error.reason())),
        ),
    };
    let trace = report.trace();
    let mut groups = BTreeMap::<(String, &'static str), (usize, Vec<i64>)>::new();
    for item in trace.frontier() {
        let support = item
            .target
            .powers()
            .iter()
            .map(|&n| if n > 0 { '1' } else { '0' })
            .collect();
        let reason = match item.reason {
            CandidateRoutedFrontierReason::MissingOwner => "missing_owner",
            CandidateRoutedFrontierReason::MissingRule { .. } => "missing_rule",
        };
        let entry = groups
            .entry((support, reason))
            .or_insert_with(|| (0, item.target.powers().to_vec()));
        entry.0 += 1;
    }
    let complete = result.is_ok() && trace.frontier().is_empty();
    let document = json!({"schema":"rustred.routed-campaign.json.v1", "event":"finished",
        "status": if error.is_some() { "incomplete" } else if complete { "finite_trace_complete" } else { "frontier" },
        "completed_finite_trace":complete, "traversal_finished":result.is_ok(), "error":error,
        "family_closure_claim":false, "ibp_generation":false, "coefficient_backsubstitution":false,
        "effective_limits":{"workers":request.workers,"load":format!("{limits:?}"),
            "trace":format!("{:?}",request.trace_limits),"reduction":format!("{:?}",request.reduction_limits)},
        "work_checkpoint":false, "frontier_details_complete":false,
        "family_fingerprint":trace.family_fingerprint(), "snapshot":snapshot_json(report.snapshot()),
        "elapsed_seconds":started.elapsed().as_secs_f64(),
        "frontier_groups":groups.into_iter().map(|((mask, reason),(count,example))|
            json!({"mask":mask,"reason":reason,"count":count,"example":example})).collect::<Vec<_>>()});
    observer(document.clone());
    Ok(RoutedCampaignResult {
        completed_finite_trace: complete,
        document,
    })
}

fn snapshot_json<const N: usize>(s: &CandidateRoutedCampaignSnapshot<N>) -> Value {
    let failure = s.first_failure.as_ref().map(|failure| {
        json!({
        "kind":match failure { CandidateRoutedCampaignFailure::Trace(_) => "trace",
            CandidateRoutedCampaignFailure::Cancelled => "cancelled",
            CandidateRoutedCampaignFailure::WorkerPanicked => "worker_panicked" },
        "detail":format!("{failure:?}")})
    });
    snapshot_json_with_failure(s, failure)
}

// Shared scalar projection; feedback supplies an already bounded error value.
// The default trace-only formatter retains its existing complete Debug field.
fn snapshot_json_with_failure<const N: usize>(
    s: &CandidateRoutedCampaignSnapshot<N>,
    failure: Option<Value>,
) -> Value {
    json!({"event":"shared_progress", "elapsed_seconds":s.elapsed.as_secs_f64(), "workers":s.workers,
        "input_targets":s.input_targets, "requested_targets":s.requested_targets,
        "scheduled_nodes":s.scheduled_nodes, "queued_nodes":s.queued_nodes, "active_nodes":s.active_nodes,
        "completed_nodes":s.completed_nodes, "failed_nodes":s.failed_nodes, "deduplication_hits":s.deduplication_hits,
        "reachable_integrals":s.reachable_integrals, "rule_attempts":s.rule_attempts, "rule_applications":s.rule_applications,
        "transport_calls":s.transport_calls, "transport_operations":s.transport_operations,
        "transport_endpoints":s.transport_endpoints, "coalescing_additions":s.coalescing_additions,
        "reserved_coalescing_additions":s.reserved_coalescing_additions, "declared_terminals":s.declared_terminals,
        "visited_zeros":s.visited_zeros, "missing_owners":s.missing_owners, "missing_rules":s.missing_rules,
        "max_numerator_rank":s.max_numerator_rank.to_string(), "max_dot_excess":s.max_dot_excess.to_string(),
        "first_failure":failure,
        "first_failure_work":s.first_failure_work.as_ref().map(work_json),
        "finished":s.finished, "completed_nodes_are_local_expansions":true, "target_completion_known":s.finished,
        "active":s.active.iter().map(work_json).collect::<Vec<_>>()})
}

fn work_json<const N: usize>(work: &CandidateRoutedWork<N>) -> Value {
    match work {
        CandidateRoutedWork::Route(target) => json!({"phase":"route","target":target.powers()}),
        CandidateRoutedWork::Apply {
            owner_sector,
            target,
        } => json!({"phase":"apply",
            "target":target.powers(), "owner_mask":owner_sector.iter()
                .map(|&b| if b {'1'} else {'0'}).collect::<String>()}),
    }
}
