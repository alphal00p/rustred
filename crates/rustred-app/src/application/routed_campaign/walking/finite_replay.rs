//! Opt-in, whole-initial-singleton exact replay. No shared memo or new terminal.

#[cfg(test)]
mod tests;

/// Cumulative aggregate policy for the single initial-root exact-trace attempt.
/// At app preparation, node/transport fields configure the otherwise unused
/// concrete-trace aggregate slots, replacing routed-campaign defaults. The core
/// attempt then intersects this policy with its admitted trace/reduction limits.
/// Matching reduction and per-formula expansion/algebra limits are unchanged;
/// zero is an explicit zero-work allowance.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    bincode::Encode,
    bincode::Decode,
)]
#[serde(deny_unknown_fields)]
pub struct OwnerDomainWalkFiniteReplayLimits {
    pub max_nodes: usize,
    pub max_rule_applications: usize,
    pub max_transport_calls: usize,
    pub max_transport_operations: usize,
    pub max_transport_endpoints: usize,
    pub max_coalescing_additions: usize,
}

impl OwnerDomainWalkFiniteReplayLimits {
    pub(crate) fn budget(self) -> rustred::solver::CandidateRoutedWorkBudget {
        rustred::solver::CandidateRoutedWorkBudget {
            max_nodes: self.max_nodes,
            max_rule_applications: self.max_rule_applications,
            max_transport_calls: self.max_transport_calls,
            max_transport_operations: self.max_transport_operations,
            max_transport_endpoints: self.max_transport_endpoints,
            max_coalescing_additions: self.max_coalescing_additions,
        }
    }
}

/// Opt-in semantic marker. Absent requests retain their existing identity.
pub const OWNER_DOMAIN_WALK_FINITE_REPLAY_VERSION: u32 = 1;

/// Symbolic walking did not previously use concrete aggregate trace caps.
/// Bind those slots to this opt-in policy in BOTH online and cold preparation;
/// retain every native expansion/scratch/guard limit and matching reduction cap.
pub(super) fn configure_load(
    request: &super::OwnerDomainWalkRequest,
    load: &mut super::RoutedCampaignRequest,
) {
    if let Some(limits) = request.finite_replay {
        load.trace_limits.max_input_targets = limits.max_nodes;
        load.trace_limits.max_unique_nodes = limits.max_nodes;
        load.trace_limits.max_transport_calls = limits.max_transport_calls;
        load.trace_limits.max_transport_operations = limits.max_transport_operations;
        load.trace_limits.max_transport_endpoints = limits.max_transport_endpoints;
    }
}

use super::{OwnerDomainWalkRequest, inspection, matching, queue::Domain};
use rustred::family::IntegralKey;
use rustred::solver::{
    CandidateEntryAdmission, CandidateRoutedCampaignFailure, CandidateRoutedCampaignSnapshot,
    CandidateRoutedError, FiniteRootAdmission, RootRegionInput, RoutedCandidateReducer,
};
use serde_json::{Value, json};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// Fresh-walk-only reservation and diagnostic account. This stores no keys,
/// coefficients, domain graph or memoized closure. Enabled resumes are refused.
pub(super) struct Account {
    reserved: AtomicBool,
    work: Mutex<Value>,
}
impl Default for Account {
    fn default() -> Self {
        Self {
            reserved: AtomicBool::new(false),
            work: Mutex::new(json!({"status":"not_attempted"})),
        }
    }
}
impl Account {
    pub fn reserve(&self) -> bool {
        !self.reserved.swap(true, Ordering::AcqRel)
    }
    pub fn record(&self, work: Value) {
        *self
            .work
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = work;
    }
    pub fn report(&self) -> Value {
        json!({"attempts":usize::from(self.reserved.load(Ordering::Acquire)),
            "max_attempts":1,"work":self.work.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone(),
            "scope":"fresh whole initial ID0 only; includes declined or cancelled trace work; not proof"})
    }
}

/// The image is the enclosing typed record's exact domain, not another copy
/// in diagnostic JSON. The immutable pool/request binding supplies all payload,
/// order, original-source and terminal authority. Replay supplies the proof.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    bincode::Encode,
    bincode::Decode,
)]
#[serde(deny_unknown_fields)]
pub(super) struct Recipe {
    pub version: u32,
    pub limits: OwnerDomainWalkFiniteReplayLimits,
}
impl Recipe {
    pub fn validate(self) -> Result<(), &'static str> {
        if self.version != OWNER_DOMAIN_WALK_FINITE_REPLAY_VERSION {
            return Err("unsupported finite replay recipe version");
        }
        Ok(())
    }
}

