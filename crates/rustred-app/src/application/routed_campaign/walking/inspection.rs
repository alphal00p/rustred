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
    initial_overlap::{InitialOverlapIndex, InitialOverlapScope},
    mask, power_bounds_json,
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
    pub powers: rustred::solver::DomainPowerBounds,
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
        target: usize,
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
                    target: a_target,
                    successor: a,
                    conditional: b,
                },
                Effect::PreAdmittedOrthantReuse {
                    target: b_target,
                    successor: c,
                    conditional: d,
                },
            ) => a == c && b == d && a_target == b_target,
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
    ApplyPartial(OwnerAppliedStats, InitialOverlapScope),
    /// G2' plan: the residual D band only (stats zero for a full cover).
    ApplyG2(OwnerAppliedStats, super::g2::G2Scope),
    Route(rustred::solver::CandidateDomainRouteStats),
}
pub(super) struct Finished {
    pub stats: NativeStats,
    pub error: Option<String>,
    pub error_kind: &'static str,
    pub seconds: f64,
}
impl Finished {
    pub fn initial_overlap_scope(&self) -> Option<InitialOverlapScope> {
        match self.stats {
            NativeStats::ApplyPartial(_, scope) => Some(scope),
            _ => None,
        }
    }
    pub fn g2_scope(&self) -> Option<super::g2::G2Scope> {
        match self.stats {
            NativeStats::ApplyG2(_, scope) => Some(scope),
            _ => None,
        }
    }
    pub fn native_operations(&self) -> usize {
        match self.stats {
            NativeStats::Apply(s)
            | NativeStats::ApplyPartial(s, _)
            | NativeStats::ApplyG2(s, _) => s.native_operations,
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
    overlap: &InitialOverlapIndex<N>,
    emit: &mut (impl FnMut(Event<N>) -> ControlFlow<()> + ?Sized),
) -> Finished {
    // Test-only stream tap for the W0.3 differential proof (reinspection.rs).
    #[cfg(all(test, feature = "cli"))]
    if super::reinspection::tap::enabled() {
        return super::reinspection::tap::record(domain, emit, |emit| {
            inspect_untapped(
                reducer,
                domain,
                request,
                cancellation,
                initial,
                overlap,
                emit,
            )
        });
    }
    inspect_untapped(
        reducer,
        domain,
        request,
        cancellation,
        initial,
        overlap,
        emit,
    )
}

fn inspect_untapped<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    domain: &Domain<N>,
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
    initial: &InitialOrthants<N>,
    overlap: &InitialOverlapIndex<N>,
    emit: &mut (impl FnMut(Event<N>) -> ControlFlow<()> + ?Sized),
) -> Finished {
    if let Some(finished) = inspect_initial_overlap(
        reducer,
        domain,
        request,
        cancellation,
        initial,
        overlap,
        emit,
    ) {
        return finished;
    }
    inspect_options(reducer, domain, request, cancellation, initial, true, emit)
}

pub(super) fn inspect_initial_overlap<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    domain: &Domain<N>,
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
    initial: &InitialOrthants<N>,
    overlap: &InitialOverlapIndex<N>,
    emit: &mut (impl FnMut(Event<N>) -> ControlFlow<()> + ?Sized),
) -> Option<Finished> {
    if !request.reuse_initial_d_bands {
        return None;
    }
    let started = Instant::now();
    let plan = overlap.plan(domain, cancellation)?;
    // Exactly one unchanged native visitor; the residual is NOT a
    // queue child and cannot be suppressed by its original parent.
    let mut finished = inspect_options(
        reducer,
        &plan.residual,
        request,
        cancellation,
        initial,
        true,
        emit,
    );
    let NativeStats::Apply(stats) = finished.stats else {
        unreachable!("Apply-only initial overlap");
    };
    finished.stats = NativeStats::ApplyPartial(stats, plan.scope);
    finished.seconds = started.elapsed().as_secs_f64();
    Some(finished)
}

