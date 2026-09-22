//! Worker-local native inspection. Only small owned descriptors cross threads.
use std::fmt::{self, Write};
use std::ops::ControlFlow;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use rustred::algebra::IndexedAlgebraError;
use rustred::solver::{
    OwnerAppliedEvent, OwnerAppliedNonzero, OwnerAppliedStats, OwnerDomainMatchDisposition,
    RoutedCandidateReducer,
};
use serde_json::{Value, json};

use super::{
    OwnerDomainWalkRequest,
    initial_orthants::InitialOrthants,
    mask,
    queue::{Domain, Phase},
};

pub(super) const DETAIL_LIMIT: usize = 4096;

/// Bounded formatting even if a backend supplies a very large message. The
/// visible suffix makes truncation explicit; it never changes failure status.
pub(super) fn debug(value: &impl fmt::Debug) -> String {
    struct Limited(String);
    impl Write for Limited {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            let remaining = DETAIL_LIMIT.saturating_sub(self.0.len());
            if s.len() <= remaining {
                self.0.push_str(s);
                return Ok(());
            }
            let mut end = remaining;
            while !s.is_char_boundary(end) {
                end -= 1;
            }
            self.0.push_str(&s[..end]);
            Err(fmt::Error)
        }
    }
    let mut out = Limited(String::new());
    if write!(&mut out, "{value:?}").is_err() {
        out.0.push_str(" [truncated]");
    }
    out.0
}

pub(super) struct OptionalDiagnostic {
    pub disposition: OwnerDomainMatchDisposition,
    pub rank: Option<u32>,
    pub lower: Vec<u64>,
    pub upper: Vec<Option<u64>>,
    pub shift: Vec<i64>,
    pub ordinal: Option<usize>,
    pub resource: &'static str,
    pub requested: usize,
    pub limit: usize,
}

pub(super) enum Effect<const N: usize> {
    Count,
    /// This job emitted an identical scheduling request earlier, in order.
    /// Pending reuse only; this does not certify completed coverage.
    KnownReuse {
        successor: bool,
        conditional: bool,
    },
    /// Contained in a full orthant actually admitted before execution began.
    /// Its pending inspection is still required; native checks were not skipped.
    PreAdmittedOrthantReuse {
        successor: bool,
        conditional: bool,
    },
    Admit {
        domain: Domain<N>,
        successor: bool,
        conditional: bool,
    },
    Frontier {
        value: Value,
        successor: bool,
        conditional: bool,
    },
    Optional(OptionalDiagnostic),
}

pub(super) struct Event<const N: usize> {
    /// Adjacent homogeneous callback charge vectors may be compacted while
    /// preserving their exact ordered prefix at every cap.
    pub count: usize,
    pub effect: Effect<N>,
}
impl<const N: usize> Event<N> {
    pub fn one(effect: Effect<N>) -> Self {
        Self { count: 1, effect }
    }
    pub fn mergeable(&self, other: &Self) -> bool {
        match (&self.effect, &other.effect) {
            (Effect::Count, Effect::Count) => true,
            (
                Effect::KnownReuse {
                    successor: a,
                    conditional: b,
                },
                Effect::KnownReuse {
                    successor: c,
                    conditional: d,
                },
            ) => a == c && b == d,
            (
                Effect::PreAdmittedOrthantReuse {
                    successor: a,
                    conditional: b,
                },
                Effect::PreAdmittedOrthantReuse {
                    successor: c,
                    conditional: d,
                },
            ) => a == c && b == d,
            _ => false,
        }
    }
    /// Conservative logical owned-storage charge, NOT an allocator/RSS bound.
    pub fn weight(&self) -> usize {
        fn json_weight(v: &Value) -> usize {
            128 + match v {
                Value::String(s) => s.capacity(),
                Value::Array(a) => {
                    a.capacity() * std::mem::size_of::<Value>()
                        + a.iter().map(json_weight).sum::<usize>()
                }
                Value::Object(o) => o
                    .iter()
                    .map(|(k, v)| 128 + k.capacity() + json_weight(v))
                    .sum(),
                _ => 0,
            }
        }
        std::mem::size_of::<Self>()
            + match &self.effect {
                Effect::Count
                | Effect::KnownReuse { .. }
                | Effect::PreAdmittedOrthantReuse { .. } => 0,
                Effect::Admit { domain, .. } => {
                    domain.lower.capacity() * 8
                        + domain.upper.capacity() * std::mem::size_of::<Option<u64>>()
                }
                Effect::Frontier { value, .. } => json_weight(value),
                Effect::Optional(d) => {
                    d.lower.capacity() * 8
                        + d.upper.capacity() * std::mem::size_of::<Option<u64>>()
                        + d.shift.capacity() * 8
                }
            }
    }
}

