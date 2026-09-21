//! Bounded summary of saved-rule successor domains, without concrete targets.
use std::collections::{BTreeMap, BTreeSet};
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use rustred::reduction::ReductionLimits;
use rustred::solver::{OwnerSuccessorLimits, OwnerSuccessorTransition};
use serde_json::{Value, json};

use super::{RoutedCampaignRequest, input, prepare};
use crate::AppError;

#[derive(Clone, Debug)]
pub struct OwnerDomainScanRequest {
    pub selection_json: String,
    pub owner_base: PathBuf,
    /// Explicit requested scope; None keeps numerator powers unbounded too.
    pub max_numerator_rank: Option<u32>,
    pub reduction_limits: ReductionLimits,
    /// Per-owner work/scratch limits, not a closure boundary.
    pub scan_limits: OwnerSuccessorLimits,
    pub max_total_regions: usize,
    pub max_summary_groups: usize,
}
impl OwnerDomainScanRequest {
    pub fn new(selection_json: String, max_numerator_rank: Option<u32>) -> Self {
        Self {
            selection_json,
            owner_base: PathBuf::from("."),
            max_numerator_rank,
            reduction_limits: Default::default(),
            scan_limits: Default::default(),
            max_total_regions: 20_000_000,
            max_summary_groups: 16_384,
        }
    }
}

#[derive(Clone, Debug)]
pub struct OwnerDomainScanResult {
    /// Every installed rule was scanned; this NEVER asserts recursive closure.
    pub scan_complete: bool,
    pub document: Value,
}

impl OwnerDomainScanResult {
    /// Progress is not the result payload. Keep both the service observer and
    /// final CLI heartbeat independent of the retained successor-group count.
    /// The full saved document remains authoritative, including full errors.
    pub(crate) fn completion_progress(document: &Value) -> Value {
        let owners = document["owners"].as_array().map_or(&[][..], Vec::as_slice);
        let failed = owners.iter().find(|owner| owner["scan_complete"] == false);
        let mut progress = json!({
            "event":"finished", "operation":"owner_domain_scan",
            "completed_owners":owners.iter().filter(|owner| owner["scan_complete"] == true).count(),
            "total_owners":document["installed_owners"].as_u64().unwrap_or(0),
            "full_result_in_output_document":true,
        });
        for field in [
            "schema",
            "status",
            "scan_complete",
            "family_closure_claim",
            "priority_overapproximation",
            "positive_powers_unbounded",
            "guard_satisfiability_decided",
            "ibp_generation",
            "requested_max_numerator_rank",
            "saved_entry_rank",
            "prepared_seconds",
            "scan_seconds",
            "elapsed_seconds",
            "retained_regions",
            "summary_groups",
            "installed_owners",
            "error_kind",
        ] {
            if let Some(value) = document.get(field) {
                progress[field] = value.clone();
            }
        }
        if let Some(owner) = failed {
            progress["incomplete_owner"] = owner["owner"].clone();
            progress["summary_limit"] = owner["summary_limit"].clone();
            for field in ["rules", "terms", "regions", "split_operations"] {
                progress[field] = owner[field].clone();
            }
        }
        if let Some(error) = document["error"]
            .as_str()
            .or_else(|| failed.and_then(|owner| owner["error"].as_str()))
        {
            // Bound even preparation errors, which may include native context.
            let mut chars = error.chars();
            let detail: String = chars.by_ref().take(512).collect();
            progress["error"] = json!(detail);
            progress["error_truncated"] = json!(chars.next().is_some());
        }
        progress
    }
}