/// `inspect` with G2' residual anchors: the initial-D-band plan keeps
/// precedence; an eligible Apply job then inspects only its planned residual
/// D band (nothing for a full cover). The decision is handed to the
/// coordinator through the store before any event is emitted. Record seconds
/// include the plan.
#[allow(clippy::too_many_arguments)]
pub(super) fn inspect_g2<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    id: usize,
    domain: &Domain<N>,
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
    initial: &InitialOrthants<N>,
    overlap: &InitialOverlapIndex<N>,
    store: &super::g2::Store<N>,
    emit: &mut (impl FnMut(Event<N>) -> ControlFlow<()> + ?Sized),
) -> Finished {
    if !store.eligible(id, domain) {
        return inspect(
            reducer,
            domain,
            request,
            cancellation,
            initial,
            overlap,
            emit,
        );
    }
    if let Some(finished) = inspect_initial_overlap(
        reducer,
        domain,
        request,
        cancellation,
        initial,
        overlap,
        emit,
    ) {
        return finished;
    }
    let started = Instant::now();
    let outcome = store.decide(id, domain, cancellation);
    let super::g2::Outcome::Planned(plan) = &*outcome else {
        let mut finished =
            inspect_options(reducer, domain, request, cancellation, initial, true, emit);
        finished.seconds = started.elapsed().as_secs_f64();
        return finished;
    };
    let mut finished = match plan.residual_domain(domain) {
        Some(residual) => inspect_options(
            reducer,
            &residual,
            request,
            cancellation,
            initial,
            true,
            emit,
        ),
        None => Finished {
            stats: NativeStats::Apply(OwnerAppliedStats::default()),
            error: None,
            error_kind: "none",
            seconds: 0.0,
        },
    };
    let NativeStats::Apply(stats) = finished.stats else {
        unreachable!("Apply-only G2' residual");
    };
    finished.stats = NativeStats::ApplyG2(stats, plan.scope());
    finished.seconds = started.elapsed().as_secs_f64();
    finished
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

pub(super) fn inspect_options<const N: usize>(
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
        None,
        cancellation,
        initial,
        &mut |event| cache.forward(event, emit),
    )
}

/// Frontier kind of a rescue-abandoned obligation (`rescue.rs`).
pub(super) const RESCUE_ABANDONED_KIND: &str = "rescue_abandoned_dead_cone";

/// A rescue-abandoned obligation: no live input root reaches it after a
/// frontier rescue, so no query can certify through it. It is published
/// without any native inspection, carrying one explicit frontier (so it
/// never seals and nothing that reaches it can close) and no successor; the
/// single accepted event is that frontier.
pub(super) fn abandoned<const N: usize>(
    domain: &Domain<N>,
    emit: &mut (impl FnMut(Event<N>) -> ControlFlow<()> + ?Sized),
) -> Finished {
    let _ = emit(Event::one(Effect::Frontier {
        successor: false,
        conditional: false,
        value: json!({"kind":RESCUE_ABANDONED_KIND,
            "reason":"frontier rescue: no live input root reaches this pending obligation; not inspected",
            "reached_missing_rule_claim":false}),
    }));
    let stats = if domain.phase == Phase::Route {
        NativeStats::Route(rustred::solver::CandidateDomainRouteStats {
            events: 1,
            ..Default::default()
        })
    } else {
        NativeStats::Apply(OwnerAppliedStats {
            events: 1,
            ..Default::default()
        })
    };
    Finished {
        stats,
        error: None,
        error_kind: "none",
        seconds: 0.0,
    }
}

