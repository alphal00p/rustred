//! Concrete descendant diagnostics through the existing native routed engine.
//! No symbolic coverage claim, coefficient dumping or master inference occurs.

use std::fs::File;
use std::io::{Read, Write};
use std::ops::ControlFlow;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use rustred::family::IntegralKey;
use rustred::solver::{
    CandidateEntryAdmission, CandidateOwnerPrograms, CandidateRoutedCampaignFailure,
    CandidateRoutedError, CandidateRoutedFrontierReason, FiniteRootAdmission, OwnerAppliedEvent,
    OwnerAppliedNonzero, RootRegionInput, RoutedCandidateReducer,
};
use serde_json::{Value, json};

use super::{Result, bounded_message};

const MAX_TARGETS: usize = 256;
const MAX_TARGET_BYTES: usize = 64 * 1024;
const MAX_DETAILS: usize = 256;

pub(super) fn read_targets<const N: usize>(
    path: &Path,
    owner: &[bool; N],
    fixed: &[Option<i16>; N],
) -> Result<Vec<IntegralKey>> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|_| "cannot open follow target CSV")?
        .take(MAX_TARGET_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "cannot read follow target CSV")?;
    if bytes.len() > MAX_TARGET_BYTES {
        return Err("follow target CSV exceeds byte allowance".into());
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| "follow target CSV is not UTF-8")?;
    parse_targets(text, owner, fixed)
}

fn parse_targets<const N: usize>(
    text: &str,
    owner: &[bool; N],
    fixed: &[Option<i16>; N],
) -> Result<Vec<IntegralKey>> {
    let mut targets = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        if targets.len() == MAX_TARGETS {
            return Err("too many follow targets".into());
        }
        let powers = line
            .split(',')
            .map(|value| value.trim().parse::<i64>())
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| "invalid signed follow target")?;
        if powers.len() != N
            || (0..N).any(|axis| {
                (powers[axis] > 0) != owner[axis]
                    || fixed[axis].is_some_and(|value| powers[axis] != i64::from(value))
            })
        {
            return Err("follow target is outside the nominated coordinate case".into());
        }
        targets.push(IntegralKey::try_new(powers).map_err(|_| "invalid follow target key")?);
    }
    if targets.is_empty() {
        return Err("no follow targets".into());
    }
    Ok(targets)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follow_targets_keep_actual_rank_and_nomination_without_entry_clipping() {
        let owner = [false, true, false];
        let fixed = [None, Some(2), Some(-1)];
        let targets = parse_targets("-10,2,-1\n-11,2,-1\n-12,2,-1\n", &owner, &fixed).unwrap();
        assert_eq!(targets.len(), 3);
        assert_eq!(targets[2].powers(), [-12, 2, -1]);
        assert_eq!(local_coordinates(&targets[2]), [12, 1, 1]);
        for text in ["", "-10,1,-1", "-10,2,0", "1,2,-1", "-10,2", "x,2,-1"] {
            assert!(parse_targets(text, &owner, &fixed).is_err(), "{text}");
        }
    }
}

fn local_coordinates(key: &IntegralKey) -> Vec<u64> {
    key.powers()
        .iter()
        .map(|&power| {
            if power > 0 {
                power as u64 - 1
            } else {
                power.unsigned_abs()
            }
        })
        .collect()
}

fn failure(error: &CandidateRoutedCampaignFailure) -> Value {
    match error {
        CandidateRoutedCampaignFailure::Cancelled => json!({"kind":"cancelled"}),
        CandidateRoutedCampaignFailure::WorkerPanicked => json!({"kind":"worker-panicked"}),
        CandidateRoutedCampaignFailure::Trace(error) => match error {
            CandidateRoutedError::ResourceLimit {
                resource,
                requested,
                limit,
            } => {
                json!({"kind":"resource-limit","resource":resource,"requested":requested,"limit":limit})
            }
            CandidateRoutedError::InvalidInput(message) => {
                json!({"kind":"invalid-input","message":bounded_message(message)})
            }
            CandidateRoutedError::Cycle { target } => {
                json!({"kind":"operational-cycle","target":target.powers()})
            }
            CandidateRoutedError::UnsupportedSupportTransition { target, child } => {
                json!({"kind":"unsupported-support-transition","target":target.powers(),"child":child.powers()})
            }
            CandidateRoutedError::Candidate(_) => json!({"kind":"native-rule-evaluation-error"}),
            CandidateRoutedError::EntryAdmission(_) => {
                json!({"kind":"finite-root-admission-error"})
            }
            CandidateRoutedError::Transport(_) => json!({"kind":"native-momentum-transport-error"}),
        },
    }
}