/// Reuse saved owners and verified routes. Stream conservative one-hop domains
/// with unbounded positive powers; no generation, certification or point sweep.
pub fn owner_domain_scan_with_progress(
    request: OwnerDomainScanRequest,
    cancellation: &AtomicBool,
    observer: impl Fn(Value),
) -> Result<OwnerDomainScanResult, AppError> {
    if request.max_total_regions == 0 || !(1..=1_000_000).contains(&request.max_summary_groups) {
        return Err(AppError::input(
            "positive region limit and 1..=1000000 summary groups required",
        ));
    }
    rustred::campaign::ParallelExecution::preflight_requested_core_budget(1)
        .map_err(|e| AppError::input(e.to_string()))?;
    let (selection, n, limits) = input::Selection::parse(&request.selection_json)?;
    observer(
        json!({"event":"admitted", "operation":"owner_domain_scan", "arity":n,
        "max_numerator_rank":request.max_numerator_rank, "positive_powers_unbounded":true,
        "family_closure_claim":false, "ibp_generation":false, "priority_overapproximation":true,
        "scan_limits_per_owner":format!("{:?}",request.scan_limits),
        "max_total_regions":request.max_total_regions, "max_summary_groups":request.max_summary_groups}),
    );
    macro_rules! dispatch { ($($n:literal),*) => { match n {
        $($n => run::<$n>(&request, &selection, limits, cancellation, &observer),)*
        _ => unreachable!("admitted arity"),
    }} }
    dispatch!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)
}

fn mask(bits: &[bool]) -> String {
    bits.iter().map(|&b| if b { '1' } else { '0' }).collect()
}