pub(super) enum Outcome {
    Closed {
        recipe: Recipe,
        work: Value,
    },
    Declined {
        work: Value,
    },
    Failed {
        error: String,
        cancelled: bool,
        work: Value,
    },
}
impl Outcome {
    pub fn work(&self) -> &Value {
        match self {
            Self::Closed { work, .. } | Self::Declined { work } | Self::Failed { work, .. } => work,
        }
    }
}

/// Re-derive the exact FIRST original query, not a containing alias or a
/// reconstruction from a physical point that drops caps. A source-validity
/// frontier is not an eligible initial domain. The bounded input was already
/// parsed before native preparation; this independent check is also used cold.
pub(super) fn original_initial<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    request: &OwnerDomainWalkRequest,
) -> Result<Option<Domain<N>>, String> {
    let queries = matching::input::parse(
        &request.matching.queries_json,
        N,
        request.matching.max_queries,
        request.matching.max_query_bytes,
    )
    .map_err(|e| e.to_string())?;
    let Some(query) = queries.first() else {
        return Err("missing original finite replay query".into());
    };
    let installed = reducer
        .programs()
        .owner_sectors()
        .any(|owner| owner.as_slice() == query.owner);
    let phase = if installed || !request.route_domain_overcover {
        super::queue::Phase::Apply
    } else if !reducer.domain_routing_requires_source_conditions() {
        super::queue::Phase::Route
    } else {
        return Ok(None);
    };
    Ok(Some(Domain {
        phase,
        owner: query
            .owner
            .as_slice()
            .try_into()
            .map_err(|_| "finite replay original arity")?,
        lower: query.lower.clone(),
        upper: query.upper.clone(),
        rank: query.rank,
        powers: query.powers,
    }))
}

