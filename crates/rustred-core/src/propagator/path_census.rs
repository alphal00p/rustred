//! Opt-in path census of one p reduction (M5 R0 WP0).
//!
//! A caller turns it on by returning a census from
//! [`PInspector::path_census`](super::library::PInspector::path_census); the
//! default inspector (and every closure inspector) returns `None`, and the
//! engine then pays one branch per popped key and per step. The census never
//! changes a step, a weight or the schedule: on and off give the same
//! reduction, byte for byte.
//!
//! What it records, on the caller's thread:
//! - every popped key (once its step is merged: the drivers' `processed_keys`
//!   then counts it): pop order (run-length by phase and owner or
//!   presentation), repeats (a 128-bit key hash) and phase regressions: the
//!   layered breadth-first certificate [`PPathCensus::certificate`] (T-BFS);
//! - every computed step, aggregated per (phase, kind, id, lines, dots, rank):
//!   keys stepped, step time, children by kind (same-owner Apply, other Apply,
//!   Route, Route on a zero sector), terminals, coefficient sizes, and children
//!   that stay in the parent's phase;
//! - at every phase start, the inflow: the distinct pending keys of the phase,
//!   per owner (Apply) or presentation (Route), and the Apply inflow by
//!   (lines, dots, rank);
//! - at every phase boundary, the driver's progress counters;
//! - per Custom owner, the hit count of every rule per phase
//!   ([`PPathCensus::rule_hits`]), and per Scheme owner the hit count of every
//!   scheme rule per phase ([`PPathCensus::scheme_rule_hits`]);
//! - optionally ([`PPathCensusOptions::keep_apply_keys`]) every stepped Apply
//!   key (owner class, powers and tag slots), for orbit counts;
//! - optionally ([`PPathCensusOptions::max_keys`]) a cap on popped keys: the
//!   request then stops with [`PError::Unsupported`] (sizing runs);
//! - every routed Route step (a sector with a census map onto another owner):
//!   per route and numerator part of the key, keys, transport terms and time
//!   ([`PPathCensus::routes`], M5 maps).
use std::collections::{BTreeMap, HashSet};
use std::time::Instant;

use crate::solver::CandidateOrderedReductionProgress;

use super::error::PError;

/// Kind of a scheduled key: `0` Route (id = presentation), `1` Apply (id =
/// owner class).
pub type PCensusKind = u8;

/// `(phase, kind, id, lines, dots, rank)` of a stepped key; dots =
/// `Sum (n - 1)` over `n > 1`, rank = `Sum -n` over `n < 0`, lines = lines of
/// the support.
pub type PCensusKey = (u32, PCensusKind, u64, u32, i64, i64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PPathCensusOptions {
    /// Keep every stepped Apply key ([`PPathCensus::apply_keys`]).
    pub keep_apply_keys: bool,
    /// Stop the request once more keys than this were popped.
    pub max_keys: Option<u64>,
}

/// Aggregate of the steps of one [`PCensusKey`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PCensusAgg {
    pub steps: u64,
    pub nanos: u64,
    /// Apply children of the parent's own owner (Apply parents only).
    pub ch_same: u64,
    pub ch_apply: u64,
    pub ch_route: u64,
    /// Route children on a zero (scaleless) sector; included in `ch_route`.
    pub ch_zero: u64,
    pub terminals: u64,
    /// Largest number of children of one step.
    pub ch_max: u64,
    /// Terms of the child and terminal coefficients (numerator + denominator).
    pub coeff_terms: u64,
    pub coeff_terms_max: u64,
    /// Steps without children and terminals.
    pub empty: u64,
    /// Children in the parent's phase (same-owner Apply children).
    pub ch_same_phase: u64,
    /// Children in the parent's phase that are not same-owner Apply keys, and
    /// children in an earlier phase: both must stay 0 (T-BFS).
    pub ch_foreign_same_phase: u64,
    pub ch_earlier_phase: u64,
}

impl PCensusAgg {
    fn add(&mut self, other: &Self) {
        self.steps += other.steps;
        self.nanos += other.nanos;
        self.ch_same += other.ch_same;
        self.ch_apply += other.ch_apply;
        self.ch_route += other.ch_route;
        self.ch_zero += other.ch_zero;
        self.terminals += other.terminals;
        self.ch_max = self.ch_max.max(other.ch_max);
        self.coeff_terms += other.coeff_terms;
        self.coeff_terms_max = self.coeff_terms_max.max(other.coeff_terms_max);
        self.empty += other.empty;
        self.ch_same_phase += other.ch_same_phase;
        self.ch_foreign_same_phase += other.ch_foreign_same_phase;
        self.ch_earlier_phase += other.ch_earlier_phase;
    }

    /// Children plus terminals: the edges the driver emits for these steps.
    pub fn edges(&self) -> u64 {
        self.ch_same + self.ch_apply + self.ch_route + self.terminals
    }
}

