//! Ordered local applicability queries over shared saved programs.
pub(super) mod input;

use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use rustred::reduction::ReductionLimits;
use rustred::solver::{
    OwnerDomainMatchDisposition, OwnerDomainMatchFailure, OwnerDomainMatchLimits,
};
use serde_json::{Value, json};

use super::{RoutedCampaignRequest, input as owners, prepare};
use crate::AppError;

#[derive(Clone, Debug)]
pub struct OwnerDomainMatchRequest {
    pub selection_json: String,
    pub queries_json: String,
    pub owner_base: PathBuf,
    pub reduction_limits: ReductionLimits,
    pub match_limits: OwnerDomainMatchLimits,
    pub max_queries: usize,
    /// Global retained result pieces, distinct from native per-query work.
    pub max_total_pieces: usize,
}
impl OwnerDomainMatchRequest {
    pub fn new(selection_json: String, queries_json: String) -> Self {
        Self {
            selection_json,
            queries_json,
            owner_base: PathBuf::from("."),
            reduction_limits: Default::default(),
            match_limits: Default::default(),
            max_queries: 256,
            max_total_pieces: 100_000,
        }
    }
}

#[derive(Clone, Debug)]
pub struct OwnerDomainMatchResult {
    /// Every query has an exact local dispatch classification (which may be Gap).
    /// Never denotes RHS evaluation, source provenance or recursive closure.
    pub classification_complete: bool,
    pub all_queries_locally_applicable: bool,
    pub document: Value,
}
impl OwnerDomainMatchResult {
    pub(crate) fn completion_progress(document: &Value) -> Value {
        let mut event = json!({"event":"finished", "operation":"owner_domain_match",
            "full_result_in_output_document":true});
        for key in [
            "schema",
            "status",
            "classification_complete",
            "all_queries_locally_applicable",
            "family_closure_claim",
            "ibp_generation",
            "rhs_successors_expanded",
            "query_count",
            "completed_queries",
            "processed_queries",
            "retained_pieces",
            "counts",
            "elapsed_seconds",
            "prepared_seconds",
            "matching_seconds",
            "error_kind",
            "error_query_id",
        ] {
            if let Some(value) = document.get(key) {
                event[key] = value.clone();
            }
        }
        if let Some(error) = document["error"].as_str() {
            event["error"] = json!(error.chars().take(512).collect::<String>());
            event["error_truncated"] = json!(error.chars().count() > 512);
        }
        event
    }
}

/// Match explicit original-coordinate boxes without solving or expanding RHSs.
/// A query is not silently made reachable from another input: callers retain
/// the provenance/guards of any nominated successor separately.
pub fn owner_domain_match_with_progress(
    request: OwnerDomainMatchRequest,
    cancellation: &AtomicBool,
    observer: impl Fn(Value),
) -> Result<OwnerDomainMatchResult, AppError> {
    if !(1..=10_000).contains(&request.max_queries)
        || !(1..=1_000_000).contains(&request.max_total_pieces)
    {
        return Err(AppError::input(
            "positive match allowances must fit 10000 queries /1000000 pieces",
        ));
    }
    rustred::campaign::ParallelExecution::preflight_requested_core_budget(1)
        .map_err(|error| AppError::input(error.to_string()))?;
    let (selection, arity, limits) = owners::Selection::parse(&request.selection_json)?;
    let queries = input::parse(&request.queries_json, arity, request.max_queries)?;
    observer(
        json!({"event":"admitted", "operation":"owner_domain_match", "arity":arity,
        "query_count":queries.len(), "max_total_pieces":request.max_total_pieces,
        "match_limits":format!("{:?}",request.match_limits), "family_closure_claim":false,
        "ibp_generation":false, "rhs_successors_expanded":false}),
    );
    macro_rules! dispatch { ($($n:literal),*) => { match arity {
        $($n => run::<$n>(&request, &selection, limits, &queries, cancellation, &observer),)*
        _ => unreachable!("admitted arity"),
    }} }
    dispatch!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)
}