fn point<const N: usize>(domain: &Domain<N>) -> Result<Option<IntegralKey>, String> {
    if domain.lower.len() != N || domain.upper.len() != N {
        return Err("finite replay domain arity".into());
    }
    if domain
        .lower
        .iter()
        .zip(&domain.upper)
        .any(|(&lo, &hi)| hi != Some(lo))
    {
        return Ok(None);
    }
    let powers = (0..N)
        .map(|axis| {
            let coordinate =
                i64::try_from(domain.lower[axis]).map_err(|_| "finite replay coordinate range")?;
            if domain.owner[axis] {
                coordinate
                    .checked_add(1)
                    .ok_or("finite replay positive power range")
            } else {
                coordinate
                    .checked_neg()
                    .ok_or("finite replay numerator power range")
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    IntegralKey::try_new(powers)
        .map(Some)
        .map_err(|e| e.to_string())
}

pub(super) fn work_json<const N: usize>(
    s: &CandidateRoutedCampaignSnapshot<N>,
    status: &str,
) -> Value {
    json!({"status":status,"workers":s.workers,"seconds":s.elapsed.as_secs_f64(),
        "input_targets":s.input_targets,"requested_targets":s.requested_targets,
        "scheduled_nodes":s.scheduled_nodes,"completed_nodes":s.completed_nodes,
        "queued_nodes":s.queued_nodes,"active_nodes":s.active_nodes,"failed_nodes":s.failed_nodes,
        "reachable_integrals":s.reachable_integrals,"deduplication_hits":s.deduplication_hits,
        "rule_attempts":s.rule_attempts,"rule_applications":s.rule_applications,
        "transport_calls":s.transport_calls,"transport_operations":s.transport_operations,
        "transport_endpoints":s.transport_endpoints,"coalescing_additions":s.coalescing_additions,
        "reserved_coalescing_additions":s.reserved_coalescing_additions,
        "declared_terminals":s.declared_terminals,"visited_zeros":s.visited_zeros,
        "missing_owners":s.missing_owners,"missing_rules":s.missing_rules,"finished":s.finished,
        "authority":"diagnostic counters only; cold native replay required"})
}

#[derive(Debug, PartialEq, Eq)]
enum FailureDisposition {
    Decline,
    Cancelled,
    Hard,
}

fn failure_disposition(
    reason: &CandidateRoutedCampaignFailure,
    cancellation: bool,
) -> FailureDisposition {
    // A late cancellation after a resource result must not enter fallback.
    // Coincident hard source/algebra errors retain their own classification.
    match reason {
        CandidateRoutedCampaignFailure::Cancelled => FailureDisposition::Cancelled,
        CandidateRoutedCampaignFailure::Trace(
            CandidateRoutedError::ResourceLimit { .. }
            | CandidateRoutedError::Transport(
                rustred::sector::symmetry::integral_transport::Error::Expansion(
                    rustred::sector::symmetry::integral_transport::ExpansionError::ResourceLimit {
                        ..
                    },
                ),
            ),
        ) => {
            if cancellation {
                FailureDisposition::Cancelled
            } else {
                FailureDisposition::Decline
            }
        }
        _ => FailureDisposition::Hard,
    }
}

/// Closed is possible only after the native kernel drains and its actual
/// frontier is empty. Counter equality is a consistency check, not the proof.
pub(super) fn run<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    domain: &Domain<N>,
    recipe: Recipe,
    cancel: &AtomicBool,
    account: Option<&Account>,
) -> Outcome {
    let empty = |status| json!({"status":status,"attempted":false});
    let fail = |error: String| Outcome::Failed {
        error,
        cancelled: cancel.load(Ordering::Acquire),
        work: empty("admission_error"),
    };
    if let Err(error) = recipe.validate() {
        return fail(error.into());
    }
    let target = match point(domain) {
        Ok(Some(target)) => target,
        Ok(None) => {
            return Outcome::Declined {
                work: empty("non_singleton"),
            };
        }
        Err(error) => return fail(error),
    };
    let admission = match FiniteRootAdmission::try_new(
        [RootRegionInput {
            support: domain.owner,
            lower: domain.lower.clone(),
            upper: domain.upper.clone(),
            rank: domain.rank,
            powers: domain.powers,
        }],
        1,
    ) {
        Ok(admission) => admission,
        Err(rustred::solver::RootAdmissionError::EmptyRegion { .. }) => {
            return Outcome::Declined {
                work: empty("empty_capped_domain"),
            };
        }
        Err(error) => return fail(inspection::debug(&error)),
    };
    // The caller's panic boundary converts a native panic into an error, never
    // a retryable summary miss. No completed-state cache is retained here.
    let result = reducer.trace_targets_inline_with_entry_admission_and_observer(
        [target],
        CandidateEntryAdmission::ExplicitFinite(&admission),
        recipe.limits.budget(),
        cancel,
        |snapshot| {
            if let Some(account) = account {
                account.record(work_json(snapshot, "running"));
            }
        },
    );
    match result {
        Ok(report) => {
            let s = report.snapshot();
            if cancel.load(Ordering::Acquire) {
                return Outcome::Failed {
                    error: "finite replay cancelled".into(),
                    cancelled: true,
                    work: work_json(s, "cancelled"),
                };
            }
            if !s.finished
                || s.active_nodes != 0
                || s.queued_nodes != 0
                || s.failed_nodes != 0
                || s.reserved_coalescing_additions != 0
            {
                return Outcome::Failed {
                    error: "finite replay unfinished native result".into(),
                    cancelled: false,
                    work: work_json(s, "invalid_completion"),
                };
            }
            if !report.trace().frontier().is_empty() {
                Outcome::Declined {
                    work: work_json(s, "frontier"),
                }
            } else {
                Outcome::Closed {
                    recipe,
                    work: work_json(s, "closed"),
                }
            }
        }
        Err(error) => {
            // Deliberately small typed decline set. UnsupportedSupportTransition,
            // source/context/order/algebra failures and panics are HARD errors.
            let disposition = failure_disposition(error.reason(), cancel.load(Ordering::Acquire));
            if disposition == FailureDisposition::Decline {
                Outcome::Declined {
                    work: work_json(error.snapshot(), "aggregate_budget"),
                }
            } else {
                Outcome::Failed {
                    error: inspection::debug(error.reason()),
                    cancelled: disposition == FailureDisposition::Cancelled,
                    work: work_json(error.snapshot(), "native_error"),
                }
            }
        }
    }
}