/// One computed step, built by the engine (on a worker when the scheduler is
/// concurrent) and absorbed on the caller's thread.
#[derive(Clone, Debug, Default)]
pub(crate) struct PStepRecord {
    pub(crate) key: PCensusKey,
    pub(crate) agg: PCensusAgg,
    /// The Apply key's view (powers, tag slots), when kept.
    pub(crate) apply_view: Option<Box<[i64]>>,
    /// The rule a Custom step applied: `(chart, position in the chart's rule
    /// list)` (M5 artefact v2, spec item 8).
    pub(crate) rule: Option<(u64, u32)>,
    /// The rule a Scheme step applied: its position in the scheme's stored
    /// rule list, `u32::MAX` for the Custom fallback closure (scheme-certs).
    pub(crate) scheme_rule: Option<u32>,
    /// A Route key on a routed (census-mapped) sector: its route and the
    /// numerator part of its powers (M5 maps).
    pub(crate) route: Option<PRouteRecord>,
}

/// One routed Route step (M5 maps): the sector `(presentation, support)`, the
/// numerator part of the key (`min(power, 0)` per slot) and the terms the
/// transport gave.
#[derive(Clone, Debug, Default)]
pub(crate) struct PRouteRecord {
    pub(crate) presentation: u32,
    pub(crate) support: u64,
    pub(crate) numerator: Box<[i64]>,
    pub(crate) terms: u64,
}

/// Aggregate of the routed steps of one numerator on one route (M5 maps).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PRouteAgg {
    pub keys: u64,
    pub terms: u64,
    pub nanos: u64,
}

/// An owner seen by the census.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PCensusOwner {
    pub lane: String,
    pub presentation: u32,
    pub t: usize,
    /// Node rank in the engine's DAG order (parents first, larger first).
    pub rank: u32,
}

/// One phase boundary.
#[derive(Clone, Copy, Debug)]
pub struct PCensusPhase {
    pub phase: usize,
    /// `true` at the start, `false` at the end.
    pub started: bool,
    /// Seconds since the census saw its first event.
    pub seconds: f64,
    pub progress: CandidateOrderedReductionProgress,
}

/// The layered breadth-first certificate of one request (T-BFS).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PBfsCertificate {
    pub popped: u64,
    pub distinct: u64,
    /// Keys popped more than once (must be 0).
    pub repeats: u64,
    /// Pops in an earlier phase than the previous pop (must be 0).
    pub phase_regressions: u64,
    /// Apply phases in which some owner has more than one contiguous run of
    /// pops (must be 0), and the number of Apply phases with pops.
    pub split_owner_phases: u64,
    pub apply_phases: u64,
    /// Children in an earlier phase, or in the parent's phase without being a
    /// same-owner Apply key (both must be 0; the driver's `check_descent`
    /// enforces the strict order inside a phase).
    pub earlier_phase_children: u64,
    pub foreign_same_phase_children: u64,
    /// Keys stepped from the step cache (their children are not in the census).
    pub cache_hits: u64,
}

impl PBfsCertificate {
    pub fn holds(&self) -> bool {
        self.repeats == 0
            && self.phase_regressions == 0
            && self.split_owner_phases == 0
            && self.earlier_phase_children == 0
            && self.foreign_same_phase_children == 0
            && self.popped == self.distinct
    }
}

#[derive(Debug)]
#[allow(clippy::type_complexity)]
pub struct PPathCensus {
    pub options: PPathCensusOptions,
    pub steps: BTreeMap<PCensusKey, PCensusAgg>,
    /// Phase start: the pending keys of the phase per `(kind, id)`.
    pub inflow: BTreeMap<usize, BTreeMap<(PCensusKind, u64), u64>>,
    /// Phase start: the pending Apply keys of the phase per (lines, dots, rank).
    pub inflow_profile: BTreeMap<usize, BTreeMap<(u32, i64, i64), u64>>,
    /// Phase start: pending keys of later phases.
    pub later: BTreeMap<usize, u64>,
    /// Pop order, run-length: `(phase, kind, id, count)`.
    pub order: Vec<(u32, PCensusKind, u64, u64)>,
    pub owners: BTreeMap<u64, PCensusOwner>,
    pub phases: Vec<PCensusPhase>,
    /// Stepped Apply keys `(class, view)` when kept, in step order.
    pub apply_keys: Vec<(u64, Box<[i64]>)>,
    /// Routed Route steps per `(presentation, support)` and numerator part
    /// (M5 maps): keys, transport terms, step time.
    pub routes: BTreeMap<(u32, u64), BTreeMap<Box<[i64]>, PRouteAgg>>,
    pub popped: u64,
    pub repeats: u64,
    pub phase_regressions: u64,
    pub cache_hits: u64,
    /// Per-rule hit counts of Custom owners (M5 artefact v2, spec item 8):
    /// `(owner class, chart, rule position in the chart's application order,
    /// phase)` -> steps that applied it.
    pub rule_hits: BTreeMap<(u64, u64, u32, u32), u64>,
    /// Per-rule hit counts of Scheme owners (scheme-certs): `(owner class,
    /// rule position in the scheme's stored order (`u32::MAX`: the Custom
    /// fallback), phase)` -> steps that applied it.
    pub scheme_rule_hits: BTreeMap<(u64, u32, u32), u64>,
    seen: HashSet<u128>,
    last_phase: Option<usize>,
    started: Option<Instant>,
}

