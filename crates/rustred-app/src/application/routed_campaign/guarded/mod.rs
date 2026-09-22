//! Bounded display-only inspection of one explicitly selected saved rule.
//! It neither invokes ordered dispatch nor inserts guarded images into a queue.
mod input;
pub(crate) mod policy;
mod render;

use super::{RoutedCampaignRequest, input as owners, prepare};
use crate::AppError;
use rustred::reduction::ReductionLimits;
use rustred::solver::{OwnerGuardedFailure, OwnerGuardedLimits};
use serde_json::{Value, json};
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct OwnerGuardedApplyRequest {
    pub selection_json: String,
    pub queries_json: String,
    pub owner_base: PathBuf,
    pub reduction_limits: ReductionLimits,
    pub limits: OwnerGuardedLimits,
    pub max_queries: usize,
    pub max_report_events: usize,
    /// Cumulative conservative JSON payload allowance, including discarded
    /// partial formatting. This is not a native-memory or process-RSS bound.
    pub max_report_bytes: usize,
    pub max_expression_bytes: usize,
}
impl OwnerGuardedApplyRequest {
    pub fn new(selection_json: String, queries_json: String) -> Self {
        Self {
            selection_json,
            queries_json,
            owner_base: PathBuf::from("."),
            reduction_limits: Default::default(),
            limits: Default::default(),
            max_queries: 256,
            max_report_events: 100_000,
            max_report_bytes: 16 * 1024 * 1024,
            max_expression_bytes: 64 * 1024,
        }
    }
}
#[derive(Clone, Debug)]
pub struct OwnerGuardedApplyResult {
    /// Only all requested native inspections AND diagnostic rendering finished.
    /// Residual complements and RHS problems can remain even when true.
    pub diagnostic_complete: bool,
    pub document: Value,
}
impl OwnerGuardedApplyResult {
    pub(crate) fn preparation_error_document(error: &AppError) -> Value {
        json!({"schema":"rustred.owner-guarded-apply.json.v1","status":"preparation_error","diagnostic_complete":false,
            "family_closure_claim":false,"first_priority_dispatch_claim":false,"integer_feasibility_claim":false,"work_queue_insertion":false,
            "error_kind":error.kind().as_str(),"error":render::bounded_debug(error)})
    }
    pub(crate) fn completion_progress(document: &Value) -> Value {
        let mut event = json!({"event":"finished","operation":"owner_guarded_apply","full_result_in_output_document":true});
        for key in [
            "schema",
            "status",
            "diagnostic_complete",
            "query_count",
            "processed_queries",
            "completed_queries",
            "retained_events",
            "report_payload_bytes_charged",
            "error_kind",
            "error_query_id",
            "elapsed_seconds",
            "prepared_seconds",
            "inspection_seconds",
            "family_closure_claim",
            "first_priority_dispatch_claim",
        ] {
            if let Some(v) = document.get(key) {
                event[key] = v.clone();
            }
        }
        event
    }
}

pub fn owner_guarded_apply_with_progress(
    request: OwnerGuardedApplyRequest,
    cancellation: &AtomicBool,
    observer: impl Fn(Value),
) -> Result<OwnerGuardedApplyResult, AppError> {
    if !(1..=10_000).contains(&request.max_queries)
        || !(1..=1_000_000).contains(&request.max_report_events)
        || !(131_072..=256 * 1024 * 1024).contains(&request.max_report_bytes)
        || !(1..=16 * 1024 * 1024).contains(&request.max_expression_bytes)
    {
        return Err(AppError::input(
            "guarded report allowances: 1..10000 queries, 1..1000000 events, 131072..268435456 payload bytes, 1..16777216 expression bytes",
        ));
    }
    rustred::campaign::ParallelExecution::preflight_requested_core_budget(1)
        .map_err(|e| AppError::input(e.to_string()))?;
    let (selection, arity, load_limits) = owners::Selection::parse(&request.selection_json)?;
    let queries = input::parse(&request.queries_json, arity, request.max_queries)?;
    observer(
        json!({"event":"admitted","operation":"owner_guarded_apply","arity":arity,"query_count":queries.len(),
        "diagnostic_only":true,"first_priority_dispatch_claim":false,"family_closure_claim":false}),
    );
    macro_rules! dispatch {($($n:literal),*)=>{match arity {
        $($n=>run::<$n>(&request,&selection,load_limits,&queries,cancellation,&observer),)*
        _=>unreachable!("admitted arity"),
    }}}
    dispatch!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)
}