#[derive(Clone, Copy)]
pub(super) enum NativeStats {
    Apply(OwnerAppliedStats),
    Route(rustred::solver::CandidateDomainRouteStats),
}
pub(super) struct Finished {
    pub stats: NativeStats,
    pub error: Option<String>,
    pub error_kind: &'static str,
    pub seconds: f64,
}
impl Finished {
    pub fn native_operations(&self) -> usize {
        match self.stats {
            NativeStats::Apply(s) => s.native_operations,
            NativeStats::Route(_) => 0,
        }
    }
}

pub(super) fn inspect<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    domain: &Domain<N>,
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
    initial: &InitialOrthants<N>,
    emit: &mut (impl FnMut(Event<N>) -> ControlFlow<()> + ?Sized),
) -> Finished {
    inspect_options(reducer, domain, request, cancellation, initial, true, emit)
}

/// Private cache-off reference seam for tests/controlled experiments. No new
/// public request/CLI policy or native applicability mode is introduced.
#[cfg(test)]
pub(super) fn inspect_with_reuse<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    domain: &Domain<N>,
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
    enabled: bool,
    emit: &mut (impl FnMut(Event<N>) -> ControlFlow<()> + ?Sized),
) -> Finished {
    inspect_options(
        reducer,
        domain,
        request,
        cancellation,
        &InitialOrthants::empty(),
        enabled,
        emit,
    )
}

fn inspect_options<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    domain: &Domain<N>,
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
    initial: &InitialOrthants<N>,
    enabled: bool,
    emit: &mut (impl FnMut(Event<N>) -> ControlFlow<()> + ?Sized),
) -> Finished {
    let mut cache = super::reuse::Cache::new(enabled);
    inspect_native(
        reducer,
        domain,
        request,
        cancellation,
        initial,
        &mut |event| cache.forward(event, emit),
    )
}