impl PPathCensus {
    pub fn new(options: PPathCensusOptions) -> Self {
        Self {
            options,
            steps: BTreeMap::new(),
            inflow: BTreeMap::new(),
            inflow_profile: BTreeMap::new(),
            later: BTreeMap::new(),
            order: Vec::new(),
            owners: BTreeMap::new(),
            phases: Vec::new(),
            apply_keys: Vec::new(),
            routes: BTreeMap::new(),
            popped: 0,
            repeats: 0,
            phase_regressions: 0,
            cache_hits: 0,
            rule_hits: BTreeMap::new(),
            scheme_rule_hits: BTreeMap::new(),
            seen: HashSet::new(),
            last_phase: None,
            started: None,
        }
    }

    /// Distinct popped keys.
    pub fn distinct(&self) -> u64 {
        self.seen.len() as u64
    }

    fn seconds(&mut self) -> f64 {
        self.started
            .get_or_insert_with(Instant::now)
            .elapsed()
            .as_secs_f64()
    }

    pub(crate) fn popped(
        &mut self,
        phase: usize,
        kind: PCensusKind,
        id: u64,
        hash: u128,
    ) -> Result<(), PError> {
        self.seconds();
        self.popped += 1;
        if !self.seen.insert(hash) {
            self.repeats += 1;
        }
        if self.last_phase.is_some_and(|last| phase < last) {
            self.phase_regressions += 1;
        }
        self.last_phase = Some(phase);
        let phase = u32::try_from(phase).unwrap_or(u32::MAX);
        match self.order.last_mut() {
            Some(last) if last.0 == phase && last.1 == kind && last.2 == id => last.3 += 1,
            _ => self.order.push((phase, kind, id, 1)),
        }
        match self.options.max_keys {
            Some(cap) if self.popped > cap => Err(PError::Unsupported(format!(
                "path census: more than {cap} keys popped"
            ))),
            _ => Ok(()),
        }
    }

    pub(crate) fn stepped(&mut self, record: PStepRecord) {
        self.steps.entry(record.key).or_default().add(&record.agg);
        if let Some((chart, rule)) = record.rule {
            *(self.rule_hits)
                .entry((record.key.2, chart, rule, record.key.0))
                .or_default() += 1;
        }
        if let Some(rule) = record.scheme_rule {
            *(self.scheme_rule_hits)
                .entry((record.key.2, rule, record.key.0))
                .or_default() += 1;
        }
        if let Some(view) = record.apply_view {
            self.apply_keys.push((record.key.2, view));
        }
        if let Some(route) = record.route {
            let agg = self
                .routes
                .entry((route.presentation, route.support))
                .or_default()
                .entry(route.numerator)
                .or_default();
            agg.keys += 1;
            agg.terms += route.terms;
            agg.nanos += record.agg.nanos;
        }
    }

    pub(crate) fn cache_hit(&mut self) {
        self.cache_hits += 1;
    }

    pub(crate) fn phase_boundary(
        &mut self,
        phase: usize,
        started: bool,
        progress: CandidateOrderedReductionProgress,
    ) {
        let seconds = self.seconds();
        self.phases.push(PCensusPhase {
            phase,
            started,
            seconds,
            progress,
        });
    }

    pub(crate) fn phase_inflow(
        &mut self,
        phase: usize,
        inflow: BTreeMap<(PCensusKind, u64), u64>,
        profile: BTreeMap<(u32, i64, i64), u64>,
        later: u64,
    ) {
        self.inflow.insert(phase, inflow);
        self.inflow_profile.insert(phase, profile);
        self.later.insert(phase, later);
    }

    /// The T-BFS certificate of what the census saw.
    pub fn certificate(&self) -> PBfsCertificate {
        let mut runs: BTreeMap<(u32, u64), u64> = BTreeMap::new();
        let mut apply_phases: HashSet<u32> = HashSet::new();
        for &(phase, kind, id, _) in &self.order {
            if kind == 1 {
                *runs.entry((phase, id)).or_default() += 1;
                apply_phases.insert(phase);
            }
        }
        let split: HashSet<u32> = runs
            .iter()
            .filter(|&(_, &count)| count > 1)
            .map(|(&(phase, _), _)| phase)
            .collect();
        let (earlier, foreign) = self.steps.values().fold((0, 0), |(e, f), agg| {
            (e + agg.ch_earlier_phase, f + agg.ch_foreign_same_phase)
        });
        PBfsCertificate {
            popped: self.popped,
            distinct: self.distinct(),
            repeats: self.repeats,
            phase_regressions: self.phase_regressions,
            split_owner_phases: split.len() as u64,
            apply_phases: apply_phases.len() as u64,
            earlier_phase_children: earlier,
            foreign_same_phase_children: foreign,
            cache_hits: self.cache_hits,
        }
    }
}