fn run<const N: usize>(
    request: &OwnerDomainScanRequest,
    selection: &input::Selection,
    limits: crate::CandidateOwnerLoadLimits,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<OwnerDomainScanResult, AppError> {
    let start = Instant::now();
    // Reuse normal native admission; prepare does not read concrete targets.
    let mut load = RoutedCampaignRequest::new(String::new(), String::new());
    load.owner_base = request.owner_base.clone();
    load.reduction_limits = request.reduction_limits;
    let Some(reducer) = prepare::prepare::<N>(&load, selection, limits, cancellation, observer)?
    else {
        return Ok(finish(
            json!({"status":"cancelled_during_preparation", "owners":[]}),
            false,
            start,
            observer,
        ));
    };
    let prepared_seconds = start.elapsed().as_secs_f64();
    let programs = reducer.programs();
    // Every route here has already passed prepare's native map admission.
    let routed: BTreeSet<_> = selection
        .initial_frontier_routes
        .iter()
        .filter(|route| route.requires_transport)
        .map(|route| route.source_mask.as_str())
        .collect();
    let mut owners = Vec::new();
    let mut total_regions = 0usize;
    let mut total_groups = 0usize;
    let mut scan_complete = true;
    for sector in programs.owner_sectors() {
        observer(json!({"event":"owner_scan_started", "owner":mask(sector),
            "completed_owners":owners.len(), "total_owners":programs.owner_count(),
            "retained_regions":total_regions}));
        // Counts only: predicates remain in native programs, never stringify CAS.
        let mut groups = BTreeMap::new();
        let mut summary_limit = None;
        let result = programs.visit_owner_rule_successors(
            *sector,
            request.max_numerator_rank,
            request.scan_limits,
            cancellation,
            |region| {
                if total_regions == request.max_total_regions {
                    summary_limit = Some("total successor regions");
                    return ControlFlow::Break(());
                }
                let transition = match region.transition() {
                    OwnerSuccessorTransition::SameSupport => "same_support",
                    OwnerSuccessorTransition::StrictPinch => "strict_pinch",
                    OwnerSuccessorTransition::UnsupportedSupportChange => {
                        "unsupported_support_change"
                    }
                };
                let key = (
                    *region.target_sector(),
                    transition,
                    region.target_rank_upper_bound(),
                    region.same_support_rank_delta(),
                    region.has_installed_target_owner(),
                    region.is_exact_zero_sector(),
                    region
                        .source_sector()
                        .iter()
                        .zip(region.source_local_upper())
                        .any(|(&active, upper)| active && upper.is_none()),
                    !region.equalities().is_empty(),
                );
                if !groups.contains_key(&key) && total_groups == request.max_summary_groups {
                    summary_limit = Some("summary groups");
                    return ControlFlow::Break(());
                }
                let row = groups.entry(key).or_insert_with(|| {
                    total_groups += 1;
                    (
                        0usize,
                        json!({"batch":region.batch_ordinal(), "rule":region.rule_ordinal(),
                        "term":region.term_ordinal(), "shift":region.shift().as_slice(),
                        "source_local_lower":region.source_local_lower(),
                        "source_local_upper":region.source_local_upper(),
                        "excluded_zero_conjunctions":region.excluded_zero_conjunctions().len()}),
                    )
                });
                row.0 += 1;
                total_regions += 1;
                if total_regions.is_multiple_of(16_384) {
                    observer(json!({"event":"owner_scan_progress", "owner":mask(sector),
                        "completed_owners":owners.len(), "total_owners":programs.owner_count(),
                        "retained_regions":total_regions, "summary_groups":total_groups}));
                }
                ControlFlow::Continue(())
            },
        );
        let (stats, error) = match result {
            Ok(stats) => (stats, None),
            Err(error) => {
                scan_complete = false;
                (error.stats, Some(format!("{:?}", error.failure)))
            }
        };
        let groups: Vec<_> = groups.into_iter().map(|((target, transition, bound, delta, installed, zero, unbounded, equalities), (count, example))| {
            let target = mask(&target);
            json!({"target_owner_installed":installed, "target_has_verified_route":routed.contains(target.as_str()),
                "exact_zero_sector":zero, "target_sector":target, "transition":transition,
                "one_step_rank_upper_bound":bound.map(|n|n.to_string()),
                "same_support_rank_delta":delta.map(|n|n.to_string()), "potential_regions":count,
                "source_positive_prefilter_unbounded":unbounded,
                "has_additional_equalities":equalities,
                "example":example})
        }).collect();
        let row = json!({"owner":mask(sector), "scan_complete":error.is_none(), "error":error,
            "summary_limit":summary_limit, "rules":stats.rules, "terms":stats.terms,
            "regions":stats.regions, "split_operations":stats.split_operations,
            "exact_zero_terms":stats.exact_zero_terms, "rank_empty_prefilters":stats.rank_empty_prefilters,
            "successor_groups":groups});
        observer(json!({"event":"owner_scan_finished", "owner":mask(sector),
            "scan_complete":scan_complete, "rules":stats.rules, "terms":stats.terms,
            "regions":stats.regions, "completed_owners":owners.len()+usize::from(scan_complete),
            "total_owners":programs.owner_count()}));
        owners.push(row);
        if !scan_complete {
            break;
        }
    }
    Ok(finish(
        json!({"status":if scan_complete { "rule_domain_scan_complete" } else { "incomplete" },
        "requested_max_numerator_rank":request.max_numerator_rank,
        "saved_entry_rank":programs.context().scope().max_numerator_rank,
        "prepared_seconds":prepared_seconds, "scan_seconds":start.elapsed().as_secs_f64()-prepared_seconds,
        "retained_regions":total_regions, "summary_groups":total_groups,
        "region_counter_semantics":"retained_regions counts summarized callbacks; owner regions also counts a callback rejected at a summary limit",
        "installed_owners":programs.owner_count(), "owners":owners}),
        scan_complete,
        start,
        observer,
    ))
}

fn finish(
    mut document: Value,
    complete: bool,
    start: Instant,
    observer: &impl Fn(Value),
) -> OwnerDomainScanResult {
    document["schema"] = json!("rustred.owner-domain-scan.json.v1");
    document["event"] = json!("finished");
    document["scan_complete"] = json!(complete);
    document["family_closure_claim"] = json!(false);
    document["priority_overapproximation"] = json!(true);
    document["positive_powers_unbounded"] = json!(true);
    document["guard_satisfiability_decided"] = json!(false);
    document["ibp_generation"] = json!(false);
    document["elapsed_seconds"] = json!(start.elapsed().as_secs_f64());
    observer(OwnerDomainScanResult::completion_progress(&document));
    OwnerDomainScanResult {
        scan_complete: complete,
        document,
    }
}