#[derive(Default)]
struct Counts {
    rule: usize,
    terminal: usize,
    zero: usize,
    gap: usize,
    unresolved: usize,
    invalid: usize,
}
impl Counts {
    fn json(&self) -> Value {
        json!({"selected_rule":self.rule, "terminal":self.terminal, "exact_zero_sector":self.zero,
            "exact_gap":self.gap, "unresolved":self.unresolved, "invalid_source_condition":self.invalid})
    }
    fn classify(&mut self, disposition: &OwnerDomainMatchDisposition) -> Value {
        match disposition {
            OwnerDomainMatchDisposition::SelectedRule { batch, rule } => {
                self.rule += 1;
                json!({"kind":"selected_rule", "batch":batch, "rule":rule})
            }
            OwnerDomainMatchDisposition::Terminal { batch } => {
                self.terminal += 1;
                json!({"kind":"terminal", "batch":batch})
            }
            OwnerDomainMatchDisposition::ExactGap => {
                self.gap += 1;
                json!({"kind":"exact_gap"})
            }
            OwnerDomainMatchDisposition::ExactZeroSector => {
                self.zero += 1;
                json!({"kind":"exact_zero_sector"})
            }
            OwnerDomainMatchDisposition::Unresolved { predicate } => {
                self.unresolved += 1;
                json!({"kind":"unresolved", "predicate":format!("{predicate:?}")})
            }
            OwnerDomainMatchDisposition::InvalidSourceCondition { ordinal } => {
                self.invalid += 1;
                json!({"kind":"invalid_source_condition", "ordinal":ordinal})
            }
        }
    }
}

