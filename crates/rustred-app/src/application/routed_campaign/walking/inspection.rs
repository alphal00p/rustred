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

/// W0.9 falsifier (THROWAWAY, branch `fable_5_1-v3-widen`): G1 hull widening
/// of Apply successors, selected by `RUSTRED_W0_G1_WIDEN=finite|all`.
///
/// W(S) = {same phase and owner, box [0, inf)^N, R <= Rmax(S), A <= Amax(S),
/// Dmin(S) <= D <= Dmax(S)} with the tight extrema of `DomainPowerSummary`.
/// No axis is ever tightened: an infinite extremum stays infinite, and an
/// extremum that does not fit the public u32/u64/i64 field becomes None.
/// `finite` keeps S unchanged when Amax(S) is infinite; `all` widens anyway.
/// Optional `RUSTRED_W0_G1_OWNERS` / `RUSTRED_W0_G1_EXCLUDE` restrict the
/// widened owners (comma-separated masks). S subset W(S) is checked in
/// release; a violation aborts the process.
pub(super) mod g1 {
    use std::collections::HashSet;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

    use rustred::solver::{DomainPowerBounds, DomainPowerSummary};
    use serde_json::{Value, json};

    use super::super::queue::{Domain, Phase};

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum Mode {
        Off,
        Finite,
        All,
    }
    struct Config {
        mode: Mode,
        owners: Option<HashSet<String>>,
        exclude: HashSet<String>,
    }
    fn masks(name: &str) -> Option<HashSet<String>> {
        std::env::var(name).ok().map(|v| {
            v.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect()
        })
    }
    fn config() -> &'static Config {
        static CONFIG: OnceLock<Config> = OnceLock::new();
        CONFIG.get_or_init(|| {
            let mode = match std::env::var("RUSTRED_W0_G1_WIDEN").as_deref() {
                Err(_) | Ok("") | Ok("off") => Mode::Off,
                Ok("finite") => Mode::Finite,
                Ok("all") => Mode::All,
                Ok(other) => panic!("RUSTRED_W0_G1_WIDEN={other}: expected off|finite|all"),
            };
            let config = Config {
                mode,
                owners: masks("RUSTRED_W0_G1_OWNERS"),
                exclude: masks("RUSTRED_W0_G1_EXCLUDE").unwrap_or_default(),
            };
            if mode != Mode::Off {
                eprintln!(
                    "W0.9 G1 widening ACTIVE: mode={mode:?} owners={:?} exclude={:?}",
                    config.owners, config.exclude
                );
            }
            config
        })
    }

    static CALLS: AtomicU64 = AtomicU64::new(0);
    static WIDENED: AtomicU64 = AtomicU64::new(0);
    static UNCHANGED: AtomicU64 = AtomicU64::new(0);
    static SKIPPED_UNBOUNDED_A: AtomicU64 = AtomicU64::new(0);
    static SKIPPED_OWNER: AtomicU64 = AtomicU64::new(0);
    static SKIPPED_EMPTY_OR_ERROR: AtomicU64 = AtomicU64::new(0);
    static ORTHANT_REUSE_AFTER_WIDENING: AtomicU64 = AtomicU64::new(0);

    pub(in super::super) fn note_orthant_reuse() {
        ORTHANT_REUSE_AFTER_WIDENING.fetch_add(1, Relaxed);
    }

    pub(in super::super) fn active() -> bool {
        config().mode != Mode::Off
    }

    /// Returns W(S) or S itself; never a domain that fails to contain S.
    pub(in super::super) fn widen<const N: usize>(domain: Domain<N>) -> Domain<N> {
        let config = config();
        if config.mode == Mode::Off || domain.phase != Phase::Apply {
            return domain;
        }
        CALLS.fetch_add(1, Relaxed);
        if config.owners.is_some() || !config.exclude.is_empty() {
            let mask = super::super::mask(&domain.owner);
            if config.owners.as_ref().is_some_and(|o| !o.contains(&mask))
                || config.exclude.contains(&mask)
            {
                SKIPPED_OWNER.fetch_add(1, Relaxed);
                return domain;
            }
        }
        let Ok(summary) = DomainPowerSummary::try_new(
            domain.owner,
            &domain.lower,
            &domain.upper,
            domain.rank,
            domain.powers,
        ) else {
            SKIPPED_EMPTY_OR_ERROR.fetch_add(1, Relaxed);
            return domain;
        };
        let Some(extrema) = summary.extrema() else {
            SKIPPED_EMPTY_OR_ERROR.fetch_add(1, Relaxed);
            return domain;
        };
        let (_, a_upper) = extrema.positive_power();
        let (_, r_upper) = extrema.numerator_rank();
        let (d_lower, d_upper) = extrema.power_difference();
        if a_upper.is_none() && config.mode == Mode::Finite {
            SKIPPED_UNBOUNDED_A.fetch_add(1, Relaxed);
            return domain;
        }
        let widened = Domain {
            phase: domain.phase,
            owner: domain.owner,
            lower: vec![0; N],
            upper: vec![None; N],
            rank: r_upper.and_then(|r| u32::try_from(r).ok()),
            powers: DomainPowerBounds {
                max_positive_power: a_upper.and_then(|a| u64::try_from(a).ok()),
                min_power_difference: d_lower.and_then(|d| i64::try_from(d).ok()),
                max_power_difference: d_upper.and_then(|d| i64::try_from(d).ok()),
            },
        };
        if widened == domain {
            UNCHANGED.fetch_add(1, Relaxed);
            return domain;
        }
        let container = DomainPowerSummary::try_new(
            widened.owner,
            &widened.lower,
            &widened.upper,
            widened.rank,
            widened.powers,
        )
        .unwrap_or_else(|e| panic!("W0.9 G1: widened domain invalid: {e}"));
        assert!(
            container.contains(&summary),
            "W0.9 G1: widened domain does not contain its successor"
        );
        WIDENED.fetch_add(1, Relaxed);
        widened
    }

    pub(in super::super) fn report() -> Value {
        let c = config();
        json!({
            "mode": format!("{:?}", c.mode),
            "owners": c.owners.as_ref().map(|o| { let mut v: Vec<_> = o.iter().cloned().collect(); v.sort(); v }),
            "exclude": { let mut v: Vec<_> = c.exclude.iter().cloned().collect(); v.sort(); v },
            "apply_successor_calls": CALLS.load(Relaxed),
            "widened": WIDENED.load(Relaxed),
            "unchanged_already_widened_shape": UNCHANGED.load(Relaxed),
            "skipped_unbounded_a": SKIPPED_UNBOUNDED_A.load(Relaxed),
            "skipped_owner_filter": SKIPPED_OWNER.load(Relaxed),
            "skipped_empty_or_error": SKIPPED_EMPTY_OR_ERROR.load(Relaxed),
            "orthant_reuse_after_widening": ORTHANT_REUSE_AFTER_WIDENING.load(Relaxed),
            "scope": "process counters over native Apply successor events (including uncommitted/cancelled); throwaway W0.9 falsifier"
        })
    }
}

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
    pub fn native_operations(&self) -> usize {
        match self.stats {
            NativeStats::Apply(s) | NativeStats::ApplyPartial(s, _) => s.native_operations,
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
    if request.reuse_initial_d_bands {
        let started = Instant::now();
        if let Some(plan) = overlap.plan(domain, cancellation) {
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
            return finished;
        }
    }
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
        None,
        cancellation,
        initial,
        &mut |event| cache.forward(event, emit),
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
                        let exact = Domain {
                            phase: Phase::Apply, owner: *child.target_sector, lower: child.target_lower.to_vec(),
                            upper: child.target_upper.to_vec(), rank: child.target_rank_limit,
                            powers: child.target_power_bounds };
                        // W0.9 falsifier: S is not pre-admitted; admit W(S) ⊇ S.
                        let domain = if g1::active() { g1::widen(exact) } else { exact };
                        if g1::active()
                            && let Some(target) = initial.target(Phase::Apply, &domain.owner, domain.rank)
                        {
                            g1::note_orthant_reuse();
                            return emit(Event::one(Effect::PreAdmittedOrthantReuse { target, successor: true, conditional }));
                        }
                        Effect::Admit { successor: true, conditional, domain }
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