/// Reference inspection for the offline closure verifier: the unchanged
/// native visitor with every walk lever off: no pre-admitted orthant
/// shortcut, no job-local reuse cache, no initial-overlap planning and no
/// physical subdivision. Every successor reaches `emit` as a raw `Admit`.
pub(super) fn inspect_reference<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    domain: &Domain<N>,
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
    emit: &mut (impl FnMut(Event<N>) -> ControlFlow<()> + ?Sized),
) -> Finished {
    inspect_native(
        reducer,
        domain,
        request,
        None,
        cancellation,
        &InitialOrthants::empty(),
        emit,
    )
}

/// Offline inventory uses the same complete native inspection as the closure
/// verifier, observing classifications before the production event stream
/// deliberately erases their rule/terminal identities. No payloads or callbacks
/// are retained by the production walk.
pub(super) fn inspect_reference_with_classifications<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    domain: &Domain<N>,
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
    classified: &mut dyn FnMut(&rustred::solver::OwnerDomainMatchPiece<N>) -> ControlFlow<()>,
    emit: &mut (impl FnMut(Event<N>) -> ControlFlow<()> + ?Sized),
) -> Finished {
    inspect_native_observed(
        reducer,
        domain,
        request,
        None,
        cancellation,
        &InitialOrthants::empty(),
        Some(classified),
        emit,
    )
}

/// A physical source duty bypasses initial-overlap planning: its pending broad
/// parent is not evidence that this part has been inspected. Outgoing reuse is
/// unchanged and remains after the native source/guard/child checks.
pub(super) fn inspect_part<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    domain: &Domain<N>,
    request: &OwnerDomainWalkRequest,
    part: u8,
    cancellation: &AtomicBool,
    initial: &InitialOrthants<N>,
    emit: &mut (impl FnMut(Event<N>) -> ControlFlow<()> + ?Sized),
) -> Finished {
    let mut parent = request.applied_limits;
    parent.matching = request.matching.match_limits;
    let limits = super::physical_parts::limits(parent, part);
    let mut cache = super::reuse::Cache::new(true);
    inspect_native(
        reducer,
        domain,
        request,
        Some(limits),
        cancellation,
        initial,
        &mut |event| cache.forward(event, emit),
    )
}

fn inspect_native<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    domain: &Domain<N>,
    request: &OwnerDomainWalkRequest,
    limits: Option<rustred::solver::OwnerAppliedLimits>,
    cancellation: &AtomicBool,
    initial: &InitialOrthants<N>,
    emit: &mut (impl FnMut(Event<N>) -> ControlFlow<()> + ?Sized),
) -> Finished {
    inspect_native_observed(
        reducer,
        domain,
        request,
        limits,
        cancellation,
        initial,
        None,
        emit,
    )
}