fn run<const N: usize>(
    request: &OwnerDomainMatchRequest,
    selection: &owners::Selection,
    limits: crate::CandidateOwnerLoadLimits,
    queries: &[input::Query],
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<OwnerDomainMatchResult, AppError> {
    let started = Instant::now();
    let mut load = RoutedCampaignRequest::new(String::new(), String::new());
    load.owner_base = request.owner_base.clone();
    load.reduction_limits = request.reduction_limits;
    let Some(reducer) = prepare::prepare::<N>(&load, selection, limits, cancellation, observer)?
    else {
        return Ok(finish(
            json!({"status":"cancelled_during_preparation", "queries":[],
            "query_count":queries.len(), "completed_queries":0, "processed_queries":0}),
            false,
            false,
            started,
            observer,
        ));
    };
    let prepared_seconds = started.elapsed().as_secs_f64();
    let matching_started = Instant::now();
    let mut records = Vec::new();
    let mut counts = Counts::default();
    let mut retained = 0usize;
    let mut complete = true;
    let mut completed_queries = 0usize;
    let mut first_error = None;
    for query in queries {
        if cancellation.load(Ordering::Acquire) {
            complete = false;
            first_error = Some((
                query.id.clone(),
                "cancelled",
                "cancelled before query".to_owned(),
            ));
            break;
        }
        observer(
            json!({"event":"domain_query_started", "operation":"owner_domain_match",
            "id":query.id, "completed_queries":completed_queries, "processed_queries":records.len(), "query_count":queries.len(),
            "retained_pieces":retained, "counts":counts.json()}),
        );
        let owner: [bool; N] = query.owner.as_slice().try_into().expect("validated arity");
        let mut pieces = Vec::new();
        let mut summary_limit = false;
        let mut unresolved = false;
        let result = reducer.programs().visit_owner_domain_matches(
            owner, &query.lower, &query.upper, query.rank, request.match_limits, cancellation,
            |piece| {
                if retained == request.max_total_pieces { summary_limit = true; return ControlFlow::Break(()); }
                unresolved |= matches!(piece.disposition(), OwnerDomainMatchDisposition::Unresolved { .. });
                let disposition = counts.classify(&piece.disposition());
                pieces.push(json!({"lower":piece.lower(), "upper":piece.upper(),
                    "max_numerator_rank":piece.max_numerator_rank(), "disposition":disposition}));
                retained += 1;
                if retained.is_multiple_of(128) {
                    observer(json!({"event":"domain_query_progress", "operation":"owner_domain_match",
                        "id":query.id, "completed_queries":completed_queries, "processed_queries":records.len(), "query_count":queries.len(),
                        "retained_pieces":retained, "counts":counts.json()}));
                }
                ControlFlow::Continue(())
            },
        );
        let (stats, error, error_kind) = match result {
            Ok(stats) => (stats, None, None),
            Err(error) => {
                let kind = failure_kind(&error.failure);
                let detail = format!("{:?}", error.failure);
                first_error = Some((query.id.clone(), kind, detail.clone()));
                (error.stats, Some(detail), Some(kind))
            }
        };
        let query_complete = error.is_none() && !unresolved;
        complete &= query_complete;
        completed_queries += usize::from(query_complete);
        records.push(json!({"id":query.id, "owner":query.owner.iter().map(|&b|if b{'1'}else{'0'}).collect::<String>(),
            "input_lower":query.lower, "input_upper":query.upper, "requested_max_numerator_rank":query.rank,
            "classification_complete":query_complete, "error":error, "error_kind":error_kind, "summary_limit":summary_limit,
            "stats":{"rules":stats.rules, "terminal_checks":stats.terminal_checks, "predicates":stats.predicates,
                "pieces":stats.pieces, "cells":stats.cells, "split_operations":stats.split_operations,
                "coordinate_cells":stats.coordinate_cells, "rank_empty_cells":stats.rank_empty_cells,
                "refinement_cells":stats.refinement_cells, "refinement_steps":stats.refinement_steps}, "pieces":pieces}));
        observer(
            json!({"event":"domain_query_finished", "operation":"owner_domain_match", "id":query.id,
            "completed_queries":completed_queries, "processed_queries":records.len(), "query_count":queries.len(), "retained_pieces":retained,
            "query_classification_complete":query_complete, "counts":counts.json()}),
        );
        if error.is_some() {
            break;
        }
    }
    complete &= records.len() == queries.len();
    let applicable = complete && counts.gap == 0 && counts.invalid == 0;
    Ok(finish(
        json!({"status":if !complete {"incomplete"} else if applicable {"locally_applicable"} else {"classified_with_gaps_or_invalid_conditions"},
        "query_count":queries.len(), "completed_queries":completed_queries, "processed_queries":records.len(), "retained_pieces":retained,
        "region_counter_semantics":"retained pieces exclude a rejected callback; native stats include it",
        "error":first_error.as_ref().map(|(_,_,e)|e),
        "error_kind":first_error.as_ref().map(|(_,k,_)|k),
        "error_query_id":first_error.as_ref().map(|(id,_,_)|id),
        "counts":counts.json(), "prepared_seconds":prepared_seconds,
        "matching_seconds":matching_started.elapsed().as_secs_f64(), "queries":records}),
        complete,
        applicable,
        started,
        observer,
    ))
}

fn failure_kind(failure: &OwnerDomainMatchFailure) -> &'static str {
    match failure {
        OwnerDomainMatchFailure::UnknownOwner => "unknown_owner",
        OwnerDomainMatchFailure::InvalidInput(_) => "invalid_input",
        OwnerDomainMatchFailure::Cancelled => "cancelled",
        OwnerDomainMatchFailure::StoppedByConsumer => "consumer_limit",
        OwnerDomainMatchFailure::ResourceLimit { .. } => "resource_limit",
        OwnerDomainMatchFailure::CountOverflow { .. } => "count_overflow",
        OwnerDomainMatchFailure::AllocationFailure { .. } => "allocation_failure",
        OwnerDomainMatchFailure::Algebra(_) => "algebra",
        OwnerDomainMatchFailure::Geometry(_) => "geometry",
    }
}

fn finish(
    mut document: Value,
    complete: bool,
    applicable: bool,
    started: Instant,
    observer: &impl Fn(Value),
) -> OwnerDomainMatchResult {
    document["schema"] = json!("rustred.owner-domain-match.json.v1");
    document["event"] = json!("finished");
    document["classification_complete"] = json!(complete);
    document["all_queries_locally_applicable"] = json!(applicable);
    document["family_closure_claim"] = json!(false);
    document["ibp_generation"] = json!(false);
    document["rhs_successors_expanded"] = json!(false);
    document["elapsed_seconds"] = json!(started.elapsed().as_secs_f64());
    observer(OwnerDomainMatchResult::completion_progress(&document));
    OwnerDomainMatchResult {
        classification_complete: complete,
        all_queries_locally_applicable: applicable,
        document,
    }
}