fn run<const N: usize>(
    request: &OwnerGuardedApplyRequest,
    selection: &owners::Selection,
    load_limits: crate::CandidateOwnerLoadLimits,
    queries: &[input::GuardedQuery],
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<OwnerGuardedApplyResult, AppError> {
    let started = Instant::now();
    let mut load = RoutedCampaignRequest::new(String::new(), String::new());
    load.owner_base = request.owner_base.clone();
    load.reduction_limits = request.reduction_limits;
    let preparation_observer = |mut event: Value| {
        event["operation"] = json!("owner_guarded_apply");
        observer(event);
    };
    let Some(reducer) = prepare::prepare::<N>(
        &load,
        selection,
        load_limits,
        cancellation,
        &preparation_observer,
    )?
    else {
        return Ok(finish(
            json!({"status":"cancelled_during_preparation","queries":[],"query_count":queries.len(),
            "processed_queries":0,"completed_queries":0,"error_kind":"cancelled"}),
            false,
            started,
            observer,
        ));
    };
    let prepared_seconds = started.elapsed().as_secs_f64();
    let inspection_started = Instant::now();
    // Fixed metadata/error envelope reserved before any retained query. Native
    // expression/debug formatting is independently prospectively bounded.
    let mut budget = render::Budget {
        bytes: 131_072,
        events: 0,
        limit: request.max_report_bytes,
        event_limit: request.max_report_events,
        expression_limit: request.max_expression_bytes,
        cancel: cancellation,
    };
    let mut records = Vec::new();
    let mut completed = 0;
    let mut failure = None;
    for selected in queries {
        let q = &selected.query;
        if let Err(error) = budget.charge(4096) {
            failure = Some((q.id.clone(), "report_limit_or_cancelled", error.to_owned()));
            break;
        }
        observer(
            json!({"event":"guarded_query_started","operation":"owner_guarded_apply","id":q.id,
            "completed_queries":completed,"processed_queries":records.len(),"query_count":queries.len(),"retained_events":budget.events}),
        );
        let owner: [bool; N] = q.owner.as_slice().try_into().expect("validated arity");
        let mut definition = None;
        let mut events = Vec::new();
        let mut render_error = None;
        let mut heartbeat = Instant::now();
        let result=reducer.programs().visit_owner_guarded_rule_successors(owner,selected.batch,selected.rule,
            &q.lower,&q.upper,q.rank,request.limits,cancellation,|event|{
                match render::event(event,&q.id,&mut definition,&mut budget) {
                    Ok(event)=>events.push(event),
                    Err(error)=>{render_error=Some(error);return ControlFlow::Break(());}
                }
                if heartbeat.elapsed()>=Duration::from_millis(250){
                    observer(json!({"event":"guarded_query_progress","operation":"owner_guarded_apply","id":q.id,
                        "completed_queries":completed,"processed_queries":records.len(),"query_count":queries.len(),
                        "retained_events":budget.events,"report_payload_bytes_charged":budget.bytes}));
                    heartbeat=Instant::now();
                }
                ControlFlow::Continue(())
            });
        let (stats, native_error, error_kind) = match result {
            Ok(stats) => (stats, None, None),
            Err(error) => (
                error.stats,
                Some(render::bounded_debug(&error.failure)),
                Some(failure_kind(&error.failure)),
            ),
        };
        let done = native_error.is_none() && render_error.is_none();
        if done {
            completed += 1;
        } else {
            failure = Some((
                q.id.clone(),
                if render_error.is_some() {
                    "report_limit_or_cancelled"
                } else {
                    error_kind.unwrap_or("native_failure")
                },
                render_error
                    .map(str::to_owned)
                    .or_else(|| native_error.clone())
                    .expect("incomplete cause"),
            ));
        }
        records.push(json!({"id":q.id,"owner":q.owner.iter().map(|&v|if v{'1'}else{'0'}).collect::<String>(),
            "batch":selected.batch,"rule":selected.rule,"input_lower":q.lower,"input_upper":q.upper,"requested_max_numerator_rank":q.rank,
            "inspection_finished":done,"native_error":native_error,"native_error_kind":error_kind,"report_error":render_error,
            "stats":render::stats(stats),"guarded_domain":definition,"events":events}));
        observer(
            json!({"event":"guarded_query_finished","operation":"owner_guarded_apply","id":q.id,
            "inspection_finished":done,"completed_queries":completed,"processed_queries":records.len(),"query_count":queries.len(),"retained_events":budget.events}),
        );
        if !done {
            break;
        }
    }
    let complete = completed == queries.len() && failure.is_none();
    let mut document = json!({"status":if complete{"diagnostic_complete"}else{"diagnostic_incomplete"},
        "query_count":queries.len(),"processed_queries":records.len(),"completed_queries":completed,"queries":records,
        "retained_events":budget.events,"report_payload_bytes_charged":budget.bytes,"max_report_bytes":request.max_report_bytes,
        "max_report_events":request.max_report_events,"max_expression_bytes":request.max_expression_bytes,
        "prepared_seconds":prepared_seconds,"inspection_seconds":inspection_started.elapsed().as_secs_f64(),
        "work_limits":policy::json(request.limits),"reduction_limits":render::bounded_debug(&request.reduction_limits)});
    if let Some((id, kind, error)) = failure {
        document["error_query_id"] = json!(id);
        document["error_kind"] = json!(kind);
        document["error"] = json!(error);
    }
    Ok(finish(document, complete, started, observer))
}
fn failure_kind(failure: &OwnerGuardedFailure) -> &'static str {
    match failure {
        OwnerGuardedFailure::UnknownOwner => "unknown_owner",
        OwnerGuardedFailure::UnknownCandidate => "unknown_candidate",
        OwnerGuardedFailure::InvalidInput(_) => "invalid_input",
        OwnerGuardedFailure::UnsupportedPowerBounds(_) => "unsupported_power_bounds",
        OwnerGuardedFailure::Cancelled => "cancelled",
        OwnerGuardedFailure::StoppedByConsumer => "stopped_by_consumer",
        OwnerGuardedFailure::ResourceLimit { .. } => "resource_limit",
        OwnerGuardedFailure::CountOverflow { .. } => "count_overflow",
        OwnerGuardedFailure::Applied(_) => "applied_failure",
    }
}
fn finish(
    mut document: Value,
    complete: bool,
    started: Instant,
    observer: &impl Fn(Value),
) -> OwnerGuardedApplyResult {
    document["schema"] = json!("rustred.owner-guarded-apply.json.v1");
    document["diagnostic_complete"] = json!(complete);
    document["elapsed_seconds"] = json!(started.elapsed().as_secs_f64());
    document["completion_semantics"] = json!(
        "requested diagnostic inspections/rendering finished; complements/problems may remain"
    );
    for field in [
        "family_closure_claim",
        "first_priority_dispatch_claim",
        "integer_feasibility_claim",
        "work_queue_insertion",
        "ibp_generation",
    ] {
        document[field] = json!(false);
    }
    document["expression_text_is_display_only"] = json!(true);
    document["box_coordinates"] =
        json!("owner-active n_i=1+x_i; owner-inactive n_i=-x_i; rank=sum inactive x_i");
    document["residuals_are_not_asserted_disjoint_or_nonempty"] = json!(true);
    observer(OwnerGuardedApplyResult::completion_progress(&document));
    OwnerGuardedApplyResult {
        diagnostic_complete: complete,
        document,
    }
}