fn inspect_native_observed<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    domain: &Domain<N>,
    request: &OwnerDomainWalkRequest,
    limits: Option<rustred::solver::OwnerAppliedLimits>,
    cancellation: &AtomicBool,
    initial: &InitialOrthants<N>,
    mut classified: Option<
        &mut dyn FnMut(&rustred::solver::OwnerDomainMatchPiece<N>) -> ControlFlow<()>,
    >,
    emit: &mut (impl FnMut(Event<N>) -> ControlFlow<()> + ?Sized),
) -> Finished {
    if domain.phase == Phase::Route {
        return super::routing::inspect(reducer, domain, request, cancellation, initial, emit);
    }
    let started = Instant::now();
    let limits = limits.unwrap_or_else(|| {
        let mut limits = request.applied_limits;
        limits.matching = request.matching.match_limits;
        limits
    });
    let mut conversion_error = None;
    let result = reducer.programs().visit_power_bounded_owner_applied_successors(
        domain.owner, &domain.lower, &domain.upper, domain.rank, domain.powers, limits, cancellation,
        |event| {
            if let OwnerAppliedEvent::Classified(piece) = &event
                && let Some(observe) = classified.as_mut()
                && observe(piece).is_break()
            {
                return ControlFlow::Break(());
            }
            let effect = match event {
                OwnerAppliedEvent::Classified(piece) => match piece.disposition() {
                    OwnerDomainMatchDisposition::SelectedRule { .. }
                    | OwnerDomainMatchDisposition::Terminal { .. }
                    | OwnerDomainMatchDisposition::ExactZeroSector => Effect::Count,
                    other => Effect::Frontier { successor: false, conditional: false,
                        value: json!({"kind":"local_dispatch_frontier", "disposition":debug(&other),
                            "lower":piece.lower(), "upper":piece.upper(), "rank":piece.max_numerator_rank(),
                            "power_bounds":power_bounds_json(piece.power_bounds()),
                            "reached_missing_rule_claim":false}) },
                },
                OwnerAppliedEvent::OptionalCoefficientRefusal { source, source_lower,
                    source_upper, shift, original_term_ordinal, failure } => {
                    let IndexedAlgebraError::ResourceLimit { resource, requested, limit } = failure else {
                        conversion_error = Some("unexpected optional coefficient refusal diagnostic".to_owned());
                        return ControlFlow::Break(());
                    };
                    Effect::Optional(OptionalDiagnostic { disposition: source.disposition(),
                        rank: source.max_numerator_rank(), powers: source.power_bounds(), lower: source_lower.to_vec(),
                        upper: source_upper.to_vec(), shift: shift.to_vec(), ordinal: original_term_ordinal,
                        resource, requested: *requested, limit: *limit })
                },
                OwnerAppliedEvent::Successor(child) => {
                    let conditional = child.coefficient_nonzero == OwnerAppliedNonzero::Conditional;
                    if child.has_installed_target_owner {
                        if let Some(target) = initial.target(Phase::Apply, child.target_sector, child.target_rank_limit) {
                            return emit(Event::one(Effect::PreAdmittedOrthantReuse { target, successor: true, conditional }));
                        }
                        Effect::Admit { successor: true, conditional, domain: Domain {
                            phase: Phase::Apply, owner: *child.target_sector, lower: child.target_lower.to_vec(),
                            upper: child.target_upper.to_vec(), rank: child.target_rank_limit,
                            powers: child.target_power_bounds } }
                    } else if request.route_domain_overcover {
                        if let Some(target) = initial.target(Phase::Route, child.target_sector, child.target_rank_limit) {
                            return emit(Event::one(Effect::PreAdmittedOrthantReuse { target, successor: true, conditional }));
                        }
                        // Routing must see the actual successor box. Widening it
                        // to a full orthant discards finite starting-power bounds
                        // before the admitted native map can transport them.
                        Effect::Admit { successor: true, conditional, domain: Domain {
                            phase: Phase::Route, owner: *child.target_sector,
                            lower: child.target_lower.to_vec(),
                            upper: child.target_upper.to_vec(), rank: child.target_rank_limit,
                            powers: child.target_power_bounds } }
                    } else {
                        Effect::Frontier { successor: true, conditional,
                            value: json!({"kind":"routing_frontier", "target_owner":mask(child.target_sector),
                            "target_lower":child.target_lower, "target_upper":child.target_upper,
                            "target_rank":child.target_rank_limit, "source_lower":child.source_lower,
                            "target_power_bounds":power_bounds_json(child.target_power_bounds),
                            "source_power_bounds":power_bounds_json(child.source.power_bounds()),
                            "source_upper":child.source_upper, "shift":child.shift.as_slice(),
                            "coefficient_nonzero":debug(&child.coefficient_nonzero),
                            "selected_rule":debug(&child.source.disposition()), "reached_missing_rule_claim":false}) }
                    }
                },
                OwnerAppliedEvent::Problem(p) => Effect::Frontier { successor: false, conditional: false,
                    value: json!({"kind":"rhs_obligation", "problem":debug(&p.kind),
                        "source_lower":p.source_lower, "source_upper":p.source_upper,
                        "source_power_bounds":power_bounds_json(p.source.power_bounds()),
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