fn inspect_native<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    domain: &Domain<N>,
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
    initial: &InitialOrthants<N>,
    emit: &mut (impl FnMut(Event<N>) -> ControlFlow<()> + ?Sized),
) -> Finished {
    if domain.phase == Phase::Route {
        return super::routing::inspect(reducer, domain, request, cancellation, initial, emit);
    }
    let started = Instant::now();
    let mut limits = request.applied_limits;
    limits.matching = request.matching.match_limits;
    let mut conversion_error = None;
    let result = reducer.programs().visit_owner_applied_successors(
        domain.owner, &domain.lower, &domain.upper, domain.rank, limits, cancellation,
        |event| {
            let effect = match event {
                OwnerAppliedEvent::Classified(piece) => match piece.disposition() {
                    OwnerDomainMatchDisposition::SelectedRule { .. }
                    | OwnerDomainMatchDisposition::Terminal { .. }
                    | OwnerDomainMatchDisposition::ExactZeroSector => Effect::Count,
                    other => Effect::Frontier { successor: false, conditional: false,
                        value: json!({"kind":"local_dispatch_frontier", "disposition":debug(&other),
                            "lower":piece.lower(), "upper":piece.upper(), "rank":piece.max_numerator_rank(),
                            "reached_missing_rule_claim":false}) },
                },
                OwnerAppliedEvent::OptionalCoefficientRefusal { source, source_lower,
                    source_upper, shift, original_term_ordinal, failure } => {
                    let IndexedAlgebraError::ResourceLimit { resource, requested, limit } = failure else {
                        conversion_error = Some("unexpected optional coefficient refusal diagnostic".to_owned());
                        return ControlFlow::Break(());
                    };
                    Effect::Optional(OptionalDiagnostic { disposition: source.disposition(),
                        rank: source.max_numerator_rank(), lower: source_lower.to_vec(),
                        upper: source_upper.to_vec(), shift: shift.to_vec(), ordinal: original_term_ordinal,
                        resource, requested: *requested, limit: *limit })
                },
                OwnerAppliedEvent::Successor(child) => {
                    let conditional = child.coefficient_nonzero == OwnerAppliedNonzero::Conditional;
                    if child.has_installed_target_owner {
                        if initial.contains(Phase::Apply, child.target_sector, child.target_rank_limit) {
                            return emit(Event::one(Effect::PreAdmittedOrthantReuse { successor: true, conditional }));
                        }
                        Effect::Admit { successor: true, conditional, domain: Domain {
                            phase: Phase::Apply, owner: *child.target_sector, lower: child.target_lower.to_vec(),
                            upper: child.target_upper.to_vec(), rank: child.target_rank_limit } }
                    } else if request.route_domain_overcover {
                        if initial.contains(Phase::Route, child.target_sector, child.target_rank_limit) {
                            return emit(Event::one(Effect::PreAdmittedOrthantReuse { successor: true, conditional }));
                        }
                        Effect::Admit { successor: true, conditional,
                            domain: Domain::route_cover(*child.target_sector, child.target_rank_limit) }
                    } else {
                        Effect::Frontier { successor: true, conditional,
                            value: json!({"kind":"routing_frontier", "target_owner":mask(child.target_sector),
                            "target_lower":child.target_lower, "target_upper":child.target_upper,
                            "target_rank":child.target_rank_limit, "source_lower":child.source_lower,
                            "source_upper":child.source_upper, "shift":child.shift.as_slice(),
                            "coefficient_nonzero":debug(&child.coefficient_nonzero),
                            "selected_rule":debug(&child.source.disposition()), "reached_missing_rule_claim":false}) }
                    }
                },
                OwnerAppliedEvent::Problem(p) => Effect::Frontier { successor: false, conditional: false,
                    value: json!({"kind":"rhs_obligation", "problem":debug(&p.kind),
                        "source_lower":p.source_lower, "source_upper":p.source_upper,
                        "shift":p.shift.as_slice(), "original_term":p.original_term_ordinal,
                        "coefficient_nonzero":debug(&p.coefficient_nonzero),
                        "selected_rule":debug(&p.source.disposition()), "reached_missing_rule_claim":false}) },
                OwnerAppliedEvent::RuleFinished { .. } => Effect::Count,
            };
            emit(Event::one(effect))
        });
    let (stats, error, error_kind) = match result {
        Ok(stats) => (stats, None, "none"),
        Err(e) => {
            use rustred::solver::{OwnerAppliedFailure as A, OwnerDomainMatchFailure as M};
            let kind = match &e.failure {
                A::Cancelled | A::Matching(M::Cancelled) => "cancelled",
                A::StoppedByConsumer | A::Matching(M::StoppedByConsumer) => "consumer_stop",
                _ => "native_failure",
            };
            (e.stats, Some(debug(&e.failure)), kind)
        }
    };
    Finished {
        stats: NativeStats::Apply(stats),
        error_kind: if conversion_error.is_some() {
            "conversion"
        } else {
            error_kind
        },
        error: conversion_error.or(error),
        seconds: started.elapsed().as_secs_f64(),
    }
}