pub(super) fn inspect_and_follow<const N: usize>(
    prepared: &RoutedCandidateReducer<N>,
    installed: Arc<CandidateOwnerPrograms<N>>,
    targets: Vec<IntegralKey>,
    cancellation: &AtomicBool,
) -> Result<Value> {
    let started = Instant::now();
    let reducer = prepared
        .with_programs(installed.clone())
        .map_err(|_| "cannot attach repaired programs to verified routing snapshot")?;
    // Admission names EXACT concrete roots. It replaces only the saved entry
    // rank gate; no source check or descendant rank is clipped or bypassed.
    let regions = targets.iter().map(|target| {
        let lower = local_coordinates(target);
        RootRegionInput {
            support: std::array::from_fn(|axis| target.powers()[axis] > 0),
            upper: lower.iter().copied().map(Some).collect(),
            lower,
            rank: None,
            powers: Default::default(),
        }
    });
    let admission = FiniteRootAdmission::try_new(regions, MAX_TARGETS)
        .map_err(|_| "cannot form exact finite follow-root admission")?;
    let mut first_hops = Vec::new();
    for target in &targets {
        let owner = std::array::from_fn(|axis| target.powers()[axis] > 0);
        let lower = local_coordinates(target);
        let upper = lower.iter().copied().map(Some).collect::<Vec<_>>();
        let mut successors = Vec::new();
        let mut successor_count = 0usize;
        let mut problems = 0usize;
        let mut conditional = 0usize;
        let outcome = installed.visit_owner_applied_successors(
            owner, &lower, &upper, None, Default::default(), cancellation, |event| {
                match event {
                    OwnerAppliedEvent::Successor(child) => {
                        successor_count += 1;
                        conditional += usize::from(child.coefficient_nonzero == OwnerAppliedNonzero::Conditional);
                        if successors.len() < MAX_DETAILS {
                            successors.push(json!({"owner":child.target_sector.iter().map(|&b|if b {'1'}else{'0'}).collect::<String>(),
                                "lower":child.target_lower,"upper":child.target_upper,
                                "rank":child.target_rank_limit,"conditional":child.coefficient_nonzero==OwnerAppliedNonzero::Conditional}));
                        }
                    }
                    OwnerAppliedEvent::Problem(_) => problems += 1,
                    _ => {}
                }
                ControlFlow::Continue(())
            },
        );
        first_hops.push(json!({"target":target.powers(),"inspection_complete":outcome.is_ok(),
            "successors":successor_count,"conditional_successors":conditional,"problems":problems,
            "successor_details":successors,"successor_details_truncated":successor_count>MAX_DETAILS,
            "failure":outcome.err().map(|_|"native-applied-inspection-error"),
            "recursive_closure_claim":false}));
    }
    let first_hop_seconds = started.elapsed().as_secs_f64();
    let trace_started = Instant::now();
    let mut last_progress = Instant::now();
    let outcome = reducer.trace_targets_parallel_with_entry_admission_and_observer(
        targets.clone(), CandidateEntryAdmission::ExplicitFinite(&admission), 1, cancellation,
        |snapshot| {
            if last_progress.elapsed() >= Duration::from_secs(2) {
                let progress = json!({"event":"repaired-target-follow","elapsed_seconds":trace_started.elapsed().as_secs_f64(),
                    "scheduled":snapshot.scheduled_nodes,"completed":snapshot.completed_nodes,
                    "pending":snapshot.queued_nodes,"active":snapshot.active_nodes,
                    "missing_rules":snapshot.missing_rules,"missing_owners":snapshot.missing_owners});
                let mut stderr = std::io::stderr().lock();
                let _ = serde_json::to_writer(&mut stderr, &progress);
                let _ = writeln!(stderr);
                last_progress = Instant::now();
            }
        },
    );
    let (native, failure) = match &outcome {
        Ok(report) => (report, Value::Null),
        Err(error) => (error.partial_report(), failure(error.reason())),
    };
    let trace = native.trace();
    let snapshot = native.snapshot();
    let complete = outcome.is_ok() && snapshot.finished;
    Ok(
        json!({"status":if complete {"concrete-trace-complete"}else{"concrete-trace-incomplete"},
        "targets":targets.iter().map(|target|target.powers()).collect::<Vec<_>>(),
        "first_hops":first_hops,"first_hop_seconds":first_hop_seconds,
        "trace_seconds":trace_started.elapsed().as_secs_f64(),"failure":failure,
        "reachable_integrals":trace.reachable_integrals(),"operational_nodes":trace.operational_nodes(),
        "rule_applications":trace.rule_applications(),"transport_calls":trace.transport_calls(),
        "max_numerator_rank":trace.max_negative_index_degree(),"max_dot_excess":trace.max_dot_excess(),
        "frontier_count":trace.frontier().len(),"frontier_details_truncated":trace.frontier().len()>MAX_DETAILS,
        "frontier":trace.frontier().iter().take(MAX_DETAILS).map(|frontier|json!({"target":frontier.target.powers(),
            "reason":match &frontier.reason {
                CandidateRoutedFrontierReason::MissingOwner => json!({"kind":"missing-owner"}),
                CandidateRoutedFrontierReason::MissingRule { owner_sector } => json!({"kind":"missing-rule","owner":owner_sector.iter().map(|&b|if b {'1'}else{'0'}).collect::<String>()}),
            }})).collect::<Vec<_>>(),
        "declared_terminals":trace.declared_terminals().len(),"visited_zeros":trace.visited_zeros().len(),
        "scheduled":snapshot.scheduled_nodes,"completed":snapshot.completed_nodes,
        "pending":snapshot.queued_nodes,"active":snapshot.active_nodes,"failed":snapshot.failed_nodes,
        "explicit_start_admission":true,"descendants_rank_clipped":false,
        "all_requested_concrete_targets_reach_existing_terminals":complete && trace.frontier().is_empty(),
        "parametric_coverage_claim":false,"family_closure_claim":false,"production_modified":false}),
    )
}
